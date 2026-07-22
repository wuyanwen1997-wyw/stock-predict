//! 行情门面：兼容旧 `crate::market::…` 引用，实现已下沉到 [`crate::ashare`]。

pub use crate::ashare::{
    apply_quote, calc_volatility, fetch_daily_klines, fetch_hot_stock_codes, fetch_hot_stocks,
    fetch_intraday_trends, fetch_klines, fetch_stock_quotes, infer_market, search_stocks, to_secid,
    to_sina_symbol, to_tencent_symbol,
};
