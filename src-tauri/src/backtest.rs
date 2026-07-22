use crate::algo::backtest::{classify_change, pct, round2, HitCounters};
use crate::cninfo::{self, MessageArchive};
use crate::factor_model;
use crate::models::{BacktestRecord, BacktestResult, DailyBar, Stock};
use crate::predictor;
use crate::strategy::{self, StrategyCompose};
use chrono::Duration;

pub use crate::algo::backtest::ACTIONABLE_LEAD;

pub async fn run_compose(
    stock: &Stock,
    bars: &[DailyBar],
    compose: &StrategyCompose,
    horizon_days: u32,
) -> BacktestResult {
    let compose = strategy::normalize_compose(compose);
    let lookback = factor_model::clamp_lookback(compose.lookback_days);
    let horizon = predictor::clamp_horizon(horizon_days) as usize;
    if bars.len() < lookback + horizon + 1 {
        return empty_result(stock, "compose", bars.len(), lookback, horizon);
    }

    let needs_message = strategy::compose_needs_message(&compose);
    let needs_capital = strategy::compose_needs_capital_flow(&compose);
    let selective = strategy::compose_is_message_primary(&compose);
    let (message_archive, message_err) = if needs_message {
        match load_message_archive(stock, bars, lookback).await {
            Ok(a) => (Some(a), None),
            Err(e) => (None, Some(e)),
        }
    } else {
        (None, None)
    };
    let message_ref = message_archive.as_ref();
    let uses_message = message_ref.map(|a| !a.is_empty()).unwrap_or(false);

    let (capital_archive, capital_err) = if needs_capital {
        match crate::capital_flow::fetch_archive_cached().await {
            Ok(a) => (Some(a), None),
            Err(e) => (None, Some(e)),
        }
    } else {
        (None, None)
    };
    let capital_ref = capital_archive.as_ref();
    let uses_capital = capital_ref.map(|a| a.usable_days() > 0).unwrap_or(false);

    let mut records = Vec::new();
    let mut hits = HitCounters::default();

    for i in lookback..(bars.len() - horizon) {
        let window = &bars[i + 1 - lookback..=i];
        let current_price = bars[i].close;
        if current_price <= 0.0 {
            continue;
        }

        let as_of = cninfo::parse_flexible_date(&bars[i].date);
        let signal = predictor::predict_direction_compose(
            stock,
            window,
            &compose,
            if needs_message { message_ref } else { None },
            if needs_capital { capital_ref } else { None },
            as_of,
            horizon as u32,
        );
        let future_close = bars[i + horizon].close;
        let change_pct = (future_close - current_price) / current_price * 100.0;
        let actual = classify_change(change_pct);
        let is_correct = signal.predicted == actual;
        hits.observe(
            &signal.predicted,
            actual,
            signal.up_probability,
            signal.down_probability,
            signal.high_confidence,
        );

        records.push(BacktestRecord {
            date: bars[i].date.clone(),
            predict_date: bars[i + horizon].date.clone(),
            close_price: round2(current_price),
            next_close: round2(future_close),
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

    let all_day_accuracy = pct(hits.correct_all, hits.total_all);
    let actionable_accuracy = pct(hits.correct_act, hits.total_act);
    let high_confidence_accuracy = pct(hits.hc_correct, hits.hc_total);
    let threshold = predictor::HIGH_CONF_THRESHOLD;
    let up_hit_rate = pct(hits.up_hits, hits.up_total);
    let down_hit_rate = pct(hits.down_hits, hits.down_total);
    let up_hit_rate_actionable = pct(hits.up_hits_act, hits.up_total_act);
    let down_hit_rate_actionable = pct(hits.down_hits_act, hits.down_total_act);

    // 主指标始终按「全部预测日」统计；有效口径另列
    let direction_accuracy = all_day_accuracy;
    let total_samples = hits.total_all;

    let mut extra_notes = String::new();
    if needs_message {
        if uses_message {
            extra_notes.push_str(&format!(
                "；消息面已纳入（公告 {} 条）",
                message_archive.as_ref().map(|a| a.len()).unwrap_or(0)
            ));
        } else if let Some(err) = message_err.as_ref() {
            extra_notes.push_str(&format!("；消息面公告拉取失败: {err}"));
        } else {
            extra_notes.push_str("；消息面启用但区间内无公告，按中性计入");
        }
    }
    if needs_capital {
        if uses_capital {
            let a = capital_archive.as_ref().unwrap();
            extra_notes.push_str(&format!(
                "；资金流已纳入（主力 {} / 成交代理 {} / 北向 {}；{}）",
                a.market_days(),
                a.activity_days(),
                a.north_days(),
                if a.source_note.is_empty() {
                    "—"
                } else {
                    &a.source_note
                }
            ));
        } else if let Some(err) = capital_err.as_ref() {
            extra_notes.push_str(&format!("；资金流拉取失败: {err}"));
        } else {
            extra_notes.push_str("；资金流启用但无可用历史（请配置 Tushare Token）");
        }
    }

    let horizon_note = if horizon <= 1 {
        "次日".to_string()
    } else {
        format!("{horizon} 日累计")
    };

    let summary = if selective {
        format!(
            "近 {} 个样本回测（回看 {} 日 · 预测{}{}）：全样本准确率 {:.1}%（{} / {}）；有效信号 {} 次 / {:.1}%；高置信 {} 次 / {:.1}%。看涨全量 {:.1}% / 有效 {:.1}%；看跌全量 {:.1}% / 有效 {:.1}%。",
            hits.total_all,
            lookback,
            horizon_note,
            extra_notes,
            all_day_accuracy,
            hits.correct_all,
            hits.total_all,
            hits.total_act,
            actionable_accuracy,
            hits.hc_total,
            high_confidence_accuracy,
            up_hit_rate,
            up_hit_rate_actionable,
            down_hit_rate,
            down_hit_rate_actionable,
        )
    } else {
        format!(
            "近 {} 个样本组合回测（回看 {} 日 · 预测{}{}）：整体 {:.1}%；高置信 {} 次 / {:.1}%。看涨 {:.1}%（有效 {:.1}%）；看跌 {:.1}%（有效 {:.1}%）。",
            hits.total_all,
            lookback,
            horizon_note,
            extra_notes,
            direction_accuracy,
            hits.hc_total,
            high_confidence_accuracy,
            up_hit_rate,
            up_hit_rate_actionable,
            down_hit_rate,
            down_hit_rate_actionable,
        )
    };

    BacktestResult {
        stock: stock.clone(),
        algorithm: "compose".into(),
        total_samples,
        direction_accuracy,
        actionable_accuracy,
        all_day_accuracy,
        actionable_samples: hits.total_act,
        selective_mode: selective,
        up_hit_rate,
        down_hit_rate,
        up_hit_rate_actionable,
        down_hit_rate_actionable,
        up_samples: hits.up_total,
        down_samples: hits.down_total,
        up_samples_actionable: hits.up_total_act,
        down_samples_actionable: hits.down_total_act,
        high_confidence_samples: hits.hc_total,
        high_confidence_accuracy,
        high_confidence_threshold: threshold,
        flat_threshold_pct: 0.0,
        lookback_days: lookback as u32,
        horizon_days: horizon as u32,
        summary,
        records,
    }
}

async fn load_message_archive(
    stock: &Stock,
    bars: &[DailyBar],
    lookback: usize,
) -> Result<MessageArchive, String> {
    let _ = lookback;
    let first = bars.first().ok_or_else(|| "K 线为空".to_string())?;
    let last = bars.last().ok_or_else(|| "K 线为空".to_string())?;
    let start = cninfo::parse_flexible_date(&first.date)
        .ok_or_else(|| format!("无法解析起始日期: {}", first.date))?
        - Duration::days(strategy::MESSAGE_LOOKBACK_DAYS + 3);
    let end = cninfo::parse_flexible_date(&last.date)
        .ok_or_else(|| format!("无法解析结束日期: {}", last.date))?;
    cninfo::fetch_archive(stock, start, end).await
}

/// 兼容旧接口
pub async fn run(stock: &Stock, algorithm: &str, bars: &[DailyBar], lookback_days: u32) -> BacktestResult {
    let mut compose = strategy::default_compose();
    compose.lookback_days = lookback_days;
    for s in &mut compose.sources {
        s.enabled = s.id == "factor" || s.id == "momentum" || s.id == "volume";
        if algorithm == "placeholder_v1" {
            s.enabled = s.id == "factor";
        }
    }
    run_compose(stock, bars, &compose, 1).await
}

fn empty_result(
    stock: &Stock,
    algorithm: &str,
    bar_count: usize,
    lookback: usize,
    horizon: usize,
) -> BacktestResult {
    BacktestResult {
        stock: stock.clone(),
        algorithm: algorithm.into(),
        total_samples: 0,
        direction_accuracy: 0.0,
        actionable_accuracy: 0.0,
        all_day_accuracy: 0.0,
        actionable_samples: 0,
        selective_mode: false,
        up_hit_rate: 0.0,
        down_hit_rate: 0.0,
        up_hit_rate_actionable: 0.0,
        down_hit_rate_actionable: 0.0,
        up_samples: 0,
        down_samples: 0,
        up_samples_actionable: 0,
        down_samples_actionable: 0,
        high_confidence_samples: 0,
        high_confidence_accuracy: 0.0,
        high_confidence_threshold: predictor::HIGH_CONF_THRESHOLD,
        flat_threshold_pct: 0.0,
        lookback_days: lookback as u32,
        horizon_days: horizon as u32,
        summary: format!(
            "历史数据不足（仅 {} 根 K 线，至少需要 {} 根）",
            bar_count,
            lookback + horizon + 1
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

    #[tokio::test]
    async fn backtest_compose_works() {
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
        let result = run(&stock, "factor_v1", &bars, 25).await;
        assert!(result.total_samples > 0);
    }
}
