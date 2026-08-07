use tauri::State;

use crate::core::adk::router::{RouteItem, RouteKind, RouteResult, ToolRouter};
use crate::utils::error::AppError;

/// 从数据库构建路由索引：技能（skills 表）+ MCP 工具（runtime 目录）
async fn build_index(state: &State<'_, crate::AppState>) -> Vec<RouteItem> {
    let mut items = Vec::new();

    // 技能索引
    let skills = sqlx::query_as::<_, crate::data::models::SkillRow>(
        "SELECT id, name, description, folder_name, source, source_url, namespace, author, tags, content_hash, is_enabled, created_at, updated_at FROM skills WHERE is_enabled = 1"
    )
    .fetch_all(&state.db.pool)
    .await
    .unwrap_or_default();

    for s in skills {
        let keywords: Vec<String> = s.tags
            .split(|c: char| c == ',' || c == ' ')
            .filter(|t| !t.is_empty())
            .map(String::from)
            .collect();
        items.push(RouteItem {
            id: s.id,
            kind: RouteKind::Skill,
            name: s.name,
            description: s.description.unwrap_or_default(),
            keywords,
            server_id: None,
        });
    }

    // MCP 工具索引（从 runtime 缓存的工具目录）
    for server in state.mcp_runtime.all_status().await {
        let tools = state.mcp_runtime.get_tools(&server.id).await;
        for tool in tools {
            items.push(RouteItem {
                id: format!("{}::{}", server.id, tool.name),
                kind: RouteKind::McpTool,
                name: tool.name,
                description: tool.description,
                keywords: Vec::new(),
                server_id: Some(server.id.clone()),
            });
        }
    }

    items
}

#[tauri::command]
pub async fn router_route(
    state: State<'_, crate::AppState>,
    query: String,
    top_k: Option<usize>,
) -> Result<RouteResult, AppError> {
    let top_k = top_k.unwrap_or(8);
    let items = build_index(&state).await;
    let mut router = ToolRouter::new();
    router.refresh(items);
    Ok(router.route(&query, 3, top_k))
}

#[tauri::command]
pub async fn router_index_status(
    state: State<'_, crate::AppState>,
) -> Result<serde_json::Value, AppError> {
    let items = build_index(&state).await;
    let skills = items.iter().filter(|i| i.kind == RouteKind::Skill).count();
    let mcp_tools = items.iter().filter(|i| i.kind == RouteKind::McpTool).count();
    Ok(serde_json::json!({
        "skills": skills,
        "mcp_tools": mcp_tools,
        "updated_at": chrono::Utc::now().timestamp(),
    }))
}
