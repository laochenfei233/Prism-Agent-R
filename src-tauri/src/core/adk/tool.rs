use async_trait::async_trait;
use serde::{Deserialize, Serialize};
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

// ── Tool Approval Request / Response ──────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ToolApprovalRequest {
    pub call_id: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub agent_id: String,
    pub risk_level: RiskLevel,
    pub description: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ToolApprovalResponse {
    Approved,
    Rejected(String),
    AlwaysApprove(String),
    Defer,
}

// ── Risk Assessment ───────────────────────────────────────

pub fn assess_risk(tool_name: &str, _args: &serde_json::Value) -> RiskLevel {
    match tool_name {
        "read_file" | "list_dir" | "glob" | "grep" | "lsp:diagnostics" | "web_search" => {
            RiskLevel::Low
        }
        "write_file" | "edit_file" => RiskLevel::Medium,
        "delete_file" | "run_command" | "http_request" => RiskLevel::High,
        "rm_rf" | "database_drop" | "send_message" => RiskLevel::Critical,
        _ => RiskLevel::High,
    }
}

// ── Approval Store ────────────────────────────────────────

type PendingMap = std::collections::HashMap<String, oneshot::Sender<ToolApprovalResponse>>;

pub struct ToolApprovalStore {
    pending: Mutex<PendingMap>,
    always_approve: Mutex<HashSet<String>>,
}

impl ToolApprovalStore {
    pub fn new() -> Self {
        Self {
            pending: Mutex::new(PendingMap::new()),
            always_approve: Mutex::new(HashSet::new()),
        }
    }

    /// Register a pending approval and return the receiver to await.
    pub async fn request_approval(
        &self,
        call_id: String,
    ) -> oneshot::Receiver<ToolApprovalResponse> {
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(call_id, tx);
        rx
    }

    /// Complete a pending approval by call_id.
    pub async fn respond(&self, call_id: &str, response: ToolApprovalResponse) -> bool {
        if let Some(tx) = self.pending.lock().await.remove(call_id) {
            let _ = tx.send(response);
            true
        } else {
            false
        }
    }

    /// Check if a tool is in the always-approve list.
    pub async fn is_always_approved(&self, tool_name: &str) -> bool {
        self.always_approve.lock().await.contains(tool_name)
    }

    /// Add a tool to the always-approve list.
    pub async fn add_always_approve(&self, tool_name: &str) {
        self.always_approve
            .lock()
            .await
            .insert(tool_name.to_string());
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
