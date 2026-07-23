//! Schema 创建与正向迁移。

use rusqlite::Connection;

use super::set_schema_version;

pub const CURRENT_SCHEMA_VERSION: i64 = 1;

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

    let ver: Option<String> = conn
        .query_row(
            "SELECT value FROM meta WHERE key = 'schema_version'",
            [],
            |r| r.get(0),
        )
        .ok();
    if ver.is_none() {
        set_schema_version(conn, CURRENT_SCHEMA_VERSION)?;
    }
    Ok(())
}

/// 从 `from` 迁到 CURRENT。当前仅 v1 基线；后续在此追加 `from == N` 分支。
pub fn migrate(conn: &mut Connection, from: i64) -> Result<(), String> {
    if from >= CURRENT_SCHEMA_VERSION {
        return Ok(());
    }
    if from == 0 {
        // 空库 / 仅有表无版本：ensure_schema 已建表，直接标为 v1
        set_schema_version(conn, 1)?;
        return Ok(());
    }
    Err(format!(
        "不支持的 schema 迁移: {from} → {CURRENT_SCHEMA_VERSION}"
    ))
}
