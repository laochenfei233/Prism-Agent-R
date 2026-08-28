use async_trait::async_trait;
use futures::{SinkExt, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::core::adk::error::AgentError;
use crate::core::adk::model::{
    ChatRole, GenerationRequest, GenerationResponse, MessageContent, ModelCapabilities,
    ModelProvider, StreamEvent, StreamHandle, ToolCall, Usage,
};

// ── Anthropic Provider ────────────────────────────────────
/// Implements the Anthropic Messages API format (`POST /messages`).
/// Compatible with native Anthropic API, DeepSeek Anthropic endpoint, and any compatible provider.
pub struct AnthropicProvider {
    id: String,
    display_name: String,
    api_key: String,
    base_url: String,
    model: String,
    client: Client,
}

impl AnthropicProvider {
    pub fn new(
        id: String,
        display_name: String,
        api_key: String,
        base_url: String,
        model: String,
    ) -> Self {
        Self {
            id,
            display_name,
            api_key,
            base_url,
            model,
            client: Client::new(),
        }
    }
}

#[async_trait]
impl ModelProvider for AnthropicProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn display_name(&self) -> &str {
        &self.display_name
    }

    fn capabilities(&self) -> ModelCapabilities {
        ModelCapabilities {
            max_tokens: 128000,
            supports_tools: true,
            supports_streaming: true,
            supports_vision: false,
        }
    }

    async fn generate(&self, request: GenerationRequest) -> Result<GenerationResponse, AgentError> {
        let body = build_anthropic_body(&self.model, &request, false);
        let url = format!("{}/messages", self.base_url);

        let resp = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AgentError::Provider(e.to_string()))?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(AgentError::Provider(format!("HTTP {status}: {text}")));
        }

        let data: AnthropicResponse = resp
            .json()
            .await
            .map_err(|e| AgentError::Provider(e.to_string()))?;

        parse_anthropic_response(&data)
    }

    async fn stream(&self, request: GenerationRequest) -> Result<StreamHandle, AgentError> {
        let body = build_anthropic_body(&self.model, &request, true);
        let url = format!("{}/messages", self.base_url);

        let resp = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AgentError::Provider(e.to_string()))?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(AgentError::Provider(format!("HTTP {status}: {text}")));
        }

        let (mut tx, rx) = futures::channel::mpsc::channel::<StreamEvent>(64);
        let stream = resp.bytes_stream();

        // State for accumulating tool calls across deltas
        let tool_calls_state: std::sync::Arc<tokio::sync::Mutex<Vec<PendingToolCall>>> =
            std::sync::Arc::new(tokio::sync::Mutex::new(Vec::new()));

        tokio::spawn(async move {
            let mut buffer = Vec::new();
            let mut stream = stream;

            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        buffer.extend_from_slice(&bytes);
                        while let Some(pos) = find_event_boundary(&buffer) {
                            let text = String::from_utf8_lossy(&buffer[..pos]).to_string();
                            buffer.drain(..pos);
                            let events = parse_anthropic_sse_buffer(&text, &tool_calls_state).await;
                            for event in events {
                                if tx.send(event).await.is_err() {
                                    return;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx
                            .send(StreamEvent::Error(format!("Stream error: {e}")))
                            .await;
                        return;
                    }
                }
            }

            if !buffer.is_empty() {
                let text = String::from_utf8_lossy(&buffer).to_string();
                let events = parse_anthropic_sse_buffer(&text, &tool_calls_state).await;
                for event in events {
                    if tx.send(event).await.is_err() {
                        return;
                    }
                }
            }
        });

        Ok(Box::pin(rx))
    }
}

// ── Pending Tool Call (accumulated across deltas) ─────────

#[derive(Clone)]
struct PendingToolCall {
    id: String,
    name: String,
    arguments: String,
}

// ── Request/Response Types ────────────────────────────────

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    messages: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<AnthropicTool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    stream: bool,
}

#[derive(Serialize)]
struct AnthropicTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContentBlock>,
    usage: Option<AnthropicUsage>,
    #[allow(dead_code)]
    stop_reason: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    block_type: Option<String>,
    text: Option<String>,
    id: Option<String>,
    name: Option<String>,
    input: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct AnthropicUsage {
    input_tokens: u64,
    output_tokens: u64,
}

// ── Build Request Body ────────────────────────────────────

fn build_anthropic_body(
    model: &str,
    request: &GenerationRequest,
    stream: bool,
) -> AnthropicRequest {
    let mut messages: Vec<serde_json::Value> = Vec::new();

    for msg in &request.messages {
        if msg.role == ChatRole::System {
            continue; // System goes into the `system` field
        }

        let role = match msg.role {
            ChatRole::User => "user",
            ChatRole::Assistant => "assistant",
            _ => continue,
        };

        match &msg.content {
            MessageContent::Text(t) => {
                messages.push(serde_json::json!({
                    "role": role,
                    "content": [{
                        "type": "text",
                        "text": t,
                    }],
                }));
            }
            MessageContent::ToolCall(tc) => {
                messages.push(serde_json::json!({
                    "role": "assistant",
                    "content": [{
                        "type": "tool_use",
                        "id": tc.id,
                        "name": tc.name,
                        "input": tc.arguments,
                    }],
                }));
            }
            MessageContent::ToolResult(to) => {
                messages.push(serde_json::json!({
                    "role": "user",
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": msg.name.clone().unwrap_or_default(),
                        "content": to.content,
                    }],
                }));
            }
        }
    }

    let tools: Vec<AnthropicTool> = request
        .tools
        .iter()
        .map(|t| AnthropicTool {
            name: t.name.clone(),
            description: t.description.clone(),
            input_schema: t.parameters.clone(),
        })
        .collect();

    let tool_choice: Option<serde_json::Value> = None;

    AnthropicRequest {
        model: model.to_string(),
        messages,
        system: request.system.clone(),
        max_tokens: request.max_tokens.unwrap_or(8192),
        tools,
        tool_choice,
        temperature: request.temperature,
        top_p: None,
        stream,
    }
}

// ── Parse Non-Streaming Response ───────────────────────────

fn parse_anthropic_response(data: &AnthropicResponse) -> Result<GenerationResponse, AgentError> {
    let mut text = String::new();
    let mut tool_calls = Vec::new();

    for block in &data.content {
        match block.block_type.as_deref() {
            Some("text") => {
                if let Some(t) = &block.text {
                    text.push_str(t);
                }
            }
            Some("thinking") => {
                // Thinking blocks are not included in main text output
                // (they would be streamed as Reasoning events)
            }
            Some("tool_use") => {
                tool_calls.push(ToolCall {
                    id: block.id.clone().unwrap_or_default(),
                    name: block.name.clone().unwrap_or_default(),
                    arguments: block.input.clone().unwrap_or_default(),
                });
            }
            _ => {}
        }
    }

    let usage = data.usage.as_ref().map(|u| Usage {
        prompt_tokens: u.input_tokens,
        completion_tokens: u.output_tokens,
        total_tokens: u.input_tokens + u.output_tokens,
    });

    Ok(GenerationResponse {
        text,
        tool_calls,
        usage,
    })
}

// ── SSE Parsing ───────────────────────────────────────────

fn find_event_boundary(buf: &[u8]) -> Option<usize> {
    for i in 0..buf.len().saturating_sub(1) {
        if buf[i] == b'\n' && buf[i + 1] == b'\n' {
            return Some(i + 2);
        }
    }
    None
}

async fn parse_anthropic_sse_buffer(
    text: &str,
    tool_calls_state: &std::sync::Arc<tokio::sync::Mutex<Vec<PendingToolCall>>>,
) -> Vec<StreamEvent> {
    let mut events = Vec::new();
    for block in text.split("\n\n") {
        let mut event_type = String::new();
        let mut data = String::new();
        for line in block.lines() {
            let line = line.trim();
            if let Some(stripped) = line.strip_prefix("event:") {
                event_type = stripped.trim().to_string();
            } else if let Some(stripped) = line.strip_prefix("data:") {
                data = stripped.trim().to_string();
            }
        }
        if data.is_empty() {
            continue;
        }
        let mut parsed = parse_anthropic_sse_event(&event_type, &data, tool_calls_state).await;
        events.append(&mut parsed);
    }
    events
}

async fn parse_anthropic_sse_event(
    event_type: &str,
    data: &str,
    tool_calls_state: &std::sync::Arc<tokio::sync::Mutex<Vec<PendingToolCall>>>,
) -> Vec<StreamEvent> {
    let chunk: serde_json::Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    match event_type {
        "content_block_start" => {
            // If it's a tool_use block, register a pending tool call
            let block_type = chunk["content_block"]["type"].as_str().unwrap_or("");
            if block_type == "tool_use" {
                let id = chunk["content_block"]["id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let name = chunk["content_block"]["name"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let mut state = tool_calls_state.lock().await;
                state.push(PendingToolCall {
                    id,
                    name,
                    arguments: String::new(),
                });
            }
            Vec::new()
        }
        "content_block_delta" => {
            let delta_type = chunk["delta"]["type"].as_str().unwrap_or("");
            match delta_type {
                "text_delta" => {
                    if let Some(text) = chunk["delta"]["text"].as_str() {
                        if !text.is_empty() {
                            return vec![StreamEvent::Text(text.to_string())];
                        }
                    }
                    Vec::new()
                }
                "thinking_delta" => {
                    if let Some(thinking) = chunk["delta"]["thinking"].as_str() {
                        if !thinking.is_empty() {
                            return vec![StreamEvent::Reasoning(thinking.to_string())];
                        }
                    }
                    Vec::new()
                }
                "input_json_delta" => {
                    // Accumulate partial JSON for tool calls
                    if let Some(partial) = chunk["delta"]["partial_json"].as_str() {
                        let mut state = tool_calls_state.lock().await;
                        if let Some(last) = state.last_mut() {
                            last.arguments.push_str(partial);
                        }
                    }
                    Vec::new()
                }
                _ => Vec::new(),
            }
        }
        "content_block_stop" => {
            // Check if we have a completed tool call
            let index = chunk["index"].as_u64().unwrap_or(0) as usize;
            let state = tool_calls_state.lock().await;
            if index < state.len() {
                let pending = state[index].clone();
                let args: serde_json::Value =
                    serde_json::from_str(&pending.arguments).unwrap_or_default();
                let tc = ToolCall {
                    id: pending.id,
                    name: pending.name,
                    arguments: args,
                };
                vec![StreamEvent::ToolCall(tc)]
            } else {
                Vec::new()
            }
        }
        "message_delta" => {
            let stop_reason = chunk["delta"]["stop_reason"].as_str().unwrap_or("");
            if stop_reason == "end_turn" || stop_reason == "tool_use" || stop_reason == "stop" {
                let usage = chunk["usage"].as_object().map(|u| Usage {
                    prompt_tokens: 0, // Anthropic reports input_tokens in message_start
                    completion_tokens: u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
                    total_tokens: u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
                });
                vec![StreamEvent::Finish { usage }]
            } else {
                Vec::new()
            }
        }
        "message_stop" => {
            vec![StreamEvent::Finish { usage: None }]
        }
        "error" => {
            let error_msg = chunk["error"]["message"]
                .as_str()
                .unwrap_or("Anthropic API error");
            vec![StreamEvent::Error(error_msg.to_string())]
        }
        _ => Vec::new(),
    }
}
