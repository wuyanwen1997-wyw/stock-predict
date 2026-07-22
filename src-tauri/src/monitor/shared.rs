use crate::models::Stock;
use crate::monitor::engine::EngineState;
use crate::monitor::rules::MonitorRule;
use serde::{Deserialize, Serialize};

pub const DEFAULT_INTERVAL_SECS: u64 = 15;

pub fn default_interval_secs() -> u64 {
    DEFAULT_INTERVAL_SECS
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MonitorSnapshot {
    pub stocks: Vec<Stock>,
    pub rules: Vec<MonitorRule>,
    pub interval_secs: u64,
    pub enabled: bool,
}

#[derive(Debug, Default)]
pub struct MonitorShared {
    pub stocks: Vec<Stock>,
    pub rules: Vec<MonitorRule>,
    pub interval_secs: u64,
    pub enabled: bool,
    pub engine: EngineState,
    pub consecutive_failures: u32,
}

impl MonitorShared {
    pub fn apply_snapshot(&mut self, snap: MonitorSnapshot) {
        self.stocks = snap.stocks;
        self.rules = snap.rules;
        self.interval_secs = if snap.interval_secs == 0 {
            DEFAULT_INTERVAL_SECS
        } else {
            snap.interval_secs
        };
        self.enabled = snap.enabled;
    }

    pub fn snapshot(&self) -> MonitorSnapshot {
        MonitorSnapshot {
            stocks: self.stocks.clone(),
            rules: self.rules.clone(),
            interval_secs: if self.interval_secs == 0 {
                DEFAULT_INTERVAL_SECS
            } else {
                self.interval_secs
            },
            enabled: self.enabled,
        }
    }
}
