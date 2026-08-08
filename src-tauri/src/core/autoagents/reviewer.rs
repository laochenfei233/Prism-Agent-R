use serde::{Deserialize, Serialize};
use crate::core::adk::tool::RiskLevel;

/// §19.3.4 Auto-review 审批子代理
///
/// 低风险自动放行，高风险转人工，每 run 硬上限防失控

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ReviewDecision {
    /// 自动放行
    AutoApprove,
    /// 需要用户确认
    AskUser { reason: String },
    /// 拒绝
    Deny { reason: String },
}

/// 审批策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalPolicy {
    /// 自动放行的最大风险等级（<= Medium 自动放行）
    pub auto_approve_max_risk: RiskLevel,
    /// 需要用户确认的风险等级
    pub ask_user_risk: RiskLevel,
}

impl Default for ApprovalPolicy {
    fn default() -> Self {
        Self {
            auto_approve_max_risk: RiskLevel::Medium,
            ask_user_risk: RiskLevel::High,
        }
    }
}

/// §19.3.4 AutoReviewer 审批子代理
pub struct AutoReviewer {
    pub policy: ApprovalPolicy,
    pub max_auto_approve_per_run: u32,
    auto_approve_count: std::sync::atomic::AtomicU32,
}

impl AutoReviewer {
    pub fn new(max_auto_approve_per_run: u32) -> Self {
        Self {
            policy: ApprovalPolicy::default(),
            max_auto_approve_per_run,
            auto_approve_count: std::sync::atomic::AtomicU32::new(0),
        }
    }

    /// 评估工具调用风险并决定是否自动放行
    pub fn review(&self, tool_name: &str, risk_level: &RiskLevel) -> ReviewDecision {
        // 检查是否超过每 run 上限
        let count = self.auto_approve_count.load(std::sync::atomic::Ordering::Relaxed);
        if count >= self.max_auto_approve_per_run {
            return ReviewDecision::AskUser {
                reason: format!("已达到自动放行上限 ({count}/{})", self.max_auto_approve_per_run),
            };
        }

        // 按风险等级决策
        match risk_level {
            RiskLevel::Low => {
                self.auto_approve_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                ReviewDecision::AutoApprove
            }
            RiskLevel::Medium => {
                if self.policy.auto_approve_max_risk == RiskLevel::Medium ||
                   self.policy.auto_approve_max_risk == RiskLevel::Low {
                    self.auto_approve_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    ReviewDecision::AutoApprove
                } else {
                    ReviewDecision::AskUser {
                        reason: format!("工具 {tool_name} 风险等级 Medium"),
                    }
                }
            }
            RiskLevel::High => {
                ReviewDecision::AskUser {
                    reason: format!("工具 {tool_name} 风险等级 High，需要用户确认"),
                }
            }
            RiskLevel::Critical => {
                ReviewDecision::Deny {
                    reason: format!("工具 {tool_name} 风险等级 Critical，自动拒绝"),
                }
            }
        }
    }

    /// 重置计数器（新 run 开始时调用）
    pub fn reset(&self) {
        self.auto_approve_count.store(0, std::sync::atomic::Ordering::Relaxed);
    }

    /// 获取当前自动放行计数
    pub fn auto_approve_count(&self) -> u32 {
        self.auto_approve_count.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// 工具风险评估（扩展 assess_risk，支持 AutoReviewer）
pub fn assess_tool_risk(tool_name: &str) -> RiskLevel {
    match tool_name {
        "read_file" | "list_dir" | "glob" | "grep" | "lsp:diagnostics" | "web_search" => RiskLevel::Low,
        "write_file" | "edit_file" => RiskLevel::Medium,
        "delete_file" | "run_command" | "http_request" => RiskLevel::High,
        "rm_rf" | "database_drop" | "send_message" => RiskLevel::Critical,
        _ => RiskLevel::High,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_approve_low_risk() {
        let reviewer = AutoReviewer::new(20);
        assert_eq!(reviewer.review("read_file", &RiskLevel::Low), ReviewDecision::AutoApprove);
        assert_eq!(reviewer.auto_approve_count(), 1);
    }

    #[test]
    fn test_auto_approve_medium_risk() {
        let reviewer = AutoReviewer::new(20);
        assert_eq!(reviewer.review("write_file", &RiskLevel::Medium), ReviewDecision::AutoApprove);
    }

    #[test]
    fn test_ask_user_high_risk() {
        let reviewer = AutoReviewer::new(20);
        match reviewer.review("run_command", &RiskLevel::High) {
            ReviewDecision::AskUser { .. } => {}
            _ => panic!("Expected AskUser for High risk"),
        }
    }

    #[test]
    fn test_deny_critical_risk() {
        let reviewer = AutoReviewer::new(20);
        match reviewer.review("rm_rf", &RiskLevel::Critical) {
            ReviewDecision::Deny { .. } => {}
            _ => panic!("Expected Deny for Critical risk"),
        }
    }

    #[test]
    fn test_max_auto_approve_limit() {
        let reviewer = AutoReviewer::new(2);
        reviewer.review("read_file", &RiskLevel::Low);
        reviewer.review("read_file", &RiskLevel::Low);
        // 第 3 次应该 AskUser
        match reviewer.review("read_file", &RiskLevel::Low) {
            ReviewDecision::AskUser { reason } => {
                assert!(reason.contains("上限"));
            }
            _ => panic!("Expected AskUser when exceeding limit"),
        }
    }

    #[test]
    fn test_reset_counter() {
        let reviewer = AutoReviewer::new(20);
        reviewer.review("read_file", &RiskLevel::Low);
        assert_eq!(reviewer.auto_approve_count(), 1);
        reviewer.reset();
        assert_eq!(reviewer.auto_approve_count(), 0);
    }
}
