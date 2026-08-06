use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::actor::{ActorId, ActorMessage, ActorReply, AgentActor};
use crate::core::adk::error::AgentError;

// ── 协调器 ────────────────────────────────────────────────

pub struct Coordinator {
    actors: RwLock<HashMap<ActorId, Arc<dyn AgentActor>>>,
    role_index: RwLock<HashMap<String, Vec<ActorId>>>,
}

impl Coordinator {
    pub fn new() -> Self {
        Self {
            actors: RwLock::new(HashMap::new()),
            role_index: RwLock::new(HashMap::new()),
        }
    }

    /// 注册一个 Actor
    pub async fn register(&self, actor: Arc<dyn AgentActor>) {
        let id = actor.actor_id();
        let role = actor.role().to_string();

        self.actors.write().await.insert(id.clone(), actor);
        self.role_index.write().await
            .entry(role)
            .or_default()
            .push(id);
    }

    /// 按角色派发任务（使用第一个匹配的 Actor）
    pub async fn dispatch(&self, role: &str, msg: ActorMessage) -> Result<ActorReply, AgentError> {
        let actors = self.actors.read().await;
        let index = self.role_index.read().await;

        let actor_id = index.get(role)
            .and_then(|ids| ids.first())
            .ok_or_else(|| AgentError::Internal(format!("没有找到角色为 '{role}' 的 Actor")))?;

        let actor = actors.get(actor_id)
            .ok_or_else(|| AgentError::Internal(format!("Actor 不存在: {actor_id}")))?;

        actor.handle(msg).await
    }

    /// 按 ID 派发任务
    pub async fn dispatch_to(&self, actor_id: &str, msg: ActorMessage) -> Result<ActorReply, AgentError> {
        let actors = self.actors.read().await;
        let actor = actors.get(actor_id)
            .ok_or_else(|| AgentError::Internal(format!("Actor 不存在: {actor_id}")))?;

        actor.handle(msg).await
    }

    /// 获取所有已注册的角色
    pub async fn roles(&self) -> Vec<String> {
        self.role_index.read().await.keys().cloned().collect()
    }

    /// 获取指定角色的 Actor ID 列表
    pub async fn actors_by_role(&self, role: &str) -> Vec<ActorId> {
        self.role_index.read().await
            .get(role)
            .cloned()
            .unwrap_or_default()
    }
}

impl Default for Coordinator {
    fn default() -> Self {
        Self::new()
    }
}
