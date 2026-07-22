//! 资金流评分：从归档序列得出方向/强度结论（无 IO）。

use chrono::{Duration, NaiveDate};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct CapitalFlowArchive {
    /// 日期 → 北向净买入（亿元）
    pub north_net_yi: BTreeMap<NaiveDate, f64>,
    /// 日期 → 大盘主力净流入（元）
    pub market_main: BTreeMap<NaiveDate, f64>,
    /// 日期 → 上证+深成指成交量/额之和（腾讯免费代理）
    pub activity_amount: BTreeMap<NaiveDate, f64>,
    /// 日期 → 上证收盘（用于成交代理的涨跌方向）
    pub activity_close: BTreeMap<NaiveDate, f64>,
    /// 数据来源说明（供回测摘要）
    pub source_note: String,
}

impl CapitalFlowArchive {
    pub fn north_days(&self) -> usize {
        self.north_net_yi.len()
    }

    pub fn market_days(&self) -> usize {
        self.market_main.len()
    }

    pub fn activity_days(&self) -> usize {
        self.activity_amount.len()
    }

    pub fn usable_days(&self) -> usize {
        self.market_days()
            .max(self.activity_days())
            .max(self.north_days())
    }

    pub fn is_empty(&self) -> bool {
        self.usable_days() == 0
    }

    pub fn merge(&mut self, other: &CapitalFlowArchive) {
        for (d, v) in &other.north_net_yi {
            self.north_net_yi.entry(*d).or_insert(*v);
        }
        for (d, v) in &other.market_main {
            self.market_main.insert(*d, *v);
        }
        for (d, v) in &other.activity_amount {
            self.activity_amount.insert(*d, *v);
        }
        for (d, v) in &other.activity_close {
            self.activity_close.insert(*d, *v);
        }
    }
}

#[derive(Debug, Clone)]
pub struct CapitalFlowSignal {
    pub score: f64,
    pub note: String,
    pub status: &'static str,
}

fn median_abs(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 1.0;
    }
    let mut v: Vec<f64> = values.iter().map(|x| x.abs()).collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = v.len() / 2;
    if v.len() % 2 == 0 {
        ((v[mid - 1] + v[mid]) / 2.0).max(1e-6)
    } else {
        v[mid].max(1e-6)
    }
}

fn score_series(
    as_of: NaiveDate,
    series: &BTreeMap<NaiveDate, f64>,
    unit_note: &str,
) -> Option<CapitalFlowSignal> {
    let window_start = as_of - Duration::days(40);
    let hist: Vec<(NaiveDate, f64)> = series
        .range(window_start..=as_of)
        .map(|(d, v)| (*d, *v))
        .collect();
    if hist.is_empty() {
        return None;
    }
    let today = hist.last().copied()?;
    if (as_of - today.0).num_days() > 5 {
        return None;
    }
    let vals: Vec<f64> = hist.iter().map(|(_, v)| *v).collect();
    let scale = median_abs(&vals[vals.len().saturating_sub(20)..]);
    let n1 = today.1 / scale;
    let n5: f64 = vals.iter().rev().take(5).sum::<f64>() / scale;
    let score = (n1 * 0.55 + n5 * 0.45).clamp(-2.5, 2.5);
    let note = format!(
        "{unit_note} · 1日 {:+.1} / 5日累计 {:+.1}（相对近20日强度）",
        n1, n5
    );
    Some(CapitalFlowSignal {
        score,
        note,
        status: "ok",
    })
}

/// 两市成交代理：放量上涨偏谨慎、放量下跌偏钝化。
fn score_activity_proxy(archive: &CapitalFlowArchive, as_of: NaiveDate) -> Option<CapitalFlowSignal> {
    let window_start = as_of - Duration::days(40);
    let dates: Vec<NaiveDate> = archive
        .activity_amount
        .range(window_start..=as_of)
        .map(|(d, _)| *d)
        .collect();
    if dates.len() < 21 {
        return None;
    }
    let today = *dates.last()?;
    if (as_of - today).num_days() > 5 {
        return None;
    }
    let prev = dates[dates.len() - 2];
    let amt_today = *archive.activity_amount.get(&today)?;
    let close_today = *archive.activity_close.get(&today)?;
    let close_prev = *archive.activity_close.get(&prev)?;
    if close_prev <= 0.0 {
        return None;
    }
    let ret = (close_today - close_prev) / close_prev;
    let recent: Vec<f64> = dates
        .iter()
        .rev()
        .take(20)
        .filter_map(|d| archive.activity_amount.get(d).copied())
        .collect();
    let med = median_abs(&recent);
    let z = amt_today / med;
    let (score, tip) = if z > 1.15 && ret > 0.005 {
        (-0.9, "放量上涨偏谨慎")
    } else if z > 1.15 && ret < -0.005 {
        (0.6, "放量下跌或钝化")
    } else if z < 0.85 {
        (-ret * 2.0, "缩量")
    } else {
        (-ret * 4.0, "量能中性偏反向")
    };
    Some(CapitalFlowSignal {
        score: score.clamp(-2.5, 2.5),
        note: format!(
            "两市成交代理 · {tip} · 量比 {:.2} · 上证 {:+.2}%",
            z,
            ret * 100.0
        ),
        status: "ok",
    })
}

/// 按日评估：真主力 > 两市成交代理 > 北向净额。
pub fn evaluate_as_of(archive: &CapitalFlowArchive, as_of: NaiveDate) -> CapitalFlowSignal {
    if let Some(sig) = score_series(as_of, &archive.market_main, "大盘主力净流入") {
        return sig;
    }
    if let Some(sig) = score_activity_proxy(archive, as_of) {
        return sig;
    }
    if let Some(mut sig) = score_series(as_of, &archive.north_net_yi, "北向净买入") {
        sig.note = format!("{} · 主力/成交代理缺日，改用北向", sig.note);
        return sig;
    }
    CapitalFlowSignal {
        score: 0.0,
        note: "当日无资金流数据，未计入".into(),
        status: "skip",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_prefers_market_main() {
        let mut archive = CapitalFlowArchive::default();
        let d0 = NaiveDate::from_ymd_opt(2025, 6, 3).unwrap();
        for i in 0..25 {
            let d = d0 + Duration::days(i);
            archive
                .market_main
                .insert(d, if i < 20 { 1e9 } else { 8e9 });
        }
        let as_of = d0 + Duration::days(24);
        let sig = evaluate_as_of(&archive, as_of);
        assert_eq!(sig.status, "ok");
        assert!(sig.score > 0.0);
        assert!(sig.note.contains("主力"));
    }

    #[test]
    fn activity_proxy_scores_without_tushare() {
        let mut archive = CapitalFlowArchive::default();
        let d0 = NaiveDate::from_ymd_opt(2025, 1, 2).unwrap();
        let mut px = 3000.0;
        for i in 0..30 {
            let d = d0 + Duration::days(i);
            let amt = if i == 29 { 2.0e9 } else { 1.0e9 };
            if i == 29 {
                px *= 1.01;
            }
            archive.activity_amount.insert(d, amt);
            archive.activity_close.insert(d, px);
        }
        let as_of = d0 + Duration::days(29);
        let sig = evaluate_as_of(&archive, as_of);
        assert_eq!(sig.status, "ok");
        assert!(sig.note.contains("成交代理"));
        assert!(sig.score < 0.0, "volume up-day should fade");
    }

    #[test]
    fn missing_day_skips() {
        let archive = CapitalFlowArchive::default();
        let sig = evaluate_as_of(&archive, NaiveDate::from_ymd_opt(2026, 7, 17).unwrap());
        assert_eq!(sig.status, "skip");
    }
}
