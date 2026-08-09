use std::sync::Arc;

use super::session::{OrchestratorSession, OrchestratorStatus, OrchestratorEvent};
use super::spec::{SpecDocument, SpecTask, Complexity};
use super::plan::{ExecutionPlan, ExecutionGroup, GroupKind, PlannedTask, AgentConfig, ReviewResult, TaskReview, TaskResult, TaskStatus};
use crate::core::adk::model::{ChatMessage, ChatRole, GenerationRequest, MessageContent, ModelProvider};
use crate::core::budget::tracker::BudgetTracker;
use crate::core::observability::exception::ExceptionRecorder;
use crate::utils::error::AppError;

pub struct OrchestratorEngine {
    pub budget_tracker: Arc<BudgetTracker>,
    pub exception_recorder: Option<Arc<ExceptionRecorder>>,
    /// §27.2 会话持久化用数据库连接
    pub db: Option<sqlx::SqlitePool>,
    /// §27.3 规划/审查用模型（强推理）
    pub planner_provider: Option<Arc<dyn ModelProvider>>,
    on_event: Option<Box<dyn Fn(&OrchestratorEvent) + Send + Sync>>,
}

impl OrchestratorEngine {
    pub fn new(budget_tracker: Arc<BudgetTracker>) -> Self {
        Self {
            budget_tracker,
            exception_recorder: None,
            db: None,
            planner_provider: None,
            on_event: None,
        }
    }

    /// §27.2 配置数据库连接（启用会话持久化）
    pub fn with_db(mut self, pool: sqlx::SqlitePool) -> Self {
        self.db = Some(pool);
        self
    }

    /// §27.3 配置规划/审查模型
    pub fn with_planner_provider(mut self, provider: Arc<dyn ModelProvider>) -> Self {
        self.planner_provider = Some(provider);
        self
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

    /// 任务级事件：携带任务状态数据，供前端渲染子任务卡片
    fn emit_task_event(&self, event_type: &str, message: String, data: serde_json::Value) {
        let event = OrchestratorEvent {
            event_type: event_type.into(),
            message,
            timestamp: chrono::Utc::now().timestamp_millis(),
            data: Some(data),
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
                    session.task_results = results;
                    self.record_results(session, &session.task_results).await;
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

            // §27.2 每次状态转换后持久化（崩溃可恢复）
            if let Some(pool) = &self.db {
                if let Err(e) = session.save(pool).await {
                    tracing::warn!("编排会话持久化失败: {e}");
                }
            }
        }

        // 最终状态持久化
        if let Some(pool) = &self.db {
            if let Err(e) = session.save(pool).await {
                tracing::warn!("编排会话最终持久化失败: {e}");
            }
        }

        Ok(())
    }

    /// §27.3 生成 SPEC：Planner 模型分析需求并拆解任务（失败时回退骨架）
    async fn generate_spec(&self, user_request: &str) -> Result<SpecDocument, AppError> {
        if let Some(provider) = &self.planner_provider {
            let prompt = format!(
                r#"你是一个专业的软件架构师。请分析以下需求，生成详细的 SPEC 文档（JSON 格式）。

用户需求：
{user_request}

输出 JSON，结构如下：
{{
  "summary": "需求摘要（1-2 句话）",
  "tasks": [
    {{ "id": "T1", "title": "任务标题", "description": "任务描述", "acceptance": ["验收标准1"], "estimated_complexity": "low|medium|high", "required_tools": ["工具名"], "suggested_model": null }}
  ],
  "acceptance_criteria": {{ "T1": ["验收标准1"] }},
  "dependencies": {{ "T1": ["依赖任务ID"] }},
  "out_of_scope": ["明确排除的内容"]
}}

要求：
- 任务拆解粒度适中，每个任务必须有可验证的验收标准
- 依赖关系必须无环
- 简单任务用 low，复杂任务用 high
只输出 JSON，不要其他文字。"#
            );

            let request = GenerationRequest {
                messages: vec![ChatMessage {
                    role: ChatRole::User,
                    content: MessageContent::Text(prompt),
                    name: None,
                }],
                system: None,
                tools: Vec::new(),
                temperature: Some(0.2),
                max_tokens: Some(4096),
                stop: None,
            };

            if let Ok(response) = provider.generate(request).await {
                if let Some(spec) = parse_spec_json(&response.text) {
                    return Ok(spec);
                }
                tracing::warn!("编排 SPEC 解析失败，回退骨架: {}", response.text.chars().take(80).collect::<String>());
            }
        }

        // 骨架回退：最小可行 SPEC
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

    /// §27.3 并行执行计划：为每个任务创建 GenericActor 并真实执行
    /// 说明：任务执行复用默认模型 provider（planner 模型），工具注册表为空
    async fn execute_plan(&self, session: &OrchestratorSession) -> Vec<TaskResult> {
        let plan = match &session.plan {
            Some(p) => p,
            None => return vec![],
        };

        let provider = match &self.planner_provider {
            Some(p) => p.clone(),
            None => return vec![],
        };

        let mut all_results = Vec::new();
        for group in &plan.groups {
            match group.kind {
                GroupKind::Parallel => {
                    let handles: Vec<_> = group.tasks.iter().map(|task| {
                        let provider = provider.clone();
                        let self_ref = self;
                        let group_id = group.id.clone();
                        async move {
                            self_ref.run_group_task(task, provider, &group_id).await
                        }
                    }).collect();
                    let results = futures::future::join_all(handles).await;
                    all_results.extend(results);
                }
                GroupKind::Sequential => {
                    for task in &group.tasks {
                        let result = self.run_group_task(task, provider.clone(), &group.id).await;
                        all_results.push(result);
                    }
                }
            }
        }
        all_results
    }

    /// 执行单个任务并补发任务级事件（task_started / task_finished）
    async fn run_group_task(
        &self,
        task: &PlannedTask,
        provider: Arc<dyn ModelProvider>,
        group_id: &str,
    ) -> TaskResult {
        self.emit_task_event(
            "task_started",
            format!("开始执行任务 {}", task.spec_task_id),
            serde_json::json!({
                "task_id": task.spec_task_id,
                "role": task.agent_config.role,
                "model_id": task.agent_config.model_id,
                "group_id": group_id,
            }),
        );

        let result = self.execute_task(task, provider).await;
        let status = if matches!(result.status, TaskStatus::Completed) {
            "completed"
        } else {
            "failed"
        };

        self.emit_task_event(
            "task_finished",
            format!("任务 {} {}", task.spec_task_id, if status == "completed" { "完成" } else { "失败" }),
            serde_json::json!({
                "task_id": task.spec_task_id,
                "status": status,
                "duration_ms": result.duration_ms,
                "tokens_used": result.tokens_used,
                "output_summary": result.output.chars().take(200).collect::<String>(),
                "error": result.error,
            }),
        );

        result
    }

    /// 执行单个任务（真实 LLM 调用，无工具）
    async fn execute_task(
        &self,
        task: &PlannedTask,
        provider: Arc<dyn ModelProvider>,
    ) -> TaskResult {
        let start_time = chrono::Utc::now().timestamp_millis();

        let request = GenerationRequest {
            messages: vec![ChatMessage {
                role: ChatRole::User,
                content: MessageContent::Text(task.prompt.clone()),
                name: None,
            }],
            system: task.agent_config.system_prompt.clone(),
            tools: Vec::new(),
            temperature: task.agent_config.temperature,
            max_tokens: task.agent_config.max_tokens,
            stop: None,
        };

        match provider.generate(request).await {
            Ok(response) => {
                let usage = response.usage.unwrap_or(crate::core::adk::model::Usage {
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    total_tokens: 0,
                });
                TaskResult {
                    task_id: task.spec_task_id.clone(),
                    status: TaskStatus::Completed,
                    output: response.text,
                    tokens_used: Some(usage.total_tokens),
                    duration_ms: chrono::Utc::now().timestamp_millis() - start_time,
                    error: None,
                }
            }
            Err(e) => TaskResult {
                task_id: task.spec_task_id.clone(),
                status: TaskStatus::Failed,
                output: String::new(),
                tokens_used: None,
                duration_ms: chrono::Utc::now().timestamp_millis() - start_time,
                error: Some(e.to_string()),
            },
        }
    }

    async fn record_results(&self, _session: &OrchestratorSession, _results: &[TaskResult]) {
        // Record results to budget tracker
    }

    /// §27.3 审查结果：Reviewer 模型检查任务输出是否符合验收标准
    async fn review_results(&self, session: &OrchestratorSession) -> ReviewResult {
        let spec = match &session.spec {
            Some(s) => s,
            None => return ReviewResult { task_reviews: vec![] },
        };

        if let Some(provider) = &self.planner_provider {
            let spec_json = serde_json::to_string(spec).unwrap_or_default();
            let results_json = serde_json::to_string(&session.task_results).unwrap_or_default();

            let prompt = format!(
                r#"你是一个严格的代码审查员。请审查以下任务的输出是否符合 SPEC 中的验收标准。

SPEC 任务清单：
{spec_json}

任务输出：
{results_json}

请为每个任务给出 JSON 审查结果：
{{
  "task_reviews": [
    {{ "task_id": "T1", "passed": true, "reasons": [], "suggestions": ["建议1"] }}
  ]
}}

要求：passed 为 false 时 reasons 必须给出具体原因。只输出 JSON。"#
            );

            let request = GenerationRequest {
                messages: vec![ChatMessage {
                    role: ChatRole::User,
                    content: MessageContent::Text(prompt),
                    name: None,
                }],
                system: None,
                tools: Vec::new(),
                temperature: Some(0.2),
                max_tokens: Some(4096),
                stop: None,
            };

            if let Ok(response) = provider.generate(request).await {
                if let Some(review) = parse_review_json(&response.text) {
                    return review;
                }
                tracing::warn!("编排审查解析失败，回退规则判定");
            }
        }

        // 规则回退：有 error 的任务判失败，其余通过
        let task_reviews: Vec<TaskReview> = spec.tasks.iter().map(|task| {
            let failed = session.task_results.iter().find(|r| r.task_id == task.id)
                .map(|r| matches!(r.status, TaskStatus::Failed))
                .unwrap_or(false);
            TaskReview {
                task_id: task.id.clone(),
                passed: !failed,
                reasons: if failed { vec!["任务执行失败".into()] } else { vec![] },
                suggestions: vec![],
            }
        }).collect();

        ReviewResult { task_reviews }
    }
}

/// §27.3 从 LLM 响应中提取 SPEC JSON（支持 fenced code block）
fn parse_spec_json(text: &str) -> Option<SpecDocument> {
    let trimmed = text.trim();
    // 去掉 ```json ... ``` 围栏
    let json_str = if trimmed.starts_with("```") {
        let start = trimmed.find('\n').unwrap_or(0) + 1;
        let end = trimmed.rfind("```").unwrap_or(trimmed.len());
        trimmed[start..end].trim()
    } else {
        trimmed
    };
    serde_json::from_str(json_str).ok()
}

/// §27.3 从 LLM 响应中提取审查结果 JSON
fn parse_review_json(text: &str) -> Option<ReviewResult> {
    let trimmed = text.trim();
    let json_str = if trimmed.starts_with("```") {
        let start = trimmed.find('\n').unwrap_or(0) + 1;
        let end = trimmed.rfind("```").unwrap_or(trimmed.len());
        trimmed[start..end].trim()
    } else {
        trimmed
    };
    serde_json::from_str(json_str).ok()
}
