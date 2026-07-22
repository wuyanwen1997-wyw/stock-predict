/// A 股代码与各数据源 symbol / secid 映射。

pub fn to_tencent_symbol(market: &str, code: &str) -> String {
    match market {
        "SH" => format!("sh{code}"),
        _ => format!("sz{code}"),
    }
}

pub fn to_sina_symbol(market: &str, code: &str) -> String {
    match market {
        "SH" => format!("sh{code}"),
        _ => format!("sz{code}"),
    }
}

pub fn to_secid(market: &str, code: &str) -> String {
    match market {
        "SZ" => format!("0.{code}"),
        _ => format!("1.{code}"),
    }
}

/// 根据代码推断市场：ETF/基金 5 开头多为沪市，1 开头多为深市
pub fn infer_market(code: &str) -> &'static str {
    let c = code.trim();
    if c.starts_with('6') || c.starts_with("688") || c.starts_with('5') {
        "SH"
    } else if c.starts_with('0') || c.starts_with('3') || c.starts_with('1') {
        "SZ"
    } else if c.starts_with('8') || c.starts_with('4') {
        // 北交所暂按深市行情接口兼容，或后续单独支持
        "SZ"
    } else {
        "SZ"
    }
}

pub(crate) fn parse_market_from_sc(sc: &str) -> (String, String) {
    if let Some(code) = sc.strip_prefix("SH") {
        ("SH".into(), code.to_string())
    } else if let Some(code) = sc.strip_prefix("SZ") {
        ("SZ".into(), code.to_string())
    } else if sc.starts_with('6') {
        ("SH".into(), sc.to_string())
    } else {
        ("SZ".into(), sc.to_string())
    }
}

pub(crate) fn market_from_search_item(item: &serde_json::Value, code: &str) -> String {
    // 优先东财 QuoteID / MktNum：1=沪 0=深
    if let Some(qid) = item.get("QuoteID").and_then(|v| v.as_str()) {
        if let Some((mkt, _)) = qid.split_once('.') {
            return match mkt {
                "1" => "SH".into(),
                "0" => "SZ".into(),
                _ => infer_market(code).into(),
            };
        }
    }
    if let Some(n) = item.get("MktNum") {
        let num = n
            .as_str()
            .and_then(|s| s.parse::<i64>().ok())
            .or_else(|| n.as_i64());
        if let Some(num) = num {
            return match num {
                1 => "SH".into(),
                0 => "SZ".into(),
                _ => infer_market(code).into(),
            };
        }
    }
    infer_market(code).into()
}

pub(crate) fn sector_from_search_item(item: &serde_json::Value, name: &str) -> String {
    let classify = item
        .get("Classify")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let security = item
        .get("SecurityTypeName")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if classify.eq_ignore_ascii_case("Fund")
        || security.contains("基金")
        || name.contains("ETF")
        || name.contains("基金")
    {
        "ETF".into()
    } else {
        "—".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secid_mapping() {
        assert_eq!(to_secid("SH", "600519"), "1.600519");
        assert_eq!(to_secid("SZ", "000858"), "0.000858");
        assert_eq!(to_secid("SH", "510980"), "1.510980");
    }

    #[test]
    fn parse_market_prefix() {
        assert_eq!(
            parse_market_from_sc("SH600519"),
            ("SH".into(), "600519".into())
        );
        assert_eq!(
            parse_market_from_sc("SZ000858"),
            ("SZ".into(), "000858".into())
        );
    }

    #[test]
    fn tencent_symbol_mapping() {
        assert_eq!(to_tencent_symbol("SH", "600519"), "sh600519");
        assert_eq!(to_tencent_symbol("SZ", "300628"), "sz300628");
        assert_eq!(to_tencent_symbol("SH", "510980"), "sh510980");
    }

    #[test]
    fn infer_market_for_etf() {
        assert_eq!(infer_market("510980"), "SH");
        assert_eq!(infer_market("159915"), "SZ");
        assert_eq!(infer_market("600519"), "SH");
        assert_eq!(infer_market("000858"), "SZ");
    }
}
