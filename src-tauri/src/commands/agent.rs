use std::path::Path;
use sqlx::Row;
use tauri::State;

use crate::data::models::*;
use crate::data::services::AgentService;
use crate::utils::error::AppError;

#[tauri::command]
pub async fn agent_list(state: State<'_, crate::AppState>) -> Result<Vec<AgentDto>, AppError> {
    let svc = AgentService::new(state.db.pool.clone());
    svc.list().await
}

#[tauri::command]
pub async fn agent_get(state: State<'_, crate::AppState>, id: String) -> Result<AgentDto, AppError> {
    let svc = AgentService::new(state.db.pool.clone());
    svc.get(&id).await
}

#[tauri::command]
pub async fn agent_create(
    state: State<'_, crate::AppState>,
    name: String,
    description: Option<String>,
    system_prompt: Option<String>,
    model_id: Option<String>,
) -> Result<AgentDto, AppError> {
    let svc = AgentService::new(state.db.pool.clone());
    svc.create(
        &name,
        description.as_deref(),
        system_prompt.as_deref(),
        model_id.as_deref(),
    ).await
}

#[tauri::command]
pub async fn agent_update(
    state: State<'_, crate::AppState>,
    id: String,
    name: Option<String>,
    description: Option<String>,
    system_prompt: Option<String>,
    model_id: Option<String>,
) -> Result<AgentDto, AppError> {
    let svc = AgentService::new(state.db.pool.clone());
    svc.update(
        &id,
        name.as_deref(),
        description.as_deref(),
        system_prompt.as_deref(),
        model_id.as_deref(),
    ).await
}

#[tauri::command]
pub async fn agent_delete(state: State<'_, crate::AppState>, id: String) -> Result<(), AppError> {
    let svc = AgentService::new(state.db.pool.clone());
    svc.delete(&id).await
}

// ── Agent Context Sidebar ─────────────────────────────────

#[tauri::command]
pub async fn context_agent(
    state: State<'_, crate::AppState>,
    agent_id: String,
    session_id: Option<String>,
) -> Result<AgentContext, AppError> {
    let svc = AgentService::new(state.db.pool.clone());
    let agent = svc.get(&agent_id).await?;

    let session_usage = load_session_usage(&state, session_id.as_deref()).await;
    let workspace = load_workspace(&state).await;
    let instructions = scan_instructions(&workspace.current_dir);
    let mcp = load_mcp_status(&state).await?;
    let lsp = detect_lsp_servers(&workspace.current_dir);
    let tree = load_dir_tree(&workspace.current_dir, 1)
        .unwrap_or_else(|_| DirTree {
            name: ".".into(),
            path: workspace.current_dir.clone(),
            is_dir: true,
            children: Some(Vec::new()),
            language: None,
            line_count: None,
        });

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

#[tauri::command]
pub async fn session_inject_file(
    state: State<'_, crate::AppState>,
    session_id: String,
    path: String,
) -> Result<(), AppError> {
    // Verify session exists
    sqlx::query("SELECT id FROM sessions WHERE id = ?")
        .bind(&session_id)
        .fetch_optional(&state.db.pool)
        .await?
        .ok_or_else(|| AppError::SessionNotFound(session_id.clone()))?;

    // Store injected file path in agent configuration via a simple JSON update
    // We append to a JSON array in the session's related agent config
    let existing: Option<String> = sqlx::query_scalar(
        "SELECT configuration FROM agents WHERE id = (SELECT agent_id FROM sessions WHERE id = ?)"
    )
    .bind(&session_id)
    .fetch_optional(&state.db.pool)
    .await?;

    let mut config: serde_json::Value = existing
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::json!({}));

    let injected = config
        .get("injected_files")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut new_injected: Vec<String> = injected
        .into_iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();

    if !new_injected.contains(&path) {
        new_injected.push(path);
    }

    config["injected_files"] = serde_json::json!(new_injected);

    sqlx::query(
        "UPDATE agents SET configuration = ?, updated_at = ? WHERE id = (SELECT agent_id FROM sessions WHERE id = ?)"
    )
    .bind(serde_json::to_string(&config).unwrap_or_default())
    .bind(chrono::Utc::now().timestamp_millis())
    .bind(&session_id)
    .execute(&state.db.pool)
    .await?;

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────

async fn load_session_usage(
    state: &State<'_, crate::AppState>,
    session_id: Option<&str>,
) -> SessionUsage {
    let base_where = match session_id {
        Some(sid) => format!("session_id = '{sid}'"),
        None => "1=1".to_string(),
    };

    let query = format!(
        r#"
        SELECT
            COALESCE(SUM(
                CAST(json_extract(usage, '$.prompt_tokens') AS INTEGER)
            ), 0) AS input_tokens,
            COALESCE(SUM(
                CAST(json_extract(usage, '$.completion_tokens') AS INTEGER)
            ), 0) AS output_tokens,
            COALESCE(SUM(
                CAST(json_extract(usage, '$.prompt_tokens') AS INTEGER)
                + CAST(json_extract(usage, '$.completion_tokens') AS INTEGER)
            ), 0) AS context_used,
            0 AS context_limit,
            COALESCE(SUM(CASE WHEN tool_calls IS NOT NULL THEN 1 ELSE 0 END), 0) AS tool_calls,
            COALESCE(SUM(
                CAST(COALESCE(json_extract(usage, '$.cost'), 0) AS REAL)
            ), 0.0) AS cost_est,
            0 AS today_calls,
            0 AS today_tokens,
            0.0 AS today_cost
        FROM messages
        WHERE usage IS NOT NULL AND {base_where}
        "#
    );

    let result = sqlx::query(&query)
        .fetch_one(&state.db.pool)
        .await;

    match result {
        Ok(row) => SessionUsage {
            input_tokens: row.get::<i64, _>("input_tokens") as u64,
            output_tokens: row.get::<i64, _>("output_tokens") as u64,
            context_used: row.get::<i64, _>("context_used") as u64,
            context_limit: row.get::<i64, _>("context_limit") as u64,
            tool_calls: row.get::<i64, _>("tool_calls") as u64,
            cost_est: row.get::<f64, _>("cost_est"),
            today_calls: 0,
            today_tokens: 0,
            today_cost: 0.0,
        },
        Err(_) => SessionUsage {
            input_tokens: 0,
            output_tokens: 0,
            context_used: 0,
            context_limit: 0,
            tool_calls: 0,
            cost_est: 0.0,
            today_calls: 0,
            today_tokens: 0,
            today_cost: 0.0,
        },
    }
}

async fn load_workspace(state: &State<'_, crate::AppState>) -> WorkspaceInfo {
    // 优先使用 preferences 中保存的工作目录（workspace_set 写入），保持侧边栏一致
    if let Some(info) = crate::commands::workspace::load_workspace_pref(&state.db.pool).await {
        return info;
    }

    // 回退：读取第一个 agent 的配置 workdir
    let result: Option<String> = sqlx::query_scalar(
        "SELECT configuration FROM agents ORDER BY order_key LIMIT 1"
    )
    .fetch_optional(&state.db.pool)
    .await
    .ok()
    .flatten();

    let current_dir = result
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get("workdir").and_then(|w| w.as_str()).map(String::from))
        .unwrap_or_else(|| std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".into()));

    WorkspaceInfo {
        current_dir,
        recent_dirs: Vec::new(),
        bound_agent_id: None,
    }
}

async fn load_mcp_status(state: &State<'_, crate::AppState>) -> Result<Vec<McpServerStatus>, AppError> {
    let rows = sqlx::query_as::<_, McpServerRow>(
        "SELECT id, name, type, command, args, env, base_url, headers, is_active, timeout_ms, created_at, updated_at FROM mcp_servers ORDER BY created_at",
    )
    .fetch_all(&state.db.pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| McpServerStatus {
            id: r.id,
            name: r.name,
            status: if r.is_active != 0 { "active".into() } else { "inactive".into() },
            tools_count: 0,
            last_error: None,
        })
        .collect())
}

fn detect_lsp_servers(workdir: &str) -> Vec<LspServerInfo> {
    let mut servers = Vec::new();
    let dir = Path::new(workdir);

    if dir.join("Cargo.toml").exists() {
        servers.push(LspServerInfo {
            id: "rust-analyzer".into(),
            cmd: "rust-analyzer".into(),
            status: "detected".into(),
            langs: vec!["rust".into()],
            index_file_count: None,
            last_error: None,
            install_hint: Some("rustup component add rust-analyzer".into()),
        });
    }

    if dir.join("package.json").exists() {
        servers.push(LspServerInfo {
            id: "typescript-language-server".into(),
            cmd: "typescript-language-server".into(),
            status: "detected".into(),
            langs: vec!["typescript".into(), "javascript".into()],
            index_file_count: None,
            last_error: None,
            install_hint: Some("npm install -g typescript-language-server".into()),
        });
    }

    if dir.join("pyproject.toml").exists() || dir.join("requirements.txt").exists() {
        servers.push(LspServerInfo {
            id: "pyright".into(),
            cmd: "pyright-langserver".into(),
            status: "detected".into(),
            langs: vec!["python".into()],
            index_file_count: None,
            last_error: None,
            install_hint: Some("npm install -g pyright".into()),
        });
    }

    if dir.join("go.mod").exists() {
        servers.push(LspServerInfo {
            id: "gopls".into(),
            cmd: "gopls".into(),
            status: "detected".into(),
            langs: vec!["go".into()],
            index_file_count: None,
            last_error: None,
            install_hint: Some("go install golang.org/x/tools/gopls@latest".into()),
        });
    }

    servers
}

fn scan_instructions(workdir: &str) -> Vec<InstructionFile> {
    let dir = Path::new(workdir);
    let candidates = [
        ("CLAUDE.md", 10u8),
        ("AGENTS.md", 9),
        (".cursor/rules", 8),
        (".prism/memory.md", 7),
        ("README.md", 5),
        ("CONTRIBUTING.md", 4),
        (".github/copilot-instructions.md", 6),
    ];

    candidates
        .iter()
        .filter_map(|(rel, priority)| {
            let path = dir.join(rel);
            let metadata = std::fs::metadata(&path).ok()?;
            if !metadata.is_file() {
                return None;
            }
            let content = std::fs::read_to_string(&path).ok()?;
            let lines = content.lines().count();
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| rel.to_string());

            Some(InstructionFile {
                path: path.display().to_string(),
                name,
                lines,
                injected: false,
                priority: *priority,
            })
        })
        .collect()
}

fn load_dir_tree(workdir: &str, depth: u8) -> Result<DirTree, AppError> {
    let dir = Path::new(workdir);
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".into());

    if depth == 0 {
        return Ok(DirTree {
            name,
            path: dir.display().to_string(),
            is_dir: true,
            children: None,
            language: None,
            line_count: None,
        });
    }

    let entries = std::fs::read_dir(dir).map_err(|e| AppError::Internal(format!("Failed to read dir: {e}")))?;

    let ignore = [".git", "node_modules", "target", ".next", "dist", "build", "__pycache__", ".venv"];

    let mut children: Vec<DirTree> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            !ignore.contains(&name.as_str()) && !name.starts_with('.')
        })
        .filter_map(|entry| {
            let path = entry.path();
            let meta = entry.metadata().ok()?;
            let is_dir = meta.is_dir();
            let child_name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            let (language, line_count) = if is_dir {
                (None, None)
            } else {
                let lang = detect_language(&child_name);
                let lines = std::fs::read_to_string(&path)
                    .ok()
                    .map(|c| c.lines().count() as u64);
                (lang, lines)
            };

            let child = if is_dir {
                load_dir_tree(&path.display().to_string(), depth - 1).ok()?
            } else {
                DirTree {
                    name: child_name,
                    path: path.display().to_string(),
                    is_dir: false,
                    children: None,
                    language,
                    line_count,
                }
            };

            Some(child)
        })
        .collect();

    children.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));

    Ok(DirTree {
        name,
        path: dir.display().to_string(),
        is_dir: true,
        children: Some(children),
        language: None,
        line_count: None,
    })
}

fn detect_language(filename: &str) -> Option<String> {
    let ext = Path::new(filename).extension()?.to_str()?;
    match ext {
        "rs" => Some("rust".into()),
        "ts" | "tsx" => Some("typescript".into()),
        "js" | "jsx" => Some("javascript".into()),
        "py" => Some("python".into()),
        "go" => Some("go".into()),
        "java" => Some("java".into()),
        "cpp" | "cc" | "cxx" | "h" | "hpp" => Some("c++".into()),
        "c" => Some("c".into()),
        "rb" => Some("ruby".into()),
        "swift" => Some("swift".into()),
        "kt" | "kts" => Some("kotlin".into()),
        "vue" => Some("vue".into()),
        "svelte" => Some("svelte".into()),
        "css" | "scss" | "less" => Some("css".into()),
        "html" | "htm" => Some("html".into()),
        "json" => Some("json".into()),
        "yaml" | "yml" => Some("yaml".into()),
        "toml" => Some("toml".into()),
        "md" => Some("markdown".into()),
        "sql" => Some("sql".into()),
        "sh" | "bash" => Some("shell".into()),
        "dockerfile" => Some("dockerfile".into()),
        _ => None,
    }
}
