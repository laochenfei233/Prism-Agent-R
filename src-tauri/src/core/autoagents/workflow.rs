use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::actor::ActorMessage;
use super::coordinator::Coordinator;
use crate::core::adk::error::AgentError;

// ── 工作流定义 ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub inputs: Vec<TaskInput>,
    pub stages: Vec<WorkflowStage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInput {
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
pub struct WorkflowStage {
    pub id: String,
    pub name: String,
    pub role: String,
    pub prompt_template: String,
    pub tools: Vec<String>,
    pub max_iterations: u32,
    pub depends_on: Vec<String>,
}

// ── 工作流运行结果 ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowResult {
    pub run_id: String,
    pub outputs: HashMap<String, String>,
    pub stage_results: Vec<StageResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageResult {
    pub stage_id: String,
    pub status: StageStatus,
    pub output: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StageStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

// ── 工作流引擎 ────────────────────────────────────────────

pub struct WorkflowEngine {
    coordinator: Arc<Coordinator>,
}

impl WorkflowEngine {
    pub fn new(coordinator: Arc<Coordinator>) -> Self {
        Self { coordinator }
    }

    /// 执行工作流
    pub async fn run(
        &self,
        workflow: &Workflow,
        inputs: HashMap<String, serde_json::Value>,
        run_id: &str,
    ) -> Result<WorkflowResult, AppError> {
        let mut outputs: HashMap<String, String> = HashMap::new();
        let mut stage_results = Vec::new();

        // 拓扑排序阶段
        let sorted_stages = topological_sort(&workflow.stages)?;

        for stage in &sorted_stages {
            // 检查依赖是否满足
            for dep in &stage.depends_on {
                if !outputs.contains_key(dep) {
                    stage_results.push(StageResult {
                        stage_id: stage.id.clone(),
                        status: StageStatus::Failed,
                        output: None,
                        error: Some(format!("依赖阶段 '{dep}' 未完成")),
                    });
                    continue;
                }
            }

            // 渲染模板
            let prompt = render_template(&stage.prompt_template, &inputs, &outputs)?;

            // 构建消息
            let msg = ActorMessage {
                task_id: format!("{}/{}", run_id, stage.id),
                prompt,
                tools: stage.tools.clone(),
                context: None,
            };

            // 派发任务
            stage_results.push(StageResult {
                stage_id: stage.id.clone(),
                status: StageStatus::Running,
                output: None,
                error: None,
            });

            match self.coordinator.dispatch(&stage.role, msg).await {
                Ok(reply) => {
                    outputs.insert(stage.id.clone(), reply.output.clone());
                    stage_results.pop();
                    stage_results.push(StageResult {
                        stage_id: stage.id.clone(),
                        status: StageStatus::Completed,
                        output: Some(reply.output),
                        error: None,
                    });
                }
                Err(e) => {
                    stage_results.pop();
                    stage_results.push(StageResult {
                        stage_id: stage.id.clone(),
                        status: StageStatus::Failed,
                        output: None,
                        error: Some(e.to_string()),
                    });
                    return Ok(WorkflowResult {
                        run_id: run_id.to_string(),
                        outputs,
                        stage_results,
                    });
                }
            }
        }

        Ok(WorkflowResult {
            run_id: run_id.to_string(),
            outputs,
            stage_results,
        })
    }
}

// ── 模板渲染 ──────────────────────────────────────────────

pub fn render_template(
    template: &str,
    inputs: &HashMap<String, serde_json::Value>,
    outputs: &HashMap<String, String>,
) -> Result<String, AppError> {
    let mut result = template.to_string();

    // 替换输入变量 {{key}}
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

    // 替换阶段输出 {{stage_id.output}}
    for (stage_id, output) in outputs {
        let placeholder = format!("{{{{{stage_id}.output}}}}");
        result = result.replace(&placeholder, output);
    }

    Ok(result)
}

// ── 拓扑排序 ──────────────────────────────────────────────

fn topological_sort(stages: &[WorkflowStage]) -> Result<Vec<WorkflowStage>, AppError> {
    let mut sorted = Vec::new();
    let mut remaining = stages.to_vec();
    let mut visited = std::collections::HashSet::new();

    while !remaining.is_empty() {
        let mut progress = false;
        let mut i = 0;
        while i < remaining.len() {
            let stage = &remaining[i];
            if stage.depends_on.iter().all(|dep| visited.contains(dep)) {
                visited.insert(stage.id.clone());
                sorted.push(remaining.remove(i));
                progress = true;
            } else {
                i += 1;
            }
        }

        if !progress {
            return Err(AppError::Validation("工作流存在循环依赖".into()));
        }
    }

    Ok(sorted)
}

// ── 任务定义（Phase 2 UI 设计区） ──────────────────────────

#[derive(Serialize, Deserialize, Clone)]
pub struct TaskDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub inputs: Vec<TaskInputDef>,
    pub stages: Vec<TaskStageDef>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TaskStageDef {
    pub id: String,
    pub name: String,
    pub role: String,
    pub agent_id: Option<String>,
    pub prompt_template: String,
    pub tools: Vec<String>,
    pub max_iterations: u32,
    pub depends_on: Vec<String>,
    pub model_hint: Option<String>,
    pub output_spec: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TaskInputDef {
    pub key: String,
    pub label: String,
    pub kind: InputKindDef,
    pub options: Option<Vec<serde_json::Value>>,
    pub default: Option<serde_json::Value>,
    pub required: bool,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum InputKindDef {
    Text,
    Textarea,
    Select,
    Number,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TaskValidationResult {
    pub ok: bool,
    pub errors: Vec<String>,
}

// ── AppError 转换 ─────────────────────────────────────────

impl From<AppError> for AgentError {
    fn from(e: AppError) -> Self {
        AgentError::Internal(e.to_string())
    }
}

// ── AppError 定义（简化版） ────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("验证错误: {0}")]
    Validation(String),
    #[error("内部错误: {0}")]
    Internal(String),
    #[error("Agent 错误: {0}")]
    Agent(#[from] AgentError),
}
