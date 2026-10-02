use tauri::State;

use crate::data::models::{IngestResultDto, RagDocumentDto, RagHitDto};
use crate::data::services::rag_service::{embedding_status, EmbeddingConfig, RagService};
use crate::utils::error::AppError;

#[tauri::command]
pub async fn rag_ingest(
    state: State<'_, crate::AppState>,
    wiki_id: String,
    file_path: String,
) -> Result<IngestResultDto, AppError> {
    let mut svc = RagService::new(state.db.clone());
    svc.configure_from_db().await?;
    let result = svc.ingest(&wiki_id, &file_path).await?;
    Ok(IngestResultDto {
        document_id: result.document_id,
        chunk_count: result.chunk_count,
        status: result.status,
    })
}

#[tauri::command]
pub async fn rag_search(
    state: State<'_, crate::AppState>,
    wiki_id: String,
    query: String,
    top_k: Option<usize>,
) -> Result<Vec<RagHitDto>, AppError> {
    let mut svc = RagService::new(state.db.clone());
    svc.configure_from_db().await?;
    let default_top_k = crate::data::settings::prefs::get_i64(&state.db.pool, "rag.top_k", 5)
        .await
        .clamp(1, 20) as usize;
    let hits = svc
        .search(&wiki_id, &query, top_k.unwrap_or(default_top_k))
        .await?;
    Ok(hits
        .into_iter()
        .map(|h| RagHitDto {
            chunk_id: h.chunk_id,
            document_title: h.document_title,
            page_start: h.page_start,
            page_end: h.page_end,
            section: h.section,
            quote: h.quote,
            score: h.score,
        })
        .collect())
}

#[tauri::command]
pub async fn rag_list_documents(
    state: State<'_, crate::AppState>,
    wiki_id: String,
) -> Result<Vec<RagDocumentDto>, AppError> {
    let mut svc = RagService::new(state.db.clone());
    svc.configure_from_db().await?;
    svc.list_documents(&wiki_id).await
}

#[tauri::command]
pub async fn rag_delete_document(
    state: State<'_, crate::AppState>,
    doc_id: String,
) -> Result<(), AppError> {
    let mut svc = RagService::new(state.db.clone());
    svc.configure_from_db().await?;
    svc.delete_document(&doc_id).await
}

/// 设置嵌入器配置：mode = "local" | "api"
#[tauri::command]
pub async fn rag_embedding_config(
    state: State<'_, crate::AppState>,
    mode: String,
    provider_id: Option<String>,
    model: Option<String>,
    dim: Option<usize>,
) -> Result<serde_json::Value, AppError> {
    let mut svc = RagService::new(state.db.clone());
    let cfg = EmbeddingConfig {
        mode,
        provider_id,
        model,
        dim,
    };
    svc.set_config(&cfg).await?;
    embedding_status(&state.db).await
}

/// 当前嵌入器状态
#[tauri::command]
pub async fn rag_embedding_status(
    state: State<'_, crate::AppState>,
) -> Result<serde_json::Value, AppError> {
    embedding_status(&state.db).await
}

/// §10.2.2 Contextual Retrieval 开关
#[tauri::command]
pub async fn rag_contextual_config(
    state: State<'_, crate::AppState>,
    enabled: bool,
) -> Result<serde_json::Value, AppError> {
    let svc = RagService::new(state.db.clone());
    svc.set_contextual(enabled).await?;
    svc.contextual_status().await
}

/// 当前 Contextual Retrieval 状态
#[tauri::command]
pub async fn rag_contextual_status(
    state: State<'_, crate::AppState>,
) -> Result<serde_json::Value, AppError> {
    let svc = RagService::new(state.db.clone());
    svc.contextual_status().await
}

/// §10.2.2 reranker 开关
#[tauri::command]
pub async fn rag_rerank_config(
    state: State<'_, crate::AppState>,
    enabled: bool,
) -> Result<serde_json::Value, AppError> {
    let svc = RagService::new(state.db.clone());
    svc.set_rerank(enabled).await?;
    svc.rerank_status().await
}

/// 当前 reranker 状态
#[tauri::command]
pub async fn rag_rerank_status(
    state: State<'_, crate::AppState>,
) -> Result<serde_json::Value, AppError> {
    let svc = RagService::new(state.db.clone());
    svc.rerank_status().await
}

