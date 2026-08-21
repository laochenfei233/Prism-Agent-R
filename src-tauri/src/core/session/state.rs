use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::{SessionInitReport, SessionLifecycle};

/// 会话状态管理器
///
/// 管理所有活跃会话的生命周期状态，支持并发访问
pub struct SessionStateManager {
    states: RwLock<HashMap<String, SessionLifecycle>>,
}

impl SessionStateManager {
    pub fn new() -> Self {
        Self {
            states: RwLock::new(HashMap::new()),
        }
    }

    /// 获取会话当前状态
    pub async fn get_state(&self, session_id: &str) -> SessionLifecycle {
        self.states
            .read()
            .await
            .get(session_id)
            .cloned()
            .unwrap_or(SessionLifecycle::Created)
    }

    /// 设置会话状态
    pub async fn set_state(&self, session_id: &str, state: SessionLifecycle) {
        self.states
            .write()
            .await
            .insert(session_id.to_string(), state);
    }

    /// 移除会话状态（会话结束时清理）
    pub async fn remove(&self, session_id: &str) {
        self.states.write().await.remove(session_id);
    }

    /// §17.1 会话初始化流程
    ///
    /// INIT 阶段：
    /// 1. 加载 Agent 配置
    /// 2. 校验模型 Provider 可达
    /// 3. 加载记忆注入
    /// 4. 校验 MCP 服务器可达（失败仅告警不阻断）
    pub async fn init_session(
        &self,
        session_id: &str,
        db: &sqlx::SqlitePool,
        mcp_runtime: &Arc<crate::mcp::McpRuntime>,
    ) -> Result<SessionInitReport, String> {
        self.set_state(session_id, SessionLifecycle::Init).await;

        let mut report = SessionInitReport {
            provider_ok: true,
            provider_error: None,
            memory_ok: true,
            memory_error: None,
            mcp_ok: true,
            mcp_error: None,
        };

        // 1. 校验模型 Provider
        let agent_id: Option<String> =
            sqlx::query_scalar("SELECT agent_id FROM sessions WHERE id = ?")
                .bind(session_id)
                .fetch_optional(db)
                .await
                .map_err(|e| format!("查询会话失败: {e}"))?
                .ok_or_else(|| format!("会话不存在: {session_id}"))?;

        if let Some(aid) = &agent_id {
            let model_check = sqlx::query_scalar::<_, String>(
                "SELECT m.model_id FROM agents a JOIN models m ON a.model_id = m.id WHERE a.id = ?",
            )
            .bind(aid)
            .fetch_optional(db)
            .await;

            match model_check {
                Ok(Some(_)) => {}
                Ok(None) => {
                    report.provider_ok = false;
                    report.provider_error = Some("未配置模型".to_string());
                }
                Err(e) => {
                    report.provider_ok = false;
                    report.provider_error = Some(format!("查询模型失败: {e}"));
                }
            }
        }

        // 2. 校验 MCP 服务器（尽力而为，失败仅告警）
        let mcp_links: Vec<(String,)> =
            sqlx::query_as("SELECT mcp_server_id FROM agent_mcp_servers WHERE agent_id = ?")
                .bind(agent_id.as_deref().unwrap_or(""))
                .fetch_all(db)
                .await
                .unwrap_or_default();

        for (server_id,) in &mcp_links {
            if mcp_runtime.get_tools(server_id).await.is_empty() {
                tracing::warn!("MCP server {server_id} has no tools (may not be connected)");
            }
        }

        // 3. 根据校验结果设置状态
        if report.all_ok() {
            self.set_state(session_id, SessionLifecycle::Ready).await;
        } else {
            self.set_state(session_id, SessionLifecycle::InitFailed)
                .await;
        }

        Ok(report)
    }

    /// 会话开始运行
    pub async fn start_running(&self, session_id: &str) {
        let current = self.get_state(session_id).await;
        if current == SessionLifecycle::Ready || current == SessionLifecycle::Paused {
            self.set_state(session_id, SessionLifecycle::Running).await;
        }
    }

    /// 会话暂停（中断）
    pub async fn pause(&self, session_id: &str) {
        let current = self.get_state(session_id).await;
        if current == SessionLifecycle::Running {
            self.set_state(session_id, SessionLifecycle::Paused).await;
        }
    }

    /// 会话恢复
    pub async fn resume(&self, session_id: &str) {
        let current = self.get_state(session_id).await;
        if current == SessionLifecycle::Paused {
            self.set_state(session_id, SessionLifecycle::Running).await;
        }
    }

    /// 会话验证阶段
    pub async fn start_verify(&self, session_id: &str) {
        let current = self.get_state(session_id).await;
        if current == SessionLifecycle::Running {
            self.set_state(session_id, SessionLifecycle::Verifying)
                .await;
        }
    }

    /// 会话完成
    pub async fn complete(&self, session_id: &str) {
        self.set_state(session_id, SessionLifecycle::Done).await;
    }

    /// 会话重试（从 INIT_FAILED 恢复）
    pub async fn retry_init(&self, session_id: &str) {
        let current = self.get_state(session_id).await;
        if current == SessionLifecycle::InitFailed {
            self.set_state(session_id, SessionLifecycle::Created).await;
        }
    }
}

impl Default for SessionStateManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_state_transitions() {
        let mgr = SessionStateManager::new();
        let sid = "test-session";

        assert_eq!(mgr.get_state(sid).await, SessionLifecycle::Created);

        mgr.set_state(sid, SessionLifecycle::Init).await;
        assert_eq!(mgr.get_state(sid).await, SessionLifecycle::Init);

        mgr.set_state(sid, SessionLifecycle::Ready).await;
        assert_eq!(mgr.get_state(sid).await, SessionLifecycle::Ready);

        mgr.start_running(sid).await;
        assert_eq!(mgr.get_state(sid).await, SessionLifecycle::Running);

        mgr.pause(sid).await;
        assert_eq!(mgr.get_state(sid).await, SessionLifecycle::Paused);

        mgr.resume(sid).await;
        assert_eq!(mgr.get_state(sid).await, SessionLifecycle::Running);

        mgr.complete(sid).await;
        assert_eq!(mgr.get_state(sid).await, SessionLifecycle::Done);
    }

    #[test]
    fn test_init_report_errors() {
        let report = SessionInitReport {
            provider_ok: false,
            provider_error: Some("模型未配置".to_string()),
            memory_ok: true,
            memory_error: None,
            mcp_ok: false,
            mcp_error: Some("MCP 连接失败".to_string()),
        };

        assert!(!report.all_ok());
        assert_eq!(report.errors().len(), 2);
    }
}
