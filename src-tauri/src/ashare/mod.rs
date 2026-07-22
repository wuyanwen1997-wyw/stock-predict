//! A 股行情基础设施：第三方源封装为稳定内部接口，供领域层使用。
//!
//! - [`quotes`] 实时报价
//! - [`kline`] 日/周/月/分钟 K 线与分时
//! - [`search`] 代码/名称搜索
//! - [`hot`] 多源人气榜
//! - [`symbol`] 代码映射
//!
//! 模块说明见仓库 `docs/ashare/`；Agent Skill 见 `.cursor/skills/ashare/`。

mod client;
mod hot;
mod kline;
mod quotes;
mod search;
mod symbol;

pub use hot::{fetch_hot_stock_codes, fetch_hot_stocks};
pub use kline::{calc_volatility, fetch_daily_klines, fetch_intraday_trends, fetch_klines};
pub use quotes::{apply_quote, fetch_stock_quotes};
pub use search::search_stocks;
pub use symbol::{infer_market, to_secid, to_sina_symbol, to_tencent_symbol};
