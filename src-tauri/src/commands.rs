use crate::models::{
    AlgorithmInfo, AnalysisResult, BacktestResult, DailyBar, PredictionResult, Stock, StocksPayload,
};
use crate::strategy::{self, StrategyCompose, StrategySourceInfo};
use crate::{backtest, market, predictor};
use std::collections::HashSet;
use std::fs;
use tauri::AppHandle;
use tauri::Manager;

#[tauri::command]
pub async fn load_stocks(app: AppHandle) -> Result<StocksPayload, String> {
    let text = read_stocks_json(&app)?;
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

fn read_stocks_json(app: &AppHandle) -> Result<String, String> {
    if let Ok(path) = app
        .path()
        .resolve("resources/stocks.json", tauri::path::BaseDirectory::Resource)
    {
        if let Ok(text) = fs::read_to_string(&path) {
            return Ok(text);
        }
    }

    // Android/iOS: resource files are APK/IPA assets; plain fs paths often fail.
    Ok(include_str!("../resources/stocks.json").to_string())
}

#[tauri::command]
pub async fn search_stocks(query: String, limit: Option<usize>) -> Result<Vec<Stock>, String> {
    market::search_stocks(&query, limit.unwrap_or(12)).await
}

#[tauri::command]
pub fn list_strategy_sources() -> Vec<StrategySourceInfo> {
    strategy::catalog()
}

#[tauri::command]
pub fn default_strategy_compose() -> StrategyCompose {
    strategy::default_compose()
}

/// 一次请求完成：行情 + K线 + 组合预测 + 回测
#[tauri::command]
pub async fn analyze_stock(
    stock: Stock,
    algorithm: Option<String>,
    lookback_days: Option<u32>,
    compose: Option<StrategyCompose>,
) -> Result<AnalysisResult, String> {
    let mut compose = strategy::normalize_compose(&compose.unwrap_or_else(strategy::default_compose));
    if let Some(lb) = lookback_days {
        compose.lookback_days = lb;
    }
    let lookback = compose.lookback_days;
    // 回看窗口用于单日特征；另拉足够 K 线，使 walk-forward 至少约 BACKTEST_HORIZON 个预测日
    // 样本数 ≈ fetch_limit - lookback - 1
    const BACKTEST_HORIZON: u32 = 120;
    let fetch_limit = (lookback + BACKTEST_HORIZON + 1).max(lookback + 80);

    let stock_list = [stock.clone()];
    let (quotes_result, klines_result) = tokio::join!(
        market::fetch_stock_quotes(&stock_list),
        market::fetch_daily_klines(&stock, fetch_limit),
    );

    let quotes = quotes_result?;
    let klines = klines_result?;

    let quote = quotes.get(&stock.code);
    let current_price = quote
        .and_then(|q| q.price.or(q.prev_close))
        .or_else(|| klines.last().map(|b| b.close))
        .filter(|p| *p > 0.0)
        .ok_or_else(|| format!("{} 暂无有效价格", stock.name))?;

    let prediction = if algorithm.as_deref() == Some("placeholder_v1") {
        predictor::predict(&stock, "placeholder_v1", &klines, current_price, lookback)
    } else {
        predictor::predict_compose(&stock, &klines, current_price, &compose).await
    };

    let backtest_result = backtest::run_compose(&stock, &klines, &compose).await;

    let chart_len = 90u32.max(lookback);
    let chart_klines = if klines.len() > chart_len as usize {
        klines[klines.len() - chart_len as usize..].to_vec()
    } else {
        klines.clone()
    };

    Ok(AnalysisResult {
        prediction,
        klines: chart_klines,
        backtest: backtest_result,
    })
}

#[tauri::command]
pub async fn predict_stock(
    stock: Stock,
    algorithm: Option<String>,
    lookback_days: Option<u32>,
    compose: Option<StrategyCompose>,
) -> Result<PredictionResult, String> {
    let result = analyze_stock(stock, algorithm, lookback_days, compose).await?;
    Ok(result.prediction)
}

#[tauri::command]
pub async fn get_stock_klines(stock: Stock, limit: Option<u32>) -> Result<Vec<DailyBar>, String> {
    market::fetch_daily_klines(&stock, limit.unwrap_or(90)).await
}

#[tauri::command]
pub async fn backtest_stock(
    stock: Stock,
    algorithm: Option<String>,
    days: Option<u32>,
    compose: Option<StrategyCompose>,
) -> Result<BacktestResult, String> {
    let mut compose = strategy::normalize_compose(&compose.unwrap_or_else(strategy::default_compose));
    if let Some(d) = days {
        compose.lookback_days = d;
    }
    let _ = algorithm;
    const BACKTEST_HORIZON: u32 = 120;
    let fetch_limit = (compose.lookback_days + BACKTEST_HORIZON + 1).max(compose.lookback_days + 80);
    let bars = market::fetch_daily_klines(&stock, fetch_limit).await?;
    Ok(backtest::run_compose(&stock, &bars, &compose).await)
}

#[tauri::command]
pub fn list_algorithms() -> Vec<AlgorithmInfo> {
    vec![
        AlgorithmInfo {
            id: "compose".into(),
            name: "组合策略".into(),
            description: "按股票自定义启用信号源并加权融合（技术/舆情/宏观）。".into(),
            enabled: true,
        },
        AlgorithmInfo {
            id: "placeholder_v1".into(),
            name: "占位模型 v1".into(),
            description: "伪随机演示算法，仅用于对比测试。".into(),
            enabled: true,
        },
    ]
}
