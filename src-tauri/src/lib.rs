pub mod algo;
pub mod ashare;
pub mod backtest;
pub mod capital_flow;
pub mod cninfo;
pub mod commands;
pub mod factor_model;
pub mod market;
pub mod message_sentiment;
pub mod models;
pub mod monitor;
pub mod predictor;
pub mod screener;
pub mod strategy;

use monitor::{MonitorBackgroundService, new_shared};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(new_shared())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_background_service::init_with_service(
            || MonitorBackgroundService::new(),
        ))
        .invoke_handler(tauri::generate_handler![
            commands::load_stocks,
            commands::search_stocks,
            commands::analyze_stock,
            commands::predict_stock,
            commands::get_stock_klines,
            commands::get_stock_intraday,
            commands::backtest_stock,
            commands::list_algorithms,
            commands::list_strategy_sources,
            commands::default_strategy_compose,
            commands::default_strategy_compose_for_stock,
            commands::get_tushare_token_status,
            commands::set_tushare_token,
            commands::run_smart_screen,
            commands::default_screen_compose,
            commands::monitor_sync_config,
            commands::monitor_get_status,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|_app_handle, _event| {});
}
