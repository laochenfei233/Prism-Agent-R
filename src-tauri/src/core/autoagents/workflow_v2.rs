use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::core::budget::config::{AgentBudget, CrewBudget};
use crate::core::guardrails::tool_guard::ToolPolicy;

// ── WorkflowV2 定义 ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowV2 {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub inputs: Vec<WorkflowInput>,
    pub stages: Vec<WorkflowStageV2>,
    pub budget: Option<CrewBudget>,
    pub guardrails: Option<ToolPolicy>,
    pub model_fallback: Option<Vec<String>>,
    pub on_exception: ExceptionPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowInput {
    pub key: String,
    pub label: String,
    pub kind: InputKind,
    pub default: Option<serde_json::Value>,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InputKind {
    Text,
    Textarea,
    Number,
    Select,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStageV2 {
    pub id: String,
    pub name: String,
    pub role: String,
    pub agent_id: Option<String>,
    pub prompt_template: String,
    pub tools: Vec<String>,
    pub max_iterations: Option<u32>,
    pub budget: Option<AgentBudget>,
    pub guardrails: Option<ToolPolicy>,
    pub depends_on: Vec<String>,
    pub retry_on_failure: Option<RetryPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub delay_ms: u64,
    pub backoff_multiplier: f64,
    pub retry_on_exceptions: Vec<String>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 2,
            delay_ms: 1000,
            backoff_multiplier: 2.0,
            retry_on_exceptions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExceptionPolicy {
    Terminate,
    ContinueAndLog,
    SkipStageAndContinue,
    PauseAndAsk,
}

impl Default for ExceptionPolicy {
    fn default() -> Self {
        Self::Terminate
    }
}

// ── WorkflowV2 运行结果 ──────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowResultV2 {
    pub run_id: String,
    pub outputs: HashMap<String, String>,
    pub stage_results: Vec<StageResultV2>,
    pub goal_status: Option<crate::core::autoagents::goal::GoalStatus>,
    pub budget_summary: Option<BudgetSummary>,
    pub exceptions: Vec<ExceptionRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageResultV2 {
    pub stage_id: String,
    pub status: StageStatus,
    pub output: Option<String>,
    pub error: Option<String>,
    pub tokens_used: Option<u64>,
    pub cost_used: Option<f64>,
    pub retries: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StageStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

impl StageStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetSummary {
    pub tokens_used: u64,
    pub cost_used: f64,
    pub iterations: u32,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExceptionRecord {
    pub exception_type: String,
    pub severity: String,
    pub message: String,
    pub stage_id: Option<String>,
}

// ── 辅助函数 ──────────────────────────────────────────────

pub fn topological_sort_v2(stages: &[WorkflowStageV2]) -> Result<Vec<WorkflowStageV2>, String> {
    let mut sorted = Vec::new();
    let mut remaining = stages.to_vec();
    let mut visited = std::collections::HashSet::new();

    while !remaining.is_empty() {
        let mut progress = false;
        let mut i = 0;
        while i < remaining.len() {
            let stage = &remaining[i];
            if stage.depends_on.iter().all(|dep| visited.contains(dep.as_str())) {
                visited.insert(stage.id.clone());
                sorted.push(remaining.remove(i));
                progress = true;
            } else {
                i += 1;
            }
        }
        if !progress {
            return Err("工作流存在循环依赖".into());
        }
    }
    Ok(sorted)
}

pub fn render_template_v2(
    template: &str,
    inputs: &HashMap<String, serde_json::Value>,
    outputs: &HashMap<String, String>,
) -> String {
    let mut result = template.to_string();
    for (key, value) in inputs {
        let placeholder = format!("{{{{{key}}}}}");
        let replacement = match value {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Null => String::new(),
            _ => serde_json::to_string(value).unwrap_or_default(),
        };
        result = result.replace(&placeholder, &replacement);
    }
    for (stage_id, output) in outputs {
        let placeholder = format!("{{{{{stage_id}.output}}}}");
        result = result.replace(&placeholder, output);
    }
    result
}
