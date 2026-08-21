use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::core::adk::error::AgentError;
use crate::core::adk::model::{
    ChatMessage, ChatRole, GenerationRequest, MessageContent, ModelProvider,
};

// ── LLM-as-Judge（§10.13.2） ───────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeResult {
    pub score: f32,
    pub rationale: String,
    pub criteria_scores: std::collections::HashMap<String, f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonResult {
    pub winner: Winner,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Winner {
    A,
    B,
    Tie,
}

pub struct AgentJudge {
    model: Arc<dyn ModelProvider>,
}

impl AgentJudge {
    pub fn new(model: Arc<dyn ModelProvider>) -> Self {
        Self { model }
    }

    /// 评估 Agent 输出质量：LLM 返回 JSON score(1-5) + rationale + criteria_scores
    pub async fn evaluate(
        &self,
        task: &str,
        output: &str,
        criteria: &[String],
    ) -> Result<JudgeResult, AgentError> {
        let prompt = format!(
            "你是一个 AI 输出质量评审员。\n\n任务: {task}\n\n输出:\n{output}\n\n评估标准: {}\n\n\
             只返回 JSON，不要其他文本：{{\"score\": 1-5, \"rationale\": \"...\", \"criteria_scores\": {{\"标准名\": 1-5}}}}",
            criteria.join(", ")
        );
        let resp = self
            .model
            .generate(GenerationRequest {
                messages: vec![ChatMessage {
                    role: ChatRole::User,
                    content: MessageContent::Text(prompt),
                    name: None,
                }],
                temperature: Some(0.0),
                max_tokens: Some(512),
                ..Default::default()
            })
            .await?;
        Ok(parse_judge_json(&resp.text))
    }

    /// 比较两个 Agent 版本的输出质量
    pub async fn compare(
        &self,
        task: &str,
        output_a: &str,
        output_b: &str,
        criteria: &[String],
    ) -> Result<ComparisonResult, AgentError> {
        let prompt = format!(
            "你是一个 AI 输出质量评审员。比较两个版本输出，指出哪个更好。\n\n\
             任务: {task}\n\n评估标准: {}\n\n版本 A:\n{output_a}\n\n版本 B:\n{output_b}\n\n\
             只返回 JSON：{{\"winner\": \"A\" | \"B\" | \"tie\", \"rationale\": \"...\"}}",
            criteria.join(", ")
        );
        let resp = self
            .model
            .generate(GenerationRequest {
                messages: vec![ChatMessage {
                    role: ChatRole::User,
                    content: MessageContent::Text(prompt),
                    name: None,
                }],
                temperature: Some(0.0),
                max_tokens: Some(256),
                ..Default::default()
            })
            .await?;
        Ok(parse_compare_json(&resp.text))
    }
}

// ── JSON 解析（宽松） ─────────────────────────────────────

fn extract_json(text: &str) -> Option<serde_json::Value> {
    let cleaned = text.trim();
    let body = if cleaned.starts_with("```") {
        let end = cleaned.rfind("```").unwrap_or(cleaned.len());
        &cleaned[cleaned.find('\n').map(|i| i + 1).unwrap_or(0)..end]
    } else {
        cleaned
    };
    let start = body.find('{')?;
    let stop = body[start..].rfind('}')? + start;
    serde_json::from_str(&body[start..=stop]).ok()
}

fn parse_judge_json(text: &str) -> JudgeResult {
    let mut result = JudgeResult {
        score: 0.0,
        rationale: String::new(),
        criteria_scores: Default::default(),
    };
    if let Some(v) = extract_json(text) {
        result.score = v.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0) as f32;
        result.rationale = v
            .get("rationale")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();
        if let Some(cs) = v.get("criteria_scores").and_then(|c| c.as_object()) {
            for (k, val) in cs {
                if let Some(n) = val.as_f64() {
                    result.criteria_scores.insert(k.clone(), n as f32);
                }
            }
        }
    }
    result
}

fn parse_compare_json(text: &str) -> ComparisonResult {
    let mut result = ComparisonResult {
        winner: Winner::Tie,
        rationale: String::new(),
    };
    if let Some(v) = extract_json(text) {
        result.winner = match v.get("winner").and_then(|w| w.as_str()) {
            Some("A") | Some("a") => Winner::A,
            Some("B") | Some("b") => Winner::B,
            _ => Winner::Tie,
        };
        result.rationale = v
            .get("rationale")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();
    }
    result
}

// ── 轨迹统计（§10.13.3 agent:stats 数据源） ────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentStats {
    pub total_runs: usize,
    pub success_runs: usize,
    pub success_rate: f32,
    pub avg_latency_ms: f64,
    pub total_tokens: u64,
    pub output_input_ratio: f32,
    pub tool_usage: std::collections::HashMap<String, usize>,
    pub failure_reasons: std::collections::HashMap<String, usize>,
    pub avg_reflection_iterations: f32,
}

/// 由轨迹列表聚合统计指标
pub fn aggregate_stats(traces: &[crate::data::services::trace_service::AgentTrace]) -> AgentStats {
    let mut stats = AgentStats::default();
    let total = traces.len();
    if total == 0 {
        return stats;
    }
    stats.total_runs = total;

    let mut latency_sum = 0u64;
    let mut input_tokens = 0i64;
    let mut output_tokens = 0i64;
    let mut reflection_steps = 0u32;
    let mut reflection_runs = 0usize;

    for t in traces {
        if t.outcome == "success" {
            stats.success_runs += 1;
        } else {
            *stats.failure_reasons.entry(t.outcome.clone()).or_insert(0) += 1;
        }
        input_tokens += t.total_prompt_tokens;
        output_tokens += t.total_completion_tokens;
        stats.total_tokens += (t.total_prompt_tokens + t.total_completion_tokens) as u64;
        for step in &t.steps {
            latency_sum += step.latency_ms;
            if let Some(tool) = &step.tool_name {
                *stats.tool_usage.entry(tool.clone()).or_insert(0) += 1;
            }
            if step.kind == "reflection" {
                reflection_steps += 1;
                reflection_runs += 1;
            }
        }
    }

    stats.success_rate = stats.success_runs as f32 / total as f32;
    stats.avg_latency_ms = latency_sum as f64 / total as f64;
    if input_tokens > 0 {
        stats.output_input_ratio = output_tokens as f32 / input_tokens as f32;
    }
    if reflection_runs > 0 {
        stats.avg_reflection_iterations = reflection_steps as f32 / reflection_runs as f32;
    }
    stats
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_judge_json_valid() {
        let r = parse_judge_json(
            r#"{"score": 4, "rationale": "不错", "criteria_scores": {"准确性": 4, "完整性": 3}}"#,
        );
        assert_eq!(r.score, 4.0);
        assert_eq!(r.rationale, "不错");
        assert_eq!(r.criteria_scores["准确性"], 4.0);
    }

    #[test]
    fn parse_judge_json_fenced() {
        let r = parse_judge_json("```json\n{\"score\": 3.5}\n```");
        assert_eq!(r.score, 3.5);
    }

    #[test]
    fn compare_json_parsed() {
        let r = parse_compare_json(r#"{"winner": "A", "rationale": "A 更详细"}"#);
        assert_eq!(r.winner, Winner::A);
    }

    #[test]
    fn aggregate_empty() {
        let s = aggregate_stats(&[]);
        assert_eq!(s.total_runs, 0);
    }
}
