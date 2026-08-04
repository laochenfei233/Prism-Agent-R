pub mod commands;
pub mod core;
pub mod data;
pub mod utils;

use std::collections::HashMap;
use data::Database;
use tauri::Manager;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

pub struct AppState {
    pub db: Database,
    pub active_cancels: Mutex<HashMap<String, CancellationToken>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    utils::logger::init_logger();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir().expect("failed to get app data dir");
            let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
            let db = rt.block_on(Database::new(&app_data_dir)).expect("failed to init database");

            app.manage(AppState {
                db,
                active_cancels: Mutex::new(HashMap::new()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::agent::agent_list,
            commands::agent::agent_get,
            commands::agent::agent_create,
            commands::agent::agent_update,
            commands::agent::agent_delete,
            commands::session::session_list,
            commands::session::session_create,
            commands::session::session_rename,
            commands::session::session_delete,
            commands::chat::chat_history,
            commands::chat::chat_send,
            commands::chat::chat_abort,
            commands::model::model_list,
            commands::model::model_providers,
            commands::settings::settings_save_provider_key,
            commands::settings::settings_add_provider,
            commands::settings::settings_add_model,
            commands::settings::model_fetch_available,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
