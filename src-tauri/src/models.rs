use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stock {
    pub code: String,
    pub name: String,
    pub market: String,
    pub sector: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change_pct: Option<f64>,
    #[serde(default)]
    pub is_hot: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockQuote {
    pub price: Option<f64>,
    pub change_pct: Option<f64>,
    pub change_amt: Option<f64>,
    pub open: Option<f64>,
    pub high: Option<f64>,
    pub low: Option<f64>,
    pub prev_close: Option<f64>,
    pub volume: Option<f64>,
    pub turnover: Option<f64>,
}

/// K 线周期。`date` 字段：日/周/月为 `YYYY-MM-DD`，分钟为 `YYYY-MM-DD HH:MM`。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum KlinePeriod {
    #[default]
    Day,
    Week,
    Month,
    /// 1 分钟
    Min1,
    Min5,
    Min15,
    Min30,
    Min60,
}

impl KlinePeriod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
            Self::Min1 => "min1",
            Self::Min5 => "min5",
            Self::Min15 => "min15",
            Self::Min30 => "min30",
            Self::Min60 => "min60",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "day" | "d" | "daily" | "101" => Ok(Self::Day),
            "week" | "w" | "weekly" | "102" => Ok(Self::Week),
            "month" | "m" | "monthly" | "103" => Ok(Self::Month),
            "min1" | "1m" | "m1" | "1" => Ok(Self::Min1),
            "min5" | "5m" | "m5" | "5" => Ok(Self::Min5),
            "min15" | "15m" | "m15" | "15" => Ok(Self::Min15),
            "min30" | "30m" | "m30" | "30" => Ok(Self::Min30),
            "min60" | "60m" | "m60" | "60" | "hour" => Ok(Self::Min60),
            other => Err(format!(
                "未知 K 线周期: {other}（支持 day/week/month/min1/min5/min15/min30/min60）"
            )),
        }
    }

    pub fn is_intraday(self) -> bool {
        matches!(
            self,
            Self::Min1 | Self::Min5 | Self::Min15 | Self::Min30 | Self::Min60
        )
    }

    /// 东方财富 klt 参数
    pub fn eastmoney_klt(self) -> u32 {
        match self {
            Self::Day => 101,
            Self::Week => 102,
            Self::Month => 103,
            Self::Min1 => 1,
            Self::Min5 => 5,
            Self::Min15 => 15,
            Self::Min30 => 30,
            Self::Min60 => 60,
        }
    }

    /// 腾讯 fqkline 周期段（day/week/month/m1…）
    pub fn tencent_freq(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
            Self::Min1 => "m1",
            Self::Min5 => "m5",
            Self::Min15 => "m15",
            Self::Min30 => "m30",
            Self::Min60 => "m60",
        }
    }

    /// 新浪 scale（分钟与日线；周/月新浪不稳，由上层跳过）
    pub fn sina_scale(self) -> Option<u32> {
        match self {
            Self::Day => Some(240),
            Self::Min1 => Some(1),
            Self::Min5 => Some(5),
            Self::Min15 => Some(15),
            Self::Min30 => Some(30),
            Self::Min60 => Some(60),
            Self::Week | Self::Month => None,
        }
    }

    pub fn default_limit(self) -> u32 {
        match self {
            Self::Day => 90,
            Self::Week => 104,
            Self::Month => 60,
            Self::Min1 => 240,
            Self::Min5 => 240,
            Self::Min15 => 160,
            Self::Min30 => 120,
            Self::Min60 => 120,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyBar {
    /// 日/周/月：`YYYY-MM-DD`；分钟：`YYYY-MM-DD HH:MM`
    pub date: String,
    pub open: f64,
    pub close: f64,
    pub high: f64,
    pub low: f64,
    pub volume: f64,
    pub change_pct: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricePoint {
    pub time: String,
    pub price: f64,
    pub volume: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioForecast {
    pub label: String,
    pub open_price: f64,
    pub close_price: f64,
    pub high_price: f64,
    pub low_price: f64,
    pub change_pct: f64,
    pub path: Vec<PricePoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalContribution {
    pub id: String,
    pub name: String,
    pub category: String,
    pub up_probability: f64,
    pub down_probability: f64,
    pub confidence: f64,
    pub weight: f64,
    pub weight_normalized: f64,
    pub note: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionResult {
    pub stock: Stock,
    pub predict_date: String,
    pub current_price: f64,
    pub up_probability: f64,
    pub down_probability: f64,
    pub flat_probability: f64,
    pub confidence: f64,
    pub predicted: String,
    pub high_confidence: bool,
    pub high_confidence_threshold: f64,
    pub algorithm: String,
    pub high_open: ScenarioForecast,
    pub low_open: ScenarioForecast,
    pub summary: String,
    #[serde(default)]
    pub signals: Vec<SignalContribution>,
    /// 预测跨度：1=次日，2–5=多日累计涨跌
    #[serde(default = "default_horizon_days")]
    pub horizon_days: u32,
}

fn default_horizon_days() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StocksPayload {
    pub stocks: Vec<Stock>,
    pub hot_stocks: Vec<Stock>,
    /// 人气榜或部分行情失败时的可读告警（列表仍可能部分可用）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestRecord {
    pub date: String,
    pub predict_date: String,
    pub close_price: f64,
    pub next_close: f64,
    pub change_pct: f64,
    pub predicted: String,
    pub actual: String,
    pub up_probability: f64,
    pub down_probability: f64,
    pub confidence: f64,
    pub high_confidence: bool,
    pub correct: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResult {
    pub stock: Stock,
    pub algorithm: String,
    pub total_samples: u32,
    pub direction_accuracy: f64,
    pub actionable_accuracy: f64,
    /// 全交易日强制预测准确率（含信号不足日）；消息面主策略时与 direction_accuracy 可能不同
    #[serde(default)]
    pub all_day_accuracy: f64,
    /// 有效信号样本数（领先概率达到出手线）
    #[serde(default)]
    pub actionable_samples: u32,
    /// 是否按「有效信号」口径统计整体准确率
    #[serde(default)]
    pub selective_mode: bool,
    pub up_hit_rate: f64,
    pub down_hit_rate: f64,
    /// 仅有效信号日的看涨命中率
    #[serde(default)]
    pub up_hit_rate_actionable: f64,
    /// 仅有效信号日的看跌命中率
    #[serde(default)]
    pub down_hit_rate_actionable: f64,
    #[serde(default)]
    pub up_samples: u32,
    #[serde(default)]
    pub down_samples: u32,
    #[serde(default)]
    pub up_samples_actionable: u32,
    #[serde(default)]
    pub down_samples_actionable: u32,
    pub high_confidence_samples: u32,
    pub high_confidence_accuracy: f64,
    pub high_confidence_threshold: f64,
    pub flat_threshold_pct: f64,
    pub lookback_days: u32,
    /// 预测跨度：1=次日，2–5=多日累计
    #[serde(default = "default_horizon_days")]
    pub horizon_days: u32,
    pub summary: String,
    pub records: Vec<BacktestRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub prediction: PredictionResult,
    pub klines: Vec<DailyBar>,
    pub backtest: BacktestResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlgorithmInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
}

/// 智能选股：股票池来源
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScreenUniverse {
    Hot,
    Watchlist,
    Seed,
    Mixed,
}

impl Default for ScreenUniverse {
    fn default() -> Self {
        Self::Mixed
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenFilters {
    #[serde(default = "default_true")]
    pub exclude_st: bool,
    #[serde(default = "default_min_price")]
    pub min_price: Option<f64>,
    #[serde(default = "default_min_change")]
    pub min_change_pct: Option<f64>,
    #[serde(default = "default_max_change")]
    pub max_change_pct: Option<f64>,
    #[serde(default)]
    pub main_board_only: bool,
}

fn default_true() -> bool {
    true
}

fn default_min_price() -> Option<f64> {
    Some(2.0)
}

fn default_min_change() -> Option<f64> {
    Some(-5.0)
}

fn default_max_change() -> Option<f64> {
    Some(7.0)
}

impl Default for ScreenFilters {
    fn default() -> Self {
        Self {
            exclude_st: true,
            min_price: default_min_price(),
            min_change_pct: default_min_change(),
            max_change_pct: default_max_change(),
            main_board_only: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenHit {
    pub stock: Stock,
    pub up_probability: f64,
    pub down_probability: f64,
    pub confidence: f64,
    pub direction: String,
    pub factor_score: f64,
    pub hints: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenProgressEvent {
    pub done: usize,
    pub total: usize,
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenResult {
    pub hits: Vec<ScreenHit>,
    pub universe_size: usize,
    pub filtered_size: usize,
    pub scored_size: usize,
    pub failed_size: usize,
    pub elapsed_ms: u64,
    pub summary: String,
    #[serde(default)]
    pub timed_out: bool,
}
