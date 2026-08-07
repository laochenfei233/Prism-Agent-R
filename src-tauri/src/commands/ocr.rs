use tauri::State;

use crate::data::models::{OcrProviderInfoDto, OcrResultDto};
use crate::data::services::ocr_service::OcrService;
use crate::utils::error::AppError;

#[tauri::command]
pub async fn ocr_recognize(
    state: State<'_, crate::AppState>,
    image_path: String,
    lang: Option<String>,
    provider: Option<String>,
) -> Result<OcrResultDto, AppError> {
    let svc = OcrService::new(state.db.pool.clone());
    let result = svc.recognize(&image_path, lang.as_deref(), provider.as_deref()).await?;
    Ok(OcrResultDto {
        text: result.text,
        lang: result.lang,
        provider: result.provider,
        blocks: result.blocks,
    })
}

#[tauri::command]
pub async fn ocr_providers(
    state: State<'_, crate::AppState>,
) -> Result<Vec<OcrProviderInfoDto>, AppError> {
    let svc = OcrService::new(state.db.pool.clone());
    Ok(svc
        .providers()
        .await
        .into_iter()
        .map(|p| OcrProviderInfoDto {
            name: p.name,
            kind: p.kind,
            available: p.available,
        })
        .collect())
}
