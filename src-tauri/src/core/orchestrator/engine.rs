use std::sync::Arc;

use super::session::{OrchestratorSession, OrchestratorStatus, OrchestratorEvent};
use super::spec::{SpecDocument, SpecTask, Complexity};
use super::plan::{ExecutionPlan, ExecutionGroup, GroupKind, PlannedTask, AgentConfig, ReviewResult, TaskReview, TaskResult, TaskStatus};
use crate::core::budget::tracker::BudgetTracker;
use crate::core::observability::exception::ExceptionRecorder;
use crate::utils::error::AppError;

pub struct OrchestratorEngine {
    pub budget_tracker: Arc<BudgetTracker>,
    pub exception_recorder: Option<Arc<ExceptionRecorder>>,
    on_event: Option<Box<dyn Fn(&OrchestratorEvent) + Send + Sync>>,
}

impl OrchestratorEngine {
    pub fn new(budget_tracker: Arc<BudgetTracker>) -> Self {
        Self {
            budget_tracker,
            exception_recorder: None,
            on_event: None,
        }
    }

    pub fn on_event<F>(mut self, f: F) -> Self
    where
        F: Fn(&OrchestratorEvent) + Send + Sync + 'static,
    {
        self.on_event = Some(Box::new(f));
        self
    }

    fn emit_event(&self, _session: &OrchestratorSession, event_type: &str, message: &str) {
        let event = OrchestratorEvent {
            event_type: event_type.into(),
            message: message.into(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            data: None,
        };
        if let Some(f) = &self.on_event {
            f(&event);
        }
    }

    /// 主循环：Spec → Plan → Execute → Review → 循环
    pub async fn run(&self, session: &mut OrchestratorSession) -> Result<(), AppError> {
        loop {
            match &session.status {
                OrchestratorStatus::SpecGenerating => {
                    self.emit_event(session, "spec_generating", "正在分析需求...");
                    let spec = self.generate_spec(&session.user_request).await?;
                    session.spec = Some(spec);
                    session.status = OrchestratorStatus::PlanGenerating;
                    self.emit_event(session, "spec_generated", "SPEC 生成完成");
                }

                OrchestratorStatus::SpecReviewing => {
                    // Skip to plan generation
                    session.status = OrchestratorStatus::PlanGenerating;
                }

                OrchestratorStatus::PlanGenerating => {
                    self.emit_event(session, "plan_generating", "正在生成执行计划...");
                    let spec = session.spec.as_ref().unwrap();
                    let plan = self.generate_plan(spec).await?;
                    session.plan = Some(plan);
                    session.status = OrchestratorStatus::Executing;
                    self.emit_event(session, "plan_generated", "执行计划生成完成");
                }

                OrchestratorStatus::Executing => {
                    self.emit_event(session, "executing", "正在执行任务...");
                    let results = self.execute_plan(session).await;
                    self.record_results(session, &results).await;
                    session.status = OrchestratorStatus::Reviewing;
                    self.emit_event(session, "execution_completed", "执行完成，开始审查");
                }

                OrchestratorStatus::Reviewing => {
                    self.emit_event(session, "reviewing", "正在审查结果...");
                    let review = self.review_results(session).await;
                    if review.all_passed() {
                        session.status = OrchestratorStatus::Completed;
                        self.emit_event(session, "review_passed", "所有任务通过审查");
                        break;
                    } else {
                        session.cycle_count += 1;
                        if session.cycle_count >= session.max_cycles {
                            session.status = OrchestratorStatus::Failed("达到最大循环次数".into());
                            self.emit_event(session, "max_cycles", "达到最大循环次数");
                            break;
                        }
                        session.status = OrchestratorStatus::Repairing;
                        self.emit_event(session, "review_failed", "审查未通过，生成修复计划");
                    }
                }

                OrchestratorStatus::Repairing => {
                    self.emit_event(session, "repairing", "正在修复失败任务...");
                    // Simplified: just re-execute
                    session.status = OrchestratorStatus::Executing;
                    self.emit_event(session, "repair_plan", "修复计划生成完成");
                }

                OrchestratorStatus::Paused | OrchestratorStatus::BudgetExhausted => {
                    self.emit_event(session, "paused", "会话已暂停");
                    break;
                }

                OrchestratorStatus::Completed | OrchestratorStatus::Failed(_) => {
                    break;
                }
            }

            // Check budget
            match self.budget_tracker.check_global_budget().await {
                Ok(crate::core::budget::tracker::BudgetCheckResult::Exceeded { .. }) => {
                    session.status = OrchestratorStatus::BudgetExhausted;
                    self.emit_event(session, "budget_exhausted", "预算耗尽");
                    break;
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// 生成 SPEC（简化版 - 实际应调用 LLM）
    async fn generate_spec(&self, user_request: &str) -> Result<SpecDocument, AppError> {
        // Simplified: generate a basic spec from the request
        let tasks = vec![
            SpecTask {
                id: "T1".into(),
                title: "分析需求".into(),
                description: format!("分析用户需求: {user_request}"),
                acceptance: vec!["输出需求分析文档".into()],
                estimated_complexity: Complexity::Medium,
                required_tools: vec![],
                suggested_model: None,
            },
        ];

        let mut acceptance_criteria = std::collections::HashMap::new();
        acceptance_criteria.insert("T1".into(), vec!["输出需求分析文档".into()]);

        let dependencies = std::collections::HashMap::new();

        Ok(SpecDocument {
            id: uuid::Uuid::new_v4().to_string(),
            summary: format!("需求分析: {user_request}"),
            tasks,
            acceptance_criteria,
            dependencies,
            out_of_scope: vec![],
        })
    }

    /// 生成执行计划（简化版）
    async fn generate_plan(&self, spec: &SpecDocument) -> Result<ExecutionPlan, AppError> {
        let mut groups = Vec::new();
        for task in &spec.tasks {
            groups.push(ExecutionGroup {
                id: uuid::Uuid::new_v4().to_string(),
                kind: GroupKind::Sequential,
                tasks: vec![PlannedTask {
                    spec_task_id: task.id.clone(),
                    agent_config: AgentConfig {
                        role: "assistant".into(),
                        model_provider: "openai".into(),
                        model_id: "gpt-4o".into(),
                        system_prompt: None,
                        temperature: None,
                        max_tokens: None,
                    },
                    prompt: task.description.clone(),
                    tools: task.required_tools.clone(),
                    timeout_secs: Some(300),
                }],
            });
        }

        Ok(ExecutionPlan {
            groups,
            total_tasks: spec.tasks.len() as u32,
            estimated_tokens: None,
        })
    }

    /// 执行计划（简化版 - 实际应调用 Actor）
    async fn execute_plan(&self, session: &OrchestratorSession) -> Vec<TaskResult> {
        let plan = match &session.plan {
            Some(p) => p,
            None => return vec![],
        };

        let mut results = Vec::new();
        for group in &plan.groups {
            for task in &group.tasks {
                results.push(TaskResult {
                    task_id: task.spec_task_id.clone(),
                    status: TaskStatus::Completed,
                    output: format!("任务 {} 执行完成", task.spec_task_id),
                    tokens_used: Some(100),
                    duration_ms: 1000,
                    error: None,
                });
            }
        }
        results
    }

    async fn record_results(&self, _session: &OrchestratorSession, _results: &[TaskResult]) {
        // Record results to budget tracker
    }

    /// 审查结果（简化版）
    async fn review_results(&self, session: &OrchestratorSession) -> ReviewResult {
        let spec = match &session.spec {
            Some(s) => s,
            None => return ReviewResult { task_reviews: vec![] },
        };

        let task_reviews: Vec<TaskReview> = spec.tasks.iter().map(|task| {
            TaskReview {
                task_id: task.id.clone(),
                passed: true,
                reasons: vec![],
                suggestions: vec![],
            }
        }).collect();

        ReviewResult { task_reviews }
    }
}
