use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::core::adk::error::AgentError;

// ── Actor ID ──────────────────────────────────────────────

pub type ActorId = String;

// ── Actor 消息 ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorMessage {
    pub task_id: String,
    pub prompt: String,
    pub tools: Vec<String>,
    pub context: Option<String>,
}

// ── Actor 回复 ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorReply {
    pub output: String,
    pub tool_calls: Vec<ToolCallInfo>,
    pub tokens_used: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallInfo {
    pub name: String,
    pub arguments: serde_json::Value,
    pub result: String,
}

// ── Agent Actor Trait ─────────────────────────────────────

#[async_trait]
pub trait AgentActor: Send + Sync {
    /// Actor 唯一标识
    fn actor_id(&self) -> ActorId;

    /// 角色名称（如 "researcher" / "writer" / "reviewer"）
    fn role(&self) -> &str;

    /// 接收任务消息，返回处理结果
    async fn handle(&self, msg: ActorMessage) -> Result<ActorReply, AgentError>;
}

// ── 内置 Actor 实现 ───────────────────────────────────────

use std::sync::Arc;
use crate::core::adk::model::{ChatMessage, ChatRole, GenerationRequest, MessageContent, ModelProvider};
use crate::core::adk::tool::ToolRegistry;
use crate::core::rig::agent::RigAgent;

/// 通用 Agent Actor：使用 RigAgent 执行任务
pub struct GenericActor {
    id: ActorId,
    role: String,
    agent: RigAgent,
}

impl GenericActor {
    pub fn new(
        id: ActorId,
        role: String,
        model_provider: Arc<dyn ModelProvider>,
        system_prompt: String,
        tools: ToolRegistry,
    ) -> Self {
        Self {
            id,
            role,
            agent: RigAgent::new(model_provider, system_prompt, tools),
        }
    }
}

#[async_trait]
impl AgentActor for GenericActor {
    fn actor_id(&self) -> ActorId {
        self.id.clone()
    }

    fn role(&self) -> &str {
        &self.role
    }

    async fn handle(&self, msg: ActorMessage) -> Result<ActorReply, AgentError> {
        let mut messages = Vec::new();

        // 添加上下文
        if let Some(ctx) = &msg.context {
            messages.push(ChatMessage {
                role: ChatRole::User,
                content: MessageContent::Text(format!("上下文信息：\n{ctx}")),
                name: None,
            });
        }

        // 添加任务提示
        messages.push(ChatMessage {
            role: ChatRole::User,
            content: MessageContent::Text(msg.prompt),
            name: None,
        });

        let request = GenerationRequest {
            messages,
            ..Default::default()
        };

        let result = self.agent.run(request).await?;

        Ok(ActorReply {
            output: result.text,
            tool_calls: result.tool_calls.into_iter().map(|tc| ToolCallInfo {
                name: tc.name,
                arguments: tc.arguments,
                result: String::new(),
            }).collect(),
            tokens_used: None,
        })
    }
}
