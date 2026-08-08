use serde::{Deserialize, Serialize};

use super::spec::SpecDocument;
use super::plan::ExecutionPlan;

/// 自主编排会话
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorSession {
    pub id: String,
    pub user_request: String,
    pub spec: Option<SpecDocument>,
    pub plan: Option<ExecutionPlan>,
    pub status: OrchestratorStatus,
    pub cycle_count: u32,
    pub max_cycles: u32,
    pub history: Vec<OrchestratorEvent>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrchestratorStatus {
    SpecGenerating,
    SpecReviewing,
    PlanGenerating,
    Executing,
    Reviewing,
    Repairing,
    Completed,
    Paused,
    BudgetExhausted,
    Failed(String),
}

impl OrchestratorStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SpecGenerating => "spec_generating",
            Self::SpecReviewing => "spec_reviewing",
            Self::PlanGenerating => "plan_generating",
            Self::Executing => "executing",
            Self::Reviewing => "reviewing",
            Self::Repairing => "repairing",
            Self::Completed => "completed",
            Self::Paused => "paused",
            Self::BudgetExhausted => "budget_exhausted",
            Self::Failed(_) => "failed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorEvent {
    pub event_type: String,
    pub message: String,
    pub timestamp: i64,
    pub data: Option<serde_json::Value>,
}

impl OrchestratorSession {
    pub fn new(user_request: String, max_cycles: u32) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            user_request,
            spec: None,
            plan: None,
            status: OrchestratorStatus::SpecGenerating,
            cycle_count: 0,
            max_cycles,
            history: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn push_event(&mut self, event: OrchestratorEvent) {
        self.history.push(event);
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    pub fn all_tasks_passed(&self) -> bool {
        // Check if all spec tasks have been reviewed and passed
        // This is a simplified check - the actual implementation tracks task results
        false
    }
}
