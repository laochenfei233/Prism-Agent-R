use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::collections::HashSet;
use tokio::sync::{oneshot, Mutex};

use super::error::AgentError;
use super::model::ToolOutput;

// ── Tool Spec (re-export for convenience) ─────────────────

pub use super::model::ToolSpec;

// ── Risk Level ────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub enum RiskLevel {
    /// read/list/glob/grep — auto-approve
    Low,
    /// write to known directory — silent log
    Medium,
    /// delete/edit/external API — needs approval
    High,
    /// rm -rf/database ops/send message — double confirm
    Critical,
}

impl RiskLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

// ── Tool Approval Request / Response ──────────────────────

fn default_approval_kind() -> ApprovalRequestKind {
    ApprovalRequestKind::Tool
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalRequestKind {
    Tool,
    Delegation,
}

impl ApprovalRequestKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Tool => "tool",
            Self::Delegation => "delegation",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ToolApprovalRequest {
    pub call_id: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub agent_id: String,
    pub risk_level: RiskLevel,
    pub description: String,
    #[serde(default = "default_approval_kind")]
    pub kind: ApprovalRequestKind,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub parent_session_id: Option<String>,
    #[serde(default)]
    pub parent_agent_id: Option<String>,
    #[serde(default)]
    pub child_agent_id: Option<String>,
    #[serde(default)]
    pub task_summary: Option<String>,
    #[serde(default)]
    pub capability_summary: Option<String>,
    #[serde(default)]
    pub confirm_step: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ToolApprovalResponse {
    Approved,
    Rejected(String),
    AlwaysApprove(String),
    Defer,
}

// ── Risk Assessment ───────────────────────────────────────

pub fn assess_risk(tool_name: &str, _args: &serde_json::Value) -> RiskLevel {
    match tool_name {
        // 只读 / 搜索类：自动放行
        "file_read" | "read_file" | "file_list" | "glob" | "grep" | "web_search"
        | "wiki_search" | "memory_search" | "task_list" => RiskLevel::Low,
        // 写入已知目录 / 会话内操作：静默记录
        "file_write" | "write_file" | "edit_file" | "wiki_write" | "memory_save"
        | "task_create" | "task_update" => RiskLevel::Medium,
        // 委派工具自身不再触发工具级 HITL，统一由 DelegationGate 结构化审批一次
        "delegate_to_agent" => RiskLevel::Low,
        // 删除类操作不再静默放行
        "task_delete" => RiskLevel::High,
        // 未知或未注册工具：默认需要审批
        _ => RiskLevel::High,
    }
}

// ── Approval Store ────────────────────────────────────────

struct PendingEntry {
    tx: oneshot::Sender<ToolApprovalResponse>,
    agent_id: String,
    tool_name: String,
    arguments: serde_json::Value,
    allow_always: bool,
}

type PendingMap = std::collections::HashMap<String, PendingEntry>;

/// 对参数做规范化序列化（对象键排序），再计算稳定 SHA-256。
pub fn normalized_args_hash(args: &serde_json::Value) -> String {
    fn canonical(value: &serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(map) => {
                let mut entries: Vec<_> =
                    map.iter().map(|(k, v)| (k.clone(), canonical(v))).collect();
                entries.sort_by(|a, b| a.0.cmp(&b.0));
                serde_json::Value::Object(entries.into_iter().collect())
            }
            serde_json::Value::Array(items) => {
                serde_json::Value::Array(items.iter().map(canonical).collect())
            }
            other => other.clone(),
        }
    }

    let serialized = serde_json::to_string(&canonical(args)).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(serialized.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub struct ToolApprovalStore {
    pending: Mutex<PendingMap>,
    /// 无数据库时的内存授权（agent|tool|hash，或旧版 tool-only）
    memory_rules: Mutex<HashSet<String>>,
    pool: Option<SqlitePool>,
}

impl ToolApprovalStore {
    pub fn new() -> Self {
        Self {
            pending: Mutex::new(PendingMap::new()),
            memory_rules: Mutex::new(HashSet::new()),
            pool: None,
        }
    }

    pub fn with_pool(pool: SqlitePool) -> Self {
        Self {
            pending: Mutex::new(PendingMap::new()),
            memory_rules: Mutex::new(HashSet::new()),
            pool: Some(pool),
        }
    }

    /// 持久化审批请求并注册 pending waiter。
    pub async fn request_approval(
        &self,
        request: ToolApprovalRequest,
    ) -> oneshot::Receiver<ToolApprovalResponse> {
        let (tx, rx) = oneshot::channel();
        let timeout = self.timeout_seconds().await;
        let expires_at = if timeout == 0 {
            None
        } else {
            Some(chrono::Utc::now().timestamp_millis() + (timeout as i64) * 1000)
        };

        if let Some(pool) = &self.pool {
            let arguments = serde_json::to_string(&request.arguments).unwrap_or_default();
            let _ = crate::data::services::approval_service::insert_request(
                pool,
                &request.call_id,
                request.kind.as_str(),
                "pending",
                request.session_id.as_deref(),
                request.parent_session_id.as_deref(),
                &request.agent_id,
                request.parent_agent_id.as_deref(),
                request.child_agent_id.as_deref(),
                Some(&request.tool_name),
                Some(&arguments),
                request.task_summary.as_deref(),
                request.capability_summary.as_deref(),
                request.risk_level.as_str(),
                Some(request.description.as_str()),
                expires_at,
            )
            .await;
        }

        self.pending.lock().await.insert(
            request.call_id,
            PendingEntry {
                tx,
                agent_id: request.agent_id,
                tool_name: request.tool_name,
                arguments: request.arguments,
                allow_always: request.kind == ApprovalRequestKind::Tool
                    && request.confirm_step.is_none(),
            },
        );
        rx
    }

    /// 完成审批：持久化决定，AlwaysApprove 落成作用域化规则。
    pub async fn respond(&self, call_id: &str, response: ToolApprovalResponse) -> bool {
        let Some(entry) = self.pending.lock().await.remove(call_id) else {
            return false;
        };

        if entry.allow_always {
            if let ToolApprovalResponse::AlwaysApprove(_) = &response {
                let hash = normalized_args_hash(&entry.arguments);
                let scope = format!("{}|{}|{}", entry.agent_id, entry.tool_name, hash);
                if let Some(pool) = &self.pool {
                    let _ = crate::data::services::approval_service::upsert_rule(
                        pool,
                        &entry.agent_id,
                        &entry.tool_name,
                        &hash,
                        &scope,
                        10,
                    )
                    .await;
                } else {
                    self.memory_rules.lock().await.insert(scope);
                }
            }
        }

        if let Some(pool) = &self.pool {
            let (status, decision, reason) = match &response {
                ToolApprovalResponse::Approved => ("approved", "approved", None),
                ToolApprovalResponse::AlwaysApprove(_) => ("approved", "always_approve", None),
                ToolApprovalResponse::Rejected(reason) => {
                    ("rejected", "rejected", Some(reason.as_str()))
                }
                ToolApprovalResponse::Defer => ("deferred", "deferred", None),
            };
            let _ = crate::data::services::approval_service::update_decision(
                pool, call_id, status, decision, reason,
            )
            .await;
        }

        let _ = entry.tx.send(response);
        true
    }

    /// 作用域化 Always Approve 匹配：agent + tool + 规范化参数哈希。
    pub async fn is_always_approved(
        &self,
        agent_id: &str,
        tool_name: &str,
        args: &serde_json::Value,
    ) -> bool {
        let hash = normalized_args_hash(args);
        let scope = format!("{agent_id}|{tool_name}|{hash}");
        if let Some(pool) = &self.pool {
            return crate::data::services::approval_service::consume_rule(
                pool, agent_id, tool_name, &hash,
            )
            .await
            .unwrap_or(false);
        }
        let rules = self.memory_rules.lock().await;
        rules.contains(&scope) || rules.contains(tool_name)
    }

    /// 旧签名：仅用于测试/无 DB 直通，等价于任意 agent 全参数放行。
    pub async fn add_always_approve(&self, tool_name: &str) {
        self.memory_rules.lock().await.insert(tool_name.to_string());
    }

    /// 按作用域添加内存授权（无 DB 时的等价路径）。
    pub async fn add_always_approve_scoped(
        &self,
        agent_id: &str,
        tool_name: &str,
        args: &serde_json::Value,
    ) {
        let hash = normalized_args_hash(args);
        let scope = format!("{agent_id}|{tool_name}|{hash}");
        self.memory_rules.lock().await.insert(scope);
    }

    /// 关闭未响应的请求（超时/取消），移除 pending 并持久化状态。
    pub async fn abandon(&self, call_id: &str, status: &str, reason: &str) {
        self.pending.lock().await.remove(call_id);
        if let Some(pool) = &self.pool {
            let _ = crate::data::services::approval_service::close_request(
                pool, call_id, status, reason,
            )
            .await;
        }
    }

    pub async fn timeout_seconds(&self) -> i64 {
        match &self.pool {
            Some(pool) => {
                crate::data::settings::prefs::get_i64(pool, "approval.timeout_seconds", 300)
                    .await
                    .max(0)
            }
            None => 300,
        }
    }
}

impl Default for ToolApprovalStore {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tool Executor Trait ───────────────────────────────────

#[async_trait]
pub trait ToolExecutor: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> serde_json::Value;
    async fn execute(&self, args: serde_json::Value) -> Result<ToolOutput, AgentError>;
}

// ── Tool Registry ─────────────────────────────────────────

pub struct ToolRegistry {
    tools: Vec<Box<dyn ToolExecutor>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    pub fn register(&mut self, tool: Box<dyn ToolExecutor>) {
        self.tools.push(tool);
    }

    /// 按名称移除已注册工具（用于 disabled_tools 过滤）。返回是否发生移除。
    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.tools.len();
        self.tools.retain(|t| t.name() != name);
        self.tools.len() != before
    }

    pub fn get(&self, name: &str) -> Option<&dyn ToolExecutor> {
        self.tools
            .iter()
            .find(|t| t.name() == name)
            .map(|t| t.as_ref())
    }

    pub fn specs(&self) -> Vec<ToolSpec> {
        self.tools
            .iter()
            .map(|t| ToolSpec {
                name: t.name().to_string(),
                description: t.description().to_string(),
                parameters: t.schema(),
            })
            .collect()
    }

    /// Return specs only for the given tool names (router-filtered injection).
    /// Order follows `names`, not registration order.
    pub fn specs_filtered(&self, names: &std::collections::HashSet<String>) -> Vec<ToolSpec> {
        names
            .iter()
            .filter_map(|name| {
                self.get(name).map(|t| ToolSpec {
                    name: t.name().to_string(),
                    description: t.description().to_string(),
                    parameters: t.schema(),
                })
            })
            .collect()
    }

    /// All registered tool names (used to seed the ToolRouter index).
    pub fn tool_names(&self) -> Vec<String> {
        self.tools.iter().map(|t| t.name().to_string()).collect()
    }

    pub fn names(&self) -> Vec<&str> {
        self.tools.iter().map(|t| t.name()).collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(call_id: &str, args: serde_json::Value) -> ToolApprovalRequest {
        ToolApprovalRequest {
            call_id: call_id.to_string(),
            tool_name: "echo_tool".to_string(),
            arguments: args,
            agent_id: "agent-a".to_string(),
            risk_level: RiskLevel::High,
            description: "test".to_string(),
            kind: ApprovalRequestKind::Tool,
            session_id: None,
            parent_session_id: None,
            parent_agent_id: None,
            child_agent_id: None,
            task_summary: None,
            capability_summary: None,
            confirm_step: None,
        }
    }

    #[test]
    fn normalized_hash_is_stable_and_scope_sensitive() {
        let args = serde_json::json!({"b": 1, "a": "x"});
        let reordered = serde_json::json!({"a": "x", "b": 1});
        assert_eq!(
            normalized_args_hash(&args),
            normalized_args_hash(&reordered)
        );
        assert_ne!(
            normalized_args_hash(&args),
            normalized_args_hash(&serde_json::json!({"a": "x", "b": 2}))
        );
    }

    #[test]
    fn delegate_tool_is_low_risk_for_single_gate() {
        assert_eq!(
            assess_risk("delegate_to_agent", &serde_json::json!({})),
            RiskLevel::Low
        );
    }

    #[tokio::test]
    async fn queue_responds_by_call_id_and_scope_is_agent_local() {
        let store = std::sync::Arc::new(ToolApprovalStore::new());
        let rx1 = store
            .request_approval(request("call-1", serde_json::json!({"text": "hi"})))
            .await;
        let rx2 = store
            .request_approval(request("call-2", serde_json::json!({"text": "yo"})))
            .await;

        assert!(
            store
                .respond("call-1", ToolApprovalResponse::Approved)
                .await
        );
        assert_eq!(rx1.await.unwrap(), ToolApprovalResponse::Approved);
        assert!(store.respond("call-2", ToolApprovalResponse::Defer).await);
        assert_eq!(rx2.await.unwrap(), ToolApprovalResponse::Defer);
        assert!(
            !store
                .respond("call-1", ToolApprovalResponse::Approved)
                .await
        );

        let args = serde_json::json!({"text": "hi"});
        store
            .add_always_approve_scoped("agent-a", "echo_tool", &args)
            .await;
        assert!(
            store
                .is_always_approved("agent-a", "echo_tool", &args)
                .await
        );
        assert!(
            !store
                .is_always_approved("agent-b", "echo_tool", &args)
                .await
        );
        assert!(
            !store
                .is_always_approved(
                    "agent-a",
                    "echo_tool",
                    &serde_json::json!({"text": "other"})
                )
                .await
        );
    }

    #[tokio::test]
    async fn critical_and_delegation_never_create_always_rules() {
        let store = std::sync::Arc::new(ToolApprovalStore::new());
        let mut critical = request("crit-1", serde_json::json!({}));
        critical.confirm_step = Some(1);
        let rx = store.request_approval(critical.clone()).await;
        store
            .respond("crit-1", ToolApprovalResponse::AlwaysApprove(String::new()))
            .await;
        rx.await.unwrap();
        assert!(
            !store
                .is_always_approved("agent-a", "echo_tool", &serde_json::json!({}))
                .await
        );

        let mut delegation = request("del-1", serde_json::json!({}));
        delegation.kind = ApprovalRequestKind::Delegation;
        let rx = store.request_approval(delegation.clone()).await;
        store
            .respond("del-1", ToolApprovalResponse::AlwaysApprove(String::new()))
            .await;
        rx.await.unwrap();
        assert!(
            !store
                .is_always_approved("agent-a", "echo_tool", &serde_json::json!({}))
                .await
        );
    }
}
