// Agent 能力检测测试套件（spec: docs/compose/specs/agent-capability-tests.md）
//
// 集中验证三大核心能力，零外部依赖（不调真实 LLM/网络/MCP）：
//   1. 对话 —— FakeProvider 驱动的 RigAgent 完整 agentic loop
//   2. 运行 —— 工具注册 → 执行 → 结果回填
//   3. 本地读写 —— commands::file 真实文件系统往返 + 沙箱黑白名单
// 另有端到端闭环：FakeProvider 驱动 RigAgent 真实调用文件工具（写→读→总结）。
// 随 cargo test 自动进入 CI test 门槛。
//
// 集成测试形态（src-tauri/tests/）：调用 RigAgent::run 会拉入 tauri 事件链
// （comctl32 v6 符号），Windows 上由 build.rs 注入 comctl32 v6 manifest 保证
// 测试 exe 可启动（详见 spec §Windows 测试 manifest）。

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use prism_agent_lib::core::adk::error::AgentError;
use prism_agent_lib::core::adk::model::{
    ChatMessage, ChatRole, GenerationRequest, GenerationResponse, MessageContent,
    ModelCapabilities, ModelProvider, StreamEvent, StreamHandle, ToolCall, ToolOutput, Usage,
};
use prism_agent_lib::core::adk::tool::{ToolApprovalStore, ToolExecutor, ToolRegistry};
use prism_agent_lib::core::guardrails::sandbox::SandboxPolicy;
use prism_agent_lib::core::rig::agent::RigAgent;
use prism_agent_lib::data::services::trace_service::AgentTrace;

// ── 测试基座 ──────────────────────────────────────────────

/// 按预置响应队列逐轮返回的假 Provider（stream 委托 generate，确定性驱动 agentic loop）。
struct FakeProvider {
    responses: Arc<tokio::sync::Mutex<VecDeque<GenerationResponse>>>,
}

impl FakeProvider {
    fn new(responses: Vec<GenerationResponse>) -> Self {
        Self {
            responses: Arc::new(tokio::sync::Mutex::new(responses.into())),
        }
    }
}

#[async_trait]
impl ModelProvider for FakeProvider {
    fn id(&self) -> &str {
        "fake"
    }

    fn display_name(&self) -> &str {
        "Fake Provider"
    }

    fn capabilities(&self) -> ModelCapabilities {
        ModelCapabilities {
            max_tokens: 4096,
            supports_tools: true,
            supports_streaming: true,
            supports_vision: false,
        }
    }

    async fn generate(&self, _req: GenerationRequest) -> Result<GenerationResponse, AgentError> {
        self.responses
            .lock()
            .await
            .pop_front()
            .ok_or_else(|| AgentError::Provider("fake responses exhausted".into()))
    }

    async fn stream(&self, req: GenerationRequest) -> Result<StreamHandle, AgentError> {
        let resp = self.generate(req).await?;
        let mut events = Vec::new();
        for call in resp.tool_calls {
            events.push(StreamEvent::ToolCall(call));
        }
        if !resp.text.is_empty() {
            events.push(StreamEvent::Text(resp.text));
        }
        events.push(StreamEvent::Finish { usage: resp.usage });
        Ok(Box::pin(futures::stream::iter(events)))
    }
}

/// 低风险工具执行器：原样回显 text 参数，并记录调用次数。
struct EchoTool {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl ToolExecutor for EchoTool {
    fn name(&self) -> &str {
        "echo_tool"
    }

    fn description(&self) -> &str {
        "Echo back the text argument"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": { "text": { "type": "string" } },
            "required": ["text"],
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolOutput, AgentError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let text = args
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        Ok(ToolOutput::text(format!("echo: {text}")))
    }
}

/// 包装真实文件命令的工具执行器（write_file=Medium / read_file=Low，均无需审批）。
enum FileToolKind {
    Write,
    Read,
}

struct FileTools(FileToolKind);

impl FileTools {
    fn write() -> Self {
        Self(FileToolKind::Write)
    }

    fn read() -> Self {
        Self(FileToolKind::Read)
    }
}

#[async_trait]
impl ToolExecutor for FileTools {
    fn name(&self) -> &str {
        match self.0 {
            FileToolKind::Write => "write_file",
            FileToolKind::Read => "read_file",
        }
    }

    fn description(&self) -> &str {
        match self.0 {
            FileToolKind::Write => "Write content to a file at the given path",
            FileToolKind::Read => "Read text content from the file at the given path",
        }
    }

    fn schema(&self) -> serde_json::Value {
        match self.0 {
            FileToolKind::Write => serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "content": { "type": "string" },
                },
                "required": ["path", "content"],
            }),
            FileToolKind::Read => serde_json::json!({
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"],
            }),
        }
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolOutput, AgentError> {
        match self.0 {
            FileToolKind::Write => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AgentError::InvalidArgs("path required".into()))?;
                let content = args
                    .get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AgentError::InvalidArgs("content required".into()))?;
                prism_agent_lib::commands::file::file_write(path.to_string(), content.to_string())
                    .await
                    .map_err(|e| AgentError::Tool(e.to_string()))?;
                Ok(ToolOutput::text(format!("wrote {path}")))
            }
            FileToolKind::Read => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AgentError::InvalidArgs("path required".into()))?;
                let text = prism_agent_lib::commands::file::file_read_text(path.to_string())
                    .await
                    .map_err(|e| AgentError::Tool(e.to_string()))?;
                Ok(ToolOutput::text(text))
            }
        }
    }
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("prism_cap_test_{tag}_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn user_message(text: &str) -> ChatMessage {
    ChatMessage {
        role: ChatRole::User,
        content: MessageContent::Text(text.to_string()),
        name: None,
    }
}

fn plain_request() -> GenerationRequest {
    GenerationRequest {
        messages: vec![user_message("ping")],
        ..Default::default()
    }
}

// ── 1. 对话能力 ───────────────────────────────────────────

#[tokio::test]
async fn conversation_roundtrip() {
    let provider = Arc::new(FakeProvider::new(vec![GenerationResponse {
        text: "你好，这是对话回复".into(),
        tool_calls: vec![],
        usage: Some(Usage {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
        }),
    }]));

    let agent = RigAgent::new(provider, "system".into(), ToolRegistry::new());
    let result = agent
        .run(plain_request())
        .await
        .expect("conversation should succeed");

    assert_eq!(result.text, "你好，这是对话回复");
    assert!(result.tool_calls.is_empty());
    let usage = result.usage.expect("usage should be present");
    assert_eq!(usage.total_tokens, 15);
}

// ── 2. 运行能力（工具执行闭环） ───────────────────────────

#[tokio::test]
async fn tool_execution_roundtrip() {
    let echo_calls = Arc::new(AtomicUsize::new(0));
    let provider = Arc::new(FakeProvider::new(vec![
        GenerationResponse {
            text: String::new(),
            tool_calls: vec![ToolCall {
                id: "call-1".into(),
                name: "echo_tool".into(),
                arguments: serde_json::json!({ "text": "hello" }),
            }],
            usage: None,
        },
        GenerationResponse {
            text: "工具结果 echo: hello 已收到".into(),
            tool_calls: vec![],
            usage: None,
        },
    ]));

    let mut registry = ToolRegistry::new();
    registry.register(Box::new(EchoTool {
        calls: echo_calls.clone(),
    }));

    // echo_tool 风险为 High：预置 always-approve 以走真实 HITL 门控的直通分支
    let store = Arc::new(ToolApprovalStore::new());
    store.add_always_approve("echo_tool").await;

    let traces: Arc<Mutex<Vec<AgentTrace>>> = Arc::new(Mutex::new(Vec::new()));
    let traces_cb = traces.clone();
    let agent = RigAgent::new(provider, "system".into(), registry)
        .with_approval_store(store)
        .with_on_trace(move |trace| {
            traces_cb.lock().unwrap().push(trace);
        });

    let result = agent
        .run(plain_request())
        .await
        .expect("tool loop should succeed");

    assert_eq!(result.text, "工具结果 echo: hello 已收到");
    assert_eq!(
        echo_calls.load(Ordering::SeqCst),
        1,
        "tool must be executed exactly once"
    );
    let trace_guard = traces.lock().unwrap();
    let kinds: Vec<&str> = trace_guard
        .first()
        .expect("trace recorded")
        .steps
        .iter()
        .map(|s| s.kind.as_str())
        .collect();
    assert!(kinds.contains(&"llm_call"));
    assert!(kinds.contains(&"tool_call"));
}

// ── 3. 本地读写能力 ───────────────────────────────────────

#[tokio::test]
async fn local_fs_read_write() {
    let dir = temp_dir("rw");
    let txt = dir.join("note.txt");
    let txt_path = txt.to_string_lossy().to_string();

    prism_agent_lib::commands::file::file_write(txt_path.clone(), "hello world".into())
        .await
        .expect("write should succeed");
    assert_eq!(
        prism_agent_lib::commands::file::file_read_text(txt_path.clone())
            .await
            .unwrap(),
        "hello world"
    );

    let parsed = prism_agent_lib::commands::file::file_parse(txt_path.clone())
        .await
        .unwrap();
    assert_eq!(parsed.kind, "text");
    assert_eq!(parsed.content.as_deref(), Some("hello world"));

    let json = dir.join("data.json");
    let json_path = json.to_string_lossy().to_string();
    prism_agent_lib::commands::file::file_write(json_path.clone(), r#"{"a":1}"#.into())
        .await
        .expect("write json should succeed");
    let parsed_json = prism_agent_lib::commands::file::file_parse(json_path)
        .await
        .unwrap();
    assert_eq!(parsed_json.kind, "json");
    assert!(parsed_json.json.is_some());

    let listed =
        prism_agent_lib::commands::file::file_list(dir.to_string_lossy().to_string(), Some(2))
            .await
            .unwrap();
    assert!(listed.iter().any(|e| e.name == "note.txt"));

    assert_eq!(
        prism_agent_lib::commands::file::file_pick(Some(txt_path.clone()))
            .await
            .unwrap(),
        txt_path
    );
    let missing = dir.join("missing.txt").to_string_lossy().to_string();
    assert!(prism_agent_lib::commands::file::file_pick(Some(missing))
        .await
        .is_err());

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn local_fs_truncation() {
    let dir = temp_dir("trunc");
    let big = dir.join("big.log");
    let big_path = big.to_string_lossy().to_string();

    // ~660KB > 200KB 上限，触发「仅前 100 行」截断
    let mut content = String::new();
    for i in 0..30_000 {
        content.push_str(&format!("0123456789 line {i}\n"));
    }
    prism_agent_lib::commands::file::file_write(big_path.clone(), content)
        .await
        .unwrap();

    let out = prism_agent_lib::commands::file::file_read_text(big_path)
        .await
        .unwrap();
    assert!(out.contains("[内容过大，已截断"));
    let lines: Vec<&str> = out.lines().collect();
    // 100 行正文 + 1 空行（截断标记前置 \n）+ 1 行截断标记
    assert_eq!(lines.len(), 102);
    assert!(lines[0].starts_with("0123456789 line 0"));
    assert!(lines[99].starts_with("0123456789 line 99"));
    assert_eq!(
        lines[101],
        "[内容过大，已截断：文件超过 200KB，仅显示前 100 行]"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sandbox_path_and_command() {
    let policy = SandboxPolicy::default();
    assert!(
        policy.check_path("/etc/passwd").is_err(),
        "黑名单路径必须拒绝"
    );
    assert!(
        policy.check_path("./src/main.rs").is_ok(),
        "白名单路径必须放行"
    );
    assert!(
        policy.check_command("rm -rf /").is_err(),
        "黑名单命令必须拒绝"
    );
    assert!(policy.check_command("ls -la").is_ok(), "白名单命令必须放行");
}

// ── 4. 端到端闭环：对话 → 工具运行 → 本地读写 ─────────────

#[tokio::test]
async fn end_to_end_capability_loop() {
    let dir = temp_dir("e2e");
    let target = dir.join("result.txt");
    let target_path = target.to_string_lossy().to_string();
    let payload = "闭环写入内容";

    let provider = Arc::new(FakeProvider::new(vec![
        GenerationResponse {
            text: String::new(),
            tool_calls: vec![ToolCall {
                id: "c1".into(),
                name: "write_file".into(),
                arguments: serde_json::json!({ "path": target_path, "content": payload }),
            }],
            usage: None,
        },
        GenerationResponse {
            text: String::new(),
            tool_calls: vec![ToolCall {
                id: "c2".into(),
                name: "read_file".into(),
                arguments: serde_json::json!({ "path": target_path }),
            }],
            usage: None,
        },
        GenerationResponse {
            text: format!("读取结果: {payload}"),
            tool_calls: vec![],
            usage: None,
        },
    ]));

    let mut registry = ToolRegistry::new();
    registry.register(Box::new(FileTools::write()));
    registry.register(Box::new(FileTools::read()));

    let traces: Arc<Mutex<Vec<AgentTrace>>> = Arc::new(Mutex::new(Vec::new()));
    let traces_cb = traces.clone();
    let agent = RigAgent::new(provider, "system".into(), registry).with_on_trace(move |trace| {
        traces_cb.lock().unwrap().push(trace);
    });

    let result = agent
        .run(plain_request())
        .await
        .expect("end-to-end loop should succeed");

    // 对话：最终文本包含读回内容
    assert_eq!(result.text, format!("读取结果: {payload}"));
    // 本地读写：文件真实落盘且内容正确
    assert_eq!(std::fs::read_to_string(&target).unwrap(), payload);
    // 运行：trace 覆盖 llm 调用与工具执行两类步骤
    let trace_guard = traces.lock().unwrap();
    let kinds: Vec<&str> = trace_guard
        .first()
        .expect("trace recorded")
        .steps
        .iter()
        .map(|s| s.kind.as_str())
        .collect();
    assert!(kinds.contains(&"llm_call"));
    assert!(kinds.contains(&"tool_call"));

    let _ = std::fs::remove_dir_all(&dir);
}
