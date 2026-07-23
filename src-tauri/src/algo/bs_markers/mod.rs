//! K 线主图买卖点：标准 MACD(12,26,9) 金叉/死叉（无 IO）。
//!
//! 与融合预测 / 回测方向无关，仅供图表叠加。

use crate::models::{BsMarker, BsMarkerKind, DailyBar};

/// MACD 快线 / 慢线 / DEA 周期（通达信/同花顺默认）
pub const MACD_FAST: usize = 12;
pub const MACD_SLOW: usize = 26;
pub const MACD_SIGNAL: usize = 9;

/// 至少需要的日 K 根数（慢线种子 + 信号线稳定）
pub const MIN_BARS: usize = 35;

/// 对全日 K 计算 MACD 金叉(B) / 死叉(S)；不足 [`MIN_BARS`] 返回空。
pub fn compute_macd_bs(bars: &[DailyBar]) -> Vec<BsMarker> {
    if bars.len() < MIN_BARS {
        return Vec::new();
    }

    let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
    let ema_fast = ema_sma_seed(&closes, MACD_FAST);
    let ema_slow = ema_sma_seed(&closes, MACD_SLOW);

    let mut dif = vec![f64::NAN; closes.len()];
    for i in 0..closes.len() {
        if ema_fast[i].is_finite() && ema_slow[i].is_finite() {
            dif[i] = ema_fast[i] - ema_slow[i];
        }
    }

    let dea = ema_of_series_sma_seed(&dif, MACD_SIGNAL);

    let mut out = Vec::new();
    for i in 1..bars.len() {
        let (pd, pdea) = (dif[i - 1], dea[i - 1]);
        let (d, de) = (dif[i], dea[i]);
        if !(pd.is_finite() && pdea.is_finite() && d.is_finite() && de.is_finite()) {
            continue;
        }
        let prev = pd - pdea;
        let curr = d - de;
        // 上穿：金叉买；下穿：死叉卖。贴零不重复触发。
        let kind = if prev <= 0.0 && curr > 0.0 {
            Some(BsMarkerKind::Buy)
        } else if prev >= 0.0 && curr < 0.0 {
            Some(BsMarkerKind::Sell)
        } else {
            None
        };
        if let Some(kind) = kind {
            out.push(BsMarker {
                date: bars[i].date.clone(),
                kind,
            });
        }
    }
    out
}

/// 按 `dates` 集合过滤标记（用于 chart 窗口切片）。
pub fn filter_markers_by_dates(markers: &[BsMarker], dates: &[String]) -> Vec<BsMarker> {
    let set: std::collections::HashSet<&str> = dates.iter().map(|s| s.as_str()).collect();
    markers
        .iter()
        .filter(|m| set.contains(m.date.as_str()))
        .cloned()
        .collect()
}

fn ema_sma_seed(values: &[f64], period: usize) -> Vec<f64> {
    let mut out = vec![f64::NAN; values.len()];
    if values.len() < period || period == 0 {
        return out;
    }
    let seed: f64 = values[..period].iter().sum::<f64>() / period as f64;
    out[period - 1] = seed;
    let k = 2.0 / (period as f64 + 1.0);
    for i in period..values.len() {
        out[i] = values[i] * k + out[i - 1] * (1.0 - k);
    }
    out
}

/// 对可能含 NaN 前缀的序列做 EMA；从第一个有限值起攒满 period 再出种子。
fn ema_of_series_sma_seed(values: &[f64], period: usize) -> Vec<f64> {
    let mut out = vec![f64::NAN; values.len()];
    if period == 0 {
        return out;
    }
    let mut window = Vec::with_capacity(period);
    let mut ema: Option<f64> = None;
    let k = 2.0 / (period as f64 + 1.0);
    for i in 0..values.len() {
        let v = values[i];
        if !v.is_finite() {
            continue;
        }
        if ema.is_none() {
            window.push(v);
            if window.len() == period {
                let seed = window.iter().sum::<f64>() / period as f64;
                out[i] = seed;
                ema = Some(seed);
            }
        } else {
            let prev = ema.unwrap();
            let next = v * k + prev * (1.0 - k);
            out[i] = next;
            ema = Some(next);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(date: &str, close: f64) -> DailyBar {
        DailyBar {
            date: date.into(),
            open: close,
            close,
            high: close,
            low: close,
            volume: 1.0,
            change_pct: None,
        }
    }

    #[test]
    fn empty_when_too_few_bars() {
        let bars: Vec<_> = (0..20)
            .map(|i| bar(&format!("2024-01-{:02}", i + 1), 10.0 + i as f64 * 0.1))
            .collect();
        assert!(compute_macd_bs(&bars).is_empty());
    }

    #[test]
    fn detects_golden_and_death_cross() {
        // 先横盘，再下跌，再强势上涨，再回落 —— 应至少出现一次金叉与一次死叉
        let mut closes = Vec::new();
        for _ in 0..30 {
            closes.push(100.0);
        }
        for i in 0..20 {
            closes.push(100.0 - i as f64 * 1.5);
        }
        for i in 0..25 {
            closes.push(70.0 + i as f64 * 2.0);
        }
        for i in 0..20 {
            closes.push(120.0 - i as f64 * 1.8);
        }

        let bars: Vec<_> = closes
            .iter()
            .enumerate()
            .map(|(i, &c)| {
                let day = i + 1;
                bar(&format!("2024-{:02}-{:02}", (day / 28) + 1, (day % 28) + 1), c)
            })
            .collect();

        let markers = compute_macd_bs(&bars);
        assert!(
            markers.iter().any(|m| m.kind == BsMarkerKind::Buy),
            "expected at least one Buy (golden cross), got {markers:?}"
        );
        assert!(
            markers.iter().any(|m| m.kind == BsMarkerKind::Sell),
            "expected at least one Sell (death cross), got {markers:?}"
        );
        // 同日至多一个
        let mut dates = std::collections::HashSet::new();
        for m in &markers {
            assert!(dates.insert(m.date.clone()), "duplicate date {}", m.date);
        }
    }

    #[test]
    fn filter_by_chart_window() {
        let all = vec![
            BsMarker {
                date: "2024-01-10".into(),
                kind: BsMarkerKind::Buy,
            },
            BsMarker {
                date: "2024-02-01".into(),
                kind: BsMarkerKind::Sell,
            },
        ];
        let window = vec!["2024-02-01".into(), "2024-02-02".into()];
        let filtered = filter_markers_by_dates(&all, &window);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].date, "2024-02-01");
        assert_eq!(filtered[0].kind, BsMarkerKind::Sell);
    }
}
