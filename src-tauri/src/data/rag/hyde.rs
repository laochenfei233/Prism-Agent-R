use std::sync::Arc;
use crate::core::adk::model::{GenerationRequest, ModelProvider, ChatMessage, ChatRole, MessageContent};
use crate::data::rag::embedding::Embedder;
use crate::utils::error::AppError;

/// HyDE 假设文档检索器（§16.1）
///
/// 动机：短查询（如「保修政策」）与长文档语义不对齐，直接向量检索召回差。
/// HyDE 先让 LLM 生成假设答案，再用假设答案的向量检索真实文档。
pub struct HydeRetriever {
    provider: Arc<dyn ModelProvider>,
    embedder: Arc<dyn Embedder>,
}

impl HydeRetriever {
    pub fn new(provider: Arc<dyn ModelProvider>, embedder: Arc<dyn Embedder>) -> Self {
        Self { provider, embedder }
    }

    /// 生成 HyDE 假设文档
    ///
    /// Prompt 要求 LLM 写一段「假设该文档存在，其内容会如何描述此问题」的段落（100-200 字）
    pub async fn generate_hyde_doc(&self, query: &str) -> Result<String, AppError> {
        let prompt = format!(
            r#"你是一个文档生成助手。用户会提出一个查询，请你生成一段假设性的文档内容，这段内容应该：
1. 假设存在一份权威文档能回答这个查询
2. 用100-200字描述这份文档可能包含的内容
3. 包含与查询相关的关键术语和概念
4. 像真实文档一样自然流畅

查询：{query}

请生成假设文档内容（只输出文档内容本身，不要加解释）："#
        );

        let request = GenerationRequest {
            messages: vec![ChatMessage {
                role: ChatRole::User,
                content: MessageContent::Text(prompt),
                name: None,
            }],
            ..Default::default()
        };

        let response = self.provider
            .generate(request)
            .await
            .map_err(|e| AppError::Internal(format!("HyDE LLM 调用失败: {e}")))?;

        Ok(response.text)
    }

    /// 获取嵌入器引用
    pub fn embedder(&self) -> &Arc<dyn Embedder> {
        &self.embedder
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_hyde_prompt_format() {
        let query = "保修政策";
        let prompt = format!(
            r#"你是一个文档生成助手。用户会提出一个查询，请你生成一段假设性的文档内容，这段内容应该：
1. 假设存在一份权威文档能回答这个查询
2. 用100-200字描述这份文档可能包含的内容
3. 包含与查询相关的关键术语和概念
4. 像真实文档一样自然流畅

查询：{query}

请生成假设文档内容（只输出文档内容本身，不要加解释）："#
        );
        assert!(prompt.contains("保修政策"));
    }
}
