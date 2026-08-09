use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub groups: Vec<ExecutionGroup>,
    pub total_tasks: u32,
    pub estimated_tokens: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionGroup {
    pub id: String,
    pub kind: GroupKind,
    pub tasks: Vec<PlannedTask>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupKind {
    #[serde(alias = "Parallel")]
    Parallel,
    #[serde(alias = "Sequential")]
    Sequential,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedTask {
    pub spec_task_id: String,
    pub agent_config: AgentConfig,
    pub prompt: String,
    pub tools: Vec<String>,
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub role: String,
    pub model_provider: String,
    pub model_id: String,
    pub system_prompt: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

// ── Review Result ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewResult {
    pub task_reviews: Vec<TaskReview>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskReview {
    pub task_id: String,
    pub passed: bool,
    pub reasons: Vec<String>,
    pub suggestions: Vec<String>,
}

impl ReviewResult {
    pub fn all_passed(&self) -> bool {
        self.task_reviews.iter().all(|r| r.passed)
    }

    pub fn failed_tasks(&self) -> Vec<&TaskReview> {
        self.task_reviews.iter().filter(|r| !r.passed).collect()
    }
}

// ── Task Result ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: String,
    pub status: TaskStatus,
    pub output: String,
    pub tokens_used: Option<u64>,
    pub duration_ms: i64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskStatus {
    Completed,
    Failed,
}
