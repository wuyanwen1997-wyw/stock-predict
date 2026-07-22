//! 实时报价：腾讯 → 新浪 → 东财（跨限流平面故障切换）。

use crate::ashare::client::{
    apply_browser_headers, apply_headers, client, decode_gbk, log_warn, parse_f64, parse_str_f64,
    sleep_retry, HTTP_RETRY, QUOTE_URLS, REFERER_SINA, REFERER_TENCENT,
};
use crate::ashare::symbol::{to_secid, to_sina_symbol, to_tencent_symbol};
use crate::models::{Stock, StockQuote};
use std::collections::HashMap;

const TENCENT_QUOTE_URLS: &[&str] = &[
    "https://qt.gtimg.cn/q=",
    "https://sqt.gtimg.cn/q=",
];
const SINA_QUOTE_URL: &str = "https://hq.sinajs.cn/list=";

/// 批量拉取实时行情（腾讯 → 新浪 → 东财）
pub async fn fetch_stock_quotes(stocks: &[Stock]) -> Result<HashMap<String, StockQuote>, String> {
    if stocks.is_empty() {
        return Ok(HashMap::new());
    }

    let http = client()?;
    let mut errors = Vec::new();

    match fetch_tencent_quotes(&http, stocks).await {
        Ok(map) if quote_map_usable(&map) => return Ok(map),
        Ok(_) => {
            let msg = "腾讯行情返回为空".to_string();
            log_warn("quote", &msg);
            errors.push(msg);
        }
        Err(e) => {
            log_warn("quote", &e);
            errors.push(e);
        }
    }

    match fetch_sina_quotes(&http, stocks).await {
        Ok(map) if quote_map_usable(&map) => return Ok(map),
        Ok(_) => {
            let msg = "新浪行情返回为空".to_string();
            log_warn("quote", &msg);
            errors.push(msg);
        }
        Err(e) => {
            log_warn("quote", &e);
            errors.push(e);
        }
    }

    let secids: Vec<String> = stocks
        .iter()
        .map(|s| to_secid(&s.market, &s.code))
        .collect();
    match fetch_quote_response(&http, &secids.join(",")).await {
        Ok(resp) => {
            let map = parse_quote_map(&resp);
            if quote_map_usable(&map) {
                return Ok(map);
            }
            let msg = "东财行情返回为空".to_string();
            log_warn("quote", &msg);
            errors.push(msg);
        }
        Err(e) => {
            log_warn("quote", &e);
            errors.push(e);
        }
    }

    Err(format!(
        "行情请求全部失败: {}",
        if errors.is_empty() {
            "无可用数据源".into()
        } else {
            errors.join("；")
        }
    ))
}

pub fn apply_quote(stock: &mut Stock, quote: &StockQuote) {
    stock.price = quote.price;
    stock.change_pct = quote.change_pct;
}

fn quote_map_usable(map: &HashMap<String, StockQuote>) -> bool {
    map.values().any(|q| q.price.map(|p| p > 0.0).unwrap_or(false))
}

async fn fetch_tencent_quotes(
    http: &reqwest::Client,
    stocks: &[Stock],
) -> Result<HashMap<String, StockQuote>, String> {
    let symbols: Vec<String> = stocks
        .iter()
        .map(|s| to_tencent_symbol(&s.market, &s.code))
        .collect();
    let query = symbols.join(",");
    let mut errors = Vec::new();

    for base in TENCENT_QUOTE_URLS {
        let url = format!("{base}{query}");
        for attempt in 0..HTTP_RETRY {
            sleep_retry(attempt).await;
            let result = apply_headers(http.get(&url), REFERER_TENCENT).send().await;
            let resp = match result {
                Ok(r) => r,
                Err(e) => {
                    let msg = format!("{base} 请求失败(尝试{}): {e}", attempt + 1);
                    log_warn("quote-tx", &msg);
                    errors.push(msg);
                    continue;
                }
            };
            let resp = match resp.error_for_status() {
                Ok(r) => r,
                Err(e) => {
                    let msg = format!("{base} 响应异常(尝试{}): {e}", attempt + 1);
                    log_warn("quote-tx", &msg);
                    errors.push(msg);
                    continue;
                }
            };
            match resp.bytes().await {
                Ok(bytes) => {
                    let text = decode_gbk(&bytes);
                    let map = parse_tencent_quote_text(&text);
                    if quote_map_usable(&map) {
                        return Ok(map);
                    }
                    let msg = format!("{base} 解析为空(尝试{})", attempt + 1);
                    log_warn("quote-tx", &msg);
                    errors.push(msg);
                }
                Err(e) => {
                    let msg = format!("{base} 读取失败(尝试{}): {e}", attempt + 1);
                    log_warn("quote-tx", &msg);
                    errors.push(msg);
                }
            }
        }
    }

    Err(format!(
        "腾讯行情失败: {}",
        if errors.is_empty() {
            "无可用节点".into()
        } else {
            errors.join("；")
        }
    ))
}

async fn fetch_sina_quotes(
    http: &reqwest::Client,
    stocks: &[Stock],
) -> Result<HashMap<String, StockQuote>, String> {
    let symbols: Vec<String> = stocks
        .iter()
        .map(|s| to_sina_symbol(&s.market, &s.code))
        .collect();
    let url = format!("{SINA_QUOTE_URL}{}", symbols.join(","));
    let mut errors = Vec::new();

    for attempt in 0..HTTP_RETRY {
        sleep_retry(attempt).await;
        let result = apply_headers(http.get(&url), REFERER_SINA).send().await;
        let resp = match result {
            Ok(r) => r,
            Err(e) => {
                let msg = format!("新浪行情请求失败(尝试{}): {e}", attempt + 1);
                log_warn("quote-sina", &msg);
                errors.push(msg);
                continue;
            }
        };
        let resp = match resp.error_for_status() {
            Ok(r) => r,
            Err(e) => {
                let msg = format!("新浪行情响应异常(尝试{}): {e}", attempt + 1);
                log_warn("quote-sina", &msg);
                errors.push(msg);
                continue;
            }
        };
        match resp.bytes().await {
            Ok(bytes) => {
                let text = decode_gbk(&bytes);
                let map = parse_sina_quote_text(&text);
                if quote_map_usable(&map) {
                    return Ok(map);
                }
                let msg = format!("新浪行情解析为空(尝试{})", attempt + 1);
                log_warn("quote-sina", &msg);
                errors.push(msg);
            }
            Err(e) => {
                let msg = format!("新浪行情读取失败(尝试{}): {e}", attempt + 1);
                log_warn("quote-sina", &msg);
                errors.push(msg);
            }
        }
    }

    Err(format!(
        "新浪行情失败: {}",
        if errors.is_empty() {
            "无可用响应".into()
        } else {
            errors.join("；")
        }
    ))
}

async fn fetch_quote_response(
    http: &reqwest::Client,
    secids: &str,
) -> Result<serde_json::Value, String> {
    let mut errors = Vec::new();
    for url in QUOTE_URLS {
        for attempt in 0..HTTP_RETRY {
            sleep_retry(attempt).await;
            let result = apply_browser_headers(http.get(*url))
                .query(&[
                    ("fltt", "2"),
                    ("invt", "2"),
                    ("fields", "f2,f3,f4,f12,f14,f15,f16,f17,f18,f5,f6"),
                    ("secids", secids),
                ])
                .send()
                .await;

            let resp = match result {
                Ok(r) => r,
                Err(e) => {
                    let msg = format!("{url} 请求失败(尝试{}): {e}", attempt + 1);
                    log_warn("quote", &msg);
                    errors.push(msg);
                    continue;
                }
            };

            let resp = match resp.error_for_status() {
                Ok(r) => r,
                Err(e) => {
                    let msg = format!("{url} 响应异常(尝试{}): {e}", attempt + 1);
                    log_warn("quote", &msg);
                    errors.push(msg);
                    continue;
                }
            };

            match resp.json::<serde_json::Value>().await {
                Ok(v) => {
                    if v.pointer("/data/diff").and_then(|d| d.as_array()).is_some() {
                        return Ok(v);
                    }
                    let msg = format!("{url} 返回无 diff 数据(尝试{})", attempt + 1);
                    log_warn("quote", &msg);
                    errors.push(msg);
                }
                Err(e) => {
                    let msg = format!("{url} 解析失败(尝试{}): {e}", attempt + 1);
                    log_warn("quote", &msg);
                    errors.push(msg);
                }
            }
        }
    }

    Err(format!(
        "东财行情请求全部失败: {}",
        if errors.is_empty() {
            "无可用节点".into()
        } else {
            errors.join("；")
        }
    ))
}

fn parse_quote_map(resp: &serde_json::Value) -> HashMap<String, StockQuote> {
    let mut map = HashMap::new();
    let Some(items) = resp.pointer("/data/diff").and_then(|v| v.as_array()) else {
        return map;
    };

    for item in items {
        let code = item
            .get("f12")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        if code.is_empty() {
            continue;
        }

        let price = parse_f64(item.get("f2").unwrap_or(&serde_json::Value::Null)).filter(|v| *v > 0.0);
        let change_pct = parse_f64(item.get("f3").unwrap_or(&serde_json::Value::Null));
        let change_amt = parse_f64(item.get("f4").unwrap_or(&serde_json::Value::Null));
        let volume = parse_f64(item.get("f5").unwrap_or(&serde_json::Value::Null));
        let turnover = parse_f64(item.get("f6").unwrap_or(&serde_json::Value::Null));
        let high = parse_f64(item.get("f15").unwrap_or(&serde_json::Value::Null));
        let low = parse_f64(item.get("f16").unwrap_or(&serde_json::Value::Null));
        let open = parse_f64(item.get("f17").unwrap_or(&serde_json::Value::Null));
        let prev_close = parse_f64(item.get("f18").unwrap_or(&serde_json::Value::Null));

        map.insert(
            code,
            StockQuote {
                price,
                change_pct,
                change_amt,
                open,
                high,
                low,
                prev_close,
                volume,
                turnover,
            },
        );
    }

    map
}

/// 解析腾讯 `v_sh600519="…~…";` 文本（已 GBK 解码）。
pub(crate) fn parse_tencent_quote_text(text: &str) -> HashMap<String, StockQuote> {
    let mut map = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some(rest) = line.strip_prefix("v_") else {
            continue;
        };
        let Some((sym, body)) = rest.split_once('=') else {
            continue;
        };
        let body = body.trim().trim_matches('"').trim_end_matches(';');
        if body.is_empty() || body == "1" {
            continue;
        }
        let fields: Vec<&str> = body.split('~').collect();
        if fields.len() < 35 {
            continue;
        }
        let code = if fields.len() > 2 && !fields[2].is_empty() {
            fields[2].to_string()
        } else {
            strip_market_prefix(sym)
        };
        if code.is_empty() {
            continue;
        }

        let price = field_f64(&fields, 3).filter(|v| *v > 0.0);
        let prev_close = field_f64(&fields, 4);
        let open = field_f64(&fields, 5);
        let volume = field_f64(&fields, 6).or_else(|| field_f64(&fields, 36));
        let change_amt = field_f64(&fields, 31);
        let change_pct = field_f64(&fields, 32);
        let high = field_f64(&fields, 33);
        let low = field_f64(&fields, 34);
        // 腾讯成交额单位为万元，换算为元与东财对齐
        let turnover = field_f64(&fields, 37).map(|v| v * 10_000.0);

        map.insert(
            code,
            StockQuote {
                price,
                change_pct,
                change_amt,
                open,
                high,
                low,
                prev_close,
                volume,
                turnover,
            },
        );
    }
    map
}

/// 解析新浪 `var hq_str_sh600519="…,…";` 文本（已 GBK 解码）。
pub(crate) fn parse_sina_quote_text(text: &str) -> HashMap<String, StockQuote> {
    let mut map = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some(eq) = line.find('=') else {
            continue;
        };
        let left = &line[..eq];
        let right = line[eq + 1..].trim().trim_matches(';').trim_matches('"');
        if right.is_empty() {
            continue;
        }
        let sym = left
            .rsplit('_')
            .next()
            .unwrap_or(left)
            .trim();
        let code = strip_market_prefix(sym);
        if code.is_empty() {
            continue;
        }

        let fields: Vec<&str> = right.split(',').collect();
        // 新浪: 0名称 1今开 2昨收 3现价 4最高 5最低 8成交量(股) 9成交额(元) …
        if fields.len() < 10 {
            continue;
        }
        let open = field_f64(&fields, 1);
        let prev_close = field_f64(&fields, 2);
        let price = field_f64(&fields, 3).filter(|v| *v > 0.0);
        let high = field_f64(&fields, 4);
        let low = field_f64(&fields, 5);
        // 成交量：股 → 手（÷100），与东财/腾讯「手」对齐
        let volume = field_f64(&fields, 8).map(|v| v / 100.0);
        let turnover = field_f64(&fields, 9);
        let (change_amt, change_pct) = match (price, prev_close) {
            (Some(p), Some(pc)) if pc > 0.0 => {
                let amt = p - pc;
                (Some(amt), Some(amt / pc * 100.0))
            }
            _ => (None, None),
        };

        map.insert(
            code,
            StockQuote {
                price,
                change_pct,
                change_amt,
                open,
                high,
                low,
                prev_close,
                volume,
                turnover,
            },
        );
    }
    map
}

fn strip_market_prefix(sym: &str) -> String {
    let s = sym.trim();
    if s.len() > 2 && (s.starts_with("sh") || s.starts_with("sz") || s.starts_with("bj")) {
        s[2..].to_string()
    } else {
        s.to_string()
    }
}

fn field_f64(fields: &[&str], idx: usize) -> Option<f64> {
    fields.get(idx).and_then(|s| parse_str_f64(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tencent_fixture() {
        // 字段下标按文档：3价 4昨收 5开 6量 31涨跌额 32涨跌幅 33高 34低 37额(万元)
        let mut parts = vec!["x"; 40];
        parts[1] = "贵州茅台";
        parts[2] = "600519";
        parts[3] = "1460.00";
        parts[4] = "1450.00";
        parts[5] = "1455.00";
        parts[6] = "1000";
        parts[31] = "10.00";
        parts[32] = "0.69";
        parts[33] = "1470.00";
        parts[34] = "1440.00";
        parts[37] = "150.5";
        let body = parts.join("~");
        let text = format!("v_sh600519=\"{body}\";\n");
        let map = parse_tencent_quote_text(&text);
        let q = map.get("600519").expect("code");
        assert!((q.price.unwrap() - 1460.0).abs() < 1e-6);
        assert!((q.prev_close.unwrap() - 1450.0).abs() < 1e-6);
        assert!((q.open.unwrap() - 1455.0).abs() < 1e-6);
        assert!((q.volume.unwrap() - 1000.0).abs() < 1e-6);
        assert!((q.change_amt.unwrap() - 10.0).abs() < 1e-6);
        assert!((q.change_pct.unwrap() - 0.69).abs() < 1e-6);
        assert!((q.high.unwrap() - 1470.0).abs() < 1e-6);
        assert!((q.low.unwrap() - 1440.0).abs() < 1e-6);
        assert!((q.turnover.unwrap() - 1_505_000.0).abs() < 1e-3);
    }

    #[test]
    fn parse_sina_fixture() {
        let text = r#"var hq_str_sh600519="贵州茅台,1455.00,1450.00,1460.00,1470.00,1440.00,1459.00,1461.00,100000,146000000.00,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,2024-01-01,15:00:00,00";
"#;
        let map = parse_sina_quote_text(text);
        let q = map.get("600519").expect("code");
        assert!((q.price.unwrap() - 1460.0).abs() < 1e-6);
        assert!((q.open.unwrap() - 1455.0).abs() < 1e-6);
        assert!((q.prev_close.unwrap() - 1450.0).abs() < 1e-6);
        assert!((q.high.unwrap() - 1470.0).abs() < 1e-6);
        assert!((q.low.unwrap() - 1440.0).abs() < 1e-6);
        assert!((q.volume.unwrap() - 1000.0).abs() < 1e-6); // 100000股 → 1000手
        assert!((q.turnover.unwrap() - 146_000_000.0).abs() < 1e-3);
        assert!((q.change_amt.unwrap() - 10.0).abs() < 1e-6);
        assert!((q.change_pct.unwrap() - 10.0 / 1450.0 * 100.0).abs() < 1e-6);
    }

    #[test]
    fn parse_eastmoney_fixture() {
        let resp = serde_json::json!({
            "data": {
                "diff": [{
                    "f12": "600519",
                    "f2": 1460.0,
                    "f3": 0.69,
                    "f4": 10.0,
                    "f5": 1000.0,
                    "f6": 1505000.0,
                    "f15": 1470.0,
                    "f16": 1440.0,
                    "f17": 1455.0,
                    "f18": 1450.0
                }]
            }
        });
        let map = parse_quote_map(&resp);
        let q = map.get("600519").expect("code");
        assert!((q.price.unwrap() - 1460.0).abs() < 1e-6);
        assert!((q.change_pct.unwrap() - 0.69).abs() < 1e-6);
        assert!((q.turnover.unwrap() - 1_505_000.0).abs() < 1e-3);
    }

    #[test]
    fn usable_requires_positive_price() {
        let mut empty = HashMap::new();
        empty.insert(
            "600519".into(),
            StockQuote {
                price: Some(0.0),
                change_pct: None,
                change_amt: None,
                open: None,
                high: None,
                low: None,
                prev_close: None,
                volume: None,
                turnover: None,
            },
        );
        assert!(!quote_map_usable(&empty));
    }
}
