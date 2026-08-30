use sqlx::{Row, SqlitePool};

use crate::utils::error::AppError;

/// 持久化一条待处理/已决定的审批请求。
#[allow(clippy::too_many_arguments)]
pub async fn insert_request(
    pool: &SqlitePool,
    id: &str,
    kind: &str,
    status: &str,
    session_id: Option<&str>,
    parent_session_id: Option<&str>,
    agent_id: &str,
    parent_agent_id: Option<&str>,
    child_agent_id: Option<&str>,
    tool_name: Option<&str>,
    arguments: Option<&str>,
    task_summary: Option<&str>,
    capability_summary: Option<&str>,
    risk_level: &str,
    reason: Option<&str>,
    expires_at: Option<i64>,
) -> Result<(), AppError> {
    let now = chrono::Utc::now().timestamp_millis();
    sqlx::query(
        "INSERT INTO agent_approval_requests
         (id, kind, status, session_id, parent_session_id, agent_id, parent_agent_id,
          child_agent_id, tool_name, arguments, task_summary, capability_summary,
          risk_level, reason, expires_at, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(kind)
    .bind(status)
    .bind(session_id)
    .bind(parent_session_id)
    .bind(agent_id)
    .bind(parent_agent_id)
    .bind(child_agent_id)
    .bind(tool_name)
    .bind(arguments)
    .bind(task_summary)
    .bind(capability_summary)
    .bind(risk_level)
    .bind(reason)
    .bind(expires_at)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

/// 更新一条审批请求的最终决定。
pub async fn update_decision(
    pool: &SqlitePool,
    id: &str,
    status: &str,
    decision: &str,
    decision_reason: Option<&str>,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE agent_approval_requests
         SET status = ?, decision = ?, decision_reason = ?, decided_by = 'user',
             decided_at = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(status)
    .bind(decision)
    .bind(decision_reason)
    .bind(chrono::Utc::now().timestamp_millis())
    .bind(chrono::Utc::now().timestamp_millis())
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// 关闭一条未收到响应的请求（超时/取消）。
pub async fn close_request(
    pool: &SqlitePool,
    id: &str,
    status: &str,
    reason: &str,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE agent_approval_requests
         SET status = ?, decision_reason = COALESCE(decision_reason, ?), updated_at = ?
         WHERE id = ? AND status = 'pending'",
    )
    .bind(status)
    .bind(reason)
    .bind(chrono::Utc::now().timestamp_millis())
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// 作用域化授权规则：命中且未超限/未过期则原子消耗一次。
pub async fn consume_rule(
    pool: &SqlitePool,
    agent_id: &str,
    tool_name: &str,
    normalized_args_hash: &str,
) -> Result<bool, AppError> {
    let row = sqlx::query(
        "SELECT id, max_uses, use_count, expires_at
         FROM agent_approval_rules
         WHERE agent_id = ? AND tool_name = ? AND normalized_args_hash = ?",
    )
    .bind(agent_id)
    .bind(tool_name)
    .bind(normalized_args_hash)
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
        return Ok(false);
    };
    let rule_id: String = row.get("id");
    let max_uses: Option<i64> = row.get("max_uses");
    let use_count: i64 = row.get("use_count");
    let expires_at: Option<i64> = row.get("expires_at");

    if let Some(max_uses) = max_uses {
        if use_count >= max_uses {
            return Ok(false);
        }
    }
    if let Some(expires_at) = expires_at {
        if expires_at <= chrono::Utc::now().timestamp_millis() {
            return Ok(false);
        }
    }

    let updated = sqlx::query(
        "UPDATE agent_approval_rules
         SET use_count = use_count + 1, updated_at = ?
         WHERE id = ? AND (max_uses IS NULL OR use_count < max_uses)
           AND (expires_at IS NULL OR expires_at > ?)",
    )
    .bind(chrono::Utc::now().timestamp_millis())
    .bind(&rule_id)
    .bind(chrono::Utc::now().timestamp_millis())
    .execute(pool)
    .await?;
    Ok(updated.rows_affected() > 0)
}

/// 插入（或重置）一条作用域化授权规则。
pub async fn upsert_rule(
    pool: &SqlitePool,
    agent_id: &str,
    tool_name: &str,
    normalized_args_hash: &str,
    scope_hash: &str,
    max_uses: i64,
) -> Result<(), AppError> {
    let now = chrono::Utc::now().timestamp_millis();
    sqlx::query(
        "INSERT OR REPLACE INTO agent_approval_rules
         (id, agent_id, tool_name, normalized_args_hash, scope_hash, max_uses,
          use_count, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, 0, ?, ?)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(agent_id)
    .bind(tool_name)
    .bind(normalized_args_hash)
    .bind(scope_hash)
    .bind(max_uses)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

/// 查询审批历史（可按 Agent 过滤）。
pub async fn list_requests(
    pool: &SqlitePool,
    agent_id: Option<&str>,
    limit: i64,
) -> Result<Vec<crate::data::models::ApprovalRequestRow>, AppError> {
    let rows = if let Some(agent_id) = agent_id {
        sqlx::query_as::<_, crate::data::models::ApprovalRequestRow>(
            "SELECT id, kind, status, session_id, parent_session_id, agent_id,
                    parent_agent_id, child_agent_id, tool_name, arguments,
                    task_summary, capability_summary, risk_level, reason,
                    decision, decision_reason, decided_by, decided_at,
                    expires_at, created_at, updated_at
             FROM agent_approval_requests
             WHERE agent_id = ?
             ORDER BY created_at DESC
             LIMIT ?",
        )
        .bind(agent_id)
        .bind(limit)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, crate::data::models::ApprovalRequestRow>(
            "SELECT id, kind, status, session_id, parent_session_id, agent_id,
                    parent_agent_id, child_agent_id, tool_name, arguments,
                    task_summary, capability_summary, risk_level, reason,
                    decision, decision_reason, decided_by, decided_at,
                    expires_at, created_at, updated_at
             FROM agent_approval_requests
             ORDER BY created_at DESC
             LIMIT ?",
        )
        .bind(limit)
        .fetch_all(pool)
        .await?
    };
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn temp_db() -> crate::data::Database {
        let dir = std::env::temp_dir().join(format!("prism_approval_{}", uuid::Uuid::new_v4()));
        crate::data::db::Database::new(&dir).await.unwrap()
    }

    async fn seed_agent(db: &crate::data::Database, agent_id: &str) {
        let now = chrono::Utc::now().timestamp_millis();
        sqlx::query(
            "INSERT INTO agents (id, name, disabled_tools, configuration, order_key, created_at, updated_at)
             VALUES (?, ?, '[]', '{}', 0, ?, ?)",
        )
        .bind(agent_id)
        .bind(agent_id)
        .bind(now)
        .bind(now)
        .execute(&db.pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn request_lifecycle_is_persisted() {
        let db = temp_db().await;
        seed_agent(&db, "agent-a").await;
        insert_request(
            &db.pool,
            "req-1",
            "tool",
            "pending",
            None,
            None,
            "agent-a",
            None,
            None,
            Some("file_write"),
            Some("{\"path\":\"/tmp/a\"}"),
            None,
            None,
            "high",
            Some("test"),
            None,
        )
        .await
        .unwrap();
        update_decision(&db.pool, "req-1", "rejected", "rejected", Some("no"))
            .await
            .unwrap();

        let rows = list_requests(&db.pool, Some("agent-a"), 10).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "rejected");
        assert_eq!(rows[0].decision_reason.as_deref(), Some("no"));
    }

    #[tokio::test]
    async fn rule_expires_after_max_uses() {
        let db = temp_db().await;
        seed_agent(&db, "agent-a").await;
        upsert_rule(&db.pool, "agent-a", "file_write", "hash-1", "scope-1", 2)
            .await
            .unwrap();
        assert!(consume_rule(&db.pool, "agent-a", "file_write", "hash-1")
            .await
            .unwrap());
        assert!(consume_rule(&db.pool, "agent-a", "file_write", "hash-1")
            .await
            .unwrap());
        assert!(!consume_rule(&db.pool, "agent-a", "file_write", "hash-1")
            .await
            .unwrap());
    }
}
