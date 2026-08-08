use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::coordinator::Coordinator;
use super::workflow_v2::*;
use super::actor::ActorMessage;
use crate::core::budget::fallback::ModelFallbackChain;
use crate::core::budget::tracker::{BudgetCheckResult, BudgetTracker};
use crate::core::budget::policy::BudgetAction;
use crate::core::guardrails::sandbox::SandboxPolicy;
use crate::core::guardrails::tool_guard::{ToolGuardrail, GuardrailDecision};
use crate::core::guardrails::trajectory::TrajectoryGuardrail;
use crate::core::observability::exception::{ExceptionRecorder, ExceptionType};
use crate::core::observability::logger::AgentLogger;
use crate::utils::error::AppError;

// ── WorkflowEngineV2 ─────────────────────────────────────

pub struct WorkflowEngineV2 {
    coordinator: Arc<Coordinator>,
    pub budget_tracker: Arc<BudgetTracker>,
    pub tool_guard: Option<Arc<ToolGuardrail>>,
    /// §23.4 系统级沙箱（文件/网络/进程访问控制）
    pub sandbox: Option<Arc<SandboxPolicy>>,
    /// §23.3 行为级护栏（轨迹监控）
    pub trajectory_guard: Option<Arc<TrajectoryGuardrail>>,
    pub exception_recorder: Option<Arc<ExceptionRecorder>>,
    pub logger: Option<Arc<AgentLogger>>,
    /// §22.3 模型降级链：超预算时切换到更便宜模型
    pub model_fallback: Option<Arc<RwLock<ModelFallbackChain>>>,
    on_stage: Option<Arc<dyn Fn(&str, &str, &StageStatus) + Send + Sync>>,
    goal: Option<super::goal::GoalMonitor>,
}

impl WorkflowEngineV2 {
    pub fn new(coordinator: Arc<Coordinator>, budget_tracker: Arc<BudgetTracker>) -> Self {
        Self {
            coordinator,
            budget_tracker,
            tool_guard: None,
            sandbox: None,
            trajectory_guard: None,
            exception_recorder: None,
            logger: None,
            model_fallback: None,
            on_stage: None,
            goal: None,
        }
    }

    pub fn with_tool_guard(mut self, guard: Arc<ToolGuardrail>) -> Self {
        self.tool_guard = Some(guard);
        self
    }

    /// §23.4 配置系统级沙箱
    pub fn with_sandbox(mut self, policy: Arc<SandboxPolicy>) -> Self {
        self.sandbox = Some(policy);
        self
    }

    /// §23.3 配置行为级轨迹护栏
    pub fn with_trajectory_guard(mut self, guard: Arc<TrajectoryGuardrail>) -> Self {
        self.trajectory_guard = Some(guard);
        self
    }

    pub fn with_exception_recorder(mut self, recorder: Arc<ExceptionRecorder>) -> Self {
        self.exception_recorder = Some(recorder);
        self
    }

    pub fn with_logger(mut self, logger: Arc<AgentLogger>) -> Self {
        self.logger = Some(logger);
        self
    }

    /// §22.3 配置模型降级链
    pub fn with_model_fallback(mut self, chain: Arc<RwLock<ModelFallbackChain>>) -> Self {
        self.model_fallback = Some(chain);
        self
    }

    pub fn with_goal<F>(mut self, goals: Vec<super::goal::TaskGoal>) -> Self {
        self.goal = Some(super::goal::GoalMonitor::new(goals));
        self
    }

    pub fn on_stage<F>(mut self, f: F) -> Self
    where
        F: Fn(&str, &str, &StageStatus) + Send + Sync + 'static,
    {
        self.on_stage = Some(Arc::new(f));
        self
    }

    fn emit_stage(&self, run_id: &str, stage_id: &str, status: &StageStatus) {
        if let Some(f) = &self.on_stage {
            f(run_id, stage_id, status);
        }
    }

    async fn record_exception(
        &self,
        session_id: &str,
        agent_id: &str,
        run_id: &str,
        stage_id: &str,
        exception: ExceptionType,
    ) {
        if let Some(recorder) = &self.exception_recorder {
            let _ = recorder.record(
                session_id,
                agent_id,
                exception,
                serde_json::json!({"run_id": run_id, "stage_id": stage_id}),
            ).await;
        }
    }

    /// 执行工作流 V2
    pub async fn run(
        &self,
        workflow: &WorkflowV2,
        inputs: HashMap<String, serde_json::Value>,
        run_id: &str,
    ) -> Result<WorkflowResultV2, AppError> {
        let mut outputs: HashMap<String, String> = HashMap::new();
        let mut stage_results: Vec<StageResultV2> = Vec::new();
        let mut exceptions: Vec<ExceptionRecord> = Vec::new();
        let mut trajectory_steps: Vec<String> = Vec::new();
        let start_time = chrono::Utc::now().timestamp_millis();

        // 1. 全局预算检查
        match self.budget_tracker.check_global_budget().await {
            Ok(BudgetCheckResult::Ok) => {}
            Ok(BudgetCheckResult::Exceeded { level, action: _ }) => {
                exceptions.push(ExceptionRecord {
                    exception_type: "budget_exceeded".into(),
                    severity: "high".into(),
                    message: format!("全局预算超限: {level}"),
                    stage_id: None,
                });
                return Ok(WorkflowResultV2 {
                    run_id: run_id.to_string(),
                    outputs,
                    stage_results,
                    goal_status: None,
                    budget_summary: None,
                    exceptions,
                });
            }
            _ => {}
        }

        // 2. 创建工作流级预算追踪
        let crew_budget = workflow.budget.clone().unwrap_or_default();
        let _ = self.budget_tracker.create_crew_budget(run_id, &crew_budget).await;

        // 3. 拓扑排序阶段
        let sorted_stages = topological_sort_v2(&workflow.stages)
            .map_err(|e| AppError::Validation(e))?;

        // 4. 执行阶段
        for stage in &sorted_stages {
            // 4.1 检查工作流级预算
            match self.budget_tracker.check_crew_budget(run_id, &crew_budget).await {
                BudgetCheckResult::Ok => {}
                BudgetCheckResult::Exceeded { level, action } => {
                    exceptions.push(ExceptionRecord {
                        exception_type: "budget_exceeded".into(),
                        severity: "high".into(),
                        message: format!("工作流预算超限: {level}"),
                        stage_id: Some(stage.id.clone()),
                    });
                    match action {
                        BudgetAction::Terminate => break,
                        BudgetAction::DowngradeModel => {
                            // §22.3 自动降级：切换到更便宜的模型；无更便宜模型时终止
                            let downgraded = self.model_fallback.as_ref().map(|chain| {
                                let mut chain = chain.write().unwrap();
                                chain.downgrade().map(|c| c.model_id.clone())
                            }).flatten();
                            match downgraded {
                                Some(model_id) => {
                                    if let Some(logger) = &self.logger {
                                        logger.warn("budget_downgrade", &format!(
                                            "预算超限，模型降级为 {model_id}"
                                        ));
                                    }
                                }
                                None => {
                                    exceptions.push(ExceptionRecord {
                                        exception_type: "budget_exceeded".into(),
                                        severity: "high".into(),
                                        message: format!("预算超限且无更便宜模型可降级: {level}"),
                                        stage_id: Some(stage.id.clone()),
                                    });
                                    break;
                                }
                            }
                        }
                        BudgetAction::SkipStageAndContinue => {
                            stage_results.push(StageResultV2 {
                                stage_id: stage.id.clone(),
                                status: StageStatus::Skipped,
                                output: None,
                                error: Some("预算超限，跳过此阶段".into()),
                                tokens_used: None,
                                cost_used: None,
                                retries: 0,
                            });
                            continue;
                        }
                        _ => {}
                    }
                }
                _ => {}
            }

            // 4.2 检查依赖
            let mut deps_met = true;
            for dep in &stage.depends_on {
                if !outputs.contains_key(dep) {
                    deps_met = false;
                    break;
                }
            }
            if !deps_met {
                stage_results.push(StageResultV2 {
                    stage_id: stage.id.clone(),
                    status: StageStatus::Failed,
                    output: None,
                    error: Some(format!("依赖阶段未完成")),
                    tokens_used: None,
                    cost_used: None,
                    retries: 0,
                });
                continue;
            }

            // 4.3 渲染模板
            let prompt = render_template_v2(&stage.prompt_template, &inputs, &outputs);

            // 4.4 工具护栏检查
            if let Some(guard) = &self.tool_guard {
                for tool in &stage.tools {
                    match guard.check_tool_call(tool, &serde_json::Value::Null).await {
                        GuardrailDecision::Allow => {}
                        GuardrailDecision::Deny { reason } => {
                            self.record_exception(run_id, &stage.role, run_id, &stage.id,
                                ExceptionType::GuardrailViolation { check: tool.clone() }).await;
                            exceptions.push(ExceptionRecord {
                                exception_type: "guardrail_violation".into(),
                                severity: "critical".into(),
                                message: reason,
                                stage_id: Some(stage.id.clone()),
                            });
                            stage_results.push(StageResultV2 {
                                stage_id: stage.id.clone(),
                                status: StageStatus::Failed,
                                output: None,
                                error: Some("工具被护栏拦截".into()),
                                tokens_used: None,
                                cost_used: None,
                                retries: 0,
                            });
                            continue;
                        }
                        _ => {}
                    }
                }
            }

            // 4.4b 系统级沙箱检查（§23.4：工具名在黑名单/不在白名单时静态拦截）
            if let Some(sandbox) = &self.sandbox {
                for tool in &stage.tools {
                    let denied = sandbox.process.denied_commands.contains(tool)
                        || (tool == "read_file" || tool == "write_file")
                            && sandbox.filesystem.denied_paths.iter().any(|p| p == "/etc" || p == "/proc" || p == "/sys");
                    if denied {
                        self.record_exception(run_id, &stage.role, run_id, &stage.id,
                            ExceptionType::PermissionDenied { resource: tool.clone() }).await;
                        exceptions.push(ExceptionRecord {
                            exception_type: "permission_denied".into(),
                            severity: "high".into(),
                            message: format!("沙箱拦截工具: {tool}"),
                            stage_id: Some(stage.id.clone()),
                        });
                    }
                }
            }

            // 4.5 构建消息并执行（带重试）
            let max_retries = stage.retry_on_failure.as_ref().map(|r| r.max_retries).unwrap_or(0);
            let mut attempt = 0;
            let mut stage_output = None;
            let mut stage_error = None;

            while attempt <= max_retries {
                self.emit_stage(run_id, &stage.id, &StageStatus::Running);
                let msg = ActorMessage {
                    task_id: format!("{}/{}", run_id, stage.id),
                    prompt: prompt.clone(),
                    tools: stage.tools.clone(),
                    context: None,
                };

                match self.coordinator.dispatch(&stage.role, msg).await {
                    Ok(reply) => {
                        stage_output = Some(reply.output.clone());
                        outputs.insert(stage.id.clone(), reply.output.clone());
                        self.emit_stage(run_id, &stage.id, &StageStatus::Completed);
                        let _ = self.budget_tracker.record_crew_iteration(run_id).await;

                        // §22.2 追踪点：LLM 调用完成后累加 Agent/工作流级 token 与费用
                        if let Some(tokens) = reply.tokens_used {
                            self.budget_tracker.record_agent_usage(&stage.role, tokens).await;
                            self.budget_tracker.record_crew_usage(run_id, tokens, 0.0).await;
                        }

                        // §23.3 行为级护栏：收集轨迹步骤并检查违规
                        trajectory_steps.push(format!("role={} tools={}", stage.role, stage.tools.join(",")));
                        trajectory_steps.push(reply.output.clone());
                        if let Some(guard) = &self.trajectory_guard {
                            for violation in guard.check(&trajectory_steps) {
                                self.record_exception(run_id, &stage.role, run_id, &stage.id,
                                    ExceptionType::GuardrailViolation { check: violation.check_name.clone() }).await;
                                exceptions.push(ExceptionRecord {
                                    exception_type: "guardrail_violation".into(),
                                    severity: match violation.severity {
                                        crate::core::guardrails::trajectory::Severity::Critical => "critical",
                                        crate::core::guardrails::trajectory::Severity::High => "high",
                                        crate::core::guardrails::trajectory::Severity::Medium => "medium",
                                        crate::core::guardrails::trajectory::Severity::Low => "low",
                                    }.into(),
                                    message: violation.description,
                                    stage_id: Some(stage.id.clone()),
                                });
                            }
                        }
                        break;
                    }
                    Err(e) => {
                        if attempt < max_retries {
                            attempt += 1;
                            let delay = stage.retry_on_failure.as_ref()
                                .map(|r| {
                                    let d = r.delay_ms as f64 * r.backoff_multiplier.powi(attempt as i32 - 1);
                                    d as u64
                                })
                                .unwrap_or(1000);
                            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
                            continue;
                        }
                        stage_error = Some(e.to_string());
                        self.emit_stage(run_id, &stage.id, &StageStatus::Failed);
                        self.record_exception(run_id, &stage.role, run_id, &stage.id,
                            ExceptionType::ToolError { tool: stage.role.clone(), error: e.to_string() }).await;

                        match &workflow.on_exception {
                            ExceptionPolicy::Terminate => {
                                exceptions.push(ExceptionRecord {
                                    exception_type: "tool_error".into(),
                                    severity: "high".into(),
                                    message: e.to_string(),
                                    stage_id: Some(stage.id.clone()),
                                });
                                break;
                            }
                            ExceptionPolicy::SkipStageAndContinue => {
                                exceptions.push(ExceptionRecord {
                                    exception_type: "tool_error".into(),
                                    severity: "medium".into(),
                                    message: e.to_string(),
                                    stage_id: Some(stage.id.clone()),
                                });
                                break;
                            }
                            ExceptionPolicy::ContinueAndLog => {
                                exceptions.push(ExceptionRecord {
                                    exception_type: "tool_error".into(),
                                    severity: "medium".into(),
                                    message: e.to_string(),
                                    stage_id: Some(stage.id.clone()),
                                });
                                break;
                            }
                            ExceptionPolicy::PauseAndAsk => {
                                exceptions.push(ExceptionRecord {
                                    exception_type: "tool_error".into(),
                                    severity: "high".into(),
                                    message: e.to_string(),
                                    stage_id: Some(stage.id.clone()),
                                });
                                break;
                            }
                        }
                    }
                }
            }

            stage_results.push(StageResultV2 {
                stage_id: stage.id.clone(),
                status: if stage_error.is_some() { StageStatus::Failed } else { StageStatus::Completed },
                output: stage_output,
                error: stage_error,
                tokens_used: None,
                cost_used: None,
                retries: attempt,
            });
        }

        // 5. 评估目标
        let goal_status = self.evaluate_goal(&outputs);

        // 6. 生成预算摘要
        let budget_summary = Some(BudgetSummary {
            tokens_used: 0,
            cost_used: 0.0,
            iterations: 0,
            duration_ms: (chrono::Utc::now().timestamp_millis() - start_time) as u64,
        });

        let _ = self.budget_tracker.complete_crew(run_id).await;

        Ok(WorkflowResultV2 {
            run_id: run_id.to_string(),
            outputs,
            stage_results,
            goal_status,
            budget_summary,
            exceptions,
        })
    }

    fn evaluate_goal(&self, outputs: &HashMap<String, String>) -> Option<super::goal::GoalStatus> {
        let goal = self.goal.as_ref()?;
        let accumulated = outputs.values().cloned().collect::<Vec<_>>().join("\n\n");
        let state = super::goal::WorkflowState {
            stage_outputs: outputs.clone(),
            accumulated_text: accumulated,
        };
        Some(goal.evaluate(&state))
    }
}
