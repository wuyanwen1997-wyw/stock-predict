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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyBar {
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
pub struct PredictionResult {
    pub stock: Stock,
    pub predict_date: String,
    pub current_price: f64,
    pub up_probability: f64,
    pub down_probability: f64,
    pub flat_probability: f64,
    pub confidence: f64,
    pub algorithm: String,
    pub high_open: ScenarioForecast,
    pub low_open: ScenarioForecast,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StocksPayload {
    pub stocks: Vec<Stock>,
    pub hot_stocks: Vec<Stock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlgorithmInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
}
