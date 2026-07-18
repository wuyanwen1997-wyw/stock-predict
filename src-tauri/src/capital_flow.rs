//! 资金流：可回测。
//!
//! 数据优先级（按日评估）：
//! 1. 大盘主力净流入（Tushare / 东财，真·主力）
//! 2. **两市成交额代理**（腾讯上证+深成指成交，免费稳定，可做近期 walk-forward）
//! 3. 北向净买入（约 2024-08 前）
//!
//! 无 Tushare 时仍可用路径 2 做回测；配置 Token 后自动优先真主力。

use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration as StdDuration, Instant};
use tokio::sync::Mutex;

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const DATA_URL: &str = "https://datacenter-web.eastmoney.com/api/data/v1/get";
const TUSHARE_URL: &str = "https://api.tushare.pro";
const TENCENT_KLINE_URL: &str = "https://web.ifzq.gtimg.cn/appstock/app/fqkline/get";

#[derive(Debug, Clone, Default)]
pub struct CapitalFlowArchive {
    /// 日期 → 北向净买入（亿元）
    north_net_yi: BTreeMap<NaiveDate, f64>,
    /// 日期 → 大盘主力净流入（元）
    market_main: BTreeMap<NaiveDate, f64>,
    /// 日期 → 上证+深成指成交量/额之和（腾讯免费代理）
    activity_amount: BTreeMap<NaiveDate, f64>,
    /// 日期 → 上证收盘（用于成交代理的涨跌方向）
    activity_close: BTreeMap<NaiveDate, f64>,
    /// 数据来源说明（供回测摘要）
    pub source_note: String,
}

impl CapitalFlowArchive {
    pub fn north_days(&self) -> usize {
        self.north_net_yi.len()
    }

    pub fn market_days(&self) -> usize {
        self.market_main.len()
    }

    pub fn activity_days(&self) -> usize {
        self.activity_amount.len()
    }

    /// 可用于回测打分的交易日数
    pub fn usable_days(&self) -> usize {
        self.market_days()
            .max(self.activity_days())
            .max(self.north_days())
    }

    pub fn is_empty(&self) -> bool {
        self.usable_days() == 0
    }

    pub fn merge(&mut self, other: &CapitalFlowArchive) {
        for (d, v) in &other.north_net_yi {
            self.north_net_yi.entry(*d).or_insert(*v);
        }
        for (d, v) in &other.market_main {
            self.market_main.insert(*d, *v);
        }
        for (d, v) in &other.activity_amount {
            self.activity_amount.insert(*d, *v);
        }
        for (d, v) in &other.activity_close {
            self.activity_close.insert(*d, *v);
        }
    }
}

#[derive(Debug, Clone)]
pub struct CapitalFlowSignal {
    pub score: f64,
    pub note: String,
    pub status: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DiskCache {
    /// YYYY-MM-DD -> 主力净流入（元）
    market_main: BTreeMap<String, f64>,
    /// YYYY-MM-DD -> 北向净买入（亿元）
    north_net_yi: BTreeMap<String, f64>,
    /// YYYY-MM-DD -> 两市成交代理
    activity_amount: BTreeMap<String, f64>,
    activity_close: BTreeMap<String, f64>,
    updated_at: Option<String>,
}

struct CacheEntry {
    fetched_at: Instant,
    archive: CapitalFlowArchive,
}

static CACHE: OnceLock<Mutex<Option<CacheEntry>>> = OnceLock::new();

fn cache() -> &'static Mutex<Option<CacheEntry>> {
    CACHE.get_or_init(|| Mutex::new(None))
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(StdDuration::from_secs(8))
        .connect_timeout(StdDuration::from_secs(3))
        .user_agent(UA)
        .build()
        .map_err(|e| format!("HTTP 客户端失败: {e}"))
}

fn http_client_fast() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(StdDuration::from_secs(4))
        .connect_timeout(StdDuration::from_secs(2))
        .user_agent(UA)
        .build()
        .map_err(|e| format!("HTTP 客户端失败: {e}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchMode {
    /// 实盘/常规回测：只拉两市成交代理（+可选 Tushare），跳过北向翻页与东财慢接口
    Fast,
    /// 完整：含北向历史与东财主力（慢，仅必要时）
    Full,
}

fn data_dir() -> PathBuf {
    if let Ok(p) = std::env::var("STOCK_PREDICT_DATA") {
        return PathBuf::from(p);
    }
    #[cfg(windows)]
    {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            return PathBuf::from(local).join("stock-predict");
        }
    }
    #[cfg(not(windows))]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(".stock-predict");
        }
    }
    std::env::temp_dir().join("stock-predict")
}

fn token_path() -> PathBuf {
    data_dir().join("tushare_token.txt")
}

fn disk_cache_path() -> PathBuf {
    data_dir().join("market_fund_flow.json")
}

/// 读取 Tushare token：环境变量 > 本地配置文件 > resources/tushare_token.txt
pub fn resolve_tushare_token() -> Option<String> {
    if let Ok(t) = std::env::var("TUSHARE_TOKEN") {
        let t = t.trim().to_string();
        if !t.is_empty() {
            return Some(t);
        }
    }
    for path in [token_path(), PathBuf::from("resources/tushare_token.txt")] {
        if let Ok(s) = std::fs::read_to_string(&path) {
            let t = s.trim().to_string();
            if !t.is_empty() && !t.starts_with('#') {
                return Some(t);
            }
        }
    }
    None
}

pub fn tushare_token_configured() -> bool {
    resolve_tushare_token().is_some()
}

pub fn save_tushare_token(token: &str) -> Result<(), String> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建数据目录失败: {e}"))?;
    let t = token.trim();
    if t.is_empty() {
        let _ = std::fs::remove_file(token_path());
        return Ok(());
    }
    std::fs::write(token_path(), t).map_err(|e| format!("保存 token 失败: {e}"))
}

fn parse_trade_date(raw: &str) -> Option<NaiveDate> {
    let s = raw.trim();
    if s.len() == 8 && s.chars().all(|c| c.is_ascii_digit()) {
        return NaiveDate::parse_from_str(s, "%Y%m%d").ok();
    }
    let head = if s.len() >= 10 { &s[..10] } else { s };
    NaiveDate::parse_from_str(head, "%Y-%m-%d").ok()
}

fn load_disk_cache() -> CapitalFlowArchive {
    let mut archive = CapitalFlowArchive::default();
    for path in [
        PathBuf::from("resources/market_fund_flow.json"),
        disk_cache_path(),
    ] {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(disk) = serde_json::from_str::<DiskCache>(&text) else {
            continue;
        };
        for (d, v) in disk.market_main {
            if let Some(day) = parse_trade_date(&d) {
                archive.market_main.entry(day).or_insert(v);
            }
        }
        for (d, v) in disk.north_net_yi {
            if let Some(day) = parse_trade_date(&d) {
                archive.north_net_yi.entry(day).or_insert(v);
            }
        }
        for (d, v) in disk.activity_amount {
            if let Some(day) = parse_trade_date(&d) {
                archive.activity_amount.entry(day).or_insert(v);
            }
        }
        for (d, v) in disk.activity_close {
            if let Some(day) = parse_trade_date(&d) {
                archive.activity_close.entry(day).or_insert(v);
            }
        }
    }
    archive
}

fn save_disk_cache(archive: &CapitalFlowArchive) {
    let dir = data_dir();
    let _ = std::fs::create_dir_all(&dir);
    let mut disk = DiskCache {
        updated_at: Some(chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()),
        ..Default::default()
    };
    for (d, v) in &archive.market_main {
        disk.market_main
            .insert(d.format("%Y-%m-%d").to_string(), *v);
    }
    for (d, v) in &archive.north_net_yi {
        disk.north_net_yi
            .insert(d.format("%Y-%m-%d").to_string(), *v);
    }
    for (d, v) in &archive.activity_amount {
        disk.activity_amount
            .insert(d.format("%Y-%m-%d").to_string(), *v);
    }
    for (d, v) in &archive.activity_close {
        disk.activity_close
            .insert(d.format("%Y-%m-%d").to_string(), *v);
    }
    if let Ok(text) = serde_json::to_string_pretty(&disk) {
        let _ = std::fs::write(disk_cache_path(), text);
    }
}

#[derive(Debug, Deserialize)]
struct MutualPage {
    result: Option<MutualResult>,
}

#[derive(Debug, Deserialize)]
struct MutualResult {
    pages: Option<i64>,
    data: Option<Vec<MutualRow>>,
}

#[derive(Debug, Deserialize)]
struct MutualRow {
    #[serde(rename = "TRADE_DATE")]
    trade_date: Option<String>,
    #[serde(rename = "NET_DEAL_AMT")]
    net_deal_amt: Option<f64>,
}

async fn fetch_mutual_type(
    client: &reqwest::Client,
    mutual_type: &str,
    max_pages: usize,
) -> Result<BTreeMap<NaiveDate, f64>, String> {
    let mut out = BTreeMap::new();
    let mut page = 1usize;
    let mut total_pages = max_pages.max(1);
    while page <= total_pages {
        let resp = client
            .get(DATA_URL)
            .query(&[
                ("sortColumns", "TRADE_DATE"),
                ("sortTypes", "-1"),
                ("pageSize", "100"),
                ("pageNumber", &page.to_string()),
                ("reportName", "RPT_MUTUAL_DEAL_HISTORY"),
                ("columns", "ALL"),
                ("source", "WEB"),
                ("client", "WEB"),
                ("filter", &format!(r#"(MUTUAL_TYPE="{mutual_type}")"#)),
            ])
            .header("Referer", "https://data.eastmoney.com/hsgt/index.html")
            .send()
            .await
            .map_err(|e| format!("北向接口请求失败: {e}"))?;
        let MutualPage { result } = resp
            .json()
            .await
            .map_err(|e| format!("北向接口解析失败: {e}"))?;
        if page == 1 {
            if let Some(p) = result.as_ref().and_then(|r| r.pages) {
                total_pages = (p as usize).clamp(1, max_pages);
            }
        }
        let rows = result.and_then(|r| r.data).unwrap_or_default();
        if rows.is_empty() {
            break;
        }
        for row in &rows {
            let Some(date) = row.trade_date.as_deref().and_then(parse_trade_date) else {
                continue;
            };
            if let Some(net) = row.net_deal_amt {
                out.insert(date, net);
            }
        }
        page += 1;
    }
    Ok(out)
}

async fn fetch_north_net(client: &reqwest::Client, max_pages: usize) -> BTreeMap<NaiveDate, f64> {
    // 仅拉「北向合计」005，避免再翻沪/深各十几页
    fetch_mutual_type(client, "005", max_pages.max(1).min(10))
        .await
        .unwrap_or_default()
}

/// Tushare 大盘资金流向（DC）
async fn fetch_market_tushare(
    client: &reqwest::Client,
    token: &str,
    start: &str,
    end: &str,
) -> Result<BTreeMap<NaiveDate, f64>, String> {
    let body = serde_json::json!({
        "api_name": "moneyflow_mkt_dc",
        "token": token,
        "params": {
            "start_date": start,
            "end_date": end,
        },
        "fields": "trade_date,net_amount"
    });
    let resp = client
        .post(TUSHARE_URL)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Tushare 请求失败: {e}"))?;
    let v: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Tushare 解析失败: {e}"))?;
    if let Some(msg) = v.get("msg").and_then(|m| m.as_str()) {
        if !msg.is_empty() && msg != "ok" && v.pointer("/data/items").is_none() {
            return Err(format!("Tushare: {msg}"));
        }
    }
    let mut out = BTreeMap::new();
    let Some(items) = v.pointer("/data/items").and_then(|x| x.as_array()) else {
        return Err("Tushare 未返回 items（可能积分不足或 token 无效）".into());
    };
    let fields: Vec<String> = v
        .pointer("/data/fields")
        .and_then(|x| x.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|f| f.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_else(|| vec!["trade_date".into(), "net_amount".into()]);
    let i_date = fields.iter().position(|f| f == "trade_date").unwrap_or(0);
    let i_net = fields.iter().position(|f| f == "net_amount").unwrap_or(1);
    for row in items {
        let Some(arr) = row.as_array() else { continue };
        let date_raw = arr
            .get(i_date)
            .and_then(|x| x.as_str())
            .map(|s| s.to_string())
            .or_else(|| arr.get(i_date).and_then(|x| x.as_i64()).map(|n| n.to_string()));
        let Some(date_raw) = date_raw else { continue };
        let Some(date) = parse_trade_date(&date_raw) else {
            continue;
        };
        let net = arr
            .get(i_net)
            .and_then(|x| x.as_f64())
            .or_else(|| {
                arr.get(i_net)
                    .and_then(|x| x.as_str())
                    .and_then(|s| s.parse().ok())
            });
        if let Some(net) = net {
            out.insert(date, net);
        }
    }
    Ok(out)
}

async fn fetch_market_tushare_range(
    client: &reqwest::Client,
    token: &str,
) -> Result<BTreeMap<NaiveDate, f64>, String> {
    // 分两段拉取，降低单次体积
    let today = chrono::Local::now().date_naive();
    let mid = today - Duration::days(400);
    let start = today - Duration::days(800);
    let mut all = BTreeMap::new();
    for (a, b) in [
        (
            start.format("%Y%m%d").to_string(),
            mid.format("%Y%m%d").to_string(),
        ),
        (
            (mid + Duration::days(1)).format("%Y%m%d").to_string(),
            today.format("%Y%m%d").to_string(),
        ),
    ] {
        match fetch_market_tushare(client, token, &a, &b).await {
            Ok(part) => all.extend(part),
            Err(e) if all.is_empty() => return Err(e),
            Err(_) => {}
        }
    }
    if all.is_empty() {
        Err("Tushare 大盘资金流为空".into())
    } else {
        Ok(all)
    }
}

/// 腾讯免费：上证 + 深成指成交，作资金活跃度代理（可回测）。
async fn fetch_turnover_activity(
    client: &reqwest::Client,
    limit: u32,
) -> Result<(BTreeMap<NaiveDate, f64>, BTreeMap<NaiveDate, f64>), String> {
    let (sh_res, sz_res) = tokio::join!(
        fetch_tencent_index_day(client, "sh000001", limit),
        fetch_tencent_index_day(client, "sz399001", limit),
    );
    let sh = sh_res?;
    let sz = sz_res?;

    let mut amount: BTreeMap<NaiveDate, f64> = BTreeMap::new();
    let mut close: BTreeMap<NaiveDate, f64> = BTreeMap::new();
    for (date, amt, c) in sh {
        *amount.entry(date).or_insert(0.0) += amt;
        if c > 0.0 {
            close.insert(date, c);
        }
    }
    for (date, amt, _) in sz {
        *amount.entry(date).or_insert(0.0) += amt;
    }

    if amount.len() < 30 {
        return Err(format!("两市成交代理样本过少: {}", amount.len()));
    }
    Ok((amount, close))
}

async fn fetch_tencent_index_day(
    client: &reqwest::Client,
    symbol: &str,
    limit: u32,
) -> Result<Vec<(NaiveDate, f64, f64)>, String> {
    let param = format!("{symbol},day,,,{limit},");
    let resp = client
        .get(TENCENT_KLINE_URL)
        .header("Referer", "https://gu.qq.com/")
        .query(&[("param", param.as_str())])
        .send()
        .await
        .map_err(|e| format!("腾讯指数日线失败: {e}"))?
        .error_for_status()
        .map_err(|e| format!("腾讯指数日线响应异常: {e}"))?
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("腾讯指数日线解析失败: {e}"))?;

    let rows = resp
        .pointer(&format!("/data/{symbol}/day"))
        .and_then(|v| v.as_array())
        .cloned()
        .or_else(|| {
            resp.pointer(&format!("/data/{symbol}/qfqday"))
                .and_then(|v| v.as_array())
                .cloned()
        })
        .unwrap_or_default();

    if rows.is_empty() {
        return Err(format!("腾讯 {symbol} 日线为空"));
    }

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let Some(arr) = row.as_array() else { continue };
        if arr.len() < 6 {
            continue;
        }
        let Some(date) = arr[0].as_str().and_then(parse_trade_date) else {
            continue;
        };
        let amt = arr[5]
            .as_str()
            .and_then(|s| s.parse::<f64>().ok())
            .or_else(|| arr[5].as_f64())
            .unwrap_or(0.0);
        let c = arr[2]
            .as_str()
            .and_then(|s| s.parse::<f64>().ok())
            .or_else(|| arr[2].as_f64())
            .unwrap_or(0.0);
        out.push((date, amt, c));
    }
    Ok(out)
}

async fn fetch_market_eastmoney_once(client: &reqwest::Client) -> BTreeMap<NaiveDate, f64> {
    let mut out = BTreeMap::new();
    // 只打最快的 delay 源一次，避免多 host 串行卡死
    let Ok(resp) = client
        .get("https://push2delay.eastmoney.com/api/qt/stock/fflow/daykline/get")
        .query(&[
            ("lmt", "0"),
            ("klt", "101"),
            ("secid", "1.000001"),
            ("secid2", "0.399001"),
            ("fields1", "f1,f2,f3,f7"),
            (
                "fields2",
                "f51,f52,f53,f54,f55,f56,f57,f58,f59,f60,f61,f62,f63,f64,f65",
            ),
            ("ut", "b2884a393a59ad64002292a3e90d46a5"),
        ])
        .header("Referer", "https://data.eastmoney.com/zjlx/dpzjlx.html")
        .send()
        .await
    else {
        return out;
    };
    let Ok(v) = resp.json::<serde_json::Value>().await else {
        return out;
    };
    let Some(lines) = v.pointer("/data/klines").and_then(|x| x.as_array()) else {
        return out;
    };
    for line in lines {
        let Some(s) = line.as_str() else { continue };
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() < 2 {
            continue;
        }
        let Some(date) = parse_trade_date(parts[0]) else {
            continue;
        };
        let Ok(main) = parts[1].parse::<f64>() else {
            continue;
        };
        out.insert(date, main);
    }
    out
}

/// 拉取资金流归档。`Fast` 供预测/常规回测，避免北向翻页拖慢。
pub async fn fetch_archive(mode: FetchMode) -> Result<CapitalFlowArchive, String> {
    let client = if mode == FetchMode::Fast {
        http_client_fast()?
    } else {
        http_client()?
    };
    let mut archive = load_disk_cache();
    let mut notes: Vec<String> = Vec::new();

    // Fast：磁盘已有足够成交代理时，直接用缓存，只做极轻量刷新
    let disk_ok = archive.activity_days() >= 60;
    let limit = if mode == FetchMode::Fast { 180 } else { 400 };

    let activity_fut = fetch_turnover_activity(&client, limit);
    let tushare_token = resolve_tushare_token();
    let tushare_fut = async {
        if let Some(token) = tushare_token {
            fetch_market_tushare_range(&client, &token).await.ok()
        } else {
            None
        }
    };

    if mode == FetchMode::Fast {
        let (act_res, tushare_res) = tokio::join!(activity_fut, tushare_fut);
        match act_res {
            Ok((amt, cls)) => {
                let n = amt.len();
                archive.activity_amount = amt;
                archive.activity_close = cls;
                notes.push(format!("两市成交代理 {n} 日"));
            }
            Err(e) => {
                if disk_ok {
                    notes.push(format!("成交代理沿用缓存({e})"));
                } else {
                    notes.push(format!("成交代理失败({e})"));
                }
            }
        }
        if let Some(m) = tushare_res {
            let n = m.len();
            for (d, v) in m {
                archive.market_main.insert(d, v);
            }
            notes.push(format!("Tushare主力 {n} 日"));
        } else if resolve_tushare_token().is_none() {
            notes.push("未配置Tushare(可选)".into());
        }
        // Fast 不拉北向翻页、不拉东财慢源
    } else {
        let (act_res, tushare_res, em, north) = tokio::join!(
            activity_fut,
            tushare_fut,
            fetch_market_eastmoney_once(&client),
            fetch_north_net(&client, 8),
        );
        match act_res {
            Ok((amt, cls)) => {
                let n = amt.len();
                archive.activity_amount = amt;
                archive.activity_close = cls;
                notes.push(format!("两市成交代理 {n} 日"));
            }
            Err(e) => notes.push(format!("成交代理失败({e})")),
        }
        if let Some(m) = tushare_res {
            let n = m.len();
            for (d, v) in m {
                archive.market_main.insert(d, v);
            }
            notes.push(format!("Tushare主力 {n} 日"));
        }
        if !em.is_empty() {
            let n = em.len();
            for (d, v) in em {
                archive.market_main.insert(d, v);
            }
            notes.push(format!("东财主力 {n} 日"));
        }
        if !north.is_empty() {
            let n = north.len();
            archive.north_net_yi.extend(north);
            notes.push(format!("北向净额 {n} 日"));
        }
    }

    if archive.is_empty() {
        return Err(format!(
            "资金流无数据：{}。请检查网络；或配置 Tushare Token",
            notes.join("；")
        ));
    }

    archive.source_note = notes.join("；");
    save_disk_cache(&archive);
    Ok(archive)
}

pub async fn fetch_archive_cached() -> Result<CapitalFlowArchive, String> {
    {
        let guard = cache().lock().await;
        if let Some(entry) = guard.as_ref() {
            if entry.fetched_at.elapsed() < StdDuration::from_secs(2 * 60 * 60)
                && entry.archive.usable_days() > 0
            {
                return Ok(entry.archive.clone());
            }
        }
    }

    // 磁盘缓存足够：直接用，避免每次预测都打网（次日会因内存过期再刷新）
    let disk = load_disk_cache();
    let disk_fresh = std::fs::metadata(disk_cache_path())
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.elapsed().ok())
        .map(|d| d < StdDuration::from_secs(12 * 60 * 60))
        .unwrap_or(false);
    if disk.activity_days() >= 80 && disk_fresh {
        let mut guard = cache().lock().await;
        *guard = Some(CacheEntry {
            fetched_at: Instant::now(),
            archive: disk.clone(),
        });
        return Ok(disk);
    }

    let archive = fetch_archive(FetchMode::Fast).await?;
    let mut guard = cache().lock().await;
    *guard = Some(CacheEntry {
        fetched_at: Instant::now(),
        archive: archive.clone(),
    });
    Ok(archive)
}

fn median_abs(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 1.0;
    }
    let mut v: Vec<f64> = values.iter().map(|x| x.abs()).collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = v.len() / 2;
    if v.len() % 2 == 0 {
        ((v[mid - 1] + v[mid]) / 2.0).max(1e-6)
    } else {
        v[mid].max(1e-6)
    }
}

fn score_series(
    as_of: NaiveDate,
    series: &BTreeMap<NaiveDate, f64>,
    unit_note: &str,
) -> Option<CapitalFlowSignal> {
    let window_start = as_of - Duration::days(40);
    let hist: Vec<(NaiveDate, f64)> = series
        .range(window_start..=as_of)
        .map(|(d, v)| (*d, *v))
        .collect();
    if hist.is_empty() {
        return None;
    }
    let today = hist.last().copied()?;
    if (as_of - today.0).num_days() > 5 {
        return None;
    }
    let vals: Vec<f64> = hist.iter().map(|(_, v)| *v).collect();
    let scale = median_abs(&vals[vals.len().saturating_sub(20)..]);
    let n1 = today.1 / scale;
    let n5: f64 = vals.iter().rev().take(5).sum::<f64>() / scale;
    let score = (n1 * 0.55 + n5 * 0.45).clamp(-2.5, 2.5);
    let note = format!(
        "{unit_note} · 1日 {:+.1} / 5日累计 {:+.1}（相对近20日强度）",
        n1, n5
    );
    Some(CapitalFlowSignal {
        score,
        note,
        status: "ok",
    })
}

/// 两市成交代理：放量上涨偏谨慎、放量下跌偏钝化（宽基常见特征）。
fn score_activity_proxy(archive: &CapitalFlowArchive, as_of: NaiveDate) -> Option<CapitalFlowSignal> {
    let window_start = as_of - Duration::days(40);
    let dates: Vec<NaiveDate> = archive
        .activity_amount
        .range(window_start..=as_of)
        .map(|(d, _)| *d)
        .collect();
    if dates.len() < 21 {
        return None;
    }
    let today = *dates.last()?;
    if (as_of - today).num_days() > 5 {
        return None;
    }
    let prev = dates[dates.len() - 2];
    let amt_today = *archive.activity_amount.get(&today)?;
    let close_today = *archive.activity_close.get(&today)?;
    let close_prev = *archive.activity_close.get(&prev)?;
    if close_prev <= 0.0 {
        return None;
    }
    let ret = (close_today - close_prev) / close_prev;
    let recent: Vec<f64> = dates
        .iter()
        .rev()
        .take(20)
        .filter_map(|d| archive.activity_amount.get(d).copied())
        .collect();
    let med = median_abs(&recent);
    let z = amt_today / med;
    let (score, tip) = if z > 1.15 && ret > 0.005 {
        (-0.9, "放量上涨偏谨慎")
    } else if z > 1.15 && ret < -0.005 {
        (0.6, "放量下跌或钝化")
    } else if z < 0.85 {
        (-ret * 2.0, "缩量")
    } else {
        (-ret * 4.0, "量能中性偏反向")
    };
    Some(CapitalFlowSignal {
        score: score.clamp(-2.5, 2.5),
        note: format!(
            "两市成交代理 · {tip} · 量比 {:.2} · 上证 {:+.2}%",
            z,
            ret * 100.0
        ),
        status: "ok",
    })
}

/// 按日评估：真主力 > 两市成交代理 > 北向净额。
pub fn evaluate_as_of(archive: &CapitalFlowArchive, as_of: NaiveDate) -> CapitalFlowSignal {
    if let Some(sig) = score_series(as_of, &archive.market_main, "大盘主力净流入") {
        return sig;
    }
    if let Some(sig) = score_activity_proxy(archive, as_of) {
        return sig;
    }
    if let Some(mut sig) = score_series(as_of, &archive.north_net_yi, "北向净买入") {
        sig.note = format!("{} · 主力/成交代理缺日，改用北向", sig.note);
        return sig;
    }
    CapitalFlowSignal {
        score: 0.0,
        note: "当日无资金流数据，未计入".into(),
        status: "skip",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn score_prefers_market_main() {
        let mut archive = CapitalFlowArchive::default();
        let d0 = NaiveDate::from_ymd_opt(2025, 6, 3).unwrap();
        for i in 0..25 {
            let d = d0 + Duration::days(i);
            archive
                .market_main
                .insert(d, if i < 20 { 1e9 } else { 8e9 });
        }
        let as_of = d0 + Duration::days(24);
        let sig = evaluate_as_of(&archive, as_of);
        assert_eq!(sig.status, "ok");
        assert!(sig.score > 0.0);
        assert!(sig.note.contains("主力"));
    }

    #[test]
    fn activity_proxy_scores_without_tushare() {
        let mut archive = CapitalFlowArchive::default();
        let d0 = NaiveDate::from_ymd_opt(2025, 1, 2).unwrap();
        let mut px = 3000.0;
        for i in 0..30 {
            let d = d0 + Duration::days(i);
            let amt = if i == 29 { 2.0e9 } else { 1.0e9 };
            if i == 29 {
                px *= 1.01;
            }
            archive.activity_amount.insert(d, amt);
            archive.activity_close.insert(d, px);
        }
        let as_of = d0 + Duration::days(29);
        let sig = evaluate_as_of(&archive, as_of);
        assert_eq!(sig.status, "ok");
        assert!(sig.note.contains("成交代理"));
        assert!(sig.score < 0.0, "volume up-day should fade");
    }

    #[test]
    fn missing_day_skips() {
        let archive = CapitalFlowArchive::default();
        let sig = evaluate_as_of(&archive, NaiveDate::from_ymd_opt(2026, 7, 17).unwrap());
        assert_eq!(sig.status, "skip");
    }
}
