use crate::models::{DailyBar, Stock, StockQuote};
use std::collections::HashMap;

const QUOTE_URL: &str = "https://push2.eastmoney.com/api/qt/ulist.np/get";
const KLINE_URL: &str = "https://push2his.eastmoney.com/api/qt/stock/kline/get";
const TENCENT_KLINE_URL: &str = "https://web.ifzq.gtimg.cn/appstock/app/fqkline/get";
const SINA_KLINE_URL: &str = "https://money.finance.sina.com.cn/quotes_service/api/json_v2.php/CN_MarketData.getKLineData";
const HOT_URL: &str = "https://emappdata.eastmoney.com/stockrank/getAllCurrentList";
const SEARCH_URL: &str = "https://searchapi.eastmoney.com/api/suggest/get";

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
        .timeout(std::time::Duration::from_secs(10))
        .connect_timeout(std::time::Duration::from_secs(5))
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

/// 批量拉取实时行情
pub async fn fetch_stock_quotes(stocks: &[Stock]) -> Result<HashMap<String, StockQuote>, String> {
    if stocks.is_empty() {
        return Ok(HashMap::new());
    }

    let secids: Vec<String> = stocks
        .iter()
        .map(|s| to_secid(&s.market, &s.code))
        .collect();

    let client = client()?;
    let resp = apply_browser_headers(client.get(QUOTE_URL))
        .query(&[
            ("fltt", "2"),
            ("invt", "2"),
            ("fields", "f2,f3,f4,f12,f14,f15,f16,f17,f18,f5,f6"),
            ("secids", &secids.join(",")),
        ])
        .send()
        .await
        .map_err(|e| format!("行情请求失败: {e}"))?
        .error_for_status()
        .map_err(|e| format!("行情响应异常: {e}"))?
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("行情解析失败: {e}"))?;

    let mut map = HashMap::new();
    if let Some(items) = resp
        .pointer("/data/diff")
        .and_then(|v| v.as_array())
    {
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
    }

    Ok(map)
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

/// 拉取人气榜热门股票（含名称与行情）
pub async fn fetch_hot_stocks(limit: usize) -> Result<Vec<Stock>, String> {
    let client = client()?;
    let body = serde_json::json!({
        "appId": "appId01",
        "globalId": "stockpredict",
        "pageNo": 1,
        "pageSize": limit,
    });

    let resp = client
        .post(HOT_URL)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("热门股请求失败: {e}"))?
        .error_for_status()
        .map_err(|e| format!("热门股响应异常: {e}"))?
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("热门股解析失败: {e}"))?;

    let ranked: Vec<(String, String)> = resp
        .pointer("/data")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    let sc = item.get("sc")?.as_str()?;
                    let (market, code) = parse_market_from_sc(sc);
                    Some((market, code))
                })
                .collect()
        })
        .unwrap_or_default();

    if ranked.is_empty() {
        return Ok(vec![]);
    }

    let secids: Vec<String> = ranked
        .iter()
        .map(|(market, code)| to_secid(market, code))
        .collect();

    let quote_resp = client
        .get(QUOTE_URL)
        .query(&[
            ("fltt", "2"),
            ("invt", "2"),
            ("fields", "f2,f3,f12,f14"),
            ("secids", &secids.join(",")),
        ])
        .send()
        .await
        .map_err(|e| format!("热门股行情请求失败: {e}"))?
        .error_for_status()
        .map_err(|e| format!("热门股行情响应异常: {e}"))?
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("热门股行情解析失败: {e}"))?;

    let mut quote_map: HashMap<String, (String, Option<f64>, Option<f64>)> = HashMap::new();
    if let Some(items) = quote_resp
        .pointer("/data/diff")
        .and_then(|v| v.as_array())
    {
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
            let price = parse_f64(item.get("f2").unwrap_or(&serde_json::Value::Null));
            let change_pct = parse_f64(item.get("f3").unwrap_or(&serde_json::Value::Null));
            if !code.is_empty() {
                quote_map.insert(code, (name, price, change_pct));
            }
        }
    }

    let mut stocks = Vec::with_capacity(ranked.len());
    for (market, code) in ranked {
        let (name, price, change_pct) = quote_map
            .get(&code)
            .cloned()
            .unwrap_or_else(|| (code.clone(), None, None));

        stocks.push(Stock {
            code,
            name,
            market,
            sector: "人气榜".to_string(),
            price,
            change_pct,
            is_hot: true,
        });
    }

    Ok(stocks)
}

/// 东方财富人气榜 Top N 股票代码
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
}
