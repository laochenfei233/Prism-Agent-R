use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use super::config::{AgentBudget, BudgetConfig, CrewBudget};
use super::policy::{BudgetAction, BudgetPolicy};

// ── 预算状态 ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalBudgetState {
    pub daily_tokens_used: u64,
    pub daily_cost_used: f64,
    pub monthly_cost_used: f64,
    pub active_workflows: u32,
    pub last_reset: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrewBudgetState {
    pub tokens_used: u64,
    pub cost_used: f64,
    pub start_time: i64,
    pub iterations: u32,
    pub requests_made: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentBudgetState {
    pub tokens_used: u64,
    pub iterations: u32,
    pub start_time: i64,
    pub retry_count: u32,
}

// ── 预算检查结果 ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BudgetCheckResult {
    Ok,
    Warning {
        level: String,
        current: f64,
        limit: f64,
    },
    Exceeded {
        level: String,
        action: BudgetAction,
    },
}

// ── 预算追踪器 ────────────────────────────────────────────

pub struct BudgetTracker {
    config: BudgetConfig,
    policy: BudgetPolicy,
    global: Arc<RwLock<GlobalBudgetState>>,
    crews: RwLock<HashMap<String, CrewBudgetState>>,
    agents: RwLock<HashMap<String, AgentBudgetState>>,
    on_event: Option<Arc<dyn Fn(BudgetEvent) + Send + Sync>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetEvent {
    pub event_type: String,
    pub level: String,
    pub entity_type: String,
    pub entity_id: String,
    pub current: Option<f64>,
    pub limit: Option<f64>,
    pub action: Option<BudgetAction>,
    pub message: Option<String>,
    pub timestamp: i64,
}

impl BudgetTracker {
    pub fn new(config: BudgetConfig, policy: BudgetPolicy) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            config,
            policy,
            global: Arc::new(RwLock::new(GlobalBudgetState {
                daily_tokens_used: 0,
                daily_cost_used: 0.0,
                monthly_cost_used: 0.0,
                active_workflows: 0,
                last_reset: now,
            })),
            crews: RwLock::new(HashMap::new()),
            agents: RwLock::new(HashMap::new()),
            on_event: None,
        }
    }

    pub fn on_event<F>(mut self, f: F) -> Self
    where
        F: Fn(BudgetEvent) + Send + Sync + 'static,
    {
        self.on_event = Some(Arc::new(f));
        self
    }

    fn emit_event(&self, event: BudgetEvent) {
        if let Some(f) = &self.on_event {
            f(event);
        }
    }

    // ── 全局预算 ──────────────────────────────────────────

    pub async fn check_global_budget(&self) -> Result<BudgetCheckResult, String> {
        let global = self.global.read().await;

        // Daily token check
        if let Some(limit) = self.config.global.daily_token_limit {
            if global.daily_tokens_used >= limit {
                let action = self.policy.on_token_exceeded.clone();
                self.emit_event(BudgetEvent {
                    event_type: "exceeded".into(),
                    level: "global".into(),
                    entity_type: "global".into(),
                    entity_id: "global".into(),
                    current: Some(global.daily_tokens_used as f64),
                    limit: Some(limit as f64),
                    action: Some(action.clone()),
                    message: Some(format!(
                        "每日 token 已超限: {}/{}",
                        global.daily_tokens_used, limit
                    )),
                    timestamp: chrono::Utc::now().timestamp_millis(),
                });
                return Ok(BudgetCheckResult::Exceeded {
                    level: "daily_tokens".into(),
                    action,
                });
            }
            let pct = global.daily_tokens_used as f64 / limit as f64;
            if pct >= 0.8 {
                self.emit_event(BudgetEvent {
                    event_type: "warning".into(),
                    level: "global".into(),
                    entity_type: "global".into(),
                    entity_id: "global".into(),
                    current: Some(global.daily_tokens_used as f64),
                    limit: Some(limit as f64),
                    action: None,
                    message: Some(format!("Token 使用量已达 {}%", (pct * 100.0) as u32)),
                    timestamp: chrono::Utc::now().timestamp_millis(),
                });
            }
        }

        // Daily cost check
        if let Some(limit) = self.config.global.daily_cost_limit {
            if global.daily_cost_used >= limit {
                let action = self.policy.on_cost_exceeded.clone();
                self.emit_event(BudgetEvent {
                    event_type: "exceeded".into(),
                    level: "global".into(),
                    entity_type: "global".into(),
                    entity_id: "global".into(),
                    current: Some(global.daily_cost_used),
                    limit: Some(limit),
                    action: Some(action.clone()),
                    message: Some(format!(
                        "每日费用已超限: ${:.2}/${:.2}",
                        global.daily_cost_used, limit
                    )),
                    timestamp: chrono::Utc::now().timestamp_millis(),
                });
                return Ok(BudgetCheckResult::Exceeded {
                    level: "daily_cost".into(),
                    action,
                });
            }
            let pct = global.daily_cost_used / limit;
            if pct >= 0.8 {
                self.emit_event(BudgetEvent {
                    event_type: "warning".into(),
                    level: "global".into(),
                    entity_type: "global".into(),
                    entity_id: "global".into(),
                    current: Some(global.daily_cost_used),
                    limit: Some(limit),
                    action: None,
                    message: Some(format!("费用使用量已达 {}%", (pct * 100.0) as u32)),
                    timestamp: chrono::Utc::now().timestamp_millis(),
                });
            }
        }

        Ok(BudgetCheckResult::Ok)
    }

    // ── 工作流级预算 ──────────────────────────────────────

    pub async fn create_crew_budget(
        &self,
        run_id: &str,
        _config: &CrewBudget,
    ) -> Result<(), String> {
        let mut crews = self.crews.write().await;
        crews.insert(
            run_id.to_string(),
            CrewBudgetState {
                tokens_used: 0,
                cost_used: 0.0,
                start_time: chrono::Utc::now().timestamp(),
                iterations: 0,
                requests_made: Vec::new(),
            },
        );
        let mut global = self.global.write().await;
        global.active_workflows += 1;
        Ok(())
    }

    pub async fn check_crew_budget(&self, run_id: &str, config: &CrewBudget) -> BudgetCheckResult {
        let crews = self.crews.read().await;
        let Some(state) = crews.get(run_id) else {
            return BudgetCheckResult::Ok;
        };

        // Token check
        if let Some(limit) = config.max_tokens {
            if state.tokens_used >= limit {
                return BudgetCheckResult::Exceeded {
                    level: "crew_tokens".into(),
                    action: self.policy.on_token_exceeded.clone(),
                };
            }
            let pct = state.tokens_used as f64 / limit as f64;
            if pct >= 0.8 {
                self.emit_event(BudgetEvent {
                    event_type: "warning".into(),
                    level: "crew".into(),
                    entity_type: "crew".into(),
                    entity_id: run_id.into(),
                    current: Some(state.tokens_used as f64),
                    limit: Some(limit as f64),
                    action: None,
                    message: Some(format!("工作流 token 已达 {}%", (pct * 100.0) as u32)),
                    timestamp: chrono::Utc::now().timestamp_millis(),
                });
            }
        }

        // Cost check
        if let Some(limit) = config.max_cost {
            if state.cost_used >= limit {
                return BudgetCheckResult::Exceeded {
                    level: "crew_cost".into(),
                    action: self.policy.on_cost_exceeded.clone(),
                };
            }
        }

        // Time check
        if let Some(limit) = config.max_execution_time_secs {
            let elapsed = chrono::Utc::now().timestamp() - state.start_time;
            if elapsed >= limit as i64 {
                return BudgetCheckResult::Exceeded {
                    level: "crew_time".into(),
                    action: self.policy.on_time_exceeded.clone(),
                };
            }
        }

        // Iteration check
        if let Some(limit) = config.max_iterations {
            if state.iterations >= limit {
                return BudgetCheckResult::Exceeded {
                    level: "crew_iterations".into(),
                    action: self.policy.on_iteration_exceeded.clone(),
                };
            }
        }

        // RPM check
        if let Some(limit) = config.max_rpm {
            let now = chrono::Utc::now().timestamp_millis();
            let window = state
                .requests_made
                .iter()
                .filter(|&&t| now - t < 60_000)
                .count() as u32;
            if window >= limit {
                return BudgetCheckResult::Exceeded {
                    level: "crew_rpm".into(),
                    action: self.policy.on_rpm_exceeded.clone(),
                };
            }
        }

        BudgetCheckResult::Ok
    }

    pub async fn record_crew_usage(&self, run_id: &str, tokens: u64, cost: f64) {
        let mut crews = self.crews.write().await;
        if let Some(state) = crews.get_mut(run_id) {
            state.tokens_used += tokens;
            state.cost_used += cost;
            state
                .requests_made
                .push(chrono::Utc::now().timestamp_millis());
        }

        let mut global = self.global.write().await;
        global.daily_tokens_used += tokens;
        global.daily_cost_used += cost;
        global.monthly_cost_used += cost;
    }

    pub async fn record_crew_iteration(&self, run_id: &str) {
        let mut crews = self.crews.write().await;
        if let Some(state) = crews.get_mut(run_id) {
            state.iterations += 1;
        }
    }

    pub async fn complete_crew(&self, run_id: &str) {
        let mut crews = self.crews.write().await;
        crews.remove(run_id);
        let mut global = self.global.write().await;
        if global.active_workflows > 0 {
            global.active_workflows -= 1;
        }
    }

    // ── Agent 级预算 ──────────────────────────────────────

    pub async fn check_agent_budget(
        &self,
        agent_id: &str,
        config: &AgentBudget,
    ) -> BudgetCheckResult {
        let agents = self.agents.read().await;
        let Some(state) = agents.get(agent_id) else {
            return BudgetCheckResult::Ok;
        };

        if let Some(limit) = config.max_tokens {
            if state.tokens_used >= limit {
                return BudgetCheckResult::Exceeded {
                    level: "agent_tokens".into(),
                    action: self.policy.on_token_exceeded.clone(),
                };
            }
        }

        if let Some(limit) = config.max_iterations {
            if state.iterations >= limit {
                return BudgetCheckResult::Exceeded {
                    level: "agent_iterations".into(),
                    action: self.policy.on_iteration_exceeded.clone(),
                };
            }
        }

        if let Some(limit) = config.max_execution_time_secs {
            let elapsed = chrono::Utc::now().timestamp() - state.start_time;
            if elapsed >= limit as i64 {
                return BudgetCheckResult::Exceeded {
                    level: "agent_time".into(),
                    action: self.policy.on_time_exceeded.clone(),
                };
            }
        }

        BudgetCheckResult::Ok
    }

    pub async fn record_agent_usage(&self, agent_id: &str, tokens: u64) {
        let mut agents = self.agents.write().await;
        let state = agents
            .entry(agent_id.to_string())
            .or_insert_with(|| AgentBudgetState {
                tokens_used: 0,
                iterations: 0,
                start_time: chrono::Utc::now().timestamp(),
                retry_count: 0,
            });
        state.tokens_used += tokens;
    }

    pub async fn record_agent_iteration(&self, agent_id: &str) {
        let mut agents = self.agents.write().await;
        if let Some(state) = agents.get_mut(agent_id) {
            state.iterations += 1;
        }
    }

    // ── 快照 ──────────────────────────────────────────────

    pub async fn snapshot(&self) -> GlobalBudgetState {
        self.global.read().await.clone()
    }

    pub async fn crew_states(&self) -> HashMap<String, CrewBudgetState> {
        self.crews.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn default_budget_tracker_ok() {
        let tracker = BudgetTracker::new(BudgetConfig::default(), BudgetPolicy::default());
        let result = tracker.check_global_budget().await.unwrap();
        assert!(matches!(result, BudgetCheckResult::Ok));
    }

    #[tokio::test]
    async fn crew_budget_tracking() {
        let tracker = BudgetTracker::new(BudgetConfig::default(), BudgetPolicy::default());
        let config = CrewBudget {
            max_tokens: Some(100),
            ..Default::default()
        };
        tracker.create_crew_budget("run1", &config).await.unwrap();
        tracker.record_crew_usage("run1", 50, 0.01).await;
        let result = tracker.check_crew_budget("run1", &config).await;
        assert!(matches!(result, BudgetCheckResult::Ok));
        tracker.record_crew_usage("run1", 60, 0.01).await;
        let result = tracker.check_crew_budget("run1", &config).await;
        assert!(matches!(result, BudgetCheckResult::Exceeded { .. }));
    }
}
