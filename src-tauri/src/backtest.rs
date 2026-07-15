use crate::factor_model;
use crate::models::{BacktestRecord, BacktestResult, DailyBar, Stock};
use crate::predictor;
use crate::strategy::StrategyCompose;

pub fn run_compose(
    stock: &Stock,
    bars: &[DailyBar],
    compose: &StrategyCompose,
) -> BacktestResult {
    let compose = crate::strategy::normalize_compose(compose);
    let lookback = factor_model::clamp_lookback(compose.lookback_days);
    if bars.len() < lookback + 2 {
        return empty_result(stock, "compose", bars.len(), lookback);
    }

    let mut records = Vec::new();
    let mut correct = 0u32;
    let mut up_hits = 0u32;
    let mut up_total = 0u32;
    let mut down_hits = 0u32;
    let mut down_total = 0u32;
    let mut hc_correct = 0u32;
    let mut hc_total = 0u32;

    for i in lookback..(bars.len() - 1) {
        let window = &bars[i + 1 - lookback..=i];
        let current_price = bars[i].close;
        if current_price <= 0.0 {
            continue;
        }

        let signal = predictor::predict_direction_compose(window, &compose);
        let next_close = bars[i + 1].close;
        let change_pct = (next_close - current_price) / current_price * 100.0;
        let actual = classify_change(change_pct);
        let is_correct = signal.predicted == actual;

        if is_correct {
            correct += 1;
        }
        if signal.predicted == "up" {
            up_total += 1;
            if actual == "up" {
                up_hits += 1;
            }
        } else {
            down_total += 1;
            if actual == "down" {
                down_hits += 1;
            }
        }
        if signal.high_confidence {
            hc_total += 1;
            if is_correct {
                hc_correct += 1;
            }
        }

        records.push(BacktestRecord {
            date: bars[i].date.clone(),
            predict_date: bars[i + 1].date.clone(),
            close_price: round2(current_price),
            next_close: round2(next_close),
            change_pct: round2(change_pct),
            predicted: signal.predicted,
            actual: actual.to_string(),
            up_probability: signal.up_probability,
            down_probability: signal.down_probability,
            confidence: signal.confidence,
            high_confidence: signal.high_confidence,
            correct: is_correct,
        });
    }

    let total = records.len() as u32;
    let direction_accuracy = pct(correct, total);
    let up_hit_rate = pct(up_hits, up_total);
    let down_hit_rate = pct(down_hits, down_total);
    let high_confidence_accuracy = pct(hc_correct, hc_total);
    let threshold = predictor::HIGH_CONF_THRESHOLD;

    let summary = format!(
        "近 {} 个交易日组合回测（回看 {} 日，仅技术类信号）：整体 {:.1}%；高置信 {} 次 / {:.1}%。",
        total, lookback, direction_accuracy, hc_total, high_confidence_accuracy,
    );

    BacktestResult {
        stock: stock.clone(),
        algorithm: "compose".into(),
        total_samples: total,
        direction_accuracy,
        actionable_accuracy: direction_accuracy,
        up_hit_rate,
        down_hit_rate,
        high_confidence_samples: hc_total,
        high_confidence_accuracy,
        high_confidence_threshold: threshold,
        flat_threshold_pct: 0.0,
        lookback_days: lookback as u32,
        summary,
        records,
    }
}

/// 兼容旧接口
pub fn run(stock: &Stock, algorithm: &str, bars: &[DailyBar], lookback_days: u32) -> BacktestResult {
    let mut compose = crate::strategy::default_compose();
    compose.lookback_days = lookback_days;
    for s in &mut compose.sources {
        s.enabled = s.id == "factor" || s.id == "momentum" || s.id == "volume";
        if algorithm == "placeholder_v1" {
            s.enabled = s.id == "factor";
        }
    }
    run_compose(stock, bars, &compose)
}

fn classify_change(change_pct: f64) -> &'static str {
    if change_pct > 0.0 {
        "up"
    } else {
        "down"
    }
}

fn pct(n: u32, total: u32) -> f64 {
    if total == 0 {
        0.0
    } else {
        round1(n as f64 / total as f64 * 100.0)
    }
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn empty_result(stock: &Stock, algorithm: &str, bar_count: usize, lookback: usize) -> BacktestResult {
    BacktestResult {
        stock: stock.clone(),
        algorithm: algorithm.to_string(),
        total_samples: 0,
        direction_accuracy: 0.0,
        actionable_accuracy: 0.0,
        up_hit_rate: 0.0,
        down_hit_rate: 0.0,
        high_confidence_samples: 0,
        high_confidence_accuracy: 0.0,
        high_confidence_threshold: predictor::HIGH_CONF_THRESHOLD,
        flat_threshold_pct: 0.0,
        lookback_days: lookback as u32,
        summary: format!(
            "历史数据不足（仅 {} 根 K 线，至少需要 {} 根）",
            bar_count,
            lookback + 2
        ),
        records: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(date: &str, close: f64) -> DailyBar {
        DailyBar {
            date: date.into(),
            open: close,
            close,
            high: close * 1.01,
            low: close * 0.99,
            volume: 1_000_000.0,
            change_pct: None,
        }
    }

    #[test]
    fn backtest_compose_works() {
        let stock = Stock {
            code: "600519".into(),
            name: "贵州茅台".into(),
            market: "SH".into(),
            sector: "白酒".into(),
            price: None,
            change_pct: None,
            is_hot: false,
        };
        let bars: Vec<DailyBar> = (0..60)
            .map(|i| bar(&format!("2024-01-{:02}", (i % 28) + 1), 100.0 + i as f64 * 0.5))
            .collect();
        let result = run(&stock, "factor_v1", &bars, 25);
        assert!(result.total_samples > 0);
    }
}
