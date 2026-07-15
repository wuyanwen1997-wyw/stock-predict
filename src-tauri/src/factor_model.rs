use crate::models::DailyBar;
use crate::market;

/// 技术指标快照
#[derive(Debug, Clone)]
pub struct FactorSnapshot {
    pub ma5: f64,
    pub ma10: f64,
    pub ma20: f64,
    pub rsi14: f64,
    pub momentum5: f64,
    pub momentum10: f64,
    pub volume_ratio: f64,
    pub volatility: f64,
    pub score: f64,
    pub hints: Vec<String>,
}

pub const MIN_BARS: usize = 25;

pub fn clamp_lookback(days: u32) -> usize {
    match days {
        25 | 50 | 60 | 90 | 120 => days as usize,
        _ if days < 25 => 25,
        _ if days > 120 => 120,
        _ => 50,
    }
}

pub fn take_lookback<'a>(bars: &'a [DailyBar], lookback: usize) -> &'a [DailyBar] {
    let n = lookback.max(MIN_BARS);
    if bars.len() <= n {
        bars
    } else {
        &bars[bars.len() - n..]
    }
}

pub fn compute(bars: &[DailyBar]) -> Option<FactorSnapshot> {
    if bars.len() < MIN_BARS {
        return None;
    }

    let price = bars.last()?.close;
    if price <= 0.0 {
        return None;
    }

    let ma5 = sma(bars, 5)?;
    let ma10 = sma(bars, 10)?;
    let ma20 = sma(bars, 20)?;
    let rsi14 = rsi(bars, 14)?;
    let momentum5 = momentum(bars, 5)?;
    let momentum10 = momentum(bars, 10)?;
    let vol_period = 20.min(bars.len());
    let volume_ratio = volume_ratio(bars, vol_period)?;
    let volatility = market::calc_volatility(bars);

    let (score, hints) = score_factors(price, ma5, ma10, ma20, rsi14, momentum5, momentum10, volume_ratio);

    Some(FactorSnapshot {
        ma5,
        ma10,
        ma20,
        rsi14,
        momentum5,
        momentum10,
        volume_ratio,
        volatility,
        score,
        hints,
    })
}

pub struct FactorSignal {
    pub up_probability: f64,
    pub down_probability: f64,
    pub confidence: f64,
    pub volatility: f64,
    pub trend_bias: f64,
    pub summary_hint: String,
}

pub fn to_signal(factors: &FactorSnapshot) -> FactorSignal {
    let score = factors.score;
    let vol = factors.volatility;

    let strength = score.abs().clamp(0.0, 2.5) / 2.5;
    let confidence = (45.0 + strength * 40.0 + (1.0 - vol / 0.05).clamp(0.0, 1.0) * 10.0).clamp(40.0, 92.0);

    // 二分类：上涨 + 下跌 = 100%
    let up_share = (0.5 + (score / 2.5).clamp(-0.45, 0.45)).clamp(0.08, 0.92);
    let up = up_share * 100.0;
    let down = 100.0 - up;

    let trend_bias = (score * 0.012).clamp(-0.03, 0.03);
    let summary_hint = factors.hints.join("，");

    FactorSignal {
        up_probability: up,
        down_probability: down,
        confidence,
        volatility: vol,
        trend_bias,
        summary_hint,
    }
}

fn score_factors(
    price: f64,
    ma5: f64,
    ma10: f64,
    ma20: f64,
    rsi14: f64,
    momentum5: f64,
    momentum10: f64,
    volume_ratio: f64,
) -> (f64, Vec<String>) {
    let mut score = 0.0;
    let mut hints = Vec::new();

    // 均线排列
    if price > ma5 && ma5 > ma10 && ma10 > ma20 {
        score += 1.0;
        hints.push("均线多头排列".into());
    } else if price < ma5 && ma5 < ma10 && ma10 < ma20 {
        score -= 1.0;
        hints.push("均线空头排列".into());
    } else {
        if price > ma20 {
            score += 0.25;
        } else {
            score -= 0.25;
        }
        hints.push("均线交织".into());
    }

    // 价格相对 MA20 偏离（均值回归 + 趋势）
    let ma_dev = (price - ma20) / ma20;
    if ma_dev > 0.06 {
        score -= 0.4;
        hints.push("偏离 MA20 过远".into());
    } else if ma_dev < -0.06 {
        score += 0.4;
        hints.push("低于 MA20 较多".into());
    } else if ma_dev > 0.0 {
        score += 0.2;
    } else {
        score -= 0.2;
    }

    // RSI
    if rsi14 < 32.0 {
        score += 0.7;
        hints.push(format!("RSI {:.0} 超卖", rsi14));
    } else if rsi14 > 68.0 {
        score -= 0.7;
        hints.push(format!("RSI {:.0} 超买", rsi14));
    } else if rsi14 > 55.0 {
        score += 0.25;
    } else if rsi14 < 45.0 {
        score -= 0.25;
    }

    // 动量
    score += momentum5.clamp(-0.06, 0.06) * 10.0;
    score += momentum10.clamp(-0.08, 0.08) * 5.0;
    if momentum5 > 0.02 {
        hints.push(format!("5日动量 {:+.1}%", momentum5 * 100.0));
    } else if momentum5 < -0.02 {
        hints.push(format!("5日动量 {:+.1}%", momentum5 * 100.0));
    }

    // 量能确认
    if volume_ratio > 1.4 {
        if score > 0.0 {
            score += 0.25;
            hints.push("放量上涨".into());
        } else if score < 0.0 {
            score -= 0.25;
            hints.push("放量下跌".into());
        }
    } else if volume_ratio < 0.7 {
        score *= 0.85;
        hints.push("缩量整理".into());
    }

    (score.clamp(-2.5, 2.5), hints)
}

fn sma(bars: &[DailyBar], period: usize) -> Option<f64> {
    if bars.len() < period {
        return None;
    }
    let sum: f64 = bars[bars.len() - period..].iter().map(|b| b.close).sum();
    Some(sum / period as f64)
}

fn momentum(bars: &[DailyBar], period: usize) -> Option<f64> {
    if bars.len() <= period {
        return None;
    }
    let curr = bars.last()?.close;
    let prev = bars[bars.len() - 1 - period].close;
    if prev <= 0.0 {
        return None;
    }
    Some((curr - prev) / prev)
}

fn volume_ratio(bars: &[DailyBar], period: usize) -> Option<f64> {
    if bars.len() < period {
        return None;
    }
    let avg: f64 = bars[bars.len() - period..].iter().map(|b| b.volume).sum::<f64>() / period as f64;
    let today = bars.last()?.volume;
    if avg <= 0.0 {
        return Some(1.0);
    }
    Some(today / avg)
}

fn rsi(bars: &[DailyBar], period: usize) -> Option<f64> {
    if bars.len() <= period {
        return None;
    }

    let slice = &bars[bars.len() - period - 1..];
    let mut gains = 0.0;
    let mut losses = 0.0;

    for w in slice.windows(2) {
        let delta = w[1].close - w[0].close;
        if delta > 0.0 {
            gains += delta;
        } else {
            losses -= delta;
        }
    }

    if losses < 1e-9 {
        return Some(100.0);
    }
    let rs = gains / losses;
    Some(100.0 - 100.0 / (1.0 + rs))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(close: f64, volume: f64) -> DailyBar {
        DailyBar {
            date: "2024-01-01".into(),
            open: close,
            close,
            high: close * 1.01,
            low: close * 0.99,
            volume,
            change_pct: None,
        }
    }

    #[test]
    fn uptrend_scores_positive() {
        let bars: Vec<DailyBar> = (0..30)
            .map(|i| bar(100.0 + i as f64, 1_000_000.0 + i as f64 * 10_000.0))
            .collect();
        let factors = compute(&bars).expect("factors");
        assert!(factors.score > 0.0);
        let signal = to_signal(&factors);
        assert!(signal.up_probability > signal.down_probability);
    }

    #[test]
    fn downtrend_scores_negative() {
        let bars: Vec<DailyBar> = (0..30)
            .map(|i| bar(130.0 - i as f64, 1_000_000.0))
            .collect();
        let factors = compute(&bars).expect("factors");
        assert!(factors.score < 0.0);
    }
}
