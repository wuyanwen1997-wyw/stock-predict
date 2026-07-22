use crate::capital_flow::CapitalFlowArchive;
use crate::cninfo::MessageArchive;
use crate::factor_model;
use crate::models::{DailyBar, SignalContribution, Stock};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// 信号源目录条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategySourceInfo {
    pub id: String,
    pub name: String,
    pub category: String,
    pub description: String,
    /// 是否可参与历史回测（仅依赖 K 线的可回测）
    pub backtestable: bool,
    pub available: bool,
}

/// 用户为某只股票配置的单个信号源
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategySourceConfig {
    pub id: String,
    pub enabled: bool,
    /// 相对权重，启用项会按权重归一化
    pub weight: f64,
}

/// 组合配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyCompose {
    pub sources: Vec<StrategySourceConfig>,
    #[serde(default = "default_lookback")]
    pub lookback_days: u32,
}

fn default_lookback() -> u32 {
    50
}

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

pub fn catalog() -> Vec<StrategySourceInfo> {
    vec![
        StrategySourceInfo {
            id: "factor".into(),
            name: "技术多因子".into(),
            category: "技术面".into(),
            description: "MA/RSI/动量/量能；宽基ETF自动切换为MA20+隔日反向模型。".into(),
            backtestable: true,
            available: true,
        },
        StrategySourceInfo {
            id: "momentum".into(),
            name: "趋势动量".into(),
            category: "技术面".into(),
            description: "短中期动量；宽基ETF改为3日动量互补（不与多因子重复隔日反向）。".into(),
            backtestable: true,
            available: true,
        },
        StrategySourceInfo {
            id: "mean_reversion".into(),
            name: "均值回归".into(),
            category: "技术面".into(),
            description: "偏离 MA20 / RSI 极端时的反转倾向。".into(),
            backtestable: true,
            available: true,
        },
        StrategySourceInfo {
            id: "volume".into(),
            name: "量价确认".into(),
            category: "技术面".into(),
            description: "放量上涨 / 放量下跌 / 缩量整理判断。".into(),
            backtestable: true,
            available: true,
        },
        StrategySourceInfo {
            id: "message".into(),
            name: "消息面".into(),
            category: "舆情".into(),
            description: "按股票类型选用关键词与打分（黄金看美联储/地缘，金融看降准息差，科技看制裁替代等）。"
                .into(),
            backtestable: true,
            available: true,
        },
        StrategySourceInfo {
            id: "news".into(),
            name: "资讯新闻".into(),
            category: "舆情".into(),
            description: "财经资讯标题情绪打分。".into(),
            backtestable: false,
            available: true,
        },
        StrategySourceInfo {
            id: "policy".into(),
            name: "政策面".into(),
            category: "宏观".into(),
            description: "政策/监管/财政等相关新闻关键词情绪。".into(),
            backtestable: false,
            available: true,
        },
        StrategySourceInfo {
            id: "us_market".into(),
            name: "美股联动".into(),
            category: "宏观".into(),
            description: "参考纳斯达克/标普隔夜涨跌作为风险偏好。".into(),
            backtestable: false,
            available: true,
        },
        StrategySourceInfo {
            id: "capital_flow".into(),
            name: "资金流(主力)".into(),
            category: "资金面".into(),
            description: "优先大盘主力净流入；无 Token 时用两市成交额代理（免费可回测）；可回退北向净额。"
                .into(),
            backtestable: true,
            available: true,
        },
    ]
}

pub fn default_compose() -> StrategyCompose {
    StrategyCompose {
        lookback_days: 50,
        sources: vec![
            StrategySourceConfig {
                id: "factor".into(),
                enabled: true,
                weight: 35.0,
            },
            StrategySourceConfig {
                id: "momentum".into(),
                enabled: true,
                weight: 20.0,
            },
            StrategySourceConfig {
                id: "mean_reversion".into(),
                enabled: false,
                weight: 15.0,
            },
            StrategySourceConfig {
                id: "volume".into(),
                enabled: true,
                weight: 15.0,
            },
            StrategySourceConfig {
                id: "message".into(),
                enabled: false,
                weight: 15.0,
            },
            StrategySourceConfig {
                id: "news".into(),
                enabled: false,
                weight: 15.0,
            },
            StrategySourceConfig {
                id: "policy".into(),
                enabled: false,
                weight: 10.0,
            },
            StrategySourceConfig {
                id: "us_market".into(),
                enabled: false,
                weight: 10.0,
            },
            StrategySourceConfig {
                id: "capital_flow".into(),
                enabled: false,
                weight: 15.0,
            },
        ],
    }
}

/// 按标的推荐组合。宽基 ETF：多因子 70% + 消息面 30%
/// （510980 离线 OOS 整体准确率 ≈ 61.7%；以全样本方向命中为准，不以高置信口径择优）
pub fn default_compose_for_stock(stock: &Stock) -> StrategyCompose {
    if factor_model::style_for_stock(stock) != factor_model::FactorStyle::IndexEtf {
        return default_compose();
    }
    StrategyCompose {
        lookback_days: 120,
        sources: vec![
            StrategySourceConfig {
                id: "factor".into(),
                enabled: true,
                weight: 70.0,
            },
            StrategySourceConfig {
                id: "momentum".into(),
                enabled: false,
                weight: 15.0,
            },
            StrategySourceConfig {
                id: "mean_reversion".into(),
                enabled: false,
                weight: 15.0,
            },
            StrategySourceConfig {
                id: "volume".into(),
                enabled: false,
                weight: 10.0,
            },
            StrategySourceConfig {
                id: "message".into(),
                enabled: true,
                weight: 30.0,
            },
            StrategySourceConfig {
                id: "news".into(),
                enabled: false,
                weight: 10.0,
            },
            StrategySourceConfig {
                id: "policy".into(),
                enabled: false,
                weight: 10.0,
            },
            StrategySourceConfig {
                id: "us_market".into(),
                enabled: false,
                weight: 10.0,
            },
            StrategySourceConfig {
                id: "capital_flow".into(),
                enabled: false,
                weight: 15.0,
            },
        ],
    }
}

pub fn normalize_compose(compose: &StrategyCompose) -> StrategyCompose {
    let mut sources = compose.sources.clone();
    let catalog = catalog();
    let known: std::collections::HashSet<_> = catalog.iter().map(|s| s.id.clone()).collect();

    // 补全新源
    for info in &catalog {
        if !sources.iter().any(|s| s.id == info.id) {
            let def = default_compose()
                .sources
                .into_iter()
                .find(|s| s.id == info.id)
                .unwrap_or(StrategySourceConfig {
                    id: info.id.clone(),
                    enabled: false,
                    weight: 10.0,
                });
            sources.push(def);
        }
    }
    sources.retain(|s| known.contains(&s.id));
    for s in &mut sources {
        if s.weight < 0.0 {
            s.weight = 0.0;
        }
        if s.weight > 100.0 {
            s.weight = 100.0;
        }
    }

    StrategyCompose {
        lookback_days: factor_model::clamp_lookback(compose.lookback_days) as u32,
        sources,
    }
}

/// 现场评估组合（含非回测源）
/// `horizon_days`：1=次日原逻辑；2–5=多日趋势专用信号（不影响单日）
pub async fn evaluate_live(
    stock: &Stock,
    bars: &[DailyBar],
    compose: &StrategyCompose,
    horizon_days: u32,
) -> EnsembleSignal {
    let compose = normalize_compose(compose);
    let lookback = factor_model::clamp_lookback(compose.lookback_days);
    let window = factor_model::take_lookback(bars, lookback);
    let horizon = horizon_days.clamp(1, 5);
    let capital_archive = if compose_needs_capital_flow(&compose) {
        crate::capital_flow::fetch_archive_cached().await.ok()
    } else {
        None
    };
    let as_of = chrono::Local::now().date_naive();

    let mut raw = Vec::new();
    for cfg in compose.sources.iter().filter(|s| s.enabled && s.weight > 0.0) {
        let (w, contrib) = match cfg.id.as_str() {
            "factor" => (cfg.weight, eval_factor(stock, window, horizon)),
            "momentum" => (cfg.weight, eval_momentum(stock, window, horizon)),
            "mean_reversion" => (cfg.weight, eval_mean_reversion(window, horizon)),
            "volume" => (cfg.weight, eval_volume(stock, window, horizon)),
            "message" => (cfg.weight, eval_message(stock).await),
            "news" => (cfg.weight, eval_news().await),
            "policy" => (cfg.weight, eval_policy().await),
            "us_market" => (cfg.weight, eval_us_market().await),
            "capital_flow" => {
                let c = match capital_archive.as_ref() {
                    Some(archive) => eval_capital_flow(archive, as_of),
                    None => contrib(
                        "capital_flow",
                        "资金流(主力)",
                        "资金面",
                        0.0,
                        "资金流数据暂不可用".into(),
                        "degraded",
                    ),
                };
                let w = if c.status == "skip" { 0.0 } else { cfg.weight };
                (w, c)
            }
            _ => (cfg.weight, neutral(&cfg.id, "未知信号源", "跳过")),
        };
        raw.push((w, contrib));
    }

    if raw.is_empty() {
        // 兜底：至少跑技术多因子
        raw.push((1.0, eval_factor(stock, window, horizon)));
    }

    if horizon <= 1 {
        reconcile_index_momentum(stock, &mut raw);
        reconcile_index_factor_message(stock, &mut raw);
        reconcile_index_factor_capital(stock, &mut raw);
    } else {
        // 多日：先压短线噪声，再沿用宽基消息/资金门控（否则 30% 消息会把命中率拉向 50%）
        reconcile_multiday_noise(&mut raw);
        reconcile_index_factor_message(stock, &mut raw);
        reconcile_index_factor_capital(stock, &mut raw);
    }
    fuse(raw)
}

/// 历史回测用：仅可回测信号源（消息面/资金流需传入归档）
pub fn evaluate_historical(
    stock: &Stock,
    bars: &[DailyBar],
    compose: &StrategyCompose,
    message: Option<&MessageArchive>,
    capital_flow: Option<&CapitalFlowArchive>,
    as_of: Option<NaiveDate>,
    horizon_days: u32,
) -> EnsembleSignal {
    let compose = normalize_compose(compose);
    let lookback = factor_model::clamp_lookback(compose.lookback_days);
    let window = factor_model::take_lookback(bars, lookback);
    let horizon = horizon_days.clamp(1, 5);
    let catalog = catalog();
    let as_of_date = as_of.or_else(|| {
        window
            .last()
            .and_then(|b| crate::cninfo::parse_flexible_date(&b.date))
    });

    let mut raw = Vec::new();
    for cfg in compose.sources.iter().filter(|s| s.enabled && s.weight > 0.0) {
        let info = catalog.iter().find(|c| c.id == cfg.id);
        if info.map(|i| i.backtestable) != Some(true) {
            continue;
        }
        let (w, contrib) = match cfg.id.as_str() {
            "factor" => (cfg.weight, eval_factor(stock, window, horizon)),
            "momentum" => (cfg.weight, eval_momentum(stock, window, horizon)),
            "mean_reversion" => (cfg.weight, eval_mean_reversion(window, horizon)),
            "volume" => (cfg.weight, eval_volume(stock, window, horizon)),
            "message" => {
                let Some(day) = as_of_date else {
                    continue;
                };
                let c = match message {
                    Some(archive) => eval_message_from_archive(stock, archive, day),
                    None => contrib(
                        "message",
                        "消息面",
                        "舆情",
                        0.0,
                        "公告归档拉取失败，按中性计入".into(),
                        "degraded",
                    ),
                };
                (cfg.weight, c)
            }
            "capital_flow" => {
                let Some(day) = as_of_date else {
                    continue;
                };
                let c = match capital_flow {
                    Some(archive) => eval_capital_flow(archive, day),
                    None => contrib(
                        "capital_flow",
                        "资金流(主力)",
                        "资金面",
                        0.0,
                        "资金流归档拉取失败，按中性跳过".into(),
                        "degraded",
                    ),
                };
                let w = if c.status == "skip" { 0.0 } else { cfg.weight };
                (w, c)
            }
            _ => continue,
        };
        raw.push((w, contrib));
    }

    if raw.is_empty() {
        raw.push((1.0, eval_factor(stock, window, horizon)));
    }

    if horizon <= 1 {
        reconcile_index_momentum(stock, &mut raw);
        reconcile_index_factor_message(stock, &mut raw);
        reconcile_index_factor_capital(stock, &mut raw);
    } else {
        reconcile_multiday_noise(&mut raw);
        reconcile_index_factor_message(stock, &mut raw);
        reconcile_index_factor_capital(stock, &mut raw);
    }
    fuse(raw)
}

pub fn compose_needs_message(compose: &StrategyCompose) -> bool {
    let compose = normalize_compose(compose);
    compose
        .sources
        .iter()
        .any(|s| s.id == "message" && s.enabled && s.weight > 0.0)
}

pub fn compose_needs_capital_flow(compose: &StrategyCompose) -> bool {
    let compose = normalize_compose(compose);
    compose
        .sources
        .iter()
        .any(|s| s.id == "capital_flow" && s.enabled && s.weight > 0.0)
}

/// 是否几乎只开了消息面（宽基指数适合「有效信号」口径回测）
pub fn compose_is_message_primary(compose: &StrategyCompose) -> bool {
    let compose = normalize_compose(compose);
    let active: Vec<_> = compose
        .sources
        .iter()
        .filter(|s| s.enabled && s.weight > 0.0)
        .collect();
    if active.is_empty() {
        return false;
    }
    let msg_w: f64 = active
        .iter()
        .filter(|s| s.id == "message")
        .map(|s| s.weight)
        .sum();
    let all_w: f64 = active.iter().map(|s| s.weight).sum();
    msg_w / all_w.max(1e-9) >= 0.85
}

/// 多日模式：压低隔夜/短线噪声源权重，避免把趋势概率拉向 50%
fn reconcile_multiday_noise(raw: &mut Vec<(f64, SignalContribution)>) {
    for (w, c) in raw.iter_mut() {
        let scale = match c.id.as_str() {
            // 消息面按次日关键词训练，多日先大幅降权；宽基再经 reconcile_index_factor_message 门控
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

/// 宽基：技术多因子与趋势动量方向冲突时，动量降为中性，避免稀释隔日反向优势
fn reconcile_index_momentum(stock: &Stock, raw: &mut Vec<(f64, SignalContribution)>) {
    if factor_model::style_for_stock(stock) != factor_model::FactorStyle::IndexEtf {
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

/// 宽基：消息面弱信号不计入（避免 50% 稀释主概率）；与多因子冲突仍计入。
/// 网格搜索（510980）显示：冲突门控会压低全样本整体准确率；仅弱信号门控 OOS ≈ 61.7%。
fn reconcile_index_factor_message(stock: &Stock, raw: &mut Vec<(f64, SignalContribution)>) {
    if factor_model::style_for_stock(stock) != factor_model::FactorStyle::IndexEtf {
        return;
    }
    let Some((msg_w, msg)) = raw
        .iter_mut()
        .find(|(_, c)| c.id == "message" && (c.status == "ok" || c.status == "degraded"))
    else {
        return;
    };
    let lead = msg.up_probability.max(msg.down_probability);
    // 中性/过弱：不计入融合（否则 50% 占权重会悄悄把主概率往 50 拉）
    if lead < 55.0 {
        msg.up_probability = 50.0;
        msg.down_probability = 50.0;
        msg.confidence = 40.0;
        msg.note = format!("{} · 弱信号未计入（无有效关键词或强度不足）", msg.note);
        msg.status = "skip".into();
        *msg_w = 0.0;
    }
}

/// 宽基：资金流仅在「有方向且与多因子一致」时计入；弱信号/冲突则权重清零，避免把主概率往 50% 拉
fn reconcile_index_factor_capital(stock: &Stock, raw: &mut Vec<(f64, SignalContribution)>) {
    if factor_model::style_for_stock(stock) != factor_model::FactorStyle::IndexEtf {
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

fn fuse(raw: Vec<(f64, SignalContribution)>) -> EnsembleSignal {
    // 权重为 0 的源仍展示明细，但不参与概率融合
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

    // 归一化涨跌概率
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

fn probs_from_score(score: f64) -> (f64, f64, f64) {
    let strength = score.abs().clamp(0.0, 2.5) / 2.5;
    let confidence = (45.0 + strength * 40.0).clamp(40.0, 92.0);
    let up_share = (0.5 + (score / 2.5).clamp(-0.45, 0.45)).clamp(0.08, 0.92);
    let up = up_share * 100.0;
    (up, 100.0 - up, confidence)
}

/// 宽基消息面：|score|<0.15 → 中性(不计入有效样本)；0.35→约60%高置信
fn probs_from_score_soft(score: f64) -> (f64, f64, f64) {
    let a = score.abs();
    if a < 0.15 {
        return (50.0, 50.0, 42.0);
    }
    // 0.15→55%，0.35→60%，更强→最高 66%
    let lead = if a < 0.35 {
        55.0 + (a - 0.15) / 0.20 * 5.0
    } else {
        60.0 + ((a - 0.35) / 1.2).clamp(0.0, 1.0) * 6.0
    };
    let up = if score > 0.0 { lead } else { 100.0 - lead };
    (up, 100.0 - up, lead)
}

fn contrib(
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

fn contrib_soft(
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

fn neutral(id: &str, name: &str, note: &str) -> SignalContribution {
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

fn eval_factor(stock: &Stock, bars: &[DailyBar], horizon_days: u32) -> SignalContribution {
    let style = factor_model::style_for_stock(stock);
    match factor_model::compute_styled_for_horizon(bars, style, horizon_days) {
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

fn eval_capital_flow(archive: &CapitalFlowArchive, as_of: NaiveDate) -> SignalContribution {
    let sig = crate::capital_flow::evaluate_as_of(archive, as_of);
    if sig.status == "skip" {
        return SignalContribution {
            id: "capital_flow".into(),
            name: "资金流(主力)".into(),
            category: "资金面".into(),
            up_probability: 50.0,
            down_probability: 50.0,
            confidence: 40.0,
            weight: 0.0,
            weight_normalized: 0.0,
            note: sig.note,
            status: "skip".into(),
        };
    }
    contrib(
        "capital_flow",
        "资金流(主力)",
        "资金面",
        sig.score,
        sig.note,
        sig.status,
    )
}

fn eval_momentum(stock: &Stock, bars: &[DailyBar], horizon_days: u32) -> SignalContribution {
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
        // 单日：原逻辑不变
        if factor_model::style_for_stock(stock) == factor_model::FactorStyle::IndexEtf {
            let score = if m3.abs() < 0.008 {
                0.0
            } else {
                (m3 * 12.0).clamp(-1.0, 1.0)
            };
            (
                score,
                format!(
                    "宽基互补·3日动量 {:+.1}%（不重复隔日反向）",
                    m3 * 100.0
                ),
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
        // 多日：与预测跨度对齐的动量为主
        let score = (mh * 14.0 + m10 * 5.0).clamp(-2.5, 2.5);
        (
            score,
            format!(
                "{h}日动量 {:+.1}% / 10日 {:+.1}%",
                mh * 100.0,
                m10 * 100.0
            ),
        )
    };
    contrib("momentum", "趋势动量", "技术面", score, note, "ok")
}

fn eval_mean_reversion(bars: &[DailyBar], horizon_days: u32) -> SignalContribution {
    if bars.len() < 25 {
        return neutral("mean_reversion", "均值回归", "K线不足");
    }
    let factors = match factor_model::compute(bars) {
        Some(f) => f,
        None => return neutral("mean_reversion", "均值回归", "指标不足"),
    };
    let price = bars.last().map(|b| b.close).unwrap_or(0.0);
    let dev = if factors.ma20 > 0.0 {
        (price - factors.ma20) / factors.ma20
    } else {
        0.0
    };
    // 偏离越大 → 回归方向越强（与趋势相反）
    let mut score = (-dev * 15.0).clamp(-2.5, 2.5);
    if factors.rsi14 > 70.0 {
        score -= 0.6;
    } else if factors.rsi14 < 30.0 {
        score += 0.6;
    }
    // 多日累计更偏趋势延续，均值回归只保留弱信号
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

fn eval_volume(stock: &Stock, bars: &[DailyBar], horizon_days: u32) -> SignalContribution {
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

    let index = factor_model::style_for_stock(stock) == factor_model::FactorStyle::IndexEtf;
    let (score, note) = if horizon_days > 1 {
        // 多日：放量跟随趋势确认
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
        // 宽基单日：放量后更偏谨慎（量能与次日收益偏负相关）
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

fn sentiment_from_titles(titles: &[String], bullish: &[&str], bearish: &[&str]) -> (f64, String) {
    if titles.is_empty() {
        return (0.0, "暂无标题".into());
    }
    let mut score: f64 = 0.0;
    let mut hits = 0;
    for t in titles {
        let lower = t.to_lowercase();
        let mut bull = 0i32;
        let mut bear = 0i32;
        for w in bullish {
            if t.contains(w) || lower.contains(&w.to_lowercase()) {
                bull += 1;
            }
        }
        for w in bearish {
            if t.contains(w) || lower.contains(&w.to_lowercase()) {
                bear += 1;
            }
        }
        if bull == 0 && bear == 0 {
            continue;
        }
        // 单条标题：按命中差计分，并封顶，避免关键词扩容后重复累加过猛
        let net = (bull - bear).clamp(-3, 3) as f64;
        score += net * 0.4;
        hits += (bull + bear) as usize;
    }
    let note = if hits == 0 {
        format!("扫描 {} 条，中性", titles.len())
    } else {
        format!("扫描 {} 条，命中 {} 次关键词", titles.len(), hits)
    };
    (score.clamp(-2.5, 2.5), note)
}

async fn fetch_eastmoney_news_titles(query: &str, limit: usize) -> Result<Vec<String>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .user_agent("Mozilla/5.0 StockPredict/0.1")
        .build()
        .map_err(|e| e.to_string())?;

    // 东方财富资讯搜索（公开接口，可能偶尔失败）
    let url = "https://search-api-web.eastmoney.com/search/jsonp";
    let cb = "jQuery";
    let param = serde_json::json!({
        "uid": "",
        "keyword": query,
        "type": ["cmsArticleWebOld"],
        "client": "web",
        "clientType": "web",
        "clientVersion": "curr",
        "param": {
            "cmsArticleWebOld": {
                "searchScope": "default",
                "sort": "default",
                "pageIndex": 1,
                "pageSize": limit,
            }
        }
    });

    let resp = client
        .get(url)
        .query(&[
            ("cb", cb),
            ("param", &param.to_string()),
        ])
        .header("Referer", "https://so.eastmoney.com/")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;

    // jsonp: jQuery({...})
    let start = resp.find('(').ok_or("jsonp 解析失败")?;
    let end = resp.rfind(')').ok_or("jsonp 解析失败")?;
    let json_text = &resp[start + 1..end];
    let v: serde_json::Value = serde_json::from_str(json_text).map_err(|e| e.to_string())?;

    let mut titles = Vec::new();
    if let Some(arr) = v
        .pointer("/result/cmsArticleWebOld")
        .and_then(|x| x.as_array())
    {
        for item in arr {
            if let Some(t) = item.get("title").and_then(|x| x.as_str()) {
                titles.push(t.to_string());
            }
        }
    }
    Ok(titles)
}

/// 公告情绪回看自然日（不含未来）；宽基指数新闻衰减快，用更短窗口
pub const MESSAGE_LOOKBACK_DAYS: i64 = 7;

fn message_lookback_for(stock: &Stock) -> i64 {
    match crate::message_sentiment::classify(stock) {
        crate::message_sentiment::MessageKind::BroadEtf => 2,
        crate::message_sentiment::MessageKind::Gold | crate::message_sentiment::MessageKind::OverseasEtf => {
            5
        }
        _ => MESSAGE_LOOKBACK_DAYS,
    }
}

fn eval_message_from_archive(
    stock: &Stock,
    archive: &MessageArchive,
    as_of: NaiveDate,
) -> SignalContribution {
    let profile = crate::message_sentiment::profile_for(stock);
    let lookback = message_lookback_for(stock);
    let dated = archive.items_as_of(as_of, lookback);
    if dated.is_empty() {
        return contrib(
            "message",
            "消息面",
            "舆情",
            0.0,
            format!("{} · 近 {lookback} 日无相关消息", profile.label),
            "ok",
        );
    }
    let (score, note) = crate::message_sentiment::score_titles_dated(&profile, as_of, &dated);
    // 宽基：软化概率，避免弱命中就标成「高置信」（此前高置信准确率反低于整体）
    if profile.kind == crate::message_sentiment::MessageKind::BroadEtf {
        contrib_soft("message", "消息面", "舆情", score, note, "ok")
    } else {
        contrib("message", "消息面", "舆情", score, note, "ok")
    }
}

async fn eval_message(stock: &Stock) -> SignalContribution {
    let lookback = message_lookback_for(stock);
    match crate::cninfo::fetch_recent(stock, lookback + 3).await {
        Ok(archive) => {
            let as_of = chrono::Local::now().date_naive();
            let mut c = eval_message_from_archive(stock, &archive, as_of);
            if archive.is_empty() {
                let profile = crate::message_sentiment::profile_for(stock);
                c.note = format!("{} · 近 {} 日未拉到消息", profile.label, lookback + 3);
                c.status = "degraded".into();
            }
            c
        }
        Err(e) => contrib(
            "message",
            "消息面",
            "舆情",
            0.0,
            format!("消息暂不可用: {e}"),
            "degraded",
        ),
    }
}

async fn eval_news() -> SignalContribution {
    match fetch_eastmoney_news_titles("A股 市场", 12).await {
        Ok(titles) => {
            let (score, note) = sentiment_from_titles(
                &titles,
                &["回暖", "反弹", "流入", "利好", "增长", "复苏"],
                &["下跌", "承压", "流出", "风险", "收紧", "暴跌"],
            );
            contrib("news", "资讯新闻", "舆情", score, note, "ok")
        }
        Err(e) => contrib(
            "news",
            "资讯新闻",
            "舆情",
            0.0,
            format!("暂不可用: {e}"),
            "degraded",
        ),
    }
}

async fn eval_policy() -> SignalContribution {
    match fetch_eastmoney_news_titles("政策 央行 财政 监管", 12).await {
        Ok(titles) => {
            let (score, note) = sentiment_from_titles(
                &titles,
                &["降准", "降息", "支持", "刺激", "放开", "利好", "扩内需"],
                &["收紧", "监管", "处罚", "限制", "加税", "整顿"],
            );
            contrib("policy", "政策面", "宏观", score, note, "ok")
        }
        Err(e) => contrib(
            "policy",
            "政策面",
            "宏观",
            0.0,
            format!("暂不可用: {e}"),
            "degraded",
        ),
    }
}

async fn eval_us_market() -> SignalContribution {
    // 腾讯行情：纳斯达克 .IXIC / 标普 .INX 不一定稳定，尝试美股 ETF proxy
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .user_agent("Mozilla/5.0 StockPredict/0.1")
        .build()
    {
        Ok(c) => c,
        Err(_) => return contrib("us_market", "美股联动", "宏观", 0.0, "客户端失败".into(), "degraded"),
    };

    // 东方财富美股指数：纳指 100.NDX / 标普 100.SPX（secid 前缀 100）
    let url = "https://push2.eastmoney.com/api/qt/ulist.np/get";
    let resp = client
        .get(url)
        .query(&[
            ("fltt", "2"),
            ("fields", "f2,f3,f12,f14"),
            ("secids", "100.NDX,100.SPX"),
        ])
        .header("Referer", "https://quote.eastmoney.com/")
        .send()
        .await;

    let Ok(resp) = resp else {
        return contrib("us_market", "美股联动", "宏观", 0.0, "请求失败".into(), "degraded");
    };
    let Ok(v) = resp.json::<serde_json::Value>().await else {
        return contrib("us_market", "美股联动", "宏观", 0.0, "解析失败".into(), "degraded");
    };

    let mut changes = Vec::new();
    if let Some(arr) = v.pointer("/data/diff").and_then(|x| x.as_array()) {
        for item in arr {
            let name = item.get("f14").and_then(|x| x.as_str()).unwrap_or("");
            let chg = item.get("f3").and_then(|x| {
                x.as_f64().or_else(|| x.as_str().and_then(|s| s.parse().ok()))
            });
            if let Some(c) = chg {
                changes.push((name.to_string(), c));
            }
        }
    }

    if changes.is_empty() {
        return contrib("us_market", "美股联动", "宏观", 0.0, "暂无美股指数".into(), "degraded");
    }

    let avg = changes.iter().map(|(_, c)| *c).sum::<f64>() / changes.len() as f64;
    let score = (avg / 1.2).clamp(-2.5, 2.5);
    let detail = changes
        .iter()
        .take(2)
        .map(|(n, c)| format!("{n} {c:+.2}%"))
        .collect::<Vec<_>>()
        .join(" · ");

    contrib("us_market", "美股联动", "宏观", score, detail, "ok")
}
