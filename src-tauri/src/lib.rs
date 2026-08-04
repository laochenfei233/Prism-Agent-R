pub mod commands;
pub mod core;
pub mod data;
pub mod utils;

use data::Database;
use tauri::Manager;

pub struct AppState {
    pub db: Database,
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

            app.manage(AppState { db });
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
            commands::model::model_list,
            commands::model::model_providers,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
