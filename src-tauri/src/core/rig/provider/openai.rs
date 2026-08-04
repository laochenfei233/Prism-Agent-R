use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::core::adk::error::AgentError;
use crate::core::adk::model::{
    ChatRole, GenerationRequest, GenerationResponse, MessageContent, ModelCapabilities,
    ModelProvider, StreamEvent, StreamHandle, ToolCall, Usage,
};

// ── OpenAI Provider ───────────────────────────────────────

pub struct OpenAiProvider {
    id: String,
    display_name: String,
    api_key: String,
    base_url: String,
    model: String,
    client: Client,
}

impl OpenAiProvider {
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
impl ModelProvider for OpenAiProvider {
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
            supports_vision: true,
        }
    }

    async fn generate(
        &self,
        request: GenerationRequest,
    ) -> Result<GenerationResponse, AgentError> {
        let body = build_request_body(&self.model, &request, false);
        let url = format!("{}/chat/completions", self.base_url);

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

        let data: OpenAiResponse = resp
            .json()
            .await
            .map_err(|e| AgentError::Provider(e.to_string()))?;

        let choice = data.choices.first().ok_or_else(|| {
            AgentError::Provider("No choices in response".to_string())
        })?;

        let mut tool_calls = Vec::new();
        let mut text = choice.message.content.clone().unwrap_or_default();

        if let Some(tc) = &choice.message.tool_calls {
            for t in tc.iter() {
                let args: serde_json::Value =
                    serde_json::from_str(&t.function.arguments).unwrap_or_default();
                tool_calls.push(ToolCall {
                    id: t.id.clone(),
                    name: t.function.name.clone(),
                    arguments: args,
                });
            }
            if text.is_empty() && !tool_calls.is_empty() {
                text = String::new();
            }
        }

        Ok(GenerationResponse {
            text,
            tool_calls,
            usage: data.usage.map(|u| Usage {
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
            }),
        })
    }

    async fn stream(&self, request: GenerationRequest) -> Result<StreamHandle, AgentError> {
        let body = build_request_body(&self.model, &request, true);
        let url = format!("{}/chat/completions", self.base_url);

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

        let stream = resp.bytes_stream();
        let mapped = stream.filter_map(move |chunk| {
            async move {
                match chunk {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes).to_string();
                        parse_sse_chunk(&text)
                    }
                    Err(_) => None,
                }
            }
        });

        Ok(Box::pin(mapped))
    }
}

// ── Request/Response Types ────────────────────────────────

#[derive(Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<OpenAiTool>,
    stream: bool,
}

#[derive(Serialize, Deserialize)]
struct OpenAiMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAiToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct OpenAiTool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAiFunction,
}

#[derive(Serialize, Deserialize)]
struct OpenAiFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Serialize, Deserialize)]
struct OpenAiToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: OpenAiToolCallFunction,
}

#[derive(Serialize, Deserialize)]
struct OpenAiToolCallFunction {
    name: String,
    arguments: String,
}

#[derive(Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
}

#[derive(Deserialize)]
struct OpenAiUsage {
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
}

fn build_request_body(
    model: &str,
    request: &GenerationRequest,
    stream: bool,
) -> OpenAiRequest {
    let mut messages = Vec::new();

    if let Some(sys) = &request.system {
        messages.push(OpenAiMessage {
            role: "system".to_string(),
            content: Some(sys.clone()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        });
    }

    for msg in &request.messages {
        let role = match msg.role {
            ChatRole::System => "system",
            ChatRole::User => "user",
            ChatRole::Assistant => "assistant",
            ChatRole::Tool => "tool",
        };

        let (content, tool_calls, tool_call_id) = match &msg.content {
            MessageContent::Text(t) => (Some(t.clone()), None, None),
            MessageContent::ToolCall(tc) => (
                None,
                Some(vec![OpenAiToolCall {
                    id: tc.id.clone(),
                    call_type: "function".to_string(),
                    function: OpenAiToolCallFunction {
                        name: tc.name.clone(),
                        arguments: serde_json::to_string(&tc.arguments).unwrap_or_default(),
                    },
                }]),
                None,
            ),
            MessageContent::ToolResult(to) => (Some(to.content.clone()), None, None),
        };

        messages.push(OpenAiMessage {
            role: role.to_string(),
            content,
            name: msg.name.clone(),
            tool_calls,
            tool_call_id,
        });
    }

    let tools: Vec<OpenAiTool> = request
        .tools
        .iter()
        .map(|t| OpenAiTool {
            tool_type: "function".to_string(),
            function: OpenAiFunction {
                name: t.name.clone(),
                description: t.description.clone(),
                parameters: t.parameters.clone(),
            },
        })
        .collect();

    OpenAiRequest {
        model: model.to_string(),
        messages,
        temperature: request.temperature,
        max_tokens: request.max_tokens,
        stop: request.stop.clone(),
        tools,
        stream,
    }
}

pub fn parse_sse_chunk(text: &str) -> Option<StreamEvent> {
    for line in text.lines() {
        let line = line.trim();
        if !line.starts_with("data: ") {
            continue;
        }
        let data = &line[6..];
        if data == "[DONE]" {
            return Some(StreamEvent::Finish { usage: None });
        }
        if let Ok(chunk) = serde_json::from_str::<OpenAiStreamChunk>(data) {
            if let Some(choice) = chunk.choices.first() {
                if let Some(delta) = &choice.delta {
                    if let Some(content) = &delta.content {
                        if !content.is_empty() {
                            return Some(StreamEvent::Text(content.clone()));
                        }
                    }
                    if let Some(tc) = &delta.tool_calls {
                        for t in tc {
                            if let Some(name) = &t.function {
                                let args: serde_json::Value =
                                    serde_json::from_str(&name.arguments).unwrap_or_default();
                                return Some(StreamEvent::ToolCall(ToolCall {
                                    id: t.id.clone().unwrap_or_default(),
                                    name: name.name.clone().unwrap_or_default(),
                                    arguments: args,
                                }));
                            }
                        }
                    }
                }
                if choice.finish_reason.as_deref() == Some("stop") {
                    return Some(StreamEvent::Finish { usage: None });
                }
            }
        }
    }
    None
}

#[derive(Deserialize)]
struct OpenAiStreamChunk {
    choices: Vec<OpenAiStreamChoice>,
}

#[derive(Deserialize)]
struct OpenAiStreamChoice {
    delta: Option<OpenAiStreamDelta>,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct OpenAiStreamDelta {
    content: Option<String>,
    tool_calls: Option<Vec<OpenAiStreamToolCall>>,
}

#[derive(Deserialize)]
struct OpenAiStreamToolCall {
    id: Option<String>,
    function: Option<OpenAiStreamFunction>,
}

#[derive(Deserialize)]
struct OpenAiStreamFunction {
    name: Option<String>,
    arguments: String,
}
