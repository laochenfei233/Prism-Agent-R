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

                // Weighted combination: 0.7 vector + 0.3 BM25
                let score = 0.7 * cos_sim + 0.3 * bm25;

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
