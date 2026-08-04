use std::sync::Arc;

use crate::core::adk::error::AgentError;
use crate::core::adk::model::{
    ChatMessage, ChatRole, GenerationRequest, MessageContent, ModelProvider, StreamEvent, ToolCall,
    ToolOutput,
};
use crate::core::adk::tool::ToolRegistry;

// ── Rig Agent ─────────────────────────────────────────────

pub struct RigAgent {
    pub model_provider: Arc<dyn ModelProvider>,
    pub system_prompt: String,
    pub tools: ToolRegistry,
    pub max_iterations: u32,
}

pub struct AgentRunResult {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
}

impl RigAgent {
    pub fn new(
        model_provider: Arc<dyn ModelProvider>,
        system_prompt: String,
        tools: ToolRegistry,
    ) -> Self {
        Self {
            model_provider,
            system_prompt,
            tools,
            max_iterations: 20,
        }
    }

    /// Execute agentic loop: generate → tool calls → fill results → regenerate
    pub async fn run(&self, request: GenerationRequest) -> Result<AgentRunResult, AgentError> {
        let mut current = request;

        for _ in 0..self.max_iterations {
            // Build full request with system prompt
            let mut req = current.clone();
            if !self.system_prompt.is_empty() {
                req.system = Some(self.system_prompt.clone());
            }

            // Add tool specs
            req.tools = self.tools.specs();

            // Generate
            let mut handle = self.model_provider.stream(req).await?;
            let mut tool_calls = Vec::new();
            let mut final_text = String::new();

            // Consume stream
            use futures::StreamExt;
            while let Some(event) = handle.next().await {
                match event {
                    StreamEvent::Text(t) => final_text.push_str(&t),
                    StreamEvent::ToolCall(call) => tool_calls.push(call),
                    StreamEvent::Finish { .. } => break,
                    StreamEvent::Error(e) => return Err(AgentError::Stream(e)),
                }
            }

            // No tool calls = done
            if tool_calls.is_empty() {
                return Ok(AgentRunResult {
                    text: final_text,
                    tool_calls: Vec::new(),
                });
            }

            // Execute tools and fill results
            let mut tool_results = Vec::new();
            for call in &tool_calls {
                let output = self.execute_tool(call).await;
                tool_results.push(ChatMessage {
                    role: ChatRole::Tool,
                    content: MessageContent::ToolResult(output),
                    name: Some(call.name.clone()),
                });
            }

            // Add assistant message and tool results to conversation
            current.messages.push(ChatMessage {
                role: ChatRole::Assistant,
                content: MessageContent::Text(final_text),
                name: None,
            });
            current.messages.extend(tool_results);
        }

        Err(AgentError::MaxIterations)
    }

    async fn execute_tool(&self, call: &ToolCall) -> ToolOutput {
        match self.tools.get(&call.name) {
            Some(tool) => match tool.execute(call.arguments.clone()).await {
                Ok(output) => output,
                Err(e) => ToolOutput::error(format!("Tool error: {e}")),
            },
            None => ToolOutput::error(format!("Unknown tool: {}", call.name)),
        }
    }
}
