use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::pin::Pin;


use super::error::AgentError;

// ── Chat Role ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChatRole {
    System,
    User,
    Assistant,
    Tool,
}

// ── Message Content ───────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageContent {
    Text(String),
    ToolCall(ToolCall),
    ToolResult(ToolOutput),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    pub content: String,
    pub is_error: bool,
}

impl ToolOutput {
    pub fn text(content: String) -> Self {
        Self { content, is_error: false }
    }

    pub fn error(content: String) -> Self {
        Self { content, is_error: true }
    }
}

// ── Chat Message ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: MessageContent,
    pub name: Option<String>,
}

// ── Generation Request ────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationRequest {
    pub messages: Vec<ChatMessage>,
    pub system: Option<String>,
    pub tools: Vec<ToolSpec>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stop: Option<Vec<String>>,
}

impl Default for GenerationRequest {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            system: None,
            tools: Vec::new(),
            temperature: None,
            max_tokens: None,
            stop: None,
        }
    }
}

// ── Tool Spec ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

// ── Generation Response ───────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationResponse {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

// ── Stream Event ──────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum StreamEvent {
    Text(String),
    ToolCall(ToolCall),
    Finish { usage: Option<Usage> },
    Error(String),
}

pub type StreamHandle = Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>;

// ── Model Capabilities ────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ModelCapabilities {
    pub max_tokens: u32,
    pub supports_tools: bool,
    pub supports_streaming: bool,
    pub supports_vision: bool,
}

// ── Model Provider Trait ──────────────────────────────────

#[async_trait]
pub trait ModelProvider: Send + Sync {
    fn id(&self) -> &str;
    fn display_name(&self) -> &str;
    fn capabilities(&self) -> ModelCapabilities;

    async fn generate(&self, request: GenerationRequest) -> Result<GenerationResponse, AgentError>;
    async fn stream(&self, request: GenerationRequest) -> Result<StreamHandle, AgentError>;
}
