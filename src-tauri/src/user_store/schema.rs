//! Schema 创建与正向迁移。

use rusqlite::{params, Connection};

use super::set_schema_version;

pub const CURRENT_SCHEMA_VERSION: i64 = 3;

pub const GROUP_WATCH: &str = "g_watch";
pub const GROUP_BUY: &str = "g_buy";
pub const GROUP_OBSERVE: &str = "g_observe";
pub const GROUP_REMOVED: &str = "g_removed";
pub const GROUP_HOLDINGS_MIRROR: &str = "g_holdings_mirror";

const META_POOL_MIGRATED: &str = "pool_migrated_from_watchlist";

const DDL_V2: &str = r#"
CREATE TABLE IF NOT EXISTS pool_groups (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    sort_order INTEGER NOT NULL,
    kind TEXT NOT NULL DEFAULT 'user'
);
CREATE TABLE IF NOT EXISTS pool_items (
    code TEXT NOT NULL,
    group_id TEXT NOT NULL,
    name TEXT NOT NULL DEFAULT '',
    market TEXT NOT NULL DEFAULT '',
    sector TEXT NOT NULL DEFAULT '',
    sort_order INTEGER NOT NULL DEFAULT 0,
    extra_json TEXT NOT NULL DEFAULT '{}',
    PRIMARY KEY (code, group_id)
);
"#;

const DDL_V3: &str = r#"
CREATE TABLE IF NOT EXISTS holdings (
    code TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL DEFAULT '',
    market TEXT NOT NULL DEFAULT '',
    sector TEXT NOT NULL DEFAULT '',
    cost REAL NOT NULL,
    qty REAL NOT NULL,
    buy_date TEXT NOT NULL,
    note TEXT NOT NULL DEFAULT ''
);
CREATE TABLE IF NOT EXISTS journal_entries (
    id TEXT PRIMARY KEY NOT NULL,
    date TEXT NOT NULL,
    code TEXT,
    body TEXT NOT NULL,
    extra_json TEXT NOT NULL DEFAULT '{}'
);
"#;

pub fn ensure_schema(conn: &mut Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS meta (
            key TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS watchlist (
            code TEXT PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            market TEXT NOT NULL,
            sector TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            extra_json TEXT NOT NULL DEFAULT '{}'
        );
        CREATE TABLE IF NOT EXISTS kv_settings (
            key TEXT PRIMARY KEY NOT NULL,
            value_json TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS strategy_compose (
            stock_code TEXT PRIMARY KEY NOT NULL,
            compose_json TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS monitor_rules (
            id TEXT PRIMARY KEY NOT NULL,
            rule_json TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS monitor_alerts (
            id TEXT PRIMARY KEY NOT NULL,
            alert_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .map_err(|e| format!("创建 schema 失败: {e}"))?;

    conn.execute_batch(DDL_V2)
        .map_err(|e| format!("创建 pool schema 失败: {e}"))?;
    conn.execute_batch(DDL_V3)
        .map_err(|e| format!("创建 holdings/journal schema 失败: {e}"))?;

    let ver: Option<String> = conn
        .query_row(
            "SELECT value FROM meta WHERE key = 'schema_version'",
            [],
            |r| r.get(0),
        )
        .ok();
    if ver.is_none() {
        // 全新库：直接落到 CURRENT，并种默认分组
        seed_default_groups(conn)?;
        set_schema_version(conn, CURRENT_SCHEMA_VERSION)?;
    }
    Ok(())
}

/// 从 `from` 迁到下一版本；`ensure_migrated` 会循环直到 CURRENT。
pub fn migrate(conn: &mut Connection, from: i64) -> Result<(), String> {
    if from >= CURRENT_SCHEMA_VERSION {
        return Ok(());
    }
    if from == 0 {
        // 空库 / 仅有表无版本：ensure_schema 已建表，标为 v1 后由循环继续
        set_schema_version(conn, 1)?;
        return Ok(());
    }
    if from == 1 {
        conn.execute_batch(DDL_V2)
            .map_err(|e| format!("迁移 v2 建表失败: {e}"))?;
        seed_default_groups(conn)?;
        migrate_watchlist_to_pool(conn)?;
        set_schema_version(conn, 2)?;
        return Ok(());
    }
    if from == 2 {
        conn.execute_batch(DDL_V3)
            .map_err(|e| format!("迁移 v3 建表失败: {e}"))?;
        set_schema_version(conn, 3)?;
        return Ok(());
    }
    Err(format!(
        "不支持的 schema 迁移: {from} → {CURRENT_SCHEMA_VERSION}"
    ))
}

pub fn seed_default_groups(conn: &Connection) -> Result<(), String> {
    let defaults: [(&str, &str, i64, &str); 5] = [
        (GROUP_WATCH, "关注", 0, "user"),
        (GROUP_BUY, "待买", 1, "user"),
        (GROUP_OBSERVE, "观察", 2, "user"),
        (GROUP_REMOVED, "已剔除", 3, "user"),
        (GROUP_HOLDINGS_MIRROR, "持仓镜像", 99, "mirror"),
    ];
    for (id, name, sort, kind) in defaults {
        conn.execute(
            "INSERT INTO pool_groups(id, name, sort_order, kind) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO NOTHING",
            params![id, name, sort, kind],
        )
        .map_err(|e| format!("写入默认分组失败: {e}"))?;
    }
    Ok(())
}

fn migrate_watchlist_to_pool(conn: &Connection) -> Result<(), String> {
    conn.execute(
        r#"
        INSERT INTO pool_items(code, group_id, name, market, sector, sort_order, extra_json)
        SELECT code, ?1, name, market, sector, sort_order, extra_json
        FROM watchlist
        WHERE NOT EXISTS (
            SELECT 1 FROM pool_items p
            WHERE p.code = watchlist.code AND p.group_id = ?1
        )
        "#,
        params![GROUP_WATCH],
    )
    .map_err(|e| format!("watchlist → pool 迁移失败: {e}"))?;

    conn.execute(
        "INSERT INTO meta(key, value) VALUES (?1, '1')
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![META_POOL_MIGRATED],
    )
    .map_err(|e| format!("写 pool 迁移标记失败: {e}"))?;
    Ok(())
}
