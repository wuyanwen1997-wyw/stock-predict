pub mod commands;
pub mod market;
pub mod models;
pub mod predictor;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::load_stocks,
            commands::search_stocks,
            commands::predict_stock,
            commands::list_algorithms,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|_app_handle, _event| {});
}
