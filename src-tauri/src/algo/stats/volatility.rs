use crate::models::DailyBar;

/// 根据日线收盘价计算历史波动率（日收益率标准差）。
pub fn calc_volatility(bars: &[DailyBar]) -> f64 {
    if bars.len() < 5 {
        return 0.02;
    }

    let mut returns = Vec::new();
    for w in bars.windows(2) {
        let prev = w[0].close;
        let curr = w[1].close;
        if prev > 0.0 && curr > 0.0 {
            returns.push((curr - prev) / prev);
        }
    }

    if returns.is_empty() {
        return 0.02;
    }

    let mean = returns.iter().sum::<f64>() / returns.len() as f64;
    let variance = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / returns.len() as f64;
    variance.sqrt().clamp(0.005, 0.08)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(close: f64) -> DailyBar {
        DailyBar {
            date: "2024-01-01".into(),
            open: close,
            close,
            high: close,
            low: close,
            volume: 1.0,
            change_pct: None,
        }
    }

    #[test]
    fn volatility_clamped() {
        let bars: Vec<_> = (0..30).map(|i| bar(100.0 + i as f64)).collect();
        let v = calc_volatility(&bars);
        assert!(v >= 0.005 && v <= 0.08);
    }
}
