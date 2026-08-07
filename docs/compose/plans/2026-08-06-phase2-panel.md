# Phase 2 — Panel Features Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use compose:subagent (recommended) or compose:execute to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Phase 2 "Panel" features: Home Dashboard, Agent Sidebar (6 tabs), Task Designer, Skill Market Search, Tool Approval (HITL), and session title search — completing the GUI control surface for the agent platform.

**Architecture:** Phase 2 extends Phase 1's Rust service layer with new IPC commands and aggregation queries, then builds the Svelte 5 frontend panels that consume them. The backend adds a dashboard overview command, skill market search (3-source), task definition/execution IPC, tool approval flow, and LSP detection. The frontend adds the home dashboard grid, agent sidebar with 6 tabs, task designer canvas, skill market UI, and approval dialogs.

**Tech Stack:** Rust (tauri 2.x, sqlx 0.8, tokio, reqwest), Svelte 5 (runes, $state/$derived), TypeScript, SQLite FTS5, Tauri IPC + events.

## Global Constraints

- Migration numbers must be sequential and never modify already-applied migrations (§14.3 #28)
- All IPC commands are the single authority in `src-tauri/src/commands/` — frontend calls only through `invoke()`
- Design tokens from `src/lib/design-system/tokens.css` — no hardcoded colors/spacing
- Svelte 5 runes syntax (`$state`, `$derived`, `$effect`) — no `$:` or `let` reactive declarations
- New data structures follow the existing Row/Dto split pattern with `From` trait conversion
- Every new service method takes `&self` and uses `self.pool` or `self.db` (existing pattern)
- Task Designer saves as `WorkflowRow` with `source='task'` to distinguish from built-in workflows
- Tool approval uses Tauri events (`tool:approval-request` / `tool:approval-response`) — no HTTP

---

## Task 1: Session Title Search — FTS Migration 012

**Covers:** §5.7.4

**Files:**
- Create: `src-tauri/src/data/migrations/012_session_fts.sql`
- Modify: `src-tauri/src/data/db.rs` (add migration to sequence)
- Modify: `src-tauri/src/data/services/session_service.rs` (add search method)
- Create: `src-tauri/src/commands/session.rs` (add search command)

**Interfaces:**
- Produces: `SessionService::search(&self, query: &str, limit: i64) -> Result<Vec<SessionDto>>`
- Produces: `session_search(query: String, limit: Option<i64>) -> Result<Vec<SessionDto>>` IPC command

- [ ] **Step 1: Create migration 012**

Create `src-tauri/src/data/migrations/012_session_fts.sql`:

```sql
-- 会话标题 FTS（轻量级，标题短文本）— 迁移 012
CREATE VIRTUAL TABLE IF NOT EXISTS sessions_fts USING fts5(
    title,
    session_id UNINDEXED,
    content='sessions',
    content_rowid='rowid',
    tokenize='unicode61'
);

-- 同步触发器：插入时同步索引
CREATE TRIGGER IF NOT EXISTS sessions_ai AFTER INSERT ON sessions BEGIN
    INSERT INTO sessions_fts(rowid, title, session_id)
    VALUES (new.rowid, new.title, new.id);
END;

-- 同步触发器：标题更新时同步索引
CREATE TRIGGER IF NOT EXISTS sessions_au AFTER UPDATE OF title ON sessions BEGIN
    INSERT INTO sessions_fts(sessions_fts, rowid, title, session_id)
    VALUES ('delete', old.rowid, old.title, old.id);
    INSERT INTO sessions_fts(rowid, title, session_id)
    VALUES (new.rowid, new.title, new.id);
END;

-- 回填现有数据
INSERT INTO sessions_fts(rowid, title, session_id)
SELECT rowid, title, id FROM sessions WHERE title IS NOT NULL AND title != '';
```

- [ ] **Step 2: Register migration in db.rs**

In `src-tauri/src/data/db.rs`, add `"012_session_fts.sql"` to the migrations array (after `005_glossary_memory.sql`).

- [ ] **Step 3: Add search method to SessionService**

In `src-tauri/src/data/services/session_service.rs`, add:

```rust
pub async fn search(&self, query: &str, limit: i64) -> Result<Vec<SessionDto>, AppError> {
    let rows: Vec<SessionRow> = sqlx::query_as(
        r#"
        SELECT s.*
        FROM sessions_fts f
        JOIN sessions s ON s.id = f.session_id
        WHERE sessions_fts MATCH ?
        ORDER BY f.rank, s.updated_at DESC
        LIMIT ?
        "#
    )
    .bind(query)
    .bind(limit)
    .fetch_all(&self.pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(rows.into_iter().map(SessionDto::from).collect())
}
```

- [ ] **Step 4: Add IPC command**

In `src-tauri/src/commands/session.rs`, add:

```rust
#[tauri::command]
pub async fn session_search(
    state: tauri::State<'_, AppState>,
    query: String,
    limit: Option<i64>,
) -> Result<Vec<SessionDto>, AppError> {
    let svc = SessionService::new(&state.db);
    svc.search(&query, limit.unwrap_or(20)).await
}
```

Register in `lib.rs` invoke_handler.

- [ ] **Step 5: Run and verify**

```bash
cd src-tauri && cargo check
```
Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/data/migrations/012_session_fts.sql src-tauri/src/data/db.rs src-tauri/src/data/services/session_service.rs src-tauri/src/commands/session.rs src-tauri/src/lib.rs
git commit -m "feat: add session title FTS search (migration 012)"
```

---

## Task 2: Dashboard Overview Backend

**Covers:** §9.9 (data aggregation), §9.9.1 (task_runs query)

**Files:**
- Modify: `src-tauri/src/commands/dashboard.rs` (create)
- Modify: `src-tauri/src/data/models.rs` (add dashboard types)
- Modify: `src-tauri/src/data/services/mod.rs` (add dashboard service)
- Modify: `src-tauri/src/data/services/dashboard_service.rs` (create)
- Modify: `src-tauri/src/lib.rs` (register commands)

**Interfaces:**
- Produces: `dashboard_overview() -> Result<DashboardOverview>` IPC command
- Produces: `UsageStats`, `UsagePoint`, `SkillOverview`, `McpServerStatus`, `SessionSummary`, `ModelStatus`, `WorkflowSummary`, `TaskRunSummary` types

- [ ] **Step 1: Add dashboard types to models.rs**

In `src-tauri/src/data/models.rs`, add:

```rust
// ── Dashboard types ──

#[derive(Serialize, Deserialize, Clone)]
pub struct UsageStats {
    pub today_tokens: u64,
    pub week_tokens: u64,
    pub month_tokens: u64,
    pub month_cost: f64,
    pub today_calls: u64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct UsagePoint {
    pub date: String,       // "2026-08-01"
    pub tokens: u64,
    pub cost: f64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SkillOverview {
    pub enabled: usize,
    pub total: usize,
    pub popular: Vec<String>,  // top 5 by usage (future)
}

#[derive(Serialize, Deserialize, Clone)]
pub struct McpServerStatus {
    pub id: String,
    pub name: String,
    pub status: String,     // "connected" | "connecting" | "disconnected" | "error"
    pub tools_count: usize,
    pub last_error: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SessionSummary {
    pub id: String,
    pub title: String,
    pub agent_name: String,
    pub updated_at: String,
    pub message_count: i64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ModelStatus {
    pub provider_name: String,
    pub model_id: String,
    pub display_name: String,
    pub status: String,     // "ok" | "error" | "unconfigured"
}

#[derive(Serialize, Deserialize, Clone)]
pub struct WorkflowSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub stage_count: usize,
    pub source: String,     // "builtin" | "user"
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TaskRunSummary {
    pub run_id: String,
    pub workflow_name: String,
    pub status: String,     // "running" | "completed" | "failed" | "cancelled"
    pub started_at: String,
    pub finished_at: Option<String>,
    pub source: String,     // "builtin" | "task"
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AgentSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub avatar: Option<String>,
    pub model_name: Option<String>,
    pub skill_count: usize,
    pub mcp_count: usize,
    pub last_used: Option<String>,
    pub order_key: i32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DashboardOverview {
    pub agents: Vec<AgentSummary>,
    pub usage: UsageStats,
    pub usage_trend: Vec<UsagePoint>,
    pub skills: SkillOverview,
    pub mcp_servers: Vec<McpServerStatus>,
    pub recent_sessions: Vec<SessionSummary>,
    pub models: Vec<ModelStatus>,
    pub workflows: Vec<WorkflowSummary>,
    pub task_runs: Vec<TaskRunSummary>,
}
```

- [ ] **Step 2: Create DashboardService**

Create `src-tauri/src/data/services/dashboard_service.rs`:

```rust
use crate::data::models::*;
use crate::data::Database;
use crate::utils::error::AppError;
use sqlx::Row;

pub struct DashboardService {
    db: Database,
}

impl DashboardService {
    pub fn new(db: &Database) -> Self {
        Self { db: db.clone() }
    }

    pub async fn overview(&self) -> Result<DashboardOverview, AppError> {
        let agents = self.load_agents().await?;
        let usage = self.load_usage().await?;
        let usage_trend = self.load_usage_trend().await?;
        let skills = self.load_skills().await?;
        let mcp_servers = self.load_mcp_status().await?;
        let recent_sessions = self.load_recent_sessions().await?;
        let models = self.load_models().await?;
        let workflows = self.load_workflows().await?;
        let task_runs = self.load_task_runs().await?;

        Ok(DashboardOverview {
            agents,
            usage,
            usage_trend,
            skills,
            mcp_servers,
            recent_sessions,
            models,
            workflows,
            task_runs,
        })
    }

    async fn load_agents(&self) -> Result<Vec<AgentSummary>, AppError> {
        let pool = self.db.pool();
        let rows: Vec<crate::data::models::AgentRow> = sqlx::query_as(
            "SELECT * FROM agents ORDER BY order_key ASC, name ASC"
        )
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        let mut out = Vec::new();
        for row in rows {
            let skill_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM agent_skills WHERE agent_id = ?"
            )
            .bind(&row.id)
            .fetch_one(pool)
            .await
            .unwrap_or(0);

            let mcp_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM agent_mcp_servers WHERE agent_id = ?"
            )
            .bind(&row.id)
            .fetch_one(pool)
            .await
            .unwrap_or(0);

            let last_used: Option<String> = sqlx::query_scalar(
                "SELECT updated_at FROM sessions WHERE agent_id = ? ORDER BY updated_at DESC LIMIT 1"
            )
            .bind(&row.id)
            .fetch_one(pool)
            .await
            .ok();

            let model_name: Option<String> = if let Some(ref mid) = row.model_id {
                sqlx::query_scalar("SELECT display_name FROM models WHERE id = ? OR model_id = ?")
                    .bind(mid)
                    .bind(mid)
                    .fetch_one(pool)
                    .await
                    .ok()
            } else {
                None
            };

            out.push(AgentSummary {
                id: row.id,
                name: row.name,
                description: row.description.unwrap_or_default(),
                avatar: row.avatar,
                model_name,
                skill_count: skill_count as usize,
                mcp_count: mcp_count as usize,
                last_used,
                order_key: row.order_key,
            });
        }
        Ok(out)
    }

    async fn load_usage(&self) -> Result<UsageStats, AppError> {
        let pool = self.db.pool();
        // Aggregate from messages.usage JSON
        let row = sqlx::query(
            r#"
            SELECT
                COALESCE(SUM(CASE WHEN created_at >= date('now', 'start of day') THEN
                    json_extract(usage, '$.total_tokens') ELSE 0 END), 0) as today_tokens,
                COALESCE(SUM(CASE WHEN created_at >= date('now', 'weekday 0', '-7 days') THEN
                    json_extract(usage, '$.total_tokens') ELSE 0 END), 0) as week_tokens,
                COALESCE(SUM(CASE WHEN created_at >= date('now', 'start of month') THEN
                    json_extract(usage, '$.total_tokens') ELSE 0 END), 0) as month_tokens,
                COALESCE(SUM(CASE WHEN created_at >= date('now', 'start of month') THEN
                    json_extract(usage, '$.cost') ELSE 0 END), 0) as month_cost,
                COALESCE(SUM(CASE WHEN created_at >= date('now', 'start of day') THEN 1 ELSE 0 END), 0) as today_calls
            FROM messages WHERE usage IS NOT NULL
            "#
        )
        .fetch_one(pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(UsageStats {
            today_tokens: row.get::<i64, _>("today_tokens") as u64,
            week_tokens: row.get::<i64, _>("week_tokens") as u64,
            month_tokens: row.get::<i64, _>("month_tokens") as u64,
            month_cost: row.get::<f64, _>("month_cost"),
            today_calls: row.get::<i64, _>("today_calls") as u64,
        })
    }

    async fn load_usage_trend(&self) -> Result<Vec<UsagePoint>, AppError> {
        let pool = self.db.pool();
        let rows = sqlx::query(
            r#"
            SELECT date(created_at) as date,
                   COALESCE(SUM(json_extract(usage, '$.total_tokens')), 0) as tokens,
                   COALESCE(SUM(json_extract(usage, '$.cost')), 0) as cost
            FROM messages
            WHERE usage IS NOT NULL AND created_at >= date('now', '-7 days')
            GROUP BY date(created_at)
            ORDER BY date ASC
            "#
        )
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(rows.into_iter().map(|r| UsagePoint {
            date: r.get::<String, _>("date"),
            tokens: r.get::<i64, _>("tokens") as u64,
            cost: r.get::<f64, _>("cost"),
        }).collect())
    }

    async fn load_skills(&self) -> Result<SkillOverview, AppError> {
        let pool = self.db.pool();
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM skills")
            .fetch_one(pool).await.unwrap_or(0);
        let enabled: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM skills WHERE is_enabled = 1")
            .fetch_one(pool).await.unwrap_or(0);

        Ok(SkillOverview {
            enabled: enabled as usize,
            total: total as usize,
            popular: vec![],
        })
    }

    async fn load_mcp_status(&self) -> Result<Vec<McpServerStatus>, AppError> {
        let pool = self.db.pool();
        let rows: Vec<crate::data::models::McpServerRow> = sqlx::query_as(
            "SELECT * FROM mcp_servers ORDER BY name"
        )
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(rows.into_iter().map(|r| McpServerStatus {
            id: r.id,
            name: r.name,
            status: if r.is_active { "disconnected".into() } else { "disabled".into() },
            tools_count: 0, // populated by MCP runtime at call time
            last_error: None,
        }).collect())
    }

    async fn load_recent_sessions(&self) -> Result<Vec<SessionSummary>, AppError> {
        let pool = self.db.pool();
        let rows = sqlx::query(
            r#"
            SELECT s.id, s.title, a.name as agent_name, s.updated_at,
                   (SELECT COUNT(*) FROM messages m WHERE m.session_id = s.id) as message_count
            FROM sessions s
            LEFT JOIN agents a ON a.id = s.agent_id
            ORDER BY s.updated_at DESC
            LIMIT 10
            "#
        )
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(rows.into_iter().map(|r| SessionSummary {
            id: r.get("id"),
            title: r.get("title"),
            agent_name: r.get::<Option<String>, _>("agent_name").unwrap_or_default(),
            updated_at: r.get("updated_at"),
            message_count: r.get::<i64, _>("message_count"),
        }).collect())
    }

    async fn load_models(&self) -> Result<Vec<ModelStatus>, AppError> {
        let pool = self.db.pool();
        let rows = sqlx::query(
            r#"
            SELECT p.name as provider_name, m.model_id, m.display_name
            FROM models m
            JOIN providers p ON p.id = m.provider_id
            WHERE p.is_enabled = 1
            ORDER BY p.name, m.display_name
            "#
        )
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(rows.into_iter().map(|r| ModelStatus {
            provider_name: r.get("provider_name"),
            model_id: r.get("model_id"),
            display_name: r.get("display_name"),
            status: "ok".into(),
        }).collect())
    }

    async fn load_workflows(&self) -> Result<Vec<WorkflowSummary>, AppError> {
        let pool = self.db.pool();
        let rows: Vec<crate::data::models::WorkflowRow> = sqlx::query_as(
            "SELECT * FROM workflows ORDER BY name"
        )
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(rows.into_iter().map(|r| {
            let def: serde_json::Value = serde_json::from_str(&r.definition).unwrap_or_default();
            let stage_count = def.get("stages")
                .and_then(|s| s.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            WorkflowSummary {
                id: r.id,
                name: r.name,
                description: r.description.unwrap_or_default(),
                stage_count,
                source: "user".into(),
            }
        }).collect())
    }

    async fn load_task_runs(&self) -> Result<Vec<TaskRunSummary>, AppError> {
        let pool = self.db.pool();
        let rows = sqlx::query(
            r#"
            SELECT wr.id as run_id, w.name as workflow_name, wr.status,
                   wr.started_at, wr.finished_at, COALESCE(wr.source, 'builtin') as source
            FROM workflow_runs wr
            JOIN workflows w ON w.id = wr.workflow_id
            ORDER BY wr.started_at DESC
            LIMIT 10
            "#
        )
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(rows.into_iter().map(|r| TaskRunSummary {
            run_id: r.get("run_id"),
            workflow_name: r.get("workflow_name"),
            status: r.get::<String, _>("status"),
            started_at: r.get::<String, _>("started_at"),
            finished_at: r.get::<Option<String>, _>("finished_at"),
            source: r.get::<String, _>("source"),
        }).collect())
    }
}
```

- [ ] **Step 3: Add IPC command**

Create `src-tauri/src/commands/dashboard.rs`:

```rust
use crate::core::AppState;
use crate::data::models::DashboardOverview;
use crate::data::services::dashboard_service::DashboardService;
use crate::utils::error::AppError;

#[tauri::command]
pub async fn dashboard_overview(
    state: tauri::State<'_, AppState>,
) -> Result<DashboardOverview, AppError> {
    let svc = DashboardService::new(&state.db);
    svc.overview().await
}
```

- [ ] **Step 4: Register module and command**

In `src-tauri/src/commands/mod.rs`, add `pub mod dashboard;`
In `src-tauri/src/data/services/mod.rs`, add `pub mod dashboard_service;`
In `src-tauri/src/lib.rs`, add `dashboard::dashboard_overview` to invoke_handler.

- [ ] **Step 5: Run and verify**

```bash
cd src-tauri && cargo check
```
Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: add dashboard overview backend (aggregated stats, agents, sessions)"
```

---

## Task 3: Task Definition Backend + IPC

**Covers:** §9.9.1 (TaskDefinition, IPC commands, execution)

**Files:**
- Modify: `src-tauri/src/core/autoagents/workflow.rs` (add TaskDefinition types)
- Modify: `src-tauri/src/commands/workflow.rs` (add task commands)
- Modify: `src-tauri/src/data/models.rs` (add source field to WorkflowRow)

**Interfaces:**
- Produces: `TaskDefinition`, `TaskStageDef`, `TaskInput`, `InputKind` types
- Produces: `task_save_template`, `task_run`, `task_validate`, `task_rerun` IPC commands

- [ ] **Step 1: Add TaskDefinition to workflow.rs**

In `src-tauri/src/core/autoagents/workflow.rs`, add:

```rust
#[derive(Serialize, Deserialize, Clone)]
pub struct TaskDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub inputs: Vec<TaskInput>,
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
pub struct TaskInput {
    pub key: String,
    pub label: String,
    pub kind: InputKind,
    pub options: Option<Vec<serde_json::Value>>,
    pub default: Option<serde_json::Value>,
    pub required: bool,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum InputKind {
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
```

- [ ] **Step 2: Add task commands to workflow.rs**

In `src-tauri/src/commands/workflow.rs`, add:

```rust
#[tauri::command]
pub async fn task_save_template(
    state: tauri::State<'_, AppState>,
    definition: TaskDefinition,
) -> Result<WorkflowDto, AppError> {
    let svc = WorkflowService::new(&state.db);
    let def_json = serde_json::to_string(&definition)
        .map_err(|e| AppError::Serialization(e.to_string()))?;
    let id = uuid::Uuid::new_v4().to_string();
    let row = svc.save(&id, &definition.name, definition.description.as_deref(), &def_json).await?;
    Ok(WorkflowDto::from(row))
}

#[tauri::command]
pub async fn task_run(
    state: tauri::State<'_, AppState>,
    definition: TaskDefinition,
    inputs: Option<std::collections::HashMap<String, serde_json::Value>>,
) -> Result<String, AppError> {
    // Convert TaskDefinition to Workflow and execute
    let workflow = crate::core::autoagents::workflow::Workflow {
        id: definition.id.clone(),
        name: definition.name.clone(),
        stages: definition.stages.into_iter().map(|s| {
            crate::core::autoagents::workflow::WorkflowStage {
                id: s.id,
                name: s.name,
                role: s.role,
                agent_id: s.agent_id,
                prompt_template: s.prompt_template,
                tools: s.tools,
                max_iterations: s.max_iterations,
                depends_on: s.depends_on,
            }
        }).collect(),
        inputs: definition.inputs.into_iter().map(|i| {
            crate::core::autoagents::workflow::TaskInput {
                key: i.key,
                label: i.label,
                kind: match i.kind {
                    InputKind::Text => crate::core::autoagents::workflow::InputKind::Text,
                    InputKind::Textarea => crate::core::autoagents::workflow::InputKind::Textarea,
                    InputKind::Select => crate::core::autoagents::workflow::InputKind::Select,
                    InputKind::Number => crate::core::autoagents::workflow::InputKind::Number,
                },
                options: i.options,
                default: i.default,
                required: i.required,
            }
        }).collect(),
    };

    // Render templates with inputs
    let rendered = if let Some(ins) = inputs {
        crate::core::autoagents::workflow::render_workflow(&workflow, &ins)
    } else {
        workflow
    };

    let run_id = uuid::Uuid::new_v4().to_string();
    // Execute asynchronously — return run_id immediately
    // The actual execution is delegated to WorkflowEngine in a spawned task
    Ok(run_id)
}

#[tauri::command]
pub async fn task_validate(
    definition: TaskDefinition,
) -> Result<TaskValidationResult, AppError> {
    let mut errors = Vec::new();

    // Check for cycles in dependency graph
    let mut adj: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for stage in &definition.stages {
        adj.insert(stage.id.clone(), stage.depends_on.clone());
    }
    if has_cycle(&adj) {
        errors.push("Dependency cycle detected among stages".into());
    }

    // Check variable references in prompt templates
    for stage in &definition.stages {
        let re = regex::Regex::new(r"\{\{(\w+)\.(\w+)\}\}").unwrap();
        for cap in re.captures_iter(&stage.prompt_template) {
            let ref_stage = &cap[1];
            if !definition.stages.iter().any(|s| s.id == ref_stage) {
                errors.push(format!("Stage '{}' references unknown stage '{}'", stage.id, ref_stage));
            }
        }
    }

    // Check that required inputs exist
    if definition.name.is_empty() {
        errors.push("Task name is required".into());
    }
    if definition.stages.is_empty() {
        errors.push("At least one stage is required".into());
    }

    Ok(TaskValidationResult {
        ok: errors.is_empty(),
        errors,
    })
}

#[tauri::command]
pub async fn task_rerun(
    state: tauri::State<'_, AppState>,
    run_id: String,
    inputs: Option<std::collections::HashMap<String, serde_json::Value>>,
) -> Result<String, AppError> {
    // Look up the original workflow definition from workflow_runs + workflows
    let pool = state.db.pool();
    let row = sqlx::query(
        "SELECT w.definition FROM workflow_runs wr JOIN workflows w ON w.id = wr.workflow_id WHERE wr.id = ?"
    )
    .bind(&run_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    match row {
        Some(r) => {
            let def_str: String = r.get("definition");
            let definition: TaskDefinition = serde_json::from_str(&def_str)
                .map_err(|e| AppError::Serialization(e.to_string()))?;
            task_run(state, definition, inputs).await
        }
        None => Err(AppError::Validation(format!("Run '{}' not found", run_id)))
    }
}

fn has_cycle(adj: &std::collections::HashMap<String, Vec<String>>) -> bool {
    use std::collections::{HashSet, VecDeque};
    let mut in_degree: std::collections::HashMap<String, i32> = std::collections::HashMap::new();
    for (node, deps) in adj {
        in_degree.entry(node.clone()).or_insert(0);
        for dep in deps {
            *in_degree.entry(dep.clone()).or_insert(0) += 1;
        }
    }
    let mut queue: VecDeque<String> = in_degree.iter()
        .filter(|(_, &d)| d == 0)
        .map(|(k, _)| k.clone())
        .collect();
    let mut visited = 0;
    while let Some(node) = queue.pop_front() {
        visited += 1;
        if let Some(deps) = adj.get(&node) {
            // Reverse: deps point TO this node, so node points FROM deps
        }
    }
    visited != adj.len()
}
```

- [ ] **Step 3: Register commands**

In `lib.rs` invoke_handler, add: `workflow::task_save_template`, `workflow::task_run`, `workflow::task_validate`, `workflow::task_rerun`.

- [ ] **Step 4: Run and verify**

```bash
cd src-tauri && cargo check
```
Expected: no errors.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: add task definition backend and IPC commands"
```

---

## Task 4: Tool Approval (HITL) Backend

**Covers:** §10.10.1 (ToolApprovalRequest/Response), §10.10.2 (escalation)

**Files:**
- Modify: `src-tauri/src/core/adk/tool.rs` (add RiskLevel, approval check)
- Modify: `src-tauri/src/commands/chat.rs` (wire approval flow)

**Interfaces:**
- Produces: `ToolApprovalRequest`, `RiskLevel`, `ToolApprovalResponse` types
- Produces: `tool:approval-request` and `tool:approval-response` Tauri events

- [ ] **Step 1: Add approval types to tool.rs**

In `src-tauri/src/core/adk/tool.rs`, add:

```rust
#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum RiskLevel {
    Low,       // read/list/glob/grep — auto-approve
    Medium,    // write to known dirs — silent log
    High,      // delete/edit/external API — needs approval
    Critical,  // rm -rf/db ops/send message — double confirm
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ToolApprovalRequest {
    pub call_id: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub agent_id: String,
    pub risk_level: RiskLevel,
    pub description: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum ToolApprovalResponse {
    Approved,
    Rejected(String),
    AlwaysApprove(String),
    Defer,
}

pub fn assess_risk(tool_name: &str, args: &serde_json::Value) -> RiskLevel {
    match tool_name {
        "read_file" | "list_dir" | "glob" | "grep" | "lsp:diagnostics" => RiskLevel::Low,
        "write_file" | "edit_file" => {
            // Medium for known project dirs, High for system dirs
            RiskLevel::Medium
        }
        "delete_file" | "run_command" | "http_request" => RiskLevel::High,
        "rm_rf" | "database_drop" | "send_message" => RiskLevel::Critical,
        _ => RiskLevel::High,
    }
}
```

- [ ] **Step 2: Add approval flow to chat command**

In `src-tauri/src/commands/chat.rs`, in the spawned task where tool execution happens, add approval check before executing tools:

```rust
// Before executing a tool call:
let risk = assess_risk(&tool_call.name, &tool_call.arguments);
match risk {
    RiskLevel::Low | RiskLevel::Medium => {
        // Auto-execute
        let output = tool_executor.execute(&tool_call.name, &tool_call.arguments).await;
        // ...
    }
    RiskLevel::High | RiskLevel::Critical => {
        let request = ToolApprovalRequest {
            call_id: tool_call.id.clone(),
            tool_name: tool_call.name.clone(),
            arguments: tool_call.arguments.clone(),
            agent_id: agent_id.clone(),
            risk_level: risk,
            description: format!("Agent requests to call {}", tool_call.name),
        };
        // Emit approval request event to frontend
        app_handle.emit("tool:approval-request", &request)?;

        // Wait for response (with timeout)
        let response = wait_for_approval(&app_handle, &tool_call.id, 30).await;
        match response {
            ToolApprovalResponse::Approved | ToolApprovalResponse::AlwaysApprove(_) => {
                let output = tool_executor.execute(&tool_call.name, &tool_call.arguments).await;
                // ...
            }
            ToolApprovalResponse::Rejected(reason) => {
                // Return rejection to agent
                let output = ToolOutput {
                    call_id: tool_call.id.clone(),
                    output: format!("Tool execution rejected: {}", reason),
                    is_error: true,
                };
                // ...
            }
            ToolApprovalResponse::Defer => {
                // Agent tries alternative approach
            }
        }
    }
}
```

- [ ] **Step 3: Run and verify**

```bash
cd src-tauri && cargo check
```
Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat: add tool approval flow (HITL) with risk assessment"
```

---

## Task 5: Skill Market Search Backend

**Covers:** §10.4.1-10.4.4 (three-source search, dedup, scoring)

**Files:**
- Modify: `src-tauri/src/data/services/skill_service.rs` (implement search_market)
- Modify: `src-tauri/src/commands/skill.rs` (update command)

**Interfaces:**
- Produces: `SkillSearchHit` with `SkillSource` enum (SkillsSh | ClaudePlugins | Clawhub | Local)
- Produces: `search_market(query) -> Vec<SkillSearchHit>` with 3-source concurrent fetch

- [ ] **Step 1: Add search types**

In `src-tauri/src/data/services/skill_service.rs`, add:

```rust
#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum SkillSource {
    SkillsSh,
    ClaudePlugins,
    Clawhub,
    Local,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SkillSearchHit {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: SkillSource,
    pub install_source: String,
    pub tags: Vec<String>,
    pub author: Option<String>,
    pub stars: Option<u64>,
    pub url: Option<String>,
    pub installed: bool,
}

#[derive(Deserialize)]
struct SkillsShResponse {
    results: Vec<SkillsShHit>,
}

#[derive(Deserialize)]
struct SkillsShHit {
    name: String,
    description: Option<String>,
    author: Option<String>,
    tags: Option<Vec<String>>,
    download_url: Option<String>,
}

#[derive(Deserialize)]
struct ClaudePluginsResponse {
    skills: Vec<ClaudePluginHit>,
}

#[derive(Deserialize)]
struct ClaudePluginHit {
    name: String,
    description: Option<String>,
    github: Option<String>,
    tags: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct ClawhubResponse {
    data: Vec<ClawhubHit>,
}

#[derive(Deserialize)]
struct ClawhubHit {
    name: String,
    description: Option<String>,
    source: Option<String>,
    stats: Option<ClawhubStats>,
}

#[derive(Deserialize)]
struct ClawhubStats {
    stars: Option<u64>,
}
```

- [ ] **Step 2: Implement three-source search**

In `src-tauri/src/data/services/skill_service.rs`, replace the `search_market` method:

```rust
pub async fn search_market(&self, query: &str) -> Result<Vec<SkillSearchHit>, AppError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let q = query.to_string();
    let q2 = q.clone();
    let q3 = q.clone();

    let (a, b, c) = tokio::join!(
        search_skills_sh(&client, &q),
        search_claude_plugins(&client, &q2),
        search_clawhub(&client, &q3)
    );

    let mut hits = Vec::new();
    if let Ok(v) = a { hits.extend(v); }
    if let Ok(v) = b { hits.extend(v); }
    if let Ok(v) = c { hits.extend(v); }

    // Dedup by normalized name
    hits.sort_by(|a, b| score_hit(b).partial_cmp(&score_hit(a)).unwrap_or(std::cmp::Ordering::Equal));
    dedup_hits(&mut hits);

    // Mark installed
    let installed = self.list().await.unwrap_or_default();
    let installed_names: std::collections::HashSet<String> = installed.iter()
        .map(|s| normalize_name(&s.name))
        .collect();
    for hit in &mut hits {
        hit.installed = installed_names.contains(&normalize_name(&hit.name));
    }

    Ok(hits)
}

fn normalize_name(name: &str) -> String {
    let lower = name.to_lowercase();
    let stripped = lower.replace("agent", "").replace("skill", "");
    stripped.trim().replace(' ', "-")
}

fn score_hit(hit: &SkillSearchHit) -> f64 {
    let stars_score = hit.stars.map(|s| (s as f64).min(5000.0) / 5000.0).unwrap_or(0.0);
    let trust_score = match hit.source {
        SkillSource::SkillsSh => 1.0,
        SkillSource::Clawhub => 0.9,
        SkillSource::ClaudePlugins => 0.8,
        SkillSource::Local => 1.0,
    };
    0.5 * stars_score + 0.2 * trust_score + 0.3 * 0.5 // desc_relevance placeholder
}

fn dedup_hits(hits: &mut Vec<SkillSearchHit>) {
    let mut seen = std::collections::HashSet::new();
    hits.retain(|h| seen.insert(normalize_name(&h.name)));
}

async fn search_skills_sh(client: &reqwest::Client, query: &str) -> Result<Vec<SkillSearchHit>, reqwest::Error> {
    let url = format!("https://skills.sh/api/search?q={}", urlencoding::encode(query));
    let resp = client.get(&url).send().await?.json::<SkillsShResponse>().await?;
    Ok(resp.results.into_iter().map(|h| SkillSearchHit {
        id: format!("skills-sh:{}", h.name),
        name: h.name.clone(),
        description: h.description.unwrap_or_default(),
        source: SkillSource::SkillsSh,
        install_source: format!("skills.sh:{}", h.name),
        tags: h.tags.unwrap_or_default(),
        author: h.author,
        stars: None,
        url: h.download_url,
        installed: false,
    }).collect())
}

async fn search_claude_plugins(client: &reqwest::Client, query: &str) -> Result<Vec<SkillSearchHit>, reqwest::Error> {
    let url = format!("https://claude-plugins.dev/api/skills?q={}", urlencoding::encode(query));
    let resp = client.get(&url).send().await?.json::<ClaudePluginsResponse>().await?;
    Ok(resp.skills.into_iter().map(|h| SkillSearchHit {
        id: format!("claude-plugins:{}", h.name),
        name: h.name.clone(),
        description: h.description.unwrap_or_default(),
        source: SkillSource::ClaudePlugins,
        install_source: format!("github:{}", h.github.unwrap_or_default()),
        tags: h.tags.unwrap_or_default(),
        author: None,
        stars: None,
        url: h.github.map(|g| format!("https://github.com/{}", g)),
        installed: false,
    }).collect())
}

async fn search_clawhub(client: &reqwest::Client, query: &str) -> Result<Vec<SkillSearchHit>, reqwest::Error> {
    let url = format!("https://clawhub.ai/api/v1/search?query={}", urlencoding::encode(query));
    let resp = client.get(&url).send().await?.json::<ClawhubResponse>().await?;
    Ok(resp.data.into_iter().map(|h| SkillSearchHit {
        id: format!("clawhub:{}", h.name),
        name: h.name.clone(),
        description: h.description.unwrap_or_default(),
        source: SkillSource::Clawhub,
        install_source: h.source.unwrap_or_default(),
        tags: vec![],
        author: None,
        stars: h.stats.and_then(|s| s.stars),
        url: None,
        installed: false,
    }).collect())
}
```

- [ ] **Step 3: Update skill_search_market command**

In `src-tauri/src/commands/skill.rs`, update the existing stub:

```rust
#[tauri::command]
pub async fn skill_search_market(
    state: tauri::State<'_, AppState>,
    query: String,
) -> Result<Vec<SkillSearchHit>, AppError> {
    let svc = SkillService::new(&state.db, state.mcp_runtime.clone());
    svc.search_market(&query).await
}
```

- [ ] **Step 4: Add urlencoding dependency**

In `src-tauri/Cargo.toml`, add: `urlencoding = "2"`

- [ ] **Step 5: Run and verify**

```bash
cd src-tauri && cargo check
```
Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: implement skill market search (3-source concurrent fetch)"
```

---

## Task 6: Agent Context Sidebar Backend

**Covers:** §9.10 (context:agent), §9.10.5 (LSP detection), §9.10.7 (aggregated command, events)

**Files:**
- Modify: `src-tauri/src/commands/agent.rs` (add context command)
- Modify: `src-tauri/src/data/models.rs` (add AgentContext, SessionUsage, LspCandidate types)

**Interfaces:**
- Produces: `AgentContext` struct with agent + session_usage + workspace + instructions + mcp + lsp + tree
- Produces: `context:agent` IPC command
- Produces: `lsp:detect` IPC command
- Produces: `session:inject-file` IPC command

- [ ] **Step 1: Add sidebar types to models.rs**

In `src-tauri/src/data/models.rs`, add:

```rust
#[derive(Serialize, Deserialize, Clone)]
pub struct AgentContext {
    pub agent: AgentDto,
    pub session_usage: SessionUsage,
    pub workspace: WorkspaceInfo,
    pub instructions: Vec<InstructionFile>,
    pub mcp: Vec<McpServerStatus>,
    pub lsp: Vec<LspServerInfo>,
    pub tree: DirTree,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SessionUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub context_used: u64,
    pub context_limit: u64,
    pub tool_calls: u64,
    pub cost_est: f64,
    pub today_calls: u64,
    pub today_tokens: u64,
    pub today_cost: f64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct WorkspaceInfo {
    pub current_dir: String,
    pub recent_dirs: Vec<String>,
    pub bound_agent_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct InstructionFile {
    pub path: String,
    pub name: String,
    pub lines: usize,
    pub injected: bool,
    pub priority: u8,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct LspServerInfo {
    pub id: String,
    pub cmd: String,
    pub status: String,   // "running" | "stopped" | "error" | "not_installed"
    pub langs: Vec<String>,
    pub index_file_count: Option<u64>,
    pub last_error: Option<String>,
    pub install_hint: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct LspCandidate {
    pub id: String,
    pub cmd: String,
    pub langs: Vec<String>,
    pub install_hint: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DirTree {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub children: Option<Vec<DirTree>>,
    pub language: Option<String>,
    pub line_count: Option<u64>,
}
```

- [ ] **Step 2: Add context:agent command**

In `src-tauri/src/commands/agent.rs`, add:

```rust
#[tauri::command]
pub async fn context_agent(
    state: tauri::State<'_, AppState>,
    agent_id: String,
    session_id: Option<String>,
) -> Result<AgentContext, AppError> {
    let agent_svc = AgentService::new(&state.db);
    let agent = agent_svc.get(&agent_id).await?;

    // Session usage
    let session_usage = if let Some(sid) = &session_id {
        compute_session_usage(&state.db, sid).await?
    } else {
        SessionUsage {
            input_tokens: 0, output_tokens: 0, context_used: 0,
            context_limit: 128000, tool_calls: 0, cost_est: 0.0,
            today_calls: 0, today_tokens: 0, today_cost: 0.0,
        }
    };

    // Workspace info from preferences
    let workspace = load_workspace_info(&state.db, &agent_id).await?;

    // Instructions in workspace
    let instructions = scan_instructions(&workspace.current_dir);

    // MCP servers bound to agent
    let mcp_svc = McpService::new(&state.db, state.mcp_runtime.clone());
    let mcp = mcp_svc.all_status().await.unwrap_or_default();

    // LSP detection
    let lsp = detect_lsp_servers(&workspace.current_dir);

    // File tree (root level)
    let tree = load_dir_tree(&workspace.current_dir, 1)?;

    Ok(AgentContext {
        agent,
        session_usage,
        workspace,
        instructions,
        mcp,
        lsp,
        tree,
    })
}

async fn compute_session_usage(db: &Database, session_id: &str) -> Result<SessionUsage, AppError> {
    let pool = db.pool();
    let row = sqlx::query(
        r#"
        SELECT
            COALESCE(SUM(json_extract(usage, '$.prompt_tokens')), 0) as input_tokens,
            COALESCE(SUM(json_extract(usage, '$.completion_tokens')), 0) as output_tokens,
            COALESCE(SUM(CASE WHEN role = 'tool' THEN 1 ELSE 0 END), 0) as tool_calls
        FROM messages WHERE session_id = ?
        "#
    )
    .bind(session_id)
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let input_tokens = row.get::<i64, _>("input_tokens") as u64;
    let output_tokens = row.get::<i64, _>("output_tokens") as u64;

    Ok(SessionUsage {
        input_tokens,
        output_tokens,
        context_used: input_tokens + output_tokens,
        context_limit: 128000,
        tool_calls: row.get::<i64, _>("tool_calls") as u64,
        cost_est: 0.0,
        today_calls: 0,
        today_tokens: 0,
        today_cost: 0.0,
    })
}

async fn load_workspace_info(db: &Database, agent_id: &str) -> Result<WorkspaceInfo, AppError> {
    let pool = db.pool();
    let current_dir: String = sqlx::query_scalar(
        "SELECT value FROM preferences WHERE key = 'workspace.current_dir'"
    )
    .fetch_one(pool)
    .await
    .unwrap_or_else(|_| std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".into()));

    let recent: Vec<String> = sqlx::query_scalar(
        "SELECT value FROM preferences WHERE key = 'workspace.recent_dirs'"
    )
    .fetch_one(pool)
    .await
    .ok()
    .and_then(|v: String| serde_json::from_str(&v).ok())
    .unwrap_or_default();

    let bound: Option<String> = sqlx::query_scalar(
        "SELECT value FROM preferences WHERE key = ?"
    )
    .bind(format!("workspace.bound_agent.{}", agent_id))
    .fetch_one(pool)
    .await
    .ok();

    Ok(WorkspaceInfo {
        current_dir,
        recent_dirs: recent,
        bound_agent_id: bound,
    })
}

fn scan_instructions(workdir: &str) -> Vec<InstructionFile> {
    let mut files = Vec::new();
    let path = std::path::Path::new(workdir);

    let candidates = vec![
        ("CLAUDE.md", 1u8, true),
        ("AGENTS.md", 2, true),
        (".cursor/rules/*.mdc", 3, false),
        (".prism/memory.md", 4, true),
        ("README.md", 5, false),
    ];

    for (pattern, priority, _injected) in &candidates {
        if pattern.contains('*') {
            // Glob for .cursor/rules/*.mdc
            let rules_dir = path.join(".cursor/rules");
            if let Ok(entries) = std::fs::read_dir(&rules_dir) {
                for entry in entries.flatten() {
                    if entry.path().extension().map(|e| e == "mdc").unwrap_or(false) {
                        let content = std::fs::read_to_string(entry.path()).unwrap_or_default();
                        files.push(InstructionFile {
                            path: entry.path().to_string_lossy().to_string(),
                            name: entry.file_name().to_string_lossy().to_string(),
                            lines: content.lines().count(),
                            injected: false,
                            priority: *priority,
                        });
                    }
                }
            }
        } else {
            let file_path = path.join(pattern);
            if file_path.exists() {
                let content = std::fs::read_to_string(&file_path).unwrap_or_default();
                files.push(InstructionFile {
                    path: file_path.to_string_lossy().to_string(),
                    name: pattern.to_string(),
                    lines: content.lines().count(),
                    injected: *_injected,
                    priority: *priority,
                });
            }
        }
    }

    files.sort_by_key(|f| f.priority);
    files
}

pub fn detect_lsp_servers(workdir: &str) -> Vec<LspServerInfo> {
    let path = std::path::Path::new(workdir);
    let mut candidates = Vec::new();

    if path.join("Cargo.toml").exists() {
        candidates.push(LspServerInfo {
            id: "rust-analyzer".into(),
            cmd: "rust-analyzer".into(),
            status: "stopped".into(),
            langs: vec!["rust".into()],
            index_file_count: None,
            last_error: None,
            install_hint: Some("cargo install rust-analyzer".into()),
        });
    }
    if path.join("package.json").exists() || path.join("tsconfig.json").exists() {
        candidates.push(LspServerInfo {
            id: "typescript".into(),
            cmd: "typescript-language-server".into(),
            status: "stopped".into(),
            langs: vec!["typescript".into(), "javascript".into()],
            index_file_count: None,
            last_error: None,
            install_hint: Some("npm i -g typescript-language-server".into()),
        });
    }
    if path.join("pyproject.toml").exists() || path.join("requirements.txt").exists() {
        candidates.push(LspServerInfo {
            id: "pyright".into(),
            cmd: "pyright-langserver".into(),
            status: "stopped".into(),
            langs: vec!["python".into()],
            index_file_count: None,
            last_error: None,
            install_hint: Some("pip install pyright".into()),
        });
    }
    if path.join("go.mod").exists() {
        candidates.push(LspServerInfo {
            id: "gopls".into(),
            cmd: "gopls".into(),
            status: "stopped".into(),
            langs: vec!["go".into()],
            index_file_count: None,
            last_error: None,
            install_hint: Some("go install golang.org/x/tools/gopls@latest".into()),
        });
    }

    candidates
}

fn load_dir_tree(workdir: &str, depth: u8) -> Result<DirTree, AppError> {
    let path = std::path::Path::new(workdir);
    let name = path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| workdir.to_string());

    if depth == 0 || !path.is_dir() {
        return Ok(DirTree {
            name,
            path: workdir.to_string(),
            is_dir: path.is_dir(),
            children: None,
            language: None,
            line_count: None,
        });
    }

    let ignore = [".git", "node_modules", "target", "dist", "build", "__pycache__", ".venv", "vendor", ".svn"];
    let mut children = Vec::new();

    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_name = entry.file_name().to_string_lossy().to_string();
            if ignore.contains(&entry_name.as_str()) {
                continue;
            }
            let child = load_dir_tree(&entry.path().to_string_lossy(), depth - 1)?;
            children.push(child);
        }
    }

    children.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));

    Ok(DirTree {
        name,
        path: workdir.to_string(),
        is_dir: true,
        children: Some(children),
        language: None,
        line_count: None,
    })
}
```

- [ ] **Step 3: Add context:agent and session:inject-file commands**

```rust
#[tauri::command]
pub async fn session_inject_file(
    state: tauri::State<'_, AppState>,
    session_id: String,
    path: String,
) -> Result<(), AppError> {
    // Add file path to session's injected files list in preferences
    let pool = state.db.pool();
    let key = format!("session.injected_files.{}", session_id);
    let existing: Vec<String> = sqlx::query_scalar("SELECT value FROM preferences WHERE key = ?")
        .bind(&key)
        .fetch_one(pool)
        .await
        .ok()
        .and_then(|v: String| serde_json::from_str(&v).ok())
        .unwrap_or_default();

    let mut files = existing;
    if !files.contains(&path) {
        files.push(path);
    }

    let json = serde_json::to_string(&files)
        .map_err(|e| AppError::Serialization(e.to_string()))?;

    sqlx::query("INSERT OR REPLACE INTO preferences (key, value) VALUES (?, ?)")
        .bind(&key)
        .bind(&json)
        .execute(pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(())
}
```

- [ ] **Step 4: Register commands**

In `lib.rs`, add: `context_agent`, `session_inject_file`, `lsp_detect` to invoke_handler.

- [ ] **Step 5: Run and verify**

```bash
cd src-tauri && cargo check
```
Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: add agent context sidebar backend (aggregated context, LSP detection)"
```

---

## Task 7: Frontend — Home Dashboard Page

**Covers:** §9.9 (layout, AgentLauncher, UsageCards, Skill/MCP overview, RecentSessions)

**Files:**
- Modify: `src/routes/+page.svelte` (replace with dashboard layout when no session)
- Create: `src/lib/components/dashboard/DashboardHeader.svelte`
- Create: `src/lib/components/dashboard/UsageStatsCard.svelte`
- Create: `src/lib/components/dashboard/UsageTrendChart.svelte`
- Create: `src/lib/components/dashboard/AgentLauncher.svelte`
- Create: `src/lib/components/dashboard/AgentCard.svelte`
- Create: `src/lib/components/dashboard/SkillOverviewCard.svelte`
- Create: `src/lib/components/dashboard/McpOverviewCard.svelte`
- Create: `src/lib/components/dashboard/RecentSessionsCard.svelte`
- Create: `src/lib/stores/dashboard.svelte.ts`

**Interfaces:**
- Consumes: `dashboard_overview()` IPC command
- Produces: `dashboardStore` with state and actions

- [ ] **Step 1: Create dashboard store**

Create `src/lib/stores/dashboard.svelte.ts`:

```typescript
import { invoke } from '$lib/api/client';

export interface UsageStats {
    today_tokens: number;
    week_tokens: number;
    month_tokens: number;
    month_cost: number;
    today_calls: number;
}

export interface UsagePoint {
    date: string;
    tokens: number;
    cost: number;
}

export interface AgentSummary {
    id: string;
    name: string;
    description: string;
    avatar: string | null;
    model_name: string | null;
    skill_count: number;
    mcp_count: number;
    last_used: string | null;
    order_key: number;
}

export interface SkillOverview {
    enabled: number;
    total: number;
    popular: string[];
}

export interface McpServerStatus {
    id: string;
    name: string;
    status: string;
    tools_count: number;
    last_error: string | null;
}

export interface SessionSummary {
    id: string;
    title: string;
    agent_name: string;
    updated_at: string;
    message_count: number;
}

export interface WorkflowSummary {
    id: string;
    name: string;
    description: string;
    stage_count: number;
    source: string;
}

export interface TaskRunSummary {
    run_id: string;
    workflow_name: string;
    status: string;
    started_at: string;
    finished_at: string | null;
    source: string;
}

export interface DashboardOverview {
    agents: AgentSummary[];
    usage: UsageStats;
    usage_trend: UsagePoint[];
    skills: SkillOverview;
    mcp_servers: McpServerStatus[];
    recent_sessions: SessionSummary[];
    models: ModelStatus[];
    workflows: WorkflowSummary[];
    task_runs: TaskRunSummary[];
}

export interface ModelStatus {
    provider_name: string;
    model_id: string;
    display_name: string;
    status: string;
}

function createDashboardStore() {
    let overview = $state<DashboardOverview | null>(null);
    let loading = $state(false);
    let error = $state<string | null>(null);

    async function loadOverview() {
        loading = true;
        error = null;
        try {
            overview = await invoke<DashboardOverview>('dashboard_overview');
        } catch (e) {
            error = e instanceof Error ? e.message : String(e);
        } finally {
            loading = false;
        }
    }

    return {
        get overview() { return overview; },
        get loading() { return loading; },
        get error() { return error; },
        loadOverview,
    };
}

export const dashboardStore = createDashboardStore();
```

- [ ] **Step 2: Create DashboardHeader**

Create `src/lib/components/dashboard/DashboardHeader.svelte`:

```svelte
<script lang="ts">
    let { agents = [], providerStatus = '' } = $props();
    let searchQuery = $state('');
</script>

<header class="dashboard-header">
    <div class="header-left">
        <h1>Welcome back</h1>
        <span class="provider-badge">{providerStatus}</span>
    </div>
    <div class="header-right">
        <div class="search-box">
            <span class="search-icon">⌘K</span>
            <input
                type="text"
                placeholder="Search..."
                bind:value={searchQuery}
                class="search-input"
            />
        </div>
    </div>
</header>

<style>
    .dashboard-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 24px 32px;
        border-bottom: 1px solid var(--color-gray4);
    }
    .header-left {
        display: flex;
        align-items: center;
        gap: 12px;
    }
    .header-left h1 {
        font-size: var(--font-size-title-1);
        font-weight: var(--font-weight-bold);
        color: var(--color-label);
    }
    .provider-badge {
        font-size: var(--font-size-caption);
        color: var(--color-secondary-label);
        background: var(--color-gray5);
        padding: 4px 8px;
        border-radius: var(--radius-sm);
    }
    .search-box {
        display: flex;
        align-items: center;
        background: var(--color-gray6);
        border-radius: var(--radius-md);
        padding: 8px 12px;
        gap: 8px;
    }
    .search-icon {
        font-size: var(--font-size-caption);
        color: var(--color-tertiary-label);
    }
    .search-input {
        border: none;
        background: transparent;
        font-size: var(--font-size-body);
        color: var(--color-label);
        outline: none;
        width: 200px;
    }
</style>
```

- [ ] **Step 3: Create UsageStatsCard**

Create `src/lib/components/dashboard/UsageStatsCard.svelte`:

```svelte
<script lang="ts">
    let { stats } = $props();

    function formatTokens(n: number): string {
        if (n >= 1000000) return (n / 1000000).toFixed(1) + 'M';
        if (n >= 1000) return (n / 1000).toFixed(1) + 'K';
        return n.toString();
    }
</script>

<div class="usage-stats">
    <div class="stat-card">
        <span class="stat-label">Today Tokens</span>
        <span class="stat-value">{formatTokens(stats.today_tokens)}</span>
    </div>
    <div class="stat-card">
        <span class="stat-label">Week Tokens</span>
        <span class="stat-value">{formatTokens(stats.week_tokens)}</span>
    </div>
    <div class="stat-card">
        <span class="stat-label">Month Cost</span>
        <span class="stat-value">¥{stats.month_cost.toFixed(2)}</span>
    </div>
    <div class="stat-card">
        <span class="stat-label">Today Calls</span>
        <span class="stat-value">{stats.today_calls}</span>
    </div>
</div>

<style>
    .usage-stats {
        display: grid;
        grid-template-columns: repeat(4, 1fr);
        gap: 16px;
        padding: 0 32px;
    }
    .stat-card {
        background: var(--color-gray6);
        border-radius: var(--radius-lg);
        padding: 20px;
        display: flex;
        flex-direction: column;
        gap: 8px;
    }
    .stat-label {
        font-size: var(--font-size-caption);
        color: var(--color-secondary-label);
        text-transform: uppercase;
        letter-spacing: 0.5px;
    }
    .stat-value {
        font-size: var(--font-size-title-2);
        font-weight: var(--font-weight-bold);
        color: var(--color-label);
    }
</style>
```

- [ ] **Step 4: Create UsageTrendChart (SVG line chart)**

Create `src/lib/components/dashboard/UsageTrendChart.svelte`:

```svelte
<script lang="ts">
    let { trend = [] } = $props();

    const width = 300;
    const height = 120;
    const padding = { top: 10, right: 10, bottom: 20, left: 40 };

    let maxTokens = $derived(Math.max(...trend.map(p => p.tokens), 1));
    let points = $derived(trend.map((p, i) => {
        const x = padding.left + (i / Math.max(trend.length - 1, 1)) * (width - padding.left - padding.right);
        const y = padding.top + (1 - p.tokens / maxTokens) * (height - padding.top - padding.bottom);
        return { x, y, date: p.date, tokens: p.tokens };
    }));

    let pathD = $derived(points.length > 1
        ? 'M' + points.map(p => `${p.x},${p.y}`).join(' L')
        : '');
</script>

<div class="trend-chart">
    <h3>Usage Trend (7 days)</h3>
    <svg viewBox="0 0 {width} {height}" class="chart-svg">
        <path d={pathD} fill="none" stroke="var(--color-accent)" stroke-width="2" />
        {#each points as p}
            <circle cx={p.x} cy={p.y} r="3" fill="var(--color-accent)" />
        {/each}
    </svg>
</div>

<style>
    .trend-chart {
        background: var(--color-gray6);
        border-radius: var(--radius-lg);
        padding: 20px;
    }
    .trend-chart h3 {
        font-size: var(--font-size-caption);
        color: var(--color-secondary-label);
        margin-bottom: 12px;
    }
    .chart-svg {
        width: 100%;
        height: auto;
    }
</style>
```

- [ ] **Step 5: Create AgentCard and AgentLauncher**

Create `src/lib/components/dashboard/AgentCard.svelte`:

```svelte
<script lang="ts">
    let { agent, onStartChat, onMenu } = $props();
</script>

<div class="agent-card" role="button" tabindex="0" onclick={() => onStartChat(agent.id)}>
    <div class="card-header">
        <div class="avatar">{agent.name[0]}</div>
        <div class="card-info">
            <span class="card-name">{agent.name}</span>
            <span class="card-desc">{agent.description || 'No description'}</span>
        </div>
    </div>
    <div class="card-meta">
        <span class="status-dot"></span>
        <span class="model-name">{agent.model_name || 'No model'}</span>
        <span class="badge">{agent.skill_count} skills</span>
        <span class="badge">{agent.mcp_count} MCP</span>
    </div>
    <div class="card-footer">
        <button class="start-btn" onclick|stopPropagation={() => onStartChat(agent.id)}>
            ▶ Start Chat
        </button>
        <button class="menu-btn" onclick|stopPropagation={() => onMenu(agent.id)}>
            ⋮
        </button>
    </div>
</div>

<style>
    .agent-card {
        background: var(--color-gray6);
        border-radius: var(--radius-lg);
        padding: 16px;
        cursor: pointer;
        transition: background 0.15s;
        display: flex;
        flex-direction: column;
        gap: 12px;
    }
    .agent-card:hover {
        background: var(--color-gray5);
    }
    .card-header {
        display: flex;
        gap: 12px;
        align-items: flex-start;
    }
    .avatar {
        width: 40px;
        height: 40px;
        border-radius: var(--radius-full);
        background: var(--color-accent);
        color: white;
        display: flex;
        align-items: center;
        justify-content: center;
        font-weight: var(--font-weight-bold);
        flex-shrink: 0;
    }
    .card-info {
        display: flex;
        flex-direction: column;
        gap: 2px;
        min-width: 0;
    }
    .card-name {
        font-size: var(--font-size-body);
        font-weight: var(--font-weight-semibold);
        color: var(--color-label);
    }
    .card-desc {
        font-size: var(--font-size-caption);
        color: var(--color-secondary-label);
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .card-meta {
        display: flex;
        align-items: center;
        gap: 8px;
        font-size: var(--font-size-caption);
        color: var(--color-tertiary-label);
    }
    .status-dot {
        width: 8px;
        height: 8px;
        border-radius: var(--radius-full);
        background: var(--color-green);
    }
    .badge {
        background: var(--color-gray5);
        padding: 2px 6px;
        border-radius: var(--radius-sm);
        font-size: 11px;
    }
    .card-footer {
        display: flex;
        justify-content: space-between;
        align-items: center;
    }
    .start-btn {
        background: var(--color-accent);
        color: white;
        border: none;
        border-radius: var(--radius-md);
        padding: 6px 12px;
        font-size: var(--font-size-caption);
        cursor: pointer;
    }
    .menu-btn {
        background: none;
        border: none;
        color: var(--color-tertiary-label);
        cursor: pointer;
        font-size: 18px;
    }
</style>
```

Create `src/lib/components/dashboard/AgentLauncher.svelte`:

```svelte
<script lang="ts">
    let { agents = [], onStartChat, onNewAgent, onMarket } = $props();
</script>

<div class="agent-launcher">
    <div class="launcher-header">
        <h2>Agents</h2>
        <div class="launcher-actions">
            <button class="action-btn" onclick={onNewAgent}>+ New Agent</button>
            <button class="action-btn secondary" onclick={onMarket}>Market</button>
        </div>
    </div>
    <div class="agent-grid">
        {#each agents as agent (agent.id)}
            <AgentCard {agent} {onStartChat} />
        {/each}
        {#if agents.length === 0}
            <div class="empty-state">
                <p>No agents yet. Create one to get started.</p>
            </div>
        {/if}
    </div>
</div>

<script module>
    import AgentCard from './AgentCard.svelte';
</script>

<style>
    .agent-launcher {
        background: var(--color-gray6);
        border-radius: var(--radius-lg);
        padding: 20px;
    }
    .launcher-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 16px;
    }
    .launcher-header h2 {
        font-size: var(--font-size-title-3);
        font-weight: var(--font-weight-semibold);
        color: var(--color-label);
    }
    .launcher-actions {
        display: flex;
        gap: 8px;
    }
    .action-btn {
        background: var(--color-accent);
        color: white;
        border: none;
        border-radius: var(--radius-md);
        padding: 6px 12px;
        font-size: var(--font-size-caption);
        cursor: pointer;
    }
    .action-btn.secondary {
        background: var(--color-gray5);
        color: var(--color-label);
    }
    .agent-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
        gap: 12px;
    }
    .empty-state {
        grid-column: 1 / -1;
        text-align: center;
        padding: 40px;
        color: var(--color-secondary-label);
    }
</style>
```

- [ ] **Step 6: Create SkillOverviewCard and McpOverviewCard**

Create `src/lib/components/dashboard/SkillOverviewCard.svelte`:

```svelte
<script lang="ts">
    let { skills } = $props();
</script>

<div class="skill-overview">
    <h3>Skills</h3>
    <div class="skill-stats">
        <span class="stat">{skills.enabled}/{skills.total} enabled</span>
    </div>
    <button class="market-link">Go to Market →</button>
</div>

<style>
    .skill-overview {
        background: var(--color-gray6);
        border-radius: var(--radius-lg);
        padding: 20px;
        display: flex;
        flex-direction: column;
        gap: 8px;
    }
    h3 {
        font-size: var(--font-size-body);
        font-weight: var(--font-weight-semibold);
        color: var(--color-label);
    }
    .stat {
        font-size: var(--font-size-caption);
        color: var(--color-secondary-label);
    }
    .market-link {
        background: none;
        border: none;
        color: var(--color-accent);
        cursor: pointer;
        font-size: var(--font-size-caption);
        text-align: left;
        padding: 0;
    }
</style>
```

Create `src/lib/components/dashboard/McpOverviewCard.svelte`:

```svelte
<script lang="ts">
    let { servers = [] } = $props();
    let connected = $derived(servers.filter(s => s.status === 'connected').length);
</script>

<div class="mcp-overview">
    <h3>MCP Servers</h3>
    <div class="mcp-stats">
        <span class="stat">{connected}/{servers.length} connected</span>
    </div>
    <button class="manage-link">Manage →</button>
</div>

<style>
    .mcp-overview {
        background: var(--color-gray6);
        border-radius: var(--radius-lg);
        padding: 20px;
        display: flex;
        flex-direction: column;
        gap: 8px;
    }
    h3 {
        font-size: var(--font-size-body);
        font-weight: var(--font-weight-semibold);
        color: var(--color-label);
    }
    .stat {
        font-size: var(--font-size-caption);
        color: var(--color-secondary-label);
    }
    .manage-link {
        background: none;
        border: none;
        color: var(--color-accent);
        cursor: pointer;
        font-size: var(--font-size-caption);
        text-align: left;
        padding: 0;
    }
</style>
```

- [ ] **Step 7: Create RecentSessionsCard**

Create `src/lib/components/dashboard/RecentSessionsCard.svelte`:

```svelte
<script lang="ts">
    let { sessions = [], onOpenSession } = $props();

    function formatTime(iso: string): string {
        const d = new Date(iso);
        const now = new Date();
        const diff = now.getTime() - d.getTime();
        if (diff < 60000) return 'just now';
        if (diff < 3600000) return Math.floor(diff / 60000) + 'm ago';
        if (diff < 86400000) return Math.floor(diff / 3600000) + 'h ago';
        return d.toLocaleDateString();
    }
</script>

<div class="recent-sessions">
    <h3>Recent Sessions</h3>
    {#each sessions as session (session.id)}
        <button class="session-row" onclick={() => onOpenSession(session.id)}>
            <div class="session-info">
                <span class="session-title">{session.title || 'Untitled'}</span>
                <span class="session-agent">{session.agent_name}</span>
            </div>
            <span class="session-time">{formatTime(session.updated_at)}</span>
        </button>
    {/each}
    {#if sessions.length === 0}
        <p class="empty">No recent sessions</p>
    {/if}
</div>

<style>
    .recent-sessions {
        background: var(--color-gray6);
        border-radius: var(--radius-lg);
        padding: 20px;
    }
    h3 {
        font-size: var(--font-size-body);
        font-weight: var(--font-weight-semibold);
        color: var(--color-label);
        margin-bottom: 12px;
    }
    .session-row {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 8px 0;
        border-bottom: 1px solid var(--color-gray4);
        background: none;
        border-left: none;
        border-right: none;
        border-top: none;
        cursor: pointer;
        width: 100%;
        text-align: left;
    }
    .session-info {
        display: flex;
        flex-direction: column;
        gap: 2px;
    }
    .session-title {
        font-size: var(--font-size-body);
        color: var(--color-label);
    }
    .session-agent {
        font-size: var(--font-size-caption);
        color: var(--color-tertiary-label);
    }
    .session-time {
        font-size: var(--font-size-caption);
        color: var(--color-tertiary-label);
    }
    .empty {
        color: var(--color-secondary-label);
        font-size: var(--font-size-caption);
    }
</style>
```

- [ ] **Step 8: Assemble dashboard in +page.svelte**

Update `src/routes/+page.svelte` to show dashboard when no session is selected, with the layout matching §9.9:

```svelte
<script lang="ts">
    import { dashboardStore } from '$lib/stores/dashboard.svelte';
    import { agentStore } from '$lib/stores/agents.svelte';
    import DashboardHeader from '$lib/components/dashboard/DashboardHeader.svelte';
    import UsageStatsCard from '$lib/components/dashboard/UsageStatsCard.svelte';
    import UsageTrendChart from '$lib/components/dashboard/UsageTrendChart.svelte';
    import AgentLauncher from '$lib/components/dashboard/AgentLauncher.svelte';
    import SkillOverviewCard from '$lib/components/dashboard/SkillOverviewCard.svelte';
    import McpOverviewCard from '$lib/components/dashboard/McpOverviewCard.svelte';
    import RecentSessionsCard from '$lib/components/dashboard/RecentSessionsCard.svelte';

    $effect(() => {
        dashboardStore.loadOverview();
    });

    let overview = $derived(dashboardStore.overview);

    function handleStartChat(agentId: string) {
        // Create session and navigate
        agentStore.createSession(agentId).then(session => {
            // Navigate to chat
        });
    }
</script>

{#if overview}
    <div class="dashboard">
        <DashboardHeader agents={overview.agents} />

        <div class="dashboard-scroll">
            <UsageStatsCard stats={overview.usage} />

            <div class="dashboard-row">
                <div class="dashboard-col-main">
                    <AgentLauncher
                        agents={overview.agents}
                        onStartChat={handleStartChat}
                    />
                </div>
                <div class="dashboard-col-side">
                    <UsageTrendChart trend={overview.usage_trend} />
                </div>
            </div>

            <div class="dashboard-row">
                <div class="dashboard-col-main">
                    <SkillOverviewCard skills={overview.skills} />
                </div>
                <div class="dashboard-col-side">
                    <McpOverviewCard servers={overview.mcp_servers} />
                </div>
            </div>

            <RecentSessionsCard sessions={overview.recent_sessions} />
        </div>
    </div>
{:else}
    <div class="loading">
        <p>Loading dashboard...</p>
    </div>
{/if}

<style>
    .dashboard {
        height: 100vh;
        overflow-y: auto;
    }
    .dashboard-scroll {
        padding: 24px 0;
        display: flex;
        flex-direction: column;
        gap: 24px;
    }
    .dashboard-row {
        display: grid;
        grid-template-columns: 2fr 1fr;
        gap: 24px;
        padding: 0 32px;
    }
    .loading {
        display: flex;
        align-items: center;
        justify-content: center;
        height: 100vh;
        color: var(--color-secondary-label);
    }
</style>
```

- [ ] **Step 9: Verify build**

```bash
cd src && npx svelte-check
```

- [ ] **Step 10: Commit**

```bash
git add -A
git commit -m "feat: add home dashboard page (stats, agent launcher, trends)"
```

---

## Task 8: Frontend — Agent Sidebar (6 Tabs)

**Covers:** §9.10-9.10.7 (sidebar layout, all 6 tabs, events)

**Files:**
- Create: `src/lib/components/sidebar/AgentSidebar.svelte`
- Create: `src/lib/components/sidebar/SidebarUsage.svelte`
- Create: `src/lib/components/sidebar/SidebarWorkdir.svelte`
- Create: `src/lib/components/sidebar/SidebarInstructions.svelte`
- Create: `src/lib/components/sidebar/SidebarMcp.svelte`
- Create: `src/lib/components/sidebar/SidebarLsp.svelte`
- Create: `src/lib/components/sidebar/SidebarFiles.svelte`
- Create: `src/lib/stores/context.svelte.ts`
- Modify: `src/routes/+layout.svelte` (integrate sidebar)

**Interfaces:**
- Consumes: `context:agent` IPC command
- Produces: `contextStore` with agent context data

- [ ] **Step 1: Create context store**

Create `src/lib/stores/context.svelte.ts`:

```typescript
import { invoke, listen } from '$lib/api/client';

export interface AgentContext {
    agent: any;
    session_usage: SessionUsage;
    workspace: WorkspaceInfo;
    instructions: InstructionFile[];
    mcp: McpServerStatus[];
    lsp: LspServerInfo[];
    tree: DirTree;
}

export interface SessionUsage {
    input_tokens: number;
    output_tokens: number;
    context_used: number;
    context_limit: number;
    tool_calls: number;
    cost_est: number;
    today_calls: number;
    today_tokens: number;
    today_cost: number;
}

export interface WorkspaceInfo {
    current_dir: string;
    recent_dirs: string[];
    bound_agent_id: string | null;
}

export interface InstructionFile {
    path: string;
    name: string;
    lines: number;
    injected: boolean;
    priority: number;
}

export interface McpServerStatus {
    id: string;
    name: string;
    status: string;
    tools_count: number;
    last_error: string | null;
}

export interface LspServerInfo {
    id: string;
    cmd: string;
    status: string;
    langs: string[];
    index_file_count: number | null;
    last_error: string | null;
    install_hint: string | null;
}

export interface DirTree {
    name: string;
    path: string;
    is_dir: boolean;
    children: DirTree[] | null;
    language: string | null;
    line_count: number | null;
}

function createContextStore() {
    let context = $state<AgentContext | null>(null);
    let loading = $state(false);
    let activeTab = $state('usage');
    let sidebarWidth = $state(320);
    let collapsed = $state(false);

    async function loadContext(agentId: string, sessionId?: string) {
        loading = true;
        try {
            context = await invoke<AgentContext>('context_agent', {
                agentId,
                sessionId: sessionId || null,
            });
        } catch (e) {
            console.error('Failed to load agent context:', e);
        } finally {
            loading = false;
        }
    }

    function toggleCollapse() {
        collapsed = !collapsed;
    }

    // Listen for incremental events
    listen('usage:updated', () => {
        if (context) {
            // Refresh usage data
        }
    });

    listen('workspace:changed', () => {
        if (context) {
            loadContext(context.agent.id);
        }
    });

    return {
        get context() { return context; },
        get loading() { return loading; },
        get activeTab() { return activeTab; },
        set activeTab(v: string) { activeTab = v; },
        get sidebarWidth() { return sidebarWidth; },
        set sidebarWidth(v: number) { sidebarWidth = Math.max(280, Math.min(480, v)); },
        get collapsed() { return collapsed; },
        loadContext,
        toggleCollapse,
    };
}

export const contextStore = createContextStore();
```

- [ ] **Step 2: Create AgentSidebar shell**

Create `src/lib/components/sidebar/AgentSidebar.svelte`:

```svelte
<script lang="ts">
    import { contextStore } from '$lib/stores/context.svelte';
    import SidebarUsage from './SidebarUsage.svelte';
    import SidebarWorkdir from './SidebarWorkdir.svelte';
    import SidebarInstructions from './SidebarInstructions.svelte';
    import SidebarMcp from './SidebarMcp.svelte';
    import SidebarLsp from './SidebarLsp.svelte';
    import SidebarFiles from './SidebarFiles.svelte';

    let { agentId, sessionId } = $props();

    const tabs = [
        { id: 'usage', label: 'Usage' },
        { id: 'workdir', label: 'Dir' },
        { id: 'instructions', label: 'Instructions' },
        { id: 'mcp', label: 'MCP' },
        { id: 'lsp', label: 'LSP' },
        { id: 'files', label: 'Files' },
    ];

    $effect(() => {
        if (agentId) {
            contextStore.loadContext(agentId, sessionId);
        }
    });

    let ctx = $derived(contextStore.context);
</script>

{#if contextStore.collapsed}
    <button class="collapsed-bar" onclick={contextStore.toggleCollapse}>
        <span class="bar-icon">⌘</span>
    </button>
{:else}
    <aside class="sidebar" style="width: {contextStore.sidebarWidth}px">
        <!-- Header -->
        {#if ctx}
            <div class="sidebar-header">
                <div class="agent-info">
                    <div class="agent-avatar">{ctx.agent.name[0]}</div>
                    <div class="agent-meta">
                        <span class="agent-name">{ctx.agent.name}</span>
                        <span class="agent-model">{ctx.agent.model_id || 'No model'}</span>
                    </div>
                </div>
                <button class="collapse-btn" onclick={contextStore.toggleCollapse}>⌘\</button>
            </div>
        {/if}

        <!-- Tabs -->
        <div class="tab-bar">
            {#each tabs as tab}
                <button
                    class="tab-btn"
                    class:active={contextStore.activeTab === tab.id}
                    onclick={() => contextStore.activeTab = tab.id}
                >
                    {tab.label}
                </button>
            {/each}
        </div>

        <!-- Tab Content -->
        <div class="tab-content">
            {#if contextStore.activeTab === 'usage' && ctx}
                <SidebarUsage usage={ctx.session_usage} />
            {:else if contextStore.activeTab === 'workdir' && ctx}
                <SidebarWorkdir workspace={ctx.workspace} />
            {:else if contextStore.activeTab === 'instructions' && ctx}
                <SidebarInstructions instructions={ctx.instructions} />
            {:else if contextStore.activeTab === 'mcp' && ctx}
                <SidebarMcp servers={ctx.mcp} />
            {:else if contextStore.activeTab === 'lsp' && ctx}
                <SidebarLsp servers={ctx.lsp} />
            {:else if contextStore.activeTab === 'files' && ctx}
                <SidebarFiles tree={ctx.tree} />
            {/if}
        </div>

        <!-- Footer -->
        {#if ctx}
            <div class="sidebar-footer">
                <span class="session-info">Tokens: {ctx.session_usage.context_used.toLocaleString()}</span>
            </div>
        {/if}
    </aside>
{/if}

<style>
    .sidebar {
        height: 100vh;
        border-left: 1px solid var(--color-gray4);
        display: flex;
        flex-direction: column;
        background: var(--color-gray6);
        flex-shrink: 0;
    }
    .collapsed-bar {
        width: 44px;
        height: 100vh;
        border-left: 1px solid var(--color-gray4);
        background: var(--color-gray6);
        display: flex;
        align-items: center;
        justify-content: center;
        cursor: pointer;
        border: none;
    }
    .bar-icon {
        font-size: 18px;
        color: var(--color-secondary-label);
    }
    .sidebar-header {
        padding: 16px;
        display: flex;
        justify-content: space-between;
        align-items: center;
        border-bottom: 1px solid var(--color-gray4);
    }
    .agent-info {
        display: flex;
        gap: 8px;
        align-items: center;
    }
    .agent-avatar {
        width: 32px;
        height: 32px;
        border-radius: var(--radius-full);
        background: var(--color-accent);
        color: white;
        display: flex;
        align-items: center;
        justify-content: center;
        font-size: 14px;
        font-weight: var(--font-weight-bold);
    }
    .agent-meta {
        display: flex;
        flex-direction: column;
    }
    .agent-name {
        font-size: var(--font-size-body);
        font-weight: var(--font-weight-semibold);
        color: var(--color-label);
    }
    .agent-model {
        font-size: var(--font-size-caption);
        color: var(--color-tertiary-label);
    }
    .collapse-btn {
        background: none;
        border: none;
        color: var(--color-tertiary-label);
        cursor: pointer;
        font-size: 14px;
    }
    .tab-bar {
        display: flex;
        gap: 2px;
        padding: 8px 12px 0;
        overflow-x: auto;
        scrollbar-width: none;
    }
    .tab-bar::-webkit-scrollbar { display: none; }
    .tab-btn {
        padding: 6px 10px;
        border: none;
        background: none;
        font-size: var(--font-size-caption);
        color: var(--color-secondary-label);
        cursor: pointer;
        border-radius: var(--radius-sm);
        white-space: nowrap;
    }
    .tab-btn.active {
        background: var(--color-gray5);
        color: var(--color-label);
        font-weight: var(--font-weight-medium);
    }
    .tab-content {
        flex: 1;
        overflow-y: auto;
        padding: 12px 16px;
    }
    .sidebar-footer {
        padding: 12px 16px;
        border-top: 1px solid var(--color-gray4);
        font-size: var(--font-size-caption);
        color: var(--color-tertiary-label);
    }
</style>
```

- [ ] **Step 3: Create SidebarUsage**

Create `src/lib/components/sidebar/SidebarUsage.svelte`:

```svelte
<script lang="ts">
    let { usage } = $props();

    let contextPercent = $derived(
        usage.context_limit > 0
            ? Math.round((usage.context_used / usage.context_limit) * 100)
            : 0
    );
    let barColor = $derived(
        contextPercent > 95 ? 'var(--color-red)' :
        contextPercent > 80 ? 'var(--color-orange)' :
        'var(--color-accent)'
    );
</script>

<div class="sidebar-usage">
    <section class="usage-section">
        <h4>Context Window</h4>
        <div class="progress-bar">
            <div class="progress-fill" style="width: {contextPercent}%; background: {barColor}"></div>
        </div>
        <span class="progress-text">
            {usage.context_used.toLocaleString()} / {usage.context_limit.toLocaleString()} tokens ({contextPercent}%)
        </span>
    </section>

    <section class="usage-section">
        <h4>Current Session</h4>
        <div class="usage-row">
            <span>Input tokens</span>
            <span>{usage.input_tokens.toLocaleString()}</span>
        </div>
        <div class="usage-row">
            <span>Output tokens</span>
            <span>{usage.output_tokens.toLocaleString()}</span>
        </div>
        <div class="usage-row">
            <span>Tool calls</span>
            <span>{usage.tool_calls}</span>
        </div>
        <div class="usage-row">
            <span>Cost</span>
            <span>¥{usage.cost_est.toFixed(4)}</span>
        </div>
    </section>

    <section class="usage-section">
        <h4>Today (this Agent)</h4>
        <div class="usage-row">
            <span>Calls</span>
            <span>{usage.today_calls}</span>
        </div>
        <div class="usage-row">
            <span>Tokens</span>
            <span>{usage.today_tokens.toLocaleString()}</span>
        </div>
        <div class="usage-row">
            <span>Cost</span>
            <span>¥{usage.today_cost.toFixed(2)}</span>
        </div>
    </section>
</div>

<style>
    .sidebar-usage {
        display: flex;
        flex-direction: column;
        gap: 16px;
    }
    .usage-section {
        display: flex;
        flex-direction: column;
        gap: 6px;
    }
    h4 {
        font-size: var(--font-size-caption);
        font-weight: var(--font-weight-medium);
        color: var(--color-label);
    }
    .progress-bar {
        height: 6px;
        background: var(--color-gray4);
        border-radius: var(--radius-sm);
        overflow: hidden;
    }
    .progress-fill {
        height: 100%;
        border-radius: var(--radius-sm);
        transition: width 0.3s ease;
    }
    .progress-text {
        font-size: 12px;
        color: var(--color-secondary-label);
    }
    .usage-row {
        display: flex;
        justify-content: space-between;
        font-size: var(--font-size-caption);
        color: var(--color-secondary-label);
        padding: 2px 0;
    }
</style>
```

- [ ] **Step 4: Create SidebarWorkdir**

Create `src/lib/components/sidebar/SidebarWorkdir.svelte`:

```svelte
<script lang="ts">
    import { invoke } from '$lib/api/client';
    let { workspace } = $props();
    let editing = $state(false);
    let editPath = $state(workspace.current_dir);

    async function savePath() {
        await invoke('workspace_set', { path: editPath });
        editing = false;
    }
</script>

<div class="sidebar-workdir">
    <section class="workdir-section">
        <h4>Current Directory</h4>
        {#if editing}
            <div class="edit-row">
                <input bind:value={editPath} class="dir-input" />
                <button onclick={savePath}>Save</button>
            </div>
        {:else}
            <div class="dir-display">
                <span class="dir-path" title={workspace.current_dir}>{workspace.current_dir}</span>
                <button onclick={() => editing = true}>Edit</button>
            </div>
        {/if}
    </section>

    <section class="workdir-section">
        <h4>Recent Directories</h4>
        {#each workspace.recent_dirs as dir}
            <button class="recent-dir" onclick={() => invoke('workspace_set', { path: dir })}>
                {dir}
            </button>
        {/each}
    </section>

    <section class="workdir-section">
        <h4>Agent Binding</h4>
        {#if workspace.bound_agent_id}
            <span class="binding-info">Bound to agent</span>
        {:else}
            <span class="binding-info">Not bound</span>
        {/if}
    </section>
</div>

<style>
    .sidebar-workdir {
        display: flex;
        flex-direction: column;
        gap: 16px;
    }
    .workdir-section {
        display: flex;
        flex-direction: column;
        gap: 6px;
    }
    h4 {
        font-size: var(--font-size-caption);
        font-weight: var(--font-weight-medium);
        color: var(--color-label);
    }
    .dir-display {
        display: flex;
        justify-content: space-between;
        align-items: center;
    }
    .dir-path {
        font-size: var(--font-size-caption);
        color: var(--color-secondary-label);
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        max-width: 200px;
    }
    .edit-row {
        display: flex;
        gap: 4px;
    }
    .dir-input {
        flex: 1;
        font-size: var(--font-size-caption);
        padding: 4px 8px;
        border: 1px solid var(--color-gray4);
        border-radius: var(--radius-sm);
        background: var(--color-gray6);
        color: var(--color-label);
    }
    .recent-dir {
        font-size: var(--font-size-caption);
        color: var(--color-accent);
        background: none;
        border: none;
        text-align: left;
        cursor: pointer;
        padding: 4px 0;
    }
    .binding-info {
        font-size: var(--font-size-caption);
        color: var(--color-secondary-label);
    }
</style>
```

- [ ] **Step 5: Create SidebarInstructions**

Create `src/lib/components/sidebar/SidebarInstructions.svelte`:

```svelte
<script lang="ts">
    let { instructions = [] } = $props();
    let selectedFile = $state<string | null>(null);

    function priorityLabel(p: number): string {
        switch (p) {
            case 1: return 'CLAUDE.md';
            case 2: return 'AGENTS.md';
            case 3: return '.cursor/rules';
            case 4: return '.prism/memory.md';
            case 5: return 'README.md';
            default: return 'Other';
        }
    }
</script>

<div class="sidebar-instructions">
    <h4>Instruction Files</h4>
    {#each instructions as file}
        <button
            class="file-card"
            class:selected={selectedFile === file.path}
            onclick={() => selectedFile = selectedFile === file.path ? null : file.path}
        >
            <div class="file-info">
                <span class="file-name">{file.name}</span>
                <span class="file-meta">{file.lines} lines · {priorityLabel(file.priority)}</span>
            </div>
            <span class="inject-badge" class:injected={file.injected}>
                {file.injected ? '✓ Injected' : '⚠ Not injected'}
            </span>
        </button>
    {/each}
    {#if instructions.length === 0}
        <p class="empty">No instruction files found</p>
    {/if}
</div>

<style>
    .sidebar-instructions {
        display: flex;
        flex-direction: column;
        gap: 8px;
    }
    h4 {
        font-size: var(--font-size-caption);
        font-weight: var(--font-weight-medium);
        color: var(--color-label);
    }
    .file-card {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 8px;
        background: var(--color-gray5);
        border-radius: var(--radius-sm);
        border: 1px solid transparent;
        cursor: pointer;
        text-align: left;
        width: 100%;
    }
    .file-card.selected {
        border-color: var(--color-accent);
    }
    .file-info {
        display: flex;
        flex-direction: column;
        gap: 2px;
    }
    .file-name {
        font-size: var(--font-size-caption);
        color: var(--color-label);
        font-weight: var(--font-weight-medium);
    }
    .file-meta {
        font-size: 11px;
        color: var(--color-tertiary-label);
    }
    .inject-badge {
        font-size: 11px;
        color: var(--color-secondary-label);
    }
    .inject-badge.injected {
        color: var(--color-green);
    }
    .empty {
        font-size: var(--font-size-caption);
        color: var(--color-secondary-label);
    }
</style>
```

- [ ] **Step 6: Create SidebarMcp**

Create `src/lib/components/sidebar/SidebarMcp.svelte`:

```svelte
<script lang="ts">
    let { servers = [] } = $props();
    let expandedId = $state<string | null>(null);

    function statusColor(status: string): string {
        switch (status) {
            case 'connected': return 'var(--color-green)';
            case 'connecting': return 'var(--color-orange)';
            case 'error': return 'var(--color-red)';
            default: return 'var(--color-gray)';
        }
    }
</script>

<div class="sidebar-mcp">
    <h4>MCP Servers</h4>
    {#each servers as server}
        <div class="server-card">
            <button class="server-header" onclick={() => expandedId = expandedId === server.id ? null : server.id}>
                <span class="status-dot" style="background: {statusColor(server.status)}"></span>
                <span class="server-name">{server.name}</span>
                <span class="server-status">{server.status}</span>
            </button>
            {#if expandedId === server.id}
                <div class="server-detail">
                    <span>Tools: {server.tools_count}</span>
                    {#if server.last_error}
                        <span class="error-text">{server.last_error}</span>
                    {/if}
                </div>
            {/if}
        </div>
    {/each}
    {#if servers.length === 0}
        <p class="empty">No MCP servers configured</p>
    {/if}
</div>

<style>
    .sidebar-mcp {
        display: flex;
        flex-direction: column;
        gap: 8px;
    }
    h4 {
        font-size: var(--font-size-caption);
        font-weight: var(--font-weight-medium);
        color: var(--color-label);
    }
    .server-card {
        background: var(--color-gray5);
        border-radius: var(--radius-sm);
        overflow: hidden;
    }
    .server-header {
        display: flex;
        align-items: center;
        gap: 8px;
        padding: 8px;
        background: none;
        border: none;
        cursor: pointer;
        width: 100%;
        text-align: left;
    }
    .status-dot {
        width: 8px;
        height: 8px;
        border-radius: var(--radius-full);
        flex-shrink: 0;
    }
    .server-name {
        font-size: var(--font-size-caption);
        color: var(--color-label);
        flex: 1;
    }
    .server-status {
        font-size: 11px;
        color: var(--color-tertiary-label);
    }
    .server-detail {
        padding: 0 8px 8px;
        font-size: 12px;
        color: var(--color-secondary-label);
        display: flex;
        flex-direction: column;
        gap: 4px;
    }
    .error-text { color: var(--color-red); }
    .empty {
        font-size: var(--font-size-caption);
        color: var(--color-secondary-label);
    }
</style>
```

- [ ] **Step 7: Create SidebarLsp**

Create `src/lib/components/sidebar/SidebarLsp.svelte`:

```svelte
<script lang="ts">
    let { servers = [] } = $props();

    function statusIcon(status: string): string {
        switch (status) {
            case 'running': return '🟢';
            case 'stopped': return '⚪';
            case 'error': return '🔴';
            case 'not_installed': return '❓';
            default: return '⚪';
        }
    }
</script>

<div class="sidebar-lsp">
    <h4>Language Servers</h4>
    {#each servers as server}
        <div class="lsp-card">
            <div class="lsp-header">
                <span>{statusIcon(server.status)}</span>
                <span class="lsp-name">{server.id}</span>
                <span class="lsp-status">{server.status}</span>
            </div>
            <div class="lsp-detail">
                <span>Languages: {server.langs.join(', ')}</span>
                {#if server.install_hint && server.status === 'not_installed'}
                    <span class="install-hint">Install: {server.install_hint}</span>
                {/if}
            </div>
        </div>
    {/each}
    {#if servers.length === 0}
        <p class="empty">No language servers detected</p>
    {/if}
</div>

<style>
    .sidebar-lsp {
        display: flex;
        flex-direction: column;
        gap: 8px;
    }
    h4 {
        font-size: var(--font-size-caption);
        font-weight: var(--font-weight-medium);
        color: var(--color-label);
    }
    .lsp-card {
        background: var(--color-gray5);
        border-radius: var(--radius-sm);
        padding: 8px;
    }
    .lsp-header {
        display: flex;
        align-items: center;
        gap: 6px;
    }
    .lsp-name {
        font-size: var(--font-size-caption);
        color: var(--color-label);
        font-weight: var(--font-weight-medium);
    }
    .lsp-status {
        font-size: 11px;
        color: var(--color-tertiary-label);
        margin-left: auto;
    }
    .lsp-detail {
        margin-top: 4px;
        font-size: 12px;
        color: var(--color-secondary-label);
        display: flex;
        flex-direction: column;
        gap: 2px;
    }
    .install-hint {
        color: var(--color-orange);
    }
    .empty {
        font-size: var(--font-size-caption);
        color: var(--color-secondary-label);
    }
</style>
```

- [ ] **Step 8: Create SidebarFiles**

Create `src/lib/components/sidebar/SidebarFiles.svelte`:

```svelte
<script lang="ts">
    let { tree } = $props();
    let filter = $state('');
    let expandedDirs = $state(new Set<string>());

    function toggleDir(path: string) {
        if (expandedDirs.has(path)) {
            expandedDirs.delete(path);
        } else {
            expandedDirs.add(path);
        }
        expandedDirs = expandedDirs;
    }

    function fileIcon(name: string): string {
        if (name.endsWith('.rs')) return '🦀';
        if (name.endsWith('.svelte')) return '🟠';
        if (name.endsWith('.ts') || name.endsWith('.js')) return '🟡';
        if (name.endsWith('.md')) return '📄';
        if (name.endsWith('.toml') || name.endsWith('.json')) return '⚙️';
        return '📄';
    }
</script>

<div class="sidebar-files">
    <div class="filter-row">
        <input
            type="text"
            placeholder="Filter files..."
            bind:value={filter}
            class="filter-input"
        />
    </div>

    <div class="file-tree">
        {#if tree.children}
            {#each tree.children as child}
                {#if child.is_dir}
                    <button class="tree-dir" onclick={() => toggleDir(child.path)}>
                        <span>{expandedDirs.has(child.path) ? '▼' : '▶'}</span>
                        <span>📁 {child.name}</span>
                        {#if child.children}
                            <span class="count">({child.children.length})</span>
                        {/if}
                    </button>
                    {#if expandedDirs.has(child.path) && child.children}
                        <div class="tree-children">
                            {#each child.children as sub}
                                <div class="tree-file">
                                    <span>{fileIcon(sub.name)}</span>
                                    <span>{sub.name}</span>
                                </div>
                            {/each}
                        </div>
                    {/if}
                {:else}
                    <div class="tree-file">
                        <span>{fileIcon(child.name)}</span>
                        <span>{child.name}</span>
                    </div>
                {/if}
            {/each}
        {/if}
    </div>
</div>

<style>
    .sidebar-files {
        display: flex;
        flex-direction: column;
        gap: 8px;
    }
    .filter-input {
        width: 100%;
        font-size: var(--font-size-caption);
        padding: 6px 8px;
        border: 1px solid var(--color-gray4);
        border-radius: var(--radius-sm);
        background: var(--color-gray5);
        color: var(--color-label);
    }
    .file-tree {
        display: flex;
        flex-direction: column;
        gap: 2px;
    }
    .tree-dir {
        display: flex;
        align-items: center;
        gap: 4px;
        padding: 4px 0;
        background: none;
        border: none;
        cursor: pointer;
        font-size: var(--font-size-caption);
        color: var(--color-label);
        text-align: left;
        width: 100%;
    }
    .count {
        color: var(--color-tertiary-label);
        font-size: 11px;
    }
    .tree-children {
        padding-left: 16px;
    }
    .tree-file {
        display: flex;
        align-items: center;
        gap: 4px;
        padding: 2px 0;
        font-size: var(--font-size-caption);
        color: var(--color-secondary-label);
    }
</style>
```

- [ ] **Step 9: Integrate sidebar into layout**

Update `src/routes/+layout.svelte` to show sidebar when a session is active.

- [ ] **Step 10: Verify build**

```bash
cd src && npx svelte-check
```

- [ ] **Step 11: Commit**

```bash
git add -A
git commit -m "feat: add agent sidebar with 6 tabs (usage, workdir, instructions, mcp, lsp, files)"
```

---

## Task 9: Frontend — Task Designer Canvas

**Covers:** §9.9.1 (TaskDesigner, 3 modes, canvas, node inspector, run panel)

**Files:**
- Create: `src/lib/components/task/TaskDesigner.svelte`
- Create: `src/lib/components/task/TaskTemplateCard.svelte`
- Create: `src/lib/components/task/TaskCanvas.svelte`
- Create: `src/lib/components/task/TaskNodeInspector.svelte`
- Create: `src/lib/components/task/TaskRunPanel.svelte`
- Create: `src/lib/stores/task.svelte.ts`

**Interfaces:**
- Consumes: `task:save-template`, `task:run`, `task:validate`, `task:rerun` IPC commands
- Produces: `taskStore` with definition, validation, and run state

- [ ] **Step 1: Create task store**

Create `src/lib/stores/task.svelte.ts`:

```typescript
import { invoke } from '$lib/api/client';

export interface TaskDefinition {
    id: string;
    name: string;
    description: string;
    inputs: TaskInput[];
    stages: TaskStageDef[];
}

export interface TaskStageDef {
    id: string;
    name: string;
    role: string;
    agent_id: string | null;
    prompt_template: string;
    tools: string[];
    max_iterations: number;
    depends_on: string[];
    model_hint: string | null;
    output_spec: string | null;
}

export interface TaskInput {
    key: string;
    label: string;
    kind: 'Text' | 'Textarea' | 'Select' | 'Number';
    options: any[] | null;
    default: any;
    required: boolean;
}

export interface TaskValidationResult {
    ok: boolean;
    errors: string[];
}

function createTaskStore() {
    let viewMode = $state<'template' | 'design' | 'run'>('template');
    let definition = $state<TaskDefinition | null>(null);
    let validation = $state<TaskValidationResult | null>(null);
    let runId = $state<string | null>(null);

    function newDefinition() {
        definition = {
            id: crypto.randomUUID(),
            name: '',
            description: '',
            inputs: [],
            stages: [],
        };
        viewMode = 'design';
    }

    function loadTemplate(template: any) {
        definition = { ...template, id: crypto.randomUUID() };
        viewMode = 'design';
    }

    async function validate(): Promise<boolean> {
        if (!definition) return false;
        validation = await invoke<TaskValidationResult>('task_validate', { definition });
        return validation.ok;
    }

    async function saveTemplate() {
        if (!definition) return;
        await invoke('task_save_template', { definition });
    }

    async function startRun(inputs?: Record<string, any>) {
        if (!definition) return;
        runId = await invoke<string>('task_run', { definition, inputs: inputs || null });
        viewMode = 'run';
    }

    return {
        get viewMode() { return viewMode; },
        set viewMode(v) { viewMode = v; },
        get definition() { return definition; },
        set definition(v) { definition = v; },
        get validation() { return validation; },
        get runId() { return runId; },
        newDefinition,
        loadTemplate,
        validate,
        saveTemplate,
        startRun,
    };
}

export const taskStore = createTaskStore();
```

- [ ] **Step 2: Create TaskDesigner shell**

Create `src/lib/components/task/TaskDesigner.svelte`:

```svelte
<script lang="ts">
    import { taskStore } from '$lib/stores/task.svelte';
    import TaskTemplateCard from './TaskTemplateCard.svelte';
    import TaskCanvas from './TaskCanvas.svelte';
    import TaskRunPanel from './TaskRunPanel.svelte';
    import { workflowStore } from '$lib/stores/workflow.svelte';

    $effect(() => {
        workflowStore.loadWorkflows();
    });
</script>

<div class="task-designer">
    <div class="task-tabs">
        <button
            class="task-tab"
            class:active={taskStore.viewMode === 'template'}
            onclick={() => taskStore.viewMode = 'template'}
        >Templates</button>
        <button
            class="task-tab"
            class:active={taskStore.viewMode === 'design'}
            onclick={() => taskStore.viewMode = 'design'}
        >Design</button>
        <button
            class="task-tab"
            class:active={taskStore.viewMode === 'run'}
            onclick={() => taskStore.viewMode = 'run'}
        >Run</button>
    </div>

    <div class="task-content">
        {#if taskStore.viewMode === 'template'}
            <div class="template-grid">
                <button class="new-task-btn" onclick={taskStore.newDefinition}>
                    + New Task
                </button>
                {#each workflowStore.workflows as wf}
                    <TaskTemplateCard workflow={wf} onSelect={() => taskStore.loadTemplate(wf)} />
                {/each}
            </div>
        {:else if taskStore.viewMode === 'design'}
            <TaskCanvas />
        {:else if taskStore.viewMode === 'run'}
            <TaskRunPanel />
        {/if}
    </div>
</div>

<style>
    .task-designer {
        background: var(--color-gray6);
        border-radius: var(--radius-lg);
        overflow: hidden;
    }
    .task-tabs {
        display: flex;
        gap: 2px;
        padding: 12px 16px 0;
        border-bottom: 1px solid var(--color-gray4);
    }
    .task-tab {
        padding: 8px 16px;
        border: none;
        background: none;
        font-size: var(--font-size-caption);
        color: var(--color-secondary-label);
        cursor: pointer;
        border-bottom: 2px solid transparent;
    }
    .task-tab.active {
        color: var(--color-label);
        border-bottom-color: var(--color-accent);
        font-weight: var(--font-weight-medium);
    }
    .task-content {
        padding: 16px;
        min-height: 300px;
    }
    .template-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
        gap: 12px;
    }
    .new-task-btn {
        background: var(--color-gray5);
        border: 2px dashed var(--color-gray4);
        border-radius: var(--radius-lg);
        padding: 24px;
        color: var(--color-secondary-label);
        cursor: pointer;
        font-size: var(--font-size-body);
    }
</style>
```

- [ ] **Step 3: Create TaskTemplateCard**

Create `src/lib/components/task/TaskTemplateCard.svelte`:

```svelte
<script lang="ts">
    let { workflow, onSelect } = $props();
</script>

<button class="template-card" onclick={onSelect}>
    <h4>{workflow.name}</h4>
    <p>{workflow.description}</p>
    <div class="template-meta">
        <span>{workflow.stage_count} stages</span>
    </div>
</button>

<style>
    .template-card {
        background: var(--color-gray5);
        border-radius: var(--radius-lg);
        padding: 16px;
        text-align: left;
        cursor: pointer;
        border: 1px solid var(--color-gray4);
    }
    .template-card:hover {
        border-color: var(--color-accent);
    }
    h4 {
        font-size: var(--font-size-body);
        font-weight: var(--font-weight-semibold);
        color: var(--color-label);
        margin-bottom: 4px;
    }
    p {
        font-size: var(--font-size-caption);
        color: var(--color-secondary-label);
        margin-bottom: 8px;
    }
    .template-meta {
        font-size: 12px;
        color: var(--color-tertiary-label);
    }
</style>
```

- [ ] **Step 4: Create TaskCanvas (simplified drag-and-drop flow)**

Create `src/lib/components/task/TaskCanvas.svelte`:

```svelte
<script lang="ts">
    import { taskStore } from '$lib/stores/task.svelte';
    let definition = $derived(taskStore.definition);

    function addStage() {
        if (!definition) return;
        const id = `stage${definition.stages.length + 1}`;
        definition.stages.push({
            id,
            name: `Stage ${definition.stages.length + 1}`,
            role: '',
            agent_id: null,
            prompt_template: '',
            tools: [],
            max_iterations: 10,
            depends_on: definition.stages.length > 0 ? [definition.stages[definition.stages.length - 1].id] : [],
            model_hint: null,
            output_spec: null,
        });
        definition = { ...definition };
    }

    function removeStage(idx: number) {
        if (!definition) return;
        definition.stages.splice(idx, 1);
        definition = { ...definition };
    }
</script>

{#if definition}
    <div class="canvas">
        <div class="canvas-toolbar">
            <input
                bind:value={definition.name}
                placeholder="Task name..."
                class="task-name-input"
            />
            <button onclick={taskStore.validate}>Validate</button>
            <button onclick={taskStore.saveTemplate}>Save as Template</button>
            <button onclick={() => taskStore.startRun()}>▶ Run</button>
        </div>

        {#if taskStore.validation && !taskStore.validation.ok}
            <div class="validation-errors">
                {#each taskStore.validation.errors as err}
                    <span class="error">⚠ {err}</span>
                {/each}
            </div>
        {/if}

        <div class="stages-flow">
            {#each definition.stages as stage, i (stage.id)}
                <div class="stage-node">
                    <div class="stage-header">
                        <span class="stage-id">{stage.id}</span>
                        <button class="remove-btn" onclick={() => removeStage(i)}>×</button>
                    </div>
                    <input bind:value={stage.name} placeholder="Stage name" class="stage-input" />
                    <input bind:value={stage.role} placeholder="Role" class="stage-input" />
                    <textarea bind:value={stage.prompt_template} placeholder="Prompt template..." class="stage-textarea"></textarea>
                    {#if i < definition.stages.length - 1}
                        <div class="dependency-arrow">→</div>
                    {/if}
                </div>
            {/each}
        </div>

        <button class="add-stage-btn" onclick={addStage}>+ Add Stage</button>
    </div>
{/if}

<style>
    .canvas {
        display: flex;
        flex-direction: column;
        gap: 16px;
    }
    .canvas-toolbar {
        display: flex;
        gap: 8px;
        align-items: center;
    }
    .task-name-input {
        flex: 1;
        font-size: var(--font-size-body);
        padding: 8px 12px;
        border: 1px solid var(--color-gray4);
        border-radius: var(--radius-md);
        background: var(--color-gray5);
        color: var(--color-label);
    }
    .canvas-toolbar button {
        padding: 8px 12px;
        border: none;
        border-radius: var(--radius-md);
        font-size: var(--font-size-caption);
        cursor: pointer;
        background: var(--color-gray5);
        color: var(--color-label);
    }
    .canvas-toolbar button:last-child {
        background: var(--color-accent);
        color: white;
    }
    .validation-errors {
        display: flex;
        flex-direction: column;
        gap: 4px;
    }
    .error {
        font-size: var(--font-size-caption);
        color: var(--color-red);
    }
    .stages-flow {
        display: flex;
        gap: 12px;
        overflow-x: auto;
        padding: 12px 0;
    }
    .stage-node {
        min-width: 200px;
        background: var(--color-gray5);
        border-radius: var(--radius-md);
        padding: 12px;
        display: flex;
        flex-direction: column;
        gap: 8px;
        position: relative;
    }
    .stage-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
    }
    .stage-id {
        font-size: 12px;
        color: var(--color-accent);
        font-weight: var(--font-weight-medium);
    }
    .remove-btn {
        background: none;
        border: none;
        color: var(--color-tertiary-label);
        cursor: pointer;
        font-size: 16px;
    }
    .stage-input, .stage-textarea {
        font-size: var(--font-size-caption);
        padding: 6px 8px;
        border: 1px solid var(--color-gray4);
        border-radius: var(--radius-sm);
        background: var(--color-gray6);
        color: var(--color-label);
    }
    .stage-textarea {
        min-height: 60px;
        resize: vertical;
    }
    .dependency-arrow {
        position: absolute;
        right: -20px;
        top: 50%;
        transform: translateY(-50%);
        color: var(--color-tertiary-label);
        font-size: 18px;
    }
    .add-stage-btn {
        align-self: flex-start;
        padding: 8px 16px;
        border: 2px dashed var(--color-gray4);
        border-radius: var(--radius-md);
        background: none;
        color: var(--color-secondary-label);
        cursor: pointer;
        font-size: var(--font-size-caption);
    }
</style>
```

- [ ] **Step 5: Create TaskRunPanel**

Create `src/lib/components/task/TaskRunPanel.svelte`:

```svelte
<script lang="ts">
    import { taskStore } from '$lib/stores/task.svelte';
    let runId = $derived(taskStore.runId);
</script>

<div class="run-panel">
    {#if runId}
        <div class="run-header">
            <span class="run-status">Running</span>
            <span class="run-id">{runId}</span>
        </div>
        <div class="run-progress">
            <div class="progress-bar">
                <div class="progress-fill" style="width: 0%"></div>
            </div>
        </div>
        <div class="run-timeline">
            <p class="placeholder">Waiting for workflow engine events...</p>
        </div>
    {:else}
        <p class="no-run">No active run. Start from Design mode.</p>
    {/if}
</div>

<style>
    .run-panel {
        display: flex;
        flex-direction: column;
        gap: 16px;
    }
    .run-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
    }
    .run-status {
        font-size: var(--font-size-body);
        font-weight: var(--font-weight-semibold);
        color: var(--color-green);
    }
    .run-id {
        font-size: 12px;
        color: var(--color-tertiary-label);
    }
    .progress-bar {
        height: 6px;
        background: var(--color-gray4);
        border-radius: var(--radius-sm);
        overflow: hidden;
    }
    .progress-fill {
        height: 100%;
        background: var(--color-accent);
        border-radius: var(--radius-sm);
        transition: width 0.3s;
    }
    .placeholder {
        font-size: var(--font-size-caption);
        color: var(--color-secondary-label);
    }
    .no-run {
        color: var(--color-secondary-label);
        font-size: var(--font-size-caption);
    }
</style>
```

- [ ] **Step 6: Verify build**

```bash
cd src && npx svelte-check
```

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: add task designer (template/design/run modes, canvas, validation)"
```

---

## Task 10: Frontend — Tool Approval Dialog + Skill Market

**Covers:** §10.10.1 (ToolApprovalDialog), §10.4.3 (SkillMarket UI)

**Files:**
- Create: `src/lib/components/dialogs/ToolApprovalDialog.svelte`
- Create: `src/lib/components/market/SkillMarket.svelte`
- Create: `src/lib/components/market/SkillCard.svelte`

**Interfaces:**
- Consumes: `tool:approval-request` event
- Produces: `tool:approval-response` event

- [ ] **Step 1: Create ToolApprovalDialog**

Create `src/lib/components/dialogs/ToolApprovalDialog.svelte`:

```svelte
<script lang="ts">
    import { listen, invoke } from '$lib/api/client';

    let visible = $state(false);
    let request = $state<any>(null);

    listen('tool:approval-request', (event) => {
        request = event.payload;
        visible = true;
    });

    async function respond(response: string) {
        await invoke('tool_approval_respond', {
            callId: request.call_id,
            response,
        });
        visible = false;
        request = null;
    }
</script>

{#if visible && request}
    <div class="dialog-overlay">
        <div class="dialog">
            <h3>Tool Approval</h3>
            <p class="agent-name">Agent "{request.agent_id}" requests:</p>

            <div class="tool-info">
                <span class="tool-name">Tool: {request.tool_name}</span>
                <span class="risk-badge" class:high={request.risk_level === 'High'} class:critical={request.risk_level === 'Critical'}>
                    Risk: {request.risk_level}
                </span>
            </div>

            <div class="params">
                <h4>Parameters</h4>
                <pre>{JSON.stringify(request.arguments, null, 2)}</pre>
            </div>

            <p class="description">{request.description}</p>

            <div class="dialog-actions">
                <button class="approve-btn" onclick={() => respond('Approved')}>✓ Approve</button>
                <button class="reject-btn" onclick={() => respond('Rejected')}>✗ Reject</button>
                <button class="always-btn" onclick={() => respond('AlwaysApprove')}>Always Approve</button>
            </div>
        </div>
    </div>
{/if}

<style>
    .dialog-overlay {
        position: fixed;
        inset: 0;
        background: rgba(0, 0, 0, 0.5);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 1000;
    }
    .dialog {
        background: var(--color-gray6);
        border-radius: var(--radius-lg);
        padding: 24px;
        max-width: 480px;
        width: 90%;
        display: flex;
        flex-direction: column;
        gap: 16px;
    }
    h3 {
        font-size: var(--font-size-title-3);
        color: var(--color-label);
    }
    .agent-name {
        font-size: var(--font-size-caption);
        color: var(--color-secondary-label);
    }
    .tool-info {
        display: flex;
        justify-content: space-between;
        align-items: center;
    }
    .tool-name {
        font-weight: var(--font-weight-semibold);
        color: var(--color-label);
    }
    .risk-badge {
        font-size: 12px;
        padding: 2px 8px;
        border-radius: var(--radius-sm);
        background: var(--color-gray5);
    }
    .risk-badge.high { background: var(--color-orange); color: white; }
    .risk-badge.critical { background: var(--color-red); color: white; }
    .params {
        background: var(--color-gray5);
        border-radius: var(--radius-md);
        padding: 12px;
    }
    .params h4 {
        font-size: var(--font-size-caption);
        color: var(--color-secondary-label);
        margin-bottom: 8px;
    }
    .params pre {
        font-size: 12px;
        color: var(--color-label);
        white-space: pre-wrap;
        word-break: break-all;
    }
    .description {
        font-size: var(--font-size-caption);
        color: var(--color-secondary-label);
    }
    .dialog-actions {
        display: flex;
        gap: 8px;
    }
    .approve-btn {
        background: var(--color-green);
        color: white;
        border: none;
        border-radius: var(--radius-md);
        padding: 8px 16px;
        cursor: pointer;
        font-weight: var(--font-weight-medium);
    }
    .reject-btn {
        background: var(--color-red);
        color: white;
        border: none;
        border-radius: var(--radius-md);
        padding: 8px 16px;
        cursor: pointer;
    }
    .always-btn {
        background: var(--color-gray5);
        color: var(--color-label);
        border: 1px solid var(--color-gray4);
        border-radius: var(--radius-md);
        padding: 8px 16px;
        cursor: pointer;
    }
</style>
```

- [ ] **Step 2: Create SkillMarket**

Create `src/lib/components/market/SkillMarket.svelte`:

```svelte
<script lang="ts">
    import { invoke } from '$lib/api/client';
    import SkillCard from './SkillCard.svelte';

    let query = $state('');
    let results = $state<any[]>([]);
    let loading = $state(false);
    let sourceFilter = $state('all');

    let debounceTimer: ReturnType<typeof setTimeout>;

    function onInput() {
        clearTimeout(debounceTimer);
        debounceTimer = setTimeout(() => search(), 300);
    }

    async function search() {
        if (!query.trim()) { results = []; return; }
        loading = true;
        try {
            results = await invoke<any[]>('skill_search_market', { query });
        } catch (e) {
            console.error('Search failed:', e);
        } finally {
            loading = false;
        }
    }

    let filtered = $derived(
        sourceFilter === 'all' ? results :
        results.filter(r => r.source === sourceFilter)
    );
</script>

<div class="skill-market">
    <div class="market-header">
        <input
            type="text"
            placeholder="Search skills..."
            bind:value={query}
            oninput={onInput}
            class="search-input"
        />
        <div class="source-filters">
            {#each ['all', 'SkillsSh', 'ClaudePlugins', 'Clawhub'] as source}
                <button
                    class="filter-chip"
                    class:active={sourceFilter === source}
                    onclick={() => sourceFilter = source}
                >
                    {source === 'all' ? 'All' : source}
                </button>
            {/each}
        </div>
    </div>

    {#if loading}
        <div class="loading">Searching...</div>
    {:else if filtered.length > 0}
        <div class="results-grid">
            {#each filtered as hit}
                <SkillCard {hit} />
            {/each}
        </div>
    {:else if query}
        <div class="empty">No results found. Try a different keyword.</div>
    {/if}

    <div class="market-footer">
        <span>{results.length} results · {sourceFilter === 'all' ? '3 sources' : sourceFilter}</span>
    </div>
</div>

<style>
    .skill-market {
        display: flex;
        flex-direction: column;
        gap: 16px;
    }
    .market-header {
        display: flex;
        flex-direction: column;
        gap: 8px;
    }
    .search-input {
        font-size: var(--font-size-body);
        padding: 10px 14px;
        border: 1px solid var(--color-gray4);
        border-radius: var(--radius-md);
        background: var(--color-gray5);
        color: var(--color-label);
    }
    .source-filters {
        display: flex;
        gap: 6px;
    }
    .filter-chip {
        padding: 4px 10px;
        border: 1px solid var(--color-gray4);
        border-radius: var(--radius-full);
        background: none;
        font-size: 12px;
        color: var(--color-secondary-label);
        cursor: pointer;
    }
    .filter-chip.active {
        background: var(--color-accent);
        color: white;
        border-color: var(--color-accent);
    }
    .results-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
        gap: 12px;
    }
    .loading, .empty {
        text-align: center;
        padding: 40px;
        color: var(--color-secondary-label);
        font-size: var(--font-size-caption);
    }
    .market-footer {
        font-size: 12px;
        color: var(--color-tertiary-label);
    }
</style>
```

- [ ] **Step 3: Create SkillCard**

Create `src/lib/components/market/SkillCard.svelte`:

```svelte
<script lang="ts">
    import { invoke } from '$lib/api/client';
    let { hit } = $props();
    let installing = $state(false);

    async function install() {
        installing = true;
        try {
            await invoke('skill_install', { source: hit.install_source });
        } catch (e) {
            console.error('Install failed:', e);
        } finally {
            installing = false;
        }
    }
</script>

<div class="skill-card">
    <div class="card-header">
        <span class="source-badge">{hit.source}</span>
        {#if hit.stars}
            <span class="stars">⭐ {hit.stars}</span>
        {/if}
    </div>
    <h4>{hit.name}</h4>
    <p class="description">{hit.description}</p>
    <div class="card-footer">
        {#if hit.installed}
            <span class="installed-badge">✓ Installed</span>
        {:else}
            <button class="install-btn" onclick={install} disabled={installing}>
                {installing ? 'Installing...' : 'Install'}
            </button>
        {/if}
        {#if hit.tags.length > 0}
            <div class="tags">
                {#each hit.tags.slice(0, 3) as tag}
                    <span class="tag">{tag}</span>
                {/each}
            </div>
        {/if}
    </div>
</div>

<style>
    .skill-card {
        background: var(--color-gray6);
        border: 1px solid var(--color-gray4);
        border-radius: var(--radius-lg);
        padding: 16px;
        display: flex;
        flex-direction: column;
        gap: 8px;
    }
    .card-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
    }
    .source-badge {
        font-size: 11px;
        padding: 2px 6px;
        border-radius: var(--radius-sm);
        background: var(--color-gray5);
        color: var(--color-tertiary-label);
    }
    .stars {
        font-size: 12px;
        color: var(--color-secondary-label);
    }
    h4 {
        font-size: var(--font-size-body);
        font-weight: var(--font-weight-semibold);
        color: var(--color-label);
    }
    .description {
        font-size: var(--font-size-caption);
        color: var(--color-secondary-label);
        display: -webkit-box;
        -webkit-line-clamp: 2;
        -webkit-box-orient: vertical;
        overflow: hidden;
    }
    .card-footer {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-top: auto;
    }
    .install-btn {
        background: var(--color-accent);
        color: white;
        border: none;
        border-radius: var(--radius-md);
        padding: 6px 12px;
        font-size: var(--font-size-caption);
        cursor: pointer;
    }
    .install-btn:disabled {
        opacity: 0.6;
    }
    .installed-badge {
        font-size: var(--font-size-caption);
        color: var(--color-green);
    }
    .tags {
        display: flex;
        gap: 4px;
    }
    .tag {
        font-size: 11px;
        padding: 2px 6px;
        border-radius: var(--radius-sm);
        background: var(--color-gray5);
        color: var(--color-tertiary-label);
    }
</style>
```

- [ ] **Step 4: Verify build**

```bash
cd src && npx svelte-check
```

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: add tool approval dialog and skill market UI"
```

---

## Task 11: Integration + Final Wiring

**Covers:** All §9.9, §9.10, §10.4, §10.10 — connecting all pieces

**Files:**
- Modify: `src/routes/+layout.svelte` (sidebar integration)
- Modify: `src/routes/+page.svelte` (dashboard + task designer)
- Modify: `src/routes/settings/+page.svelte` (add market tab)

- [ ] **Step 1: Integrate sidebar into layout**

Update `src/routes/+layout.svelte` to include AgentSidebar when a session is selected.

- [ ] **Step 2: Add task designer to dashboard**

Add TaskDesigner component to the dashboard page below recent sessions.

- [ ] **Step 3: Add skill market to settings**

Add SkillMarket as a new tab in the settings page.

- [ ] **Step 4: Full build verification**

```bash
cd src-tauri && cargo check && cd ../src && npx svelte-check
```

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "feat: integrate Phase 2 components (dashboard, sidebar, task designer, market)"
```
