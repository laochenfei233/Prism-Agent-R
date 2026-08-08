pub mod items;
pub mod state;

use serde::{Deserialize, Serialize};

/// §17.1 会话状态机
///
/// CREATED → INIT → READY → RUNNING → (VERIFY)* → DONE
///                │          └── 中断 → PAUSED → RESUME → RUNNING
///                └── init 失败 → INIT_FAILED（提示 + 可重试）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SessionLifecycle {
    Created,
    Init,
    Ready,
    Running,
    Paused,
    Verifying,
    Done,
    InitFailed,
}

/// 会话初始化报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInitReport {
    pub provider_ok: bool,
    pub provider_error: Option<String>,
    pub memory_ok: bool,
    pub memory_error: Option<String>,
    pub mcp_ok: bool,
    pub mcp_error: Option<String>,
}

impl SessionInitReport {
    pub fn all_ok(&self) -> bool {
        self.provider_ok && self.memory_ok && self.mcp_ok
    }

    pub fn errors(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if !self.provider_ok {
            if let Some(e) = &self.provider_error {
                errors.push(format!("模型 Provider: {e}"));
            }
        }
        if !self.memory_ok {
            if let Some(e) = &self.memory_error {
                errors.push(format!("记忆系统: {e}"));
            }
        }
        if !self.mcp_ok {
            if let Some(e) = &self.mcp_error {
                errors.push(format!("MCP 服务器: {e}"));
            }
        }
        errors
    }
}
