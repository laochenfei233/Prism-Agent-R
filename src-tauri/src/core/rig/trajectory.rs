use serde::{Deserialize, Serialize};
use crate::core::adk::model::ToolCall;

/// §19.3.5 轨迹级监控
///
/// 单动作安全 ≠ 轨迹安全（token 拆两半绕过扫描器）
/// 检测：凭据/令牌拆分拼接重建、多次同源失败后换路径、访问范围外系统

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SuspiciousAction {
    /// 暂停会话 + 通知前端 + 等用户确认
    PauseAndConfirm,
    /// 仅记录日志
    LogOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryCheck {
    pub name: String,
    pub description: String,
    pub action: SuspiciousAction,
}

/// 轨迹监控器
pub struct TrajectoryMonitor {
    checks: Vec<TrajectoryCheck>,
    enabled: bool,
}

impl TrajectoryMonitor {
    pub fn new() -> Self {
        Self {
            checks: Self::default_checks(),
            enabled: false,
        }
    }

    pub fn with_enabled(enabled: bool) -> Self {
        let mut monitor = Self::new();
        monitor.enabled = enabled;
        monitor
    }

    /// 默认检测规则
    fn default_checks() -> Vec<TrajectoryCheck> {
        vec![
            TrajectoryCheck {
                name: "credential_concat".into(),
                description: "检测凭据/令牌拆分拼接重建".into(),
                action: SuspiciousAction::PauseAndConfirm,
            },
            TrajectoryCheck {
                name: "sandbox_escape".into(),
                description: "检测沙箱逃逸尝试".into(),
                action: SuspiciousAction::PauseAndConfirm,
            },
            TrajectoryCheck {
                name: "unauthorized_access".into(),
                description: "检测访问范围外系统".into(),
                action: SuspiciousAction::PauseAndConfirm,
            },
            TrajectoryCheck {
                name: "repeated_failure".into(),
                description: "多次同源失败后换路径".into(),
                action: SuspiciousAction::LogOnly,
            },
        ]
    }

    /// 检查工具调用轨迹是否可疑
    pub fn check_trajectory(&self, calls: &[ToolCall]) -> Option<TrajectoryAlert> {
        if !self.enabled {
            return None;
        }

        // 检测 1: 凭据拼接（连续的短字符串拼接可能在重建 token）
        if let Some(alert) = self.check_credential_concat(calls) {
            return Some(alert);
        }

        // 检测 2: 沙箱逃逸（尝试访问 /etc/passwd、~/.ssh 等）
        if let Some(alert) = self.check_sandbox_escape(calls) {
            return Some(alert);
        }

        // 检测 3: 越权访问（访问范围外系统）
        if let Some(alert) = self.check_unauthorized_access(calls) {
            return Some(alert);
        }

        None
    }

    /// 检测凭据拼接
    fn check_credential_concat(&self, calls: &[ToolCall]) -> Option<TrajectoryAlert> {
        // 检查是否有连续的短 write_file 调用（可能在拼接 token）
        let short_writes: Vec<_> = calls.iter()
            .filter(|c| {
                c.name == "write_file" || c.name == "edit_file"
            })
            .filter(|c| {
                let args_str = c.arguments.to_string();
                args_str.len() < 100
            })
            .collect();

        if short_writes.len() >= 3 {
            return Some(TrajectoryAlert {
                check_name: "credential_concat".into(),
                reason: "检测到多次短内容写入，可能在拼接敏感凭据".into(),
                evidence: short_writes.iter().map(|c| c.id.clone()).collect(),
                action: SuspiciousAction::PauseAndConfirm,
            });
        }

        None
    }

    /// 检测沙箱逃逸
    fn check_sandbox_escape(&self, calls: &[ToolCall]) -> Option<TrajectoryAlert> {
        let dangerous_paths = ["/etc/passwd", "/etc/shadow", "~/.ssh", "~/.aws", "~/.env"];

        for call in calls {
            if call.name == "read_file" || call.name == "write_file" {
                let args_str = call.arguments.to_string();
                for path in &dangerous_paths {
                    if args_str.contains(path) {
                        return Some(TrajectoryAlert {
                            check_name: "sandbox_escape".into(),
                            reason: format!("尝试访问敏感路径: {path}"),
                            evidence: vec![call.id.clone()],
                            action: SuspiciousAction::PauseAndConfirm,
                        });
                    }
                }
            }
        }

        None
    }

    /// 检测越权访问
    fn check_unauthorized_access(&self, calls: &[ToolCall]) -> Option<TrajectoryAlert> {
        // 检查是否有 run_command 调用敏感命令
        let sensitive_commands = ["rm -rf", "sudo", "chmod 777", "curl", "wget"];

        for call in calls {
            if call.name == "run_command" {
                let args_str = call.arguments.to_string();
                for cmd in &sensitive_commands {
                    if args_str.contains(cmd) {
                        return Some(TrajectoryAlert {
                            check_name: "unauthorized_access".into(),
                            reason: format!("检测到敏感命令: {cmd}"),
                            evidence: vec![call.id.clone()],
                            action: SuspiciousAction::PauseAndConfirm,
                        });
                    }
                }
            }
        }

        None
    }

    /// 启用/禁用监控
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// 当前监控是否启用
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// 返回已注册的检测规则
    pub fn checks(&self) -> &[TrajectoryCheck] {
        &self.checks
    }
}

impl Default for TrajectoryMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryAlert {
    pub check_name: String,
    pub reason: String,
    pub evidence: Vec<String>,
    pub action: SuspiciousAction,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_tool_call(name: &str, args: serde_json::Value) -> ToolCall {
        ToolCall {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            arguments: args,
        }
    }

    #[test]
    fn test_monitor_disabled() {
        let monitor = TrajectoryMonitor::with_enabled(false);
        let calls = vec![make_tool_call("read_file", json!({"path": "/etc/passwd"}))];
        assert!(monitor.check_trajectory(&calls).is_none());
    }

    #[test]
    fn test_credential_concat_detection() {
        let monitor = TrajectoryMonitor::with_enabled(true);
        let calls = vec![
            make_tool_call("write_file", json!({"content": "abc"})),
            make_tool_call("write_file", json!({"content": "def"})),
            make_tool_call("write_file", json!({"content": "ghi"})),
        ];
        let alert = monitor.check_trajectory(&calls);
        assert!(alert.is_some());
        assert_eq!(alert.unwrap().check_name, "credential_concat");
    }

    #[test]
    fn test_sandbox_escape_detection() {
        let monitor = TrajectoryMonitor::with_enabled(true);
        let calls = vec![make_tool_call("read_file", json!({"path": "~/.ssh/id_rsa"}))];
        let alert = monitor.check_trajectory(&calls);
        assert!(alert.is_some());
        assert_eq!(alert.unwrap().check_name, "sandbox_escape");
    }

    #[test]
    fn test_unauthorized_access_detection() {
        let monitor = TrajectoryMonitor::with_enabled(true);
        let calls = vec![make_tool_call("run_command", json!({"command": "sudo rm -rf /"}))];
        let alert = monitor.check_trajectory(&calls);
        assert!(alert.is_some());
        assert_eq!(alert.unwrap().check_name, "unauthorized_access");
    }

    #[test]
    fn test_safe_calls_no_alert() {
        let monitor = TrajectoryMonitor::with_enabled(true);
        let calls = vec![
            make_tool_call("read_file", json!({"path": "src/main.rs"})),
            make_tool_call("web_search", json!({"query": "rust tutorial"})),
        ];
        assert!(monitor.check_trajectory(&calls).is_none());
    }
}
