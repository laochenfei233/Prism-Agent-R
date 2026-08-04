use tauri::State;

use crate::utils::error::AppError;

#[tauri::command]
pub async fn settings_save_provider_key(
    state: State<'_, crate::AppState>,
    provider_id: String,
    api_key: String,
) -> Result<(), AppError> {
    // TODO: Encrypt with AES-GCM before storing
    sqlx::query("UPDATE providers SET api_key_enc = ?, updated_at = ? WHERE id = ?")
        .bind(&api_key)
        .bind(chrono::Utc::now().timestamp_millis())
        .bind(&provider_id)
        .execute(&state.db.pool)
        .await?;
    Ok(())
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

    sqlx::query(
        "INSERT INTO providers (id, name, kind, base_url, api_key_enc, is_enabled, created_at, updated_at) VALUES (?, ?, ?, ?, ?, 1, ?, ?)"
    )
    .bind(&id)
    .bind(&name)
    .bind(&kind)
    .bind(&base_url)
    .bind(&api_key)
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
