use async_trait::async_trait;
use serde::Deserialize;

use crate::utils::error::AppError;

const DEFAULT_BATCH: usize = 20;

/// Embedding trait — swap LocalEmbedder for a real provider later.
#[async_trait]
pub trait Embedder: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, AppError>;
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, AppError>;
}

/// Cosine similarity between two vectors.
pub fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if mag_a == 0.0 || mag_b == 0.0 {
        0.0
    } else {
        dot / (mag_a * mag_b)
    }
}

/// BM25 scoring (Okapi BM25).
pub fn bm25_score(query: &str, doc: &str, avg_dl: f32, k1: f32, b: f32) -> f32 {
    let query_terms: Vec<String> = tokenize(query);
    let doc_terms: Vec<String> = tokenize(doc);
    let doc_len = doc_terms.len() as f32;

    // Build term frequency map for the document
    let mut tf: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for term in &doc_terms {
        *tf.entry(term.clone()).or_insert(0) += 1;
    }

    let mut score = 0.0f32;
    for term in &query_terms {
        if let Some(&freq) = tf.get(term) {
            let tf_val = freq as f32;
            let numerator = tf_val * (k1 + 1.0);
            let denominator = tf_val + k1 * (1.0 - b + b * doc_len / avg_dl);
            let idf_f: f64 = 2.0_f64.ln().max(0.0);
            let idf = idf_f as f32;
            score += idf * numerator / denominator;
        }
    }
    score
}

/// Tokenize text into lowercase words (simple whitespace + punctuation split).
fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

// ── OpenAI 兼容嵌入器（/embeddings 端点） ───────────────────

#[derive(Deserialize)]
struct OpenAiEmbeddingResponse {
    data: Vec<OpenAiEmbeddingItem>,
}

#[derive(Deserialize)]
struct OpenAiEmbeddingItem {
    embedding: Vec<f32>,
}

/// 调用 OpenAI 兼容 `/embeddings` 端点（OpenAI / Ollama / MiMo / DashScope 等），
/// 批量 20 条/请求（对齐设计文档 §10.2 嵌入模式）。
pub struct OpenAiEmbedder {
    base_url: String,
    api_key: String,
    model: String,
    client: reqwest::Client,
    batch_size: usize,
}

impl OpenAiEmbedder {
    pub fn new(base_url: String, api_key: String, model: String) -> Self {
        Self {
            base_url,
            api_key,
            model,
            client: reqwest::Client::new(),
            batch_size: DEFAULT_BATCH,
        }
    }

    async fn embed_inner(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, AppError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let url = format!("{}/embeddings", self.base_url.trim_end_matches('/'));
        let body = serde_json::json!({
            "model": self.model,
            "input": texts,
        });

        let mut req = self.client.post(&url).header("Content-Type", "application/json");
        if !self.api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", self.api_key));
        }

        let resp = req
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Embedding request failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!("Embedding HTTP {status}: {text}")));
        }

        let data: OpenAiEmbeddingResponse = resp
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Embedding parse failed: {e}")))?;

        if data.data.len() != texts.len() {
            return Err(AppError::Internal(format!(
                "Embedding count mismatch: expected {}, got {}",
                texts.len(),
                data.data.len()
            )));
        }

        Ok(data.data.into_iter().map(|i| i.embedding).collect())
    }
}

#[async_trait]
impl Embedder for OpenAiEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, AppError> {
        let mut out = self.embed_inner(&[text.to_string()]).await?;
        out.pop().ok_or_else(|| AppError::Internal("Empty embedding response".into()))
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, AppError> {
        let mut results = Vec::with_capacity(texts.len());
        for batch in texts.chunks(self.batch_size) {
            results.extend(self.embed_inner(batch).await?);
        }
        Ok(results)
    }
}

// ── 本地确定性嵌入器（离线，特征哈希） ───────────────────────

/// FNV-1a 64 位哈希（稳定跨进程）
fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// 中文感知分词：ASCII 词 + CJK 单字 + CJK 双字 bigram
fn tokenize_local(text: &str) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();

    // 英文/数字词
    let mut word = String::new();
    for c in text.chars() {
        if c.is_alphanumeric() && !is_cjk(c) {
            word.push(c.to_ascii_lowercase());
        } else {
            if !word.is_empty() {
                tokens.push(std::mem::take(&mut word));
            }
            if is_cjk(c) {
                tokens.push(c.to_string());
            }
        }
    }
    if !word.is_empty() {
        tokens.push(word);
    }

    // CJK bigram（增强中文语义）
    let cjk_chars: Vec<char> = text.chars().filter(|c| is_cjk(*c)).collect();
    for pair in cjk_chars.windows(2) {
        let bigram: String = pair.iter().collect();
        tokens.push(format!("##{bigram}"));
    }

    tokens
}

fn is_cjk(c: char) -> bool {
    let cp = c as u32;
    (0x4E00..=0x9FFF).contains(&cp)
        || (0x3400..=0x4DBF).contains(&cp)
        || (0x20000..=0x2A6DF).contains(&cp)
        || (0x2A700..=0x2B73F).contains(&cp)
        || (0xF900..=0xFAFF).contains(&cp)
        || (0x3040..=0x30FF).contains(&cp) // 日文假名
        || (0xAC00..=0xD7AF).contains(&cp) // 韩文
}

/// 本地确定性嵌入（无网络）：特征哈希到 dim 维稀疏向量 → L2 归一化。
/// 这是 fastembed（ONNX 量化）的轻量离线替代，语义相近文本余弦相似度 > 0。
pub struct LocalEmbedder {
    pub dim: usize,
}

impl Default for LocalEmbedder {
    fn default() -> Self {
        Self { dim: 256 }
    }
}

impl LocalEmbedder {
    pub fn new(dim: usize) -> Self {
        Self { dim }
    }

    fn embed_text(&self, text: &str) -> Vec<f32> {
        let mut vec = vec![0.0f32; self.dim];
        for token in tokenize_local(text) {
            let h = fnv1a(&token);
            let idx = (h % self.dim as u64) as usize;
            let sign = if (h >> 32) & 1 == 1 { 1.0f32 } else { -1.0f32 };
            vec[idx] += sign;
        }
        let mag: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if mag > 0.0 {
            vec.iter_mut().for_each(|x| *x /= mag);
        }
        vec
    }
}

#[async_trait]
impl Embedder for LocalEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, AppError> {
        Ok(self.embed_text(text))
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, AppError> {
        Ok(texts.iter().map(|t| self.embed_text(t)).collect())
    }
}

/// Serialize embedding vector to little-endian bytes for SQLite BLOB storage.
pub fn embedding_to_bytes(vec: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vec.len() * 4);
    for &v in vec {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    bytes
}

/// Deserialize embedding vector from little-endian bytes.
pub fn bytes_to_embedding(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn local_embedder_similar_texts() {
        let e = LocalEmbedder::default();
        let a = e.embed("Rust 是一门系统编程语言").await.unwrap();
        let b = e.embed("Rust 是系统编程语言").await.unwrap();
        let c = e.embed("今天天气很好我们去公园散步").await.unwrap();
        assert!(cosine_sim(&a, &b) > 0.5);
        assert!(cosine_sim(&a, &c) < cosine_sim(&a, &b));
    }

    #[tokio::test]
    async fn local_embedder_empty() {
        let e = LocalEmbedder::default();
        let v = e.embed("").await.unwrap();
        assert_eq!(v.len(), 256);
        assert!(v.iter().all(|x| *x == 0.0));
    }

    #[test]
    fn bytes_roundtrip() {
        let v = vec![1.5f32, -2.25, 0.0, 42.0];
        let bytes = embedding_to_bytes(&v);
        assert_eq!(bytes_to_embedding(&bytes), v);
    }
}
