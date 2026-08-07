use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ── 目标定义（§10.11） ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGoal {
    pub description: String,
    pub criteria: Vec<GoalCriterion>,
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalCriterion {
    pub metric: String,
    pub operator: CriterionOp,
    pub value: serde_json::Value,
    pub weight: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CriterionOp {
    Gt,
    Lt,
    Eq,
    Contains,
    NotContains,
    RegexMatch,
    LlmJudge,
}

// ── 目标状态 ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalStatus {
    pub achieved: bool,
    pub score: f32,
    pub details: Vec<CriterionStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriterionStatus {
    pub metric: String,
    pub passed: bool,
    pub weight: f32,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryAction {
    Continue,
    Replan(String),
    EscalateToUser(String),
}

// ── 目标监控器 ────────────────────────────────────────────

/// 运行时监控：对工作流状态快照评估各准则，权重加权后判定目标达成。
/// 达成阈值 80%（对齐设计文档）。
pub struct GoalMonitor {
    pub goals: Vec<TaskGoal>,
}

const ACHIEVED_THRESHOLD: f32 = 0.8;

/// 工作流状态快照：各阶段输出 + 汇总文本（供准则评估）
#[derive(Debug, Clone, Default)]
pub struct WorkflowState {
    /// stage_id → 该阶段输出
    pub stage_outputs: HashMap<String, String>,
    /// 当前累计产出（全文拼接）
    pub accumulated_text: String,
}

impl GoalMonitor {
    pub fn new(goals: Vec<TaskGoal>) -> Self {
        Self { goals }
    }

    /// 评估当前状态是否满足目标（按权重加权）
    pub fn evaluate(&self, state: &WorkflowState) -> GoalStatus {
        let mut total_score = 0.0f32;
        let mut total_weight = 0.0f32;
        let mut details = Vec::new();

        for goal in &self.goals {
            for criterion in &goal.criteria {
                let (passed, detail) = self.evaluate_criterion(criterion, state);
                let weighted = if passed { criterion.weight } else { 0.0 };
                total_score += weighted;
                total_weight += criterion.weight;
                details.push(CriterionStatus {
                    metric: criterion.metric.clone(),
                    passed,
                    weight: criterion.weight,
                    detail,
                });
            }
        }

        let score = if total_weight > 0.0 { total_score / total_weight } else { 0.0 };
        GoalStatus {
            achieved: score >= ACHIEVED_THRESHOLD,
            score,
            details,
        }
    }

    /// 偏离目标时触发恢复动作（对齐 §10.11 阈值）
    pub fn on_drift(&self, status: &GoalStatus) -> RecoveryAction {
        if status.score < 0.3 {
            RecoveryAction::EscalateToUser("目标严重偏离，建议人工介入".into())
        } else if status.score < 0.6 {
            RecoveryAction::Replan("目标部分达成，尝试调整策略".into())
        } else {
            RecoveryAction::Continue
        }
    }

    fn evaluate_criterion(&self, criterion: &GoalCriterion, state: &WorkflowState) -> (bool, String) {
        let text = &state.accumulated_text;
        match criterion.operator {
            CriterionOp::Contains => {
                let needle = criterion.value.as_str().unwrap_or_default();
                (text.contains(needle), format!("包含「{needle}」"))
            }
            CriterionOp::NotContains => {
                let needle = criterion.value.as_str().unwrap_or_default();
                (!text.contains(needle), format!("不含「{needle}」"))
            }
            CriterionOp::Gt => {
                let target = criterion.value.as_u64().unwrap_or(0) as usize;
                let actual = state.stage_outputs.len();
                (actual > target, format!("阶段数 {actual} > {target}"))
            }
            CriterionOp::Lt => {
                let target = criterion.value.as_u64().unwrap_or(0) as usize;
                let actual = text.chars().count();
                (actual < target, format!("长度 {actual} < {target}"))
            }
            CriterionOp::Eq => {
                let target = criterion.value.as_u64().unwrap_or(0) as usize;
                let actual = state.stage_outputs.len();
                (actual == target, format!("阶段数 {actual} == {target}"))
            }
            CriterionOp::RegexMatch => {
                let pattern = criterion.value.as_str().unwrap_or_default();
                let re = regex::Regex::new(pattern).map_err(|e| e.to_string());
                match re {
                    Ok(re) => (re.is_match(text), format!("匹配 /{pattern}/")),
                    Err(e) => (false, format!("正则无效: {e}")),
                }
            }
            CriterionOp::LlmJudge => {
                // LLM 判定由外部调用方注入结果（见 judge 模块）；此处默认未评估
                (false, "LLM 判定待外部评估".into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_goal() -> TaskGoal {
        TaskGoal {
            description: "生成一份包含摘要和正文的研究报告".into(),
            criteria: vec![
                GoalCriterion {
                    metric: "contains_summary".into(),
                    operator: CriterionOp::Contains,
                    value: serde_json::json!("摘要"),
                    weight: 0.4,
                },
                GoalCriterion {
                    metric: "contains_body".into(),
                    operator: CriterionOp::Contains,
                    value: serde_json::json!("正文"),
                    weight: 0.4,
                },
                GoalCriterion {
                    metric: "min_stages".into(),
                    operator: CriterionOp::Gt,
                    value: serde_json::json!(1),
                    weight: 0.2,
                },
            ],
            timeout_secs: Some(300),
        }
    }

    #[test]
    fn achieved_when_all_met() {
        let monitor = GoalMonitor::new(vec![sample_goal()]);
        let state = WorkflowState {
            accumulated_text: "这是摘要部分，正文部分完整。".into(),
            stage_outputs: HashMap::from([("a".into(), "1".into()), ("b".into(), "2".into())]),
        };
        let status = monitor.evaluate(&state);
        assert!(status.achieved);
        assert!(status.score >= 0.8);
    }

    #[test]
    fn not_achieved_when_missing() {
        let monitor = GoalMonitor::new(vec![sample_goal()]);
        // 含"摘要"但缺"正文"且阶段数不足 → 0.4/1.0 = 0.4 → Replan
        let state = WorkflowState {
            accumulated_text: "这是摘要部分，内容完整。".into(),
            stage_outputs: HashMap::from([("a".into(), "1".into())]),
        };
        let status = monitor.evaluate(&state);
        assert!(!status.achieved);
        assert!(matches!(monitor.on_drift(&status), RecoveryAction::Replan(_)));
    }

    #[test]
    fn escalate_on_severe_drift() {
        let monitor = GoalMonitor::new(vec![sample_goal()]);
        let state = WorkflowState::default();
        let status = monitor.evaluate(&state);
        assert!(matches!(monitor.on_drift(&status), RecoveryAction::EscalateToUser(_)));
    }
}
