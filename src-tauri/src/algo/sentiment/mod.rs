//! 按标的类型划分消息面关键词、搜索词与打分强度。

use crate::models::Stock;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageKind {
    /// 一般上市公司
    Corporate,
    /// 白酒 / 消费
    Consumer,
    /// 银行 / 保险 / 证券
    Finance,
    /// 半导体 / AI / 新能源 / 光伏 / 安防 / 面板
    Tech,
    /// 医药
    Pharma,
    /// 有色（非纯黄金）/ 电力等周期
    Cyclical,
    /// 黄金股 / 黄金 ETF
    Gold,
    /// 宽基 A 股 ETF
    BroadEtf,
    /// 行业主题 ETF（映射到对应主题逻辑）
    ThemeEtf,
    /// 海外指数 ETF
    OverseasEtf,
}

#[derive(Debug, Clone, Copy)]
pub struct MessageProfile {
    pub kind: MessageKind,
    pub label: &'static str,
    /// 单条标题净命中的基础分（再乘 scale）
    pub hit_weight: f64,
    /// 整体信号强度 0~1，主题/宽基 ETF 通常更弱
    pub scale: f64,
    pub bullish: &'static [&'static str],
    pub bearish: &'static [&'static str],
    /// 额外资讯搜索词（不含股票名本身）
    pub extra_queries: &'static [&'static str],
}

// ---------- 通用公司事件 ----------
const CORP_BULL: &[&str] = &[
    "增长", "回购", "中标", "利好", "上涨", "突破", "签约", "盈利", "增持", "预增", "扭亏",
    "获得", "通过", "批准", "超预期", "高增", "景气", "扩产", "投产", "量产", "订单",
    "合作", "收购", "战略", "独家", "专利", "授权", "落地", "复牌", "摘帽", "注入",
    "业绩预喜", "同比增", "环比增", "创新高", "大涨", "涨停", "反弹", "走强", "净流入",
    "加仓", "超配",
];
const CORP_BEAR: &[&str] = &[
    "下调", "减持", "亏损", "处罚", "立案", "跌停", "风险", "爆雷", "问询", "预减", "退市",
    "违规", "诉讼", "冻结", "警示", "停牌", "调查", "造假", "违约", "债务", "爆仓",
    "业绩预降", "业绩变脸", "商誉减值", "计提", "终止", "失败", "否决", "撤回", "推迟",
    "下滑", "同比降", "环比降", "大跌", "暴跌", "走弱", "跌破", "净流出", "减仓", "低配",
];

// ---------- 消费 / 白酒 ----------
const CONSUMER_BULL: &[&str] = &[
    "提价", "涨价", "动销", "旺季", "开瓶", "批价上涨", "渠道补货", "库存下降", "高端化",
    "消费复苏", "出行复苏", "免税", "客流回升", "销量增长", "份额提升",
];
const CONSUMER_BEAR: &[&str] = &[
    "降价", "窜货", "库存高企", "动销疲软", "批价下跌", "消费疲软", "需求下滑",
    "渠道去库存", "宴席减少", "客流下滑",
];

// ---------- 金融 ----------
const FIN_BULL: &[&str] = &[
    "降准", "降息", "宽松", "息差改善", "不良下降", "资产质量改善", "零售回暖",
    "成交额放大", "两融余额升", "日均成交", "牛市", "行情回暖", "保费增长", "新单增长",
];
const FIN_BEAR: &[&str] = &[
    "加息", "收紧", "息差收窄", "不良上升", "资产质量恶化", "爆雷", "理财赎回",
    "成交低迷", "两融余额降", "熊市", "保费下滑", "退保",
];

// ---------- 科技 / 制造 ----------
const TECH_BULL: &[&str] = &[
    "国产替代", "自主可控", "芯片", "先进制程", "产能满载", "涨价函", "缺货", "大模型",
    "算力", "AI应用", "中标", "定点", "装机量", "渗透率提升", "补贴", "出海",
];
const TECH_BEAR: &[&str] = &[
    "制裁", "出口管制", "实体清单", "禁令", "产能过剩", "价格战", "降价", "库存积压",
    "需求不及预期", "砍单", "扩产放缓", "补贴退坡",
];

// ---------- 医药 ----------
const PHARMA_BULL: &[&str] = &[
    "获批", "IND", "临床", "三期", "NDA", "BLA", "纳入医保", "集采中标", "创新药",
    "授权出海", "BD合作", "阳性结果", "突破性疗法", "新药上市",
];
const PHARMA_BEAR: &[&str] = &[
    "集采降价", "未中标", "临床失败", "安全性", "召回", "停产", "医保谈判降幅",
    "负面临床", "驳回", "撤回申请",
];

// ---------- 周期 ----------
const CYCLICAL_BULL: &[&str] = &[
    "涨价", "供给收缩", "库存低位", "景气上行", "开工率升", "电价上浮", "来水偏丰",
    "铜价上涨", "锂价上涨", "煤价上涨",
];
const CYCLICAL_BEAR: &[&str] = &[
    "跌价", "供给过剩", "库存高企", "景气下行", "开工率降", "电价下调", "来水偏枯",
    "铜价下跌", "锂价下跌", "煤价下跌",
];

// ---------- 黄金（股/ETF）——宏观驱动为主 ----------
const GOLD_BULL: &[&str] = &[
    "降息", "宽松", "暂停加息", "结束加息", "鸽派", "降息预期", "实际利率下行",
    "美元走弱", "美元下跌", "美元指数回落", "通胀回升", "滞胀",
    "避险", "避险情绪", "避险买盘", "地缘冲突", "地缘紧张", "战争", "开战", "空袭",
    "冲突升级", "制裁", "中东紧张", "台海紧张", "军事冲突",
    "金价上涨", "金价大涨", "金价走强", "金价创新高", "金价飙升", "升破", "收涨",
    "购金", "央行购金", "黄金储备", "增持黄金", "净申购", "资金流入",
];
const GOLD_BEAR: &[&str] = &[
    "加息", "鹰派", "继续加息", "加速加息", "紧缩", "缩表", "加息预期",
    "实际利率上行", "美元走强", "美元上涨", "美元指数走强",
    "避险情绪降温", "风险偏好回升", "停火", "和谈", "局势缓和", "冲突降温",
    "金价下跌", "金价大跌", "金价走弱", "金价跌破", "收跌", "抛售黄金",
    "获利了结", "净赎回", "资金流出",
];
const GOLD_QUERIES: &[&str] = &[
    "黄金 美联储",
    "黄金 加息",
    "黄金 降息",
    "金价 避险",
    "黄金 战争",
    "黄金 地缘",
    "美元指数 黄金",
];

// ---------- 宽基 ETF（上证/沪深宽基；细权重见 message_weights_broad_etf.json）----------
const BROAD_BULL: &[&str] = &[
    "央行降准", "降准", "LPR下调", "北向净买入", "北向流入", "外资流入", "社融超预期",
    "PMI回升", "成交额破万亿", "政策托底", "稳增长", "流动性宽松", "赚钱效应",
    "风险偏好回升", "降息", "宽松", "政策利好", "经济复苏",
];
const BROAD_BEAR: &[&str] = &[
    "北向净卖出", "北向流出", "外资流出", "暴跌", "跳水", "杀跌", "破位", "亏钱效应",
    "流动性收紧", "成交低迷", "风险偏好下降", "沪指下跌", "上证下跌", "ETF净赎回",
    "两融余额降", "社融不及预期", "PMI回落", "收紧", "熊市", "恐慌", "失守", "经济疲软",
    // 样本上标题出现后次日偏跌（追高/情绪词，作反向）
    "回落", "承压", "缩量", "走弱", "下跌", "跌破", "反弹", "放量", "收涨", "新高", "活跃",
    "增量资金",
];
const BROAD_QUERIES: &[&str] = &[
    "上证指数",
    "沪指",
    "A股 市场",
    "北向资金",
    "两市成交",
    "降准 稳增长",
    "成交额破万亿",
    "风险偏好",
];

#[derive(Debug, Deserialize)]
struct BroadWeightFile {
    bullish: HashMap<String, f64>,
    bearish: HashMap<String, f64>,
    #[serde(default)]
    extra_queries: Vec<String>,
    hit_weight: Option<f64>,
    scale: Option<f64>,
}

fn broad_weight_file() -> Option<&'static BroadWeightFile> {
    static FILE: OnceLock<Option<BroadWeightFile>> = OnceLock::new();
    FILE.get_or_init(|| {
        serde_json::from_str::<BroadWeightFile>(include_str!(
            "../../../resources/message_weights_broad_etf.json"
        ))
        .ok()
    })
    .as_ref()
}

// ---------- 海外 ETF ----------
const OVERSEAS_BULL: &[&str] = &[
    "降息", "鸽派", "纳指上涨", "标普上涨", "科技股大涨", "风险偏好回升", "软着陆",
];
const OVERSEAS_BEAR: &[&str] = &[
    "加息", "鹰派", "纳指下跌", "标普下跌", "科技股大跌", "衰退担忧", "关税",
];
const OVERSEAS_QUERIES: &[&str] = &["纳斯达克", "标普500", "美股 科技", "美联储"];

fn merge_words(base: &'static [&'static str], extra: &'static [&'static str]) -> Vec<&'static str> {
    let mut v = Vec::with_capacity(base.len() + extra.len());
    v.extend_from_slice(base);
    v.extend_from_slice(extra);
    v
}

/// 解析标的消息面类型（含 ETF 主题细分）
pub fn classify(stock: &Stock) -> MessageKind {
    let name = stock.name.as_str();
    let sector = stock.sector.as_str();
    let code = stock.code.as_str();

    // 黄金优先（名称/代码/有色里的黄金股）
    if name.contains("黄金")
        || code == "518880"
        || code == "159934"
        || code == "159937"
        || code == "518800"
        || code == "600547"
    {
        return MessageKind::Gold;
    }

    if sector == "ETF" || name.contains("ETF") {
        if name.contains("纳指") || name.contains("标普") || name.contains("道指") || code.starts_with("513")
        {
            return MessageKind::OverseasEtf;
        }
        if name.contains("黄金") {
            return MessageKind::Gold;
        }
        if name.contains("沪深300")
            || name.contains("中证500")
            || name.contains("上证50")
            || name.contains("上证指数")
            || name.contains("上证综指")
            || name.contains("创业板")
            || name.contains("科创50")
            || name.contains("红利")
        {
            return MessageKind::BroadEtf;
        }
        return MessageKind::ThemeEtf;
    }

    match sector {
        "白酒" | "消费" | "家电" => MessageKind::Consumer,
        "银行" | "保险" | "证券" => MessageKind::Finance,
        "半导体" | "AI" | "新能源" | "光伏" | "安防" | "面板" => MessageKind::Tech,
        "医药" => MessageKind::Pharma,
        "有色" | "电力" => MessageKind::Cyclical,
        "ETF" => MessageKind::BroadEtf,
        _ => MessageKind::Corporate,
    }
}

fn theme_from_etf_name(name: &str) -> MessageKind {
    if name.contains("证券") {
        MessageKind::Finance
    } else if name.contains("半导体") || name.contains("人工智能") || name.contains("光伏") {
        MessageKind::Tech
    } else if name.contains("医药") {
        MessageKind::Pharma
    } else if name.contains("酒") {
        MessageKind::Consumer
    } else if name.contains("煤炭") {
        MessageKind::Cyclical
    } else if name.contains("黄金") {
        MessageKind::Gold
    } else {
        MessageKind::BroadEtf
    }
}

/// 生成完整画像（主题 ETF 会叠一层行业词）
pub fn profile_for(stock: &Stock) -> MessageProfile {
    let kind = classify(stock);
    match kind {
        MessageKind::Gold => MessageProfile {
            kind,
            label: "黄金/避险",
            hit_weight: 0.45,
            scale: 1.0,
            bullish: GOLD_BULL,
            bearish: GOLD_BEAR,
            extra_queries: GOLD_QUERIES,
        },
        MessageKind::BroadEtf => {
            let (hit_weight, scale) = broad_weight_file()
                .map(|w| {
                    (
                        w.hit_weight.unwrap_or(0.4),
                        w.scale.unwrap_or(0.95),
                    )
                })
                .unwrap_or((0.4, 0.95));
            MessageProfile {
                kind,
                label: "宽基ETF",
                hit_weight,
                scale,
                bullish: BROAD_BULL,
                bearish: BROAD_BEAR,
                extra_queries: BROAD_QUERIES,
            }
        },
        MessageKind::OverseasEtf => MessageProfile {
            kind,
            label: "海外ETF",
            hit_weight: 0.4,
            scale: 0.85,
            bullish: OVERSEAS_BULL,
            bearish: OVERSEAS_BEAR,
            extra_queries: OVERSEAS_QUERIES,
        },
        MessageKind::ThemeEtf => {
            let theme = theme_from_etf_name(&stock.name);
            let mut p = profile_for_kind(theme);
            p.kind = MessageKind::ThemeEtf;
            p.label = "主题ETF";
            p.scale = (p.scale * 0.85).clamp(0.5, 1.0);
            p
        }
        other => profile_for_kind(other),
    }
}

fn profile_for_kind(kind: MessageKind) -> MessageProfile {
    match kind {
        MessageKind::Consumer => MessageProfile {
            kind,
            label: "消费/白酒",
            hit_weight: 0.4,
            scale: 1.0,
            bullish: leak_static(CORP_BULL, CONSUMER_BULL),
            bearish: leak_static(CORP_BEAR, CONSUMER_BEAR),
            extra_queries: &["白酒 批价", "消费复苏"],
        },
        MessageKind::Finance => MessageProfile {
            kind,
            label: "金融",
            hit_weight: 0.4,
            scale: 1.0,
            bullish: leak_static(CORP_BULL, FIN_BULL),
            bearish: leak_static(CORP_BEAR, FIN_BEAR),
            extra_queries: &["降准", "两融余额", "银行 息差"],
        },
        MessageKind::Tech => MessageProfile {
            kind,
            label: "科技制造",
            hit_weight: 0.4,
            scale: 1.0,
            bullish: leak_static(CORP_BULL, TECH_BULL),
            bearish: leak_static(CORP_BEAR, TECH_BEAR),
            extra_queries: &["国产替代", "芯片 制裁", "新能源 装机"],
        },
        MessageKind::Pharma => MessageProfile {
            kind,
            label: "医药",
            hit_weight: 0.4,
            scale: 1.0,
            bullish: leak_static(CORP_BULL, PHARMA_BULL),
            bearish: leak_static(CORP_BEAR, PHARMA_BEAR),
            extra_queries: &["创新药", "集采", "医保谈判"],
        },
        MessageKind::Cyclical => MessageProfile {
            kind,
            label: "周期",
            hit_weight: 0.4,
            scale: 1.0,
            bullish: leak_static(CORP_BULL, CYCLICAL_BULL),
            bearish: leak_static(CORP_BEAR, CYCLICAL_BEAR),
            extra_queries: &["有色 涨价", "煤价", "电力 来水"],
        },
        MessageKind::Corporate
        | MessageKind::Gold
        | MessageKind::BroadEtf
        | MessageKind::ThemeEtf
        | MessageKind::OverseasEtf => MessageProfile {
            kind: MessageKind::Corporate,
            label: "一般个股",
            hit_weight: 0.4,
            scale: 1.0,
            bullish: CORP_BULL,
            bearish: CORP_BEAR,
            extra_queries: &[],
        },
    }
}

/// 合并两份静态词表为泄漏的 'static 切片（进程内缓存）
fn leak_static(a: &'static [&'static str], b: &'static [&'static str]) -> &'static [&'static str] {
    // 词表固定，用 OnceLock 按地址对缓存
    use std::collections::HashMap;
    use std::sync::OnceLock;
    static CACHE: OnceLock<std::sync::Mutex<HashMap<(usize, usize), &'static [&'static str]>>> =
        OnceLock::new();
    let key = (a.as_ptr() as usize, b.as_ptr() as usize);
    let cache = CACHE.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    let mut guard = cache.lock().unwrap();
    if let Some(v) = guard.get(&key) {
        return *v;
    }
    let merged = merge_words(a, b);
    let leaked: &'static [&'static str] = Box::leak(merged.into_boxed_slice());
    guard.insert(key, leaked);
    leaked
}

/// 按画像对标题打分；返回 (score, note)
pub fn score_titles(profile: &MessageProfile, titles: &[String]) -> (f64, String) {
    let dated: Vec<(chrono::NaiveDate, String)> = titles
        .iter()
        .map(|t| (chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap(), t.clone()))
        .collect();
    let as_of = chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
    score_titles_dated(profile, as_of, &dated)
}

/// 带日期的打分：近日报权更高；宽基混杂标题会向中性收缩
pub fn score_titles_dated(
    profile: &MessageProfile,
    as_of: chrono::NaiveDate,
    dated: &[(chrono::NaiveDate, String)],
) -> (f64, String) {
    if dated.is_empty() {
        return (0.0, format!("{} · 暂无标题", profile.label));
    }
    if profile.kind == MessageKind::BroadEtf {
        if let Some(w) = broad_weight_file() {
            return score_broad_dated(
                profile,
                as_of,
                dated,
                &w.bullish,
                &w.bearish,
                profile.hit_weight,
                profile.scale,
            );
        }
    }
    let titles: Vec<String> = dated.iter().map(|(_, t)| t.clone()).collect();
    score_titles_count(profile, &titles)
}

fn recency_weight(as_of: chrono::NaiveDate, day: chrono::NaiveDate) -> f64 {
    let age = (as_of - day).num_days().max(0) as f64;
    // day0=1.0, day1=0.72, day2=0.52, day3+=0.38
    (1.0 - 0.22 * age).clamp(0.35, 1.0)
}

fn score_broad_dated(
    profile: &MessageProfile,
    as_of: chrono::NaiveDate,
    dated: &[(chrono::NaiveDate, String)],
    bullish: &HashMap<String, f64>,
    bearish: &HashMap<String, f64>,
    hit_weight: f64,
    scale: f64,
) -> (f64, String) {
    let mut bull_keys: Vec<(&String, f64)> = bullish.iter().map(|(k, v)| (k, *v)).collect();
    let mut bear_keys: Vec<(&String, f64)> = bearish.iter().map(|(k, v)| (k, *v)).collect();
    bull_keys.sort_by(|a, b| b.0.chars().count().cmp(&a.0.chars().count()));
    bear_keys.sort_by(|a, b| b.0.chars().count().cmp(&a.0.chars().count()));

    let mut score = 0.0;
    let mut hits = 0usize;
    let mut bull_w = 0.0;
    let mut bear_w = 0.0;

    for (day, t) in dated {
        let rw = recency_weight(as_of, *day);
        let mut work = t.clone();
        let mut bull = 0.0;
        let mut bear = 0.0;

        for (kw, wt) in &bull_keys {
            let kw = kw.trim();
            if kw.is_empty() || *wt <= 0.0 {
                continue;
            }
            if let Some(idx) = work.find(kw) {
                bull += *wt;
                hits += 1;
                let end = idx + kw.len();
                work.replace_range(idx..end, &"　".repeat(kw.chars().count()));
            }
        }
        for (kw, wt) in &bear_keys {
            let kw = kw.trim();
            if kw.is_empty() || *wt <= 0.0 {
                continue;
            }
            if let Some(idx) = work.find(kw) {
                bear += *wt;
                hits += 1;
                let end = idx + kw.len();
                work.replace_range(idx..end, &"　".repeat(kw.chars().count()));
            }
        }

        if bull == 0.0 && bear == 0.0 {
            continue;
        }
        let net = (bull - bear).clamp(-2.5, 2.5);
        score += net * hit_weight * rw;
        bull_w += bull * rw;
        bear_w += bear * rw;
    }

    score *= scale;
    score = score.clamp(-2.2, 2.2);

    let note = if hits == 0 {
        format!("{} · 扫描 {} 条，信号不足", profile.label, dated.len())
    } else {
        let side = if score > 0.15 {
            "偏多"
        } else if score < -0.15 {
            "偏空"
        } else {
            "中性"
        };
        format!(
            "{} · {}条/命中{} · {} · 多{:.1}/空{:.1}",
            profile.label,
            dated.len(),
            hits,
            side,
            bull_w,
            bear_w
        )
    };
    (score, note)
}

fn score_titles_count(profile: &MessageProfile, titles: &[String]) -> (f64, String) {
    let mut score = 0.0;
    let mut hits = 0usize;
    for t in titles {
        let lower = t.to_lowercase();
        let mut bull = 0i32;
        let mut bear = 0i32;
        for w in profile.bullish {
            let w = w.trim();
            if w.is_empty() {
                continue;
            }
            if t.contains(w) || lower.contains(&w.to_lowercase()) {
                bull += 1;
            }
        }
        for w in profile.bearish {
            let w = w.trim();
            if w.is_empty() {
                continue;
            }
            if t.contains(w) || lower.contains(&w.to_lowercase()) {
                bear += 1;
            }
        }
        if bull == 0 && bear == 0 {
            continue;
        }
        let net = (bull - bear).clamp(-3, 3) as f64;
        score += net * profile.hit_weight;
        hits += (bull + bear) as usize;
    }
    score = (score * profile.scale).clamp(-2.5, 2.5);
    let note = if hits == 0 {
        format!("{} · 扫描 {} 条，中性", profile.label, titles.len())
    } else {
        format!(
            "{} · 扫描 {} 条，命中 {} 次",
            profile.label,
            titles.len(),
            hits
        )
    };
    (score, note)
}

/// 资讯搜索关键词列表：股票名 + 类型附加词
pub fn search_queries(stock: &Stock) -> Vec<String> {
    let profile = profile_for(stock);
    let code = stock
        .code
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>();
    let code = if code.len() >= 6 {
        code[code.len() - 6..].to_string()
    } else {
        format!("{:0>6}", code)
    };

    let mut q = Vec::new();
    if !stock.name.is_empty() {
        q.push(format!("{} {}", stock.name, code));
    } else {
        q.push(code);
    }
    for extra in profile.extra_queries {
        q.push((*extra).to_string());
    }
    // 宽基：合并 JSON 里的额外搜索词（训练脚本可更新）
    if profile.kind == MessageKind::BroadEtf {
        if let Some(w) = broad_weight_file() {
            for extra in &w.extra_queries {
                if !q.iter().any(|x| x == extra) {
                    q.push(extra.clone());
                }
            }
        }
    }
    q
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn classifies_gold_etf() {
        let s = stock("518880", "黄金ETF", "ETF");
        assert_eq!(classify(&s), MessageKind::Gold);
        let p = profile_for(&s);
        assert!(p.extra_queries.iter().any(|q| q.contains("美联储")));
    }

    #[test]
    fn gold_scores_rate_cut_positive() {
        let s = stock("518880", "黄金ETF", "ETF");
        let p = profile_for(&s);
        let (score, note) = score_titles(&p, &["美联储释放降息信号 金价走强".into()]);
        assert!(score > 0.0, "score={score} note={note}");
        assert!(note.contains("黄金"));
    }

    #[test]
    fn bank_uses_finance_profile() {
        let s = stock("600036", "招商银行", "银行");
        assert_eq!(classify(&s), MessageKind::Finance);
    }

    #[test]
    fn sse_etf_uses_weighted_broad() {
        let s = stock("510980", "上证指数ETF汇添富", "ETF");
        assert_eq!(classify(&s), MessageKind::BroadEtf);
        let p = profile_for(&s);
        let day = chrono::NaiveDate::from_ymd_opt(2026, 7, 16).unwrap();
        let (score, note) = score_titles_dated(
            &p,
            day,
            &[(
                day,
                "央行降准落地 北向净买入创近期新高".into(),
            )],
        );
        assert!(score > 0.0, "score={score} note={note}");
        assert!(note.contains("共识") || note.contains("命中"));
    }

    #[test]
    fn broad_bear_outflow_negative() {
        let s = stock("510980", "上证指数ETF汇添富", "ETF");
        let p = profile_for(&s);
        let day = chrono::NaiveDate::from_ymd_opt(2026, 7, 16).unwrap();
        let (score, note) = score_titles_dated(
            &p,
            day,
            &[(day, "北向净卖出扩大 沪指下跌 市场走弱".into())],
        );
        assert!(score < 0.0, "score={score} note={note}");
    }

    #[test]
    fn mixed_titles_can_net_near_neutral() {
        let s = stock("510980", "上证指数ETF汇添富", "ETF");
        let p = profile_for(&s);
        let day = chrono::NaiveDate::from_ymd_opt(2026, 7, 16).unwrap();
        let (score, _) = score_titles_dated(
            &p,
            day,
            &[
                (day, "央行降准 北向净买入".into()),
                (day, "北向净卖出 暴跌 跳水".into()),
            ],
        );
        // 多空对冲后幅度应明显小于单边强信号
        let (bull_only, _) = score_titles_dated(
            &p,
            day,
            &[(day, "央行降准 北向净买入".into())],
        );
        assert!(
            score.abs() < bull_only.abs(),
            "mixed={score} bull_only={bull_only}"
        );
    }
}
