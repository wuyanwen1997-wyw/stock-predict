use crate::models::{AlgorithmInfo, PredictionResult, Stock, StocksPayload};
use crate::{market, predictor};
use std::collections::HashSet;
use std::fs;
use tauri::AppHandle;
use tauri::Manager;

#[tauri::command]
pub async fn load_stocks(app: AppHandle) -> Result<StocksPayload, String> {
    let path = app
        .path()
        .resolve("resources/stocks.json", tauri::path::BaseDirectory::Resource)
        .map_err(|e| e.to_string())?;

    let text = fs::read_to_string(path).map_err(|e| format!("读取股票列表失败: {e}"))?;
    let mut stocks: Vec<Stock> =
        serde_json::from_str(&text).map_err(|e| format!("解析股票列表失败: {e}"))?;

    let hot_stocks = market::fetch_hot_stocks(12).await.unwrap_or_default();
    let hot_codes: HashSet<String> = hot_stocks.iter().map(|s| s.code.clone()).collect();

    let quotes = market::fetch_stock_quotes(&stocks).await.unwrap_or_default();
    for stock in &mut stocks {
        stock.is_hot = hot_codes.contains(&stock.code);
        if let Some(quote) = quotes.get(&stock.code) {
            market::apply_quote(stock, quote);
        }
    }

    stocks.sort_by(|a, b| {
        b.is_hot
            .cmp(&a.is_hot)
            .then_with(|| a.code.cmp(&b.code))
    });

    Ok(StocksPayload { stocks, hot_stocks })
}

#[tauri::command]
pub async fn search_stocks(query: String, limit: Option<usize>) -> Result<Vec<Stock>, String> {
    market::search_stocks(&query, limit.unwrap_or(12)).await
}

#[tauri::command]
pub async fn predict_stock(stock: Stock, algorithm: Option<String>) -> Result<PredictionResult, String> {
    let algo = algorithm.unwrap_or_else(|| "placeholder_v1".to_string());

    let quotes = market::fetch_stock_quotes(&[stock.clone()]).await?;
    let quote = quotes
        .get(&stock.code)
        .ok_or_else(|| format!("未找到 {} 的行情数据", stock.code))?;

    let current_price = quote
        .price
        .or(quote.prev_close)
        .filter(|p| *p > 0.0)
        .ok_or_else(|| format!("{} 暂无有效价格", stock.name))?;

    let klines = market::fetch_daily_klines(&stock, 60).await.unwrap_or_default();
    let volatility = market::calc_volatility(&klines);

    Ok(predictor::predict(&stock, &algo, current_price, volatility))
}

#[tauri::command]
pub fn list_algorithms() -> Vec<AlgorithmInfo> {
    vec![
        AlgorithmInfo {
            id: "placeholder_v1".into(),
            name: "占位模型 v1".into(),
            description: "基于真实收盘价与历史波动率的演示算法，用于 UI 预览。后续可接入真实 ML 模型。".into(),
            enabled: true,
        },
        AlgorithmInfo {
            id: "lstm_v2".into(),
            name: "LSTM v2（预留）".into(),
            description: "长短期记忆网络时序预测，尚未接入。".into(),
            enabled: false,
        },
        AlgorithmInfo {
            id: "xgboost_v1".into(),
            name: "XGBoost v1（预留）".into(),
            description: "梯度提升树因子模型，尚未接入。".into(),
            enabled: false,
        },
    ]
}
