//! Compose workflow engine — single-agent compose sessions.
//!
//! Replaces the deleted multi-agent orchestrator with a simpler workflow
//! that runs within a single Agent's context.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::Emitter;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::core::adk::model::{
    ChatMessage, ChatRole, GenerationRequest, MessageContent, ModelProvider,
};
use crate::core::rig::provider::OpenAiProvider;
use crate::data::models::ProviderRow;
use crate::utils::error::AppError;

// ── Compose Session Types ────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeSession {
    pub id: String,
    pub user_request: String,
    pub agent_id: String,
    pub status: ComposeStatus,
    pub spec: Option<SpecDocument>,
    pub tasks: Vec<ComposeTask>,
    pub review: Option<ReviewResult>,
    pub summary: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum ComposeStatus {
    Brainstorming,
    Designing,
    Implementing,
    Verifying,
    Reviewing,
    Completed,
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposeTask {
    pub id: String,
    pub description: String,
    pub acceptance: String,
    pub status: TaskStatus,
    pub depends_on: Vec<String>,
    pub result: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecDocument {
    pub summary: String,
    pub tasks: Vec<SpecTask>,
    pub dependencies: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecTask {
    pub id: String,
    pub title: String,
    pub description: String,
    pub acceptance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewResult {
    pub critical: Vec<String>,
    pub important: Vec<String>,
    pub minor: Vec<String>,
    pub ready_to_merge: bool,
}

// ── Compose Workflow Engine ──────────────────────────────

pub struct ComposeEngine {
    pub sessions: Arc<Mutex<HashMap<String, ComposeSession>>>,
    pub cancels: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

impl Default for ComposeEngine {
    fn default() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            cancels: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl ComposeEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a new compose session. Returns the initial session state.
    pub async fn start(
        &self,
        user_request: String,
        agent_id: String,
        db: &sqlx::SqlitePool,
        app: &tauri::AppHandle,
    ) -> Result<ComposeSession, AppError> {
        let now = chrono::Utc::now().timestamp_millis();
        let session_id = uuid::Uuid::new_v4().to_string();

        let session = ComposeSession {
            id: session_id.clone(),
            user_request: user_request.clone(),
            agent_id: agent_id.clone(),
            status: ComposeStatus::Brainstorming,
            spec: None,
            tasks: Vec::new(),
            review: None,
            summary: None,
            created_at: now,
            updated_at: now,
        };

        self.sessions
            .lock()
            .await
            .insert(session_id.clone(), session.clone());

        let cancel = CancellationToken::new();
        self.cancels
            .lock()
            .await
            .insert(session_id.clone(), cancel.clone());

        // Spawn the workflow in background
        let sessions = self.sessions.clone();
        let cancels = self.cancels.clone();
        let app_clone = app.clone();
        let db_clone = db.clone();
        let sid = session_id.clone();
        let aid = agent_id.clone();
        let req = user_request.clone();

        tokio::spawn(async move {
            if let Err(e) =
                run_compose_workflow(&sid, &aid, &req, &sessions, &cancels, &db_clone, &app_clone)
                    .await
            {
                tracing::error!("compose workflow failed: {e}");
                let mut sessions = sessions.lock().await;
                if let Some(s) = sessions.get_mut(&sid) {
                    s.status = ComposeStatus::Failed(e.to_string());
                    s.updated_at = chrono::Utc::now().timestamp_millis();
                }
                let _ = app_clone.emit(
                    "compose:error",
                    serde_json::json!({
                        "session_id": sid,
                        "message": e.to_string(),
                    }),
                );
            }
            // Cleanup cancel token
            cancels.lock().await.remove(&sid);
        });

        Ok(session)
    }

    /// Pause a running compose session.
    pub async fn pause(&self, session_id: &str) -> Result<(), AppError> {
        let mut sessions = self.sessions.lock().await;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| AppError::Internal(format!("Session not found: {session_id}")))?;

        match &session.status {
            ComposeStatus::Brainstorming
            | ComposeStatus::Designing
            | ComposeStatus::Implementing
            | ComposeStatus::Verifying
            | ComposeStatus::Reviewing => {}
            _ => {
                return Err(AppError::Validation(
                    "Cannot pause: session is not in an active stage".to_string(),
                ));
            }
        }

        // Store the status to resume from later
        let paused_status = session.status.clone();
        session.status = ComposeStatus::Failed(format!("Paused at: {paused_status:?}"));
        session.updated_at = chrono::Utc::now().timestamp_millis();

        // Cancel the background task
        if let Some(token) = self.cancels.lock().await.remove(session_id) {
            token.cancel();
        }

        Ok(())
    }

    /// Resume a paused compose session.
    pub async fn resume(
        &self,
        session_id: &str,
        db: &sqlx::SqlitePool,
        app: &tauri::AppHandle,
    ) -> Result<(), AppError> {
        let (user_request, agent_id) = {
            let mut sessions = self.sessions.lock().await;
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| AppError::Internal(format!("Session not found: {session_id}")))?;

            match &session.status {
                ComposeStatus::Failed(msg) if msg.starts_with("Paused at:") => {
                    // Restore status
                    session.status = ComposeStatus::Brainstorming;
                    session.updated_at = chrono::Utc::now().timestamp_millis();
                    (session.user_request.clone(), session.agent_id.clone())
                }
                _ => {
                    return Err(AppError::Validation(
                        "Cannot resume: session is not paused".to_string(),
                    ));
                }
            }
        };

        // Re-spawn the workflow
        let sessions = self.sessions.clone();
        let cancels = self.cancels.clone();
        let app_clone = app.clone();
        let db_clone = db.clone();
        let sid = session_id.to_string();
        let aid = agent_id;
        let req = user_request;
        let cancel = CancellationToken::new();
        self.cancels
            .lock()
            .await
            .insert(session_id.to_string(), cancel);

        tokio::spawn(async move {
            if let Err(e) =
                run_compose_workflow(&sid, &aid, &req, &sessions, &cancels, &db_clone, &app_clone)
                    .await
            {
                tracing::error!("compose workflow failed on resume: {e}");
                let mut sessions = sessions.lock().await;
                if let Some(s) = sessions.get_mut(&sid) {
                    s.status = ComposeStatus::Failed(e.to_string());
                    s.updated_at = chrono::Utc::now().timestamp_millis();
                }
                let _ = app_clone.emit(
                    "compose:error",
                    serde_json::json!({
                        "session_id": sid,
                        "message": e.to_string(),
                    }),
                );
            }
            cancels.lock().await.remove(&sid);
        });

        Ok(())
    }

    /// Stop a compose session permanently.
    pub async fn stop(&self, session_id: &str) -> Result<(), AppError> {
        let mut sessions = self.sessions.lock().await;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| AppError::Internal(format!("Session not found: {session_id}")))?;

        // Cancel background task
        if let Some(token) = self.cancels.lock().await.remove(session_id) {
            token.cancel();
        }

        session.status = ComposeStatus::Failed("Stopped by user".to_string());
        session.updated_at = chrono::Utc::now().timestamp_millis();
        Ok(())
    }

    /// Get a session by ID.
    pub async fn get(&self, session_id: &str) -> Result<ComposeSession, AppError> {
        let sessions = self.sessions.lock().await;
        sessions
            .get(session_id)
            .cloned()
            .ok_or_else(|| AppError::Internal(format!("Session not found: {session_id}")))
    }
}

// ── LLM Helper ──────────────────────────────────────────

async fn create_provider(
    agent_id: &str,
    db: &sqlx::SqlitePool,
) -> Result<Arc<dyn ModelProvider>, AppError> {
    // Get agent config
    let agent_row = sqlx::query_as::<_, crate::data::models::AgentRow>(
        "SELECT id, name, description, avatar, system_prompt, model_id, plan_model_id, small_model_id, temperature, max_tokens, disabled_tools, configuration, order_key, created_at, updated_at FROM agents WHERE id = ?"
    )
    .bind(agent_id)
    .fetch_optional(db)
    .await?
    .ok_or_else(|| AppError::AgentNotFound(agent_id.to_string()))?;

    // Find model
    let model_row = if let Some(ref mid) = agent_row.model_id {
        let found = sqlx::query_as::<_, crate::data::models::ModelRow>(
            "SELECT id, provider_id, model_id, display_name, kind, max_tokens, is_default, created_at FROM models WHERE id = ?"
        )
        .bind(mid)
        .fetch_optional(db)
        .await?;
        if found.is_some() {
            found
        } else {
            sqlx::query_as::<_, crate::data::models::ModelRow>(
                "SELECT id, provider_id, model_id, display_name, kind, max_tokens, is_default, created_at FROM models WHERE model_id = ? LIMIT 1"
            )
            .bind(mid)
            .fetch_optional(db)
            .await?
        }
    } else {
        sqlx::query_as::<_, crate::data::models::ModelRow>(
            "SELECT id, provider_id, model_id, display_name, kind, max_tokens, is_default, created_at FROM models WHERE is_default = 1 LIMIT 1"
        )
        .fetch_optional(db)
        .await?
    };

    let model_row = model_row
        .ok_or_else(|| AppError::LlmProvider("No model configured for compose".to_string()))?;

    let provider_row = sqlx::query_as::<_, ProviderRow>(
        "SELECT id, name, kind, base_url, api_key_enc, is_enabled, created_at, updated_at FROM providers WHERE id = ?"
    )
    .bind(&model_row.provider_id)
    .fetch_optional(db)
    .await?
    .ok_or_else(|| AppError::LlmProvider(format!("Provider not found: {}", model_row.provider_id)))?;

    let base_url = provider_row
        .base_url
        .unwrap_or_else(|| match provider_row.kind.as_str() {
            "ollama" => "http://localhost:11434/v1".to_string(),
            _ => "https://api.openai.com/v1".to_string(),
        });

    let api_key = provider_row
        .api_key_enc
        .as_deref()
        .map(crate::commands::settings::decrypt_provider_key)
        .unwrap_or_default();

    Ok(Arc::new(OpenAiProvider::new(
        model_row.provider_id.clone(),
        model_row
            .display_name
            .clone()
            .unwrap_or_else(|| model_row.model_id.clone()),
        api_key,
        base_url,
        model_row.model_id.clone(),
    )))
}

async fn llm_generate(
    provider: &Arc<dyn ModelProvider>,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String, AppError> {
    let request = GenerationRequest {
        system: Some(system_prompt.to_string()),
        messages: vec![ChatMessage {
            role: ChatRole::User,
            content: MessageContent::Text(user_prompt.to_string()),
            name: None,
        }],
        ..Default::default()
    };

    let response = provider
        .generate(request)
        .await
        .map_err(|e| AppError::LlmProvider(e.to_string()))?;

    Ok(response.text)
}

// ── Compose Workflow Steps ───────────────────────────────

async fn run_compose_workflow(
    session_id: &str,
    agent_id: &str,
    user_request: &str,
    sessions: &Arc<Mutex<HashMap<String, ComposeSession>>>,
    cancels: &Arc<Mutex<HashMap<String, CancellationToken>>>,
    db: &sqlx::SqlitePool,
    app: &tauri::AppHandle,
) -> Result<(), AppError> {
    let provider = create_provider(agent_id, db).await?;
    let cancel = cancels
        .lock()
        .await
        .get(session_id)
        .cloned()
        .unwrap_or_default();

    // Step 1: Brainstorming
    update_status(sessions, session_id, ComposeStatus::Brainstorming, app).await;

    if cancel.is_cancelled() {
        return Err(AppError::Internal("Cancelled".to_string()));
    }

    let brainstorm_prompt = format!(
        r#"Analyze the following user request and provide a brief analysis of the scope, key requirements, and potential challenges. Be concise (2-3 paragraphs).

User Request:
{user_request}"#
    );

    let brainstorm_system = "You are a software architect helping to analyze and break down feature requests. Provide clear, structured analysis.";
    let _brainstorm_result = llm_generate(&provider, brainstorm_system, &brainstorm_prompt).await?;

    // Step 2: Design - Generate SPEC
    update_status(sessions, session_id, ComposeStatus::Designing, app).await;

    if cancel.is_cancelled() {
        return Err(AppError::Internal("Cancelled".to_string()));
    }

    let design_prompt = format!(
        r#"Based on the user request below, create a specification document with tasks and dependencies.

User Request:
{user_request}

Return a JSON object with this exact structure:
{{
  "summary": "Brief summary of what will be built",
  "tasks": [
    {{
      "id": "task-1",
      "title": "Task title",
      "description": "Detailed description of what to do",
      "acceptance": "Acceptance criteria for this task"
    }}
  ],
  "dependencies": {{
    "task-2": ["task-1"],
    "task-3": ["task-1"]
  }}
}}

Keep tasks to 3-8 items. Make tasks actionable and specific."#
    );

    let design_system = "You are a software architect. Generate a JSON specification document. Output ONLY valid JSON, no markdown fencing.";

    let spec_text = llm_generate(&provider, design_system, &design_prompt).await?;

    // Parse the spec
    let spec: SpecDocument = serde_json::from_str(&spec_text).map_err(|e| {
        AppError::Internal(format!("Failed to parse spec JSON: {e}\nRaw: {spec_text}"))
    })?;

    // Convert spec tasks to compose tasks
    let mut tasks: Vec<ComposeTask> = spec
        .tasks
        .iter()
        .map(|t| ComposeTask {
            id: t.id.clone(),
            description: t.description.clone(),
            acceptance: t.acceptance.clone(),
            status: TaskStatus::Pending,
            depends_on: spec.dependencies.get(&t.id).cloned().unwrap_or_default(),
            result: None,
            error: None,
        })
        .collect();

    // Update session with spec
    {
        let mut sessions = sessions.lock().await;
        if let Some(s) = sessions.get_mut(session_id) {
            s.spec = Some(spec.clone());
            s.tasks = tasks.clone();
            s.updated_at = chrono::Utc::now().timestamp_millis();
        }
    }

    let _ = app.emit(
        "compose:spec",
        serde_json::json!({
            "session_id": session_id,
            "spec": spec,
        }),
    );

    // Step 3: Implement - Execute tasks
    update_status(sessions, session_id, ComposeStatus::Implementing, app).await;

    let total_tasks = tasks.len();
    for i in 0..total_tasks {
        if cancel.is_cancelled() {
            return Err(AppError::Internal("Cancelled".to_string()));
        }

        // Get task deps for dependency check
        let task_deps = tasks[i].depends_on.clone();

        // Check if dependencies are met
        let deps_met = task_deps.iter().all(|dep| {
            tasks
                .iter()
                .find(|t| t.id == *dep)
                .map(|t| t.status == TaskStatus::Completed)
                .unwrap_or(false)
        });

        if !deps_met {
            tasks[i].status = TaskStatus::Failed;
            tasks[i].error = Some("Dependencies not met".to_string());
            continue;
        }

        tasks[i].status = TaskStatus::Running;
        emit_task_event(app, session_id, &tasks[i].id, &tasks[i].status, None);

        let impl_prompt = format!(
            r#"Execute the following task as part of a larger feature implementation.

Task: {title}
Description: {description}
Acceptance Criteria: {acceptance}

Provide a detailed implementation plan or code changes needed. Be specific about files to create/modify, functions to implement, and any configuration changes."#,
            title = tasks[i].id,
            description = tasks[i].description,
            acceptance = tasks[i].acceptance
        );

        match llm_generate(
            &provider,
            "You are a senior software engineer. Implement tasks precisely.",
            &impl_prompt,
        )
        .await
        {
            Ok(result) => {
                tasks[i].status = TaskStatus::Completed;
                tasks[i].result = Some(result.clone());
                emit_task_event(
                    app,
                    session_id,
                    &tasks[i].id,
                    &tasks[i].status,
                    Some(&result),
                );
            }
            Err(e) => {
                tasks[i].status = TaskStatus::Failed;
                tasks[i].error = Some(e.to_string());
                emit_task_event(app, session_id, &tasks[i].id, &tasks[i].status, None);
            }
        }

        // Update session tasks
        let mut sessions = sessions.lock().await;
        if let Some(s) = sessions.get_mut(session_id) {
            s.tasks = tasks.clone();
            s.updated_at = chrono::Utc::now().timestamp_millis();
        }

        let _ = app.emit(
            "compose:progress",
            serde_json::json!({
                "session_id": session_id,
                "current": i + 1,
                "total": total_tasks,
            }),
        );
    }

    // Step 4: Verify
    update_status(sessions, session_id, ComposeStatus::Verifying, app).await;

    if cancel.is_cancelled() {
        return Err(AppError::Internal("Cancelled".to_string()));
    }

    let verify_prompt = format!(
        r#"Review the completed tasks and verify they meet the acceptance criteria.

Completed tasks:
{tasks_summary}

Provide a verification report with:
1. What was verified
2. Any issues found
3. Suggested verification commands the user should run"#,
        tasks_summary = tasks
            .iter()
            .map(|t| format!(
                "- {} ({}): {}",
                t.id,
                if t.status == TaskStatus::Completed {
                    "DONE"
                } else {
                    "FAILED"
                },
                t.result.as_deref().unwrap_or("No result")
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );

    let verify_result = llm_generate(
        &provider,
        "You are a QA engineer reviewing completed work.",
        &verify_prompt,
    )
    .await?;

    // Step 5: Review
    update_status(sessions, session_id, ComposeStatus::Reviewing, app).await;

    if cancel.is_cancelled() {
        return Err(AppError::Internal("Cancelled".to_string()));
    }

    let review_prompt = format!(
        r#"Review the implementation results and categorize findings:

Verification Report:
{verify_result}

Original Request:
{user_request}

Return a JSON object with this exact structure:
{{
  "critical": ["Critical issues that must be fixed"],
  "important": ["Important improvements"],
  "minor": ["Minor suggestions"],
  "ready_to_merge": true or false
}}"#
    );

    let review_text = llm_generate(
        &provider,
        "You are a code reviewer. Output ONLY valid JSON, no markdown fencing.",
        &review_prompt,
    )
    .await?;

    let review: ReviewResult = serde_json::from_str(&review_text).map_err(|e| {
        AppError::Internal(format!(
            "Failed to parse review JSON: {e}\nRaw: {review_text}"
        ))
    })?;

    {
        let mut sessions = sessions.lock().await;
        if let Some(s) = sessions.get_mut(session_id) {
            s.review = Some(review.clone());
            s.updated_at = chrono::Utc::now().timestamp_millis();
        }
    }

    let _ = app.emit(
        "compose:review",
        serde_json::json!({
            "session_id": session_id,
            "review": review,
        }),
    );

    // Step 6: Summary
    let summary_prompt = format!(
        r#"Generate a brief summary of the compose workflow results.

User Request: {user_request}
Tasks Completed: {completed}/{total}
Ready to Merge: {ready}

Provide a 2-3 sentence summary of what was accomplished and any next steps."#,
        completed = tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Completed)
            .count(),
        total = tasks.len(),
        ready = review.ready_to_merge
    );

    let summary = llm_generate(
        &provider,
        "You are a project manager summarizing completed work.",
        &summary_prompt,
    )
    .await?;

    // Final status
    {
        let mut sessions = sessions.lock().await;
        if let Some(s) = sessions.get_mut(session_id) {
            s.status = ComposeStatus::Completed;
            s.summary = Some(summary);
            s.updated_at = chrono::Utc::now().timestamp_millis();
        }
    }

    let _ = app.emit(
        "compose:done",
        serde_json::json!({
            "session_id": session_id,
            "status": "completed",
        }),
    );

    tracing::info!("compose workflow completed for session {session_id}");
    Ok(())
}

// ── Helper Functions ─────────────────────────────────────

async fn update_status(
    sessions: &Arc<Mutex<HashMap<String, ComposeSession>>>,
    session_id: &str,
    status: ComposeStatus,
    app: &tauri::AppHandle,
) {
    let mut sessions = sessions.lock().await;
    if let Some(s) = sessions.get_mut(session_id) {
        s.status = status.clone();
        s.updated_at = chrono::Utc::now().timestamp_millis();
    }

    let _ = app.emit(
        "compose:stage",
        serde_json::json!({
            "session_id": session_id,
            "stage": status,
        }),
    );
}

fn emit_task_event(
    app: &tauri::AppHandle,
    session_id: &str,
    task_id: &str,
    status: &TaskStatus,
    result: Option<&str>,
) {
    let mut payload = serde_json::json!({
        "session_id": session_id,
        "task_id": task_id,
        "status": status,
    });

    if let Some(r) = result {
        payload["result"] = serde_json::json!(r);
    }

    let _ = app.emit("compose:task", payload);
}
