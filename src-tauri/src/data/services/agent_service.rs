use sqlx::SqlitePool;
use uuid::Uuid;

use crate::data::models::{AgentDto, AgentRow};
use crate::utils::error::AppError;

pub struct AgentService {
    pub pool: SqlitePool,
}

impl AgentService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(&self) -> Result<Vec<AgentDto>, AppError> {
        let rows = sqlx::query_as::<_, AgentRow>(
            "SELECT id, name, description, avatar, system_prompt, model_id, plan_model_id, small_model_id, temperature, max_tokens, disabled_tools, configuration, order_key, created_at, updated_at FROM agents ORDER BY order_key"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    pub async fn get(&self, id: &str) -> Result<AgentDto, AppError> {
        let row = sqlx::query_as::<_, AgentRow>(
            "SELECT id, name, description, avatar, system_prompt, model_id, plan_model_id, small_model_id, temperature, max_tokens, disabled_tools, configuration, order_key, created_at, updated_at FROM agents WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::AgentNotFound(id.to_string()))?;

        Ok(row.into())
    }

    pub async fn create(
        &self,
        name: &str,
        description: Option<&str>,
        system_prompt: Option<&str>,
        model_id: Option<&str>,
    ) -> Result<AgentDto, AppError> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp_millis();

        // 新建 Agent 默认参数（可配置，回退 0.7 / 8192）
        let temperature = crate::data::settings::prefs::get_f64(&self.pool, "agent.default.temperature", 0.7)
            .await
            .clamp(0.0, 2.0);
        let max_tokens = crate::data::settings::prefs::get_i64(&self.pool, "agent.default.max_tokens", 8192)
            .await
            .clamp(256, 128_000);

        sqlx::query(
            "INSERT INTO agents (id, name, description, system_prompt, model_id, temperature, max_tokens, disabled_tools, configuration, order_key, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, '[]', '{}', 0, ?, ?)"
        )
        .bind(&id)
        .bind(name)
        .bind(description)
        .bind(system_prompt)
        .bind(model_id)
        .bind(temperature)
        .bind(max_tokens)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        self.get(&id).await
    }

    pub async fn update(
        &self,
        id: &str,
        name: Option<&str>,
        description: Option<&str>,
        system_prompt: Option<&str>,
        model_id: Option<&str>,
    ) -> Result<AgentDto, AppError> {
        let now = chrono::Utc::now().timestamp_millis();

        if let Some(n) = name {
            sqlx::query("UPDATE agents SET name = ?, updated_at = ? WHERE id = ?")
                .bind(n).bind(now).bind(id).execute(&self.pool).await?;
        }
        if let Some(d) = description {
            sqlx::query("UPDATE agents SET description = ?, updated_at = ? WHERE id = ?")
                .bind(d).bind(now).bind(id).execute(&self.pool).await?;
        }
        if let Some(sp) = system_prompt {
            sqlx::query("UPDATE agents SET system_prompt = ?, updated_at = ? WHERE id = ?")
                .bind(sp).bind(now).bind(id).execute(&self.pool).await?;
        }
        if let Some(m) = model_id {
            sqlx::query("UPDATE agents SET model_id = ?, updated_at = ? WHERE id = ?")
                .bind(m).bind(now).bind(id).execute(&self.pool).await?;
        }

        self.get(id).await
    }

    pub async fn delete(&self, id: &str) -> Result<(), AppError> {
        sqlx::query("DELETE FROM agents WHERE id = ?")
            .bind(id).execute(&self.pool).await?;
        Ok(())
    }
}

impl From<AgentRow> for AgentDto {
    fn from(r: AgentRow) -> Self {
        let disabled_tools: Vec<String> = serde_json::from_str(&r.disabled_tools).unwrap_or_default();
        Self {
            id: r.id,
            name: r.name,
            description: r.description,
            avatar: r.avatar,
            system_prompt: r.system_prompt,
            model_id: r.model_id,
            temperature: r.temperature,
            max_tokens: r.max_tokens,
            disabled_tools,
            order_key: r.order_key,
        }
    }
}
