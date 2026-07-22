# ashare 公开 API

均位于 `crate::ashare`（或兼容 `crate::market`）。错误统一为 `Result<_, String>`。

## 报价

```rust
use crate::ashare::{apply_quote, fetch_stock_quotes};
use crate::models::Stock;

let quotes = fetch_stock_quotes(&stocks).await?;
for stock in &mut stocks {
    if let Some(q) = quotes.get(&stock.code) {
        apply_quote(stock, q);
    }
}
```

- `fetch_stock_quotes(&[Stock]) -> HashMap<code, StockQuote>`
- 字段含 price / change_pct / OHLC / volume / turnover（东财 `f2…f18`）

## K 线

```rust
use crate::ashare::{fetch_daily_klines, fetch_intraday_trends, fetch_klines};
use crate::models::KlinePeriod;

let day = fetch_daily_klines(&stock, 90).await?;
let week = fetch_klines(&stock, KlinePeriod::Week, 104).await?;
let m5 = fetch_klines(&stock, KlinePeriod::Min5, 240).await?;
let trends = fetch_intraday_trends(&stock).await?; // Vec<PricePoint>
```

### `KlinePeriod`（`models.rs`）

| 枚举 | 字符串 | 东财 klt | 腾讯 freq | 新浪 scale |
|------|--------|----------|-----------|------------|
| Day | `day` | 101 | day | 240 |
| Week | `week` | 102 | week | — |
| Month | `month` | 103 | month | — |
| Min1…Min60 | `min1`…`min60` | 1…60 | m1…m60 | 1…60 |

`DailyBar.date`：日/周/月为 `YYYY-MM-DD`；分钟为 `YYYY-MM-DD HH:MM`。  
`limit == 0` 时使用 `period.default_limit()`。

### IPC（前端）

- `get_stock_klines(stock, limit?, period?)` → `DailyBar[]`
- `get_stock_intraday(stock)` → `PricePoint[]`

## 搜索 / 热股

```rust
let hits = search_stocks("茅台", 12).await?;
let hot = fetch_hot_stocks(12).await?;
let codes = fetch_hot_stock_codes(20).await?;
```

热股：多源失败时仍可能返回部分列表；全部失败才 `Err`。

## Symbol

```rust
to_secid("SH", "600519");        // "1.600519"
to_tencent_symbol("SZ", "000858"); // "sz000858"
infer_market("510980");          // "SH"
```

## 注意

- `calc_volatility` 已迁至 `algo::stats`；`ashare` / `market` 仅兼容 re-export。
- 勿在 `src/` 直连第三方 URL；勿把东财原始 JSON 泄漏到领域层。
