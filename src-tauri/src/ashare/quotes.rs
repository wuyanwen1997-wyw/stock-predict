//! 实时报价。

use crate::ashare::client::{
    apply_browser_headers, client, log_warn, parse_f64, sleep_retry, HTTP_RETRY, QUOTE_URLS,
};
use crate::ashare::symbol::to_secid;
use crate::models::{Stock, StockQuote};
use std::collections::HashMap;

/// 批量拉取实时行情（多节点 + 重试）
pub async fn fetch_stock_quotes(stocks: &[Stock]) -> Result<HashMap<String, StockQuote>, String> {
    if stocks.is_empty() {
        return Ok(HashMap::new());
    }

    let secids: Vec<String> = stocks
        .iter()
        .map(|s| to_secid(&s.market, &s.code))
        .collect();

    let http = client()?;
    let resp = fetch_quote_response(&http, &secids.join(",")).await?;
    Ok(parse_quote_map(&resp))
}

pub fn apply_quote(stock: &mut Stock, quote: &StockQuote) {
    stock.price = quote.price;
    stock.change_pct = quote.change_pct;
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
        "行情请求全部失败: {}",
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

/// 热股补全用：code → (name, price, change_pct)
pub(crate) async fn fetch_hot_quote_map(
    http: &reqwest::Client,
    secids: &str,
) -> Result<HashMap<String, (String, Option<f64>, Option<f64>)>, String> {
    let mut errors = Vec::new();
    for url in QUOTE_URLS {
        for attempt in 0..HTTP_RETRY {
            sleep_retry(attempt).await;
            let result = apply_browser_headers(http.get(*url))
                .query(&[
                    ("fltt", "2"),
                    ("invt", "2"),
                    ("fields", "f2,f3,f12,f14"),
                    ("secids", secids),
                ])
                .send()
                .await;

            let resp = match result {
                Ok(r) => r,
                Err(e) => {
                    let msg = format!("{url} 热股行情失败(尝试{}): {e}", attempt + 1);
                    log_warn("hot-quote", &msg);
                    errors.push(msg);
                    continue;
                }
            };

            let resp = match resp.error_for_status() {
                Ok(r) => r,
                Err(e) => {
                    let msg = format!("{url} 热股行情响应异常(尝试{}): {e}", attempt + 1);
                    log_warn("hot-quote", &msg);
                    errors.push(msg);
                    continue;
                }
            };

            let Ok(json) = resp.json::<serde_json::Value>().await else {
                let msg = format!("{url} 热股行情解析失败(尝试{})", attempt + 1);
                log_warn("hot-quote", &msg);
                errors.push(msg);
                continue;
            };

            let mut quote_map: HashMap<String, (String, Option<f64>, Option<f64>)> = HashMap::new();
            if let Some(items) = json.pointer("/data/diff").and_then(|v| v.as_array()) {
                for item in items {
                    let code = item
                        .get("f12")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let name = item
                        .get("f14")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let price =
                        parse_f64(item.get("f2").unwrap_or(&serde_json::Value::Null)).filter(|v| *v > 0.0);
                    let change_pct = parse_f64(item.get("f3").unwrap_or(&serde_json::Value::Null));
                    if !code.is_empty() {
                        quote_map.insert(code, (name, price, change_pct));
                    }
                }
            }

            if !quote_map.is_empty() {
                return Ok(quote_map);
            }

            let msg = format!("{url} 热股行情为空(尝试{})", attempt + 1);
            log_warn("hot-quote", &msg);
            errors.push(msg);
        }
    }

    Err(format!(
        "热股行情补全失败: {}",
        if errors.is_empty() {
            "无可用节点".into()
        } else {
            errors.join("；")
        }
    ))
}
