# algo — 纯算法层

代码：`src-tauri/src/algo/`  

## 什么算「算法」（本仓库约定）

**迁入 `algo`：** 相对复杂的「数据 → 信息/结论」计算——输入行情、标题、资金序列、信号列表等，输出得分、概率、方向、门控权重、命中统计等；**无 HTTP、无磁盘、无 Tauri、不编排用例**。

**留在业务/基础设施：**

| 层 | 路径 | 职责 |
|----|------|------|
| 表现 | `src/` | UI、Zustand、IPC 薄封装 |
| 用例编排 | `commands` / `predictor` / `strategy::evaluate_*` / `backtest::run_*` / `screener` | 拉数、拼结果、默认组合、进度 |
| 行情 IO | `ashare` | 第三方 HTTP → DTO |
| 其它 IO | `cninfo` / `capital_flow`（fetch） | 公告、资金流拉取与缓存 |
| 共享 DTO | `models` | 序列化结构，不含算法 |

**混在一起时：** 拆开——例如 `capital_flow` 只保留 fetch；`evaluate_as_of` 进 `algo::capital`。`strategy` 保留 catalog / live 资讯 HTTP；动量等打分进 `algo::tech`。

## 模块一览

| 子模块 | 输入 → 输出 |
|--------|-------------|
| `stats` | K 线 → 波动率 |
| `factor` | K 线 → 技术多因子快照/得分 |
| `tech` | K 线 → 动量 / 均值回归 / 量价 / 多因子信号贡献 |
| `fuse` | 多源 `(weight, SignalContribution)` → 融合概率 + 宽基/多日门控 |
| `sentiment` | 标的画像 + 标题 → 情绪分（含词表/宽基权重 JSON） |
| `capital` | `CapitalFlowArchive` + 日期 → 资金流方向强度 |
| `backtest` | 预测 vs 实际 → `HitCounters` / 出手线 / 命中率口径 |
| `bs_markers` | K 线 → MACD 金叉/死叉 B/S（主图叠加，不进融合）；设计见 [K线买卖点设计说明](../K线买卖点设计说明.md) |

## 兼容门面（旧路径仍可用）

- `factor_model` → `algo::factor`
- `message_sentiment` → `algo::sentiment`
- `capital_flow::{CapitalFlowArchive, evaluate_as_of}` → `algo::capital`（fetch 仍在 `capital_flow`）
- `strategy::EnsembleSignal` → `algo::fuse`
- `market::calc_volatility` → `algo::stats`
- `backtest::ACTIONABLE_LEAD` → `algo::backtest`

新代码优先：`use crate::algo::…`。

## 刻意不放进 algo

- 信号源目录、默认 compose、normalize（产品配置）
- `evaluate_live` 拉新闻 / 美股（IO）
- 交易日日历与场景路径（仍在 `predictor`；日历可后续再迁）
- 选股硬过滤流水线（规则薄 + 强依赖编排，暂留 `screener`）

Skill：`.cursor/skills/algo/SKILL.md` · API 细节：[api.md](./api.md)
