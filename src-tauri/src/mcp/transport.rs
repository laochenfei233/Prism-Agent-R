use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot, Mutex};

use crate::utils::error::AppError;

// ── MCP Transport Errors ──────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("Connection failed: {0}")]
    Connection(String),
    #[error("Protocol error: {0}")]
    Protocol(String),
    #[error("Timeout after {0}ms")]
    Timeout(u64),
    #[error("IO error: {0}")]
    Io(String),
    #[error("Not connected")]
    NotConnected,
}

impl From<McpError> for AppError {
    fn from(e: McpError) -> Self {
        AppError::McpServer(e.to_string())
    }
}

// ── JSON-RPC Types ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<u64>,
    #[serde(default)]
    pub result: Option<serde_json::Value>,
    #[serde(default)]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

// ── MCP Tool ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpCallResult {
    pub content: Vec<McpContent>,
    pub is_error: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum McpContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { data: String, mime_type: String },
    #[serde(rename = "resource")]
    Resource { uri: String, text: String },
}

// ── Transport Trait ───────────────────────────────────────

#[async_trait]
pub trait McpTransport: Send + Sync {
    async fn initialize(&mut self, client_info: &ClientInfo) -> Result<(), McpError>;
    async fn list_tools(&self) -> Result<Vec<McpTool>, McpError>;
    async fn call_tool(&self, name: &str, args: serde_json::Value) -> Result<McpCallResult, McpError>;
    async fn close(&mut self) -> Result<(), McpError>;
    fn is_connected(&self) -> bool;
}

// ── Stdio Transport ───────────────────────────────────────

pub struct StdioTransport {
    child: Option<Child>,
    stdin_tx: Option<mpsc::Sender<String>>,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<serde_json::Value, McpError>>>>>,
    next_id: Arc<Mutex<u64>>,
    connected: bool,
    command: String,
    args: Vec<String>,
    env: HashMap<String, String>,
}

impl StdioTransport {
    pub fn new(command: String, args: Vec<String>, env: HashMap<String, String>) -> Self {
        Self {
            child: None,
            stdin_tx: None,
            pending: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(Mutex::new(1)),
            connected: false,
            command,
            args,
            env,
        }
    }

    async fn send_request(&self, method: &str, params: Option<serde_json::Value>) -> Result<serde_json::Value, McpError> {
        let id = {
            let mut next = self.next_id.lock().await;
            let id = *next;
            *next += 1;
            id
        };

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        };

        let tx = self.stdin_tx.as_ref().ok_or(McpError::NotConnected)?;
        let (resp_tx, resp_rx) = oneshot::channel();

        // 类型明确化
        let pending: &Arc<Mutex<HashMap<u64, oneshot::Sender<Result<serde_json::Value, McpError>>>>> = &self.pending;
        pending.lock().await.insert(id, resp_tx);

        let msg = serde_json::to_string(&request).map_err(|e| McpError::Protocol(e.to_string()))?;
        tx.send(format!("{msg}\n")).await.map_err(|e| McpError::Io(e.to_string()))?;

        tokio::time::timeout(std::time::Duration::from_secs(30), resp_rx)
            .await
            .map_err(|_| McpError::Timeout(30000))?
            .map_err(|_| McpError::Protocol("Response channel closed".into()))?
    }
}

#[async_trait]
impl McpTransport for StdioTransport {
    async fn initialize(&mut self, client_info: &ClientInfo) -> Result<(), McpError> {
        let mut cmd = Command::new(&self.command);
        cmd.args(&self.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (k, v) in &self.env {
            cmd.env(k, v);
        }

        let mut child = cmd.spawn().map_err(|e| McpError::Connection(format!("Failed to spawn {}: {e}", self.command)))?;

        let stdin = child.stdin.take().ok_or(McpError::Connection("No stdin".into()))?;
        let stdout = child.stdout.take().ok_or(McpError::Connection("No stdout".into()))?;

        let (stdin_tx, mut stdin_rx) = mpsc::channel::<String>(32);
        tokio::spawn(async move {
            let mut stdin = stdin;
            while let Some(msg) = stdin_rx.recv().await {
                if stdin.write_all(msg.as_bytes()).await.is_err() {
                    break;
                }
                let _ = stdin.flush().await;
            }
        });

        // Read stdout in background
        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<serde_json::Value, McpError>>>>> = self.pending.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(resp) = serde_json::from_str::<JsonRpcResponse>(&line) {
                    if let Some(id) = resp.id {
                        if let Some(tx) = pending.lock().await.remove(&id) {
                            let result = if let Some(err) = resp.error {
                                Err(McpError::Protocol(format!("{}: {}", err.code, err.message)))
                            } else {
                                Ok(resp.result.unwrap_or(serde_json::Value::Null))
                            };
                            let _ = tx.send(result);
                        }
                    }
                }
            }
        });

        self.child = Some(child);
        self.stdin_tx = Some(stdin_tx);
        self.connected = true;

        // Send initialize
        let _init_result = self.send_request("initialize", Some(serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": client_info.name,
                "version": client_info.version,
            }
        }))).await?;

        // Send initialized notification
        if let Some(tx) = &self.stdin_tx {
            let notification = serde_json::json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized",
            });
            let msg = serde_json::to_string(&notification).unwrap_or_default();
            let _ = tx.send(format!("{msg}\n")).await;
        }

        Ok(())
    }

    async fn list_tools(&self) -> Result<Vec<McpTool>, McpError> {
        let result = self.send_request("tools/list", None).await?;
        let tools: Vec<McpTool> = serde_json::from_value(
            result.get("tools").cloned().unwrap_or(serde_json::json!([]))
        ).map_err(|e| McpError::Protocol(e.to_string()))?;
        Ok(tools)
    }

    async fn call_tool(&self, name: &str, args: serde_json::Value) -> Result<McpCallResult, McpError> {
        let result = self.send_request("tools/call", Some(serde_json::json!({
            "name": name,
            "arguments": args,
        }))).await?;
        let call_result: McpCallResult = serde_json::from_value(result)
            .map_err(|e| McpError::Protocol(e.to_string()))?;
        Ok(call_result)
    }

    async fn close(&mut self) -> Result<(), McpError> {
        self.connected = false;
        self.stdin_tx = None;
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
        }
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
    }
}

// ── HTTP Transport ────────────────────────────────────────

pub struct HttpTransport {
    client: reqwest::Client,
    base_url: String,
    headers: HashMap<String, String>,
    connected: bool,
    next_id: Mutex<u64>,
}

impl HttpTransport {
    pub fn new(base_url: String, headers: HashMap<String, String>, timeout_ms: u64) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(timeout_ms))
            .build()
            .unwrap_or_default();

        Self {
            client,
            base_url,
            headers,
            connected: false,
            next_id: Mutex::new(1),
        }
    }

    async fn send_request(&self, method: &str, params: Option<serde_json::Value>) -> Result<serde_json::Value, McpError> {
        let id = {
            let mut next = self.next_id.lock().await;
            let id = *next;
            *next += 1;
            id
        };

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        };

        let mut req = self.client.post(&self.base_url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream");

        for (k, v) in &self.headers {
            req = req.header(k, v);
        }

        let resp = req.json(&request).send().await
            .map_err(|e| McpError::Connection(e.to_string()))?;

        let body = resp.text().await.map_err(|e| McpError::Connection(e.to_string()))?;

        // Try parsing as JSON-RPC response
        if let Ok(json_resp) = serde_json::from_str::<JsonRpcResponse>(&body) {
            if let Some(err) = json_resp.error {
                return Err(McpError::Protocol(format!("{}: {}", err.code, err.message)));
            }
            return Ok(json_resp.result.unwrap_or(serde_json::Value::Null));
        }

        // Try parsing as SSE stream
        let mut result = serde_json::Value::Null;
        for line in body.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if let Ok(json_resp) = serde_json::from_str::<JsonRpcResponse>(data) {
                    if let Some(err) = json_resp.error {
                        return Err(McpError::Protocol(format!("{}: {}", err.code, err.message)));
                    }
                    if json_resp.result.is_some() {
                        result = json_resp.result.unwrap();
                    }
                }
            }
        }
        Ok(result)
    }
}

#[async_trait]
impl McpTransport for HttpTransport {
    async fn initialize(&mut self, client_info: &ClientInfo) -> Result<(), McpError> {
        let _ = self.send_request("initialize", Some(serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": client_info.name,
                "version": client_info.version,
            }
        }))).await?;

        // Send initialized notification
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
        });
        let mut req = self.client.post(&self.base_url)
            .header("Content-Type", "application/json");
        for (k, v) in &self.headers {
            req = req.header(k, v);
        }
        let _ = req.json(&notification).send().await;

        self.connected = true;
        Ok(())
    }

    async fn list_tools(&self) -> Result<Vec<McpTool>, McpError> {
        let result = self.send_request("tools/list", None).await?;
        let tools: Vec<McpTool> = serde_json::from_value(
            result.get("tools").cloned().unwrap_or(serde_json::json!([]))
        ).map_err(|e| McpError::Protocol(e.to_string()))?;
        Ok(tools)
    }

    async fn call_tool(&self, name: &str, args: serde_json::Value) -> Result<McpCallResult, McpError> {
        let result = self.send_request("tools/call", Some(serde_json::json!({
            "name": name,
            "arguments": args,
        }))).await?;
        let call_result: McpCallResult = serde_json::from_value(result)
            .map_err(|e| McpError::Protocol(e.to_string()))?;
        Ok(call_result)
    }

    async fn close(&mut self) -> Result<(), McpError> {
        self.connected = false;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
    }
}
