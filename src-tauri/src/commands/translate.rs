use tauri::State;

use crate::data::models::{DetectResultDto, TranslateHistoryResultDto, TranslateResultDto};
use crate::data::services::translate_service::TranslateService;
use crate::utils::error::AppError;

#[tauri::command]
pub async fn translate_translate(
    state: State<'_, crate::AppState>,
    text: String,
    source: Option<String>,
    target: String,
    model_id: Option<String>,
) -> Result<TranslateResultDto, AppError> {
    let svc = TranslateService::new(state.db.pool.clone(), state.translate_cache.clone());
    let result = svc
        .translate(&text, source.as_deref(), &target, model_id.as_deref())
        .await?;
    Ok(TranslateResultDto {
        translated: result.translated,
        source_lang: result.source_lang,
        from_cache: result.from_cache,
    })
}

#[tauri::command]
pub async fn translate_batch(
    state: State<'_, crate::AppState>,
    texts: Vec<String>,
    source: Option<String>,
    target: String,
) -> Result<Vec<TranslateResultDto>, AppError> {
    let svc = TranslateService::new(state.db.pool.clone(), state.translate_cache.clone());
    let results = svc.batch(&texts, source.as_deref(), &target).await?;
    Ok(results
        .into_iter()
        .map(|r| TranslateResultDto {
            translated: r.translated,
            source_lang: r.source_lang,
            from_cache: r.from_cache,
        })
        .collect())
}

/// §10.5.4 translate:file：读文件内容 → 整文件翻译（保留 Markdown 结构）
#[tauri::command]
pub async fn translate_file(
    state: State<'_, crate::AppState>,
    path: String,
    source: Option<String>,
    target: String,
) -> Result<String, AppError> {
    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| AppError::Validation(format!("文件不可读: {path}: {e}")))?;
    let svc = TranslateService::new(state.db.pool.clone(), state.translate_cache.clone());
    svc.translate_file(&content, source.as_deref(), &target)
        .await
}

#[tauri::command]
pub async fn translate_history(
    state: State<'_, crate::AppState>,
    query: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<TranslateHistoryResultDto, AppError> {
    let svc = TranslateService::new(state.db.pool.clone(), state.translate_cache.clone());
    let result = svc.history(query.as_deref(), limit, offset).await?;
    Ok(TranslateHistoryResultDto {
        items: result.items,
        total: result.total,
    })
}

#[tauri::command]
pub async fn translate_detect(
    state: State<'_, crate::AppState>,
    text: String,
) -> Result<DetectResultDto, AppError> {
    let svc = TranslateService::new(state.db.pool.clone(), state.translate_cache.clone());
    let result = svc.detect(&text).await?;
    Ok(DetectResultDto {
        lang: result.lang,
        confidence: result.confidence,
    })
}

/// 设置翻译专用模型（preferences: translate.model_id）；传空则回退默认模型
#[tauri::command]
pub async fn translate_model_config(
    state: State<'_, crate::AppState>,
    model_id: Option<String>,
) -> Result<serde_json::Value, AppError> {
    let svc = TranslateService::new(state.db.pool.clone(), state.translate_cache.clone());
    svc.set_model_config(model_id.as_deref()).await?;
    let current = svc.model_config().await?;
    Ok(serde_json::json!({ "model_id": current }))
}

/// 当前翻译模型配置
#[tauri::command]
pub async fn translate_model_status(
    state: State<'_, crate::AppState>,
) -> Result<serde_json::Value, AppError> {
    let svc = TranslateService::new(state.db.pool.clone(), state.translate_cache.clone());
    let configured = svc.model_config().await?;
    Ok(serde_json::json!({ "model_id": configured }))
}
