use serde::Serialize;
use tauri::State;

use crate::data::models::ProviderRow;
use crate::utils::crypto::{decrypt_key, encrypt_key};
use crate::utils::error::AppError;

#[tauri::command]
pub async fn settings_save_provider_key(
    state: State<'_, crate::AppState>,
    provider_id: String,
    api_key: String,
) -> Result<(), AppError> {
    // 非空 key 用 AES-GCM 加密后存储；空 key 保持原逻辑（清空）
    let stored = if api_key.is_empty() {
        api_key
    } else {
        encrypt_key(&api_key)?
    };
    sqlx::query("UPDATE providers SET api_key_enc = ?, updated_at = ? WHERE id = ?")
        .bind(&stored)
        .bind(chrono::Utc::now().timestamp_millis())
        .bind(&provider_id)
        .execute(&state.db.pool)
        .await?;
    Ok(())
}

/// 解密已加密的 key；失败（如旧明文数据）则原样返回，保持向后兼容
pub fn decrypt_provider_key(encoded: &str) -> String {
    decrypt_key(encoded).unwrap_or_else(|_| encoded.to_string())
}

#[tauri::command]
pub async fn settings_add_provider(
    state: State<'_, crate::AppState>,
    name: String,
    kind: String,
    base_url: Option<String>,
    api_key: Option<String>,
) -> Result<(), AppError> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();

    // 非空 key 用 AES-GCM 加密后存储（§12 安全要求）
    let stored_key = match &api_key {
        Some(k) if !k.is_empty() => Some(encrypt_key(k)?),
        _ => api_key,
    };

    sqlx::query(
        "INSERT INTO providers (id, name, kind, base_url, api_key_enc, is_enabled, created_at, updated_at) VALUES (?, ?, ?, ?, ?, 1, ?, ?)"
    )
    .bind(&id)
    .bind(&name)
    .bind(&kind)
    .bind(&base_url)
    .bind(&stored_key)
    .bind(now)
    .bind(now)
    .execute(&state.db.pool)
    .await?;

    Ok(())
}

#[tauri::command]
pub async fn settings_add_model(
    state: State<'_, crate::AppState>,
    provider_id: String,
    model_id: String,
    display_name: Option<String>,
    is_default: Option<bool>,
) -> Result<(), AppError> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();

    sqlx::query(
        "INSERT INTO models (id, provider_id, model_id, display_name, kind, max_tokens, is_default, created_at) VALUES (?, ?, ?, ?, 'chat', 8192, ?, ?)"
    )
    .bind(&id)
    .bind(&provider_id)
    .bind(&model_id)
    .bind(&display_name)
    .bind(is_default.unwrap_or(false) as i32)
    .bind(now)
    .execute(&state.db.pool)
    .await?;

    Ok(())
}

#[derive(Serialize)]
pub struct ModelListResult {
    models: Vec<String>,
}

#[tauri::command]
pub async fn model_fetch_available(
    state: State<'_, crate::AppState>,
    provider_id: String,
) -> Result<ModelListResult, AppError> {
    let provider = sqlx::query_as::<_, ProviderRow>(
        "SELECT id, name, kind, base_url, api_key_enc, is_enabled, created_at, updated_at FROM providers WHERE id = ?"
    )
    .bind(&provider_id)
    .fetch_optional(&state.db.pool)
    .await?
    .ok_or_else(|| AppError::Internal("Provider not found".to_string()))?;

    let base_url = provider.base_url.unwrap_or_else(|| {
        match provider.kind.as_str() {
            "ollama" => "http://localhost:11434/v1".to_string(),
            _ => "https://api.openai.com/v1".to_string(),
        }
    });

    let api_key = provider
        .api_key_enc
        .as_deref()
        .map(decrypt_provider_key)
        .unwrap_or_default();
    let url = format!("{}/models", base_url);

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Request failed: {e}")))?;

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("Parse failed: {e}")))?;

    let mut models = Vec::new();
    if let Some(arr) = data["data"].as_array() {
        for item in arr {
            if let Some(id) = item["id"].as_str() {
                models.push(id.to_string());
            }
        }
    }

    models.sort();
    Ok(ModelListResult { models })
}
