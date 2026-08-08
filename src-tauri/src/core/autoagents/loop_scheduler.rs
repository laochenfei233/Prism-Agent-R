use serde::{Deserialize, Serialize};

use super::goal::{GoalMonitor, RecoveryAction, TaskGoal, WorkflowState};
use super::workflow::Workflow;

/// §17.2 Loop 自动化
///
/// Goal loop: 每轮执行工作流 → GoalMonitor 评估 → 未达标重试
/// Timer loop: 按 interval 触发工作流
/// Maker-Checker: maker 产出 → checker 独立评审 → 不通过则重做

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LoopKind {
    Goal,
    Timer,
    MakerChecker,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentLoop {
    pub id: String,
    pub kind: LoopKind,
    pub interval_secs: Option<u64>,
    pub max_rounds: u32,
    pub goal: Option<TaskGoal>,
    pub maker_workflow_id: Option<String>,
    pub checker_workflow_id: Option<String>,
    pub status: LoopStatus,
    pub current_round: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LoopStatus {
    Idle,
    Running,
    Paused,
    Completed,
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopRound {
    pub round: u32,
    pub status: String,
    pub output_summary: String,
    pub goal_achieved: Option<bool>,
    pub checker_verdict: Option<String>,
    pub started_at: i64,
    pub finished_at: Option<i64>,
}

/// Loop 运行结果
#[derive(Debug, Clone)]
pub struct LoopResult {
    pub status: LoopStatus,
    pub rounds: Vec<LoopRound>,
    pub final_output: String,
}

/// Loop 调度器（内存态，重启后丢失）
pub struct LoopScheduler {
    loops: std::sync::Mutex<Vec<AgentLoop>>,
}

impl LoopScheduler {
    pub fn new() -> Self {
        Self {
            loops: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// 创建新的 Loop
    pub fn create_loop(
        &self,
        kind: LoopKind,
        interval_secs: Option<u64>,
        max_rounds: u32,
        goal: Option<TaskGoal>,
        maker_workflow_id: Option<String>,
        checker_workflow_id: Option<String>,
    ) -> AgentLoop {
        let id = uuid::Uuid::new_v4().to_string();
        let loop_ = AgentLoop {
            id: id.clone(),
            kind,
            interval_secs,
            max_rounds,
            goal,
            maker_workflow_id,
            checker_workflow_id,
            status: LoopStatus::Idle,
            current_round: 0,
        };
        self.loops.lock().unwrap().push(loop_);
        self.get_loop(&id).unwrap()
    }

    /// 获取 Loop
    pub fn get_loop(&self, id: &str) -> Option<AgentLoop> {
        self.loops.lock().unwrap().iter().find(|l| l.id == id).cloned()
    }

    /// 列出所有 Loop
    pub fn list_loops(&self) -> Vec<AgentLoop> {
        self.loops.lock().unwrap().clone()
    }

    /// 停止 Loop
    pub fn stop_loop(&self, id: &str) -> bool {
        let mut loops = self.loops.lock().unwrap();
        if let Some(loop_) = loops.iter_mut().find(|l| l.id == id) {
            loop_.status = LoopStatus::Paused;
            true
        } else {
            false
        }
    }

    /// 删除 Loop
    pub fn remove_loop(&self, id: &str) -> bool {
        let mut loops = self.loops.lock().unwrap();
        let len = loops.len();
        loops.retain(|l| l.id != id);
        loops.len() < len
    }
}

impl Default for LoopScheduler {
    fn default() -> Self {
        Self::new()
    }
}

/// Goal Loop 执行逻辑
pub async fn run_goal_loop(
    loop_: &mut AgentLoop,
    goal_monitor: &GoalMonitor,
    executor: &dyn Fn() -> Result<String, String>,
) -> LoopResult {
    let mut rounds = Vec::new();
    let mut final_output = String::new();

    for round in 1..=loop_.max_rounds {
        loop_.current_round = round;
        loop_.status = LoopStatus::Running;

        let started_at = chrono::Utc::now().timestamp();
        let mut round_data = LoopRound {
            round,
            status: "running".into(),
            output_summary: String::new(),
            goal_achieved: None,
            checker_verdict: None,
            started_at,
            finished_at: None,
        };

        // 执行工作流
        match executor() {
            Ok(output) => {
                final_output = output.clone();
                round_data.output_summary = truncate(&output, 200);
                round_data.status = "completed".into();

                // 评估目标
                let state = WorkflowState {
                    accumulated_text: output,
                    ..Default::default()
                };
                let status = goal_monitor.evaluate(&state);
                round_data.goal_achieved = Some(status.achieved);

                if status.achieved {
                    loop_.status = LoopStatus::Completed;
                    round_data.finished_at = Some(chrono::Utc::now().timestamp());
                    rounds.push(round_data);
                    return LoopResult {
                        status: LoopStatus::Completed,
                        rounds,
                        final_output,
                    };
                }

                // 根据偏离程度决定恢复动作
                match goal_monitor.on_drift(&status) {
                    RecoveryAction::Continue => {}
                    RecoveryAction::Replan(msg) => {
                        tracing::info!("Goal loop round {round}: replan - {msg}");
                    }
                    RecoveryAction::EscalateToUser(msg) => {
                        loop_.status = LoopStatus::Failed(msg.clone());
                        round_data.status = "escalated".into();
                        round_data.finished_at = Some(chrono::Utc::now().timestamp());
                        rounds.push(round_data);
                        return LoopResult {
                            status: LoopStatus::Failed(msg),
                            rounds,
                            final_output,
                        };
                    }
                }
            }
            Err(e) => {
                round_data.status = "failed".into();
                round_data.output_summary = e.clone();
                tracing::warn!("Goal loop round {round} failed: {e}");
            }
        }

        round_data.finished_at = Some(chrono::Utc::now().timestamp());
        rounds.push(round_data);
    }

    // 达到最大轮次
    loop_.status = LoopStatus::Completed;
    LoopResult {
        status: LoopStatus::Completed,
        rounds,
        final_output,
    }
}

/// Maker-Checker Loop 执行逻辑
pub async fn run_maker_checker_loop(
    loop_: &mut AgentLoop,
    maker_executor: &dyn Fn() -> Result<String, String>,
    checker_executor: &dyn Fn(&str) -> Result<String, String>,
) -> LoopResult {
    let mut rounds = Vec::new();
    let mut final_output = String::new();

    for round in 1..=loop_.max_rounds {
        loop_.current_round = round;
        loop_.status = LoopStatus::Running;

        let started_at = chrono::Utc::now().timestamp();
        let mut round_data = LoopRound {
            round,
            status: "running".into(),
            output_summary: String::new(),
            goal_achieved: None,
            checker_verdict: None,
            started_at,
            finished_at: None,
        };

        // Maker 阶段
        let maker_output = match maker_executor() {
            Ok(output) => output,
            Err(e) => {
                round_data.status = "maker_failed".into();
                round_data.output_summary = e;
                round_data.finished_at = Some(chrono::Utc::now().timestamp());
                rounds.push(round_data);
                continue;
            }
        };

        // Checker 阶段
        match checker_executor(&maker_output) {
            Ok(verdict) => {
                round_data.checker_verdict = Some(verdict.clone());
                final_output = maker_output;

                if verdict.to_lowercase().contains("pass") || verdict.to_lowercase().contains("通过") {
                    loop_.status = LoopStatus::Completed;
                    round_data.status = "approved".into();
                    round_data.goal_achieved = Some(true);
                    round_data.finished_at = Some(chrono::Utc::now().timestamp());
                    rounds.push(round_data);
                    return LoopResult {
                        status: LoopStatus::Completed,
                        rounds,
                        final_output,
                    };
                } else {
                    // 不通过，下一轮重做
                    round_data.status = "rejected".into();
                    round_data.goal_achieved = Some(false);
                }
            }
            Err(e) => {
                round_data.status = "checker_failed".into();
                round_data.output_summary = e;
            }
        }

        round_data.finished_at = Some(chrono::Utc::now().timestamp());
        rounds.push(round_data);
    }

    loop_.status = LoopStatus::Completed;
    LoopResult {
        status: LoopStatus::Completed,
        rounds,
        final_output,
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.len() <= max_chars {
        s.to_string()
    } else {
        format!("{}...", &s[..max_chars])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loop_kind_serialize() {
        let kind = LoopKind::Goal;
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, "\"Goal\"");
    }

    #[test]
    fn test_loop_status_serialize() {
        let status = LoopStatus::Running;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"Running\"");
    }

    #[test]
    fn test_loop_scheduler_crud() {
        let scheduler = LoopScheduler::new();
        let loop_ = scheduler.create_loop(
            LoopKind::Goal,
            None,
            5,
            None,
            None,
            None,
        );
        assert_eq!(scheduler.list_loops().len(), 1);
        assert_eq!(scheduler.get_loop(&loop_.id).unwrap().status, LoopStatus::Idle);

        scheduler.stop_loop(&loop_.id);
        assert_eq!(scheduler.get_loop(&loop_.id).unwrap().status, LoopStatus::Paused);

        scheduler.remove_loop(&loop_.id);
        assert_eq!(scheduler.list_loops().len(), 0);
    }
}
