use sqlx::SqlitePool;
use uuid::Uuid;

use crate::data::models::{SessionDto, SessionRow};
use crate::utils::error::AppError;

pub struct SessionService {
    pub pool: SqlitePool,
}

impl SessionService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(&self, agent_id: Option<&str>) -> Result<Vec<SessionDto>, AppError> {
        let rows = if let Some(aid) = agent_id {
            sqlx::query_as::<_, SessionRow>(
                "SELECT id, agent_id, title, pinned, created_at, updated_at FROM sessions WHERE agent_id = ? ORDER BY updated_at DESC"
            )
            .bind(aid)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, SessionRow>(
                "SELECT id, agent_id, title, pinned, created_at, updated_at FROM sessions ORDER BY updated_at DESC"
            )
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    pub async fn create(&self, agent_id: &str, title: Option<&str>) -> Result<SessionDto, AppError> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp_millis();

        sqlx::query(
            "INSERT INTO sessions (id, agent_id, title, pinned, created_at, updated_at) VALUES (?, ?, ?, 0, ?, ?)"
        )
        .bind(&id)
        .bind(agent_id)
        .bind(title)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        self.get(&id).await
    }

    pub async fn get(&self, id: &str) -> Result<SessionDto, AppError> {
        let row = sqlx::query_as::<_, SessionRow>(
            "SELECT id, agent_id, title, pinned, created_at, updated_at FROM sessions WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::SessionNotFound(id.to_string()))?;

        Ok(row.into())
    }

    pub async fn rename(&self, id: &str, title: &str) -> Result<SessionDto, AppError> {
        let now = chrono::Utc::now().timestamp_millis();
        sqlx::query("UPDATE sessions SET title = ?, updated_at = ? WHERE id = ?")
            .bind(title).bind(now).bind(id).execute(&self.pool).await?;
        self.get(id).await
    }

    pub async fn delete(&self, id: &str) -> Result<(), AppError> {
        sqlx::query("DELETE FROM sessions WHERE id = ?")
            .bind(id).execute(&self.pool).await?;
        Ok(())
    }
}

impl From<SessionRow> for SessionDto {
    fn from(r: SessionRow) -> Self {
        Self {
            id: r.id,
            agent_id: r.agent_id,
            title: r.title,
            pinned: r.pinned != 0,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}
