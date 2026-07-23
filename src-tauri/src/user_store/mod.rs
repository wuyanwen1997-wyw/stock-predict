//! 用户态 SQLite：自选 / 策略 / 设置 / 盯盘。安装目录外持久化，带 schema 迁移与备份。

mod schema;

use crate::models::Stock;
use crate::monitor::{MonitorAlert, MonitorRule};
use crate::paths::{self, backups_dir, user_db_path};
use crate::strategy::StrategyCompose;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Mutex;

pub use schema::{
    CURRENT_SCHEMA_VERSION, GROUP_BUY, GROUP_HOLDINGS_MIRROR, GROUP_OBSERVE, GROUP_REMOVED,
    GROUP_WATCH,
};

const META_SCHEMA: &str = "schema_version";
const META_LS_MIGRATED: &str = "localstorage_migrated";
const MAX_ALERTS: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UserSettings {
    #[serde(default)]
    pub lookback_days: Option<u32>,
    #[serde(default)]
    pub predict_mode: Option<String>,
    #[serde(default)]
    pub predict_horizon_days: Option<u32>,
    #[serde(default)]
    pub screen_compose: Option<StrategyCompose>,
    #[serde(default)]
    pub monitor_enabled: Option<bool>,
    #[serde(default)]
    pub predict_compose_collapsed: Option<bool>,
    #[serde(default)]
    pub predict_compose_height: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PoolGroup {
    pub id: String,
    pub name: String,
    pub sort_order: i64,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PoolItem {
    pub code: String,
    pub group_id: String,
    pub name: String,
    pub market: String,
    pub sector: String,
    pub sort_order: i64,
    #[serde(default = "default_extra_json")]
    pub extra_json: String,
}

fn default_extra_json() -> String {
    "{}".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Holding {
    pub code: String,
    pub name: String,
    pub market: String,
    pub sector: String,
    pub cost: f64,
    pub qty: f64,
    pub buy_date: String,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct JournalEntry {
    pub id: String,
    pub date: String,
    #[serde(default)]
    pub code: Option<String>,
    pub body: String,
    #[serde(default = "default_extra_json")]
    pub extra_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UserDataSnapshot {
    #[serde(default)]
    pub watchlist: Vec<Stock>,
    #[serde(default)]
    pub strategy_map: HashMap<String, StrategyCompose>,
    #[serde(default)]
    pub settings: UserSettings,
    #[serde(default)]
    pub monitor_rules: Vec<MonitorRule>,
    #[serde(default)]
    pub monitor_alerts: Vec<MonitorAlert>,
    #[serde(default)]
    pub pool_groups: Vec<PoolGroup>,
    #[serde(default)]
    pub pool_items: Vec<PoolItem>,
    #[serde(default)]
    pub holdings: Vec<Holding>,
    #[serde(default)]
    pub journal_entries: Vec<JournalEntry>,
    pub schema_version: i64,
    pub localstorage_migrated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LegacyLocalStoragePayload {
    #[serde(default)]
    pub watchlist: Option<Value>,
    #[serde(default)]
    pub watchlist_legacy: Option<Value>,
    #[serde(default)]
    pub strategy_map: Option<HashMap<String, StrategyCompose>>,
    #[serde(default)]
    pub settings: UserSettings,
    #[serde(default)]
    pub monitor_rules: Option<Vec<MonitorRule>>,
    #[serde(default)]
    pub monitor_alerts: Option<Vec<MonitorAlert>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDbStatus {
    pub schema_version: i64,
    pub path: String,
    pub needs_localstorage_import: bool,
    pub has_user_rows: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportBundle {
    pub format_version: u32,
    pub exported_at: String,
    pub data: UserDataSnapshot,
}

pub struct UserStore {
    conn: Mutex<Connection>,
}

impl UserStore {
    pub fn open(data_dir: &Path) -> Result<Self, String> {
        fs::create_dir_all(data_dir).map_err(|e| format!("创建数据目录失败: {e}"))?;
        fs::create_dir_all(backups_dir()).map_err(|e| format!("创建备份目录失败: {e}"))?;
        let db_path = data_dir.join("user_data.sqlite");
        let conn = Connection::open(&db_path).map_err(|e| format!("打开用户库失败: {e}"))?;
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")
            .map_err(|e| format!("PRAGMA 失败: {e}"))?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.ensure_migrated()?;
        Ok(store)
    }

    pub fn status(&self) -> Result<UserDbStatus, String> {
        let conn = self.lock()?;
        let schema_version = meta_i64(&conn, META_SCHEMA)?.unwrap_or(0);
        let migrated = meta_bool(&conn, META_LS_MIGRATED)?.unwrap_or(false);
        let has_user_rows = count_user_rows(&conn)? > 0;
        Ok(UserDbStatus {
            schema_version,
            path: user_db_path().display().to_string(),
            needs_localstorage_import: !migrated && !has_user_rows,
            has_user_rows,
        })
    }

    pub fn load(&self) -> Result<UserDataSnapshot, String> {
        let conn = self.lock()?;
        let pool_groups = load_pool_groups(&conn)?;
        let pool_items = load_pool_items(&conn)?;
        let watchlist = {
            let from_pool = pool_items_to_stocks(&pool_items, GROUP_WATCH);
            if !from_pool.is_empty() {
                from_pool
            } else {
                load_watchlist_table(&conn)?
            }
        };
        Ok(UserDataSnapshot {
            watchlist,
            strategy_map: load_strategy_map(&conn)?,
            settings: load_settings(&conn)?,
            monitor_rules: load_monitor_rules(&conn)?,
            monitor_alerts: load_monitor_alerts(&conn)?,
            pool_groups,
            pool_items,
            holdings: load_holdings(&conn)?,
            journal_entries: load_journal_entries(&conn)?,
            schema_version: meta_i64(&conn, META_SCHEMA)?.unwrap_or(CURRENT_SCHEMA_VERSION),
            localstorage_migrated: meta_bool(&conn, META_LS_MIGRATED)?.unwrap_or(false),
        })
    }

    /// 兼容旧 API：写入「关注」分组（`g_watch`），不再改写 watchlist 表。
    pub fn save_watchlist(&self, items: &[Stock]) -> Result<(), String> {
        let pool_items: Vec<PoolItem> = items
            .iter()
            .enumerate()
            .map(|(i, s)| PoolItem {
                code: s.code.clone(),
                group_id: GROUP_WATCH.to_string(),
                name: s.name.clone(),
                market: s.market.clone(),
                sector: s.sector.clone(),
                sort_order: i as i64,
                extra_json: serde_json::json!({
                    "price": s.price,
                    "change_pct": s.change_pct,
                    "is_hot": s.is_hot,
                })
                .to_string(),
            })
            .collect();
        let conn = self.lock()?;
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| format!("事务失败: {e}"))?;
        schema::seed_default_groups(&tx)?;
        tx.execute(
            "DELETE FROM pool_items WHERE group_id = ?1",
            params![GROUP_WATCH],
        )
        .map_err(|e| format!("清空关注分组失败: {e}"))?;
        for item in &pool_items {
            insert_pool_item(&tx, item)?;
        }
        tx.commit().map_err(|e| format!("提交自选失败: {e}"))?;
        Ok(())
    }

    pub fn save_pool(&self, groups: &[PoolGroup], items: &[PoolItem]) -> Result<(), String> {
        let conn = self.lock()?;
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| format!("事务失败: {e}"))?;
        tx.execute("DELETE FROM pool_items", [])
            .map_err(|e| format!("清空股票池条目失败: {e}"))?;
        tx.execute("DELETE FROM pool_groups", [])
            .map_err(|e| format!("清空股票池分组失败: {e}"))?;
        if groups.is_empty() {
            schema::seed_default_groups(&tx)?;
        } else {
            for g in groups {
                tx.execute(
                    "INSERT INTO pool_groups(id, name, sort_order, kind) VALUES (?1, ?2, ?3, ?4)",
                    params![g.id, g.name, g.sort_order, g.kind],
                )
                .map_err(|e| format!("写入股票池分组失败: {e}"))?;
            }
        }
        for item in items {
            insert_pool_item(&tx, item)?;
        }
        tx.commit().map_err(|e| format!("提交股票池失败: {e}"))?;
        Ok(())
    }

    pub fn save_holdings(&self, items: &[Holding]) -> Result<(), String> {
        let conn = self.lock()?;
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| format!("事务失败: {e}"))?;
        tx.execute("DELETE FROM holdings", [])
            .map_err(|e| format!("清空持仓失败: {e}"))?;
        for h in items {
            tx.execute(
                "INSERT INTO holdings(code, name, market, sector, cost, qty, buy_date, note)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    h.code,
                    h.name,
                    h.market,
                    h.sector,
                    h.cost,
                    h.qty,
                    h.buy_date,
                    h.note
                ],
            )
            .map_err(|e| format!("写入持仓失败: {e}"))?;
        }
        tx.commit().map_err(|e| format!("提交持仓失败: {e}"))?;
        Ok(())
    }

    pub fn save_journal(&self, entries: &[JournalEntry]) -> Result<(), String> {
        let conn = self.lock()?;
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| format!("事务失败: {e}"))?;
        tx.execute("DELETE FROM journal_entries", [])
            .map_err(|e| format!("清空复盘失败: {e}"))?;
        for e in entries {
            tx.execute(
                "INSERT INTO journal_entries(id, date, code, body, extra_json)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![e.id, e.date, e.code, e.body, e.extra_json],
            )
            .map_err(|e| format!("写入复盘失败: {e}"))?;
        }
        tx.commit().map_err(|e| format!("提交复盘失败: {e}"))?;
        Ok(())
    }

    pub fn save_strategy_map(
        &self,
        map: &HashMap<String, StrategyCompose>,
    ) -> Result<(), String> {
        let conn = self.lock()?;
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| format!("事务失败: {e}"))?;
        tx.execute("DELETE FROM strategy_compose", [])
            .map_err(|e| format!("清空策略失败: {e}"))?;
        for (code, compose) in map {
            let json = serde_json::to_string(compose)
                .map_err(|e| format!("序列化策略失败: {e}"))?;
            tx.execute(
                "INSERT INTO strategy_compose(stock_code, compose_json) VALUES (?1, ?2)",
                params![normalize_code(code), json],
            )
            .map_err(|e| format!("写入策略失败: {e}"))?;
        }
        tx.commit().map_err(|e| format!("提交策略失败: {e}"))?;
        Ok(())
    }

    pub fn save_settings(&self, settings: &UserSettings) -> Result<(), String> {
        let conn = self.lock()?;
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| format!("事务失败: {e}"))?;
        upsert_setting_opt(&tx, "lookback_days", &settings.lookback_days)?;
        upsert_setting_opt(&tx, "predict_mode", &settings.predict_mode)?;
        upsert_setting_opt(&tx, "predict_horizon_days", &settings.predict_horizon_days)?;
        upsert_setting_opt(&tx, "screen_compose", &settings.screen_compose)?;
        upsert_setting_opt(&tx, "monitor_enabled", &settings.monitor_enabled)?;
        upsert_setting_opt(
            &tx,
            "predict_compose_collapsed",
            &settings.predict_compose_collapsed,
        )?;
        upsert_setting_opt(
            &tx,
            "predict_compose_height",
            &settings.predict_compose_height,
        )?;
        tx.commit().map_err(|e| format!("提交设置失败: {e}"))?;
        Ok(())
    }

    pub fn save_monitor_rules(&self, rules: &[MonitorRule]) -> Result<(), String> {
        let conn = self.lock()?;
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| format!("事务失败: {e}"))?;
        tx.execute("DELETE FROM monitor_rules", [])
            .map_err(|e| format!("清空规则失败: {e}"))?;
        for r in rules {
            let json =
                serde_json::to_string(r).map_err(|e| format!("序列化规则失败: {e}"))?;
            tx.execute(
                "INSERT INTO monitor_rules(id, rule_json) VALUES (?1, ?2)",
                params![r.id, json],
            )
            .map_err(|e| format!("写入规则失败: {e}"))?;
        }
        tx.commit().map_err(|e| format!("提交规则失败: {e}"))?;
        Ok(())
    }

    pub fn save_monitor_alerts(&self, alerts: &[MonitorAlert]) -> Result<(), String> {
        let conn = self.lock()?;
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| format!("事务失败: {e}"))?;
        tx.execute("DELETE FROM monitor_alerts", [])
            .map_err(|e| format!("清空预警失败: {e}"))?;
        for a in alerts.iter().take(MAX_ALERTS) {
            let json =
                serde_json::to_string(a).map_err(|e| format!("序列化预警失败: {e}"))?;
            let created = a.fired_at.to_rfc3339();
            tx.execute(
                "INSERT INTO monitor_alerts(id, alert_json, created_at) VALUES (?1, ?2, ?3)",
                params![a.id, json, created],
            )
            .map_err(|e| format!("写入预警失败: {e}"))?;
        }
        tx.commit().map_err(|e| format!("提交预警失败: {e}"))?;
        Ok(())
    }

    /// 一次性从 localStorage 导入；若库已有用户行或已标记迁移则跳过。
    pub fn import_from_localstorage(
        &self,
        payload: LegacyLocalStoragePayload,
    ) -> Result<UserDataSnapshot, String> {
        {
            let conn = self.lock()?;
            let migrated = meta_bool(&conn, META_LS_MIGRATED)?.unwrap_or(false);
            let has_user_rows = count_user_rows(&conn)? > 0;
            if migrated || has_user_rows {
                set_meta_bool(&conn, META_LS_MIGRATED, true)?;
                drop(conn);
                return self.load();
            }
        }

        let watchlist = parse_watchlist_value(
            payload
                .watchlist
                .as_ref()
                .or(payload.watchlist_legacy.as_ref()),
        );
        let strategy_map = payload.strategy_map.unwrap_or_default();
        let rules = payload.monitor_rules.unwrap_or_default();
        let alerts = payload.monitor_alerts.unwrap_or_default();

        if !watchlist.is_empty() {
            self.save_watchlist(&watchlist)?;
        }
        if !strategy_map.is_empty() {
            self.save_strategy_map(&strategy_map)?;
        }
        self.save_settings(&payload.settings)?;
        if !rules.is_empty() {
            self.save_monitor_rules(&rules)?;
        }
        if !alerts.is_empty() {
            self.save_monitor_alerts(&alerts)?;
        }

        {
            let conn = self.lock()?;
            set_meta_bool(&conn, META_LS_MIGRATED, true)?;
        }
        self.load()
    }

    pub fn mark_localstorage_migrated(&self) -> Result<(), String> {
        let conn = self.lock()?;
        set_meta_bool(&conn, META_LS_MIGRATED, true)
    }

    pub fn export_json(&self) -> Result<String, String> {
        let data = self.load()?;
        let bundle = ExportBundle {
            format_version: CURRENT_SCHEMA_VERSION as u32,
            exported_at: chrono::Local::now().to_rfc3339(),
            data,
        };
        serde_json::to_string_pretty(&bundle).map_err(|e| format!("导出序列化失败: {e}"))
    }

    pub fn import_json(&self, json: &str) -> Result<UserDataSnapshot, String> {
        let bundle: ExportBundle = serde_json::from_str(json)
            .or_else(|_| {
                // 兼容裸 snapshot
                serde_json::from_str::<UserDataSnapshot>(json).map(|data| ExportBundle {
                    format_version: 1,
                    exported_at: chrono::Local::now().to_rfc3339(),
                    data,
                })
            })
            .map_err(|e| format!("导入 JSON 无效: {e}"))?;

        self.backup_db("pre_import")?;
        self.replace_all(&bundle.data)?;
        {
            let conn = self.lock()?;
            set_meta_bool(&conn, META_LS_MIGRATED, true)?;
        }
        self.load()
    }

    fn replace_all(&self, data: &UserDataSnapshot) -> Result<(), String> {
        self.save_pool(&data.pool_groups, &data.pool_items)?;
        self.save_holdings(&data.holdings)?;
        self.save_journal(&data.journal_entries)?;
        // 旧 format（无 pool）或 pool 关注为空时，用 watchlist 回填 g_watch
        let has_watch_pool = data
            .pool_items
            .iter()
            .any(|i| i.group_id == GROUP_WATCH);
        if !has_watch_pool && !data.watchlist.is_empty() {
            self.save_watchlist(&data.watchlist)?;
        }
        self.save_strategy_map(&data.strategy_map)?;
        self.save_settings(&data.settings)?;
        self.save_monitor_rules(&data.monitor_rules)?;
        self.save_monitor_alerts(&data.monitor_alerts)?;
        Ok(())
    }

    fn ensure_migrated(&self) -> Result<(), String> {
        let mut conn = self.lock()?;
        schema::ensure_schema(&mut conn)?;
        let ver = meta_i64(&conn, META_SCHEMA)?.unwrap_or(0);
        if ver > CURRENT_SCHEMA_VERSION {
            return Err(format!(
                "用户库 schema_version={ver} 高于本应用支持的 {CURRENT_SCHEMA_VERSION}，请升级应用或从备份恢复"
            ));
        }
        drop(conn);
        while {
            let conn = self.lock()?;
            meta_i64(&conn, META_SCHEMA)?.unwrap_or(0) < CURRENT_SCHEMA_VERSION
        } {
            self.backup_db("pre_migrate")?;
            let mut conn = self.lock()?;
            let from = meta_i64(&conn, META_SCHEMA)?.unwrap_or(0);
            schema::migrate(&mut conn, from)?;
        }
        Ok(())
    }

    fn backup_db(&self, reason: &str) -> Result<(), String> {
        let src = user_db_path();
        if !src.exists() {
            return Ok(());
        }
        let dir = backups_dir();
        fs::create_dir_all(&dir).map_err(|e| format!("创建备份目录失败: {e}"))?;
        let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let dest = dir.join(format!("user_data_{reason}_{stamp}.sqlite"));
        // Checkpoint WAL so copy is consistent enough for recovery
        {
            let conn = self.lock()?;
            let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        }
        fs::copy(&src, &dest).map_err(|e| format!("备份用户库失败: {e}"))?;
        Ok(())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, String> {
        self.conn
            .lock()
            .map_err(|_| "用户库锁损坏".to_string())
    }
}

fn normalize_code(code: &str) -> String {
    let digits: String = code.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() >= 6 {
        digits[digits.len() - 6..].to_string()
    } else if !digits.is_empty() {
        format!("{:0>6}", digits)
    } else {
        code.trim().to_string()
    }
}

fn count_user_rows(conn: &Connection) -> Result<usize, String> {
    let w: i64 = conn
        .query_row("SELECT COUNT(*) FROM watchlist", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let p: i64 = conn
        .query_row("SELECT COUNT(*) FROM pool_items", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let h: i64 = conn
        .query_row("SELECT COUNT(*) FROM holdings", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let s: i64 = conn
        .query_row("SELECT COUNT(*) FROM strategy_compose", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let k: i64 = conn
        .query_row("SELECT COUNT(*) FROM kv_settings", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let r: i64 = conn
        .query_row("SELECT COUNT(*) FROM monitor_rules", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    Ok((w + p + h + s + k + r) as usize)
}

fn meta_i64(conn: &Connection, key: &str) -> Result<Option<i64>, String> {
    conn.query_row(
        "SELECT value FROM meta WHERE key = ?1",
        params![key],
        |r| {
            let s: String = r.get(0)?;
            Ok(s.parse::<i64>().ok())
        },
    )
    .optional()
    .map_err(|e| e.to_string())
    .map(|o| o.flatten())
}

fn meta_bool(conn: &Connection, key: &str) -> Result<Option<bool>, String> {
    conn.query_row(
        "SELECT value FROM meta WHERE key = ?1",
        params![key],
        |r| {
            let s: String = r.get(0)?;
            Ok(s == "1" || s.eq_ignore_ascii_case("true"))
        },
    )
    .optional()
    .map_err(|e| e.to_string())
}

fn set_meta(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO meta(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(|e| format!("写 meta 失败: {e}"))?;
    Ok(())
}

fn set_meta_bool(conn: &Connection, key: &str, v: bool) -> Result<(), String> {
    set_meta(conn, key, if v { "1" } else { "0" })
}

pub(crate) fn set_schema_version(conn: &Connection, v: i64) -> Result<(), String> {
    set_meta(conn, META_SCHEMA, &v.to_string())
}

fn load_watchlist_table(conn: &Connection) -> Result<Vec<Stock>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT code, name, market, sector, extra_json FROM watchlist ORDER BY sort_order ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            let extra_raw: String = r.get(4)?;
            let extra: Value =
                serde_json::from_str(&extra_raw).unwrap_or(Value::Object(Default::default()));
            Ok(Stock {
                code: r.get(0)?,
                name: r.get(1)?,
                market: r.get(2)?,
                sector: r.get(3)?,
                price: extra.get("price").and_then(|v| v.as_f64()),
                change_pct: extra.get("change_pct").and_then(|v| v.as_f64()),
                is_hot: extra
                    .get("is_hot")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

fn load_pool_groups(conn: &Connection) -> Result<Vec<PoolGroup>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, sort_order, kind FROM pool_groups ORDER BY sort_order ASC, id ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(PoolGroup {
                id: r.get(0)?,
                name: r.get(1)?,
                sort_order: r.get(2)?,
                kind: r.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

fn load_pool_items(conn: &Connection) -> Result<Vec<PoolItem>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT code, group_id, name, market, sector, sort_order, extra_json
             FROM pool_items ORDER BY group_id ASC, sort_order ASC, code ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(PoolItem {
                code: r.get(0)?,
                group_id: r.get(1)?,
                name: r.get(2)?,
                market: r.get(3)?,
                sector: r.get(4)?,
                sort_order: r.get(5)?,
                extra_json: r.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

fn load_holdings(conn: &Connection) -> Result<Vec<Holding>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT code, name, market, sector, cost, qty, buy_date, note
             FROM holdings ORDER BY buy_date ASC, code ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Holding {
                code: r.get(0)?,
                name: r.get(1)?,
                market: r.get(2)?,
                sector: r.get(3)?,
                cost: r.get(4)?,
                qty: r.get(5)?,
                buy_date: r.get(6)?,
                note: r.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

fn load_journal_entries(conn: &Connection) -> Result<Vec<JournalEntry>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, date, code, body, extra_json
             FROM journal_entries ORDER BY date DESC, id ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(JournalEntry {
                id: r.get(0)?,
                date: r.get(1)?,
                code: r.get(2)?,
                body: r.get(3)?,
                extra_json: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

fn insert_pool_item(conn: &Connection, item: &PoolItem) -> Result<(), String> {
    conn.execute(
        "INSERT INTO pool_items(code, group_id, name, market, sector, sort_order, extra_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            item.code,
            item.group_id,
            item.name,
            item.market,
            item.sector,
            item.sort_order,
            item.extra_json
        ],
    )
    .map_err(|e| format!("写入股票池条目失败: {e}"))?;
    Ok(())
}

fn pool_items_to_stocks(items: &[PoolItem], group_id: &str) -> Vec<Stock> {
    let mut filtered: Vec<&PoolItem> = items
        .iter()
        .filter(|i| i.group_id == group_id)
        .collect();
    filtered.sort_by_key(|i| (i.sort_order, i.code.as_str()));
    filtered
        .into_iter()
        .map(|i| {
            let extra: Value =
                serde_json::from_str(&i.extra_json).unwrap_or(Value::Object(Default::default()));
            Stock {
                code: i.code.clone(),
                name: i.name.clone(),
                market: i.market.clone(),
                sector: i.sector.clone(),
                price: extra.get("price").and_then(|v| v.as_f64()),
                change_pct: extra.get("change_pct").and_then(|v| v.as_f64()),
                is_hot: extra
                    .get("is_hot")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            }
        })
        .collect()
}

fn load_strategy_map(conn: &Connection) -> Result<HashMap<String, StrategyCompose>, String> {
    let mut stmt = conn
        .prepare("SELECT stock_code, compose_json FROM strategy_compose")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            let code: String = r.get(0)?;
            let json: String = r.get(1)?;
            Ok((code, json))
        })
        .map_err(|e| e.to_string())?;
    let mut out = HashMap::new();
    for row in rows {
        let (code, json) = row.map_err(|e| e.to_string())?;
        if let Ok(c) = serde_json::from_str::<StrategyCompose>(&json) {
            out.insert(normalize_code(&code), c);
        }
    }
    Ok(out)
}

fn load_settings(conn: &Connection) -> Result<UserSettings, String> {
    let mut stmt = conn
        .prepare("SELECT key, value_json FROM kv_settings")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            let k: String = r.get(0)?;
            let v: String = r.get(1)?;
            Ok((k, v))
        })
        .map_err(|e| e.to_string())?;

    let mut settings = UserSettings::default();
    for row in rows {
        let (k, v) = row.map_err(|e| e.to_string())?;
        match k.as_str() {
            "lookback_days" => {
                settings.lookback_days = serde_json::from_str(&v).ok();
            }
            "predict_mode" => {
                settings.predict_mode = serde_json::from_str(&v).ok();
            }
            "predict_horizon_days" => {
                settings.predict_horizon_days = serde_json::from_str(&v).ok();
            }
            "screen_compose" => {
                settings.screen_compose = serde_json::from_str(&v).ok();
            }
            "monitor_enabled" => {
                settings.monitor_enabled = serde_json::from_str(&v).ok();
            }
            "predict_compose_collapsed" => {
                settings.predict_compose_collapsed = serde_json::from_str(&v).ok();
            }
            "predict_compose_height" => {
                settings.predict_compose_height = serde_json::from_str(&v).ok();
            }
            _ => {}
        }
    }
    Ok(settings)
}

fn load_monitor_rules(conn: &Connection) -> Result<Vec<MonitorRule>, String> {
    let mut stmt = conn
        .prepare("SELECT rule_json FROM monitor_rules")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            let json: String = r.get(0)?;
            Ok(json)
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        let json = row.map_err(|e| e.to_string())?;
        if let Ok(r) = serde_json::from_str::<MonitorRule>(&json) {
            out.push(r);
        }
    }
    Ok(out)
}

fn load_monitor_alerts(conn: &Connection) -> Result<Vec<MonitorAlert>, String> {
    let mut stmt = conn
        .prepare("SELECT alert_json FROM monitor_alerts ORDER BY created_at DESC LIMIT ?1")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![MAX_ALERTS as i64], |r| {
            let json: String = r.get(0)?;
            Ok(json)
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        let json = row.map_err(|e| e.to_string())?;
        if let Ok(a) = serde_json::from_str::<MonitorAlert>(&json) {
            out.push(a);
        }
    }
    Ok(out)
}

fn upsert_setting_opt<T: Serialize>(
    conn: &Connection,
    key: &str,
    value: &Option<T>,
) -> Result<(), String> {
    match value {
        Some(v) => {
            let json = serde_json::to_string(v).map_err(|e| e.to_string())?;
            conn.execute(
                "INSERT INTO kv_settings(key, value_json) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json",
                params![key, json],
            )
            .map_err(|e| format!("写设置 {key} 失败: {e}"))?;
        }
        None => {}
    }
    Ok(())
}

fn parse_watchlist_value(raw: Option<&Value>) -> Vec<Stock> {
    let Some(v) = raw else {
        return vec![];
    };
    if let Ok(stocks) = serde_json::from_value::<Vec<Stock>>(v.clone()) {
        return stocks;
    }
    // legacy: array of codes — cannot resolve names here; skip (FE recoverLegacy still helps pre-import)
    if let Some(arr) = v.as_array() {
        if arr.iter().all(|x| x.is_string()) {
            return arr
                .iter()
                .filter_map(|x| x.as_str())
                .map(|code| Stock {
                    code: code.to_string(),
                    name: code.to_string(),
                    market: String::new(),
                    sector: String::new(),
                    price: None,
                    change_pct: None,
                    is_hot: false,
                })
                .collect();
        }
    }
    vec![]
}

/// 启动时初始化路径并打开用户库。
pub fn bootstrap(app_data: &Path) -> Result<UserStore, String> {
    paths::init_app_data_dir(app_data.to_path_buf());
    paths::migrate_legacy_files_if_needed();
    UserStore::open(app_data)
}
