use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BudgetAction {
    Continue,
    Warn { message: String },
    DowngradeModel,
    PauseAndAsk,
    Terminate,
    SkipStageAndContinue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetPolicy {
    pub on_token_warning: BudgetAction,
    pub on_token_exceeded: BudgetAction,
    pub on_cost_warning: BudgetAction,
    pub on_cost_exceeded: BudgetAction,
    pub on_time_exceeded: BudgetAction,
    pub on_rpm_exceeded: BudgetAction,
    pub on_iteration_exceeded: BudgetAction,
}

impl Default for BudgetPolicy {
    fn default() -> Self {
        Self {
            on_token_warning: BudgetAction::Warn { message: "Token 使用量接近上限".into() },
            on_token_exceeded: BudgetAction::DowngradeModel,
            on_cost_warning: BudgetAction::Warn { message: "费用接近上限".into() },
            on_cost_exceeded: BudgetAction::PauseAndAsk,
            on_time_exceeded: BudgetAction::Terminate,
            on_rpm_exceeded: BudgetAction::Warn { message: "请求频率过高，等待窗口重置".into() },
            on_iteration_exceeded: BudgetAction::Terminate,
        }
    }
}
