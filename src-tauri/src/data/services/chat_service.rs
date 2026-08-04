use sqlx::SqlitePool;
use uuid::Uuid;

use crate::data::models::{MessageDto, MessageRow};
use crate::utils::error::AppError;

pub struct ChatService {
    pub pool: SqlitePool,
}

impl ChatService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn history(
        &self,
        session_id: &str,
        limit: Option<i64>,
    ) -> Result<Vec<MessageDto>, AppError> {
        let limit = limit.unwrap_or(50);
        let rows = sqlx::query_as::<_, MessageRow>(
            "SELECT id, session_id, role, content, tool_calls, tool_call_id, model_id, usage, created_at FROM messages WHERE session_id = ? ORDER BY created_at ASC LIMIT ?"
        )
        .bind(session_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    pub async fn save_message(
        &self,
        session_id: &str,
        role: &str,
        content: &str,
        tool_calls: Option<&str>,
        tool_call_id: Option<&str>,
        model_id: Option<&str>,
        usage: Option<&str>,
    ) -> Result<MessageDto, AppError> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp_millis();

        sqlx::query(
            "INSERT INTO messages (id, session_id, role, content, tool_calls, tool_call_id, model_id, usage, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(session_id)
        .bind(role)
        .bind(content)
        .bind(tool_calls)
        .bind(tool_call_id)
        .bind(model_id)
        .bind(usage)
        .bind(now)
        .execute(&self.pool)
        .await?;

        // Update session timestamp
        sqlx::query("UPDATE sessions SET updated_at = ? WHERE id = ?")
            .bind(now).bind(session_id).execute(&self.pool).await?;

        let row = sqlx::query_as::<_, MessageRow>(
            "SELECT id, session_id, role, content, tool_calls, tool_call_id, model_id, usage, created_at FROM messages WHERE id = ?"
        )
        .bind(&id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.into())
    }
}

impl From<MessageRow> for MessageDto {
    fn from(r: MessageRow) -> Self {
        let tool_calls = r.tool_calls.and_then(|tc| serde_json::from_str(&tc).ok());
        let usage = r.usage.and_then(|u| serde_json::from_str(&u).ok());
        Self {
            id: r.id,
            session_id: r.session_id,
            role: r.role,
            content: r.content,
            tool_calls,
            tool_call_id: r.tool_call_id,
            model_id: r.model_id,
            usage,
            created_at: r.created_at,
        }
    }
}
