use crate::cninfo::MessageArchive;
use crate::factor_model;
use crate::models::{DailyBar, PricePoint, PredictionResult, ScenarioForecast, Stock};
use crate::strategy::{self, EnsembleSignal, StrategyCompose};
use chrono::{Datelike, Duration, Local, NaiveDate, Weekday};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone)]
pub struct DirectionSignal {
    pub up_probability: f64,
    pub down_probability: f64,
    pub confidence: f64,
    pub predicted: String,
    pub high_confidence: bool,
}

/// 领先一侧概率 ≥ 该阈值视为高置信信号
pub const HIGH_CONF_THRESHOLD: f64 = 60.0;

pub fn is_high_confidence(up: f64, down: f64) -> bool {
    up.max(down) >= HIGH_CONF_THRESHOLD
}

/// 粗略 A 股休市日（周末 + 近年常见假期；未覆盖临时调休）
fn is_ashare_closed(date: NaiveDate) -> bool {
    matches!(date.weekday(), Weekday::Sat | Weekday::Sun) || is_ashare_holiday(date)
}

fn is_ashare_holiday(date: NaiveDate) -> bool {
    // YYYYMMDD，覆盖 2025–2026 主要休市日（含常见调休；临时安排可能有偏差）
    const HOLIDAYS: &[&str] = &[
        // 2025
        "20250101",
        "20250128", "20250129", "20250130", "20250131", "20250201", "20250202", "20250203", "20250204",
        "20250404", "20250405", "20250406",
        "20250501", "20250502", "20250505",
        "20250531", "20250601", "20250602",
        "20251001", "20251002", "20251003", "20251006", "20251007", "20251008",
        // 2026
        "20260101", "20260102",
        "20260216", "20260217", "20260218", "20260219", "20260220", "20260221", "20260222", "20260223",
        "20260405", "20260406",
        "20260501", "20260502", "20260503", "20260504",
        "20260619", "20260620", "20260621",
        "20261001", "20261002", "20261005", "20261006", "20261007",
    ];
    let key = date.format("%Y%m%d").to_string();
    HOLIDAYS.contains(&key.as_str())
}

/// 下一 A 股交易日：从「今天之后」起跳过周末与假期（周五/周末 → 下周一）
pub fn next_trading_day(from: NaiveDate) -> NaiveDate {
    let mut d = from + Duration::days(1);
    for _ in 0..20 {
        if !is_ashare_closed(d) {
            return d;
        }
        d += Duration::days(1);
    }
    d
}

pub fn next_trading_day_from_today() -> NaiveDate {
    next_trading_day(Local::now().date_naive())
}

/// 直播预测：组合策略（含消息/政策/美股等非回测源）
pub async fn predict_compose(
    stock: &Stock,
    bars: &[DailyBar],
    current_price: f64,
    compose: &StrategyCompose,
) -> PredictionResult {
    let compose = strategy::normalize_compose(compose);
    let lookback = compose.lookback_days;
    let ensemble = strategy::evaluate_live(stock, bars, &compose).await;
    build_result(stock, bars, current_price, lookback, "compose", next_trading_day_from_today(), ensemble)
}

/// 历史单点预测（仅可回测信号源）
pub fn predict_compose_historical(
    stock: &Stock,
    bars: &[DailyBar],
    current_price: f64,
    predict_date: NaiveDate,
    compose: &StrategyCompose,
    message: Option<&MessageArchive>,
) -> PredictionResult {
    let compose = strategy::normalize_compose(compose);
    let lookback = compose.lookback_days;
    let as_of = bars
        .last()
        .and_then(|b| crate::cninfo::parse_flexible_date(&b.date));
    let ensemble = strategy::evaluate_historical(stock, bars, &compose, message, as_of);
    build_result(stock, bars, current_price, lookback, "compose", predict_date, ensemble)
}

pub fn predict_direction_compose(
    stock: &Stock,
    bars: &[DailyBar],
    compose: &StrategyCompose,
    message: Option<&MessageArchive>,
    as_of: Option<NaiveDate>,
) -> DirectionSignal {
    let ensemble = strategy::evaluate_historical(stock, bars, compose, message, as_of);
    DirectionSignal {
        up_probability: ensemble.up_probability,
        down_probability: ensemble.down_probability,
        confidence: ensemble.confidence,
        predicted: ensemble.predicted,
        high_confidence: ensemble.high_confidence,
    }
}

/// 兼容旧单算法入口
pub fn predict(
    stock: &Stock,
    algorithm: &str,
    bars: &[DailyBar],
    current_price: f64,
    lookback_days: u32,
) -> PredictionResult {
    let mut compose = strategy::default_compose();
    compose.lookback_days = lookback_days;
    for s in &mut compose.sources {
        s.enabled = match algorithm {
            "placeholder_v1" => false,
            _ => s.id == "factor" || s.id == "momentum" || s.id == "volume",
        };
        if algorithm == "placeholder_v1" && s.id == "factor" {
            s.enabled = true;
        }
    }
    if algorithm == "placeholder_v1" {
        // 简化：占位仍走随机
        let predict_date = next_trading_day_from_today();
        let lookback = factor_model::clamp_lookback(lookback_days);
        let window = factor_model::take_lookback(bars, lookback);
        let signal = placeholder_signal(stock, algorithm, current_price, predict_date);
        return build_from_internal(stock, algorithm, current_price, lookback as u32, predict_date, signal, window);
    }

    let ensemble = strategy::evaluate_historical(stock, bars, &compose, None, None);
    build_result(
        stock,
        bars,
        current_price,
        lookback_days,
        algorithm,
        next_trading_day_from_today(),
        ensemble,
    )
}

pub fn predict_direction(
    stock: &Stock,
    algorithm: &str,
    bars: &[DailyBar],
    current_price: f64,
    predict_date: NaiveDate,
    lookback_days: u32,
) -> DirectionSignal {
    let _ = (stock, current_price, predict_date);
    let mut compose = strategy::default_compose();
    compose.lookback_days = lookback_days;
    for s in &mut compose.sources {
        s.enabled = s.id == "factor" || s.id == "momentum" || s.id == "volume";
        if algorithm == "placeholder_v1" {
            s.enabled = s.id == "factor";
        }
    }
    if algorithm == "placeholder_v1" {
        let signal = placeholder_signal(stock, algorithm, current_price, predict_date);
        return DirectionSignal {
            up_probability: round1(signal.up_probability),
            down_probability: round1(signal.down_probability),
            confidence: round1(signal.confidence),
            predicted: resolve_direction(signal.up_probability, signal.down_probability),
            high_confidence: is_high_confidence(signal.up_probability, signal.down_probability),
        };
    }
    predict_direction_compose(stock, bars, &compose, None, None)
}

fn build_result(
    stock: &Stock,
    bars: &[DailyBar],
    current_price: f64,
    lookback_days: u32,
    algorithm: &str,
    predict_date: NaiveDate,
    ensemble: EnsembleSignal,
) -> PredictionResult {
    let lookback = factor_model::clamp_lookback(lookback_days);
    let seed = hash_seed(&stock.code, algorithm, predict_date);
    let volatility = market_vol(bars, lookback);
    let trend_bias = ((ensemble.up_probability - ensemble.down_probability) / 100.0 * 0.03).clamp(-0.03, 0.03);

    let open_skew = (ensemble.up_probability - ensemble.down_probability) / 100.0 * 0.015;
    let high_open_pct = (volatility * 0.75 + open_skew.max(0.0) + 0.004).clamp(0.003, 0.04);
    let low_open_pct = -(volatility * 0.75 + (-open_skew).max(0.0) + 0.004).clamp(0.003, 0.04);

    let high_open = build_scenario(
        "高开场景",
        current_price,
        current_price * (1.0 + high_open_pct),
        volatility,
        trend_bias + 0.008,
        &mut SeededRng::new(seed.wrapping_add(1)),
    );
    let low_open = build_scenario(
        "低开场景",
        current_price,
        current_price * (1.0 + low_open_pct),
        volatility,
        trend_bias - 0.008,
        &mut SeededRng::new(seed.wrapping_add(2)),
    );

    let bias_label = if ensemble.high_confidence {
        if ensemble.predicted == "up" {
            "高置信看涨"
        } else {
            "高置信看跌"
        }
    } else if ensemble.predicted == "up" {
        "看涨"
    } else {
        "看跌"
    };

    let active = ensemble
        .contributions
        .iter()
        .filter(|c| c.weight_normalized > 0.0)
        .count();

    let summary = if ensemble.summary_hint.is_empty() {
        format!(
            "组合策略（{} 个信号源 · 近 {} 日）预测 {} {} {}，上涨 {:.1}% / 下跌 {:.1}%。",
            active,
            lookback,
            stock.name,
            predict_date.format("%Y-%m-%d"),
            bias_label,
            ensemble.up_probability,
            ensemble.down_probability,
        )
    } else {
        format!(
            "组合策略（{} 个信号源 · 近 {} 日）预测 {} {} {}，上涨 {:.1}% / 下跌 {:.1}%。明细：{}。",
            active,
            lookback,
            stock.name,
            predict_date.format("%Y-%m-%d"),
            bias_label,
            ensemble.up_probability,
            ensemble.down_probability,
            ensemble.summary_hint,
        )
    };

    PredictionResult {
        stock: stock.clone(),
        predict_date: predict_date.format("%Y-%m-%d").to_string(),
        current_price: round2(current_price),
        up_probability: round1(ensemble.up_probability),
        down_probability: round1(ensemble.down_probability),
        flat_probability: 0.0,
        confidence: round1(ensemble.confidence),
        predicted: ensemble.predicted,
        high_confidence: ensemble.high_confidence,
        high_confidence_threshold: HIGH_CONF_THRESHOLD,
        algorithm: algorithm.to_string(),
        high_open,
        low_open,
        summary,
        signals: ensemble.contributions,
    }
}

struct InternalSignal {
    up_probability: f64,
    down_probability: f64,
    confidence: f64,
    volatility: f64,
    trend_bias: f64,
    summary_hint: String,
}

fn build_from_internal(
    stock: &Stock,
    algorithm: &str,
    current_price: f64,
    lookback: u32,
    predict_date: NaiveDate,
    signal: InternalSignal,
    _bars: &[DailyBar],
) -> PredictionResult {
    let ensemble = EnsembleSignal {
        up_probability: signal.up_probability,
        down_probability: signal.down_probability,
        confidence: signal.confidence,
        predicted: resolve_direction(signal.up_probability, signal.down_probability),
        high_confidence: is_high_confidence(signal.up_probability, signal.down_probability),
        summary_hint: signal.summary_hint,
        contributions: vec![],
    };
    let _ = signal.volatility;
    let _ = signal.trend_bias;
    build_result(stock, &[], current_price, lookback, algorithm, predict_date, ensemble)
}

fn placeholder_signal(
    stock: &Stock,
    algorithm: &str,
    current_price: f64,
    predict_date: NaiveDate,
) -> InternalSignal {
    let _ = current_price;
    let seed = hash_seed(&stock.code, algorithm, predict_date);
    let mut rng = SeededRng::new(seed);
    let volatility = 0.012 + rng.next_f64() * 0.015;
    let trend_bias = (rng.next_f64() - 0.5) * 0.04;
    let raw_up = 0.35 + rng.next_f64() * 0.35 + trend_bias;
    let raw_down = 0.35 + rng.next_f64() * 0.35 - trend_bias;
    let total = (raw_up + raw_down).max(1e-9);
    let up_probability = (raw_up / total * 100.0).clamp(8.0, 92.0);
    let down_probability = 100.0 - up_probability;
    InternalSignal {
        up_probability,
        down_probability,
        confidence: 55.0 + rng.next_f64() * 35.0,
        volatility,
        trend_bias,
        summary_hint: String::new(),
    }
}

fn resolve_direction(up: f64, down: f64) -> String {
    if up >= down {
        "up".into()
    } else {
        "down".into()
    }
}

fn market_vol(bars: &[DailyBar], lookback: usize) -> f64 {
    let window = factor_model::take_lookback(bars, lookback);
    crate::market::calc_volatility(window)
}

fn build_scenario(
    label: &str,
    prev_close: f64,
    open_price: f64,
    volatility: f64,
    trend: f64,
    rng: &mut SeededRng,
) -> ScenarioForecast {
    let times = [
        "09:30", "09:45", "10:00", "10:15", "10:30", "10:45", "11:00", "11:15", "11:30", "13:00",
        "13:15", "13:30", "13:45", "14:00", "14:15", "14:30", "14:45", "15:00",
    ];
    let mut price = open_price;
    let mut path = Vec::with_capacity(times.len());
    let mut high = open_price;
    let mut low = open_price;
    for (i, time) in times.iter().enumerate() {
        if i > 0 {
            let progress = i as f64 / (times.len() - 1) as f64;
            let drift = trend * (1.0 - progress * 0.5);
            let noise = (rng.next_f64() - 0.5) * volatility * 1.5;
            price *= 1.0 + drift + noise;
        }
        high = high.max(price);
        low = low.min(price);
        path.push(PricePoint {
            time: time.to_string(),
            price: round2(price),
            volume: round0(1_000_000.0 + rng.next_f64() * 5_000_000.0),
        });
    }
    let close = path.last().map(|p| p.price).unwrap_or(open_price);
    let change_pct = (close - prev_close) / prev_close * 100.0;
    ScenarioForecast {
        label: label.to_string(),
        open_price: round2(open_price),
        close_price: round2(close),
        high_price: round2(high),
        low_price: round2(low),
        change_pct: round2(change_pct),
        path,
    }
}

fn hash_seed(code: &str, algorithm: &str, date: NaiveDate) -> u64 {
    let mut hasher = DefaultHasher::new();
    code.hash(&mut hasher);
    algorithm.hash(&mut hasher);
    date.format("%Y-%m-%d").to_string().hash(&mut hasher);
    hasher.finish()
}

fn round0(v: f64) -> f64 {
    v.round()
}
fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}
fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

struct SeededRng {
    state: u64,
}

impl SeededRng {
    fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }
    fn next_f64(&mut self) -> f64 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (self.state >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategy::StrategyCompose;

    #[test]
    fn weekend_predicts_monday() {
        // 2026-07-17 Friday → Monday 2026-07-20
        let fri = NaiveDate::from_ymd_opt(2026, 7, 17).unwrap();
        assert_eq!(
            next_trading_day(fri),
            NaiveDate::from_ymd_opt(2026, 7, 20).unwrap()
        );
        // Saturday → Monday
        let sat = NaiveDate::from_ymd_opt(2026, 7, 18).unwrap();
        assert_eq!(
            next_trading_day(sat),
            NaiveDate::from_ymd_opt(2026, 7, 20).unwrap()
        );
        // Sunday → Monday
        let sun = NaiveDate::from_ymd_opt(2026, 7, 19).unwrap();
        assert_eq!(
            next_trading_day(sun),
            NaiveDate::from_ymd_opt(2026, 7, 20).unwrap()
        );
        // Thursday → Friday
        let thu = NaiveDate::from_ymd_opt(2026, 7, 16).unwrap();
        assert_eq!(
            next_trading_day(thu),
            NaiveDate::from_ymd_opt(2026, 7, 17).unwrap()
        );
    }

    fn bars_uptrend(n: usize) -> Vec<DailyBar> {
        (0..n)
            .map(|i| DailyBar {
                date: format!("2024-01-{:02}", (i % 28) + 1),
                open: 100.0 + i as f64,
                close: 100.0 + i as f64,
                high: 101.0 + i as f64,
                low: 99.0 + i as f64,
                volume: 1_000_000.0,
                change_pct: None,
            })
            .collect()
    }

    #[test]
    fn compose_historical_works() {
        let stock = Stock {
            code: "600519".into(),
            name: "贵州茅台".into(),
            market: "SH".into(),
            sector: "白酒".into(),
            price: None,
            change_pct: None,
            is_hot: false,
        };
        let bars = bars_uptrend(40);
        let compose = StrategyCompose {
            lookback_days: 25,
            sources: strategy::default_compose().sources,
        };
        let result = predict_compose_historical(
            &stock,
            &bars,
            139.0,
            NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
            &compose,
            None,
        );
        assert!(!result.signals.is_empty());
    }
}
