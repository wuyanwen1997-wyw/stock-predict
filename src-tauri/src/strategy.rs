use crate::algo::fuse::{
    contrib, contrib_soft, fuse, neutral, reconcile_index_factor_capital,
    reconcile_index_factor_message, reconcile_index_momentum, reconcile_multiday_noise,
};
use crate::algo::tech::{eval_factor, eval_mean_reversion, eval_momentum, eval_volume};
use crate::capital_flow::CapitalFlowArchive;
use crate::cninfo::MessageArchive;
use crate::factor_model;
use crate::models::{DailyBar, SignalContribution, Stock};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

pub use crate::algo::fuse::EnsembleSignal;

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
