use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GuardrailDecision {
    Allow,
    Deny {
        reason: String,
    },
    NeedApproval {
        tool: String,
        args: serde_json::Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolPolicy {
    pub allowed_tools: Option<Vec<String>>,
    pub denied_tools: Vec<String>,
    pub tool_configs: HashMap<String, ToolConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolConfig {
    pub max_calls_per_run: Option<u32>,
    pub require_approval: bool,
    pub param_validators: Vec<ParamValidator>,
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParamValidator {
    PathWhitelist(Vec<String>),
    PathBlacklist(Vec<String>),
    Regex(String),
    LengthRange(usize, usize),
}

impl ParamValidator {
    pub fn validate(&self, args: &serde_json::Value) -> bool {
        match self {
            ParamValidator::PathWhitelist(paths) => {
                if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
                    paths.iter().any(|p| path.starts_with(p))
                } else {
                    true
                }
            }
            ParamValidator::PathBlacklist(paths) => {
                if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
                    !paths.iter().any(|p| path.starts_with(p))
                } else {
                    true
                }
            }
            ParamValidator::Regex(pattern) => {
                if let Ok(re) = regex::Regex::new(pattern) {
                    let text = args.to_string();
                    re.is_match(&text)
                } else {
                    true
                }
            }
            ParamValidator::LengthRange(min, max) => {
                if let Some(text) = args.get("text").and_then(|v| v.as_str()) {
                    let len = text.len();
                    len >= *min && len <= *max
                } else {
                    true
                }
            }
        }
    }

    pub fn error_msg(&self) -> String {
        match self {
            ParamValidator::PathWhitelist(paths) => format!("路径不在白名单中: {:?}", paths),
            ParamValidator::PathBlacklist(paths) => format!("路径在黑名单中: {:?}", paths),
            ParamValidator::Regex(p) => format!("参数不匹配正则: {p}"),
            ParamValidator::LengthRange(min, max) => format!("长度不在范围 {min}-{max}"),
        }
    }
}

/// 工具级护栏
pub struct ToolGuardrail {
    pub policy: ToolPolicy,
    call_counts: std::sync::RwLock<HashMap<String, u32>>,
}

impl ToolGuardrail {
    pub fn new(policy: ToolPolicy) -> Self {
        Self {
            policy,
            call_counts: std::sync::RwLock::new(HashMap::new()),
        }
    }

    pub async fn check_tool_call(
        &self,
        tool_name: &str,
        args: &serde_json::Value,
    ) -> GuardrailDecision {
        // 1. 黑名单检查
        if self.policy.denied_tools.contains(&tool_name.to_string()) {
            return GuardrailDecision::Deny {
                reason: format!("工具 '{}' 被策略禁止", tool_name),
            };
        }

        // 2. 白名单检查
        if let Some(allowed) = &self.policy.allowed_tools {
            if !allowed.contains(&tool_name.to_string()) {
                return GuardrailDecision::Deny {
                    reason: format!("工具 '{}' 不在白名单中", tool_name),
                };
            }
        }

        // 3. 工具独立配置检查
        if let Some(config) = self.policy.tool_configs.get(tool_name) {
            // 调用次数检查
            if let Some(max) = config.max_calls_per_run {
                let counts = self.call_counts.read().unwrap();
                let count = counts.get(tool_name).copied().unwrap_or(0);
                if count >= max {
                    return GuardrailDecision::Deny {
                        reason: format!("工具 '{}' 已达调用上限 {}", tool_name, max),
                    };
                }
            }

            // 参数校验
            for validator in &config.param_validators {
                if !validator.validate(args) {
                    return GuardrailDecision::Deny {
                        reason: format!(
                            "工具 '{}' 参数校验失败: {}",
                            tool_name,
                            validator.error_msg()
                        ),
                    };
                }
            }

            // 审批检查
            if config.require_approval {
                return GuardrailDecision::NeedApproval {
                    tool: tool_name.to_string(),
                    args: args.clone(),
                };
            }
        }

        GuardrailDecision::Allow
    }

    pub fn record_call(&self, tool_name: &str) {
        let mut counts = self.call_counts.write().unwrap();
        *counts.entry(tool_name.to_string()).or_insert(0) += 1;
    }

    pub fn reset_counts(&self) {
        let mut counts = self.call_counts.write().unwrap();
        counts.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deny_blacklisted_tool() {
        let policy = ToolPolicy {
            denied_tools: vec!["rm".into()],
            ..Default::default()
        };
        let guard = ToolGuardrail::new(policy);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(guard.check_tool_call("rm", &serde_json::json!({"path": "/tmp"})));
        assert!(matches!(result, GuardrailDecision::Deny { .. }));
    }

    #[test]
    fn allow_normal_tool() {
        let guard = ToolGuardrail::new(ToolPolicy::default());
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result =
            rt.block_on(guard.check_tool_call("web_search", &serde_json::json!({"query": "test"})));
        assert!(matches!(result, GuardrailDecision::Allow));
    }
}
