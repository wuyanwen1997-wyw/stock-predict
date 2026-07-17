//! 个股公告/资讯归档：东财公告 → 东财资讯搜索 → 巨潮（HTTPS）。

use crate::models::Stock;
use chrono::{Duration, NaiveDate, TimeZone, Utc};
use serde::Deserialize;
use std::sync::OnceLock;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct Announcement {
    pub date: NaiveDate,
    pub title: String,
}

#[derive(Debug, Clone, Default)]
pub struct MessageArchive {
    items: Vec<Announcement>,
}

impl MessageArchive {
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn titles_as_of(&self, as_of: NaiveDate, lookback_days: i64) -> Vec<String> {
        self.items_as_of(as_of, lookback_days)
            .into_iter()
            .map(|(_, t)| t)
            .collect()
    }

    /// (日期, 标题)，按日从新到旧
    pub fn items_as_of(&self, as_of: NaiveDate, lookback_days: i64) -> Vec<(NaiveDate, String)> {
        let start = as_of - Duration::days(lookback_days.max(1));
        let mut items: Vec<_> = self
            .items
            .iter()
            .filter(|a| a.date >= start && a.date <= as_of)
            .map(|a| (a.date, a.title.clone()))
            .collect();
        items.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        items
    }
}

static ORGID_CACHE: OnceLock<Mutex<std::collections::HashMap<String, String>>> = OnceLock::new();

fn orgid_cache() -> &'static Mutex<std::collections::HashMap<String, String>> {
    ORGID_CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

const UA_DESKTOP: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const UA_MOBILE: &str = "Mozilla/5.0 (Linux; Android 13; Mobile) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36";

fn user_agent() -> &'static str {
    if cfg!(target_os = "android") || cfg!(target_os = "ios") {
        UA_MOBILE
    } else {
        UA_DESKTOP
    }
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .connect_timeout(std::time::Duration::from_secs(5))
        .user_agent(user_agent())
        // 避免 http→https 302 把 POST 变成 GET 导致空响应
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))
}

/// 只保留 6 位数字代码
fn normalize_code(code: &str) -> String {
    let digits: String = code.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() >= 6 {
        digits[digits.len() - 6..].to_string()
    } else {
        format!("{:0>6}", digits)
    }
}

fn market_column_plate(market: &str) -> (&'static str, &'static str) {
    match market {
        "SZ" | "sz" => ("szse", "sz"),
        _ => ("sse", "sh"),
    }
}

fn heuristic_org_id(market: &str, code: &str) -> String {
    let padded = format!("{:0>7}", code.trim());
    match market {
        "SZ" | "sz" => format!("gssz{padded}"),
        _ => format!("gssh{padded}"),
    }
}

#[derive(Debug, Deserialize)]
struct TopSearchItem {
    code: Option<String>,
    #[serde(rename = "orgId")]
    org_id: Option<String>,
}

async fn resolve_org_id(client: &reqwest::Client, stock: &Stock) -> String {
    let code = normalize_code(&stock.code);
    {
        let cache = orgid_cache().lock().await;
        if let Some(id) = cache.get(&code) {
            return id.clone();
        }
    }

    let mut org_id = heuristic_org_id(&stock.market, &code);
    let body = format!("keyWord={}&maxNum=8", urlencoding_simple(&code));
    if let Ok(resp) = client
        .post("https://www.cninfo.com.cn/new/information/topSearch/query")
        .header("Content-Type", "application/x-www-form-urlencoded; charset=UTF-8")
        .header("Referer", "https://www.cninfo.com.cn/")
        .header("Origin", "https://www.cninfo.com.cn")
        .header("Accept", "application/json, text/javascript, */*; q=0.01")
        .header("X-Requested-With", "XMLHttpRequest")
        .body(body)
        .send()
        .await
    {
        if let Ok(text) = resp.text().await {
            if let Ok(items) = serde_json::from_str::<Vec<TopSearchItem>>(&text) {
                if let Some(hit) = items.iter().find(|i| {
                    i.code
                        .as_deref()
                        .map(|c| normalize_code(c) == code)
                        .unwrap_or(false)
                }) {
                    if let Some(id) = hit.org_id.as_deref().filter(|s| !s.is_empty()) {
                        org_id = id.to_string();
                    }
                } else if let Some(id) = items
                    .first()
                    .and_then(|i| i.org_id.as_deref())
                    .filter(|s| !s.is_empty())
                {
                    org_id = id.to_string();
                }
            }
        }
    }

    let mut cache = orgid_cache().lock().await;
    cache.insert(code, org_id.clone());
    org_id
}

fn urlencoding_simple(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn ms_to_date(ms: i64) -> Option<NaiveDate> {
    Utc.timestamp_millis_opt(ms)
        .single()
        .map(|dt| dt.date_naive())
}

pub fn parse_flexible_date(s: &str) -> Option<NaiveDate> {
    let t = s.trim();
    if t.len() >= 10 {
        let head = &t[..10];
        if let Ok(d) = NaiveDate::parse_from_str(head, "%Y-%m-%d") {
            return Some(d);
        }
        if let Ok(d) = NaiveDate::parse_from_str(head, "%Y/%m/%d") {
            return Some(d);
        }
    }
    if t.len() >= 8 {
        let digits: String = t.chars().filter(|c| c.is_ascii_digit()).take(8).collect();
        if digits.len() == 8 {
            return NaiveDate::parse_from_str(&digits, "%Y%m%d").ok();
        }
    }
    None
}

fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("\"", "")
        .trim()
        .to_string()
}

fn extract_json_body(text: &str) -> &str {
    let t = text.trim();
    if let Some(start) = t.find('{') {
        if let Some(end) = t.rfind('}') {
            return &t[start..=end];
        }
    }
    // JSONP: cb({...})
    if let Some(l) = t.find('(') {
        if let Some(r) = t.rfind(')') {
            if r > l {
                let inner = &t[l + 1..r];
                if let Some(start) = inner.find('{') {
                    if let Some(end) = inner.rfind('}') {
                        return &inner[start..=end];
                    }
                }
            }
        }
    }
    t
}

fn preview(text: &str) -> String {
    let t = text.trim().replace('\n', " ");
    if t.is_empty() {
        "(空响应)".into()
    } else {
        t.chars().take(80).collect()
    }
}

async fn fetch_cninfo(
    client: &reqwest::Client,
    stock: &Stock,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Vec<Announcement>, String> {
    let code = normalize_code(&stock.code);
    let org_id = resolve_org_id(client, stock).await;
    let (column, plate) = market_column_plate(&stock.market);
    let se_date = format!("{start}~{end}");
    let stock_param = format!("{code},{org_id}");

    let mut out = Vec::new();
    let mut page = 1u32;
    const PAGE_SIZE: u32 = 30;
    const MAX_PAGES: u32 = 8;

    while page <= MAX_PAGES {
        let body = format!(
            "pageNum={page}&pageSize={PAGE_SIZE}&column={column}&tabName=fulltext&plate={plate}&stock={}&searchkey=&secid=&category=&trade=&seDate={}&isHLtitle=true",
            urlencoding_simple(&stock_param),
            urlencoding_simple(&se_date),
        );

        // 必须用 HTTPS：HTTP 会 302，POST 变 GET，返回空/HTML
        let resp = client
            .post("https://www.cninfo.com.cn/new/hisAnnouncement/query")
            .header(
                "Content-Type",
                "application/x-www-form-urlencoded; charset=UTF-8",
            )
            .header(
                "Referer",
                "https://www.cninfo.com.cn/new/commonUrl?url=disclosure/list/notice",
            )
            .header("Origin", "https://www.cninfo.com.cn")
            .header("Accept", "application/json, text/javascript, */*; q=0.01")
            .header("X-Requested-With", "XMLHttpRequest")
            .body(body)
            .send()
            .await
            .map_err(|e| format!("巨潮请求失败: {e}"))?;

        let status = resp.status();
        let text = resp.text().await.map_err(|e| format!("巨潮读体失败: {e}"))?;
        if !status.is_success() {
            return Err(format!("巨潮 HTTP {status}: {}", preview(&text)));
        }
        if text.trim().is_empty() {
            return Err("巨潮返回空响应".into());
        }
        if text.trim_start().starts_with('<') {
            return Err(format!("巨潮返回 HTML: {}", preview(&text)));
        }

        let json_text = extract_json_body(&text);
        let v: serde_json::Value = serde_json::from_str(json_text)
            .map_err(|e| format!("巨潮 JSON 解析失败: {e} · {}", preview(&text)))?;

        let arr = v
            .get("announcements")
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default();
        if arr.is_empty() {
            break;
        }

        let n = arr.len();
        for item in arr {
            let title = item
                .get("announcementTitle")
                .or_else(|| item.get("shortTitle"))
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if title.is_empty() {
                continue;
            }
            let date = item
                .get("announcementTime")
                .and_then(|x| x.as_i64())
                .and_then(ms_to_date);
            let Some(date) = date else { continue };
            if date < start || date > end {
                continue;
            }
            out.push(Announcement { date, title });
        }

        let has_more = v.get("hasMore").and_then(|x| x.as_bool()).unwrap_or(false);
        if !has_more || n < PAGE_SIZE as usize {
            break;
        }
        page += 1;
    }

    if out.is_empty() {
        return Err("巨潮无公告".into());
    }
    Ok(out)
}

async fn fetch_eastmoney_ann(
    client: &reqwest::Client,
    stock: &Stock,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Vec<Announcement>, String> {
    let code = normalize_code(&stock.code);
    let mut out = Vec::new();
    let mut page = 1u32;
    const PAGE_SIZE: u32 = 50;
    const MAX_PAGES: u32 = 10;
    let mut last_error = String::new();
    let mut saw_any_item = false;

    while page <= MAX_PAGES {
        let page_s = page.to_string();
        let size_s = PAGE_SIZE.to_string();
        // 带 cb 的 JSONP 在部分移动网络更稳
        let cb = format!("jQuery{page}");
        let referer = format!("https://data.eastmoney.com/notices/stock/{code}.html");
        let resp = match client
            .get("https://np-anotice-stock.eastmoney.com/api/security/ann")
            .query(&[
                ("cb", cb.as_str()),
                ("sr", "-1"),
                ("page_size", size_s.as_str()),
                ("page_index", page_s.as_str()),
                ("ann_type", "A"),
                ("client_source", "web"),
                ("stock_list", code.as_str()),
                ("f_node", "0"),
                ("s_node", "0"),
            ])
            .header("Referer", &referer)
            .header("Accept", "*/*")
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                last_error = format!("东财公告请求失败: {e}");
                break;
            }
        };

        let status = resp.status();
        let text = match resp.text().await {
            Ok(t) => t,
            Err(e) => {
                last_error = format!("东财公告读体失败: {e}");
                break;
            }
        };
        if !status.is_success() {
            last_error = format!("东财公告 HTTP {status}: {}", preview(&text));
            break;
        }

        let json_text = extract_json_body(&text);
        let v: serde_json::Value = match serde_json::from_str(json_text) {
            Ok(v) => v,
            Err(e) => {
                last_error = format!("东财公告 JSON 失败: {e} · {}", preview(&text));
                break;
            }
        };

        let arr = v
            .pointer("/data/list")
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default();
        if arr.is_empty() {
            if page == 1 {
                last_error = "东财公告列表为空".into();
            }
            break;
        }

        let n = arr.len();
        let mut oldest: Option<NaiveDate> = None;
        for item in arr {
            saw_any_item = true;
            let title = item
                .get("title_ch")
                .or_else(|| item.get("title"))
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if title.is_empty() {
                continue;
            }
            let date = item
                .get("notice_date")
                .and_then(|x| x.as_str())
                .and_then(parse_flexible_date)
                .or_else(|| {
                    item.get("eiTime")
                        .and_then(|x| x.as_str())
                        .and_then(parse_flexible_date)
                })
                .or_else(|| {
                    item.get("display_time")
                        .and_then(|x| x.as_str())
                        .and_then(parse_flexible_date)
                });
            let Some(date) = date else { continue };
            oldest = Some(oldest.map_or(date, |o| o.min(date)));
            if date >= start && date <= end {
                out.push(Announcement { date, title });
            }
        }

        if oldest.map(|d| d < start).unwrap_or(false) {
            break;
        }
        if n < PAGE_SIZE as usize {
            break;
        }
        page += 1;
    }

    if out.is_empty() {
        if !last_error.is_empty() {
            return Err(last_error);
        }
        if saw_any_item {
            return Err(format!("东财公告不在区间 {start}~{end}"));
        }
        return Err("东财公告为空".into());
    }
    Ok(out)
}

async fn fetch_eastmoney_search_keyword(
    client: &reqwest::Client,
    keyword: &str,
    start: NaiveDate,
    end: NaiveDate,
    max_pages: u32,
) -> Result<Vec<Announcement>, String> {
    let mut out = Vec::new();
    for page in 1u32..=max_pages {
        let param = serde_json::json!({
            "uid": "",
            "keyword": keyword,
            "type": ["cmsArticleWebOld"],
            "client": "web",
            "clientType": "web",
            "clientVersion": "curr",
            "param": {
                "cmsArticleWebOld": {
                    "searchScope": "default",
                    "sort": "default",
                    "pageIndex": page,
                    "pageSize": 20,
                }
            }
        });

        let resp = client
            .get("https://search-api-web.eastmoney.com/search/jsonp")
            .query(&[("cb", "jQuery"), ("param", &param.to_string())])
            .header("Referer", "https://so.eastmoney.com/")
            .header("Accept", "*/*")
            .send()
            .await
            .map_err(|e| format!("资讯搜索失败: {e}"))?;

        let text = resp
            .text()
            .await
            .map_err(|e| format!("资讯搜索读体失败: {e}"))?;
        let json_text = extract_json_body(&text);
        let v: serde_json::Value = serde_json::from_str(json_text)
            .map_err(|e| format!("资讯搜索 JSON 失败: {e} · {}", preview(&text)))?;

        let arr = v
            .pointer("/result/cmsArticleWebOld")
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default();
        if arr.is_empty() {
            break;
        }

        let n = arr.len();
        let mut oldest: Option<NaiveDate> = None;
        for item in arr {
            let title = item
                .get("title")
                .and_then(|x| x.as_str())
                .map(strip_html)
                .unwrap_or_default();
            if title.is_empty() {
                continue;
            }
            let date = item
                .get("date")
                .and_then(|x| x.as_str())
                .and_then(parse_flexible_date);
            let Some(date) = date else { continue };
            oldest = Some(oldest.map_or(date, |o| o.min(date)));
            if date >= start && date <= end {
                out.push(Announcement { date, title });
            }
        }

        if oldest.map(|d| d < start).unwrap_or(false) {
            break;
        }
        if n < 20 {
            break;
        }
    }
    Ok(out)
}

/// 东财资讯搜索（含日期）——按标的类型附加宏观/行业检索词
async fn fetch_eastmoney_search(
    client: &reqwest::Client,
    stock: &Stock,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Vec<Announcement>, String> {
    let keywords = crate::message_sentiment::search_queries(stock);
    let mut out = Vec::new();
    let mut errors = Vec::new();
    for (i, kw) in keywords.iter().enumerate() {
        let pages = if i == 0 { 4 } else { 2 };
        match fetch_eastmoney_search_keyword(client, kw, start, end, pages).await {
            Ok(mut items) => out.append(&mut items),
            Err(e) => errors.push(e),
        }
    }

    if out.is_empty() {
        return Err(if errors.is_empty() {
            "资讯搜索无结果".into()
        } else {
            errors.join("；")
        });
    }
    Ok(out)
}

fn dedupe_sort(mut items: Vec<Announcement>) -> Vec<Announcement> {
    items.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.title.cmp(&b.title)));
    items.dedup_by(|a, b| a.date == b.date && a.title == b.title);
    items
}

pub async fn fetch_archive(
    stock: &Stock,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<MessageArchive, String> {
    if end < start {
        return Ok(MessageArchive::default());
    }
    let client = http_client()?;

    let (ann, search, cn) = tokio::join!(
        fetch_eastmoney_ann(&client, stock, start, end),
        fetch_eastmoney_search(&client, stock, start, end),
        fetch_cninfo(&client, stock, start, end),
    );

    let mut errors = Vec::new();
    let mut pick = |r: Result<Vec<Announcement>, String>| -> Option<Vec<Announcement>> {
        match r {
            Ok(v) if !v.is_empty() => Some(v),
            Ok(_) => {
                errors.push("空列表".into());
                None
            }
            Err(e) => {
                errors.push(e);
                None
            }
        }
    };

    let items = pick(ann)
        .or_else(|| pick(search))
        .or_else(|| pick(cn))
        .ok_or_else(|| errors.join("；"))?;

    Ok(MessageArchive {
        items: dedupe_sort(items),
    })
}

pub async fn fetch_recent(stock: &Stock, recent_days: i64) -> Result<MessageArchive, String> {
    let end = chrono::Local::now().date_naive();
    let start = end - Duration::days(recent_days.max(1));
    fetch_archive(stock, start, end).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Stock;

    #[test]
    fn titles_as_of_filters_future() {
        let archive = MessageArchive {
            items: vec![
                Announcement {
                    date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                    title: "旧".into(),
                },
                Announcement {
                    date: NaiveDate::from_ymd_opt(2025, 1, 10).unwrap(),
                    title: "中".into(),
                },
                Announcement {
                    date: NaiveDate::from_ymd_opt(2025, 1, 20).unwrap(),
                    title: "未来".into(),
                },
            ],
        };
        let as_of = NaiveDate::from_ymd_opt(2025, 1, 12).unwrap();
        assert_eq!(archive.titles_as_of(as_of, 7), vec!["中".to_string()]);
    }

    #[test]
    fn normalize_code_strips_prefix() {
        assert_eq!(normalize_code("SH600519"), "600519");
        assert_eq!(normalize_code("600519"), "600519");
    }

    #[tokio::test]
    async fn fetch_maotai_archive_smoke() {
        let stock = Stock {
            code: "600519".into(),
            name: "贵州茅台".into(),
            market: "SH".into(),
            sector: "白酒".into(),
            price: None,
            change_pct: None,
            is_hot: false,
        };
        let start = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2025, 6, 30).unwrap();
        let archive = fetch_archive(&stock, start, end)
            .await
            .expect("should fetch announcements");
        assert!(archive.len() > 3, "got {}", archive.len());
    }
}
