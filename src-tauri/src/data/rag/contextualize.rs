use std::sync::Arc;

use async_trait::async_trait;

use crate::core::adk::error::AgentError;
use crate::core::adk::model::{
    ChatMessage, ChatRole, GenerationRequest, MessageContent, ModelProvider,
};

/// 上下文生成器 trait（§10.2.2 Contextual Retrieval）
/// 摄取时为每个 chunk 生成 50-150 token 的中文上下文说明，prepend 到原文前
/// 再做嵌入与 BM25 索引，检索时只把 content 给模型作答。
#[async_trait]
pub trait Contextualizer: Send + Sync {
    async fn contextualize(&self, document: &str, chunk: &str) -> Result<String, AgentError>;
}

/// 默认 LLM 实现（对齐 Anthropic Contextual Retrieval 模板，适配中文）
pub struct LlmContextualizer {
    model: Arc<dyn ModelProvider>,
}

const CONTEXT_PROMPT: &str = r#"
<document>
{document}
</document>
这里是需要结合整篇文档定位的片段：
<chunk>
{chunk}
</chunk>
请用一两句简洁的中文说明该片段在文档中的位置与主题（所属章节、涉及实体、时间范围、上下文关系），用于改善检索。只输出说明本身，不要复述片段内容。
"#;

impl LlmContextualizer {
    pub fn new(model: Arc<dyn ModelProvider>) -> Self {
        Self { model }
    }

    pub async fn model(&self) -> &Arc<dyn ModelProvider> {
        &self.model
    }
}

#[async_trait]
impl Contextualizer for LlmContextualizer {
    async fn contextualize(&self, document: &str, chunk: &str) -> Result<String, AgentError> {
        let prompt = CONTEXT_PROMPT
            .replace("{document}", document)
            .replace("{chunk}", chunk);
        let resp = self
            .model
            .generate(GenerationRequest {
                messages: vec![ChatMessage {
                    role: ChatRole::User,
                    content: MessageContent::Text(prompt),
                    name: None,
                }],
                temperature: Some(0.2),
                max_tokens: Some(150),
                ..Default::default()
            })
            .await?;
        let text = resp.text.trim().to_string();
        Ok(text)
    }
}

/// 占位实现：无 LLM 配置时用「文档标题 + 首段摘要」做启发式上下文（零成本降级）
pub struct HeuristicContextualizer;

#[async_trait]
impl Contextualizer for HeuristicContextualizer {
    async fn contextualize(&self, document: &str, _chunk: &str) -> Result<String, AgentError> {
        let summary: String = document.chars().take(120).collect();
        Ok(format!("文档开篇背景：{summary}…"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn heuristic_works() {
        let c = HeuristicContextualizer;
        let ctx = c
            .contextualize("这是一篇关于公司财务的文档，介绍季度收入。", "收入增长3%")
            .await
            .unwrap();
        assert!(ctx.contains("公司财务"));
    }
}
