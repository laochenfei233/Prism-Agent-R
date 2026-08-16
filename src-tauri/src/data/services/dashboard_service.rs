use crate::core::session::SessionLifecycle;
use crate::data::models::*;
use crate::data::Database;
use crate::utils::error::AppError;
use sqlx::Row;

pub struct DashboardService {
    db: Database,
}

impl DashboardService {
    pub fn new(db: &Database) -> Self {
        Self { db: db.clone() }
    }

    pub async fn overview(&self) -> Result<DashboardOverview, AppError> {
        let agents = self.load_agents().await?;
        let usage = self.load_usage().await?;
        let usage_trend = self.load_usage_trend().await?;
        let skills = self.load_skills().await?;
        let mcp_servers = self.load_mcp_status().await?;
        let recent_sessions = self.load_recent_sessions().await?;
        let models = self.load_models().await?;

        Ok(DashboardOverview {
            agents,
            usage,
            usage_trend,
            skills,
            mcp_servers,
            recent_sessions,
            models,
        })
    }

    pub async fn kanban(
        &self,
        session_state: &std::sync::Arc<crate::core::session::state::SessionStateManager>,
    ) -> Result<KanbanData, AppError> {
        // 1. Load all agents with system_prompt configured
        let agent_rows = sqlx::query(
            r#"
            SELECT
                a.id,
                a.name,
                COALESCE(a.description, '') AS description,
                a.avatar,
                a.order_key,
                m.display_name AS model_name
            FROM agents a
            LEFT JOIN models m ON m.id = a.model_id
            WHERE a.system_prompt IS NOT NULL AND a.system_prompt != ''
            ORDER BY a.order_key
            "#,
        )
        .fetch_all(&self.db.pool)
        .await?;

        let mut idle = Vec::new();
        let mut running = Vec::new();
        let mut done = Vec::new();
        let mut failed = Vec::new();

        for row in &agent_rows {
            let agent_id: String = row.get("id");
            let agent_name: String = row.get("name");
            let agent_avatar: Option<String> = row.get("avatar");
            let model_name: Option<String> = row.get("model_name");

            // 2. For each agent, find their most recent session
            let session_row = sqlx::query(
                r#"
                SELECT
                    s.id,
                    COALESCE(s.title, '新会话') AS title,
                    s.updated_at,
                    (SELECT COUNT(*) FROM messages m WHERE m.session_id = s.id) AS message_count
                FROM sessions s
                WHERE s.agent_id = ?
                ORDER BY s.updated_at DESC
                LIMIT 1
                "#,
            )
            .bind(&agent_id)
            .fetch_optional(&self.db.pool)
            .await?;

            let (session_id, session_title, session_updated_at, message_count): (
                Option<String>,
                Option<String>,
                Option<i64>,
                i64,
            ) = match session_row {
                Some(r) => (
                    Some(r.get("id")),
                    Some(r.get("title")),
                    Some(r.get("updated_at")),
                    r.get::<i64, _>("message_count"),
                ),
                None => (None, None, None, 0),
            };

            // 3. Query session_state to get SessionLifecycle
            let lifecycle = match &session_id {
                Some(sid) => session_state.get_state(sid).await,
                None => SessionLifecycle::Created,
            };

            // 4 & 5. Map lifecycle to kanban column
            let column = match lifecycle {
                SessionLifecycle::Init
                | SessionLifecycle::Running
                | SessionLifecycle::Verifying
                | SessionLifecycle::Paused => "running",
                SessionLifecycle::Done => "done",
                SessionLifecycle::InitFailed => "failed",
                SessionLifecycle::Created | SessionLifecycle::Ready => {
                    // Could be genuinely Created/Ready, or not in state manager (app restarted).
                    // If the session has messages, it was likely completed before restart.
                    if message_count > 0 {
                        "done"
                    } else {
                        "idle"
                    }
                }
            };

            let card = KanbanCard {
                agent_id,
                agent_name,
                agent_avatar,
                model_name,
                session_id,
                session_title,
                session_updated_at,
                lifecycle: format!("{:?}", lifecycle),
                message_count,
            };

            match column {
                "running" => running.push(card),
                "done" => done.push(card),
                "failed" => failed.push(card),
                _ => idle.push(card),
            }
        }

        Ok(KanbanData {
            idle,
            running,
            done,
            failed,
        })
    }

    async fn load_agents(&self) -> Result<Vec<AgentSummary>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT
                a.id,
                a.name,
                COALESCE(a.description, '') AS description,
                a.avatar,
                a.order_key,
                m.display_name AS model_name,
                (SELECT COUNT(*) FROM agent_skills WHERE agent_id = a.id) AS skill_count,
                (SELECT COUNT(*) FROM agent_mcp_servers WHERE agent_id = a.id) AS mcp_count,
                (SELECT MAX(s.updated_at) FROM sessions s WHERE s.agent_id = a.id) AS last_used
            FROM agents a
            LEFT JOIN models m ON m.id = a.model_id
            ORDER BY a.order_key
            "#,
        )
        .fetch_all(&self.db.pool)
        .await?;

        let mut result = Vec::new();
        for row in rows {
            result.push(AgentSummary {
                id: row.get("id"),
                name: row.get("name"),
                description: row.get("description"),
                avatar: row.get("avatar"),
                model_name: row.get("model_name"),
                skill_count: row.get::<i64, _>("skill_count") as usize,
                mcp_count: row.get::<i64, _>("mcp_count") as usize,
                last_used: row.get::<Option<i64>, _>("last_used").map(|ts| ts.to_string()),
                order_key: row.get("order_key"),
            });
        }
        Ok(result)
    }

    async fn load_usage(&self) -> Result<UsageStats, AppError> {
        let now = chrono::Utc::now().timestamp_millis();
        let today_start = now - (now % (24 * 60 * 60 * 1000));
        let week_start = today_start - 6 * 24 * 60 * 60 * 1000;
        let month_start = today_start - 29 * 24 * 60 * 60 * 1000;

        let rows = sqlx::query("SELECT usage FROM messages WHERE usage IS NOT NULL AND created_at >= ?")
            .bind(month_start)
            .fetch_all(&self.db.pool)
            .await?;

        let mut today_tokens: u64 = 0;
        let mut week_tokens: u64 = 0;
        let mut month_tokens: u64 = 0;
        let mut month_cost: f64 = 0.0;
        let mut today_calls: u64 = 0;

        for row in &rows {
            let usage_str: String = row.get("usage");
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&usage_str) {
                let prompt = val.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                let completion = val.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                let tokens = prompt + completion;
                let cost = val.get("cost").and_then(|v| v.as_f64()).unwrap_or(0.0);
                month_tokens += tokens;
                month_cost += cost;
            }
        }

        // Get per-day breakdown for today and week
        let day_rows = sqlx::query(
            r#"
            SELECT
                created_at,
                usage
            FROM messages
            WHERE usage IS NOT NULL AND created_at >= ?
            "#,
        )
        .bind(week_start)
        .fetch_all(&self.db.pool)
        .await?;

        for row in &day_rows {
            let ts: i64 = row.get("created_at");
            let usage_str: String = row.get("usage");
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&usage_str) {
                let tokens = val.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0)
                    + val.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                if ts >= today_start {
                    today_tokens += tokens;
                    today_calls += 1;
                }
                if ts >= week_start {
                    week_tokens += tokens;
                }
            }
        }

        Ok(UsageStats {
            today_tokens,
            week_tokens,
            month_tokens,
            month_cost,
            today_calls,
        })
    }

    async fn load_usage_trend(&self) -> Result<Vec<UsagePoint>, AppError> {
        let now = chrono::Utc::now().timestamp_millis();
        let seven_days_ago = now - 7 * 24 * 60 * 60 * 1000;

        let rows = sqlx::query(
            r#"
            SELECT created_at, usage
            FROM messages
            WHERE usage IS NOT NULL AND created_at >= ?
            "#,
        )
        .bind(seven_days_ago)
        .fetch_all(&self.db.pool)
        .await?;

        // Aggregate by date
        let mut daily: std::collections::HashMap<String, (u64, f64)> = std::collections::HashMap::new();

        for row in &rows {
            let ts: i64 = row.get("created_at");
            let dt = chrono::DateTime::from_timestamp_millis(ts)
                .unwrap_or_default()
                .with_timezone(&chrono::Utc);
            let date_key = dt.format("%Y-%m-%d").to_string();

            let usage_str: String = row.get("usage");
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&usage_str) {
                let tokens = val.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0)
                    + val.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                let cost = val.get("cost").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let entry = daily.entry(date_key).or_insert((0, 0.0));
                entry.0 += tokens;
                entry.1 += cost;
            }
        }

        // Build 7-day series
        let mut trend = Vec::new();
        for i in (0..7).rev() {
            let day_ts = now - (i as i64) * 24 * 60 * 60 * 1000;
            let dt = chrono::DateTime::from_timestamp_millis(day_ts)
                .unwrap_or_default()
                .with_timezone(&chrono::Utc);
            let date_key = dt.format("%Y-%m-%d").to_string();
            let (tokens, cost) = daily.get(&date_key).copied().unwrap_or((0, 0.0));
            trend.push(UsagePoint { date: date_key, tokens, cost });
        }

        Ok(trend)
    }

    async fn load_skills(&self) -> Result<SkillOverview, AppError> {
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM skills")
            .fetch_one(&self.db.pool)
            .await?;
        let enabled: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM skills WHERE is_enabled = 1")
            .fetch_one(&self.db.pool)
            .await?;

        Ok(SkillOverview {
            enabled: enabled as usize,
            total: total as usize,
            popular: Vec::new(),
        })
    }

    async fn load_mcp_status(&self) -> Result<Vec<McpServerStatus>, AppError> {
        let rows = sqlx::query_as::<_, McpServerRow>(
            "SELECT id, name, type, command, args, env, base_url, headers, is_active, timeout_ms, created_at, updated_at FROM mcp_servers ORDER BY created_at",
        )
        .fetch_all(&self.db.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| McpServerStatus {
                id: r.id,
                name: r.name,
                status: if r.is_active != 0 { "active".into() } else { "inactive".into() },
                tools_count: 0,
                last_error: None,
            })
            .collect())
    }

    async fn load_recent_sessions(&self) -> Result<Vec<SessionSummary>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT
                s.id,
                COALESCE(s.title, 'Untitled') AS title,
                a.name AS agent_name,
                s.updated_at,
                (SELECT COUNT(*) FROM messages m WHERE m.session_id = s.id) AS message_count
            FROM sessions s
            JOIN agents a ON a.id = s.agent_id
            ORDER BY s.updated_at DESC
            LIMIT 10
            "#,
        )
        .fetch_all(&self.db.pool)
        .await?;

        let mut result = Vec::new();
        for row in rows {
            let ts: i64 = row.get("updated_at");
            result.push(SessionSummary {
                id: row.get("id"),
                title: row.get("title"),
                agent_name: row.get("agent_name"),
                updated_at: ts.to_string(),
                message_count: row.get::<i64, _>("message_count"),
            });
        }
        Ok(result)
    }

    async fn load_models(&self) -> Result<Vec<ModelStatus>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT
                p.name AS provider_name,
                m.model_id,
                COALESCE(m.display_name, m.model_id) AS display_name,
                CASE WHEN m.is_default = 1 THEN 'default' ELSE 'available' END AS status
            FROM models m
            JOIN providers p ON p.id = m.provider_id
            ORDER BY m.is_default DESC, m.display_name
            "#,
        )
        .fetch_all(&self.db.pool)
        .await?;

        let mut result = Vec::new();
        for row in rows {
            result.push(ModelStatus {
                provider_name: row.get("provider_name"),
                model_id: row.get("model_id"),
                display_name: row.get("display_name"),
                status: row.get("status"),
            });
        }
        Ok(result)
    }
}
