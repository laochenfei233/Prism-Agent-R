use tauri::State;

use crate::data::models::{IngestResultDto, RagDocumentDto, RagHitDto};
use crate::data::services::rag_service::{EmbeddingConfig, RagService, embedding_status};
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
    let hits = svc.search(&wiki_id, &query, top_k.unwrap_or(5)).await?;
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
    let cfg = EmbeddingConfig { mode, provider_id, model, dim };
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

// ── §10.2.5 RAG 多维评测 ─────────────────────────────────

/// 运行评测（rag:eval）：核心逻辑在 data::rag::eval::run_eval（CI 门槛共用）。
/// 完成后落库 rag_eval_reports，供 rag:eval-report 趋势对比。
#[tauri::command]
pub async fn rag_eval(
    state: State<'_, crate::AppState>,
    wiki_id: Option<String>,
    suite: Option<String>,
    top_k: Option<usize>,
) -> Result<crate::data::rag::eval::EvalReport, AppError> {
    use crate::data::rag::eval::{load_cases, run_eval, save_report};

    let mut cases = load_cases(&state.db, suite.as_deref()).await?;
    if let Some(wid) = &wiki_id {
        cases.retain(|c| &c.wiki_id == wid);
    }

    let suite = suite.unwrap_or_else(|| "default".into());
    let report = run_eval(&state.db, cases, top_k.unwrap_or(5), suite).await?;

    // 落库（趋势数据源）
    save_report(&state.db, &report).await?;
    Ok(report)
}

/// 添加评测用例（rag:eval-add）
#[tauri::command]
pub async fn rag_eval_add(
    state: State<'_, crate::AppState>,
    case: serde_json::Value,
) -> Result<String, AppError> {
    use crate::data::rag::eval::{add_case, ensure_wiki, EvalCase, EvalExpect};

    let id = case["id"].as_str().unwrap_or_default();
    let id = if id.is_empty() { uuid::Uuid::new_v4().to_string() } else { id.to_string() };
    let wiki_id = case["wiki_id"].as_str().ok_or_else(|| AppError::Validation("缺少 wiki_id".into()))?;
    let question = case["question"].as_str().ok_or_else(|| AppError::Validation("缺少 question".into()))?;
    let suite = case["suite"].as_str().unwrap_or("default");
    let expect: EvalExpect = if let Some(e) = case.get("expect") {
        serde_json::from_value(e.clone())?
    } else {
        EvalExpect::default()
    };

    ensure_wiki(&state.db, wiki_id).await?;
    add_case(&state.db, &EvalCase {
        id: id.clone(),
        wiki_id: wiki_id.into(),
        question: question.into(),
        expect,
        suite: suite.into(),
    })
    .await?;
    Ok(id)
}

/// 历史评测报告（rag:eval-report）
#[tauri::command]
pub async fn rag_eval_report(
    state: State<'_, crate::AppState>,
) -> Result<Vec<crate::data::rag::eval::EvalReport>, AppError> {
    crate::data::rag::eval::list_reports(&state.db).await
}
