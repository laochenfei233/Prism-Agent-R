use serde::Serialize;
use tauri::State;

use crate::data::models::ProviderRow;
use crate::data::settings::registry::{self, SettingKind, SettingSpec};
use crate::utils::crypto::{decrypt_key, encrypt_key};
use crate::utils::error::AppError;

/// 设置项 DTO：注册表定义 + 当前值（读 preferences，无记录回退默认）
#[derive(Serialize)]
pub struct SettingSpecDto {
    key: String,
    label: String,
    group: String,
    group_label: String,
    kind: String,
    default: serde_json::Value,
    value: serde_json::Value,
    description: String,
    options: Option<Vec<String>>,
    min: Option<f64>,
    max: Option<f64>,
    step: Option<f64>,
}

fn spec_to_dto(spec: SettingSpec, current: serde_json::Value) -> SettingSpecDto {
    SettingSpecDto {
        key: spec.key.to_string(),
        label: spec.label.to_string(),
        group: serde_json::to_value(spec.group)
            .map(|v| v.as_str().unwrap_or("").to_string())
            .unwrap_or_default(),
        group_label: spec.group.label().to_string(),
        kind: serde_json::to_value(spec.kind)
            .map(|v| v.as_str().unwrap_or("").to_string())
            .unwrap_or_default(),
        default: spec.default.clone(),
        value: current,
        description: spec.description.to_string(),
        options: spec.options.map(|o| o.iter().map(|s| s.to_string()).collect()),
        min: spec.min,
        max: spec.max,
        step: spec.step,
    }
}

async fn read_current(db: &crate::data::db::Database, spec: &SettingSpec) -> serde_json::Value {
    use crate::data::settings::prefs;
    match spec.kind {
        SettingKind::Bool => {
            serde_json::json!(prefs::get_bool(&db.pool, spec.key, spec.default.as_bool().unwrap_or(false)).await)
        }
        SettingKind::Int => {
            serde_json::json!(prefs::get_i64(&db.pool, spec.key, spec.default.as_i64().unwrap_or(0)).await)
        }
        SettingKind::Float => {
            serde_json::json!(prefs::get_f64(&db.pool, spec.key, spec.default.as_f64().unwrap_or(0.0)).await)
        }
        SettingKind::String | SettingKind::Select => {
            serde_json::json!(prefs::get_str(&db.pool, spec.key, spec.default.as_str().unwrap_or("")).await)
        }
    }
}

/// 返回全部已注册设置项（含当前值），前端据此按分组渲染
#[tauri::command]
pub async fn settings_get_all(
    state: State<'_, crate::AppState>,
) -> Result<Vec<SettingSpecDto>, AppError> {
    let mut out = Vec::new();
    for spec in registry::specs() {
        let current = read_current(&state.db, &spec).await;
        out.push(spec_to_dto(spec, current));
    }
    Ok(out)
}

/// 写入单个设置项（类型/范围校验后落 preferences）；未知 key 拒绝
#[tauri::command]
pub async fn settings_set(
    state: State<'_, crate::AppState>,
    key: String,
    value: serde_json::Value,
) -> Result<SettingSpecDto, AppError> {
    let spec = registry::spec_by_key(&key)
        .ok_or_else(|| AppError::Validation(format!("未知设置项: {key}")))?;
    let normalized = registry::validate(&spec, &value).map_err(AppError::Validation)?;
    let text = match spec.kind {
        SettingKind::Bool => normalized.as_bool().unwrap_or(false).to_string(),
        SettingKind::Int => normalized.as_i64().unwrap_or(0).to_string(),
        SettingKind::Float => normalized.as_f64().unwrap_or(0.0).to_string(),
        SettingKind::String | SettingKind::Select => normalized.as_str().unwrap_or("").to_string(),
    };
    crate::data::settings::prefs::set(&state.db.pool, &key, &text).await?;
    Ok(spec_to_dto(spec, normalized))
}

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

/// 更新 Provider 连接信息（Base URL / API Key / 名称 / 图标），对齐 Cherry Studio 连接设置可编辑
#[tauri::command]
pub async fn settings_update_provider(
    state: State<'_, crate::AppState>,
    provider_id: String,
    name: Option<String>,
    base_url: Option<String>,
    api_key: Option<String>,
    avatar: Option<String>,
) -> Result<(), AppError> {
    let now = chrono::Utc::now().timestamp_millis();

    if let Some(name) = name {
        if name.trim().is_empty() {
            return Err(AppError::Validation("名称不能为空".into()));
        }
        sqlx::query("UPDATE providers SET name = ?, updated_at = ? WHERE id = ?")
            .bind(name.trim())
            .bind(now)
            .bind(&provider_id)
            .execute(&state.db.pool)
            .await?;
    }
    if let Some(base_url) = base_url {
        let trimmed = base_url.trim();
        let stored = if trimmed.is_empty() { None } else { Some(trimmed.to_string()) };
        sqlx::query("UPDATE providers SET base_url = ?, updated_at = ? WHERE id = ?")
            .bind(stored)
            .bind(now)
            .bind(&provider_id)
            .execute(&state.db.pool)
            .await?;
    }
    if let Some(api_key) = api_key {
        let stored = if api_key.trim().is_empty() {
            String::new()
        } else {
            encrypt_key(api_key.trim())?
        };
        sqlx::query("UPDATE providers SET api_key_enc = ?, updated_at = ? WHERE id = ?")
            .bind(&stored)
            .bind(now)
            .bind(&provider_id)
            .execute(&state.db.pool)
            .await?;
    }
    if let Some(avatar) = avatar {
        let trimmed = avatar.trim();
        let stored = if trimmed.is_empty() { None } else { Some(trimmed.to_string()) };
        sqlx::query("UPDATE providers SET avatar = ?, updated_at = ? WHERE id = ?")
            .bind(stored)
            .bind(now)
            .bind(&provider_id)
            .execute(&state.db.pool)
            .await?;
    }

    Ok(())
}

/// 解密已加密的 key；失败（如旧明文数据）则原样返回，保持向后兼容
pub fn decrypt_provider_key(encoded: &str) -> String {
    decrypt_key(encoded).unwrap_or_else(|_| encoded.to_string())
}

/// 加密 key；失败时原样返回（空 key 不加密）
pub fn encrypt_provider_key(plain: &str) -> String {
    if plain.is_empty() {
        plain.to_string()
    } else {
        encrypt_key(plain).unwrap_or_else(|_| plain.to_string())
    }
}

#[tauri::command]
pub async fn settings_add_provider(
    state: State<'_, crate::AppState>,
    name: String,
    kind: String,
    base_url: Option<String>,
    api_key: Option<String>,
) -> Result<(), AppError> {
    // 去重检查：同名或同 URL 的 Provider 已存在则拒绝
    let name_trimmed = name.trim();
    let url_trimmed = base_url.as_deref().map(|u| u.trim());

    if let Some(url) = url_trimmed {
        if !url.is_empty() {
            let dup = sqlx::query_scalar::<_, String>(
                "SELECT id FROM providers WHERE base_url = ? LIMIT 1"
            )
            .bind(url)
            .fetch_optional(&state.db.pool)
            .await?;
            if dup.is_some() {
                return Err(AppError::Validation(format!("已存在相同 URL 的 Provider: {url}")));
            }
        }
    }

    let dup_name = sqlx::query_scalar::<_, String>(
        "SELECT id FROM providers WHERE name = ? LIMIT 1"
    )
    .bind(name_trimmed)
    .fetch_optional(&state.db.pool)
    .await?;
    if dup_name.is_some() {
        return Err(AppError::Validation(format!("已存在同名 Provider: {name_trimmed}")));
    }

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
    .bind(name_trimmed)
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
pub async fn model_delete(
    state: State<'_, crate::AppState>,
    id: String,
) -> Result<(), AppError> {
    let svc = crate::data::services::ModelService::new(state.db.pool.clone());
    svc.delete(&id).await
}

/// 将该模型所在 provider 下全部模型置为非默认，再将目标模型设为默认（事务）
#[tauri::command]
pub async fn model_set_default(
    state: State<'_, crate::AppState>,
    id: String,
) -> Result<(), AppError> {
    let svc = crate::data::services::ModelService::new(state.db.pool.clone());
    svc.set_default(&id).await
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
