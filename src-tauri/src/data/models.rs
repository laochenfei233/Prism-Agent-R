use serde::{Deserialize, Serialize};

// ── Provider ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProviderRow {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub base_url: Option<String>,
    pub api_key_enc: Option<String>,
    pub is_enabled: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderDto {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub base_url: Option<String>,
    pub is_enabled: bool,
}

// ── Model ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ModelRow {
    pub id: String,
    pub provider_id: String,
    pub model_id: String,
    pub display_name: Option<String>,
    pub kind: String,
    pub max_tokens: Option<i32>,
    pub is_default: i32,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDto {
    pub id: String,
    pub provider_id: String,
    pub model_id: String,
    pub display_name: Option<String>,
    pub kind: String,
    pub max_tokens: Option<i32>,
    pub is_default: bool,
}

// ── Agent ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AgentRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub avatar: Option<String>,
    pub system_prompt: Option<String>,
    pub model_id: Option<String>,
    pub plan_model_id: Option<String>,
    pub small_model_id: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i32>,
    pub disabled_tools: String,
    pub configuration: String,
    pub order_key: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDto {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub avatar: Option<String>,
    pub system_prompt: Option<String>,
    pub model_id: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i32>,
    pub disabled_tools: Vec<String>,
    pub order_key: i32,
}

// ── Session ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SessionRow {
    pub id: String,
    pub agent_id: String,
    pub title: Option<String>,
    pub pinned: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDto {
    pub id: String,
    pub agent_id: String,
    pub title: Option<String>,
    pub pinned: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

// ── Message ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MessageRow {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub tool_calls: Option<String>,
    pub tool_call_id: Option<String>,
    pub model_id: Option<String>,
    pub usage: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDto {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub tool_calls: Option<serde_json::Value>,
    pub tool_call_id: Option<String>,
    pub model_id: Option<String>,
    pub usage: Option<serde_json::Value>,
    pub created_at: i64,
}

// ── MCP Server ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct McpServerRow {
    pub id: String,
    pub name: String,
    pub r#type: String,
    pub command: Option<String>,
    pub args: String,
    pub env: String,
    pub base_url: Option<String>,
    pub headers: String,
    pub is_active: i32,
    pub timeout_ms: Option<i32>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerDto {
    pub id: String,
    pub name: String,
    pub r#type: String,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub base_url: Option<String>,
    pub is_active: bool,
    pub timeout_ms: Option<i32>,
}

// ── Skill ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SkillRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub folder_name: String,
    pub source: String,
    pub source_url: Option<String>,
    pub namespace: Option<String>,
    pub author: Option<String>,
    pub tags: String,
    pub content_hash: String,
    pub is_enabled: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDto {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub folder_name: String,
    pub source: String,
    pub is_enabled: bool,
}

// ── Workflow ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WorkflowRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub definition: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDto {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub definition: serde_json::Value,
}
