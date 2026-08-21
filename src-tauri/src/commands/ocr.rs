use tauri::State;

use crate::data::models::{OcrProviderInfoDto, OcrResultDto};
use crate::data::services::ocr_service::OcrService;
use crate::utils::error::AppError;

#[tauri::command]
pub async fn ocr_recognize(
    state: State<'_, crate::AppState>,
    image_path: Option<String>,
    image_data: Option<String>,
    lang: Option<String>,
    provider: Option<String>,
) -> Result<OcrResultDto, AppError> {
    let svc = OcrService::new(state.db.pool.clone());
    // data URL 优先（前端 FileReader 场景）；否则用磁盘路径
    let input = image_data.or(image_path).ok_or_else(|| {
        AppError::Validation("缺少图片输入：image_data 或 image_path 至少一个".into())
    })?;
    let result = svc
        .recognize_input(&input, lang.as_deref(), provider.as_deref())
        .await?;
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
