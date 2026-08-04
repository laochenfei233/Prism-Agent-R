use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::error::AgentError;

// ── Memory Scope ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryScope {
    Global,
    Project { project_id: String },
    Session { session_id: String },
}

// ── Memory Item ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryItem {
    pub path: String,
    pub body: String,
    pub scope: MemoryScope,
    pub memory_type: String,
}

// ── Memory Context ────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryContext {
    pub summary: String,
    pub items: Vec<MemoryItem>,
}

// ── Message Exchange ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageExchange {
    pub user_message: String,
    pub assistant_message: String,
}

// ── Memory Store Trait ────────────────────────────────────

#[async_trait]
pub trait MemoryStore: Send + Sync {
    async fn build_context(
        &self,
        session_id: &str,
        agent_id: &str,
    ) -> Result<MemoryContext, AgentError>;

    async fn record(
        &self,
        session_id: &str,
        agent_id: &str,
        exchange: MessageExchange,
    ) -> Result<(), AgentError>;

    async fn search(
        &self,
        query: &str,
        scope: Option<MemoryScope>,
    ) -> Result<Vec<MemoryItem>, AgentError>;
}
