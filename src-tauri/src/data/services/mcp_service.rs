use std::collections::HashMap;
use std::sync::Arc;

use crate::data::models::{McpServerDto, McpServerRow};
use crate::data::Database;
use crate::mcp::McpRuntime;
use crate::utils::error::AppError;

// ── MCP 服务 ──────────────────────────────────────────────

pub struct McpService {
    db: Database,
    runtime: Arc<McpRuntime>,
}

impl McpService {
    pub fn new(db: Database, runtime: Arc<McpRuntime>) -> Self {
        Self { db, runtime }
    }

    /// 列出所有 MCP 服务器
    pub async fn list(&self) -> Result<Vec<McpServerDto>, AppError> {
        let rows = sqlx::query_as::<_, McpServerRow>(
            "SELECT id, name, type, command, args, env, base_url, headers, is_active, timeout_ms, created_at, updated_at FROM mcp_servers ORDER BY created_at"
        )
        .fetch_all(&self.db.pool)
        .await?;

        Ok(rows.into_iter().map(|r| McpServerDto {
            id: r.id,
            name: r.name,
            r#type: r.r#type,
            command: r.command,
            args: serde_json::from_str(&r.args).unwrap_or_default(),
            base_url: r.base_url,
            is_active: r.is_active != 0,
            timeout_ms: r.timeout_ms,
        }).collect())
    }

    /// 添加 MCP 服务器
    pub async fn add(
        &self,
        name: String,
        r#type: String,
        command: Option<String>,
        args: Option<Vec<String>>,
        env: Option<HashMap<String, String>>,
        base_url: Option<String>,
        headers: Option<HashMap<String, String>>,
        timeout_ms: Option<i32>,
    ) -> Result<McpServerDto, AppError> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp_millis();
        let args_json = serde_json::to_string(&args.unwrap_or_default()).unwrap_or_else(|_| "[]".into());
        let env_json = serde_json::to_string(&env.unwrap_or_default()).unwrap_or_else(|_| "{}".into());
        let headers_json = serde_json::to_string(&headers.unwrap_or_default()).unwrap_or_else(|_| "{}".into());

        sqlx::query(
            "INSERT INTO mcp_servers (id, name, type, command, args, env, base_url, headers, is_active, timeout_ms, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?, ?)"
        )
        .bind(&id)
        .bind(&name)
        .bind(&r#type)
        .bind(&command)
        .bind(&args_json)
        .bind(&env_json)
        .bind(&base_url)
        .bind(&headers_json)
        .bind(&timeout_ms.unwrap_or(30000))
        .bind(now)
        .bind(now)
        .execute(&self.db.pool)
        .await?;

        // 注册到运行时
        let row = McpServerRow {
            id: id.clone(),
            name: name.clone(),
            r#type: r#type.clone(),
            command: command.clone(),
            args: args_json.clone(),
            env: env_json,
            base_url: base_url.clone(),
            headers: headers_json,
            is_active: 1,
            timeout_ms: Some(timeout_ms.unwrap_or(30000)),
            created_at: now,
            updated_at: now,
        };
        self.runtime.register_server(row.clone()).await;

        Ok(McpServerDto {
            id,
            name: row.name,
            r#type: row.r#type,
            command: row.command,
            args: serde_json::from_str(&args_json).unwrap_or_default(),
            base_url: row.base_url,
            is_active: true,
            timeout_ms: row.timeout_ms,
        })
    }

    /// 更新 MCP 服务器
    pub async fn update(
        &self,
        id: &str,
        name: Option<String>,
        r#type: Option<String>,
        command: Option<String>,
        args: Option<Vec<String>>,
        base_url: Option<String>,
        timeout_ms: Option<i32>,
    ) -> Result<McpServerDto, AppError> {
        let existing = self.get_row(id).await?;

        let new_name = name.unwrap_or(existing.name);
        let new_type = r#type.unwrap_or(existing.r#type);
        let new_command = command.or(existing.command);
        let new_args = args.map(|a| serde_json::to_string(&a).unwrap_or_else(|_| "[]".into())).unwrap_or(existing.args);
        let new_base_url = base_url.or(existing.base_url);
        let new_timeout = timeout_ms.unwrap_or(existing.timeout_ms.unwrap_or(30000));
        let now = chrono::Utc::now().timestamp_millis();

        sqlx::query(
            "UPDATE mcp_servers SET name = ?, type = ?, command = ?, args = ?, base_url = ?, timeout_ms = ?, updated_at = ? WHERE id = ?"
        )
        .bind(&new_name)
        .bind(&new_type)
        .bind(&new_command)
        .bind(&new_args)
        .bind(&new_base_url)
        .bind(&new_timeout)
        .bind(now)
        .bind(id)
        .execute(&self.db.pool)
        .await?;

        // 重新注册
        self.runtime.remove_server(id).await;
        let row = McpServerRow {
            id: id.to_string(),
            name: new_name.clone(),
            r#type: new_type.clone(),
            command: new_command.clone(),
            args: new_args.clone(),
            env: existing.env,
            base_url: new_base_url.clone(),
            headers: existing.headers,
            is_active: 1,
            timeout_ms: Some(new_timeout),
            created_at: existing.created_at,
            updated_at: now,
        };
        self.runtime.register_server(row).await;

        Ok(McpServerDto {
            id: id.to_string(),
            name: new_name,
            r#type: new_type,
            command: new_command,
            args: serde_json::from_str(&new_args).unwrap_or_default(),
            base_url: new_base_url,
            is_active: true,
            timeout_ms: Some(new_timeout),
        })
    }

    /// 移除 MCP 服务器
    pub async fn remove(&self, id: &str) -> Result<(), AppError> {
        self.runtime.remove_server(id).await;
        sqlx::query("DELETE FROM mcp_servers WHERE id = ?")
            .bind(id)
            .execute(&self.db.pool)
            .await?;
        Ok(())
    }

    /// 测试连接
    pub async fn test(&self, id: &str) -> Result<McpTestResult, AppError> {
        let row = self.get_row(id).await?;

        // 注册并连接
        self.runtime.remove_server(id).await;
        self.runtime.register_server(row.clone()).await;

        let start = std::time::Instant::now();
        match self.runtime.connect(id).await {
            Ok(()) => {
                let tools = self.runtime.get_tools(id).await;
                let elapsed = start.elapsed().as_millis() as u64;
                Ok(McpTestResult {
                    ok: true,
                    tools_count: tools.len() as i32,
                    latency_ms: Some(elapsed),
                    error: None,
                })
            }
            Err(e) => {
                Ok(McpTestResult {
                    ok: false,
                    tools_count: 0,
                    latency_ms: None,
                    error: Some(e.to_string()),
                })
            }
        }
    }

    /// 获取服务器工具列表
    pub async fn tools(&self, server_id: Option<&str>) -> Result<Vec<crate::mcp::transport::McpTool>, AppError> {
        if let Some(sid) = server_id {
            Ok(self.runtime.get_tools(sid).await)
        } else {
            // 合并所有已连接服务器的工具
            let mut all_tools = Vec::new();
            let servers = self.runtime.all_status().await;
            for s in &servers {
                if let crate::mcp::runtime::McpStatus::Connected = s.status {
                    all_tools.extend(self.runtime.get_tools(&s.id).await);
                }
            }
            Ok(all_tools)
        }
    }

    /// 获取所有服务器状态
    pub async fn all_status(&self) -> Vec<crate::mcp::runtime::ServerStatusInfo> {
        self.runtime.all_status().await
    }

    /// 启动时加载所有服务器配置，并尽力连接 active 服务器
    pub async fn load_all(&self) -> Result<(), AppError> {
        let rows = sqlx::query_as::<_, McpServerRow>(
            "SELECT id, name, type, command, args, env, base_url, headers, is_active, timeout_ms, created_at, updated_at FROM mcp_servers WHERE is_active = 1"
        )
        .fetch_all(&self.db.pool)
        .await?;

        for row in rows {
            let server_id = row.id.clone();
            self.runtime.register_server(row).await;
            // 连接失败不阻塞整体加载；未连接的工具会在调用时按需重试
            if let Err(e) = self.runtime.connect(&server_id).await {
                tracing::warn!("MCP 服务器 {server_id} 连接失败: {e}");
            }
        }
        Ok(())
    }

    async fn get_row(&self, id: &str) -> Result<McpServerRow, AppError> {
        sqlx::query_as::<_, McpServerRow>(
            "SELECT id, name, type, command, args, env, base_url, headers, is_active, timeout_ms, created_at, updated_at FROM mcp_servers WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.db.pool)
        .await?
        .ok_or_else(|| AppError::McpServer(format!("MCP 服务器不存在: {id}")))
    }
}

// ── 测试结果 ──────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct McpTestResult {
    pub ok: bool,
    pub tools_count: i32,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}
