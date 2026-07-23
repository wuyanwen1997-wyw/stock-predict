# algo 公开 API 说明

代码根：`src-tauri/src/algo/`。边界总览见 [README.md](./README.md)。

**通用约定**

- 算法为纯函数：不发起网络、不读写磁盘、不依赖 Tauri。
- 缺数据时不抛「网络错误」，而是用 `None`、`status: "skip"`、中性分（约 50/50）表达。
- 得分类信号常见区间约 `[-2.5, 2.5]`，再经 `fuse::contrib` / `probs_from_score` 转成涨跌概率。
- 新代码优先 `crate::algo::…`；旧门面（`factor_model` 等）仅兼容。

---

## 1. `stats` — 统计原语

| 函数 | 说明 |
|------|------|
| `calc_volatility(bars) -> f64` | 用相邻收盘价算日收益率标准差，结果 clamp 到 `[0.005, 0.08]`；样本过少时返回默认 `0.02`。供因子/场景波动估计，**不是**行情接口。 |

---

## 2. `factor` — 技术多因子

从一段日线算出「多因子快照」：均线、RSI、动量、量比、波动率与综合 `score`，并附带可读 `hints`。

| 符号 | 说明 |
|------|------|
| `FactorStyle::Default` | 个股：趋势 + 超买超卖 |
| `FactorStyle::IndexEtf` | 宽基 A 股 ETF：MA20 过滤 + 隔日反向等（如 510980 一类） |
| `MIN_BARS` | 最少 K 线根数（25） |
| `style_for_stock(stock)` | 按名称/代码/板块推断用哪套风格 |
| `clamp_lookback(days)` | 把回看天数规范到允许档位（约 25–120） |
| `take_lookback(bars, n)` | 截取末尾窗口 |
| `compute(bars)` | 默认风格打分 |
| `compute_styled(bars, style)` | 指定风格，默认 horizon=1（次日） |
| `compute_styled_for_horizon(bars, style, horizon_days)` | `horizon_days` 1=次日；2–5=多日累计，会弱化隔日反向等短线项 |

返回 `Option<FactorSnapshot>`：K 线不足或价格异常时为 `None`。

```rust
use crate::algo::factor::{compute_styled_for_horizon, style_for_stock, take_lookback};

let window = take_lookback(bars, 50);
let snap = compute_styled_for_horizon(window, style_for_stock(stock), 1);
```

---

## 3. `tech` — 单源技术信号

在 `factor` 之上，把 K 线直接变成可融合的 `SignalContribution`（含 id/名称/涨跌概率草稿/note/status）。  
由 `strategy::evaluate_*` 收集多源后再交给 `fuse`。

| 函数 | 在算什么 |
|------|----------|
| `eval_factor(stock, bars, horizon)` | 调用多因子 → 一条「技术多因子」贡献 |
| `eval_momentum(…)` | 短中期动量；宽基单日用 3 日动量互补（避免与多因子重复隔日反向） |
| `eval_mean_reversion(bars, horizon)` | 相对 MA20 / RSI 极端的回归倾向；多日 horizon 会降权 |
| `eval_volume(…)` | 量比 × 涨跌：个股偏趋势确认，宽基单日偏「放量谨慎」 |

K 线不足时返回 `status: skip` 的中性贡献（经 `neutral`）。

---

## 4. `fuse` — 融合与门控

把多路 `(相对权重, SignalContribution)` 合成最终涨跌概率，并对宽基/多日做规则门控（改权重或标 skip，**不拉数**）。

| 符号 | 说明 |
|------|------|
| `EnsembleSignal` | 融合结果：`up/down/confidence/predicted/high_confidence/summary_hint/contributions` |
| `fuse(raw)` | 权重 >0 的源按归一化权重加权；权重 0 仍出现在明细里但不进概率 |
| `reconcile_multiday_noise` | 多日模式：压低消息/资金/均值回归等短线源权重 |
| `reconcile_index_momentum` | 宽基：动量与多因子方向冲突时清掉动量权重 |
| `reconcile_index_factor_message` | 宽基：消息面领先概率 <55% 则 skip，避免 50% 稀释主概率 |
| `reconcile_index_factor_capital` | 宽基：资金流弱信号或与多因子冲突则 skip |
| `probs_from_score` / `probs_from_score_soft` | score → (up%, down%, confidence)；soft 用于宽基消息等 |
| `contrib` / `contrib_soft` / `neutral` | 构造单条 `SignalContribution` |

典型顺序：各源 `eval_*` →（若 horizon>1）`reconcile_multiday_*` → 宽基 `reconcile_index_*` → `fuse`。

---

## 5. `sentiment` — 消息情绪

按标的类型选词表/强度，对标题列表打分，得到有向 `score` 与说明 `note`。

| 符号 | 说明 |
|------|------|
| `MessageKind` | Corporate / Finance / Gold / BroadEtf / … |
| `MessageProfile` | 某类标的的打分配置（bullish/bearish 词、hit_weight、scale） |
| `classify(stock)` | 推断消息面类型 |
| `profile_for(stock)` | 得到完整画像（宽基会合并 JSON 权重） |
| `score_titles(profile, titles)` | 无日期衰减的标题打分 |
| `score_titles_dated(profile, as_of, dated)` | 带交易日衰减，适合回测 as_of |
| `search_queries(stock)` | 纯字符串：给 `cninfo` 等用的搜索词（无 IO） |

算法参数：`src-tauri/resources/message_weights_broad_etf.json`（宽基加权词）。

---

## 6. `capital` — 资金流评分

| 符号 | 说明 |
|------|------|
| `CapitalFlowArchive` | 按日序列：主力净流入、成交代理、北向等（**数据结构**；填充靠 `capital_flow::fetch_*`） |
| `CapitalFlowSignal` | `{ score, note, status }`，`status` 为 `ok` / `skip` 等 |
| `evaluate_as_of(archive, as_of)` | **优先级**：大盘主力 → 两市成交代理 → 北向；都不可用则 skip |

成交代理语义概要：放量上涨偏谨慎、放量下跌偏钝化（宽基常见特征）。

---

## 7. `backtest` — 回测口径（统计核）

不负责拉 K 线/公告；只定义「怎么算命中」。

| 符号 | 说明 |
|------|------|
| `ACTIONABLE_LEAD` | `55.0`：领先一侧概率 ≥ 该值才算「有效信号」样本 |
| `classify_change(change_pct)` | 实际涨跌：`>0` → `"up"`，否则 `"down"` |
| `is_actionable(up, down)` | 是否达到出手线 |
| `HitCounters` | 累计全样本/有效样本/高置信/分方向命中 |
| `HitCounters::observe(…)` | 登记一条预测结果 |
| `pct(hits, total)` | 命中率百分比，**一位小数**（与历史 UI 展示一致） |
| `round2` | 价格等两位小数 |

用例循环仍在 `crate::backtest::run_compose`（拉数 + 调预测 + 调 `HitCounters`）。

---

## 8. `bs_markers` — K 线主图 B/S（MACD）

供预测页日 K **叠加标注**，与融合预测 / 回测方向**无关**，不进入 `fuse` 权重。产品口径与选型见 [K线买卖点设计说明](../K线买卖点设计说明.md)。

| 符号 | 说明 |
|------|------|
| `MACD_FAST` / `MACD_SLOW` / `MACD_SIGNAL` | `12` / `26` / `9`（通达信/同花顺默认） |
| `MIN_BARS` | `35`：不足则返回空列表 |
| `compute_macd_bs(bars)` | 收盘价 EMA → DIF/DEA；DIF 上穿 DEA → `Buy`(B)，下穿 → `Sell`(S) |
| `filter_markers_by_dates(markers, dates)` | 按 chart 窗口日期过滤 |

**编排约定**：`analyze_stock` 对**全量**拉取的日 K 调用 `compute_macd_bs`，再按下发的 `chart_klines` 日期过滤，避免窗口截断导致 EMA 失真。DTO 为 `models::BsMarker { date, kind }`，经 `AnalysisResult.bs_markers` 下发。

---

## 依赖方向

```
ashare / cninfo / capital_flow(fetch)   ← IO
        ↓ DTO
strategy / predictor / backtest / screener   ← 编排
        ↓
      algo/*   ← 本层
        ↓
      models
```

禁止：`algo` 依赖 `ashare` / `reqwest` / Tauri；禁止 UI 直连第三方行情。
