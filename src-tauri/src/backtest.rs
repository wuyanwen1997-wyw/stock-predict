use crate::cninfo::{self, MessageArchive};
use crate::factor_model;
use crate::models::{BacktestRecord, BacktestResult, DailyBar, Stock};
use crate::predictor;
use crate::strategy::{self, StrategyCompose};
use chrono::Duration;

/// 有效信号出手线：领先一侧概率 ≥ 该值才计入「整体/有效准确率」
const ACTIONABLE_LEAD: f64 = 55.0;

pub async fn run_compose(
    stock: &Stock,
    bars: &[DailyBar],
    compose: &StrategyCompose,
) -> BacktestResult {
    let compose = strategy::normalize_compose(compose);
    let lookback = factor_model::clamp_lookback(compose.lookback_days);
    if bars.len() < lookback + 2 {
        return empty_result(stock, "compose", bars.len(), lookback);
    }

    let needs_message = strategy::compose_needs_message(&compose);
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

    let mut records = Vec::new();
    let mut correct_all = 0u32;
    let mut total_all = 0u32;
    let mut correct_act = 0u32;
    let mut total_act = 0u32;
    let mut up_hits = 0u32;
    let mut up_total = 0u32;
    let mut down_hits = 0u32;
    let mut down_total = 0u32;
    let mut up_hits_act = 0u32;
    let mut up_total_act = 0u32;
    let mut down_hits_act = 0u32;
    let mut down_total_act = 0u32;
    let mut hc_correct = 0u32;
    let mut hc_total = 0u32;

    for i in lookback..(bars.len() - 1) {
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
            as_of,
        );
        let next_close = bars[i + 1].close;
        let change_pct = (next_close - current_price) / current_price * 100.0;
        let actual = classify_change(change_pct);
        let is_correct = signal.predicted == actual;
        let lead = signal.up_probability.max(signal.down_probability);
        let actionable = lead + 1e-9 >= ACTIONABLE_LEAD;

        total_all += 1;
        if is_correct {
            correct_all += 1;
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

        if actionable {
            total_act += 1;
            if is_correct {
                correct_act += 1;
            }
            if signal.predicted == "up" {
                up_total_act += 1;
                if actual == "up" {
                    up_hits_act += 1;
                }
            } else {
                down_total_act += 1;
                if actual == "down" {
                    down_hits_act += 1;
                }
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

    let all_day_accuracy = pct(correct_all, total_all);
    let actionable_accuracy = pct(correct_act, total_act);
    let high_confidence_accuracy = pct(hc_correct, hc_total);
    let threshold = predictor::HIGH_CONF_THRESHOLD;
    let up_hit_rate = pct(up_hits, up_total);
    let down_hit_rate = pct(down_hits, down_total);
    let up_hit_rate_actionable = pct(up_hits_act, up_total_act);
    let down_hit_rate_actionable = pct(down_hits_act, down_total_act);

    // 主指标始终按「全部预测日」统计；有效口径另列
    let direction_accuracy = all_day_accuracy;
    let total_samples = total_all;

    let msg_note = if needs_message {
        if uses_message {
            format!(
                "；消息面已纳入（公告 {} 条）",
                message_archive.as_ref().map(|a| a.len()).unwrap_or(0)
            )
        } else if let Some(err) = message_err.as_ref() {
            format!("；消息面公告拉取失败: {err}")
        } else {
            "；消息面启用但区间内无公告，按中性计入".into()
        }
    } else {
        String::new()
    };

    let summary = if selective {
        format!(
            "近 {} 个交易日回测（回看 {} 日{}）：全样本准确率 {:.1}%（{} / {}）；有效信号 {} 次 / {:.1}%；高置信 {} 次 / {:.1}%。看涨全量 {:.1}% / 有效 {:.1}%；看跌全量 {:.1}% / 有效 {:.1}%。",
            total_all,
            lookback,
            msg_note,
            all_day_accuracy,
            correct_all,
            total_all,
            total_act,
            actionable_accuracy,
            hc_total,
            high_confidence_accuracy,
            up_hit_rate,
            up_hit_rate_actionable,
            down_hit_rate,
            down_hit_rate_actionable,
        )
    } else {
        format!(
            "近 {} 个交易日组合回测（回看 {} 日{}）：整体 {:.1}%；高置信 {} 次 / {:.1}%。看涨 {:.1}%（有效 {:.1}%）；看跌 {:.1}%（有效 {:.1}%）。",
            total_all,
            lookback,
            msg_note,
            direction_accuracy,
            hc_total,
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
        actionable_samples: total_act,
        selective_mode: selective,
        up_hit_rate,
        down_hit_rate,
        up_hit_rate_actionable,
        down_hit_rate_actionable,
        up_samples: up_total,
        down_samples: down_total,
        up_samples_actionable: up_total_act,
        down_samples_actionable: down_total_act,
        high_confidence_samples: hc_total,
        high_confidence_accuracy,
        high_confidence_threshold: threshold,
        flat_threshold_pct: 0.0,
        lookback_days: lookback as u32,
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
    run_compose(stock, bars, &compose).await
}

fn classify_change(change_pct: f64) -> &'static str {
    if change_pct > 0.0 {
        "up"
    } else {
        "down"
    }
}

fn pct(hits: u32, total: u32) -> f64 {
    if total == 0 {
        0.0
    } else {
        (hits as f64 / total as f64 * 1000.0).round() / 10.0
    }
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn empty_result(stock: &Stock, algorithm: &str, bar_count: usize, lookback: usize) -> BacktestResult {
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
