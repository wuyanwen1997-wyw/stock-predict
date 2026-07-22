//! 多源人气/热股榜。

use crate::ashare::client::{client, log_warn, parse_f64, sleep_retry, HTTP_RETRY};
use crate::ashare::quotes::fetch_hot_quote_map;
use crate::ashare::symbol::{infer_market, parse_market_from_sc, to_secid};
use crate::models::Stock;
use std::collections::HashMap;

const HOT_EM_URL: &str = "https://emappdata.eastmoney.com/stockrank/getAllCurrentList";
const HOT_EM_SURGE_URL: &str = "https://emappdata.eastmoney.com/stockrank/getAllHisRcList";
const HOT_THS_URL: &str = "https://dq.10jqka.com.cn/fuyao/hot_list_data/out/hot_list/v1/stock";
const HOT_EM_GLOBAL_ID: &str = "786e4c21-70dc-435a-93bb-38";
const HOT_RRF_K: f64 = 60.0;

/// 单源热榜条目（不含完整行情）
#[derive(Clone, Debug)]
struct HotRankItem {
    market: String,
    code: String,
    name: Option<String>,
    change_pct: Option<f64>,
}

fn ths_market_to_str(market: i64, code: &str) -> String {
    match market {
        17 => "SH".into(),
        33 => "SZ".into(),
        _ => infer_market(code).into(),
    }
}

fn hot_em_payload(page_size: usize) -> serde_json::Value {
    serde_json::json!({
        "appId": "appId01",
        "globalId": HOT_EM_GLOBAL_ID,
        "marketType": "",
        "pageNo": 1,
        "pageSize": page_size,
    })
}

fn apply_hot_em_headers(builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    builder
        .header("Accept", "application/json, text/plain, */*")
        .header("Accept-Language", "zh-CN,zh;q=0.9")
        .header("Content-Type", "application/json")
        .header("Origin", "https://guba.eastmoney.com")
        .header("Referer", "https://guba.eastmoney.com/rank/")
}

fn apply_hot_ths_headers(builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    builder
        .header("Accept", "application/json, text/plain, */*")
        .header("Accept-Language", "zh-CN,zh;q=0.9")
        .header("Referer", "https://eq.10jqka.com.cn/")
}

async fn fetch_hot_rank_eastmoney(
    http: &reqwest::Client,
    limit: usize,
) -> Result<Vec<HotRankItem>, String> {
    let mut last_err = String::new();
    for attempt in 0..HTTP_RETRY {
        sleep_retry(attempt).await;
        let result = apply_hot_em_headers(http.post(HOT_EM_URL))
            .json(&hot_em_payload(limit))
            .send()
            .await;

        let resp = match result {
            Ok(r) => r,
            Err(e) => {
                last_err = format!("东财人气榜请求失败(尝试{}): {e}", attempt + 1);
                log_warn("hot-em", &last_err);
                continue;
            }
        };

        let resp = match resp.error_for_status() {
            Ok(r) => r,
            Err(e) => {
                last_err = format!("东财人气榜响应异常(尝试{}): {e}", attempt + 1);
                log_warn("hot-em", &last_err);
                continue;
            }
        };

        let resp = match resp.json::<serde_json::Value>().await {
            Ok(v) => v,
            Err(e) => {
                last_err = format!("东财人气榜解析失败(尝试{}): {e}", attempt + 1);
                log_warn("hot-em", &last_err);
                continue;
            }
        };

        let items = resp
            .pointer("/data")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut out = Vec::with_capacity(items.len());
        for item in items {
            let sc = item.get("sc").and_then(|v| v.as_str()).unwrap_or_default();
            if sc.is_empty() {
                continue;
            }
            let (market, code) = parse_market_from_sc(sc);
            out.push(HotRankItem {
                market,
                code,
                name: None,
                change_pct: None,
            });
        }
        return Ok(out);
    }

    Err(if last_err.is_empty() {
        "东财人气榜请求失败".into()
    } else {
        last_err
    })
}

async fn fetch_hot_rank_eastmoney_surge(
    http: &reqwest::Client,
    limit: usize,
) -> Result<Vec<HotRankItem>, String> {
    let mut last_err = String::new();
    for attempt in 0..HTTP_RETRY {
        sleep_retry(attempt).await;
        let result = apply_hot_em_headers(http.post(HOT_EM_SURGE_URL))
            .json(&hot_em_payload(limit))
            .send()
            .await;

        let resp = match result {
            Ok(r) => r,
            Err(e) => {
                last_err = format!("东财飙升榜请求失败(尝试{}): {e}", attempt + 1);
                log_warn("hot-em-surge", &last_err);
                continue;
            }
        };

        let resp = match resp.error_for_status() {
            Ok(r) => r,
            Err(e) => {
                last_err = format!("东财飙升榜响应异常(尝试{}): {e}", attempt + 1);
                log_warn("hot-em-surge", &last_err);
                continue;
            }
        };

        let resp = match resp.json::<serde_json::Value>().await {
            Ok(v) => v,
            Err(e) => {
                last_err = format!("东财飙升榜解析失败(尝试{}): {e}", attempt + 1);
                log_warn("hot-em-surge", &last_err);
                continue;
            }
        };

        let items = resp
            .pointer("/data")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut rows: Vec<(i64, HotRankItem)> = Vec::with_capacity(items.len());
        for (idx, item) in items.into_iter().enumerate() {
            let sc = item.get("sc").and_then(|v| v.as_str()).unwrap_or_default();
            if sc.is_empty() {
                continue;
            }
            let (market, code) = parse_market_from_sc(sc);
            let rank = item
                .get("hrcrk")
                .and_then(|v| v.as_i64())
                .unwrap_or(idx as i64 + 1);
            rows.push((
                rank,
                HotRankItem {
                    market,
                    code,
                    name: None,
                    change_pct: None,
                },
            ));
        }
        rows.sort_by_key(|(rank, _)| *rank);
        return Ok(rows.into_iter().map(|(_, item)| item).take(limit).collect());
    }

    Err(if last_err.is_empty() {
        "东财飙升榜请求失败".into()
    } else {
        last_err
    })
}

async fn fetch_hot_rank_tonghuashun(
    http: &reqwest::Client,
    limit: usize,
) -> Result<Vec<HotRankItem>, String> {
    let mut last_err = String::new();
    for attempt in 0..HTTP_RETRY {
        sleep_retry(attempt).await;
        let result = apply_hot_ths_headers(http.get(HOT_THS_URL))
            .query(&[
                ("stock_type", "a"),
                ("type", "hour"),
                ("list_type", "normal"),
            ])
            .send()
            .await;

        let resp = match result {
            Ok(r) => r,
            Err(e) => {
                last_err = format!("同花顺热榜请求失败(尝试{}): {e}", attempt + 1);
                log_warn("hot-ths", &last_err);
                continue;
            }
        };

        let resp = match resp.error_for_status() {
            Ok(r) => r,
            Err(e) => {
                last_err = format!("同花顺热榜响应异常(尝试{}): {e}", attempt + 1);
                log_warn("hot-ths", &last_err);
                continue;
            }
        };

        let resp = match resp.json::<serde_json::Value>().await {
            Ok(v) => v,
            Err(e) => {
                last_err = format!("同花顺热榜解析失败(尝试{}): {e}", attempt + 1);
                log_warn("hot-ths", &last_err);
                continue;
            }
        };

        let status = resp
            .get("status_code")
            .and_then(|v| v.as_i64())
            .unwrap_or(-1);
        if status != 0 {
            last_err = format!("同花顺热榜业务失败: status_code={status}");
            log_warn("hot-ths", &last_err);
            continue;
        }

        let items = resp
            .pointer("/data/stock_list")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut out = Vec::with_capacity(items.len().min(limit));
        for item in items.into_iter().take(limit) {
            let code = item
                .get("code")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            if code.is_empty() {
                continue;
            }
            let mkt = item.get("market").and_then(|v| v.as_i64()).unwrap_or(-1);
            let market = ths_market_to_str(mkt, &code);
            let name = item
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            let change_pct = item.get("rise_and_fall").and_then(parse_f64);
            out.push(HotRankItem {
                market,
                code,
                name,
                change_pct,
            });
        }
        return Ok(out);
    }

    Err(if last_err.is_empty() {
        "同花顺热榜请求失败".into()
    } else {
        last_err
    })
}

fn merge_hot_ranks_rrf(sources: &[(f64, Vec<HotRankItem>)], limit: usize) -> Vec<HotRankItem> {
    let mut scores: HashMap<String, f64> = HashMap::new();
    let mut meta: HashMap<String, HotRankItem> = HashMap::new();

    for (weight, list) in sources {
        if *weight <= 0.0 || list.is_empty() {
            continue;
        }
        for (idx, item) in list.iter().enumerate() {
            let key = format!("{}:{}", item.market, item.code);
            let contrib = *weight / (HOT_RRF_K + idx as f64 + 1.0);
            *scores.entry(key.clone()).or_insert(0.0) += contrib;
            meta.entry(key)
                .and_modify(|existing| {
                    if existing.name.is_none() {
                        existing.name = item.name.clone();
                    }
                    if existing.change_pct.is_none() {
                        existing.change_pct = item.change_pct;
                    }
                })
                .or_insert_with(|| item.clone());
        }
    }

    let mut ranked: Vec<(String, f64)> = scores.into_iter().collect();
    ranked.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });

    ranked
        .into_iter()
        .filter_map(|(key, _)| meta.remove(&key))
        .take(limit)
        .collect()
}

/// 联网拉取多源人气榜并融合，再补实时行情。
pub async fn fetch_hot_stocks(limit: usize) -> Result<Vec<Stock>, String> {
    let limit = limit.max(1);
    let per_source = (limit * 2).clamp(20, 100);
    let http = client()?;

    let (em_pop, ths, em_surge) = tokio::join!(
        fetch_hot_rank_eastmoney(&http, per_source),
        fetch_hot_rank_tonghuashun(&http, per_source),
        fetch_hot_rank_eastmoney_surge(&http, per_source),
    );

    let mut sources: Vec<(f64, Vec<HotRankItem>)> = Vec::with_capacity(3);
    let mut warnings = Vec::new();

    match em_pop {
        Ok(list) if !list.is_empty() => {
            log_warn("hot", &format!("东财人气榜 OK: {} 条", list.len()));
            sources.push((1.0, list));
        }
        Ok(_) => {
            let msg = "东财人气榜为空".to_string();
            log_warn("hot", &msg);
            warnings.push(msg);
        }
        Err(e) => {
            log_warn("hot", &e);
            warnings.push(e);
        }
    }
    match ths {
        Ok(list) if !list.is_empty() => {
            log_warn("hot", &format!("同花顺热榜 OK: {} 条", list.len()));
            sources.push((1.0, list));
        }
        Ok(_) => {
            let msg = "同花顺热榜为空".to_string();
            log_warn("hot", &msg);
            warnings.push(msg);
        }
        Err(e) => {
            log_warn("hot", &e);
            warnings.push(e);
        }
    }
    match em_surge {
        Ok(list) if !list.is_empty() => {
            log_warn("hot", &format!("东财飙升榜 OK: {} 条", list.len()));
            sources.push((0.55, list));
        }
        Ok(_) => {}
        Err(e) => {
            log_warn("hot", &e);
            warnings.push(e);
        }
    }

    let ranked = merge_hot_ranks_rrf(&sources, limit);
    if ranked.is_empty() {
        let err = format!(
            "人气榜联网采集失败: {}",
            if warnings.is_empty() {
                "无可用数据源".into()
            } else {
                warnings.join("；")
            }
        );
        log_warn("hot", &err);
        return Err(err);
    }

    if !warnings.is_empty() {
        log_warn(
            "hot",
            &format!(
                "部分数据源失败，已用 {} 个源融合出 {} 条: {}",
                sources.len(),
                ranked.len(),
                warnings.join("；")
            ),
        );
    }

    let secids: Vec<String> = ranked
        .iter()
        .map(|item| to_secid(&item.market, &item.code))
        .collect();

    let quote_map = match fetch_hot_quote_map(&http, &secids.join(",")).await {
        Ok(map) => map,
        Err(e) => {
            log_warn("hot", &e);
            HashMap::new()
        }
    };

    let mut stocks = Vec::with_capacity(ranked.len());
    for item in ranked {
        let hint_name = item.name.clone().unwrap_or_else(|| item.code.clone());
        let hint_pct = item.change_pct;
        let (name, price, change_pct) = if let Some((n, p, c)) = quote_map.get(&item.code).cloned()
        {
            let name = if n.is_empty() { hint_name } else { n };
            let change_pct = c.or(hint_pct);
            (name, p, change_pct)
        } else {
            (hint_name, None, hint_pct)
        };

        stocks.push(Stock {
            code: item.code,
            name,
            market: item.market,
            sector: "人气榜".to_string(),
            price,
            change_pct,
            is_hot: true,
        });
    }

    Ok(stocks)
}

/// 多源人气榜 Top N 股票代码
pub async fn fetch_hot_stock_codes(limit: usize) -> Result<Vec<String>, String> {
    let hot = fetch_hot_stocks(limit).await?;
    Ok(hot.into_iter().map(|s| s.code).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ths_market_mapping() {
        assert_eq!(ths_market_to_str(17, "600519"), "SH");
        assert_eq!(ths_market_to_str(33, "000858"), "SZ");
        assert_eq!(ths_market_to_str(99, "600000"), "SH");
    }

    #[test]
    fn rrf_prefers_multi_source_hits() {
        let em = vec![
            HotRankItem {
                market: "SZ".into(),
                code: "001309".into(),
                name: None,
                change_pct: None,
            },
            HotRankItem {
                market: "SH".into(),
                code: "601991".into(),
                name: None,
                change_pct: None,
            },
        ];
        let ths = vec![
            HotRankItem {
                market: "SH".into(),
                code: "601991".into(),
                name: Some("大唐发电".into()),
                change_pct: Some(1.2),
            },
            HotRankItem {
                market: "SZ".into(),
                code: "002384".into(),
                name: None,
                change_pct: None,
            },
        ];
        let merged = merge_hot_ranks_rrf(&[(1.0, em), (1.0, ths)], 3);
        assert_eq!(merged[0].code, "601991");
        assert_eq!(merged[0].name.as_deref(), Some("大唐发电"));
        assert!(merged.iter().any(|s| s.code == "001309"));
        assert!(merged.iter().any(|s| s.code == "002384"));
    }
}
