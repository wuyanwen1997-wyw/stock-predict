//! 统一用户数据目录：优先 `STOCK_PREDICT_DATA`，否则 Tauri `app_data_dir`。
//! 旧版 `%LOCALAPPDATA%/stock-predict` / `~/.stock-predict` 在首次启动时拷贝兼容。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static APP_DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// 由应用启动时注入 Tauri `app_data_dir`。
pub fn init_app_data_dir(dir: PathBuf) {
    let _ = APP_DATA_DIR.set(dir);
}

/// 用户态根目录（Token、SQLite、cache 均在其下）。
pub fn app_data_dir() -> PathBuf {
    if let Ok(p) = std::env::var("STOCK_PREDICT_DATA") {
        let p = PathBuf::from(p.trim());
        if !p.as_os_str().is_empty() {
            return p;
        }
    }
    if let Some(p) = APP_DATA_DIR.get() {
        return p.clone();
    }
    legacy_data_dir()
}

pub fn cache_dir() -> PathBuf {
    app_data_dir().join("cache")
}

pub fn backups_dir() -> PathBuf {
    app_data_dir().join("backups")
}

pub fn user_db_path() -> PathBuf {
    app_data_dir().join("user_data.sqlite")
}

pub fn token_path() -> PathBuf {
    app_data_dir().join("tushare_token.txt")
}

pub fn fund_flow_cache_path() -> PathBuf {
    cache_dir().join("market_fund_flow.json")
}

/// 升级前旧版自定义目录。
pub fn legacy_data_dir() -> PathBuf {
    #[cfg(windows)]
    {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            return PathBuf::from(local).join("stock-predict");
        }
    }
    #[cfg(not(windows))]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(".stock-predict");
        }
    }
    std::env::temp_dir().join("stock-predict")
}

fn copy_if_missing(from: &Path, to: &Path) {
    if to.exists() || !from.exists() {
        return;
    }
    if let Some(parent) = to.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::copy(from, to);
}

/// 新目录缺 Token/缓存时，从旧目录或根下旧缓存路径拷贝一次。
pub fn migrate_legacy_files_if_needed() {
    let root = app_data_dir();
    let _ = fs::create_dir_all(&root);
    let _ = fs::create_dir_all(cache_dir());

    let legacy = legacy_data_dir();
    if legacy != root {
        copy_if_missing(
            &legacy.join("tushare_token.txt"),
            &token_path(),
        );
        copy_if_missing(
            &legacy.join("market_fund_flow.json"),
            &fund_flow_cache_path(),
        );
        copy_if_missing(
            &legacy.join("cache").join("market_fund_flow.json"),
            &fund_flow_cache_path(),
        );
        copy_if_missing(
            &legacy.join("user_data.sqlite"),
            &user_db_path(),
        );
    }

    // 同目录下旧版把缓存放在根部时，挪到 cache/
    copy_if_missing(
        &root.join("market_fund_flow.json"),
        &fund_flow_cache_path(),
    );
}
