//! 信号加权融合、宽基门控、score→概率。

use crate::algo::factor::{self, FactorStyle};
use crate::models::{SignalContribution, Stock};

#[derive(Debug, Clone)]
pub struct EnsembleSignal {
    pub up_probability: f64,
    pub down_probability: f64,
    pub confidence: f64,
    pub predicted: String,
    pub high_confidence: bool,
    pub summary_hint: String,
    pub contributions: Vec<SignalContribution>,
}

/// 多日模式：压低隔夜/短线噪声源权重，避免把趋势概率拉向 50%
pub fn reconcile_multiday_noise(raw: &mut Vec<(f64, SignalContribution)>) {
    for (w, c) in raw.iter_mut() {
        let scale = match c.id.as_str() {
            "message" => 0.25,
            "us_market" | "capital_flow" => 0.25,
            "mean_reversion" => 0.2,
            "news" | "policy" => 0.35,
            _ => 1.0,
        };
        if scale < 1.0 && *w > 0.0 {
            *w *= scale;
            if !c.note.contains("多日降权") {
                c.note = format!("{} · 多日降权×{:.2}", c.note, scale);
            }
        }
    }
}

/// 宽基：技术多因子与趋势动量方向冲突时，动量降为中性
pub fn reconcile_index_momentum(stock: &Stock, raw: &mut Vec<(f64, SignalContribution)>) {
    if factor::style_for_stock(stock) != FactorStyle::IndexEtf {
        return;
    }
    let factor_up = raw
        .iter()
        .find(|(_, c)| c.id == "factor" && c.status == "ok")
        .map(|(_, c)| c.up_probability);
    let Some(factor_up) = factor_up else {
        return;
    };
    let Some((mom_w, mom)) = raw
        .iter_mut()
        .find(|(_, c)| c.id == "momentum" && c.status == "ok")
    else {
        return;
    };
    if (mom.up_probability - 50.0).abs() < 1.0 {
        return;
    }
    let factor_bull = factor_up >= 50.0;
    let mom_bull = mom.up_probability >= 50.0;
    if factor_bull == mom_bull {
        return;
    }
    mom.up_probability = 50.0;
    mom.down_probability = 50.0;
    mom.confidence = 40.0;
    mom.note = format!("{} · 与多因子冲突已降权", mom.note);
    *mom_w = 0.0;
}

/// 宽基：消息面弱信号不计入
pub fn reconcile_index_factor_message(stock: &Stock, raw: &mut Vec<(f64, SignalContribution)>) {
    if factor::style_for_stock(stock) != FactorStyle::IndexEtf {
        return;
    }
    let Some((msg_w, msg)) = raw
        .iter_mut()
        .find(|(_, c)| c.id == "message" && (c.status == "ok" || c.status == "degraded"))
    else {
        return;
    };
    let lead = msg.up_probability.max(msg.down_probability);
    if lead < 55.0 {
        msg.up_probability = 50.0;
        msg.down_probability = 50.0;
        msg.confidence = 40.0;
        msg.note = format!("{} · 弱信号未计入（无有效关键词或强度不足）", msg.note);
        msg.status = "skip".into();
        *msg_w = 0.0;
    }
}

/// 宽基：资金流仅在「有方向且与多因子一致」时计入
pub fn reconcile_index_factor_capital(stock: &Stock, raw: &mut Vec<(f64, SignalContribution)>) {
    if factor::style_for_stock(stock) != FactorStyle::IndexEtf {
        return;
    }
    let factor_up = raw
        .iter()
        .find(|(_, c)| c.id == "factor" && c.status == "ok")
        .map(|(_, c)| c.up_probability);
    let Some(factor_up) = factor_up else {
        return;
    };
    let Some((cap_w, cap)) = raw
        .iter_mut()
        .find(|(_, c)| c.id == "capital_flow" && (c.status == "ok" || c.status == "degraded"))
    else {
        return;
    };
    let lead = cap.up_probability.max(cap.down_probability);
    if lead < 55.0 {
        cap.up_probability = 50.0;
        cap.down_probability = 50.0;
        cap.confidence = 40.0;
        cap.note = format!("{} · 弱信号未计入", cap.note);
        cap.status = "skip".into();
        *cap_w = 0.0;
        return;
    }
    let factor_bull = factor_up >= 50.0;
    let cap_bull = cap.up_probability >= 50.0;
    if factor_bull != cap_bull {
        cap.up_probability = 50.0;
        cap.down_probability = 50.0;
        cap.confidence = 40.0;
        cap.note = format!("{} · 与多因子冲突未计入", cap.note);
        cap.status = "skip".into();
        *cap_w = 0.0;
    }
}

/// 加权融合：权重为 0 的源仍展示明细，但不参与概率融合
pub fn fuse(raw: Vec<(f64, SignalContribution)>) -> EnsembleSignal {
    let total_w: f64 = raw
        .iter()
        .filter(|(w, _)| *w > 0.0)
        .map(|(w, _)| *w)
        .sum::<f64>()
        .max(1e-9);
    let mut contributions = Vec::new();
    let mut up_acc = 0.0;
    let mut down_acc = 0.0;
    let mut conf_acc = 0.0;
    let mut hints = Vec::new();

    for (w, mut c) in raw {
        let nw = if w > 0.0 { w / total_w } else { 0.0 };
        c.weight = w;
        c.weight_normalized = (nw * 100.0 * 10.0).round() / 10.0;
        if w > 0.0 {
            up_acc += c.up_probability * nw;
            down_acc += c.down_probability * nw;
            conf_acc += c.confidence * nw;
        }
        if (c.status == "ok" || c.status == "skip") && !c.note.is_empty() {
            hints.push(format!("{}: {}", c.name, c.note));
        }
        contributions.push(c);
    }

    let s = (up_acc + down_acc).max(1e-9);
    let up = (up_acc / s * 100.0).clamp(8.0, 92.0);
    let down = 100.0 - up;
    let predicted = if up >= down { "up".into() } else { "down".into() };
    let high_confidence = up.max(down) >= 60.0;

    EnsembleSignal {
        up_probability: (up * 10.0).round() / 10.0,
        down_probability: (down * 10.0).round() / 10.0,
        confidence: (conf_acc * 10.0).round() / 10.0,
        predicted,
        high_confidence,
        summary_hint: hints.into_iter().take(4).collect::<Vec<_>>().join("；"),
        contributions,
    }
}

pub fn probs_from_score(score: f64) -> (f64, f64, f64) {
    let strength = score.abs().clamp(0.0, 2.5) / 2.5;
    let confidence = (45.0 + strength * 40.0).clamp(40.0, 92.0);
    let up_share = (0.5 + (score / 2.5).clamp(-0.45, 0.45)).clamp(0.08, 0.92);
    let up = up_share * 100.0;
    (up, 100.0 - up, confidence)
}

/// 宽基消息面：|score|<0.15 → 中性；0.35→约60%高置信
pub fn probs_from_score_soft(score: f64) -> (f64, f64, f64) {
    let a = score.abs();
    if a < 0.15 {
        return (50.0, 50.0, 42.0);
    }
    let lead = if a < 0.35 {
        55.0 + (a - 0.15) / 0.20 * 5.0
    } else {
        60.0 + ((a - 0.35) / 1.2).clamp(0.0, 1.0) * 6.0
    };
    let up = if score > 0.0 { lead } else { 100.0 - lead };
    (up, 100.0 - up, lead)
}

pub fn contrib(
    id: &str,
    name: &str,
    category: &str,
    score: f64,
    note: String,
    status: &str,
) -> SignalContribution {
    let (up, down, confidence) = probs_from_score(score);
    SignalContribution {
        id: id.into(),
        name: name.into(),
        category: category.into(),
        up_probability: (up * 10.0).round() / 10.0,
        down_probability: (down * 10.0).round() / 10.0,
        confidence: (confidence * 10.0).round() / 10.0,
        weight: 0.0,
        weight_normalized: 0.0,
        note,
        status: status.into(),
    }
}

pub fn contrib_soft(
    id: &str,
    name: &str,
    category: &str,
    score: f64,
    note: String,
    status: &str,
) -> SignalContribution {
    let (up, down, confidence) = probs_from_score_soft(score);
    SignalContribution {
        id: id.into(),
        name: name.into(),
        category: category.into(),
        up_probability: (up * 10.0).round() / 10.0,
        down_probability: (down * 10.0).round() / 10.0,
        confidence: (confidence * 10.0).round() / 10.0,
        weight: 0.0,
        weight_normalized: 0.0,
        note,
        status: status.into(),
    }
}

pub fn neutral(id: &str, name: &str, note: &str) -> SignalContribution {
    SignalContribution {
        id: id.into(),
        name: name.into(),
        category: "—".into(),
        up_probability: 50.0,
        down_probability: 50.0,
        confidence: 40.0,
        weight: 0.0,
        weight_normalized: 0.0,
        note: note.into(),
        status: "skip".into(),
    }
}
