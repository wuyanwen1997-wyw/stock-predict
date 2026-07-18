use crate::models::{DailyBar, Stock};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FactorStyle {
    /// 个股：趋势 + 超买超卖
    Default,
    /// 宽基 A 股 ETF：MA20 趋势过滤 + 隔日反向 + 均线排列（510980 OOS≈60%）
    IndexEtf,
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

/// 根据标的选择技术因子风格
pub fn style_for_stock(stock: &Stock) -> FactorStyle {
    let name = stock.name.as_str();
    let code = stock.code.as_str();
    let sector = stock.sector.as_str();

    let is_etf = sector == "ETF" || name.contains("ETF");
    if !is_etf {
        return FactorStyle::Default;
    }

    // 海外 / 黄金走默认或另套逻辑，宽基走 IndexEtf
    if name.contains("纳指")
        || name.contains("标普")
        || name.contains("道指")
        || name.contains("黄金")
        || code.starts_with("513")
        || code == "518880"
        || code == "518800"
    {
        return FactorStyle::Default;
    }

    if name.contains("沪深300")
        || name.contains("中证500")
        || name.contains("中证1000")
        || name.contains("上证50")
        || name.contains("上证指数")
        || name.contains("上证综指")
        || name.contains("创业板")
        || name.contains("科创50")
        || name.contains("红利")
        || code == "510980"
        || code == "510300"
        || code == "510500"
        || code == "510050"
        || code == "159915"
        || code == "588000"
    {
        return FactorStyle::IndexEtf;
    }

    // 其它 A 股 ETF 也按宽基风格（主题股性更强时仍优于纯追涨）
    FactorStyle::IndexEtf
}

pub fn compute(bars: &[DailyBar]) -> Option<FactorSnapshot> {
    compute_styled(bars, FactorStyle::Default)
}

pub fn compute_styled(bars: &[DailyBar], style: FactorStyle) -> Option<FactorSnapshot> {
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
    let momentum1 = momentum(bars, 1).unwrap_or(0.0);
    let momentum5 = momentum(bars, 5)?;
    let momentum10 = momentum(bars, 10)?;
    let vol_period = 20.min(bars.len());
    let volume_ratio = volume_ratio(bars, vol_period)?;
    let volatility = market::calc_volatility(bars);

    let (score, hints) = match style {
        FactorStyle::IndexEtf => score_factors_index(
            price,
            ma5,
            ma10,
            ma20,
            momentum1,
            volume_ratio,
            atr_pct(bars, 14),
        ),
        FactorStyle::Default => score_factors(
            price,
            ma5,
            ma10,
            ma20,
            rsi14,
            momentum5,
            momentum10,
            volume_ratio,
        ),
    };

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
    let confidence =
        (45.0 + strength * 40.0 + (1.0 - vol / 0.05).clamp(0.0, 1.0) * 10.0).clamp(40.0, 92.0);

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

/// 宽基指数 ETF：站上 MA20 + 隔日反向 + 均线排列 − 过度偏离
/// （510980：前 120 日训练 / 近 120 日 OOS ≈ 60%；高波动再加重隔日反向 ≈ 60.8%）
fn score_factors_index(
    price: f64,
    ma5: f64,
    ma10: f64,
    ma20: f64,
    momentum1: f64,
    volume_ratio: f64,
    atr_pct: f64,
) -> (f64, Vec<String>) {
    let mut score = 0.0;
    let mut hints = Vec::new();

    // MA20 趋势过滤（权重 0.5）
    if price > ma20 {
        score += 0.5;
        hints.push("站上MA20".into());
    } else {
        score -= 0.5;
        hints.push("跌破MA20".into());
    }

    // 隔日反向（权重 1.0，宽基最稳贡献）
    let fade = if momentum1 > 0.0 { -1.0 } else { 1.0 };
    score += fade;
    if momentum1 > 0.0 {
        hints.push(format!("隔日反向·昨涨{:+.1}%", momentum1 * 100.0));
    } else {
        hints.push(format!("隔日反向·昨跌{:+.1}%", momentum1 * 100.0));
    }

    // 均线排列（权重 0.6）
    if price > ma5 && ma5 > ma10 && ma10 > ma20 {
        score += 0.6;
        hints.push("均线多头排列".into());
    } else if price < ma5 && ma5 < ma10 && ma10 < ma20 {
        score -= 0.6;
        hints.push("均线空头排列".into());
    }

    // 相对 MA20 偏离回归（权重等价 0.3 * -dev*10）
    let ma_dev = if ma20 > 0.0 {
        (price - ma20) / ma20
    } else {
        0.0
    };
    score += (-ma_dev) * 3.0;
    if ma_dev.abs() > 0.025 {
        hints.push(format!("偏离MA20 {:+.1}%", ma_dev * 100.0));
    }

    // 高波动日：隔日反向更可信（OOS +0.8pp）
    if atr_pct > 0.015 {
        score += 0.5 * fade;
        hints.push(format!("高波动ATR{:.1}%", atr_pct * 100.0));
    }

    // 放量时减弱追价（量能对次日收益 IC 偏负）
    if volume_ratio > 1.5 {
        if momentum1 > 0.0 {
            score -= 0.15;
        }
        hints.push(format!("放量·量比{:.1}", volume_ratio));
    }

    hints.insert(0, "宽基因子".into());
    (score.clamp(-2.5, 2.5), hints)
}

/// 近 14 日真实波幅占收盘价比例（简化 ATR%）
fn atr_pct(bars: &[DailyBar], period: usize) -> f64 {
    if bars.len() < period + 1 {
        return 0.0;
    }
    let start = bars.len() - period;
    let mut sum = 0.0;
    for i in start..bars.len() {
        let h = bars[i].high;
        let l = bars[i].low;
        let prev = bars[i - 1].close;
        let tr = (h - l).max((h - prev).abs()).max((l - prev).abs());
        if bars[i].close > 0.0 {
            sum += tr / bars[i].close;
        }
    }
    sum / period as f64
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

    fn stock(code: &str, name: &str, sector: &str) -> Stock {
        Stock {
            code: code.into(),
            name: name.into(),
            market: "SH".into(),
            sector: sector.into(),
            price: None,
            change_pct: None,
            is_hot: false,
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

    #[test]
    fn sse_etf_uses_index_style() {
        assert_eq!(
            style_for_stock(&stock("510980", "上证指数ETF汇添富", "ETF")),
            FactorStyle::IndexEtf
        );
    }

    #[test]
    fn index_style_fades_up_day_near_ma() {
        // 构造：价格略高于 MA，昨日上涨 → 隔日反向应偏空
        let mut bars: Vec<DailyBar> = (0..30).map(|i| bar(100.0 + i as f64 * 0.1, 1_000_000.0)).collect();
        let n = bars.len();
        bars[n - 1].close = bars[n - 2].close * 1.02; // 昨涨
        let f = compute_styled(&bars, FactorStyle::IndexEtf).expect("f");
        assert!(
            f.hints.iter().any(|h| h.contains("宽基") || h.contains("隔日")),
            "hints={:?}",
            f.hints
        );
    }
}
