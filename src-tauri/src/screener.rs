//! 智能选股：两阶段（硬过滤 + 策略打分），复用 strategy / factor_model。

use crate::factor_model;
use crate::market;
use crate::models::{
    DailyBar, ScreenFilters, ScreenHit, ScreenProgressEvent, ScreenResult, ScreenUniverse, Stock,
};
use crate::strategy::{self, StrategyCompose, StrategySourceConfig};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, Semaphore};

const MAX_UNIVERSE: usize = 120;
const HOT_LIMIT: usize = 50;
const BATCH_SOFT_TIMEOUT: Duration = Duration::from_secs(180);
const KLINE_CACHE_TTL: Duration = Duration::from_secs(3600);

type CacheKey = (String, u32);
type CacheEntry = (Instant, Vec<DailyBar>);

fn kline_cache() -> &'static Mutex<HashMap<CacheKey, CacheEntry>> {
    static CELL: OnceLock<Mutex<HashMap<CacheKey, CacheEntry>>> = OnceLock::new();
    CELL.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScreenRequest {
    #[serde(default)]
    pub universe: ScreenUniverse,
    #[serde(default)]
    pub watchlist: Vec<Stock>,
    #[serde(default)]
    pub filters: ScreenFilters,
    #[serde(default)]
    pub compose: Option<StrategyCompose>,
    #[serde(default = "default_horizon")]
    pub horizon_days: u32,
    #[serde(default = "default_lookback")]
    pub lookback_days: u32,
    #[serde(default = "default_top_n")]
    pub top_n: usize,
    #[serde(default = "default_concurrency")]
    pub concurrency: usize,
}

fn default_horizon() -> u32 {
    1
}
fn default_lookback() -> u32 {
    50
}
fn default_top_n() -> usize {
    20
}
fn default_concurrency() -> usize {
    4
}

/// 选股默认组合：技术面三源（快、可回测）
pub fn default_screen_compose() -> StrategyCompose {
    StrategyCompose {
        lookback_days: 50,
        sources: vec![
            StrategySourceConfig {
                id: "factor".into(),
                enabled: true,
                weight: 40.0,
            },
            StrategySourceConfig {
                id: "momentum".into(),
                enabled: true,
                weight: 30.0,
            },
            StrategySourceConfig {
                id: "mean_reversion".into(),
                enabled: false,
                weight: 15.0,
            },
            StrategySourceConfig {
                id: "volume".into(),
                enabled: true,
                weight: 20.0,
            },
            StrategySourceConfig {
                id: "message".into(),
                enabled: false,
                weight: 20.0,
            },
            StrategySourceConfig {
                id: "news".into(),
                enabled: false,
                weight: 10.0,
            },
            StrategySourceConfig {
                id: "policy".into(),
                enabled: false,
                weight: 10.0,
            },
            StrategySourceConfig {
                id: "us_market".into(),
                enabled: false,
                weight: 10.0,
            },
            StrategySourceConfig {
                id: "capital_flow".into(),
                enabled: false,
                weight: 15.0,
            },
        ],
    }
}

/// 批量选股用：关闭 live-only 与慢速源（消息/资金流需归档）
pub fn compose_for_screen(compose: &StrategyCompose) -> StrategyCompose {
    let mut c = strategy::normalize_compose(compose);
    for s in &mut c.sources {
        match s.id.as_str() {
            "news" | "policy" | "us_market" | "message" | "capital_flow" => {
                s.enabled = false;
            }
            _ => {}
        }
    }
    let any_tech = c.sources.iter().any(|s| {
        s.enabled
            && s.weight > 0.0
            && matches!(
                s.id.as_str(),
                "factor" | "momentum" | "mean_reversion" | "volume"
            )
    });
    if !any_tech {
        return strategy::normalize_compose(&default_screen_compose());
    }
    c
}

pub async fn run_smart_screen(
    app: &AppHandle,
    seed_stocks: Vec<Stock>,
    request: ScreenRequest,
) -> Result<ScreenResult, String> {
    let started = Instant::now();
    let mut compose = compose_for_screen(&request.compose.unwrap_or_else(default_screen_compose));
    if request.lookback_days > 0 {
        compose.lookback_days = request.lookback_days;
    }
    let horizon = request.horizon_days.clamp(1, 5);
    let top_n = request.top_n.max(1).min(50);
    let concurrency = request.concurrency.clamp(1, 8);
    let lookback = factor_model::clamp_lookback(compose.lookback_days) as u32;
    let fetch_limit = (lookback + 10).max(40);

    let universe = build_universe(request.universe, &seed_stocks, &request.watchlist).await?;
    let universe_size = universe.len();

    let enriched = enrich_quotes(universe).await;
    let filtered = hard_filter(&enriched, &request.filters);
    let filtered_size = filtered.len();

    if filtered.is_empty() {
        return Ok(ScreenResult {
            hits: vec![],
            universe_size,
            filtered_size: 0,
            scored_size: 0,
            failed_size: 0,
            elapsed_ms: started.elapsed().as_millis() as u64,
            summary: format!("股票池 {universe_size} 只，硬过滤后无候选。请放宽条件后重试。"),
            timed_out: false,
        });
    }

    let (hits, timed_out) =
        batch_score(app, filtered, &compose, horizon, fetch_limit, concurrency).await;

    let failed_size = hits.iter().filter(|h| h.error.is_some()).count();
    let mut scored: Vec<ScreenHit> = hits.into_iter().filter(|h| h.error.is_none()).collect();
    scored.sort_by(|a, b| {
        b.up_probability
            .partial_cmp(&a.up_probability)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                b.confidence
                    .partial_cmp(&a.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| {
                b.factor_score
                    .partial_cmp(&a.factor_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });
    let scored_size = scored.len();
    scored.truncate(top_n);

    let timeout_note = if timed_out {
        "（已软超时，返回已完成部分）"
    } else {
        ""
    };
    let summary = format!(
        "池 {universe_size} → 过滤 {filtered_size} → 成功打分 {scored_size}，失败 {failed_size}，展示 Top {}。耗时 {:.1}s{timeout_note}",
        scored.len(),
        started.elapsed().as_secs_f64(),
    );

    Ok(ScreenResult {
        hits: scored,
        universe_size,
        filtered_size,
        scored_size,
        failed_size,
        elapsed_ms: started.elapsed().as_millis() as u64,
        summary,
        timed_out,
    })
}

async fn build_universe(
    kind: ScreenUniverse,
    seed: &[Stock],
    watchlist: &[Stock],
) -> Result<Vec<Stock>, String> {
    let hot = match kind {
        ScreenUniverse::Hot | ScreenUniverse::Mixed => match market::fetch_hot_stocks(HOT_LIMIT)
            .await
        {
            Ok(list) => list,
            Err(e) => {
                eprintln!("[stock-predict:screener] 人气榜失败: {e}");
                if matches!(kind, ScreenUniverse::Hot) {
                    return Err(format!("人气榜获取失败: {e}"));
                }
                // mixed：人气失败仍可用自选+种子
                vec![]
            }
        },
        _ => vec![],
    };

    let mut by_code: HashMap<String, Stock> = HashMap::new();
    let push = |map: &mut HashMap<String, Stock>, s: Stock| {
        let key = normalize_code(&s.code);
        if key.is_empty() {
            return;
        }
        map.entry(key).or_insert(s);
    };

    match kind {
        ScreenUniverse::Hot => {
            for s in hot {
                push(&mut by_code, s);
            }
        }
        ScreenUniverse::Watchlist => {
            for s in watchlist {
                push(&mut by_code, s.clone());
            }
        }
        ScreenUniverse::Seed => {
            for s in seed {
                push(&mut by_code, s.clone());
            }
        }
        ScreenUniverse::Mixed => {
            for s in hot {
                push(&mut by_code, s);
            }
            for s in watchlist {
                push(&mut by_code, s.clone());
            }
            for s in seed {
                push(&mut by_code, s.clone());
            }
        }
    }

    let mut out: Vec<Stock> = by_code.into_values().collect();
    out.sort_by(|a, b| a.code.cmp(&b.code));
    if out.len() > MAX_UNIVERSE {
        out.truncate(MAX_UNIVERSE);
    }
    if out.is_empty() {
        return Err("股票池为空，请检查自选股或网络后重试".into());
    }
    Ok(out)
}

async fn enrich_quotes(mut stocks: Vec<Stock>) -> Vec<Stock> {
    let need: Vec<Stock> = stocks
        .iter()
        .filter(|s| s.price.is_none() || s.change_pct.is_none())
        .cloned()
        .collect();
    if need.is_empty() {
        return stocks;
    }
    for chunk in need.chunks(80) {
        if let Ok(quotes) = market::fetch_stock_quotes(chunk).await {
            for s in &mut stocks {
                if let Some(q) = quotes.get(&s.code) {
                    market::apply_quote(s, q);
                }
            }
        }
    }
    stocks
}

fn hard_filter(stocks: &[Stock], filters: &ScreenFilters) -> Vec<Stock> {
    stocks
        .iter()
        .filter(|s| {
            if filters.exclude_st {
                let n = s.name.to_uppercase();
                if n.contains("ST") {
                    return false;
                }
            }
            if filters.main_board_only && !is_main_board(&s.code) {
                return false;
            }
            if let Some(min_p) = filters.min_price {
                match s.price {
                    Some(p) if p >= min_p => {}
                    Some(_) => return false,
                    None => {}
                }
            }
            if let Some(min_c) = filters.min_change_pct {
                if let Some(c) = s.change_pct {
                    if c < min_c {
                        return false;
                    }
                }
            }
            if let Some(max_c) = filters.max_change_pct {
                if let Some(c) = s.change_pct {
                    if c > max_c {
                        return false;
                    }
                }
            }
            true
        })
        .cloned()
        .collect()
}

fn is_main_board(code: &str) -> bool {
    let digits = normalize_code(code);
    if digits.len() < 6 {
        return false;
    }
    let c = &digits[..6];
    !(c.starts_with("300")
        || c.starts_with("301")
        || c.starts_with("688")
        || c.starts_with('8')
        || c.starts_with('4'))
}

fn normalize_code(code: &str) -> String {
    let digits: String = code.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() >= 6 {
        digits[digits.len() - 6..].to_string()
    } else if digits.is_empty() {
        String::new()
    } else {
        format!("{:0>6}", digits)
    }
}

/// 带软超时的批量打分：超时后返回已完成子集
async fn batch_score(
    app: &AppHandle,
    stocks: Vec<Stock>,
    compose: &StrategyCompose,
    horizon: u32,
    fetch_limit: u32,
    concurrency: usize,
) -> (Vec<ScreenHit>, bool) {
    let total = stocks.len();
    let done = Arc::new(AtomicUsize::new(0));
    let sem = Arc::new(Semaphore::new(concurrency));
    let compose = Arc::new(compose.clone());
    let (tx, mut rx) = mpsc::channel::<ScreenHit>(total.max(1));

    for stock in stocks {
        let sem = Arc::clone(&sem);
        let compose = Arc::clone(&compose);
        let done = Arc::clone(&done);
        let app = app.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            let permit = match sem.acquire_owned().await {
                Ok(p) => p,
                Err(_) => {
                    let _ = tx
                        .send(ScreenHit {
                            stock: stock.clone(),
                            up_probability: 50.0,
                            down_probability: 50.0,
                            confidence: 0.0,
                            direction: "flat".into(),
                            factor_score: 0.0,
                            hints: vec![],
                            error: Some("并发许可失败".into()),
                        })
                        .await;
                    return;
                }
            };
            let code = stock.code.clone();
            let hit = score_one(&stock, &compose, horizon, fetch_limit).await;
            drop(permit);
            let n = done.fetch_add(1, Ordering::Relaxed) + 1;
            let _ = app.emit(
                "smart-screen-progress",
                ScreenProgressEvent {
                    done: n,
                    total,
                    code,
                },
            );
            let _ = tx.send(hit).await;
        });
    }
    drop(tx);

    let deadline = Instant::now() + BATCH_SOFT_TIMEOUT;
    let mut out = Vec::with_capacity(total);
    let mut timed_out = false;
    while out.len() < total {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            timed_out = true;
            break;
        }
        match tokio::time::timeout(left, rx.recv()).await {
            Ok(Some(hit)) => out.push(hit),
            Ok(None) => break,
            Err(_) => {
                timed_out = true;
                break;
            }
        }
    }
    (out, timed_out)
}

async fn score_one(
    stock: &Stock,
    compose: &StrategyCompose,
    horizon: u32,
    fetch_limit: u32,
) -> ScreenHit {
    let mut last_err = None;
    for attempt in 0..2 {
        match fetch_klines_cached(stock, fetch_limit).await {
            Ok(bars) => {
                if bars.len() < factor_model::MIN_BARS {
                    return ScreenHit {
                        stock: stock.clone(),
                        up_probability: 50.0,
                        down_probability: 50.0,
                        confidence: 0.0,
                        direction: "flat".into(),
                        factor_score: 0.0,
                        hints: vec![],
                        error: Some(format!("K线不足（{}根）", bars.len())),
                    };
                }
                let as_of = bars
                    .last()
                    .and_then(|b| crate::cninfo::parse_flexible_date(&b.date));
                let ensemble =
                    strategy::evaluate_historical(stock, &bars, compose, None, None, as_of, horizon);
                let style = factor_model::style_for_stock(stock);
                let snap = factor_model::compute_styled_for_horizon(&bars, style, horizon);
                let factor_score = snap.as_ref().map(|s| s.score).unwrap_or(0.0);
                let hints = snap.map(|s| s.hints).unwrap_or_default();
                return ScreenHit {
                    stock: stock.clone(),
                    up_probability: round1(ensemble.up_probability),
                    down_probability: round1(ensemble.down_probability),
                    confidence: round1(ensemble.confidence),
                    direction: ensemble.predicted,
                    factor_score: round2(factor_score),
                    hints,
                    error: None,
                };
            }
            Err(e) => {
                last_err = Some(e);
                if attempt == 0 {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
            }
        }
    }
    ScreenHit {
        stock: stock.clone(),
        up_probability: 50.0,
        down_probability: 50.0,
        confidence: 0.0,
        direction: "flat".into(),
        factor_score: 0.0,
        hints: vec![],
        error: last_err,
    }
}

async fn fetch_klines_cached(stock: &Stock, limit: u32) -> Result<Vec<DailyBar>, String> {
    let key = (normalize_code(&stock.code), limit);
    if let Ok(guard) = kline_cache().lock() {
        if let Some((at, bars)) = guard.get(&key) {
            if at.elapsed() < KLINE_CACHE_TTL {
                return Ok(bars.clone());
            }
        }
    }
    let bars = market::fetch_daily_klines(stock, limit).await?;
    if let Ok(mut guard) = kline_cache().lock() {
        if guard.len() > 200 {
            guard.clear();
        }
        guard.insert(key, (Instant::now(), bars.clone()));
    }
    Ok(bars)
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_board_rules() {
        assert!(is_main_board("600519"));
        assert!(is_main_board("000858"));
        assert!(!is_main_board("300750"));
        assert!(!is_main_board("688981"));
    }

    #[test]
    fn filter_st() {
        let filters = ScreenFilters::default();
        let stocks = vec![
            Stock {
                code: "600000".into(),
                name: "浦发银行".into(),
                market: "SH".into(),
                sector: "银行".into(),
                price: Some(10.0),
                change_pct: Some(1.0),
                is_hot: false,
            },
            Stock {
                code: "000001".into(),
                name: "ST示例".into(),
                market: "SZ".into(),
                sector: "—".into(),
                price: Some(5.0),
                change_pct: Some(1.0),
                is_hot: false,
            },
        ];
        let out = hard_filter(&stocks, &filters);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].code, "600000");
    }

    #[test]
    fn compose_strips_live() {
        let mut c = default_screen_compose();
        for s in &mut c.sources {
            if s.id == "news" {
                s.enabled = true;
            }
        }
        let out = compose_for_screen(&c);
        assert!(out
            .sources
            .iter()
            .all(|s| !matches!(s.id.as_str(), "news" | "policy" | "us_market") || !s.enabled));
    }
}
