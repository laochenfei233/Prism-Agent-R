use serde::{Deserialize, Serialize};

/// 三级预算体系
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetConfig {
    pub global: GlobalBudget,
    pub crew: CrewBudget,
    pub agent: AgentBudget,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalBudget {
    pub daily_token_limit: Option<u64>,
    pub daily_cost_limit: Option<f64>,
    pub monthly_cost_limit: Option<f64>,
    pub max_concurrent_workflows: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrewBudget {
    pub max_tokens: Option<u64>,
    pub max_cost: Option<f64>,
    pub max_execution_time_secs: Option<u64>,
    pub max_iterations: Option<u32>,
    pub max_rpm: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentBudget {
    pub max_tokens: Option<u64>,
    pub max_iterations: Option<u32>,
    pub max_execution_time_secs: Option<u64>,
    pub max_retry_limit: Option<u32>,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            global: GlobalBudget {
                daily_token_limit: Some(1_000_000),
                daily_cost_limit: Some(10.0),
                monthly_cost_limit: Some(200.0),
                max_concurrent_workflows: 4,
            },
            crew: CrewBudget {
                max_tokens: Some(100_000),
                max_cost: Some(5.0),
                max_execution_time_secs: Some(600),
                max_iterations: Some(20),
                max_rpm: Some(60),
            },
            agent: AgentBudget {
                max_tokens: Some(50_000),
                max_iterations: Some(20),
                max_execution_time_secs: Some(300),
                max_retry_limit: Some(2),
            },
        }
    }
}

impl Default for GlobalBudget {
    fn default() -> Self {
        Self {
            daily_token_limit: Some(1_000_000),
            daily_cost_limit: Some(10.0),
            monthly_cost_limit: Some(200.0),
            max_concurrent_workflows: 4,
        }
    }
}

impl Default for CrewBudget {
    fn default() -> Self {
        Self {
            max_tokens: Some(100_000),
            max_cost: Some(5.0),
            max_execution_time_secs: Some(600),
            max_iterations: Some(20),
            max_rpm: Some(60),
        }
    }
}

impl Default for AgentBudget {
    fn default() -> Self {
        Self {
            max_tokens: Some(50_000),
            max_iterations: Some(20),
            max_execution_time_secs: Some(300),
            max_retry_limit: Some(2),
        }
    }
}
