//! 纯算法层：无 HTTP / 磁盘 / Tauri，只做「数据 → 结论」的计算。
//!
//! | 子模块 | 输入 → 输出 |
//! |--------|-------------|
//! | [`stats`] | K 线 → 波动率 |
//! | [`factor`] | K 线 → 技术多因子得分 |
//! | [`tech`] | K 线 → 动量/均值回归/量价信号 |
//! | [`fuse`] | 多源贡献 → 融合概率 / 门控 |
//! | [`sentiment`] | 标题(+档案) → 消息情绪分 |
//! | [`capital`] | 资金流归档 → 方向强度 |
//! | [`backtest`] | 预测 vs 实际 → 命中统计口径 |
//!
//! 领域编排（`strategy` / `predictor` / `backtest` / `capital_flow`）负责 IO 与用例；
//! 行情 HTTP 在 `ashare`。详见 `docs/algo/`。

pub mod backtest;
pub mod capital;
pub mod factor;
pub mod fuse;
pub mod sentiment;
pub mod stats;
pub mod tech;

pub use backtest::{classify_change, pct, round2, HitCounters, ACTIONABLE_LEAD};
pub use capital::{evaluate_as_of, CapitalFlowArchive, CapitalFlowSignal};
pub use factor::{
    clamp_lookback, compute, compute_styled, compute_styled_for_horizon, style_for_stock,
    take_lookback, FactorSnapshot, FactorStyle, MIN_BARS,
};
pub use fuse::{
    contrib, contrib_soft, fuse, neutral, probs_from_score, probs_from_score_soft,
    reconcile_index_factor_capital, reconcile_index_factor_message, reconcile_index_momentum,
    reconcile_multiday_noise, EnsembleSignal,
};
pub use sentiment::{
    classify, profile_for, score_titles, score_titles_dated, MessageKind, MessageProfile,
};
pub use stats::calc_volatility;
pub use tech::{eval_factor, eval_mean_reversion, eval_momentum, eval_volume};
