use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::core::adk::error::AgentError;
use crate::core::adk::model::{
    ChatMessage, ChatRole, GenerationRequest, MessageContent, ModelProvider,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectionConfig {
    pub enabled: bool,
    pub max_iterations: u32,
    pub reviewer_prompt: String,
    pub stop_condition: StopCondition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StopCondition {
    ScoreAbove(f32),
    NoChanges,
    MaxIterations,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectionResult {
    pub text: String,
    pub iterations: u32,
    pub history: Vec<String>,
}

impl Default for ReflectionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_iterations: 3,
            reviewer_prompt: "你是一个严格的评审者。评估以下输出的质量，返回 JSON: {\"score\": 1-5, \"issues\": [], \"suggestions\": []}".into(),
            stop_condition: StopCondition::ScoreAbove(4.0),
        }
    }
}

/// Reviewer 输出解析（宽松）：提取 score 字段，失败回退 0.0
fn parse_score(text: &str) -> f32 {
    let cleaned = text.trim();
    if let Some(rest) = cleaned.strip_prefix("```") {
        let end = rest.find("```").unwrap_or(rest.len());
        let body = &rest[..end];
        if let Some(start) = body.find('{') {
            if let Some(stop) = body[start..].rfind('}') {
                if let Ok(v) =
                    serde_json::from_str::<serde_json::Value>(&body[start..start + stop + 1])
                {
                    if let Some(score) = v.get("score").and_then(|s| s.as_f64()) {
                        return score as f32;
                    }
                }
            }
        }
        return 0.0;
    }
    if let Some(start) = cleaned.find('{') {
        if let Some(stop) = cleaned[start..].rfind('}') {
            if let Ok(v) =
                serde_json::from_str::<serde_json::Value>(&cleaned[start..start + stop + 1])
            {
                if let Some(score) = v.get("score").and_then(|s| s.as_f64()) {
                    return score as f32;
                }
            }
        }
    }
    0.0
}

/// 反思循环：生成 → 评审 → 将评审反馈注入下一轮生成 → 重复，直到满足停止条件。
pub async fn run_reflection_loop(
    model: Arc<dyn ModelProvider>,
    system_prompt: &str,
    original_task: &str,
    initial_output: &str,
    config: &ReflectionConfig,
) -> Result<ReflectionResult, AgentError> {
    let mut current_text = initial_output.to_string();
    let mut history = vec![current_text.clone()];
    let max_iterations = config.max_iterations.max(1);

    for i in 0..max_iterations {
        let iter = i as usize;
        // 1. 评审者评估当前输出
        let critique = model
            .generate(GenerationRequest {
                messages: vec![
                    ChatMessage {
                        role: ChatRole::System,
                        content: MessageContent::Text(config.reviewer_prompt.clone()),
                        name: None,
                    },
                    ChatMessage {
                        role: ChatRole::User,
                        content: MessageContent::Text(format!(
                            "原始任务：{original_task}\n\n生成输出：\n{current_text}"
                        )),
                        name: None,
                    },
                ],
                temperature: Some(0.1),
                ..Default::default()
            })
            .await?;

        let score = parse_score(&critique.text);

        // 2. 停止条件检查
        match &config.stop_condition {
            StopCondition::ScoreAbove(threshold) if score >= *threshold => {
                return Ok(ReflectionResult {
                    text: current_text,
                    iterations: (iter + 1) as u32,
                    history,
                });
            }
            StopCondition::NoChanges if iter >= 1 && history[iter] == history[iter - 1] => {
                return Ok(ReflectionResult {
                    text: current_text,
                    iterations: (iter + 1) as u32,
                    history,
                });
            }
            _ => {}
        }

        // 3. 将评审反馈注入下一轮生成
        let improved = model
            .generate(GenerationRequest {
                messages: vec![
                    ChatMessage {
                        role: ChatRole::System,
                        content: MessageContent::Text(format!(
                            "你是输出改进者。根据评审反馈改进以下输出。\n{system_prompt}"
                        )),
                        name: None,
                    },
                    ChatMessage {
                        role: ChatRole::User,
                        content: MessageContent::Text(format!(
                            "原始任务：{original_task}\n\n当前输出：\n{current_text}\n\n评审反馈：\n{}",
                            critique.text
                        )),
                        name: None,
                    },
                ],
                temperature: Some(0.3),
                ..Default::default()
            })
            .await?;

        if improved.text.trim().is_empty() {
            break;
        }
        current_text = improved.text;
        history.push(current_text.clone());
    }

    Ok(ReflectionResult {
        text: current_text,
        iterations: history.len() as u32,
        history,
    })
}
