use crate::models::{DailyBar, Stock, StockQuote};
use std::collections::HashMap;

/// 东财行情多节点：push2 在部分网络会直接断连，需 delay / 编号节点兜底
const QUOTE_URLS: &[&str] = &[
    "https://push2delay.eastmoney.com/api/qt/ulist.np/get",
    "https://push2.eastmoney.com/api/qt/ulist.np/get",
    "https://82.push2.eastmoney.com/api/qt/ulist.np/get",
    "https://push2delay.eastmoney.com/api/qt/ulist/get",
];
const KLINE_URL: &str = "https://push2his.eastmoney.com/api/qt/stock/kline/get";
const TENCENT_KLINE_URL: &str = "https://web.ifzq.gtimg.cn/appstock/app/fqkline/get";
const SINA_KLINE_URL: &str = "https://money.finance.sina.com.cn/quotes_service/api/json_v2.php/CN_MarketData.getKLineData";
/// 东方财富股吧人气榜（浏览/讨论热度）
const HOT_EM_URL: &str = "https://emappdata.eastmoney.com/stockrank/getAllCurrentList";
/// 东方财富股吧飙升榜（排名快速上升）
const HOT_EM_SURGE_URL: &str = "https://emappdata.eastmoney.com/stockrank/getAllHisRcList";
/// 同花顺小时热榜（搜索/关注热度）
const HOT_THS_URL: &str =
    "https://dq.10jqka.com.cn/fuyao/hot_list_data/out/hot_list/v1/stock";
const SEARCH_URL: &str = "https://searchapi.eastmoney.com/api/suggest/get";
const HOT_EM_GLOBAL_ID: &str = "786e4c21-70dc-435a-93bb-38";
/// Reciprocal Rank Fusion 常数（越大越平滑）
const HOT_RRF_K: f64 = 60.0;
const HTTP_RETRY: u32 = 2;
const HTTP_RETRY_DELAY_MS: u64 = 400;

const USER_AGENT_DESKTOP: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const USER_AGENT_MOBILE: &str = "Mozilla/5.0 (Linux; Android 13; Mobile) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36";

fn user_agent() -> &'static str {
    if cfg!(target_os = "android") || cfg!(target_os = "ios") {
        USER_AGENT_MOBILE
    } else {
        USER_AGENT_DESKTOP
    }
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .connect_timeout(std::time::Duration::from_secs(6))
        .pool_idle_timeout(std::time::Duration::from_secs(30))
        .user_agent(user_agent())
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))
}

fn apply_browser_headers(builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    builder
        .header("Accept", "application/json, text/plain, */*")
        .header("Accept-Language", "zh-CN,zh;q=0.9")
        .header("Referer", "https://quote.eastmoney.com/")
}

fn log_warn(scope: &str, msg: &str) {
    eprintln!("[stock-predict:{scope}] {msg}");
}

async fn sleep_retry(attempt: u32) {
    if attempt == 0 {
        return;
    }
    tokio::time::sleep(std::time::Duration::from_millis(
        HTTP_RETRY_DELAY_MS * u64::from(attempt),
    ))
    .await;
}

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

fn market_from_search_item(item: &serde_json::Value, code: &str) -> String {
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

fn sector_from_search_item(item: &serde_json::Value, name: &str) -> String {
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

fn parse_market_from_sc(sc: &str) -> (String, String) {
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

fn parse_f64(v: &serde_json::Value) -> Option<f64> {
    match v {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => {
            let t = s.trim();
            if t.is_empty() || t == "-" {
                None
            } else {
                t.parse().ok()
            }
        }
        _ => None,
    }
}

/// 批量拉取实时行情（多节点 + 重试）
pub async fn fetch_stock_quotes(stocks: &[Stock]) -> Result<HashMap<String, StockQuote>, String> {
    if stocks.is_empty() {
        return Ok(HashMap::new());
    }

    let secids: Vec<String> = stocks
        .iter()
        .map(|s| to_secid(&s.market, &s.code))
        .collect();

    let client = client()?;
    let resp = fetch_quote_response(&client, &secids.join(",")).await?;
    Ok(parse_quote_map(&resp))
}

async fn fetch_quote_response(
    client: &reqwest::Client,
    secids: &str,
) -> Result<serde_json::Value, String> {
    let mut errors = Vec::new();
    for url in QUOTE_URLS {
        for attempt in 0..HTTP_RETRY {
            sleep_retry(attempt).await;
            let result = apply_browser_headers(client.get(*url))
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

        let price = parse_f64(item.get("f2").unwrap_or(&serde_json::Value::Null))
            .filter(|v| *v > 0.0);
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

async fn fetch_hot_quote_map(
    client: &reqwest::Client,
    secids: &str,
) -> Result<HashMap<String, (String, Option<f64>, Option<f64>)>, String> {
    let mut errors = Vec::new();
    for url in QUOTE_URLS {
        for attempt in 0..HTTP_RETRY {
            sleep_retry(attempt).await;
            let result = apply_browser_headers(client.get(*url))
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
                    let price = parse_f64(item.get("f2").unwrap_or(&serde_json::Value::Null))
                        .filter(|v| *v > 0.0);
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

/// 拉取日线 K 线（最近 N 根）。优先腾讯/新浪（更快更稳），东财作兜底。
pub async fn fetch_daily_klines(stock: &Stock, limit: u32) -> Result<Vec<DailyBar>, String> {
    let mut errors = Vec::new();

    match fetch_tencent_klines(stock, limit).await {
        Ok(bars) if !bars.is_empty() => return Ok(bars),
        Ok(_) => errors.push("腾讯日线返回为空".into()),
        Err(e) => errors.push(e),
    }

    match fetch_sina_klines(stock, limit).await {
        Ok(bars) if !bars.is_empty() => return Ok(bars),
        Ok(_) => errors.push("新浪日线返回为空".into()),
        Err(e) => errors.push(e),
    }

    match fetch_em_klines(stock, limit).await {
        Ok(bars) if !bars.is_empty() => return Ok(bars),
        Ok(_) => errors.push("东方财富日线返回为空".into()),
        Err(e) => errors.push(e),
    }

    Err(format!("日线获取失败: {}", errors.join("；")))
}

async fn fetch_em_klines(stock: &Stock, limit: u32) -> Result<Vec<DailyBar>, String> {
    let client = client()?;
    let secid = to_secid(&stock.market, &stock.code);

    let resp = apply_browser_headers(client.get(KLINE_URL))
        .query(&[
            ("secid", secid.as_str()),
            ("fields1", "f1,f2,f3,f4,f5,f6"),
            ("fields2", "f51,f52,f53,f54,f55,f56,f57,f58,f59,f60,f61"),
            ("klt", "101"),
            ("fqt", "1"),
            ("beg", "0"),
            ("end", "20500101"),
            ("lmt", &limit.to_string()),
        ])
        .send()
        .await
        .map_err(|e| format!("东方财富日线请求失败: {e}"))?
        .error_for_status()
        .map_err(|e| format!("东方财富日线响应异常: {e}"))?
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("东方财富日线解析失败: {e}"))?;

    parse_em_klines(&resp)
}

async fn fetch_tencent_klines(stock: &Stock, limit: u32) -> Result<Vec<DailyBar>, String> {
    let client = client()?;
    let symbol = to_tencent_symbol(&stock.market, &stock.code);
    let param = format!("{symbol},day,,,{limit},qfq");

    let resp = apply_browser_headers(client.get(TENCENT_KLINE_URL))
        .header("Referer", "https://gu.qq.com/")
        .query(&[("param", param.as_str())])
        .send()
        .await
        .map_err(|e| format!("腾讯日线请求失败: {e}"))?
        .error_for_status()
        .map_err(|e| format!("腾讯日线响应异常: {e}"))?
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("腾讯日线解析失败: {e}"))?;

    parse_tencent_klines(&resp, &symbol, limit)
}

async fn fetch_sina_klines(stock: &Stock, limit: u32) -> Result<Vec<DailyBar>, String> {
    let client = client()?;
    let symbol = to_sina_symbol(&stock.market, &stock.code);

    let resp = apply_browser_headers(client.get(SINA_KLINE_URL))
        .header("Referer", "https://finance.sina.com.cn/")
        .query(&[
            ("symbol", symbol.as_str()),
            ("scale", "240"),
            ("ma", "no"),
            ("datalen", &limit.to_string()),
        ])
        .send()
        .await
        .map_err(|e| format!("新浪日线请求失败: {e}"))?
        .error_for_status()
        .map_err(|e| format!("新浪日线响应异常: {e}"))?
        .json::<Vec<serde_json::Value>>()
        .await
        .map_err(|e| format!("新浪日线解析失败: {e}"))?;

    parse_sina_klines(&resp)
}

fn parse_em_klines(resp: &serde_json::Value) -> Result<Vec<DailyBar>, String> {
    let klines = resp
        .pointer("/data/klines")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut bars = Vec::with_capacity(klines.len());
    for line in klines {
        let text = line.as_str().unwrap_or_default();
        let parts: Vec<&str> = text.split(',').collect();
        if parts.len() < 6 {
            continue;
        }

        bars.push(DailyBar {
            date: parts[0].to_string(),
            open: parts[1].parse().unwrap_or(0.0),
            close: parts[2].parse().unwrap_or(0.0),
            high: parts[3].parse().unwrap_or(0.0),
            low: parts[4].parse().unwrap_or(0.0),
            volume: parts[5].parse().unwrap_or(0.0),
            change_pct: parts.get(8).and_then(|s| s.parse().ok()),
        });
    }

    Ok(bars)
}

fn parse_tencent_klines(
    resp: &serde_json::Value,
    symbol: &str,
    limit: u32,
) -> Result<Vec<DailyBar>, String> {
    let key = format!("/data/{symbol}/qfqday");
    let rows = resp
        .pointer(&key)
        .or_else(|| resp.pointer("/data").and_then(|d| d.get(symbol)).and_then(|s| s.get("qfqday")))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut bars = Vec::with_capacity(rows.len());
    for row in rows {
        let parts = row.as_array().cloned().unwrap_or_default();
        if parts.len() < 6 {
            continue;
        }
        let date = parts[0].as_str().unwrap_or_default();
        bars.push(DailyBar {
            date: date.to_string(),
            open: parse_json_f64(&parts[1]),
            close: parse_json_f64(&parts[2]),
            high: parse_json_f64(&parts[3]),
            low: parse_json_f64(&parts[4]),
            volume: parse_json_f64(&parts[5]),
            change_pct: None,
        });
    }

    if bars.len() > limit as usize {
        bars = bars.split_off(bars.len() - limit as usize);
    }

    if bars.is_empty() {
        return Err("腾讯日线返回为空".into());
    }

    Ok(bars)
}

fn parse_sina_klines(items: &[serde_json::Value]) -> Result<Vec<DailyBar>, String> {
    let mut bars = Vec::with_capacity(items.len());
    for item in items {
        let date = item.get("day").and_then(|v| v.as_str()).unwrap_or_default();
        if date.is_empty() {
            continue;
        }
        bars.push(DailyBar {
            date: date.to_string(),
            open: item.get("open").and_then(parse_f64).unwrap_or(0.0),
            close: item.get("close").and_then(parse_f64).unwrap_or(0.0),
            high: item.get("high").and_then(parse_f64).unwrap_or(0.0),
            low: item.get("low").and_then(parse_f64).unwrap_or(0.0),
            volume: item.get("volume").and_then(parse_f64).unwrap_or(0.0),
            change_pct: None,
        });
    }

    if bars.is_empty() {
        return Err("新浪日线返回为空".into());
    }

    Ok(bars)
}

fn parse_json_f64(v: &serde_json::Value) -> f64 {
    parse_f64(v).unwrap_or(0.0)
}

/// 单源热榜条目（不含完整行情）
#[derive(Clone, Debug)]
struct HotRankItem {
    market: String,
    code: String,
    name: Option<String>,
    change_pct: Option<f64>,
}

fn ths_market_to_str(market: i64, code: &str) -> String {
    // 同花顺：17=沪市，33=深市；其余按代码推断
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

/// 东方财富-股吧人气榜
async fn fetch_hot_rank_eastmoney(
    client: &reqwest::Client,
    limit: usize,
) -> Result<Vec<HotRankItem>, String> {
    let mut last_err = String::new();
    for attempt in 0..HTTP_RETRY {
        sleep_retry(attempt).await;
        let result = apply_hot_em_headers(client.post(HOT_EM_URL))
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

/// 东方财富-股吧飙升榜（排名快速上升，补充「正在升温」标的）
async fn fetch_hot_rank_eastmoney_surge(
    client: &reqwest::Client,
    limit: usize,
) -> Result<Vec<HotRankItem>, String> {
    let mut last_err = String::new();
    for attempt in 0..HTTP_RETRY {
        sleep_retry(attempt).await;
        let result = apply_hot_em_headers(client.post(HOT_EM_SURGE_URL))
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

/// 同花顺-A股小时热榜
async fn fetch_hot_rank_tonghuashun(
    client: &reqwest::Client,
    limit: usize,
) -> Result<Vec<HotRankItem>, String> {
    let mut last_err = String::new();
    for attempt in 0..HTTP_RETRY {
        sleep_retry(attempt).await;
        let result = apply_hot_ths_headers(client.get(HOT_THS_URL))
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
            let change_pct = item
                .get("rise_and_fall")
                .and_then(parse_f64);
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

/// 多源 Reciprocal Rank Fusion：同时出现在多个榜单的股票优先
fn merge_hot_ranks_rrf(
    sources: &[(f64, Vec<HotRankItem>)],
    limit: usize,
) -> Vec<HotRankItem> {
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

/// 联网拉取多源人气榜并融合（东财人气 + 同花顺热榜 + 东财飙升），再补实时行情。
/// 榜单成功但行情失败时仍返回列表（名称/涨跌幅尽量用同花顺兜底），保证页面可用。
pub async fn fetch_hot_stocks(limit: usize) -> Result<Vec<Stock>, String> {
    let limit = limit.max(1);
    // 各源多取一些，融合后再截断，提高交叉命中质量
    let per_source = (limit * 2).clamp(20, 100);
    let client = client()?;

    let (em_pop, ths, em_surge) = tokio::join!(
        fetch_hot_rank_eastmoney(&client, per_source),
        fetch_hot_rank_tonghuashun(&client, per_source),
        fetch_hot_rank_eastmoney_surge(&client, per_source),
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
    // 飙升榜权重略低：反映「升温」而非绝对人气
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

    let quote_map = match fetch_hot_quote_map(&client, &secids.join(",")).await {
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

/// 按名称/代码搜索 A 股
pub async fn search_stocks(query: &str, limit: usize) -> Result<Vec<Stock>, String> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(vec![]);
    }

    let client = client()?;
    let resp = client
        .get(SEARCH_URL)
        .query(&[
            ("input", q),
            ("type", "14"),
            ("token", "D43BF722user"),
            ("count", &limit.to_string()),
        ])
        .send()
        .await
        .map_err(|e| format!("搜索请求失败: {e}"))?
        .error_for_status()
        .map_err(|e| format!("搜索响应异常: {e}"))?
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("搜索解析失败: {e}"))?;

    let mut stocks = Vec::new();
    if let Some(items) = resp
        .pointer("/QuotationCodeTable/Data")
        .and_then(|v| v.as_array())
    {
        for item in items {
            let code = item
                .get("Code")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let name = item
                .get("Name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            if code.is_empty() || name.is_empty() {
                continue;
            }

            let market = market_from_search_item(item, &code);
            let sector = sector_from_search_item(item, &name);

            stocks.push(Stock {
                code,
                name,
                market,
                sector,
                price: None,
                change_pct: None,
                is_hot: false,
            });
        }
    }

    if !stocks.is_empty() {
        let quotes = fetch_stock_quotes(&stocks).await.unwrap_or_default();
        for stock in &mut stocks {
            if let Some(q) = quotes.get(&stock.code) {
                stock.price = q.price;
                stock.change_pct = q.change_pct;
            }
        }
    }

    Ok(stocks)
}

/// 根据日线收盘价计算历史波动率（日收益率标准差）
pub fn calc_volatility(bars: &[DailyBar]) -> f64 {
    if bars.len() < 5 {
        return 0.02;
    }

    let mut returns = Vec::new();
    for w in bars.windows(2) {
        let prev = w[0].close;
        let curr = w[1].close;
        if prev > 0.0 && curr > 0.0 {
            returns.push((curr - prev) / prev);
        }
    }

    if returns.is_empty() {
        return 0.02;
    }

    let mean = returns.iter().sum::<f64>() / returns.len() as f64;
    let variance = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / returns.len() as f64;
    variance.sqrt().clamp(0.005, 0.08)
}

pub fn apply_quote(stock: &mut Stock, quote: &StockQuote) {
    stock.price = quote.price;
    stock.change_pct = quote.change_pct;
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
        assert_eq!(parse_market_from_sc("SH600519"), ("SH".into(), "600519".into()));
        assert_eq!(parse_market_from_sc("SZ000858"), ("SZ".into(), "000858".into()));
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
