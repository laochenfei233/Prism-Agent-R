//! Tauri command handlers for compose workflow.

use tauri::State;

use crate::core::compose::{ComposeEngine, ComposeSession};
use crate::utils::error::AppError;

/// Start a new compose session.
#[tauri::command]
pub async fn compose_start(
    app: tauri::AppHandle,
    state: State<'_, crate::AppState>,
    user_request: String,
    agent_id: String,
) -> Result<ComposeSession, AppError> {
    let engine = ComposeEngine {
        sessions: state.compose_sessions.clone(),
        cancels: state.compose_cancels.clone(),
    };
    engine
        .start(user_request, agent_id, &state.db.pool, &app)
        .await
}

/// Pause a running compose session.
#[tauri::command]
pub async fn compose_pause(
    state: State<'_, crate::AppState>,
    session_id: String,
) -> Result<(), AppError> {
    let engine = ComposeEngine {
        sessions: state.compose_sessions.clone(),
        cancels: state.compose_cancels.clone(),
    };
    engine.pause(&session_id).await
}

/// Resume a paused compose session.
#[tauri::command]
pub async fn compose_resume(
    app: tauri::AppHandle,
    state: State<'_, crate::AppState>,
    session_id: String,
) -> Result<(), AppError> {
    let engine = ComposeEngine {
        sessions: state.compose_sessions.clone(),
        cancels: state.compose_cancels.clone(),
    };
    engine.resume(&session_id, &state.db.pool, &app).await
}

/// Stop a compose session permanently.
#[tauri::command]
pub async fn compose_stop(
    state: State<'_, crate::AppState>,
    session_id: String,
) -> Result<(), AppError> {
    let engine = ComposeEngine {
        sessions: state.compose_sessions.clone(),
        cancels: state.compose_cancels.clone(),
    };
    engine.stop(&session_id).await
}

/// Get a compose session by ID.
#[tauri::command]
pub async fn compose_get(
    state: State<'_, crate::AppState>,
    session_id: String,
) -> Result<ComposeSession, AppError> {
    let engine = ComposeEngine {
        sessions: state.compose_sessions.clone(),
        cancels: state.compose_cancels.clone(),
    };
    engine.get(&session_id).await
}
