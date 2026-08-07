pub mod commands;
pub mod core;
pub mod data;
pub mod mcp;
pub mod utils;

use std::collections::HashMap;
use data::Database;
use tauri::Manager;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use mcp::McpRuntime;
use core::adk::tool::ToolApprovalStore;

pub struct AppState {
    pub db: Database,
    pub active_cancels: Mutex<HashMap<String, CancellationToken>>,
    pub mcp_runtime: std::sync::Arc<McpRuntime>,
    pub approval_store: std::sync::Arc<ToolApprovalStore>,
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

            let mcp_runtime = McpRuntime::new();

            // 启动时加载并连接所有 active 的 MCP 服务器
            {
                let db_clone = db.clone();
                let runtime_clone = mcp_runtime.clone();
                let _ = rt.spawn(async move {
                    let svc = data::services::McpService::new(db_clone, runtime_clone);
                    if let Err(e) = svc.load_all().await {
                        tracing::warn!("MCP load_all failed: {e}");
                    }
                });
            }

            app.manage(AppState {
                db,
                active_cancels: Mutex::new(HashMap::new()),
                mcp_runtime,
                approval_store: std::sync::Arc::new(ToolApprovalStore::new()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::agent::agent_list,
            commands::agent::agent_get,
            commands::agent::agent_create,
            commands::agent::agent_update,
            commands::agent::agent_delete,
            commands::agent::context_agent,
            commands::agent::session_inject_file,
            commands::session::session_list,
            commands::session::session_create,
            commands::session::session_rename,
            commands::session::session_delete,
            commands::session::session_search,
            commands::chat::chat_history,
            commands::chat::chat_send,
            commands::chat::chat_abort,
            commands::chat::tool_approval_respond,
            commands::model::model_list,
            commands::model::model_providers,
            commands::settings::settings_save_provider_key,
            commands::settings::settings_add_provider,
            commands::settings::settings_add_model,
            commands::settings::model_fetch_available,
            commands::mcp::mcp_list,
            commands::mcp::mcp_add,
            commands::mcp::mcp_update,
            commands::mcp::mcp_remove,
            commands::mcp::mcp_test,
            commands::mcp::mcp_tools,
            commands::mcp::mcp_status_all,
            commands::mcp::mcp_call_tool,
            commands::file::file_pick,
            commands::file::file_read_text,
            commands::file::file_write,
            commands::file::file_list,
            commands::file::file_parse,
            commands::skill::skill_list,
            commands::skill::skill_install,
            commands::skill::skill_uninstall,
            commands::skill::skill_toggle,
            commands::skill::skill_search_market,
            commands::skill::skill_install_market,
            commands::skill::skill_list_local,
            commands::workflow::workflow_list,
            commands::workflow::workflow_run,
            commands::workflow::workflow_stop,
            commands::workflow::workflow_result,
            commands::workflow::task_list_templates,
            commands::workflow::task_save_template,
            commands::workflow::task_run,
            commands::workflow::task_validate,
            commands::workflow::task_rerun,
            commands::workspace::workspace_get,
            commands::workspace::workspace_set,
            commands::workspace::workspace_tree,
            commands::workspace::workspace_read_file,
            commands::workspace::workspace_open_file,
            commands::workspace::workspace_write_instructions,
            commands::lsp::lsp_detect,
            commands::lsp::lsp_list,
            commands::lsp::lsp_start,
            commands::lsp::lsp_stop,
            commands::fs::fs_watch,
            commands::memory::memory_search,
            commands::memory::memory_read,
            commands::memory::memory_write,
            commands::memory::memory_context_dump,
            commands::memory::memory_reconcile,
            commands::dashboard::dashboard_overview,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
