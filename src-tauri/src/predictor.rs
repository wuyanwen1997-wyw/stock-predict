use crate::models::{PricePoint, PredictionResult, ScenarioForecast, Stock};
use chrono::{Local, NaiveDate};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// 占位预测算法 — 使用真实收盘价与历史波动率，其余逻辑为确定性伪随机，便于演示 UI。
/// 后续可替换为 LSTM、XGBoost、因子模型等真实算法。
pub fn predict(stock: &Stock, algorithm: &str, current_price: f64, volatility_hint: f64) -> PredictionResult {
    let seed = hash_seed(&stock.code, algorithm);
    let mut rng = SeededRng::new(seed);

    let base_price = current_price;
    let volatility = (volatility_hint * 0.6 + (0.012 + rng.next_f64() * 0.008)).clamp(0.008, 0.05);
    let trend_bias = (rng.next_f64() - 0.5) * 0.04;

    let raw_up = 0.35 + rng.next_f64() * 0.35 + trend_bias;
    let raw_down = 0.35 + rng.next_f64() * 0.35 - trend_bias;
    let total = raw_up + raw_down + 0.08;
    let up_probability = (raw_up / total * 100.0).clamp(5.0, 90.0);
    let down_probability = (raw_down / total * 100.0).clamp(5.0, 90.0);
    let flat_probability = (100.0 - up_probability - down_probability).max(2.0);

    let confidence = 55.0 + rng.next_f64() * 35.0;

    let high_open_pct = 0.008 + rng.next_f64() * 0.025;
    let low_open_pct = -(0.008 + rng.next_f64() * 0.025);

    let high_open = build_scenario(
        "高开场景",
        base_price,
        base_price * (1.0 + high_open_pct),
        volatility,
        trend_bias + 0.01,
        &mut SeededRng::new(seed.wrapping_add(1)),
    );

    let low_open = build_scenario(
        "低开场景",
        base_price,
        base_price * (1.0 + low_open_pct),
        volatility,
        trend_bias - 0.01,
        &mut SeededRng::new(seed.wrapping_add(2)),
    );

    let tomorrow = Local::now().date_naive() + chrono::Duration::days(1);
    let summary = format!(
        "模型预测 {} 明日上涨概率 {:.1}%，下跌概率 {:.1}%。高开场景预计收 {:.2}（{:+.2}%），低开场景预计收 {:.2}（{:+.2}%）。",
        stock.name,
        up_probability,
        down_probability,
        high_open.close_price,
        high_open.change_pct,
        low_open.close_price,
        low_open.change_pct,
    );

    PredictionResult {
        stock: stock.clone(),
        predict_date: tomorrow.format("%Y-%m-%d").to_string(),
        current_price: round2(base_price),
        up_probability: round1(up_probability),
        down_probability: round1(down_probability),
        flat_probability: round1(flat_probability),
        confidence: round1(confidence),
        algorithm: algorithm.to_string(),
        high_open,
        low_open,
        summary,
    }
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
        "09:30", "09:45", "10:00", "10:15", "10:30", "10:45",
        "11:00", "11:15", "11:30", "13:00", "13:15", "13:30",
        "13:45", "14:00", "14:15", "14:30", "14:45", "15:00",
    ];

    let mut price = open_price;
    let mut path = Vec::with_capacity(times.len());
    let mut high = open_price;
    let mut low = open_price;

    for (i, time) in times.iter().enumerate() {
        if i > 0 {
            let progress = i as f64 / (times.len() - 1) as f64;
            let drift = trend * (1.0 - progress * 0.5);
            let noise = (rng.next_f64() - 0.5) * volatility * 2.0;
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

fn hash_seed(code: &str, algorithm: &str) -> u64 {
    let today: NaiveDate = Local::now().date_naive();
    let mut hasher = DefaultHasher::new();
    code.hash(&mut hasher);
    algorithm.hash(&mut hasher);
    today.format("%Y-%m-%d").to_string().hash(&mut hasher);
    hasher.finish()
}

fn round0(v: f64) -> f64 { (v * 1.0).round() / 1.0 }
fn round1(v: f64) -> f64 { (v * 10.0).round() / 10.0 }
fn round2(v: f64) -> f64 { (v * 100.0).round() / 100.0 }

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

    #[test]
    fn predict_is_deterministic() {
        let stock = Stock {
            code: "600519".into(),
            name: "贵州茅台".into(),
            market: "SH".into(),
            sector: "白酒".into(),
            price: None,
            change_pct: None,
            is_hot: false,
        };
        let a = predict(&stock, "placeholder_v1", 1680.0, 0.02);
        let b = predict(&stock, "placeholder_v1", 1680.0, 0.02);
        assert_eq!(a.up_probability, b.up_probability);
        assert_eq!(a.high_open.path.len(), b.high_open.path.len());
    }
}
