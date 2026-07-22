//! 技术面单源信号：bars → 得分/结论（再经 fuse 成概率）。

use crate::algo::factor::{self, FactorStyle};
use crate::algo::fuse::{contrib, neutral};
use crate::models::{DailyBar, SignalContribution, Stock};

pub fn eval_factor(stock: &Stock, bars: &[DailyBar], horizon_days: u32) -> SignalContribution {
    let style = factor::style_for_stock(stock);
    match factor::compute_styled_for_horizon(bars, style, horizon_days) {
        Some(f) => {
            let note = if f.hints.is_empty() {
                format!("综合得分 {:+.2}", f.score)
            } else {
                f.hints.join("，")
            };
            contrib("factor", "技术多因子", "技术面", f.score, note, "ok")
        }
        None => neutral("factor", "技术多因子", "K线不足"),
    }
}

pub fn eval_momentum(stock: &Stock, bars: &[DailyBar], horizon_days: u32) -> SignalContribution {
    if bars.len() < 15 {
        return neutral("momentum", "趋势动量", "K线不足");
    }
    let n = bars.len();
    let c0 = bars[n - 1].close;
    let c1 = bars[n.saturating_sub(2)].close;
    let c3 = bars[n.saturating_sub(4)].close;
    let c5 = bars[n.saturating_sub(6)].close;
    let c10 = bars[n.saturating_sub(11)].close;
    if c1 <= 0.0 || c3 <= 0.0 || c5 <= 0.0 || c10 <= 0.0 {
        return neutral("momentum", "趋势动量", "价格异常");
    }
    let m1 = (c0 - c1) / c1;
    let m3 = (c0 - c3) / c3;
    let m5 = (c0 - c5) / c5;
    let m10 = (c0 - c10) / c10;
    let h = horizon_days.clamp(1, 5) as usize;
    let ch = bars[n.saturating_sub(h + 1)].close;
    let mh = if ch > 0.0 { (c0 - ch) / ch } else { m5 };

    let (score, note) = if h <= 1 {
        if factor::style_for_stock(stock) == FactorStyle::IndexEtf {
            let score = if m3.abs() < 0.008 {
                0.0
            } else {
                (m3 * 12.0).clamp(-1.0, 1.0)
            };
            (
                score,
                format!("宽基互补·3日动量 {:+.1}%（不重复隔日反向）", m3 * 100.0),
            )
        } else {
            (
                (m5 * 12.0 + m10 * 6.0).clamp(-2.5, 2.5),
                format!(
                    "1日 {:+.1}% / 5日 {:+.1}% / 10日 {:+.1}%",
                    m1 * 100.0,
                    m5 * 100.0,
                    m10 * 100.0
                ),
            )
        }
    } else {
        let score = (mh * 14.0 + m10 * 5.0).clamp(-2.5, 2.5);
        (
            score,
            format!("{h}日动量 {:+.1}% / 10日 {:+.1}%", mh * 100.0, m10 * 100.0),
        )
    };
    contrib("momentum", "趋势动量", "技术面", score, note, "ok")
}

pub fn eval_mean_reversion(bars: &[DailyBar], horizon_days: u32) -> SignalContribution {
    if bars.len() < 25 {
        return neutral("mean_reversion", "均值回归", "K线不足");
    }
    let factors = match factor::compute(bars) {
        Some(f) => f,
        None => return neutral("mean_reversion", "均值回归", "指标不足"),
    };
    let price = bars.last().map(|b| b.close).unwrap_or(0.0);
    let dev = if factors.ma20 > 0.0 {
        (price - factors.ma20) / factors.ma20
    } else {
        0.0
    };
    let mut score = (-dev * 15.0).clamp(-2.5, 2.5);
    if factors.rsi14 > 70.0 {
        score -= 0.6;
    } else if factors.rsi14 < 30.0 {
        score += 0.6;
    }
    if horizon_days > 1 {
        score *= 0.35;
    }
    contrib(
        "mean_reversion",
        "均值回归",
        "技术面",
        score.clamp(-2.5, 2.5),
        format!("相对MA20 {:+.1}% · RSI {:.0}", dev * 100.0, factors.rsi14),
        "ok",
    )
}

pub fn eval_volume(stock: &Stock, bars: &[DailyBar], horizon_days: u32) -> SignalContribution {
    if bars.len() < 20 {
        return neutral("volume", "量价确认", "K线不足");
    }
    let n = bars.len();
    let today = &bars[n - 1];
    let prev = &bars[n - 2];
    let avg_vol: f64 = bars[n - 20..].iter().map(|b| b.volume).sum::<f64>() / 20.0;
    let vr = if avg_vol > 0.0 {
        today.volume / avg_vol
    } else {
        1.0
    };
    let chg = if prev.close > 0.0 {
        (today.close - prev.close) / prev.close
    } else {
        0.0
    };

    let index = factor::style_for_stock(stock) == FactorStyle::IndexEtf;
    let (score, note) = if horizon_days > 1 {
        if vr > 1.4 && chg > 0.005 {
            (0.9, format!("放量确认上涨 · 量比 {:.1}", vr))
        } else if vr > 1.4 && chg < -0.005 {
            (-0.9, format!("放量确认下跌 · 量比 {:.1}", vr))
        } else if vr < 0.7 {
            (0.1 * chg.signum(), format!("缩量整理 · 量比 {:.1}", vr))
        } else {
            (chg * 6.0, format!("量能中性 · 量比 {:.1}", vr))
        }
    } else if index {
        if vr > 1.4 && chg > 0.005 {
            (-0.6, format!("放量追涨慎用 · 量比 {:.1}", vr))
        } else if vr > 1.4 && chg < -0.005 {
            (0.5, format!("放量下跌或钝化 · 量比 {:.1}", vr))
        } else if vr < 0.7 {
            (0.15 * (-chg.signum()), format!("缩量 · 量比 {:.1}", vr))
        } else {
            (-chg * 5.0, format!("量能中性偏反向 · 量比 {:.1}", vr))
        }
    } else if vr > 1.4 && chg > 0.005 {
        (1.2, format!("放量上涨 · 量比 {:.1}", vr))
    } else if vr > 1.4 && chg < -0.005 {
        (-1.2, format!("放量下跌 · 量比 {:.1}", vr))
    } else if vr < 0.7 {
        (-0.2 * chg.signum(), format!("缩量整理 · 量比 {:.1}", vr))
    } else {
        (chg * 8.0, format!("量能中性 · 量比 {:.1}", vr))
    };
    contrib(
        "volume",
        "量价确认",
        "技术面",
        score.clamp(-2.5, 2.5),
        note,
        "ok",
    )
}
