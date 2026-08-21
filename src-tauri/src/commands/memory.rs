use tauri::State;

use crate::data::services::memory_service::{MemoryDump, MemorySearchHit, MemoryService};
use crate::utils::error::AppError;

// ── 记忆命令 ──────────────────────────────────────────────

#[tauri::command]
pub async fn memory_search(
    state: State<'_, crate::AppState>,
    query: String,
    scope: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<MemorySearchHit>, AppError> {
    let base_dir = crate::utils::paths::memory_dir();
    let svc = MemoryService::new(state.db.clone(), base_dir);
    let mut hits = svc.search(&query).await?;

    // 按 scope 过滤
    if let Some(scope) = &scope {
        hits.retain(|h| &h.scope == scope);
    }

    // 限制数量
    if let Some(limit) = limit {
        hits.truncate(limit);
    }

    Ok(hits)
}

#[tauri::command]
pub async fn memory_reconcile(state: State<'_, crate::AppState>) -> Result<u64, AppError> {
    let base_dir = crate::utils::paths::memory_dir();
    let svc = MemoryService::new(state.db.clone(), base_dir);
    svc.reconcile().await
}

#[tauri::command]
pub async fn memory_read(
    state: State<'_, crate::AppState>,
    path: String,
) -> Result<String, AppError> {
    let base_dir = crate::utils::paths::memory_dir();
    let svc = MemoryService::new(state.db.clone(), base_dir);
    svc.read(&path).await
}

#[tauri::command]
pub async fn memory_write(
    state: State<'_, crate::AppState>,
    path: String,
    content: String,
) -> Result<(), AppError> {
    let base_dir = crate::utils::paths::memory_dir();
    let svc = MemoryService::new(state.db.clone(), base_dir);
    svc.write(&path, &content).await
}

#[tauri::command]
pub async fn memory_context_dump(
    state: State<'_, crate::AppState>,
) -> Result<Vec<MemoryDump>, AppError> {
    let base_dir = crate::utils::paths::memory_dir();
    let svc = MemoryService::new(state.db.clone(), base_dir);
    svc.context_dump().await
}
