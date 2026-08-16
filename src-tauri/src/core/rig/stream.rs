use futures::StreamExt;
use tauri::Emitter;

use crate::core::adk::error::AgentError;
use crate::core::adk::model::{
    GenerationRequest, ModelProvider, StreamEvent, Usage,
};

// ── Stream Pipeline ───────────────────────────────────────

pub struct StreamPipeline {
    app: Option<tauri::AppHandle>,
}

impl StreamPipeline {
    pub fn new(app: Option<tauri::AppHandle>) -> Self {
        Self { app }
    }

    pub async fn run(
        &self,
        provider: &dyn ModelProvider,
        request: GenerationRequest,
        cancel: tokio_util::sync::CancellationToken,
        session_id: String,
        message_id: String,
    ) -> Result<Option<Usage>, AgentError> {
        let mut handle = provider.stream(request).await?;

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    self.emit_event("chat:stream:aborted", &session_id, &message_id);
                    return Ok(None);
                }
                event = handle.next() => {
                    match event {
                        Some(StreamEvent::Text(delta)) => {
                            self.emit_delta(&session_id, &message_id, &delta);
                        }
                        Some(StreamEvent::Reasoning(delta)) => {
                            self.emit_reasoning(&session_id, &message_id, &delta);
                        }
                        Some(StreamEvent::ToolCall(call)) => {
                            self.emit_tool_call(&session_id, &message_id, &call);
                        }
                        Some(StreamEvent::Finish { usage }) => {
                            self.emit_done(&session_id, &message_id, &usage);
                            return Ok(usage);
                        }
                        Some(StreamEvent::Error(e)) => {
                            self.emit_error(&session_id, &message_id, &e);
                            return Err(AgentError::Stream(e));
                        }
                        None => break,
                    }
                }
            }
        }

        Ok(None)
    }

    fn emit_event(&self, event: &str, session_id: &str, message_id: &str) {
        if let Some(app) = &self.app {
            let _ = app.emit(event, serde_json::json!({
                "session_id": session_id,
                "message_id": message_id,
            }));
        }
    }

    fn emit_delta(&self, session_id: &str, message_id: &str, delta: &str) {
        if let Some(app) = &self.app {
            let _ = app.emit("chat:stream:delta", serde_json::json!({
                "session_id": session_id,
                "message_id": message_id,
                "delta": delta,
            }));
        }
    }

    fn emit_reasoning(&self, session_id: &str, message_id: &str, delta: &str) {
        if let Some(app) = &self.app {
            let _ = app.emit("chat:stream:reasoning", serde_json::json!({
                "session_id": session_id,
                "message_id": message_id,
                "delta": delta,
            }));
        }
    }

    fn emit_tool_call(
        &self,
        session_id: &str,
        message_id: &str,
        call: &crate::core::adk::model::ToolCall,
    ) {
        if let Some(app) = &self.app {
            let _ = app.emit("chat:stream:tool_call", serde_json::json!({
                "session_id": session_id,
                "message_id": message_id,
                "call": {
                    "id": call.id,
                    "name": call.name,
                    "arguments": call.arguments,
                },
            }));
        }
    }

    fn emit_done(&self, session_id: &str, message_id: &str, usage: &Option<Usage>) {
        if let Some(app) = &self.app {
            let _ = app.emit("chat:stream:done", serde_json::json!({
                "session_id": session_id,
                "message_id": message_id,
                "usage": usage,
            }));
        }
    }

    fn emit_error(&self, session_id: &str, message_id: &str, message: &str) {
        if let Some(app) = &self.app {
            let _ = app.emit("chat:stream:error", serde_json::json!({
                "session_id": session_id,
                "message_id": message_id,
                "message": message,
            }));
        }
    }
}
