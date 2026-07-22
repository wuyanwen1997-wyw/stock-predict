//! 多周期 K 线与分时。

use crate::ashare::client::{
    apply_browser_headers, client, parse_f64, parse_json_f64,
};
use crate::ashare::symbol::{to_secid, to_sina_symbol, to_tencent_symbol};
use crate::models::{DailyBar, KlinePeriod, PricePoint, Stock};

const KLINE_URL: &str = "https://push2his.eastmoney.com/api/qt/stock/kline/get";
const TENCENT_KLINE_URL: &str = "https://web.ifzq.gtimg.cn/appstock/app/fqkline/get";
const SINA_KLINE_URL: &str =
    "https://money.finance.sina.com.cn/quotes_service/api/json_v2.php/CN_MarketData.getKLineData";
const TRENDS_URL: &str = "https://push2.eastmoney.com/api/qt/stock/trends2/get";

/// 拉取日线（兼容旧调用）。
pub async fn fetch_daily_klines(stock: &Stock, limit: u32) -> Result<Vec<DailyBar>, String> {
    fetch_klines(stock, KlinePeriod::Day, limit).await
}

/// 拉取指定周期 K 线（最近 N 根）。
/// 日/分钟：腾讯 → 新浪 → 东财；周/月：腾讯 → 东财。
pub async fn fetch_klines(
    stock: &Stock,
    period: KlinePeriod,
    limit: u32,
) -> Result<Vec<DailyBar>, String> {
    let limit = if limit == 0 {
        period.default_limit()
    } else {
        limit
    };
    let label = period.as_str();
    let mut errors = Vec::new();

    match fetch_tencent_klines(stock, period, limit).await {
        Ok(bars) if !bars.is_empty() => return Ok(bars),
        Ok(_) => errors.push(format!("腾讯{label}返回为空")),
        Err(e) => errors.push(e),
    }

    if period.sina_scale().is_some() {
        match fetch_sina_klines(stock, period, limit).await {
            Ok(bars) if !bars.is_empty() => return Ok(bars),
            Ok(_) => errors.push(format!("新浪{label}返回为空")),
            Err(e) => errors.push(e),
        }
    }

    match fetch_em_klines(stock, period, limit).await {
        Ok(bars) if !bars.is_empty() => return Ok(bars),
        Ok(_) => errors.push(format!("东方财富{label}返回为空")),
        Err(e) => errors.push(e),
    }

    Err(format!("{label} K 线获取失败: {}", errors.join("；")))
}

/// 当日分时走势（东财 trends2）。
pub async fn fetch_intraday_trends(stock: &Stock) -> Result<Vec<PricePoint>, String> {
    let http = client()?;
    let secid = to_secid(&stock.market, &stock.code);

    let resp = apply_browser_headers(http.get(TRENDS_URL))
        .query(&[
            ("secid", secid.as_str()),
            ("fields1", "f1,f2,f3,f4,f5,f6,f7,f8,f9,f10,f11,f12,f13"),
            ("fields2", "f51,f52,f53,f54,f55,f56,f57,f58"),
            ("iscr", "0"),
            ("ndays", "1"),
        ])
        .send()
        .await
        .map_err(|e| format!("东财分时请求失败: {e}"))?
        .error_for_status()
        .map_err(|e| format!("东财分时响应异常: {e}"))?
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("东财分时解析失败: {e}"))?;

    parse_em_trends(&resp)
}

fn parse_em_trends(resp: &serde_json::Value) -> Result<Vec<PricePoint>, String> {
    let lines = resp
        .pointer("/data/trends")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut points = Vec::with_capacity(lines.len());
    for line in lines {
        let text = line.as_str().unwrap_or_default();
        let parts: Vec<&str> = text.split(',').collect();
        if parts.len() < 5 {
            continue;
        }
        let price: f64 = parts[1].parse().unwrap_or(0.0);
        if price <= 0.0 {
            continue;
        }
        points.push(PricePoint {
            time: parts[0].to_string(),
            price,
            volume: parts[3].parse().unwrap_or(0.0),
        });
    }

    if points.is_empty() {
        return Err("东财分时返回为空".into());
    }
    Ok(points)
}

async fn fetch_em_klines(
    stock: &Stock,
    period: KlinePeriod,
    limit: u32,
) -> Result<Vec<DailyBar>, String> {
    let http = client()?;
    let secid = to_secid(&stock.market, &stock.code);
    let klt = period.eastmoney_klt().to_string();
    let lmt = limit.to_string();
    let label = period.as_str();

    let resp = apply_browser_headers(http.get(KLINE_URL))
        .query(&[
            ("secid", secid.as_str()),
            ("fields1", "f1,f2,f3,f4,f5,f6"),
            ("fields2", "f51,f52,f53,f54,f55,f56,f57,f58,f59,f60,f61"),
            ("klt", klt.as_str()),
            ("fqt", "1"),
            ("beg", "0"),
            ("end", "20500101"),
            ("lmt", lmt.as_str()),
        ])
        .send()
        .await
        .map_err(|e| format!("东方财富{label}请求失败: {e}"))?
        .error_for_status()
        .map_err(|e| format!("东方财富{label}响应异常: {e}"))?
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("东方财富{label}解析失败: {e}"))?;

    parse_em_klines(&resp)
}

async fn fetch_tencent_klines(
    stock: &Stock,
    period: KlinePeriod,
    limit: u32,
) -> Result<Vec<DailyBar>, String> {
    let http = client()?;
    let symbol = to_tencent_symbol(&stock.market, &stock.code);
    let freq = period.tencent_freq();
    let param = if period.is_intraday() {
        format!("{symbol},{freq},,,{limit}")
    } else {
        format!("{symbol},{freq},,,{limit},qfq")
    };
    let label = period.as_str();

    let resp = apply_browser_headers(http.get(TENCENT_KLINE_URL))
        .header("Referer", "https://gu.qq.com/")
        .query(&[("param", param.as_str())])
        .send()
        .await
        .map_err(|e| format!("腾讯{label}请求失败: {e}"))?
        .error_for_status()
        .map_err(|e| format!("腾讯{label}响应异常: {e}"))?
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("腾讯{label}解析失败: {e}"))?;

    parse_tencent_klines(&resp, &symbol, period, limit)
}

async fn fetch_sina_klines(
    stock: &Stock,
    period: KlinePeriod,
    limit: u32,
) -> Result<Vec<DailyBar>, String> {
    let scale = period
        .sina_scale()
        .ok_or_else(|| format!("新浪不支持周期 {}", period.as_str()))?;
    let http = client()?;
    let symbol = to_sina_symbol(&stock.market, &stock.code);
    let label = period.as_str();
    let scale_s = scale.to_string();
    let datalen = limit.to_string();

    let resp = apply_browser_headers(http.get(SINA_KLINE_URL))
        .header("Referer", "https://finance.sina.com.cn/")
        .query(&[
            ("symbol", symbol.as_str()),
            ("scale", scale_s.as_str()),
            ("ma", "no"),
            ("datalen", datalen.as_str()),
        ])
        .send()
        .await
        .map_err(|e| format!("新浪{label}请求失败: {e}"))?
        .error_for_status()
        .map_err(|e| format!("新浪{label}响应异常: {e}"))?
        .json::<Vec<serde_json::Value>>()
        .await
        .map_err(|e| format!("新浪{label}解析失败: {e}"))?;

    parse_sina_klines(&resp, label)
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

fn tencent_series_keys(period: KlinePeriod) -> &'static [&'static str] {
    match period {
        KlinePeriod::Day => &["qfqday", "day"],
        KlinePeriod::Week => &["qfqweek", "week"],
        KlinePeriod::Month => &["qfqmonth", "month"],
        KlinePeriod::Min1 => &["m1", "qfqm1"],
        KlinePeriod::Min5 => &["m5", "qfqm5"],
        KlinePeriod::Min15 => &["m15", "qfqm15"],
        KlinePeriod::Min30 => &["m30", "qfqm30"],
        KlinePeriod::Min60 => &["m60", "qfqm60"],
    }
}

fn parse_tencent_klines(
    resp: &serde_json::Value,
    symbol: &str,
    period: KlinePeriod,
    limit: u32,
) -> Result<Vec<DailyBar>, String> {
    let label = period.as_str();
    let stock_node = resp.pointer("/data").and_then(|d| d.get(symbol));

    let mut rows: Vec<serde_json::Value> = Vec::new();
    for key in tencent_series_keys(period) {
        if let Some(arr) = stock_node
            .and_then(|s| s.get(*key))
            .and_then(|v| v.as_array())
        {
            rows = arr.clone();
            break;
        }
        let path = format!("/data/{symbol}/{key}");
        if let Some(arr) = resp.pointer(&path).and_then(|v| v.as_array()) {
            rows = arr.clone();
            break;
        }
    }

    let mut bars = Vec::with_capacity(rows.len());
    for row in rows {
        let parts = row.as_array().cloned().unwrap_or_default();
        if parts.len() < 6 {
            continue;
        }
        let date = parts[0].as_str().unwrap_or_default();
        if date.is_empty() {
            continue;
        }
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
        return Err(format!("腾讯{label}返回为空"));
    }

    Ok(bars)
}

fn parse_sina_klines(items: &[serde_json::Value], label: &str) -> Result<Vec<DailyBar>, String> {
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
        return Err(format!("新浪{label}返回为空"));
    }

    Ok(bars)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kline_period_mappings() {
        assert_eq!(KlinePeriod::parse("day").unwrap(), KlinePeriod::Day);
        assert_eq!(KlinePeriod::parse("week").unwrap(), KlinePeriod::Week);
        assert_eq!(KlinePeriod::parse("month").unwrap(), KlinePeriod::Month);
        assert_eq!(KlinePeriod::parse("min5").unwrap(), KlinePeriod::Min5);
        assert_eq!(KlinePeriod::Day.eastmoney_klt(), 101);
        assert_eq!(KlinePeriod::Week.eastmoney_klt(), 102);
        assert_eq!(KlinePeriod::Month.eastmoney_klt(), 103);
        assert_eq!(KlinePeriod::Min15.eastmoney_klt(), 15);
        assert_eq!(KlinePeriod::Min60.tencent_freq(), "m60");
        assert_eq!(KlinePeriod::Week.sina_scale(), None);
        assert_eq!(KlinePeriod::Day.sina_scale(), Some(240));
        assert!(KlinePeriod::Min1.is_intraday());
        assert!(!KlinePeriod::Day.is_intraday());
    }
}
