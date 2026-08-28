use async_trait::async_trait;
use futures::{SinkExt, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::core::adk::error::AgentError;
use crate::core::adk::model::{
    ChatRole, GenerationRequest, GenerationResponse, MessageContent, ModelCapabilities,
    ModelProvider, StreamEvent, StreamHandle, ToolCall, Usage,
};

// ── Responses API Provider ────────────────────────────────
/// Implements the OpenAI Responses API format (`POST /responses`).
/// Also compatible with DeepSeek and any endpoint supporting this protocol.
pub struct ResponsesApiProvider {
    id: String,
    display_name: String,
    api_key: String,
    base_url: String,
    model: String,
    client: Client,
}

impl ResponsesApiProvider {
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
impl ModelProvider for ResponsesApiProvider {
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
        let body = build_responses_body(&self.model, &request, false);
        let url = format!("{}/responses", self.base_url);

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
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

        let data: ResponsesApiResult = resp
            .json()
            .await
            .map_err(|e| AgentError::Provider(e.to_string()))?;

        parse_responses_result(&data)
    }

    async fn stream(&self, request: GenerationRequest) -> Result<StreamHandle, AgentError> {
        let body = build_responses_body(&self.model, &request, true);
        let url = format!("{}/responses", self.base_url);

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
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

        tokio::spawn(async move {
            let mut buffer = Vec::new();
            let mut stream = stream;

            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        buffer.extend_from_slice(&bytes);
                        // Process all complete SSE events (separated by \n\n)
                        while let Some(pos) = find_event_boundary(&buffer) {
                            let text = String::from_utf8_lossy(&buffer[..pos]).to_string();
                            buffer.drain(..pos);
                            let events = parse_responses_sse_buffer(&text);
                            for event in events {
                                if tx.send(event).await.is_err() {
                                    return; // consumer dropped
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

            // Flush remaining buffer
            if !buffer.is_empty() {
                let text = String::from_utf8_lossy(&buffer).to_string();
                let events = parse_responses_sse_buffer(&text);
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

// ── Request/Response Types ────────────────────────────────

#[derive(Serialize)]
struct ResponsesApiRequest {
    model: String,
    input: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<ResponsesApiTool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
    stream: bool,
}

#[derive(Serialize)]
struct ResponsesApiTool {
    r#type: String,
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Deserialize)]
struct ResponsesApiResult {
    output: Vec<ResponsesApiOutputItem>,
    usage: Option<ResponsesApiUsage>,
    #[allow(dead_code)]
    status: Option<String>,
}

#[derive(Deserialize)]
struct ResponsesApiOutputItem {
    #[serde(rename = "type")]
    item_type: Option<String>,
    content: Option<Vec<ResponsesApiContentPart>>,
    name: Option<String>,
    call_id: Option<String>,
    arguments: Option<String>,
}

#[derive(Deserialize)]
struct ResponsesApiContentPart {
    #[serde(rename = "type")]
    part_type: Option<String>,
    text: Option<String>,
}

#[derive(Deserialize)]
struct ResponsesApiUsage {
    input_tokens: u64,
    output_tokens: u64,
}

// ── Build Request Body ────────────────────────────────────

fn build_responses_body(
    model: &str,
    request: &GenerationRequest,
    stream: bool,
) -> ResponsesApiRequest {
    // Build input items from messages
    let mut input_items: Vec<serde_json::Value> = Vec::new();

    for msg in &request.messages {
        match msg.role {
            ChatRole::System => {
                if let MessageContent::Text(t) = &msg.content {
                    input_items.push(serde_json::json!({
                        "type": "message",
                        "role": "system",
                        "content": t,
                    }));
                }
            }
            ChatRole::User => {
                if let MessageContent::Text(t) = &msg.content {
                    input_items.push(serde_json::json!({
                        "type": "message",
                        "role": "user",
                        "content": t,
                    }));
                }
            }
            ChatRole::Assistant => match &msg.content {
                MessageContent::Text(t) => {
                    input_items.push(serde_json::json!({
                        "type": "message",
                        "role": "assistant",
                        "content": t,
                    }));
                }
                MessageContent::ToolCall(tc) => {
                    input_items.push(serde_json::json!({
                        "type": "function_call",
                        "call_id": tc.id,
                        "name": tc.name,
                        "arguments": serde_json::to_string(&tc.arguments).unwrap_or_default(),
                    }));
                }
                MessageContent::ToolResult(to) => {
                    input_items.push(serde_json::json!({
                        "type": "function_call_output",
                        "call_id": msg.name.clone().unwrap_or_default(),
                        "output": to.content,
                    }));
                }
            },
            ChatRole::Tool => {
                if let MessageContent::ToolResult(to) = &msg.content {
                    input_items.push(serde_json::json!({
                        "type": "function_call_output",
                        "call_id": msg.name.clone().unwrap_or_default(),
                        "output": to.content,
                    }));
                }
            }
        }
    }

    let input = if input_items.len() == 1 {
        // Single item: can be just the text string for simplicity
        if let Some(item) = input_items.first() {
            if item["type"] == "message" && item["role"] == "user" {
                item["content"].clone()
            } else {
                serde_json::json!(input_items)
            }
        } else {
            serde_json::json!(input_items)
        }
    } else {
        serde_json::json!(input_items)
    };

    let tools: Vec<ResponsesApiTool> = request
        .tools
        .iter()
        .map(|t| ResponsesApiTool {
            r#type: "function".to_string(),
            name: t.name.clone(),
            description: t.description.clone(),
            parameters: t.parameters.clone(),
        })
        .collect();

    ResponsesApiRequest {
        model: model.to_string(),
        input,
        instructions: request.system.clone(),
        tools,
        temperature: request.temperature,
        max_output_tokens: request.max_tokens,
        stream,
    }
}

// ── Parse Non-Streaming Response ───────────────────────────

fn parse_responses_result(data: &ResponsesApiResult) -> Result<GenerationResponse, AgentError> {
    let mut text = String::new();
    let mut tool_calls = Vec::new();

    for item in &data.output {
        match item.item_type.as_deref() {
            Some("message") => {
                if let Some(parts) = &item.content {
                    for part in parts {
                        if part.part_type.as_deref() == Some("output_text") {
                            if let Some(t) = &part.text {
                                text.push_str(t);
                            }
                        }
                    }
                }
            }
            Some("function_call") => {
                let args: serde_json::Value = item
                    .arguments
                    .as_deref()
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or_default();
                tool_calls.push(ToolCall {
                    id: item.call_id.clone().unwrap_or_default(),
                    name: item.name.clone().unwrap_or_default(),
                    arguments: args,
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

fn parse_responses_sse_buffer(text: &str) -> Vec<StreamEvent> {
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
        if let Some(evt) = parse_responses_sse_event(&event_type, &data) {
            events.push(evt);
        }
    }
    events
}

fn parse_responses_sse_event(event_type: &str, data: &str) -> Option<StreamEvent> {
    match event_type {
        "response.output_text.delta" => {
            let chunk: serde_json::Value = serde_json::from_str(data).ok()?;
            if let Some(delta) = chunk["delta"].as_str() {
                if !delta.is_empty() {
                    return Some(StreamEvent::Text(delta.to_string()));
                }
            }
            None
        }
        "response.reasoning_text.delta" => {
            let chunk: serde_json::Value = serde_json::from_str(data).ok()?;
            if let Some(delta) = chunk["delta"].as_str() {
                if !delta.is_empty() {
                    return Some(StreamEvent::Reasoning(delta.to_string()));
                }
            }
            None
        }
        "response.function_call_arguments.delta" => {
            // Accumulate function call args; emit as ToolCall when we see the done event
            // For now, we skip deltas and parse the full call on the done event
            None
        }
        "response.output_item.done" => {
            let chunk: serde_json::Value = serde_json::from_str(data).ok()?;
            if chunk["item"]["type"] == "function_call" {
                let call_id = chunk["item"]["call_id"].as_str().unwrap_or_default();
                let name = chunk["item"]["name"].as_str().unwrap_or_default();
                let args_str = chunk["item"]["arguments"].as_str().unwrap_or("{}");
                let args: serde_json::Value = serde_json::from_str(args_str).unwrap_or_default();
                return Some(StreamEvent::ToolCall(ToolCall {
                    id: call_id.to_string(),
                    name: name.to_string(),
                    arguments: args,
                }));
            }
            None
        }
        "response.completed" => {
            let chunk: serde_json::Value = serde_json::from_str(data).ok()?;
            let usage = chunk["response"]["usage"].as_object().map(|u| Usage {
                prompt_tokens: u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
                completion_tokens: u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
                total_tokens: u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0)
                    + u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
            });
            Some(StreamEvent::Finish { usage })
        }
        "response.incomplete" => {
            let chunk: serde_json::Value = serde_json::from_str(data).ok()?;
            let usage = chunk["response"]["usage"].as_object().map(|u| Usage {
                prompt_tokens: u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
                completion_tokens: u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
                total_tokens: u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0)
                    + u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
            });
            Some(StreamEvent::Finish { usage })
        }
        "response.failed" => {
            let chunk: serde_json::Value = serde_json::from_str(data).ok()?;
            let error_msg = chunk["error"]["message"]
                .as_str()
                .unwrap_or("Responses API error");
            Some(StreamEvent::Error(error_msg.to_string()))
        }
        _ => None,
    }
}
