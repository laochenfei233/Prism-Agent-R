use crate::core::adk::model::{
    ChatMessage, ChatRole, GenerationRequest, MessageContent, ModelProvider,
};
use crate::utils::error::AppError;
use serde::{Deserialize, Serialize};

// ── §19.3.1 压缩策略 ──────────────────────────────────────

/// 压缩策略：Level 1 裁剪（现有）→ Level 2 摘要化 compaction（新增，默认启用）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CompactStrategy {
    /// 简单裁剪（丢最旧消息）
    Truncate,
    /// 摘要化 compaction（调用 LLM 生成会话摘要）
    Summarize,
}

/// §19.3.1 Compactor：会话上下文压缩器
///
/// 超阈值时：调用 LLM 生成「会话摘要 + 未完成目标 + 关键约束」，
/// 用摘要 + 最近 N 条消息替换历史（保留模型潜在理解，而非简单丢最旧）。
pub struct Compactor {
    pub strategy: CompactStrategy,
    pub summarize_prompt: String,
    pub trigger_tokens: usize,
    pub keep_reasoning: bool,
}

impl Compactor {
    pub fn new() -> Self {
        Self {
            strategy: CompactStrategy::Summarize,
            summarize_prompt: SUMMARIZE_PROMPT.to_string(),
            trigger_tokens: 100_000,
            keep_reasoning: true,
        }
    }

    /// 检查是否需要压缩
    pub fn needs_compaction(&self, current_tokens: usize) -> bool {
        current_tokens >= self.trigger_tokens
    }

    /// 执行压缩：根据策略选择截断或摘要
    pub async fn compact(
        &self,
        provider: &dyn ModelProvider,
        history: &[ChatMessage],
    ) -> Result<Vec<ChatMessage>, AppError> {
        match self.strategy {
            CompactStrategy::Truncate => Ok(self.truncate(history)),
            CompactStrategy::Summarize => {
                match self.summarize(provider, history).await {
                    Ok(result) => Ok(result),
                    Err(e) => {
                        // 回退到截断策略
                        tracing::warn!("Summarize failed, falling back to truncate: {e}");
                        Ok(self.truncate(history))
                    }
                }
            }
        }
    }

    /// 简单截断：保留最近 N 条消息
    fn truncate(&self, history: &[ChatMessage]) -> Vec<ChatMessage> {
        let keep = 10; // 保留最近 10 条
        if history.len() <= keep {
            return history.to_vec();
        }
        let start = history.len() - keep;
        history[start..].to_vec()
    }

    /// 摘要化 compaction：调用 LLM 生成会话摘要
    async fn summarize(
        &self,
        provider: &dyn ModelProvider,
        history: &[ChatMessage],
    ) -> Result<Vec<ChatMessage>, AppError> {
        if history.is_empty() {
            return Ok(vec![]);
        }

        // 构建摘要请求：将历史消息转为文本
        let history_text = history
            .iter()
            .map(|msg| {
                let role = match msg.role {
                    ChatRole::User => "用户",
                    ChatRole::Assistant => "助手",
                    ChatRole::Tool => "工具",
                    ChatRole::System => "系统",
                };
                let content = match &msg.content {
                    MessageContent::Text(t) => t.clone(),
                    MessageContent::ToolCall(tc) => format!("[工具调用: {}]", tc.name),
                    MessageContent::ToolResult(_) => "[工具结果]".to_string(),
                };
                format!("{role}: {content}")
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let prompt = format!(
            "{}\n\n---\n\n以下是需要摘要的会话历史：\n\n{}",
            self.summarize_prompt, history_text
        );

        let request = GenerationRequest {
            messages: vec![ChatMessage {
                role: ChatRole::User,
                content: MessageContent::Text(prompt),
                name: None,
            }],
            ..Default::default()
        };

        let response = provider
            .generate(request)
            .await
            .map_err(|e| AppError::Internal(format!("Compactor LLM 调用失败: {e}")))?;

        // 构建压缩后的历史：系统消息 + 摘要 + 最近 5 条消息
        let mut compressed = vec![ChatMessage {
            role: ChatRole::System,
            content: MessageContent::Text(format!("[会话摘要]\n{}", response.text)),
            name: None,
        }];

        // 保留最近 5 条消息
        let keep = 5.min(history.len());
        let start = history.len() - keep;
        compressed.extend_from_slice(&history[start..]);

        Ok(compressed)
    }
}

impl Default for Compactor {
    fn default() -> Self {
        Self::new()
    }
}

/// 摘要化 prompt（固定前缀，保 prompt cache）
const SUMMARIZE_PROMPT: &str = r#"你是一个会话摘要助手。请将以下对话历史压缩为简洁摘要。

要求：
1. 保留对话的核心主题和关键决策
2. 记录未完成的目标和待办事项
3. 保留重要的约束条件和用户偏好
4. 摘要应在 200-500 字之间
5. 使用结构化格式（要点列表）

输出格式：
## 会话摘要
- 核心主题：...
- 关键决策：...
- 未完成目标：...
- 重要约束：..."#;

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
    if ratio < 0.50 {
        0
    } else if ratio < 0.70 {
        1
    } else if ratio < 0.85 {
        2
    } else {
        3
    }
}

pub fn soft_trim(output: &str) -> String {
    const THRESHOLD: usize = 4096;
    const KEEP: usize = 1536;
    if output.len() <= THRESHOLD {
        return output.to_string();
    }
    let head: String = output.chars().take(KEEP).collect();
    let tail: String = output
        .chars()
        .rev()
        .take(KEEP)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
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
