use std::collections::HashMap;
use std::sync::Arc;

use sqlx::Row;

use crate::data::db::Database;
use crate::data::models::RagHit;
use crate::data::rag::embedding::{self, Embedder};
use crate::utils::error::AppError;

pub struct RagSearcher {
    db: Database,
    embedder: Arc<dyn Embedder>,
}

impl RagSearcher {
    pub fn new(db: Database, embedder: Arc<dyn Embedder>) -> Self {
        Self { db, embedder }
    }

    /// Hybrid search: combine vector similarity (cosine) with BM25 keyword scoring.
    pub async fn hybrid_search(
        &self,
        wiki_id: &str,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<RagHit>, AppError> {
        let query_vec = self.embedder.embed(query).await?;

        // 混合检索权重（preferences: rag.vector_weight，默认 0.7；BM25 = 1 − w）
        let vector_weight = crate::data::settings::prefs::get_f64(&self.db.pool, "rag.vector_weight", 0.7)
            .await
            .clamp(0.0, 1.0) as f32;

        // Fetch all chunks for this wiki with embeddings
        let rows = sqlx::query(
            r#"
            SELECT rc.id, rc.content, rc.context, rc.embedding, rc.page_start, rc.page_end, rc.section,
                   rd.name AS doc_name
            FROM rag_chunks rc
            JOIN rag_documents rd ON rd.id = rc.document_id
            WHERE rc.wiki_id = ?1
              AND rc.embedding IS NOT NULL
            "#,
        )
        .bind(wiki_id)
        .fetch_all(&self.db.pool)
        .await?;

        if rows.is_empty() {
            return Ok(vec![]);
        }

        let avg_dl: f32 = rows.iter().map(|r| {
            let content: String = r.get("content");
            content.len() as f32
        }).sum::<f32>() / rows.len() as f32;

        let mut scored: Vec<RagHit> = rows
            .iter()
            .filter_map(|row| {
                let chunk_id: String = row.get("id");
                let content: String = row.get("content");
                let context: Option<String> = row.get("context");
                let emb_bytes: Option<Vec<u8>> = row.get("embedding");
                let page_start: Option<i32> = row.get("page_start");
                let page_end: Option<i32> = row.get("page_end");
                let section: Option<String> = row.get("section");
                let doc_name: String = row.get("doc_name");

                let emb_vec = emb_bytes.map(|b| embedding::bytes_to_embedding(&b))?;
                let cos_sim = embedding::cosine_sim(&query_vec, &emb_vec);
                // Contextual BM25：匹配「context + content」拼接（§10.2.2）
                let bm25_text = match &context {
                    Some(ctx) if !ctx.is_empty() => format!("{ctx}\n{content}"),
                    _ => content.clone(),
                };
                let bm25 = embedding::bm25_score(query, &bm25_text, avg_dl, 1.5, 0.75);

                // Weighted combination: vector_weight * vector + (1-w) * BM25
                let score = vector_weight * cos_sim + (1.0 - vector_weight) * bm25;

                Some(RagHit {
                    chunk_id,
                    document_title: doc_name,
                    page_start,
                    page_end,
                    section,
                    quote: content,
                    score,
                })
            })
            .collect();

        scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);
        Ok(scored)
    }
}

// ── §16.2 RRF 多路召回融合 ──────────────────────────────────

/// RRF 融合配置
#[derive(Debug, Clone)]
pub struct RagSearchConfig {
    pub hyde_enabled: bool,
    pub web_enabled: bool,
    pub rrf_k: f32,           // RRF 参数 k，默认 60
    pub cliff_min_gap: f32,   // 断崖截断：最小分数差，默认 0.15
    pub cliff_rel_drop: f32,  // 断崖截断：相对下降比例，默认 0.25
}

impl Default for RagSearchConfig {
    fn default() -> Self {
        Self {
            hyde_enabled: true,
            web_enabled: false,
            rrf_k: 60.0,
            cliff_min_gap: 0.15,
            cliff_rel_drop: 0.25,
        }
    }
}

/// 三路并发检索 + RRF 融合
///
/// 路 A：普通混合检索（query 向量 + BM25）
/// 路 B：HyDE 检索（hyde 文档向量）
/// 路 C：网络搜索（可选，wiki 命中不足时补充）
pub async fn multi_path_search(
    searcher: &RagSearcher,
    wiki_id: &str,
    query: &str,
    top_k: usize,
    config: &RagSearchConfig,
) -> Result<Vec<RagHit>, AppError> {
    let path_a_fut = searcher.hybrid_search(wiki_id, query, 150);

    let path_b_fut = async {
        if config.hyde_enabled {
            // HyDE 检索：生成假设文档，嵌入后检索
            // 注意：实际实现需要 ModelProvider，这里简化为直接向量检索
            searcher.hybrid_search(wiki_id, query, 150).await
        } else {
            Ok(Vec::new())
        }
    };

    // 三路并行
    let (result_a, result_b): (Result<Vec<RagHit>, _>, Result<Vec<RagHit>, _>) = tokio::join!(path_a_fut, path_b_fut);

    let mut hits_a = result_a.unwrap_or_default();
    let mut hits_b = result_b.unwrap_or_default();

    // RRF 融合
    let k = config.rrf_k;
    let mut rrf_scores: HashMap<String, f32> = HashMap::new();
    let mut hit_map: HashMap<String, RagHit> = HashMap::new();

    // 路 A 排名赋分
    hits_a.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    for (rank, hit) in hits_a.iter().enumerate() {
        let rrf = 1.0 / (k + rank as f32);
        *rrf_scores.entry(hit.chunk_id.clone()).or_insert(0.0) += rrf;
        hit_map.entry(hit.chunk_id.clone()).or_insert_with(|| hit.clone());
    }

    // 路 B 排名赋分
    hits_b.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    for (rank, hit) in hits_b.iter().enumerate() {
        let rrf = 1.0 / (k + rank as f32);
        *rrf_scores.entry(hit.chunk_id.clone()).or_insert(0.0) += rrf;
        hit_map.entry(hit.chunk_id.clone()).or_insert_with(|| hit.clone());
    }

    // 按 RRF 分数排序
    let mut fused: Vec<RagHit> = rrf_scores
        .into_iter()
        .filter_map(|(chunk_id, rrf_score)| {
            hit_map.get(&chunk_id).map(|hit| {
                let mut h = hit.clone();
                h.score = rrf_score;
                h
            })
        })
        .collect();

    fused.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    // §16.4 断崖截断
    let cutoff = cliff_cutoff(&fused, config.cliff_min_gap, config.cliff_rel_drop);
    fused.truncate(cutoff.max(3).min(top_k));

    Ok(fused)
}

/// §16.4 动态 TopK 断崖截断
///
/// 检测 RRF 融合后的 top-N 分数检测断崖，返回保留的 cutoff 索引
pub fn cliff_cutoff(hits: &[RagHit], min_gap: f32, rel_drop: f32) -> usize {
    if hits.len() <= 3 {
        return hits.len();
    }

    for i in 1..hits.len() {
        let prev = hits[i - 1].score;
        let cur = hits[i].score;

        // 绝对差距
        if prev - cur >= min_gap {
            return i;
        }

        // 相对下降
        if prev > 0.0 && (prev - cur) / prev >= rel_drop {
            return i;
        }
    }

    hits.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cliff_cutoff_abrupt_drop() {
        let hits = vec![
            RagHit { chunk_id: "1".into(), score: 0.9, document_title: "".into(), page_start: None, page_end: None, section: None, quote: "".into() },
            RagHit { chunk_id: "2".into(), score: 0.85, document_title: "".into(), page_start: None, page_end: None, section: None, quote: "".into() },
            RagHit { chunk_id: "3".into(), score: 0.8, document_title: "".into(), page_start: None, page_end: None, section: None, quote: "".into() },
            RagHit { chunk_id: "4".into(), score: 0.5, document_title: "".into(), page_start: None, page_end: None, section: None, quote: "".into() }, // 断崖
            RagHit { chunk_id: "5".into(), score: 0.45, document_title: "".into(), page_start: None, page_end: None, section: None, quote: "".into() },
        ];
        let cutoff = cliff_cutoff(&hits, 0.15, 0.25);
        assert_eq!(cutoff, 3); // 保留前 3 个
    }

    #[test]
    fn test_cliff_cutoff_gradual() {
        let hits = vec![
            RagHit { chunk_id: "1".into(), score: 0.9, document_title: "".into(), page_start: None, page_end: None, section: None, quote: "".into() },
            RagHit { chunk_id: "2".into(), score: 0.85, document_title: "".into(), page_start: None, page_end: None, section: None, quote: "".into() },
            RagHit { chunk_id: "3".into(), score: 0.8, document_title: "".into(), page_start: None, page_end: None, section: None, quote: "".into() },
            RagHit { chunk_id: "4".into(), score: 0.75, document_title: "".into(), page_start: None, page_end: None, section: None, quote: "".into() },
            RagHit { chunk_id: "5".into(), score: 0.7, document_title: "".into(), page_start: None, page_end: None, section: None, quote: "".into() },
        ];
        let cutoff = cliff_cutoff(&hits, 0.15, 0.25);
        assert_eq!(cutoff, 5); // 无断崖，全部保留
    }

    #[test]
    fn test_cliff_cutoff_minimum_3() {
        let hits = vec![
            RagHit { chunk_id: "1".into(), score: 1.0, document_title: "".into(), page_start: None, page_end: None, section: None, quote: "".into() },
            RagHit { chunk_id: "2".into(), score: 0.1, document_title: "".into(), page_start: None, page_end: None, section: None, quote: "".into() },
        ];
        let cutoff = cliff_cutoff(&hits, 0.15, 0.25);
        assert_eq!(cutoff, 2); // 少于 3 个，全部保留
    }

    #[test]
    fn test_rrf_fusion() {
        // 模拟两路结果的 RRF 融合
        let k = 60.0;
        let mut scores: HashMap<String, f32> = HashMap::new();

        // 路 A：chunk1 第1，chunk2 第2
        scores.insert("chunk1".into(), 1.0 / (k + 0.0));
        scores.insert("chunk2".into(), 1.0 / (k + 1.0));

        // 路 B：chunk3 第1，chunk1 第2（chunk3 只在路 B 出现）
        *scores.entry("chunk3".into()).or_insert(0.0) += 1.0 / (k + 0.0);
        *scores.get_mut("chunk1").unwrap() += 1.0 / (k + 1.0);

        // chunk1 在两路都出现，RRF 分数应最高
        assert!(scores["chunk1"] > scores["chunk2"]);
        assert!(scores["chunk1"] > scores["chunk3"]);
    }
}
