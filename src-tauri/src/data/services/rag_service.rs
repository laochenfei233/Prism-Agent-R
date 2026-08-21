use std::sync::Arc;

use crate::data::db::Database;
use crate::data::models::{IngestResult, RagDocumentDto, RagHit};
use crate::data::rag::chunker::chunk_text;
use crate::data::rag::embedding::{self, Embedder, LocalEmbedder, OpenAiEmbedder};
use crate::data::rag::search::RagSearcher;
use crate::data::rag::store;
use crate::utils::error::AppError;
use crate::utils::paths;

pub struct RagService {
    db: Database,
    embedder: Arc<dyn Embedder>,
    searcher: RagSearcher,
}

/// 嵌入器配置（持久化到 preferences 表）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EmbeddingConfig {
    /// "local" | "api"
    pub mode: String,
    /// API 模式下的 provider_id（用于解析 base_url/api_key）
    pub provider_id: Option<String>,
    /// API 模式下的嵌入模型名（如 text-embedding-3-small / nomic-embed-text）
    pub model: Option<String>,
    /// API 模式下的嵌入维度（用于校验）
    pub dim: Option<usize>,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            mode: "local".into(),
            provider_id: None,
            model: None,
            dim: None,
        }
    }
}

impl RagService {
    pub fn new(db: Database) -> Self {
        let embedder: Arc<dyn Embedder> = Arc::new(LocalEmbedder::default());
        let searcher = RagSearcher::new(db.clone(), embedder.clone());
        Self {
            db,
            embedder,
            searcher,
        }
    }

    /// 从 preferences 表读取嵌入器配置并构建对应嵌入器。
    /// 配置读取失败或 provider 不存在时回退 LocalEmbedder（无感降级）。
    pub async fn configure_from_db(&mut self) -> Result<EmbeddingConfig, AppError> {
        let cfg = read_embedding_config(&self.db).await?;
        self.embedder = build_embedder(&self.db, &cfg).await;
        self.searcher = RagSearcher::new(self.db.clone(), self.embedder.clone());
        Ok(cfg)
    }

    /// 保存嵌入器配置并立即应用。
    pub async fn set_config(&mut self, cfg: &EmbeddingConfig) -> Result<(), AppError> {
        write_embedding_config(&self.db, cfg).await?;
        self.embedder = build_embedder(&self.db, cfg).await;
        self.searcher = RagSearcher::new(self.db.clone(), self.embedder.clone());
        Ok(())
    }

    /// Ingest a file: read → chunk → insert document → insert chunks → mark ready.
    pub async fn ingest(&self, wiki_id: &str, file_path: &str) -> Result<IngestResult, AppError> {
        self.ingest_impl(wiki_id, file_path, None).await
    }

    /// §10.2.1 项目级自动索引：摄取并记录相对路径 + 指纹（path+mtime+size）
    pub async fn ingest_with_meta(
        &self,
        wiki_id: &str,
        file_path: &str,
        rel_path: &str,
        fingerprint: &str,
    ) -> Result<IngestResult, AppError> {
        self.ingest_impl(wiki_id, file_path, Some((rel_path, fingerprint)))
            .await
    }

    async fn ingest_impl(
        &self,
        wiki_id: &str,
        file_path: &str,
        meta: Option<(&str, &str)>,
    ) -> Result<IngestResult, AppError> {
        // ── §10.2.3 统一文档解析管线：按扩展名分发（PDF 双通道） ──
        let path = std::path::Path::new(file_path);
        let parser = crate::data::rag::parser::parser_for(path);
        let parsed = parser.parse(path).await?;
        let content: String = parsed
            .pages
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");

        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        let mime = mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string();
        let size = content.len() as i64;

        // Insert document record
        let (rel, fingerprint) = meta
            .map(|(r, f)| (Some(r), Some(f)))
            .unwrap_or((None, None));
        let doc_id = store::insert_document_with_meta(
            &self.db,
            wiki_id,
            &file_name,
            &mime,
            size,
            rel,
            fingerprint,
        )
        .await?;

        // Update status to chunking
        store::update_document_status(&self.db, &doc_id, "chunking", None).await?;

        // Chunk the text（分块大小/重叠可从设置页调整，回退 1000/200）
        let (chunk_size, overlap) = {
            use crate::data::settings::prefs;
            (
                prefs::get_i64(&self.db.pool, "rag.chunk_size", 1000)
                    .await
                    .clamp(200, 2000) as usize,
                prefs::get_i64(&self.db.pool, "rag.chunk_overlap", 200)
                    .await
                    .clamp(0, 500) as usize,
            )
        };
        let chunks = chunk_text(&content, chunk_size, overlap);

        // ── §10.2.2 Contextual Retrieval：为每个 chunk 生成上下文说明 ──
        let contextual = self.contextual_enabled().await;
        let contexts: Vec<String> = if contextual {
            use crate::data::rag::contextualize::Contextualizer;
            let ctx = crate::data::rag::contextualize::HeuristicContextualizer;
            let mut out = Vec::with_capacity(chunks.len());
            for c in &chunks {
                out.push(ctx.contextualize(&content, c).await.unwrap_or_default());
            }
            out
        } else {
            Vec::new()
        };

        // Update status to embedding
        store::update_document_status(&self.db, &doc_id, "embedding", None).await?;

        // Generate embeddings（contextual 开启时嵌入 context + content 拼接）
        let embed_texts: Vec<String> = if contextual {
            chunks
                .iter()
                .zip(contexts.iter())
                .map(|(c, ctx)| format!("{ctx}\n{c}"))
                .collect()
        } else {
            chunks.clone()
        };
        // §10.2.3/10.2.4：为每个 chunk 标注页码（PDF 按页文本累积偏移定位）
        let page_meta = build_page_meta(&parsed.pages, &chunks);

        let embeddings = self.embedder.embed_batch(&embed_texts).await?;

        // Build (content, embedding_bytes) pairs
        let chunk_pairs: Vec<(String, Option<Vec<u8>>)> = chunks
            .into_iter()
            .zip(embeddings)
            .map(|(text, emb)| {
                let bytes = embedding::embedding_to_bytes(&emb);
                (text, Some(bytes))
            })
            .collect();

        // Insert chunks（携带 context 列 + 页码 meta）
        let contexts_ref = if contextual {
            Some(&contexts[..])
        } else {
            None
        };

        let chunk_count = store::insert_chunks(
            &self.db,
            &doc_id,
            wiki_id,
            &chunk_pairs,
            contexts_ref,
            Some(&page_meta),
        )
        .await?;

        // Mark ready
        store::update_document_status(&self.db, &doc_id, "ready", None).await?;

        Ok(IngestResult {
            document_id: doc_id,
            chunk_count,
            status: "ready".to_string(),
        })
    }

    /// Hybrid search（§10.2.2 可选 reranking）：初检 top-150 → reranker 重排 → top_k
    pub async fn search(
        &self,
        wiki_id: &str,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<RagHit>, AppError> {
        // 初检取更大池（rerank 上游）
        const CANDIDATE_POOL: usize = 150;
        let mut hits = self
            .searcher
            .hybrid_search(wiki_id, query, CANDIDATE_POOL)
            .await?;

        // reranker 开关（rag.rerank 默认关——LLM 打分有成本）
        let rerank_enabled = match get_pref(&self.db, KEY_RERANK).await {
            Some(v) => v == "1" || v.eq_ignore_ascii_case("true"),
            None => false,
        };

        if rerank_enabled && hits.len() > top_k {
            use crate::data::rag::rerank::Reranker;
            let docs: Vec<String> = hits.iter().map(|h| h.quote.clone()).collect();
            match resolve_rerank_model(&self.db).await {
                Ok(model) => {
                    let reranker = crate::data::rag::rerank::LlmReranker::new(model);
                    let order = reranker
                        .rerank(query, &docs, top_k)
                        .await
                        .unwrap_or_default();
                    if !order.is_empty() {
                        hits = order
                            .into_iter()
                            .filter_map(|i| hits.get(i).cloned())
                            .collect();
                    }
                }
                Err(_) => { /* 无模型 → 保留初检顺序 */ }
            }
        }

        hits.truncate(top_k);
        Ok(hits)
    }

    /// List all documents in a wiki.
    pub async fn list_documents(&self, wiki_id: &str) -> Result<Vec<RagDocumentDto>, AppError> {
        let rows = store::list_documents(&self.db, wiki_id).await?;
        Ok(rows
            .into_iter()
            .map(|r| RagDocumentDto {
                id: r.id,
                name: r.name,
                mime_type: r.mime_type,
                size: r.size,
                chunk_count: r.chunk_count,
                status: r.status,
            })
            .collect())
    }

    /// Delete a document and its chunks.
    pub async fn delete_document(&self, doc_id: &str) -> Result<(), AppError> {
        store::delete_document(&self.db, doc_id).await
    }
}

/// 为每个 chunk 标注页码范围（§10.2.3/10.2.4）：
/// 按页文本累积字符偏移，chunk 起始偏移（前序 chunk 长度累加）落入某页区间即标注该页。
fn build_page_meta(
    pages: &[crate::data::rag::parser::ParsedPage],
    chunks: &[String],
) -> Vec<(Option<i32>, Option<i32>)> {
    // 单页文档（普通文本/markdown/单页 PDF）不标页码
    if pages.len() < 2 {
        return vec![(None, None); chunks.len()];
    }

    // 页面累积偏移（char 计）
    let mut boundaries: Vec<(u32, usize)> = Vec::new();
    let mut acc = 0usize;
    for p in pages {
        boundaries.push((p.page_no, acc));
        acc += p.text.chars().count();
    }

    let mut cursor = 0usize;
    chunks
        .iter()
        .map(|c| {
            let start = cursor;
            cursor += c.chars().count();
            // 找 start 所属页
            let mut page = boundaries[0].0;
            for (no, offset) in &boundaries {
                if start >= *offset {
                    page = *no;
                }
            }
            (Some(page as i32), Some(page as i32))
        })
        .collect()
}

// ── 配置读写 ──────────────────────────────────────────────

const KEY_MODE: &str = "rag.embedding.mode";
const KEY_PROVIDER: &str = "rag.embedding.provider_id";
const KEY_MODEL: &str = "rag.embedding.model";
const KEY_DIM: &str = "rag.embedding.dim";
const KEY_CONTEXTUAL: &str = "rag.contextual";
const KEY_RERANK: &str = "rag.rerank";

impl RagService {
    /// §10.2.2 Contextual Retrieval 开关（preferences: rag.contextual，默认开）
    pub async fn contextual_enabled(&self) -> bool {
        match get_pref(&self.db, KEY_CONTEXTUAL).await {
            Some(v) => v == "1" || v.eq_ignore_ascii_case("true"),
            None => true, // 默认开启（对齐设计 §10.2.2）
        }
    }

    /// 设置 contextual 开关
    pub async fn set_contextual(&self, enabled: bool) -> Result<(), AppError> {
        set_pref(
            &self.db,
            KEY_CONTEXTUAL,
            Some(if enabled { "1" } else { "0" }),
        )
        .await
    }

    /// 当前 contextual 配置状态
    pub async fn contextual_status(&self) -> Result<serde_json::Value, AppError> {
        Ok(serde_json::json!({ "enabled": self.contextual_enabled().await }))
    }

    /// §10.2.2 reranker 开关
    pub async fn set_rerank(&self, enabled: bool) -> Result<(), AppError> {
        set_pref(&self.db, KEY_RERANK, Some(if enabled { "1" } else { "0" })).await
    }

    /// 当前 reranker 配置状态
    pub async fn rerank_status(&self) -> Result<serde_json::Value, AppError> {
        Ok(serde_json::json!({ "enabled": self.rerank_enabled().await }))
    }

    async fn rerank_enabled(&self) -> bool {
        match get_pref(&self.db, KEY_RERANK).await {
            Some(v) => v == "1" || v.eq_ignore_ascii_case("true"),
            None => false,
        }
    }
}

async fn get_pref(db: &Database, key: &str) -> Option<String> {
    sqlx::query_scalar::<_, String>("SELECT value FROM preferences WHERE key = ?")
        .bind(key)
        .fetch_optional(&db.pool)
        .await
        .ok()
        .flatten()
}

async fn set_pref(db: &Database, key: &str, value: Option<&str>) -> Result<(), AppError> {
    let now = chrono::Utc::now().timestamp();
    match value {
        Some(v) => {
            sqlx::query(
                "INSERT OR REPLACE INTO preferences (key, value, updated_at) VALUES (?, ?, ?)",
            )
            .bind(key)
            .bind(v)
            .bind(now)
            .execute(&db.pool)
            .await?;
        }
        None => {
            sqlx::query("DELETE FROM preferences WHERE key = ?")
                .bind(key)
                .execute(&db.pool)
                .await?;
        }
    }
    Ok(())
}

async fn read_embedding_config(db: &Database) -> Result<EmbeddingConfig, AppError> {
    Ok(EmbeddingConfig {
        mode: get_pref(db, KEY_MODE)
            .await
            .unwrap_or_else(|| "local".into()),
        provider_id: get_pref(db, KEY_PROVIDER).await,
        model: get_pref(db, KEY_MODEL).await,
        dim: get_pref(db, KEY_DIM).await.and_then(|d| d.parse().ok()),
    })
}

async fn write_embedding_config(db: &Database, cfg: &EmbeddingConfig) -> Result<(), AppError> {
    set_pref(db, KEY_MODE, Some(&cfg.mode)).await?;
    set_pref(db, KEY_PROVIDER, cfg.provider_id.as_deref()).await?;
    set_pref(db, KEY_MODEL, cfg.model.as_deref()).await?;
    set_pref(db, KEY_DIM, cfg.dim.map(|d| d.to_string()).as_deref()).await?;
    Ok(())
}

/// 按配置构建嵌入器：api 模式 → OpenAiEmbedder（provider 解析失败回退 local）；local → LocalEmbedder
async fn build_embedder(db: &Database, cfg: &EmbeddingConfig) -> Arc<dyn Embedder> {
    if cfg.mode == "api" {
        if let Some(provider_id) = &cfg.provider_id {
            let row = sqlx::query_as::<_, crate::data::models::ProviderRow>(
                "SELECT id, name, kind, base_url, api_key_enc, is_enabled, created_at, updated_at FROM providers WHERE id = ?"
            )
            .bind(provider_id)
            .fetch_optional(&db.pool)
            .await
            .ok()
            .flatten();

            if let Some(p) = row {
                let base_url = p.base_url.unwrap_or_else(|| match p.kind.as_str() {
                    "ollama" => "http://localhost:11434/v1".to_string(),
                    _ => "https://api.openai.com/v1".to_string(),
                });
                let api_key = p
                    .api_key_enc
                    .as_deref()
                    .map(crate::commands::settings::decrypt_provider_key)
                    .unwrap_or_default();
                let model = cfg
                    .model
                    .clone()
                    .unwrap_or_else(|| "text-embedding-3-small".to_string());
                tracing::info!("RAG embedding: API mode ({model} @ {base_url})");
                return Arc::new(OpenAiEmbedder::new(base_url, api_key, model));
            }
        }
    }
    tracing::info!("RAG embedding: local mode (feature-hash)");
    Arc::new(LocalEmbedder::new(cfg.dim.unwrap_or(256)))
}

/// 解析默认模型构建 reranker（LLM 交叉编码；无模型时返回 Err 由调用方降级）
/// pub(crate)：§10.2.5 评测 chart_acc 的 LLM-as-Judge 复用同一解析通道
pub(crate) async fn resolve_rerank_model(
    db: &Database,
) -> Result<Arc<dyn crate::core::adk::model::ModelProvider>, AppError> {
    use crate::data::models::{ModelRow, ProviderRow};
    let model_row = sqlx::query_as::<_, ModelRow>(
        "SELECT id, provider_id, model_id, display_name, kind, max_tokens, is_default, created_at FROM models WHERE is_default = 1 LIMIT 1"
    )
    .fetch_optional(&db.pool)
    .await?
    .ok_or_else(|| AppError::LlmProvider("未配置默认模型，无法重排序".into()))?;

    let provider_row = sqlx::query_as::<_, ProviderRow>(
        "SELECT id, name, kind, base_url, api_key_enc, is_enabled, created_at, updated_at FROM providers WHERE id = ?"
    )
    .bind(&model_row.provider_id)
    .fetch_optional(&db.pool)
    .await?
    .ok_or_else(|| AppError::LlmProvider(format!("Provider not found: {}", model_row.provider_id)))?;

    let base_url = provider_row
        .base_url
        .unwrap_or_else(|| match provider_row.kind.as_str() {
            "ollama" => "http://localhost:11434/v1".to_string(),
            _ => "https://api.openai.com/v1".to_string(),
        });
    let api_key = provider_row
        .api_key_enc
        .as_deref()
        .map(crate::commands::settings::decrypt_provider_key)
        .unwrap_or_default();
    let provider: Arc<dyn crate::core::adk::model::ModelProvider> =
        Arc::new(crate::core::rig::provider::OpenAiProvider::new(
            model_row.provider_id.clone(),
            model_row
                .display_name
                .clone()
                .unwrap_or_else(|| model_row.model_id.clone()),
            api_key,
            base_url,
            model_row.model_id.clone(),
        ));
    Ok(provider)
}

/// 当前嵌入器信息（供 UI 展示）
pub async fn embedding_status(db: &Database) -> Result<serde_json::Value, AppError> {
    let cfg = read_embedding_config(db).await?;
    let kind = if cfg.mode == "api" { "api" } else { "local" };
    Ok(serde_json::json!({
        "mode": cfg.mode,
        "kind": kind,
        "provider_id": cfg.provider_id,
        "model": cfg.model,
        "dim": cfg.dim.unwrap_or(256),
        "is_local": cfg.mode != "api",
        "base_dir": paths::wiki_dir().to_string_lossy(),
    }))
}
