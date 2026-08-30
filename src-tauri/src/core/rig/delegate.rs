use async_trait::async_trait;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::core::adk::error::AgentError;
use crate::core::adk::model::{
    ChatMessage, ChatRole, GenerationRequest, MessageContent, ToolOutput,
};
use crate::core::adk::tool::{
    ApprovalRequestKind, RiskLevel, ToolApprovalRequest, ToolApprovalResponse, ToolApprovalStore,
    ToolExecutor, ToolRegistry,
};
use crate::core::rig::agent::{AgentSkill, RigAgent};
use crate::core::rig::provider::build_provider;
use crate::data::models::{AgentRow, ModelRow, ProviderRow};
use crate::data::services::SessionService;
use crate::mcp::McpRuntime;

/// `delegate_to_agent` 工具：Orchestrator 将子任务委派给指定专业 Agent 执行。
///
/// 与 `McpToolExecutor` / `WebSearchTool` 同层实现 `ToolExecutor`。
/// 子 Agent 以临时 session 运行，可使用自身绑定的 MCP 工具与启用 Skill；
/// 该工具在风险表中标记为 Low，工具级 HITL 不拦截；
/// 真正的审批由本工具内的 DelegationGate 统一完成（一次确认）。
/// 全局设置可关闭工具能力或限制注册工具数量。
#[derive(Clone)]
pub struct DelegateToAgentTool {
    pool: SqlitePool,
    mcp_runtime: Arc<McpRuntime>,
    approval_store: Arc<ToolApprovalStore>,
    app_handle: AppHandle,
    parent_session_id: String,
    parent_agent_id: String,
}

impl DelegateToAgentTool {
    pub fn new(
        pool: SqlitePool,
        mcp_runtime: Arc<McpRuntime>,
        approval_store: Arc<ToolApprovalStore>,
        app_handle: AppHandle,
        parent_session_id: String,
        parent_agent_id: String,
    ) -> Self {
        Self {
            pool,
            mcp_runtime,
            approval_store,
            app_handle,
            parent_session_id,
            parent_agent_id,
        }
    }
}

#[async_trait]
impl ToolExecutor for DelegateToAgentTool {
    fn name(&self) -> &str {
        "delegate_to_agent"
    }

    fn description(&self) -> &str {
        "将子任务委派给指定的专业 Agent 执行，并返回该 Agent 的完整回复文本。参数：agent_id（目标 Agent 的 id）、task（具体子任务描述）。"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "agent_id": {
                    "type": "string",
                    "description": "目标 Agent 的 id"
                },
                "task": {
                    "type": "string",
                    "description": "具体子任务描述"
                }
            },
            "required": ["agent_id", "task"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolOutput, AgentError> {
        let agent_id = args
            .get("agent_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim()
            .to_string();
        let task = args
            .get("task")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim()
            .to_string();

        if agent_id.is_empty() || task.is_empty() {
            return Ok(ToolOutput::error(
                "delegate_to_agent 需要 agent_id 和 task 两个参数".to_string(),
            ));
        }

        // 1. 按 agent_id 查询 Agent
        let agent_row = sqlx::query_as::<_, AgentRow>(
            "SELECT id, name, description, avatar, system_prompt, model_id, plan_model_id, small_model_id, temperature, max_tokens, disabled_tools, configuration, order_key, is_orchestrator, created_at, updated_at FROM agents WHERE id = ?",
        )
        .bind(&agent_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AgentError::Tool(format!("delegate_to_agent 查询 Agent 失败: {e}")))?;

        let Some(agent_row) = agent_row else {
            return Ok(ToolOutput::error("Agent not found".to_string()));
        };

        // 委派深度 = 1：禁止委派给另一个 Orchestrator，防止无限递归
        if agent_row.is_orchestrator != 0 {
            return Ok(ToolOutput::error(
                "不能委派给另一个 Orchestrator Agent".to_string(),
            ));
        }

        // 3. 读取全局子 Agent 工具设置 + 目标 Agent 能力配额，生成能力摘要。
        let tools_enabled = crate::data::settings::prefs::get_bool(
            &self.pool,
            "orchestrator.sub_agent_tools_enabled",
            true,
        )
        .await;
        let global_max_tools = crate::data::settings::prefs::get_i64(
            &self.pool,
            "orchestrator.sub_agent_max_tools",
            0,
        )
        .await;
        let caps = self
            .child_capability_config(&agent_row, global_max_tools)
            .await;
        let capability_summary = self
            .capability_summary(&agent_id, &caps, tools_enabled)
            .await;

        // 4. 委派 Gate：结构化审批子 Agent 目标、任务与能力计划。
        let gate_call_id = uuid::Uuid::new_v4().to_string();
        let gate_request = ToolApprovalRequest {
            call_id: gate_call_id.clone(),
            tool_name: "delegate_to_agent".to_string(),
            arguments: args.clone(),
            agent_id: self.parent_agent_id.clone(),
            risk_level: RiskLevel::High,
            description: format!(
                "Orchestrator 准备将子任务委派给 Agent「{}」",
                agent_row.name
            ),
            kind: ApprovalRequestKind::Delegation,
            session_id: Some(self.parent_session_id.clone()),
            parent_session_id: Some(self.parent_session_id.clone()),
            parent_agent_id: Some(self.parent_agent_id.clone()),
            child_agent_id: Some(agent_id.clone()),
            task_summary: Some(truncate_chars(&task, 240)),
            capability_summary: Some(capability_summary),
            confirm_step: None,
        };
        let _ = self.app_handle.emit("tool:approval-request", &gate_request);
        let gate_rx = self.approval_store.request_approval(gate_request).await;
        let timeout_secs = self.approval_store.timeout_seconds().await;
        let gate_response: Result<ToolApprovalResponse, ()> = if timeout_secs == 0 {
            gate_rx.await.map_err(|_| ())
        } else {
            match tokio::time::timeout(Duration::from_secs(timeout_secs as u64), gate_rx).await {
                Ok(Ok(response)) => Ok(response),
                _ => Err(()),
            }
        };
        match gate_response {
            Ok(ToolApprovalResponse::Approved) => {}
            Ok(ToolApprovalResponse::Rejected(reason)) => {
                return Ok(ToolOutput::error(format!("委派被用户拒绝: {reason}")));
            }
            Ok(ToolApprovalResponse::AlwaysApprove(_)) => {
                return Ok(ToolOutput::error(
                    "委派审批不允许 AlwaysApprove，请逐次批准".to_string(),
                ));
            }
            _ => {
                self.approval_store
                    .abandon(&gate_call_id, "expired", "委派审批超时")
                    .await;
                return Ok(ToolOutput::error(
                    "委派审批超时或已搁置，未执行".to_string(),
                ));
            }
        }

        // 2. 查询模型（与 chat_send 一致：UUID → model_id 字符串 → 默认模型）
        let model_row = if let Some(ref mid) = agent_row.model_id {
            let found = sqlx::query_as::<_, ModelRow>(
                "SELECT id, provider_id, model_id, display_name, kind, max_tokens, is_default, created_at FROM models WHERE id = ?",
            )
            .bind(mid)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| AgentError::Tool(format!("delegate_to_agent 查询模型失败: {e}")))?;
            if found.is_some() {
                found
            } else {
                sqlx::query_as::<_, ModelRow>(
                    "SELECT id, provider_id, model_id, display_name, kind, max_tokens, is_default, created_at FROM models WHERE model_id = ? LIMIT 1",
                )
                .bind(mid)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| AgentError::Tool(format!("delegate_to_agent 查询模型失败: {e}")))?
            }
        } else {
            sqlx::query_as::<_, ModelRow>(
                "SELECT id, provider_id, model_id, display_name, kind, max_tokens, is_default, created_at FROM models WHERE is_default = 1 LIMIT 1",
            )
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| AgentError::Tool(format!("delegate_to_agent 查询默认模型失败: {e}")))?
        };

        let Some(model_row) = model_row else {
            return Ok(ToolOutput::error(format!(
                "Agent「{}」未配置模型，无法委派",
                agent_row.name
            )));
        };

        // 3. 查询 Provider 并构建 provider
        let provider_row = sqlx::query_as::<_, ProviderRow>(
            "SELECT id, name, kind, base_url, api_key_enc, is_enabled, created_at, updated_at FROM providers WHERE id = ?",
        )
        .bind(&model_row.provider_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AgentError::Tool(format!("delegate_to_agent 查询 Provider 失败: {e}")))?
        .ok_or_else(|| AgentError::Tool(format!("Provider not found: {}", model_row.provider_id)))?;

        let base_url =
            provider_row
                .base_url
                .clone()
                .unwrap_or_else(|| match provider_row.kind.as_str() {
                    "ollama" => "http://localhost:11434/v1".to_string(),
                    _ => "https://api.openai.com/v1".to_string(),
                });
        let api_key = provider_row
            .api_key_enc
            .as_deref()
            .map(crate::commands::settings::decrypt_provider_key)
            .unwrap_or_default();
        let provider = build_provider(&provider_row, &model_row, api_key, base_url);

        // 4. 创建临时 session（标题：[Orchestrator] {task 前 30 字}）
        let session_svc = SessionService::new(self.pool.clone());
        let session = session_svc
            .create(
                &agent_id,
                Some(&format!("[Orchestrator] {}", truncate_chars(&task, 30))),
            )
            .await
            .map_err(|e| {
                AgentError::Tool(format!("delegate_to_agent 创建临时 session 失败: {e}"))
            })?;

        // 5. 读取目标 Agent 的禁用工具并按能力配额构建子 Agent 运行时。
        let child_disabled_tools: Vec<String> =
            serde_json::from_str(&agent_row.disabled_tools).unwrap_or_default();
        let child_skills = if tools_enabled {
            self.load_child_skills(&agent_id, caps.max_skills).await?
        } else {
            Vec::new()
        };
        let child_registry = if tools_enabled {
            self.build_child_registry(&agent_id, &caps, &child_disabled_tools, global_max_tools)
                .await?
        } else {
            ToolRegistry::new()
        };

        // 6. 构建子 Agent；不传入 MCP fallback，确保工具调用受上限和禁用名单约束。
        let system_prompt = agent_row.system_prompt.clone().unwrap_or_default();
        let mut child = RigAgent::new(provider, system_prompt, child_registry)
            .with_agent_id(agent_id.clone())
            .with_session_id(self.parent_session_id.clone())
            .with_approval_store(self.approval_store.clone())
            .with_app_handle(self.app_handle.clone())
            .with_skills(child_skills);
        child.router = Some(child.build_router(8));
        let request = GenerationRequest {
            messages: vec![ChatMessage {
                role: ChatRole::User,
                content: MessageContent::Text(task),
                name: None,
            }],
            temperature: agent_row.temperature.map(|t| t as f32),
            max_tokens: agent_row.max_tokens.map(|m| m as u32),
            ..Default::default()
        };
        let result = child.run(request).await;

        // 7. 删除临时 session（可选，这里删除避免污染会话列表）
        let _ = session_svc.delete(&session.id).await;

        match result {
            Ok(run) => Ok(ToolOutput::text(run.text)),
            Err(e) => Ok(ToolOutput::error(format!(
                "子 Agent「{}」执行失败: {e}",
                agent_row.name
            ))),
        }
    }
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect()
    }
}

impl DelegateToAgentTool {
    async fn build_child_registry(
        &self,
        agent_id: &str,
        caps: &ChildCapabilityConfig,
        disabled_tools: &[String],
        global_max_tools: i64,
    ) -> Result<ToolRegistry, AgentError> {
        let mut registry = ToolRegistry::new();
        let mut mcp_links: Vec<String> =
            sqlx::query_scalar("SELECT mcp_server_id FROM agent_mcp_servers WHERE agent_id = ?")
                .bind(agent_id)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| AgentError::Internal(format!("查询子 Agent MCP 绑定失败: {e}")))?;
        if caps.max_mcp_servers > 0 {
            mcp_links.truncate(caps.max_mcp_servers);
        }

        let mut total_mcp_tools = 0usize;
        for server_id in mcp_links {
            if caps.max_mcp_tools > 0 && total_mcp_tools >= caps.max_mcp_tools {
                break;
            }
            let mut tools = self.mcp_runtime.get_tools(&server_id).await;
            if let Some(limit) = caps.server_tool_limits.get(&server_id) {
                if *limit > 0 {
                    tools.truncate(*limit);
                }
            }
            for tool in tools {
                if caps.max_mcp_tools > 0 && total_mcp_tools >= caps.max_mcp_tools {
                    break;
                }
                registry.register(Box::new(crate::core::rig::agent::McpToolExecutor::new(
                    server_id.clone(),
                    tool.name.clone(),
                    tool.description.clone(),
                    tool.input_schema.clone(),
                    self.mcp_runtime.clone(),
                )));
                total_mcp_tools += 1;
            }
        }

        let search_config = crate::commands::search::get_search_config(&self.pool).await;
        let search_service = Arc::new(crate::core::search::service::SearchService::from_config(
            &search_config,
        ));
        registry.register(Box::new(
            crate::core::search::web_search::WebSearchTool::new(search_service),
        ));
        registry.register(Box::new(crate::core::adk::wiki_tool::WikiWriteTool::new(
            crate::data::Database {
                pool: self.pool.clone(),
            },
        )));
        registry.register(Box::new(crate::core::adk::wiki_tool::WikiSearchTool::new(
            crate::data::Database {
                pool: self.pool.clone(),
            },
        )));

        let memory_dir = crate::utils::paths::memory_dir();
        registry.register(Box::new(
            crate::core::adk::memory_tools::MemorySearchTool::new(
                crate::data::Database {
                    pool: self.pool.clone(),
                },
                memory_dir.clone(),
            ),
        ));
        registry.register(Box::new(
            crate::core::adk::memory_tools::MemorySaveTool::new(
                crate::data::Database {
                    pool: self.pool.clone(),
                },
                memory_dir,
            ),
        ));

        registry.register(Box::new(crate::core::adk::file_tools::FileReadTool));
        registry.register(Box::new(crate::core::adk::file_tools::FileWriteTool));
        registry.register(Box::new(crate::core::adk::file_tools::FileEditTool));
        registry.register(Box::new(crate::core::adk::file_tools::FileListTool));
        registry.register(Box::new(crate::core::adk::search_tools::GrepTool));
        registry.register(Box::new(crate::core::adk::search_tools::GlobTool));
        registry.register(Box::new(crate::core::adk::task_tools::TaskCreateTool));
        registry.register(Box::new(crate::core::adk::task_tools::TaskUpdateTool));
        registry.register(Box::new(crate::core::adk::task_tools::TaskListTool));
        registry.register(Box::new(crate::core::adk::task_tools::TaskDeleteTool));

        let max_total_tools = caps
            .max_total_tools
            .or_else(|| (global_max_tools > 0).then_some(global_max_tools as usize));
        if let Some(max_total_tools) = max_total_tools {
            while registry.specs().len() > max_total_tools {
                let Some(last_tool) = registry.specs().pop() else {
                    break;
                };
                registry.remove(&last_tool.name);
            }
        }

        if !caps.tool_allowlist.is_empty() {
            let names = registry.tool_names();
            for name in names {
                if !caps.tool_allowlist.contains(&name) {
                    registry.remove(&name);
                }
            }
        }

        for name in disabled_tools {
            registry.remove(name);
        }
        for name in &caps.tool_denylist {
            registry.remove(name);
        }

        Ok(registry)
    }

    async fn load_child_skills(
        &self,
        agent_id: &str,
        max_skills: usize,
    ) -> Result<Vec<AgentSkill>, AgentError> {
        let database = crate::data::Database {
            pool: self.pool.clone(),
        };
        let mut enabled_skills = crate::data::services::SkillService::new(database)
            .enabled_skills(agent_id)
            .await
            .map_err(|e| AgentError::Internal(format!("加载子 Agent Skill 失败: {e}")))?;
        if max_skills > 0 {
            enabled_skills.truncate(max_skills);
        }

        let mut skills = Vec::new();
        for skill_id in enabled_skills {
            let row = sqlx::query_as::<_, (String, String, Option<String>)>(
                "SELECT folder_name, name, description FROM skills WHERE id = ? AND is_enabled = 1",
            )
            .bind(&skill_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| AgentError::Internal(format!("查询子 Agent Skill 失败: {e}")))?;

            let Some((folder_name, name, description)) = row else {
                continue;
            };
            let skill_path = crate::utils::paths::skill_dir()
                .join(&folder_name)
                .join("SKILL.md");
            if let Ok(content) = tokio::fs::read_to_string(&skill_path).await {
                skills.push(AgentSkill {
                    id: skill_id,
                    name: name.clone(),
                    description: description.unwrap_or_default(),
                    content,
                    keywords: vec![name, folder_name],
                });
            }
        }
        Ok(skills)
    }

    async fn child_capability_config(
        &self,
        agent_row: &AgentRow,
        global_max_tools: i64,
    ) -> ChildCapabilityConfig {
        ChildCapabilityConfig::parse(&agent_row.configuration, global_max_tools)
    }

    async fn capability_summary(
        &self,
        agent_id: &str,
        caps: &ChildCapabilityConfig,
        tools_enabled: bool,
    ) -> String {
        if !tools_enabled {
            return "子 Agent 工具已全局关闭，仅使用纯 LLM 推理".to_string();
        }

        let mcp_links: Vec<String> =
            sqlx::query_scalar("SELECT mcp_server_id FROM agent_mcp_servers WHERE agent_id = ?")
                .bind(agent_id)
                .fetch_all(&self.pool)
                .await
                .unwrap_or_default();
        let raw_server_count = mcp_links.len();
        let server_count = if caps.max_mcp_servers > 0 {
            raw_server_count.min(caps.max_mcp_servers)
        } else {
            raw_server_count
        };

        let mut mcp_tools = 0usize;
        for server_id in mcp_links.iter().take(server_count) {
            let tools = self.mcp_runtime.get_tools(server_id).await;
            let limit = caps.server_tool_limits.get(server_id).copied().unwrap_or(0);
            let count = if limit > 0 {
                tools.len().min(limit)
            } else {
                tools.len()
            };
            mcp_tools = mcp_tools.saturating_add(count);
            if caps.max_mcp_tools > 0 && mcp_tools >= caps.max_mcp_tools {
                mcp_tools = caps.max_mcp_tools;
                break;
            }
        }

        let skill_svc = crate::data::services::SkillService::new(crate::data::Database {
            pool: self.pool.clone(),
        });
        let enabled_skills = skill_svc.enabled_skills(agent_id).await.unwrap_or_default();
        let raw_skill_count = enabled_skills.len();
        let skill_count = if caps.max_skills > 0 {
            raw_skill_count.min(caps.max_skills)
        } else {
            raw_skill_count
        };

        let total_cap = caps
            .max_total_tools
            .map(|v| v.to_string())
            .unwrap_or_else(|| "不限".to_string());
        format!(
            "MCP Servers: {server_count}/{raw_server_count} · MCP Tools: {mcp_tools} · Skills: {skill_count}/{raw_skill_count} · Total Tools 上限: {total_cap}"
        )
    }
}

struct ChildCapabilityConfig {
    max_mcp_servers: usize,
    max_mcp_tools: usize,
    server_tool_limits: HashMap<String, usize>,
    max_skills: usize,
    max_total_tools: Option<usize>,
    tool_allowlist: Vec<String>,
    tool_denylist: Vec<String>,
}

impl ChildCapabilityConfig {
    fn parse(config_json: &str, global_max_tools: i64) -> Self {
        let config: serde_json::Value = serde_json::from_str(config_json).unwrap_or_default();
        let caps = config.get("sub_agent_capabilities");
        let number = |key: &str| -> usize {
            caps.and_then(|v| v.get(key))
                .and_then(|v| v.as_i64())
                .unwrap_or(0)
                .max(0) as usize
        };

        let mut server_tool_limits = HashMap::new();
        if let Some(limits) = caps
            .and_then(|v| v.get("mcp_server_tool_limits"))
            .and_then(|v| v.as_object())
        {
            for (server_id, value) in limits {
                if let Some(n) = value.as_i64() {
                    server_tool_limits.insert(server_id.clone(), n.max(0) as usize);
                }
            }
        }

        let string_list = |key: &str| -> Vec<String> {
            caps.and_then(|v| v.get(key))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default()
        };

        let max_total_tools = if number("max_total_tools") > 0 {
            Some(number("max_total_tools"))
        } else if global_max_tools > 0 {
            Some(global_max_tools as usize)
        } else {
            None
        };

        Self {
            max_mcp_servers: number("max_mcp_servers"),
            max_mcp_tools: number("max_mcp_tools"),
            server_tool_limits,
            max_skills: number("max_skills"),
            max_total_tools,
            tool_allowlist: string_list("tool_allowlist"),
            tool_denylist: string_list("tool_denylist"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_config_parses_quota() {
        let config = r#"{
          "sub_agent_capabilities": {
            "max_mcp_servers": 2,
            "max_mcp_tools": 5,
            "mcp_server_tool_limits": {"srv-1": 3},
            "max_skills": 1,
            "max_total_tools": 8,
            "tool_allowlist": ["file_read"],
            "tool_denylist": ["task_delete"]
          }
        }"#;
        let caps = ChildCapabilityConfig::parse(config, 10);
        assert_eq!(caps.max_mcp_servers, 2);
        assert_eq!(caps.max_mcp_tools, 5);
        assert_eq!(caps.server_tool_limits.get("srv-1"), Some(&3));
        assert_eq!(caps.max_skills, 1);
        assert_eq!(caps.max_total_tools, Some(8));
        assert_eq!(caps.tool_allowlist, vec!["file_read"]);
        assert_eq!(caps.tool_denylist, vec!["task_delete"]);
    }

    #[test]
    fn capability_config_falls_back_to_global_max() {
        let caps = ChildCapabilityConfig::parse("{}", 12);
        assert_eq!(caps.max_total_tools, Some(12));
        assert_eq!(caps.max_mcp_servers, 0);
    }
}
