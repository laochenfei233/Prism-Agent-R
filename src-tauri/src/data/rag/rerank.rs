use std::sync::Arc;

use async_trait::async_trait;

use crate::core::adk::error::AgentError;
use crate::core::adk::model::{
    ChatMessage, ChatRole, GenerationRequest, MessageContent, ModelProvider,
};

// ── 重排序（§10.2.2 可选 reranking） ──────────────────────
// 初检 top-150 → reranker 重打分 → top-20 注入。
// 实现可插拔：本地 ONNX 交叉编码器或 API reranker；未配置时跳过（无感降级）。

#[async_trait]
pub trait Reranker: Send + Sync {
    /// 对候选 (query, doc) 打分（分数越高越相关），返回重排后的索引顺序
    async fn rerank(
        &self,
        query: &str,
        docs: &[String],
        top_k: usize,
    ) -> Result<Vec<usize>, AgentError>;
}

/// 零成本降级：无 reranker 时原序保留（取前 top_k）
pub struct NoopReranker;

#[async_trait]
impl Reranker for NoopReranker {
    async fn rerank(
        &self,
        _query: &str,
        docs: &[String],
        top_k: usize,
    ) -> Result<Vec<usize>, AgentError> {
        Ok((0..docs.len().min(top_k)).collect())
    }
}

/// LLM 交叉编码（轻量）：一次请求让 LLM 按相关性排序候选，输出 JSON 索引数组
pub struct LlmReranker {
    model: Arc<dyn ModelProvider>,
    max_candidates: usize,
}

impl LlmReranker {
    pub fn new(model: Arc<dyn ModelProvider>) -> Self {
        Self {
            model,
            max_candidates: 150,
        }
    }
}

#[async_trait]
impl Reranker for LlmReranker {
    async fn rerank(
        &self,
        query: &str,
        docs: &[String],
        top_k: usize,
    ) -> Result<Vec<usize>, AgentError> {
        if docs.is_empty() {
            return Ok(Vec::new());
        }
        // 超长候选截断（保护 prompt）
        let candidates: Vec<&String> = docs.iter().take(self.max_candidates).collect();
        let mut body = String::new();
        for (i, d) in candidates.iter().enumerate() {
            let snippet: String = d.chars().take(200).collect();
            body.push_str(&format!("[{i}] {snippet}\n"));
        }

        let prompt = format!(
            "你是检索重排序器。给定查询和候选文档片段，按与查询的相关性从高到低排序。\n\
             \n查询：{query}\n\n候选：\n{body}\n\n\
             只返回 JSON 数组（按相关性降序的索引列表，如 [3, 0, 1]），最多 {top_k} 个，不要其他文本。"
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
                max_tokens: Some(128),
                ..Default::default()
            })
            .await?;
        Ok(parse_index_list(&resp.text, candidates.len(), top_k))
    }
}

/// 宽松解析 JSON 索引数组（支持围栏/前缀文本）
fn parse_index_list(text: &str, n: usize, top_k: usize) -> Vec<usize> {
    let cleaned = text.trim();
    let body = if cleaned.starts_with("```") {
        let end = cleaned.rfind("```").unwrap_or(cleaned.len());
        &cleaned[cleaned.find('\n').map(|i| i + 1).unwrap_or(0)..end]
    } else {
        cleaned
    };
    let start = body.find('[').unwrap_or(0);
    let stop = body[start..]
        .rfind(']')
        .map(|i| i + start)
        .unwrap_or(body.len());
    let slice = &body[start..stop];
    let mut idx: Vec<usize> = slice
        .split(|c: char| !c.is_ascii_digit() && c != '-')
        .filter_map(|s| {
            if s.is_empty() {
                return None;
            }
            s.parse::<usize>().ok().filter(|i| *i < n)
        })
        .collect();
    // 去重保序
    let mut seen = std::collections::HashSet::new();
    idx.retain(|i| seen.insert(*i));
    idx.truncate(top_k);
    idx
}

/// 重排工具函数：输入初检结果 (id, text, score)，输出重排后的顺序
pub async fn rerank_top<T: Clone>(
    reranker: &dyn Reranker,
    query: &str,
    candidates: &[(String, T)],
    top_k: usize,
) -> Result<Vec<(String, T)>, AgentError> {
    if candidates.is_empty() {
        return Ok(Vec::new());
    }
    let docs: Vec<String> = candidates.iter().map(|(s, _)| s.clone()).collect();
    let order = reranker
        .rerank(query, &docs, top_k.min(candidates.len()))
        .await?;
    Ok(order
        .into_iter()
        .filter_map(|i| candidates.get(i).cloned())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_json_list() {
        let r = parse_index_list("[3, 0, 1]", 10, 3);
        assert_eq!(r, vec![3, 0, 1]);
    }

    #[test]
    fn parse_fenced_and_extra() {
        let r = parse_index_list("```json\n[2, 1]\n```", 10, 2);
        assert_eq!(r, vec![2, 1]);
    }

    #[test]
    fn dedup_and_clamp() {
        let r = parse_index_list("[0, 0, 99, 1, 2]", 5, 2);
        assert_eq!(r, vec![0, 1]);
    }

    #[tokio::test]
    async fn noop_preserves_order() {
        let r = NoopReranker
            .rerank("q", &["a".into(), "b".into()], 2)
            .await
            .unwrap();
        assert_eq!(r, vec![0, 1]);
    }
}
