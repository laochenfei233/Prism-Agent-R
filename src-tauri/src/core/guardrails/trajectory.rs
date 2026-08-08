use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Violation {
    pub check_name: String,
    pub severity: Severity,
    pub description: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ViolationHandler {
    LogOnly,
    PauseAndNotify,
    Terminate,
    PauseAndAsk { timeout_secs: u64 },
}

/// 轨迹级检查 trait
pub trait TrajectoryCheck: Send + Sync {
    fn name(&self) -> &str;
    fn check(&self, steps: &[String]) -> Option<Violation>;
}

/// 凭据拼接检测
pub struct CredentialConcatenationCheck;

impl TrajectoryCheck for CredentialConcatenationCheck {
    fn name(&self) -> &str { "credential_concatenation" }

    fn check(&self, steps: &[String]) -> Option<Violation> {
        let patterns = ["sk-", "api_key", "token", "password", "secret", "bearer"];
        let mut suspicious = Vec::new();
        for (i, step) in steps.iter().enumerate() {
            let lower = step.to_lowercase();
            for pattern in &patterns {
                if lower.contains(pattern) && step.len() > 20 {
                    suspicious.push(format!("步骤 {i} 疑似包含凭据"));
                    break;
                }
            }
        }
        if suspicious.is_empty() {
            None
        } else {
            Some(Violation {
                check_name: "credential_concatenation".into(),
                severity: Severity::Critical,
                description: "检测到凭据/令牌拆分拼接重建".into(),
                evidence: suspicious,
            })
        }
    }
}

/// 沙箱逃逸检测
pub struct SandboxEscapeCheck {
    pub allowed_paths: Vec<String>,
}

impl TrajectoryCheck for SandboxEscapeCheck {
    fn name(&self) -> &str { "sandbox_escape" }

    fn check(&self, steps: &[String]) -> Option<Violation> {
        let mut violations = Vec::new();
        for (i, step) in steps.iter().enumerate() {
            if step.contains("../") || step.contains("..\\") {
                violations.push(format!("步骤 {i} 包含路径遍历"));
            }
            for pattern in &["/etc/", "/proc/", "/sys/", "C:\\Windows\\", "C:\\System"] {
                if step.contains(pattern) {
                    violations.push(format!("步骤 {i} 访问受限路径"));
                }
            }
        }
        if violations.is_empty() {
            None
        } else {
            Some(Violation {
                check_name: "sandbox_escape".into(),
                severity: Severity::Critical,
                description: "检测到沙箱逃逸行为".into(),
                evidence: violations,
            })
        }
    }
}

/// 资源耗尽检测
pub struct ResourceExhaustionCheck {
    pub max_repeated_tool_calls: usize,
}

impl TrajectoryCheck for ResourceExhaustionCheck {
    fn name(&self) -> &str { "resource_exhaustion" }

    fn check(&self, steps: &[String]) -> Option<Violation> {
        if steps.len() < self.max_repeated_tool_calls {
            return None;
        }
        let recent = &steps[steps.len() - self.max_repeated_tool_calls..];
        let first_tool = recent[0].clone();
        if recent.iter().all(|s| s == &first_tool) {
            Some(Violation {
                check_name: "resource_exhaustion".into(),
                severity: Severity::Medium,
                description: format!("循环调用同一工具 {} 次无进展", self.max_repeated_tool_calls),
                evidence: recent.to_vec(),
            })
        } else {
            None
        }
    }
}

/// 轨迹级护栏
pub struct TrajectoryGuardrail {
    pub checks: Vec<Box<dyn TrajectoryCheck>>,
    pub on_violation: ViolationHandler,
}

impl TrajectoryGuardrail {
    pub fn new(on_violation: ViolationHandler) -> Self {
        Self {
            checks: vec![
                Box::new(CredentialConcatenationCheck),
                Box::new(SandboxEscapeCheck { allowed_paths: vec![] }),
                Box::new(ResourceExhaustionCheck { max_repeated_tool_calls: 5 }),
            ],
            on_violation,
        }
    }

    pub fn check(&self, steps: &[String]) -> Vec<Violation> {
        let mut violations = Vec::new();
        for check in &self.checks {
            if let Some(v) = check.check(steps) {
                violations.push(v);
            }
        }
        violations
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_check_detects_tokens() {
        let check = CredentialConcatenationCheck;
        let steps = vec!["调用 API with sk-abc123def456ghi789jkl012mno345pqr".into()];
        let v = check.check(&steps);
        assert!(v.is_some());
        assert_eq!(v.unwrap().severity, Severity::Critical);
    }

    #[test]
    fn credential_check_passes_normal() {
        let check = CredentialConcatenationCheck;
        let steps = vec!["搜索关键词: rust async".into()];
        assert!(check.check(&steps).is_none());
    }
}
