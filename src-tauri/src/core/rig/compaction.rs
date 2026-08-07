use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBudget {
    pub compaction_auto: bool,
    pub compaction_prune: bool,
    pub compaction_tail_turns: usize,
    pub compaction_preserve_recent: usize,
    pub compaction_reserved: usize,
    pub inject_checkpoint: usize,
    pub inject_memory: usize,
    pub inject_global: usize,
    pub inject_notes: usize,
    pub inject_recent_user: usize,
    pub inject_recent_user_per_msg: usize,
    pub inject_tasks_ledger: usize,
    pub inject_actor_ledger: usize,
    pub inject_memory_titles: usize,
}

impl Default for TokenBudget {
    fn default() -> Self {
        Self {
            compaction_auto: true,
            compaction_prune: true,
            compaction_tail_turns: 2,
            compaction_preserve_recent: 4000,
            compaction_reserved: 20_000,
            inject_checkpoint: 11_000,
            inject_memory: 10_000,
            inject_global: 6_000,
            inject_notes: 6_000,
            inject_recent_user: 16_000,
            inject_recent_user_per_msg: 2_000,
            inject_tasks_ledger: 2_000,
            inject_actor_ledger: 500,
            inject_memory_titles: 500,
        }
    }
}

pub fn estimate_tokens(text: &str) -> usize {
    text.len() / 4
}

pub fn pressure_level(used: usize, limit: usize) -> u8 {
    let ratio = used as f64 / limit as f64;
    if ratio < 0.50 { 0 }
    else if ratio < 0.70 { 1 }
    else if ratio < 0.85 { 2 }
    else { 3 }
}

pub fn soft_trim(output: &str) -> String {
    const THRESHOLD: usize = 4096;
    const KEEP: usize = 1536;
    if output.len() <= THRESHOLD {
        return output.to_string();
    }
    let head: String = output.chars().take(KEEP).collect();
    let tail: String = output.chars().rev().take(KEEP).collect::<Vec<_>>().into_iter().rev().collect();
    format!("{head}[... trimmed ...]{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_roughly() {
        assert!(estimate_tokens("hello world") > 0);
        assert_eq!(pressure_level(100, 1000), 0);
        assert_eq!(pressure_level(600, 1000), 1);
        assert_eq!(pressure_level(800, 1000), 2);
        assert_eq!(pressure_level(950, 1000), 3);
    }

    #[test]
    fn soft_trim_keeps_short() {
        assert_eq!(soft_trim("short"), "short");
    }

    #[test]
    fn soft_trim_truncates_long() {
        let long = "x".repeat(10_000);
        let trimmed = soft_trim(&long);
        assert!(trimmed.len() < long.len());
        assert!(trimmed.contains("trimmed"));
    }
}
