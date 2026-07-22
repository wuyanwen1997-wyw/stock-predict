use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AlertCondition {
    PriceAbove { value: f64 },
    PriceBelow { value: f64 },
    ChangePctAbove { value: f64 },
    ChangePctBelow { value: f64 },
}

impl AlertCondition {
    pub fn label(&self) -> String {
        match self {
            Self::PriceAbove { value } => format!("价格 ≥ {value:.2}"),
            Self::PriceBelow { value } => format!("价格 ≤ {value:.2}"),
            Self::ChangePctAbove { value } => format!("涨跌幅 ≥ {value:.2}%"),
            Self::ChangePctBelow { value } => format!("涨跌幅 ≤ {value:.2}%"),
        }
    }

    pub fn matches(&self, price: Option<f64>, change_pct: Option<f64>) -> bool {
        match self {
            Self::PriceAbove { value } => price.map(|p| p >= *value).unwrap_or(false),
            Self::PriceBelow { value } => price.map(|p| p <= *value).unwrap_or(false),
            Self::ChangePctAbove { value } => change_pct.map(|p| p >= *value).unwrap_or(false),
            Self::ChangePctBelow { value } => change_pct.map(|p| p <= *value).unwrap_or(false),
        }
    }
}

pub fn condition_label(c: &AlertCondition) -> String {
    c.label()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorRule {
    pub id: String,
    pub code: String,
    pub name: String,
    pub enabled: bool,
    pub condition: AlertCondition,
    #[serde(default = "default_cooldown")]
    pub cooldown_sec: u64,
    #[serde(default = "default_max_per_day")]
    pub max_per_day: u32,
    #[serde(default)]
    pub created_at: String,
}

fn default_cooldown() -> u64 {
    300
}

fn default_max_per_day() -> u32 {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorAlert {
    pub id: String,
    pub rule_id: String,
    pub code: String,
    pub name: String,
    pub message: String,
    pub price: Option<f64>,
    pub change_pct: Option<f64>,
    pub fired_at: DateTime<Utc>,
}
