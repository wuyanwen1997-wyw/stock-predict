use crate::models::{
    AlgorithmInfo, AnalysisResult, BacktestResult, DailyBar, KlinePeriod, PricePoint,
    PredictionResult, ScreenResult, Stock, StocksPayload,
};
use crate::monitor::{MonitorSnapshot, SharedMonitor};
use crate::screener::{self, ScreenRequest};
use crate::strategy::{self, StrategyCompose, StrategySourceInfo};
use crate::{backtest, market, predictor};
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use tauri::AppHandle;
use tauri::Manager;

#[tauri::command]
pub async fn load_stocks(app: AppHandle) -> Result<StocksPayload, String> {
    let text = read_stocks_json(&app)?;
    let mut stocks: Vec<Stock> =
        serde_json::from_str(&text).map_err(|e| format!("解析股票列表失败: {e}"))?;

    let mut warnings: Vec<String> = Vec::new();
    let hot_stocks = match market::fetch_hot_stocks(12).await {
        Ok(list) => {
            if list.is_empty() {
                let msg = "人气榜返回空列表，请检查网络后刷新".to_string();
                eprintln!("[stock-predict:load_stocks] {msg}");
                warnings.push(msg);
            } else if list.iter().all(|s| s.price.is_none()) {
                let msg = "人气榜行情补全失败，已展示榜单基础信息".to_string();
                eprintln!("[stock-predict:load_stocks] {msg}");
                warnings.push(msg);
            }
            list
        }
        Err(e) => {
            eprintln!("[stock-predict:load_stocks] 人气榜失败: {e}");
            warnings.push(format!("人气榜: {e}"));
            vec![]
        }
    };
    let hot_codes: HashSet<String> = hot_stocks.iter().map(|s| s.code.clone()).collect();

    let quotes = match market::fetch_stock_quotes(&stocks).await {
        Ok(q) => q,
        Err(e) => {
            eprintln!("[stock-predict:load_stocks] 种子行情失败: {e}");
            warnings.push("行情补全失败，部分价格可能缺失".into());
            Default::default()
        }
    };
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

    let warning = if warnings.is_empty() {
        None
    } else {
        Some(warnings.join("；"))
    };

    Ok(StocksPayload {
        stocks,
        hot_stocks,
        warning,
    })
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

#[tauri::command]
pub fn default_strategy_compose_for_stock(stock: Stock) -> StrategyCompose {
    strategy::normalize_compose(&strategy::default_compose_for_stock(&stock))
}

/// 一次请求完成：行情 + K线 + 组合预测 + 回测
#[tauri::command]
pub async fn analyze_stock(
    stock: Stock,
    algorithm: Option<String>,
    lookback_days: Option<u32>,
    compose: Option<StrategyCompose>,
    horizon_days: Option<u32>,
) -> Result<AnalysisResult, String> {
    let mut compose = strategy::normalize_compose(&compose.unwrap_or_else(strategy::default_compose));
    if let Some(lb) = lookback_days {
        compose.lookback_days = lb;
    }
    let lookback = compose.lookback_days;
    let horizon = predictor::clamp_horizon(horizon_days.unwrap_or(1));
    // 回看窗口用于单日特征；另拉足够 K 线，使 walk-forward 至少约 BACKTEST_HORIZON 个预测样本
    // 样本数 ≈ fetch_limit - lookback - horizon
    const BACKTEST_HORIZON: u32 = 120;
    let fetch_limit = (lookback + BACKTEST_HORIZON + horizon).max(lookback + 80 + horizon);

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
        predictor::predict_compose(&stock, &klines, current_price, &compose, horizon).await
    };

    let backtest_result = backtest::run_compose(&stock, &klines, &compose, horizon).await;

    let chart_len = 90u32.max(lookback);
    let chart_klines = if klines.len() > chart_len as usize {
        klines[klines.len() - chart_len as usize..].to_vec()
    } else {
        klines.clone()
    };

    // 全量 K 线算 MACD，再按 chart 窗口日期过滤，避免截断导致 EMA 失真
    let all_bs = crate::algo::compute_macd_bs(&klines);
    let chart_dates: Vec<String> = chart_klines.iter().map(|b| b.date.clone()).collect();
    let bs_markers = crate::algo::filter_markers_by_dates(&all_bs, &chart_dates);

    Ok(AnalysisResult {
        prediction,
        klines: chart_klines,
        backtest: backtest_result,
        bs_markers,
    })
}

#[tauri::command]
pub async fn predict_stock(
    stock: Stock,
    algorithm: Option<String>,
    lookback_days: Option<u32>,
    compose: Option<StrategyCompose>,
    horizon_days: Option<u32>,
) -> Result<PredictionResult, String> {
    let result = analyze_stock(stock, algorithm, lookback_days, compose, horizon_days).await?;
    Ok(result.prediction)
}

#[tauri::command]
pub async fn get_stock_klines(
    stock: Stock,
    limit: Option<u32>,
    period: Option<String>,
) -> Result<Vec<DailyBar>, String> {
    let period = match period.as_deref() {
        Some(raw) => KlinePeriod::parse(raw)?,
        None => KlinePeriod::Day,
    };
    let limit = limit.unwrap_or_else(|| period.default_limit());
    market::fetch_klines(&stock, period, limit).await
}

/// 当日分时走势点。
#[tauri::command]
pub async fn get_stock_intraday(stock: Stock) -> Result<Vec<PricePoint>, String> {
    market::fetch_intraday_trends(&stock).await
}

#[tauri::command]
pub async fn backtest_stock(
    stock: Stock,
    algorithm: Option<String>,
    days: Option<u32>,
    compose: Option<StrategyCompose>,
    horizon_days: Option<u32>,
) -> Result<BacktestResult, String> {
    let mut compose = strategy::normalize_compose(&compose.unwrap_or_else(strategy::default_compose));
    if let Some(d) = days {
        compose.lookback_days = d;
    }
    let _ = algorithm;
    let horizon = predictor::clamp_horizon(horizon_days.unwrap_or(1));
    const BACKTEST_HORIZON: u32 = 120;
    let fetch_limit =
        (compose.lookback_days + BACKTEST_HORIZON + horizon).max(compose.lookback_days + 80 + horizon);
    let bars = market::fetch_daily_klines(&stock, fetch_limit).await?;
    Ok(backtest::run_compose(&stock, &bars, &compose, horizon).await)
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

#[derive(serde::Serialize)]
pub struct TushareTokenStatus {
    pub configured: bool,
    pub hint: String,
}

#[tauri::command]
pub fn get_tushare_token_status() -> TushareTokenStatus {
    let configured = crate::capital_flow::tushare_token_configured();
    TushareTokenStatus {
        configured,
        hint: if configured {
            "已配置（环境变量或本地文件）。有 Token 时优先用真·大盘主力净流入；否则用两市成交代理亦可回测。"
                .into()
        } else {
            "未配置 Tushare。资金流仍可用「两市成交代理」做近期回测；配置 Token 后自动升级为真主力净流入。"
                .into()
        },
    }
}

#[tauri::command]
pub fn set_tushare_token(token: String) -> Result<TushareTokenStatus, String> {
    crate::capital_flow::save_tushare_token(&token)?;
    Ok(get_tushare_token_status())
}

#[tauri::command]
pub fn default_screen_compose() -> StrategyCompose {
    screener::default_screen_compose()
}

/// 智能选股：建池 → 硬过滤 → 批量技术打分 → TopN
#[tauri::command]
pub async fn run_smart_screen(
    app: AppHandle,
    request: ScreenRequest,
) -> Result<ScreenResult, String> {
    let text = read_stocks_json(&app)?;
    let seed: Vec<Stock> =
        serde_json::from_str(&text).map_err(|e| format!("解析股票列表失败: {e}"))?;
    screener::run_smart_screen(&app, seed, request).await
}

/// 同步盯盘配置（自选 + 规则）；开启后台服务前调用。
#[tauri::command]
pub async fn monitor_sync_config(
    app: AppHandle,
    snapshot: MonitorSnapshot,
) -> Result<(), String> {
    let shared = app
        .try_state::<SharedMonitor>()
        .ok_or_else(|| "MonitorShared 未注册".to_string())?;
    let mut guard = shared.lock().await;
    guard.apply_snapshot(snapshot);
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorStatus {
    pub enabled: bool,
    pub stock_count: usize,
    pub rule_count: usize,
    pub interval_secs: u64,
    pub consecutive_failures: u32,
}

#[tauri::command]
pub async fn monitor_get_status(app: AppHandle) -> Result<MonitorStatus, String> {
    let shared = app
        .try_state::<SharedMonitor>()
        .ok_or_else(|| "MonitorShared 未注册".to_string())?;
    let guard = shared.lock().await;
    Ok(MonitorStatus {
        enabled: guard.enabled,
        stock_count: guard.stocks.len(),
        rule_count: guard.rules.iter().filter(|r| r.enabled).count(),
        interval_secs: if guard.interval_secs == 0 {
            crate::monitor::default_interval_secs()
        } else {
            guard.interval_secs
        },
        consecutive_failures: guard.consecutive_failures,
    })
}

fn user_store(app: &AppHandle) -> Result<tauri::State<'_, crate::user_store::UserStore>, String> {
    app.try_state::<crate::user_store::UserStore>()
        .ok_or_else(|| "UserStore 未初始化".to_string())
}

#[tauri::command]
pub fn ensure_user_db(app: AppHandle) -> Result<crate::user_store::UserDbStatus, String> {
    let store = user_store(&app)?;
    store.status()
}

#[tauri::command]
pub fn load_user_data(app: AppHandle) -> Result<crate::user_store::UserDataSnapshot, String> {
    let store = user_store(&app)?;
    store.load()
}

#[tauri::command]
pub fn save_watchlist(app: AppHandle, items: Vec<Stock>) -> Result<(), String> {
    let store = user_store(&app)?;
    store.save_watchlist(&items)
}

#[tauri::command]
pub fn save_pool(
    app: AppHandle,
    groups: Vec<crate::user_store::PoolGroup>,
    items: Vec<crate::user_store::PoolItem>,
) -> Result<(), String> {
    let store = user_store(&app)?;
    store.save_pool(&groups, &items)
}

#[tauri::command]
pub fn save_holdings(
    app: AppHandle,
    items: Vec<crate::user_store::Holding>,
) -> Result<(), String> {
    let store = user_store(&app)?;
    store.save_holdings(&items)
}

#[tauri::command]
pub fn save_journal_entries(
    app: AppHandle,
    entries: Vec<crate::user_store::JournalEntry>,
) -> Result<(), String> {
    let store = user_store(&app)?;
    store.save_journal(&entries)
}

#[tauri::command]
pub fn save_strategy_map(
    app: AppHandle,
    map: std::collections::HashMap<String, strategy::StrategyCompose>,
) -> Result<(), String> {
    let store = user_store(&app)?;
    store.save_strategy_map(&map)
}

#[tauri::command]
pub fn save_user_settings(
    app: AppHandle,
    settings: crate::user_store::UserSettings,
) -> Result<(), String> {
    let store = user_store(&app)?;
    store.save_settings(&settings)
}

#[tauri::command]
pub fn save_monitor_rules(
    app: AppHandle,
    rules: Vec<crate::monitor::MonitorRule>,
) -> Result<(), String> {
    let store = user_store(&app)?;
    store.save_monitor_rules(&rules)
}

#[tauri::command]
pub fn save_monitor_alerts(
    app: AppHandle,
    alerts: Vec<crate::monitor::MonitorAlert>,
) -> Result<(), String> {
    let store = user_store(&app)?;
    store.save_monitor_alerts(&alerts)
}

#[tauri::command]
pub fn import_from_localstorage(
    app: AppHandle,
    payload: crate::user_store::LegacyLocalStoragePayload,
) -> Result<crate::user_store::UserDataSnapshot, String> {
    let store = user_store(&app)?;
    store.import_from_localstorage(payload)
}

#[tauri::command]
pub fn mark_localstorage_migrated(app: AppHandle) -> Result<(), String> {
    let store = user_store(&app)?;
    store.mark_localstorage_migrated()
}

#[tauri::command]
pub fn export_user_data(app: AppHandle) -> Result<String, String> {
    let store = user_store(&app)?;
    store.export_json()
}

#[tauri::command]
pub fn import_user_data(
    app: AppHandle,
    json: String,
) -> Result<crate::user_store::UserDataSnapshot, String> {
    let store = user_store(&app)?;
    store.import_json(&json)
}
