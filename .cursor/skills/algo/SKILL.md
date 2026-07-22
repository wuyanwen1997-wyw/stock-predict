---
name: algo
description: >-
  Guides work on the pure algorithm layer in src-tauri/src/algo (factor, tech
  signals, fuse/reconcile, sentiment scoring, capital-flow scoring, backtest
  hit metrics). Use when editing algo/*, decoupling compute from IO/business,
  or changing scoring/fuse/backtest口径.
---

# algo 纯算法层

## 必读

- `docs/algo/README.md`（边界定义）
- `docs/algo/api.md`
- 代码：`src-tauri/src/algo/`

## 边界（改前先对号）

**算算法（进 algo）：** 较复杂的「数据 → 结论」——得分、概率、方向、门控、命中统计；无网、无盘、无 IPC。

**算业务（留领域）：** 选哪些源、默认组合、拉数、缓存、拼 UI/DTO 文案、进度上报。

**混装时：** 拆 IO/编排与计算核；门面 re-export 保兼容（如 `message_sentiment`、`factor_model`）。

| 放这里 | 别放这里 |
|--------|----------|
| `factor` / `tech` / `fuse` / `sentiment` / `capital` 评分 / `backtest` 口径 | `ashare` HTTP、`capital_flow` fetch、`strategy` catalog、live 新闻 HTTP |
| 词表与打分权重 JSON | React / Zustand |

## 改代码清单

1. 新结论逻辑写在对应 `algo/*` 子模块，单测跟纯函数。
2. `strategy::evaluate_*` 只组装源并 `fuse`；单源技术打分用 `algo::tech`。
3. 资金流打分改 `algo::capital`；拉取仍改 `capital_flow`。
4. 回测出手线/命中累计改 `algo::backtest`；`run_compose` 只编排。
5. 破坏性变更同步 `docs/algo/api.md`。
6. 测试：`cargo test --manifest-path src-tauri/Cargo.toml algo::`

## 调用

```rust
use crate::algo::{fuse, eval_momentum, evaluate_as_of, HitCounters};
```
