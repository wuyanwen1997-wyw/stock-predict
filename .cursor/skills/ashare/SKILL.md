---
name: ashare
description: >-
  Guides work on the A-share market data client in src-tauri/src/ashare
  (quotes, klines, intraday, search, hot ranks, symbol mapping). Use when
  editing ashare/*, market.rs facade, get_stock_klines / get_stock_intraday,
  or adding Eastmoney/Tencent/Sina/THS data sources.
---

# ashare 行情客户端

## 必读文档

- 概览：`docs/ashare/README.md`
- API：`docs/ashare/api.md`
- 代码：`src-tauri/src/ashare/`
- 兼容门面：`src-tauri/src/market.rs`（仅 re-export，勿往回塞实现）

## 职责边界

**做**：第三方 HTTP、多源兜底、归一成 `Stock` / `DailyBar` / `StockQuote` / `PricePoint`。  
**不做**：策略融合、预测、回测、选股规则、UI。  
**禁止**：在 `src/`（React）直连东财/腾讯/新浪；新逻辑写在 `ashare` 子模块，经 `commands` 暴露。

## 改代码时

1. 按能力改文件：`quotes` / `kline` / `search` / `hot` / `symbol` / `client`。
2. 对外签名优先保持稳定；破坏性变更同步 `docs/ashare/api.md` 与前端 `src/services/api.ts`、`src/types`。
3. K 线周期用 `models::KlinePeriod`，不要散落魔法字符串；IPC 用 `period` 字符串（`day`/`week`/`month`/`min5`…）。
4. 日/分钟：腾讯 → 新浪 → 东财；周/月：腾讯 → 东财；报价：腾讯 → 新浪 → 东财。
5. 新数据源：URL + 解析放子模块内部；公开 API 仍返回项目 DTO。
6. `hot` 与 `kline` 解耦；榜单融合失败策略保持「部分源成功仍可用」。
7. 单测：`cargo test --manifest-path src-tauri/Cargo.toml ashare::`

## 调用示例

```rust
use crate::ashare::{fetch_klines, fetch_stock_quotes, search_stocks};
use crate::models::KlinePeriod;

let bars = fetch_klines(stock, KlinePeriod::Day, 90).await?;
let quotes = fetch_stock_quotes(stocks).await?;
```

旧代码可用 `crate::market::…`（等价 re-export）。

## 常见坑

- 分钟线 `DailyBar.date` 带时分，日线只有日期。
- 新浪无可靠周/月，不要强行加 sina week/month。
- Android/桌面 UA 已在 `client.rs` 区分，勿删 Referer。
- 勿把 `calc_volatility` 当行情 API 扩展点；统计原语在 `algo::stats`（见 `.cursor/skills/algo`）。
- 行情模块只负责取数与 DTO；打分/融合属 `algo`。
