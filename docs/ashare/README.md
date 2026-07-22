# ashare — A 股行情基础设施

代码：`src-tauri/src/ashare/`  
定位：把东财 / 腾讯 / 新浪 / 同花顺等**第三方非官方接口**封装成稳定内部 API，供 `commands`、`predictor`、`screener` 等业务使用。  
**不是**通用开源 SDK；返回项目 DTO（`Stock` / `DailyBar` / `PricePoint`）。

## 模块结构

```
ashare/
  client.rs   HTTP 客户端、重试、UA、JSON 数值解析
  symbol.rs   SH/SZ ↔ secid / sh600519 / sz000001
  quotes.rs   批量实时报价
  kline.rs    日/周/月/分钟 K 线 + 分时
  search.rs   代码/名称搜索
  hot.rs      多源人气榜融合
```

兼容门面：`crate::market` → re-export `crate::ashare`（旧代码可继续 `market::`）。

## 分层约定

```
领域层 (predictor / strategy / screener / backtest)
        ↓ 只依赖 ashare 公开函数
ashare（本模块）
        ↓ HTTP
东财 · 腾讯 · 新浪 · 同花顺 …
```

- UI（`src/`）禁止直连第三方行情；只通过 Tauri `invoke` → `commands` → ashare。
- 新增数据源：写在对应子模块内，对外保持函数签名稳定。
- 人气榜（`hot`）是产品能力，不是基础 K 线客户端的一部分；改榜逻辑勿拖累 `kline`/`quotes`。

## 公开 API 一览

详见 [api.md](./api.md)。

| 能力 | 函数 |
|------|------|
| 报价 | `fetch_stock_quotes` · `apply_quote` |
| K 线 | `fetch_klines` · `fetch_daily_klines` · `fetch_intraday_trends` |
| 搜索 | `search_stocks` |
| 热股 | `fetch_hot_stocks` · `fetch_hot_stock_codes` |
| 映射 | `to_secid` · `to_tencent_symbol` · `to_sina_symbol` · `infer_market` |

周期枚举：`models::KlinePeriod`（`day` / `week` / `month` / `min1`…`min60`）。

## 多源策略

| 能力 | 顺序 |
|------|------|
| 日线 / 分钟 | 腾讯 → 新浪 → 东财 |
| 周 / 月 | 腾讯 → 东财（新浪周月不稳定，跳过） |
| 分时 | 东财 `trends2` |
| 报价 | 东财 push2 多节点 + 重试 |
| 热股 | 东财人气 + 同花顺小时榜 + 东财飙升，RRF 融合后再补报价 |

## 相关

- Agent Skill：`.cursor/skills/ashare/SKILL.md`
- 总架构：[软件设计说明.md](../软件设计说明.md)
