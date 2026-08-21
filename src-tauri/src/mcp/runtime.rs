use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::transport::{HttpTransport, McpError, McpTool, McpTransport, StdioTransport};
use crate::data::models::McpServerRow;

// ── MCP 状态 ──────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum McpStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

// ── MCP 运行时 ────────────────────────────────────────────

pub struct McpRuntime {
    servers: RwLock<HashMap<String, McpServerState>>,
}

struct McpServerState {
    #[allow(dead_code)]
    config: McpServerRow,
    transport: Option<Box<dyn McpTransport>>,
    status: McpStatus,
    tools: Vec<McpTool>,
}

impl McpRuntime {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            servers: RwLock::new(HashMap::new()),
        })
    }

    /// 注册服务器配置（不立即连接）
    pub async fn register_server(&self, config: McpServerRow) {
        let mut servers = self.servers.write().await;
        servers.insert(
            config.id.clone(),
            McpServerState {
                config,
                transport: None,
                status: McpStatus::Disconnected,
                tools: Vec::new(),
            },
        );
    }

    /// 移除服务器
    pub async fn remove_server(&self, server_id: &str) {
        let mut servers = self.servers.write().await;
        if let Some(mut state) = servers.remove(server_id) {
            if let Some(mut transport) = state.transport.take() {
                let _ = transport.close().await;
            }
        }
    }

    /// 连接到服务器
    pub async fn connect(&self, server_id: &str) -> Result<(), McpError> {
        let mut servers = self.servers.write().await;
        let state = servers
            .get_mut(server_id)
            .ok_or_else(|| McpError::Connection(format!("服务器不存在: {server_id}")))?;

        // 如果已连接则跳过
        if let Some(ref transport) = state.transport {
            if transport.is_connected() {
                return Ok(());
            }
        }

        state.status = McpStatus::Connecting;

        // 创建传输层
        let mut transport: Box<dyn McpTransport> = match state.config.r#type.as_str() {
            "stdio" => {
                let cmd = state.config.command.clone().unwrap_or_default();
                let args: Vec<String> =
                    serde_json::from_str(&state.config.args).unwrap_or_default();
                let env: HashMap<String, String> =
                    serde_json::from_str(&state.config.env).unwrap_or_default();
                Box::new(StdioTransport::new(cmd, args, env))
            }
            "http" | "sse" => {
                let base_url = state.config.base_url.clone().unwrap_or_default();
                let headers: HashMap<String, String> =
                    serde_json::from_str(&state.config.headers).unwrap_or_default();
                let timeout = state.config.timeout_ms.unwrap_or(30000) as u64;
                Box::new(HttpTransport::new(base_url, headers, timeout))
            }
            _ => {
                return Err(McpError::Connection(format!(
                    "不支持的传输类型: {}",
                    state.config.r#type
                )))
            }
        };

        // 初始化
        let client_info = super::transport::ClientInfo {
            name: "Prism Agent".to_string(),
            version: "0.1.0".to_string(),
        };

        match transport.initialize(&client_info).await {
            Ok(()) => {
                // 获取工具列表
                let tools = transport.list_tools().await.unwrap_or_default();
                state.tools = tools;
                state.transport = Some(transport);
                state.status = McpStatus::Connected;
                Ok(())
            }
            Err(e) => {
                state.status = McpStatus::Error(e.to_string());
                Err(e)
            }
        }
    }

    /// 断开连接
    pub async fn disconnect(&self, server_id: &str) -> Result<(), McpError> {
        let mut servers = self.servers.write().await;
        if let Some(state) = servers.get_mut(server_id) {
            if let Some(mut transport) = state.transport.take() {
                transport.close().await?;
            }
            state.status = McpStatus::Disconnected;
            state.tools.clear();
        }
        Ok(())
    }

    /// 调用工具
    pub async fn call_tool(
        &self,
        server_id: &str,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, McpError> {
        let servers = self.servers.read().await;
        let state = servers
            .get(server_id)
            .ok_or_else(|| McpError::Connection(format!("服务器不存在: {server_id}")))?;

        let transport = state.transport.as_ref().ok_or(McpError::NotConnected)?;

        let result = transport.call_tool(tool_name, args).await?;

        // 提取文本内容
        let mut text = String::new();
        for content in &result.content {
            if let super::transport::McpContent::Text { text: t } = content {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(t);
            }
        }

        if result.is_error.unwrap_or(false) {
            Err(McpError::Protocol(text))
        } else {
            Ok(serde_json::json!({ "text": text }))
        }
    }

    /// 获取服务器状态
    pub async fn get_status(&self, server_id: &str) -> Option<McpStatus> {
        let servers = self.servers.read().await;
        servers.get(server_id).map(|s| s.status.clone())
    }

    /// 获取所有服务器状态
    pub async fn all_status(&self) -> Vec<ServerStatusInfo> {
        let servers = self.servers.read().await;
        servers
            .iter()
            .map(|(id, state)| ServerStatusInfo {
                id: id.clone(),
                name: state.config.name.clone(),
                r#type: state.config.r#type.clone(),
                status: state.status.clone(),
                tools_count: state.tools.len() as i32,
                tools: state.tools.iter().map(|t| t.name.clone()).collect(),
            })
            .collect()
    }

    /// 获取服务器工具列表
    pub async fn get_tools(&self, server_id: &str) -> Vec<McpTool> {
        let servers = self.servers.read().await;
        servers
            .get(server_id)
            .map(|s| s.tools.clone())
            .unwrap_or_default()
    }

    /// 判断工具是否可用（任意已连接服务器）
    pub async fn find_tool_server(&self, tool_name: &str) -> Option<String> {
        let servers = self.servers.read().await;
        for (id, state) in servers.iter() {
            if state.status == McpStatus::Connected
                && state.tools.iter().any(|t| t.name == tool_name)
            {
                return Some(id.clone());
            }
        }
        None
    }
}

// ── 服务器状态信息 ────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ServerStatusInfo {
    pub id: String,
    pub name: String,
    pub r#type: String,
    pub status: McpStatus,
    pub tools_count: i32,
    pub tools: Vec<String>,
}

impl Default for McpRuntime {
    fn default() -> Self {
        Self {
            servers: RwLock::new(HashMap::new()),
        }
    }
}
