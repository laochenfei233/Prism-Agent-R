use tauri::State;

use crate::data::models::{
    WikiDto, WikiPageDto, WikiPageHitDto, WikiPage, WikiPageHit, WikiRow,
};
use crate::data::services::WikiService;
use crate::utils::error::AppError;

// ── 转换辅助函数 ──────────────────────────────────────────

impl From<WikiRow> for WikiDto {
    fn from(row: WikiRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            description: row.description,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl From<WikiPage> for WikiPageDto {
    fn from(page: WikiPage) -> Self {
        Self {
            path: page.path,
            title: page.title,
            size: page.size,
        }
    }
}

impl From<WikiPageHit> for WikiPageHitDto {
    fn from(hit: WikiPageHit) -> Self {
        Self {
            path: hit.path,
            title: hit.title,
            snippet: hit.snippet,
            score: hit.score,
        }
    }
}

// ── Wiki 命令 ──────────────────────────────────────────────

#[tauri::command]
pub async fn wiki_create(
    state: State<'_, crate::AppState>,
    name: String,
    description: Option<String>,
) -> Result<WikiDto, AppError> {
    let svc = WikiService::new(state.db.clone());
    let row = svc.create_wiki(&name, description.as_deref()).await?;
    Ok(row.into())
}

#[tauri::command]
pub async fn wiki_list(
    state: State<'_, crate::AppState>,
) -> Result<Vec<WikiDto>, AppError> {
    let svc = WikiService::new(state.db.clone());
    let rows = svc.list_wikis().await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

#[tauri::command]
pub async fn wiki_get(
    state: State<'_, crate::AppState>,
    id: String,
) -> Result<WikiDto, AppError> {
    let svc = WikiService::new(state.db.clone());
    let row = svc.get_wiki(&id).await?;
    Ok(row.into())
}

#[tauri::command]
pub async fn wiki_delete(
    state: State<'_, crate::AppState>,
    id: String,
) -> Result<(), AppError> {
    let svc = WikiService::new(state.db.clone());
    svc.delete_wiki(&id).await
}

#[tauri::command]
pub async fn wiki_read_page(
    state: State<'_, crate::AppState>,
    wiki_id: String,
    path: String,
) -> Result<String, AppError> {
    let svc = WikiService::new(state.db.clone());
    svc.read_page(&wiki_id, &path).await
}

#[tauri::command]
pub async fn wiki_write_page(
    state: State<'_, crate::AppState>,
    wiki_id: String,
    path: String,
    content: String,
) -> Result<(), AppError> {
    let svc = WikiService::new(state.db.clone());
    svc.write_page(&wiki_id, &path, &content).await
}

#[tauri::command]
pub async fn wiki_list_pages(
    state: State<'_, crate::AppState>,
    wiki_id: String,
) -> Result<Vec<WikiPageDto>, AppError> {
    let svc = WikiService::new(state.db.clone());
    let pages = svc.list_pages(&wiki_id).await?;
    Ok(pages.into_iter().map(Into::into).collect())
}

#[tauri::command]
pub async fn wiki_search(
    state: State<'_, crate::AppState>,
    wiki_id: String,
    query: String,
) -> Result<Vec<WikiPageHitDto>, AppError> {
    let svc = WikiService::new(state.db.clone());
    let hits = svc.search_pages(&wiki_id, &query).await?;
    Ok(hits.into_iter().map(Into::into).collect())
}

/// §10.1.1 AI 写入：preview=true 返回计划不执行
#[tauri::command]
pub async fn wiki_write_ai(
    state: State<'_, crate::AppState>,
    wiki_id: String,
    info: String,
    preview: Option<bool>,
) -> Result<serde_json::Value, AppError> {
    let svc = WikiService::new(state.db.clone());
    svc.write_ai(&wiki_id, &info, preview.unwrap_or(true)).await
}

/// §10.1.1 用户确认计划后执行
#[tauri::command]
pub async fn wiki_apply_plan(
    state: State<'_, crate::AppState>,
    wiki_id: String,
    plan: crate::data::models::WikiWritePlan,
) -> Result<crate::data::models::WikiWriteResult, AppError> {
    let svc = WikiService::new(state.db.clone());
    svc.apply_plan(&wiki_id, &plan).await
}
