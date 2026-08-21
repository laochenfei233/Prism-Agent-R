use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use sqlx::SqlitePool;
use tokio::sync::Semaphore;

use crate::core::adk::model::{
    ChatMessage, ChatRole, GenerationRequest, MessageContent, ModelProvider,
};
use crate::core::rig::provider::OpenAiProvider;
use crate::data::models::{
    DetectResult, GlossaryTermRow, TranslateHistoryDto, TranslateHistoryResult, TranslateResult,
};
use crate::utils::error::AppError;

// ── 翻译服务 ──────────────────────────────────────────────

/// 短文本缓存：key = "text|source|target"（<500 字符，TTL 24h）
const CACHE_TTL_SECS: i64 = 24 * 3600;
const BATCH_CONCURRENCY: usize = 4;

pub struct TranslateService {
    pub pool: SqlitePool,
    cache: Arc<tokio::sync::Mutex<HashMap<String, (String, i64)>>>,
}

impl TranslateService {
    /// 传入共享缓存（AppState 持有），保证跨 IPC 调用缓存生效
    pub fn new(
        pool: SqlitePool,
        cache: Arc<tokio::sync::Mutex<HashMap<String, (String, i64)>>>,
    ) -> Self {
        Self { pool, cache }
    }

    pub async fn translate(
        &self,
        text: &str,
        source: Option<&str>,
        target: &str,
        model_id: Option<&str>,
    ) -> Result<TranslateResult, AppError> {
        let text = text.trim();
        if text.is_empty() {
            return Ok(TranslateResult {
                translated: String::new(),
                source_lang: source.unwrap_or("auto").to_string(),
                from_cache: false,
            });
        }

        let source_lang = source.unwrap_or("auto").to_string();

        // 1. 缓存命中（仅 <500 字符，TTL 24h）
        let cache_key = format!("{text}|{source_lang}|{target}");
        let now = Utc::now().timestamp();
        if text.chars().count() <= 500 {
            if let Some(hit) = self.cache_get(&cache_key).await {
                return Ok(TranslateResult {
                    translated: hit,
                    source_lang,
                    from_cache: true,
                });
            }
        }

        // 2. 组装提示（含术语表注入）
        let glossary_ctx = self.glossary_prompt(&source_lang, target).await?;
        let prompt = build_translate_prompt(text, &source_lang, target, &glossary_ctx);

        // 3. 选模型：显式指定 > preferences 翻译专用 > 默认模型
        let (provider, model_name) = self.resolve_provider(model_id).await?;
        let display = model_name;

        let resp = provider
            .generate(GenerationRequest {
                messages: vec![ChatMessage {
                    role: ChatRole::User,
                    content: MessageContent::Text(prompt),
                    name: None,
                }],
                temperature: Some(0.3),
                ..Default::default()
            })
            .await
            .map_err(|e| AppError::LlmProvider(e.to_string()))?;

        // 4. 校验输出（去包裹引号/代码块围栏/前缀）
        let cleaned = strip_artifacts(&resp.text);

        // 5. 写历史 + 缓存
        self.insert_history(text, &source_lang, target, &cleaned, &display)
            .await?;
        if text.chars().count() <= 500 {
            self.cache_put(&cache_key, cleaned.clone(), now).await;
        }

        Ok(TranslateResult {
            translated: cleaned,
            source_lang,
            from_cache: false,
        })
    }

    /// 缓存读取（TTL 校验）
    async fn cache_get(&self, key: &str) -> Option<String> {
        let now = Utc::now().timestamp();
        let cache = self.cache.lock().await;
        match cache.get(key) {
            Some((hit, ts)) if now - *ts < CACHE_TTL_SECS => Some(hit.clone()),
            _ => None,
        }
    }

    /// 缓存写入
    async fn cache_put(&self, key: &str, value: String, ts: i64) {
        self.cache.lock().await.insert(key.to_string(), (value, ts));
    }

    /// 批量翻译：并发执行（限并发 4），保持输入顺序
    pub async fn batch(
        &self,
        texts: &[String],
        source: Option<&str>,
        target: &str,
    ) -> Result<Vec<TranslateResult>, AppError> {
        let sem = Arc::new(Semaphore::new(BATCH_CONCURRENCY));
        let pool = self.pool.clone();
        let cache = self.cache.clone();
        let mut handles = Vec::with_capacity(texts.len());
        for t in texts {
            let sem = sem.clone();
            let pool = pool.clone();
            let cache = cache.clone();
            let t = t.clone();
            let source = source.map(String::from);
            let target = target.to_string();
            handles.push(tokio::spawn(async move {
                let _permit = sem
                    .acquire()
                    .await
                    .map_err(|_| AppError::Internal("信号量错误".into()))?;
                let svc = TranslateService::new(pool, cache);
                svc.translate(&t, source.as_deref(), &target, None).await
            }));
        }
        let mut results = Vec::with_capacity(texts.len());
        for h in handles {
            results.push(h.await.map_err(|e| AppError::Internal(e.to_string()))??);
        }
        Ok(results)
    }

    /// 整文件翻译（Markdown 保留结构）：按段落分块 → 逐块翻译 → 重组
    pub async fn translate_file(
        &self,
        content: &str,
        source: Option<&str>,
        target: &str,
    ) -> Result<String, AppError> {
        let blocks = split_markdown_blocks(content);
        let mut out = String::new();
        for b in blocks {
            match b.kind {
                BlockKind::Code => out.push_str(&b.text),
                BlockKind::Text => {
                    let r = self.translate(&b.text, source, target, None).await?;
                    out.push_str(&r.translated);
                }
                BlockKind::Heading { level } => {
                    let r = self.translate(&b.text, source, target, None).await?;
                    out.push_str(&format!("{} {}", "#".repeat(level as usize), r.translated));
                }
            }
            out.push('\n');
        }
        Ok(out)
    }

    pub async fn history(
        &self,
        query: Option<&str>,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<TranslateHistoryResult, AppError> {
        let limit = limit.unwrap_or(50);
        let offset = offset.unwrap_or(0);

        let (items, total) = if let Some(q) = query {
            if q.is_empty() {
                self.query_history(None, limit, offset).await?
            } else {
                self.query_history(Some(q), limit, offset).await?
            }
        } else {
            self.query_history(None, limit, offset).await?
        };

        let items: Vec<TranslateHistoryDto> = items
            .into_iter()
            .map(|r| TranslateHistoryDto {
                id: r.id,
                source_text: r.source_text,
                source_lang: r.source_lang,
                target_lang: r.target_lang,
                translated: r.translated,
                created_at: r.created_at,
            })
            .collect();

        Ok(TranslateHistoryResult { items, total })
    }

    pub async fn detect(&self, text: &str) -> Result<DetectResult, AppError> {
        let (lang, confidence) = detect_language_simple(text);
        Ok(DetectResult { lang, confidence })
    }

    pub async fn glossary_for_pair(
        &self,
        source_lang: &str,
        target_lang: &str,
    ) -> Result<Vec<GlossaryTermRow>, AppError> {
        let rows = sqlx::query_as::<_, GlossaryTermRow>(
            "SELECT id, source_lang, target_lang, source_term, target_term, category, enabled, created_at
             FROM glossary_terms
             WHERE source_lang = ? AND target_lang = ? AND enabled = 1",
        )
        .bind(source_lang)
        .bind(target_lang)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// 当前翻译模型配置（preferences: translate.model_id）
    pub async fn model_config(&self) -> Result<Option<String>, AppError> {
        let row = sqlx::query_scalar::<_, String>(
            "SELECT value FROM preferences WHERE key = 'translate.model_id'",
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn set_model_config(&self, model_id: Option<&str>) -> Result<(), AppError> {
        let now = Utc::now().timestamp();
        match model_id {
            Some(mid) => {
                sqlx::query("INSERT OR REPLACE INTO preferences (key, value, updated_at) VALUES ('translate.model_id', ?, ?)")
                    .bind(mid).bind(now)
                    .execute(&self.pool).await?;
            }
            None => {
                sqlx::query("DELETE FROM preferences WHERE key = 'translate.model_id'")
                    .execute(&self.pool)
                    .await?;
            }
        }
        Ok(())
    }

    // ── 内部 ──────────────────────────────────────────────

    async fn query_history(
        &self,
        query: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<crate::data::models::TranslateHistoryRow>, i64), AppError> {
        let (rows, total): (Vec<crate::data::models::TranslateHistoryRow>, i64) = if let Some(q) =
            query
        {
            let rows = sqlx::query_as::<_, crate::data::models::TranslateHistoryRow>(
                    "SELECT th.id, th.source_text, th.source_lang, th.target_lang, th.translated, th.created_at
                     FROM translate_history th
                     JOIN translate_fts fts ON th.rowid = fts.rowid
                     WHERE translate_fts MATCH ?
                     ORDER BY th.created_at DESC LIMIT ? OFFSET ?",
                )
                .bind(q)
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.pool)
                .await?;
            // total 必须是 MATCH 命中的行数，而非整表计数（回归：搜索时 total 虚高）
            let count_row: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM translate_history th
                         JOIN translate_fts fts ON th.rowid = fts.rowid
                         WHERE translate_fts MATCH ?",
            )
            .bind(q)
            .fetch_one(&self.pool)
            .await?;
            (rows, count_row.0)
        } else {
            let rows = sqlx::query_as::<_, crate::data::models::TranslateHistoryRow>(
                "SELECT id, source_text, source_lang, target_lang, translated, created_at
                     FROM translate_history ORDER BY created_at DESC LIMIT ? OFFSET ?",
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
            let count_row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM translate_history")
                .fetch_one(&self.pool)
                .await?;
            (rows, count_row.0)
        };
        Ok((rows, total))
    }

    async fn insert_history(
        &self,
        source_text: &str,
        source_lang: &str,
        target_lang: &str,
        translated: &str,
        model_id: &str,
    ) -> Result<(), AppError> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();
        sqlx::query(
            "INSERT INTO translate_history (id, source_text, source_lang, target_lang, translated, created_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(source_text)
        .bind(source_lang)
        .bind(target_lang)
        .bind(translated)
        .bind(now)
        .execute(&self.pool)
        .await?;
        let _ = model_id;
        Ok(())
    }

    /// 术语表注入：将启用的术语拼成约束说明
    async fn glossary_prompt(
        &self,
        source_lang: &str,
        target_lang: &str,
    ) -> Result<String, AppError> {
        let terms = self.glossary_for_pair(source_lang, target_lang).await?;
        if terms.is_empty() {
            return Ok(String::new());
        }
        let mut lines = Vec::new();
        for t in terms {
            lines.push(format!("{} → {}", t.source_term, t.target_term));
        }
        Ok(format!(
            "必须使用以下术语翻译（术语表）：\n{}\n",
            lines.join(";\n")
        ))
    }

    /// 模型解析：显式 model_id > preferences 翻译专用 > 默认模型
    /// 返回 (provider, model 显示名)
    async fn resolve_provider(
        &self,
        model_id: Option<&str>,
    ) -> Result<(OpenAiProvider, String), AppError> {
        let mid = if let Some(m) = model_id {
            Some(m.to_string())
        } else {
            self.model_config().await?
        };

        let model_row = if let Some(ref mid) = mid {
            sqlx::query_as::<_, crate::data::models::ModelRow>(
                "SELECT id, provider_id, model_id, display_name, kind, max_tokens, is_default, created_at FROM models WHERE id = ?"
            )
            .bind(mid)
            .fetch_optional(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, crate::data::models::ModelRow>(
                "SELECT id, provider_id, model_id, display_name, kind, max_tokens, is_default, created_at FROM models WHERE is_default = 1 LIMIT 1"
            )
            .fetch_optional(&self.pool)
            .await?
        };

        let model_row = model_row.ok_or_else(|| {
            AppError::LlmProvider("未配置模型。请在设置中添加 Provider 并设置默认模型。".into())
        })?;

        let provider_row = sqlx::query_as::<_, crate::data::models::ProviderRow>(
            "SELECT id, name, kind, base_url, api_key_enc, is_enabled, created_at, updated_at FROM providers WHERE id = ?"
        )
        .bind(&model_row.provider_id)
        .fetch_optional(&self.pool)
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
        let display = model_row
            .display_name
            .clone()
            .unwrap_or_else(|| model_row.model_id.clone());

        let provider = OpenAiProvider::new(
            model_row.provider_id.clone(),
            display.clone(),
            api_key,
            base_url,
            model_row.model_id.clone(),
        );
        Ok((provider, display))
    }
}

// ── 提示词与输出清洗 ──────────────────────────────────────

fn build_translate_prompt(
    text: &str,
    source_lang: &str,
    target_lang: &str,
    glossary: &str,
) -> String {
    format!(
        "Translate the following text from {source_lang} to {target_lang}.\n\
         {glossary}\
         Rules: preserve code, formatting, proper nouns and placeholders like {{var}};\n\
         output ONLY the translation without quotes.\n\n{text}"
    )
}

/// 去除 LLM 常见包裹：首尾引号、``` 代码围栏、Translation: 前缀
pub fn strip_artifacts(text: &str) -> String {
    let mut out = text.trim().to_string();

    // 去掉 "Translation:" / "翻译：" 前缀
    for prefix in ["Translation:", "翻译：", "翻译:", "Translated text:"] {
        if let Some(rest) = out.strip_prefix(prefix) {
            out = rest.trim().to_string();
            break;
        }
    }

    // 去掉 ```...``` 围栏
    if out.starts_with("```") && out.ends_with("```") {
        let inner = &out[3..out.len() - 3];
        out = inner.trim().to_string();
    }

    // 去掉首尾成对引号（英文/中文弯引号 + 直引号）
    let trimmed = out.trim();
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() >= 2 {
        let (open, close) = (chars[0], chars[chars.len() - 1]);
        let is_pair = matches!(
            (open, close),
            ('"', '"') | ('\'', '\'') | ('“', '”') | ('‘', '’') | ('「', '」')
        );
        if is_pair {
            out = chars[1..chars.len() - 1]
                .iter()
                .collect::<String>()
                .trim()
                .to_string();
        }
    }

    out
}

// ── Markdown 分块 ─────────────────────────────────────────

enum BlockKind {
    Code,
    Text,
    Heading { level: u8 },
}

struct MdBlock {
    kind: BlockKind,
    text: String,
}

/// 拆分 Markdown：代码块/标题/正文分离（代码不翻译，标题保留 # 前缀）
fn split_markdown_blocks(content: &str) -> Vec<MdBlock> {
    let mut blocks = Vec::new();
    let mut in_code = false;
    let mut code_buf = String::new();
    let mut text_buf = String::new();

    let flush = |blocks: &mut Vec<MdBlock>,
                 in_code: &mut bool,
                 code_buf: &mut String,
                 text_buf: &mut String| {
        if !code_buf.is_empty() {
            blocks.push(MdBlock {
                kind: BlockKind::Code,
                text: std::mem::take(code_buf),
            });
        }
        if !text_buf.is_empty() {
            blocks.push(MdBlock {
                kind: BlockKind::Text,
                text: std::mem::take(text_buf),
            });
        }
        *in_code = false;
    };

    for line in content.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            if in_code {
                code_buf.push_str(line);
                code_buf.push('\n');
                flush(&mut blocks, &mut in_code, &mut code_buf, &mut text_buf);
            } else {
                // 代码块开始前先 flush 文本
                if !text_buf.is_empty() {
                    blocks.push(MdBlock {
                        kind: BlockKind::Text,
                        text: std::mem::take(&mut text_buf),
                    });
                }
                in_code = true;
                code_buf.push_str(line);
                code_buf.push('\n');
            }
            continue;
        }
        if in_code {
            code_buf.push_str(line);
            code_buf.push('\n');
            continue;
        }
        // 标题
        if trimmed.starts_with('#') {
            let level = trimmed.chars().take_while(|c| *c == '#').count() as u8;
            let level_usize = level as usize;
            if level <= 6 && trimmed.chars().nth(level_usize) == Some(' ') {
                if !text_buf.is_empty() {
                    blocks.push(MdBlock {
                        kind: BlockKind::Text,
                        text: std::mem::take(&mut text_buf),
                    });
                }
                blocks.push(MdBlock {
                    kind: BlockKind::Heading { level },
                    text: trimmed[level_usize..].trim().to_string(),
                });
                continue;
            }
        }
        text_buf.push_str(line);
        text_buf.push('\n');
    }
    flush(&mut blocks, &mut in_code, &mut code_buf, &mut text_buf);
    blocks
}

// ── 语言检测 ──────────────────────────────────────────────

fn detect_language_simple(text: &str) -> (String, f32) {
    let total = text.chars().count() as f32;
    if total == 0.0 {
        return ("unknown".to_string(), 0.0);
    }

    let cjk_count = text.chars().filter(|c| is_cjk(*c)).count() as f32;
    let latin_count = text.chars().filter(|c| c.is_ascii_alphabetic()).count() as f32;

    let cjk_ratio = cjk_count / total;
    let latin_ratio = latin_count / total;

    if cjk_ratio > 0.3 {
        ("zh".to_string(), cjk_ratio.min(0.95))
    } else if latin_ratio > 0.5 {
        ("en".to_string(), latin_ratio.min(0.95))
    } else {
        ("unknown".to_string(), 0.3)
    }
}

fn is_cjk(c: char) -> bool {
    let cp = c as u32;
    (0x4E00..=0x9FFF).contains(&cp)
        || (0x3400..=0x4DBF).contains(&cp)
        || (0x20000..=0x2A6DF).contains(&cp)
        || (0x2A700..=0x2B73F).contains(&cp)
        || (0xF900..=0xFAFF).contains(&cp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_quotes_and_prefix() {
        assert_eq!(strip_artifacts("\"hello\""), "hello");
        assert_eq!(strip_artifacts("Translation: hello"), "hello");
        assert_eq!(strip_artifacts("```\nhello\n```"), "hello");
        assert_eq!(strip_artifacts("“你好”"), "你好");
        assert_eq!(strip_artifacts("plain text"), "plain text");
    }

    #[test]
    fn split_blocks_code_and_heading() {
        let md = "# Title\n\ntext here\n\n```rust\nfn main() {}\n```\n\nmore text";
        let blocks = split_markdown_blocks(md);
        assert!(blocks
            .iter()
            .any(|b| matches!(b.kind, BlockKind::Heading { level: 1 })));
        assert!(blocks.iter().any(|b| matches!(b.kind, BlockKind::Code)));
        assert!(blocks.iter().any(|b| matches!(b.kind, BlockKind::Text)));
    }

    #[test]
    fn detect_zh_en() {
        assert_eq!(detect_language_simple("今天天气很好").0, "zh");
        assert_eq!(detect_language_simple("hello world this is a test").0, "en");
    }

    /// 共享缓存跨服务实例生效（回归：缓存之前随服务实例销毁而失效）
    #[tokio::test]
    async fn shared_cache_hit_across_instances() {
        let cache: Arc<tokio::sync::Mutex<HashMap<String, (String, i64)>>> =
            Arc::new(tokio::sync::Mutex::new(HashMap::new()));
        let now = chrono::Utc::now().timestamp();

        let key = "hello|auto|zh".to_string();
        // 实例 A 写入
        {
            let mut guard = cache.lock().await;
            guard.insert(key.clone(), ("你好".to_string(), now));
        }

        // 实例 B（新 TranslateService，但共享同一 Arc cache）读取命中
        let db_dir = std::env::temp_dir().join(format!("prism_tr_cache_{}", uuid::Uuid::new_v4()));
        let db = crate::data::db::Database::new(&db_dir).await.unwrap();
        let svc_b = TranslateService::new(db.pool.clone(), cache.clone());
        let hit = svc_b.cache_get(&key).await;
        assert_eq!(hit.as_deref(), Some("你好"), "共享缓存跨实例必须命中");
        let _ = std::fs::remove_dir_all(&db_dir);
    }

    /// FTS 搜索 total 统计（回归：之前 total 用整表计数，搜索时虚高）
    #[tokio::test]
    async fn history_search_total_counts_matches_only() {
        let db_dir = std::env::temp_dir().join(format!("prism_tr_fts_{}", uuid::Uuid::new_v4()));
        let db = crate::data::db::Database::new(&db_dir).await.unwrap();
        let cache: Arc<tokio::sync::Mutex<HashMap<String, (String, i64)>>> =
            Arc::new(tokio::sync::Mutex::new(HashMap::new()));
        let svc = TranslateService::new(db.pool.clone(), cache);
        let now = chrono::Utc::now().timestamp_millis();

        // 2 条历史：1 条含 "kubernetes"，1 条不含
        for (i, (src, trans)) in [
            ("hello kubernetes world", "你好 kubernetes 世界"),
            ("apple pie", "苹果派"),
        ]
        .iter()
        .enumerate()
        {
            let id = uuid::Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO translate_history (id, source_text, source_lang, target_lang, translated, created_at) VALUES (?1, ?2, 'en', 'zh', ?3, ?4)"
            )
            .bind(&id).bind(src).bind(trans).bind(now + i as i64)
            .execute(&db.pool).await.unwrap();
        }

        let result = svc
            .history(Some("kubernetes"), Some(10), Some(0))
            .await
            .unwrap();
        assert_eq!(result.items.len(), 1, "只应命中 1 条");
        assert_eq!(result.total, 1, "total 必须是命中数而非整表计数");

        // 无搜索：total = 整表 2
        let all = svc.history(None, Some(10), Some(0)).await.unwrap();
        assert_eq!(all.total, 2);

        let _ = std::fs::remove_dir_all(&db_dir);
    }
}
