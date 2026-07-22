//! 股票搜索。

use crate::ashare::client::client;
use crate::ashare::quotes::fetch_stock_quotes;
use crate::ashare::symbol::{market_from_search_item, sector_from_search_item};
use crate::models::Stock;

const SEARCH_URL: &str = "https://searchapi.eastmoney.com/api/suggest/get";

/// 按名称/代码搜索 A 股
pub async fn search_stocks(query: &str, limit: usize) -> Result<Vec<Stock>, String> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(vec![]);
    }

    let http = client()?;
    let resp = http
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
