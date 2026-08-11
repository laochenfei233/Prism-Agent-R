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
            has_key: r.api_key_enc.as_deref().map(str::trim).filter(|s| !s.is_empty()).is_some(),
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

    /// 删除模型
    pub async fn delete(&self, id: &str) -> Result<(), AppError> {
        sqlx::query("DELETE FROM models WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// 将该模型所在 provider 下全部模型置为非默认，再将目标模型设为默认（事务）
    pub async fn set_default(&self, id: &str) -> Result<(), AppError> {
        let provider_id: Option<String> = sqlx::query_scalar("SELECT provider_id FROM models WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        let Some(provider_id) = provider_id else {
            return Err(AppError::Internal(format!("模型不存在: {id}")));
        };

        let mut tx = self.pool.begin().await?;
        sqlx::query("UPDATE models SET is_default = 0 WHERE provider_id = ?")
            .bind(&provider_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE models SET is_default = 1 WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn temp_db() -> (crate::data::Database, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("prism_model_{}", uuid::Uuid::new_v4()));
        let db = crate::data::db::Database::new(&dir).await.unwrap();
        (db, dir)
    }

    async fn insert_provider(pool: &sqlx::SqlitePool, id: &str) {
        sqlx::query(
            "INSERT INTO providers (id, name, kind, base_url, is_enabled, created_at, updated_at) VALUES (?, ?, 'openai', 'http://localhost', 1, ?, ?)"
        )
        .bind(id)
        .bind(id)
        .bind(chrono::Utc::now().timestamp_millis())
        .bind(chrono::Utc::now().timestamp_millis())
        .execute(pool)
        .await
        .unwrap();
    }

    async fn insert_model(pool: &sqlx::SqlitePool, provider_id: &str, model_id: &str, is_default: bool) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO models (id, provider_id, model_id, display_name, kind, max_tokens, is_default, created_at) VALUES (?, ?, ?, ?, 'chat', 8192, ?, ?)"
        )
        .bind(&id)
        .bind(provider_id)
        .bind(model_id)
        .bind(model_id)
        .bind(is_default as i32)
        .bind(chrono::Utc::now().timestamp_millis())
        .execute(pool)
        .await
        .unwrap();
        id
    }

    /// 设默认：同 provider 其他模型全部取消默认，目标模型置默认
    #[tokio::test]
    async fn set_default_clears_siblings_and_sets_target() {
        let (db, dir) = temp_db().await;
        let svc = ModelService::new(db.pool.clone());

        insert_provider(&db.pool, "p1").await;
        insert_provider(&db.pool, "p2").await;
        let a = insert_model(&db.pool, "p1", "model-a", true).await;
        let b = insert_model(&db.pool, "p1", "model-b", false).await;
        let c = insert_model(&db.pool, "p2", "model-c", true).await; // 另一 provider 不受影响

        svc.set_default(&b).await.unwrap();

        let models = svc.list_models().await.unwrap();
        let a_row = models.iter().find(|m| m.id == a).unwrap();
        let b_row = models.iter().find(|m| m.id == b).unwrap();
        let c_row = models.iter().find(|m| m.id == c).unwrap();
        assert!(!a_row.is_default, "同 provider 旧默认应被清除");
        assert!(b_row.is_default, "目标模型应置默认");
        assert!(c_row.is_default, "其他 provider 默认不受影响");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 设默认：不存在的模型应报错
    #[tokio::test]
    async fn set_default_unknown_id_errors() {
        let (db, dir) = temp_db().await;
        let svc = ModelService::new(db.pool.clone());
        assert!(svc.set_default("no-such-model").await.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 删除：删除后列表不再包含该模型
    #[tokio::test]
    async fn delete_removes_model() {
        let (db, dir) = temp_db().await;
        let svc = ModelService::new(db.pool.clone());
        insert_provider(&db.pool, "p1").await;
        let id = insert_model(&db.pool, "p1", "model-a", false).await;

        svc.delete(&id).await.unwrap();
        let models = svc.list_models().await.unwrap();
        assert!(!models.iter().any(|m| m.id == id));
        let _ = std::fs::remove_dir_all(&dir);
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
