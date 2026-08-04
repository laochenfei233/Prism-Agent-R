use sqlx::SqlitePool;

use crate::data::models::{ModelDto, ModelRow, ProviderDto, ProviderRow};
use crate::utils::error::AppError;

pub struct ModelService {
    pub pool: SqlitePool,
}

impl ModelService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_models(&self) -> Result<Vec<ModelDto>, AppError> {
        let rows = sqlx::query_as::<_, ModelRow>(
            "SELECT id, provider_id, model_id, display_name, kind, max_tokens, is_default, created_at FROM models ORDER BY is_default DESC, display_name"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    pub async fn list_providers(&self) -> Result<Vec<ProviderDto>, AppError> {
        let rows = sqlx::query_as::<_, ProviderRow>(
            "SELECT id, name, kind, base_url, api_key_enc, is_enabled, created_at, updated_at FROM providers ORDER BY name"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| ProviderDto {
            id: r.id,
            name: r.name,
            kind: r.kind,
            base_url: r.base_url,
            is_enabled: r.is_enabled != 0,
        }).collect())
    }

    pub async fn get_default_model(&self) -> Result<Option<ModelDto>, AppError> {
        let row = sqlx::query_as::<_, ModelRow>(
            "SELECT id, provider_id, model_id, display_name, kind, max_tokens, is_default, created_at FROM models WHERE is_default = 1 LIMIT 1"
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into()))
    }
}

impl From<ModelRow> for ModelDto {
    fn from(r: ModelRow) -> Self {
        Self {
            id: r.id,
            provider_id: r.provider_id,
            model_id: r.model_id,
            display_name: r.display_name,
            kind: r.kind,
            max_tokens: r.max_tokens,
            is_default: r.is_default != 0,
        }
    }
}
