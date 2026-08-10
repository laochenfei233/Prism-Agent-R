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
use core::autoagents::loop_scheduler::LoopScheduler;
use core::session::state::SessionStateManager;
use data::services::meeting::AudioStreamManager;

pub struct AppState {
    pub db: Database,
    pub active_cancels: Mutex<HashMap<String, CancellationToken>>,
    pub mcp_runtime: std::sync::Arc<McpRuntime>,
    pub approval_store: std::sync::Arc<ToolApprovalStore>,
    pub audio_streams: std::sync::Arc<AudioStreamManager>,
    pub session_state: std::sync::Arc<SessionStateManager>,
    pub loop_scheduler: std::sync::Arc<LoopScheduler>,
    /// 翻译短文本缓存（跨 IPC 调用共享，<500 字符，TTL 24h）
    pub translate_cache: std::sync::Arc<tokio::sync::Mutex<std::collections::HashMap<String, (String, i64)>>>,
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

            // 注册内置 ASR 后端（动态注册表，后续自定义后端可追加）
            data::services::asr::backends::builtin_register();

            // §10.2.1 项目级自动索引：启用状态下启动监听（工作目录从 preferences 读取）
            {
                let db_clone = db.clone();
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    data::services::project_index::start_if_enabled(db_clone, app_handle);
                });
            }

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
                audio_streams: std::sync::Arc::new(AudioStreamManager::new()),
                session_state: std::sync::Arc::new(SessionStateManager::new()),
                loop_scheduler: std::sync::Arc::new(LoopScheduler::new()),
                translate_cache: std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
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
            commands::session::session_init,
            commands::session::session_state_query,
            commands::session::session_cleanup,
            commands::session::session_fork,
            commands::session::session_approve,
            commands::chat::chat_history,
            commands::chat::chat_send,
            commands::chat::chat_abort,
            commands::chat::tool_approval_respond,
            commands::model::model_list,
            commands::model::model_providers,
            commands::settings::settings_save_provider_key,
            commands::settings::settings_add_provider,
            commands::settings::settings_add_model,
            commands::settings::model_delete,
            commands::settings::model_set_default,
            commands::settings::model_fetch_available,
            commands::settings::settings_get_all,
            commands::settings::settings_set,
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
            commands::workflow::goal_evaluate,
            commands::loop_cmd::loop_start,
            commands::loop_cmd::loop_stop,
            commands::loop_cmd::loop_list,
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
            commands::rag::rag_ingest,
            commands::rag::rag_search,
            commands::rag::rag_list_documents,
            commands::rag::rag_delete_document,
            commands::rag::rag_embedding_config,
            commands::rag::rag_embedding_status,
            commands::rag::rag_contextual_config,
            commands::rag::rag_contextual_status,
            commands::rag::rag_rerank_config,
            commands::rag::rag_rerank_status,
            commands::rag::rag_eval,
            commands::rag::rag_eval_add,
            commands::rag::rag_eval_report,
            commands::wiki::wiki_create,
            commands::wiki::wiki_list,
            commands::wiki::wiki_get,
            commands::wiki::wiki_delete,
            commands::wiki::wiki_read_page,
            commands::wiki::wiki_write_page,
            commands::wiki::wiki_list_pages,
            commands::wiki::wiki_search,
            commands::wiki::wiki_write_ai,
            commands::wiki::wiki_apply_plan,
            commands::wiki::wiki_ingest_ai,
            commands::wiki::wiki_restore_trash,
            commands::wiki::wiki_open_page,
            commands::translate::translate_translate,
            commands::translate::translate_batch,
            commands::translate::translate_file,
            commands::translate::translate_history,
            commands::translate::translate_detect,
            commands::translate::translate_model_config,
            commands::translate::translate_model_status,
            commands::glossary::glossary_list,
            commands::glossary::glossary_add,
            commands::glossary::glossary_remove,
            commands::glossary::glossary_update,
            commands::glossary::glossary_import_csv,
            commands::glossary::glossary_builtin_list,
            commands::glossary::glossary_import_builtin,
            commands::ocr::ocr_recognize,
            commands::ocr::ocr_providers,
            commands::meeting::meeting_create,
            commands::meeting::meeting_list,
            commands::meeting::meeting_get,
            commands::meeting::meeting_delete,
            commands::meeting::meeting_update_transcript,
            commands::meeting::meeting_get_transcript,
            commands::meeting::meeting_summary,
            commands::meeting::meeting_export,
            commands::meeting::meeting_export_translation,
            commands::meeting::meeting_clean,
            commands::meeting::meeting_qa,
            commands::meeting::meeting_push_to_agent,
            commands::meeting::meeting_retranscribe,
            commands::asr::asr_list_configs,
            commands::asr::asr_save_config,
            commands::asr::asr_delete_config,
            commands::asr::asr_backends,
            commands::asr::asr_model_catalog,
            commands::asr::asr_model_installed,
            commands::asr::asr_model_download,
            commands::asr::asr_model_remove,
            commands::asr::asr_test,
            commands::asr::meeting_start_recording,
            commands::asr::meeting_audio_chunk,
            commands::asr::meeting_stop_recording,
            commands::asr::meeting_pause_recording,
            commands::asr::meeting_resume_recording,
            commands::asr::meeting_cancel_recording,
            commands::trace::trace_list,
            commands::trace::trace_grade,
            commands::router::router_route,
            commands::router::router_index_status,
            commands::agent_eval::agent_judge_evaluate,
            commands::agent_eval::agent_judge_compare,
            commands::agent_eval::agent_stats,
            commands::project_index::project_index_status,
            commands::project_index::project_index_toggle,
            commands::project_index::project_index_reindex,
            commands::tts::tts_speak,
            commands::tts::tts_stop,
            commands::tts::tts_voices,
            commands::dashboard::dashboard_overview,
            commands::search::search_config,
            commands::search::search_config_save,
            commands::search::search_test,
            commands::monitor::budget_get_config,
            commands::monitor::budget_get_status,
            commands::monitor::exception_list,
            commands::monitor::exception_resolve,
            commands::monitor::monitor_get_budget,
            commands::monitor::monitor_get_exceptions,
            commands::monitor::guardrail_check_tool,
            commands::monitor::orchestrator_start,
            commands::monitor::orchestrator_resume,
            commands::monitor::orchestrator_pause,
            commands::monitor::orchestrator_stop,
            commands::monitor::orchestrator_list,
            commands::monitor::exception_clear,
            commands::monitor::log_export,
            commands::monitor::model_switch_list,
            commands::monitor::workflow_pause,
            commands::monitor::workflow_resume,
            commands::monitor::monitor_list_active_workflows,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
