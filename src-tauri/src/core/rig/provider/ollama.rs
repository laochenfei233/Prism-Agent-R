use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;

use crate::core::adk::error::AgentError;
use crate::core::adk::model::{
    GenerationRequest, GenerationResponse, ModelCapabilities, ModelProvider, StreamEvent,
    StreamHandle,
};

// ── Ollama Provider ───────────────────────────────────────

pub struct OllamaProvider {
    id: String,
    display_name: String,
    base_url: String,
    model: String,
    client: Client,
}

impl OllamaProvider {
    pub fn new(id: String, display_name: String, base_url: String, model: String) -> Self {
        Self {
            id,
            display_name,
            base_url,
            model,
            client: Client::new(),
        }
    }
}

#[async_trait]
impl ModelProvider for OllamaProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn display_name(&self) -> &str {
        &self.display_name
    }

    fn capabilities(&self) -> ModelCapabilities {
        ModelCapabilities {
            max_tokens: 32768,
            supports_tools: true,
            supports_streaming: true,
            supports_vision: false,
        }
    }

    async fn generate(&self, request: GenerationRequest) -> Result<GenerationResponse, AgentError> {
        // Ollama uses OpenAI-compatible API
        let provider = super::openai::OpenAiProvider::new(
            self.id.clone(),
            self.display_name.clone(),
            "ollama".to_string(),
            self.base_url.clone(),
            self.model.clone(),
        );
        provider.generate(request).await
    }

    async fn stream(&self, request: GenerationRequest) -> Result<StreamHandle, AgentError> {
        let url = format!("{}/api/chat", self.base_url);

        let mut messages = Vec::new();
        if let Some(sys) = &request.system {
            messages.push(serde_json::json!({
                "role": "system",
                "content": sys,
            }));
        }
        for msg in &request.messages {
            messages.push(serde_json::json!({
                "role": match msg.role {
                    crate::core::adk::model::ChatRole::System => "system",
                    crate::core::adk::model::ChatRole::User => "user",
                    crate::core::adk::model::ChatRole::Assistant => "assistant",
                    crate::core::adk::model::ChatRole::Tool => "tool",
                },
                "content": match &msg.content {
                    crate::core::adk::model::MessageContent::Text(t) => t.clone(),
                    crate::core::adk::model::MessageContent::ToolCall(_) => String::new(),
                    crate::core::adk::model::MessageContent::ToolResult(t) => t.content.clone(),
                },
            }));
        }

        let body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "stream": true,
        });

        let resp = self
            .client
            .post(&url)
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
                        // Ollama sends JSON lines, not SSE
                        for line in text.lines() {
                            let line = line.trim();
                            if line.is_empty() {
                                continue;
                            }
                            if let Ok(chunk) = serde_json::from_str::<serde_json::Value>(line) {
                                if let Some(content) = chunk["message"]["content"].as_str() {
                                    if !content.is_empty() {
                                        return Some(StreamEvent::Text(content.to_string()));
                                    }
                                }
                                if chunk["done"].as_bool() == Some(true) {
                                    return Some(StreamEvent::Finish { usage: None });
                                }
                            }
                        }
                        None
                    }
                    Err(_) => None,
                }
            }
        });

        Ok(Box::pin(mapped))
    }
}
