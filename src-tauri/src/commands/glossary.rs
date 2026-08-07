use tauri::State;

use crate::data::models::{
    GlossaryTermDto, GlossaryTermInput, ImportResultDto,
};
use crate::data::services::glossary_service::GlossaryService;
use crate::utils::error::AppError;

#[tauri::command]
pub async fn glossary_list(
    state: State<'_, crate::AppState>,
    lang_pair: Option<String>,
) -> Result<Vec<GlossaryTermDto>, AppError> {
    let svc = GlossaryService::new(state.db.pool.clone());
    let terms = svc.list(lang_pair.as_deref()).await?;
    Ok(terms
        .into_iter()
        .map(|t| GlossaryTermDto {
            id: t.id,
            source_lang: t.source_lang,
            target_lang: t.target_lang,
            source_term: t.source_term,
            target_term: t.target_term,
            category: t.category,
            enabled: t.enabled,
        })
        .collect())
}

#[tauri::command]
pub async fn glossary_add(
    state: State<'_, crate::AppState>,
    term: GlossaryTermInput,
) -> Result<(), AppError> {
    let svc = GlossaryService::new(state.db.pool.clone());
    svc.add(term).await
}

#[tauri::command]
pub async fn glossary_remove(
    state: State<'_, crate::AppState>,
    id: String,
) -> Result<(), AppError> {
    let svc = GlossaryService::new(state.db.pool.clone());
    svc.remove(&id).await
}

#[tauri::command]
pub async fn glossary_import_csv(
    state: State<'_, crate::AppState>,
    path: String,
) -> Result<ImportResultDto, AppError> {
    let content = tokio::fs::read_to_string(&path).await?;
    let svc = GlossaryService::new(state.db.pool.clone());
    let result = svc.import_csv(&content).await?;
    Ok(ImportResultDto {
        imported: result.imported,
        failed: result.failed,
    })
}
