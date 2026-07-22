//! 盯盘助手：规则评估、交易时段、后台轮询与系统通知。

mod engine;
mod rules;
mod service;
mod session;
mod shared;

#[cfg(target_os = "android")]
mod headless_jni;

pub use engine::{EngineState, FiredAlert, evaluate};
pub use rules::{AlertCondition, MonitorAlert, MonitorRule, condition_label};
pub use service::MonitorBackgroundService;
pub use session::is_trading_session;
pub use shared::{MonitorShared, MonitorSnapshot, default_interval_secs};

use std::sync::Arc;
use tokio::sync::Mutex;

pub type SharedMonitor = Arc<Mutex<MonitorShared>>;

pub fn new_shared() -> SharedMonitor {
    Arc::new(Mutex::new(MonitorShared::default()))
}
