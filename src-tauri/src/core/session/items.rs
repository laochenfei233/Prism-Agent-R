use serde::{Deserialize, Serialize};

/// §19.3.2 会话三原语 Item/Turn/Thread
///
/// 对齐 Codex App Server 的 Item/Turn/Thread 三原语统一了流式事件与持久化

// ── Item 类型 ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ItemKind {
    UserMessage,
    AgentMessage,
    ToolExecution,
    ApprovalRequest,
    Diff,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ItemStatus {
    Started,
    Delta,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionItem {
    pub id: String,
    pub kind: ItemKind,
    pub status: ItemStatus,
    pub payload: serde_json::Value,
    pub created_at: i64,
    pub updated_at: i64,
}

impl SessionItem {
    pub fn new(kind: ItemKind, payload: serde_json::Value) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            kind,
            status: ItemStatus::Started,
            payload,
            created_at: now,
            updated_at: now,
        }
    }

    /// 标记为 Delta（流式更新）
    pub fn mark_delta(&mut self, delta: serde_json::Value) {
        self.status = ItemStatus::Delta;
        self.payload = delta;
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// 标记为 Completed
    pub fn complete(&mut self) {
        self.status = ItemStatus::Completed;
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// 标记为 Failed
    pub fn fail(&mut self) {
        self.status = ItemStatus::Failed;
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }
}

// ── Turn ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TurnStatus {
    Running,
    Completed,
    AwaitingApproval,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTurn {
    pub id: String,
    pub items: Vec<SessionItem>,
    pub status: TurnStatus,
    pub created_at: i64,
    pub updated_at: i64,
}

impl SessionTurn {
    pub fn new() -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            items: Vec::new(),
            status: TurnStatus::Running,
            created_at: now,
            updated_at: now,
        }
    }

    /// 添加 Item
    pub fn add_item(&mut self, item: SessionItem) {
        self.items.push(item);
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// 标记为 Completed
    pub fn complete(&mut self) {
        self.status = TurnStatus::Completed;
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// 标记为 AwaitingApproval
    pub fn await_approval(&mut self) {
        self.status = TurnStatus::AwaitingApproval;
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// 标记为 Cancelled
    pub fn cancel(&mut self) {
        self.status = TurnStatus::Cancelled;
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }
}

impl Default for SessionTurn {
    fn default() -> Self {
        Self::new()
    }
}

// ── Thread ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ThreadStatus {
    Active,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionThread {
    pub id: String,
    pub turns: Vec<SessionTurn>,
    pub status: ThreadStatus,
    pub created_at: i64,
    pub updated_at: i64,
}

impl SessionThread {
    pub fn new() -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            turns: Vec::new(),
            status: ThreadStatus::Active,
            created_at: now,
            updated_at: now,
        }
    }

    /// 添加 Turn
    pub fn add_turn(&mut self, turn: SessionTurn) {
        self.turns.push(turn);
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// 从指定 Turn 分支新会话（fork 语义）
    pub fn fork(&self, turn_id: &str) -> Option<SessionThread> {
        let fork_point = self.turns.iter().position(|t| t.id == turn_id)?;
        let now = chrono::Utc::now().timestamp_millis();

        let mut new_thread = SessionThread {
            id: uuid::Uuid::new_v4().to_string(),
            turns: self.turns[..=fork_point].to_vec(),
            status: ThreadStatus::Active,
            created_at: now,
            updated_at: now,
        };

        // 标记原线程为 Archived
        Some(new_thread)
    }

    /// 归档线程
    pub fn archive(&mut self) {
        self.status = ThreadStatus::Archived;
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }
}

impl Default for SessionThread {
    fn default() -> Self {
        Self::new()
    }
}

// ── 事件载荷 ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemEventPayload {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub kind: ItemKind,
    pub status: ItemStatus,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnEventPayload {
    pub thread_id: String,
    pub turn_id: String,
    pub status: TurnStatus,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_item_lifecycle() {
        let mut item = SessionItem::new(ItemKind::UserMessage, serde_json::json!({"text": "hello"}));
        assert_eq!(item.status, ItemStatus::Started);

        item.mark_delta(serde_json::json!({"delta": "world"}));
        assert_eq!(item.status, ItemStatus::Delta);

        item.complete();
        assert_eq!(item.status, ItemStatus::Completed);
    }

    #[test]
    fn test_turn_lifecycle() {
        let mut turn = SessionTurn::new();
        assert_eq!(turn.status, TurnStatus::Running);

        turn.add_item(SessionItem::new(ItemKind::UserMessage, serde_json::json!({})));
        assert_eq!(turn.items.len(), 1);

        turn.await_approval();
        assert_eq!(turn.status, TurnStatus::AwaitingApproval);

        turn.complete();
        assert_eq!(turn.status, TurnStatus::Completed);
    }

    #[test]
    fn test_thread_fork() {
        let mut thread = SessionThread::new();
        let turn1 = SessionTurn::new();
        let turn2 = SessionTurn::new();
        let turn2_id = turn2.id.clone();

        thread.add_turn(turn1);
        thread.add_turn(turn2);
        assert_eq!(thread.turns.len(), 2);

        let forked = thread.fork(&turn2_id).unwrap();
        assert_eq!(forked.turns.len(), 2);
    }

    #[test]
    fn test_thread_archive() {
        let mut thread = SessionThread::new();
        assert_eq!(thread.status, ThreadStatus::Active);

        thread.archive();
        assert_eq!(thread.status, ThreadStatus::Archived);
    }
}
