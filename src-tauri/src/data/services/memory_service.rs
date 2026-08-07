use std::collections::HashSet;
use std::path::{Path, PathBuf};

use sqlx::Row;
use tokio::io::AsyncWriteExt;

use crate::core::adk::memory::{MemoryContext, MemoryItem, MemoryScope, MemoryStore, MessageExchange};
use crate::core::adk::error::AgentError;
use crate::data::Database;
use crate::utils::error::AppError;

// ── 记忆服务 ──────────────────────────────────────────────

const MAX_BODY_CHARS: usize = 500_000;
const FTS_LIMIT: i64 = 20;

pub struct MemoryService {
    db: Database,
    base_dir: PathBuf,
}

impl MemoryService {
    pub fn new(db: Database, base_dir: PathBuf) -> Self {
        Self { db, base_dir }
    }

    /// 回填/重建 memory_fts 索引：扫描 global/projects/sessions 下所有 .md 文件，
    /// 每个文件一行（body 为全文，path 哈希作 rowid），并清理已不存在的文件索引。
    /// 返回本次索引的文件数。
    pub async fn reconcile(&self) -> Result<u64, AppError> {
        let mut files = Vec::new();
        for sub in ["global", "projects", "sessions"] {
            let root = self.base_dir.join(sub);
            if root.is_dir() {
                collect_md_files(&root, &mut files).await?;
            }
        }

        for path in &files {
            self.index_file(path).await?;
        }

        // 清理索引中已不存在的文件
        let current: HashSet<String> = files
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        let rows = sqlx::query("SELECT path FROM memory_fts")
            .fetch_all(&self.db.pool)
            .await?;
        for row in rows {
            let path: String = row.try_get("path")?;
            if !current.contains(&path) {
                sqlx::query("DELETE FROM memory_fts WHERE path = ?")
                    .bind(&path)
                    .execute(&self.db.pool)
                    .await?;
            }
        }

        Ok(files.len() as u64)
    }

    /// 搜索记忆：优先走 memory_fts（FTS5/BM25），查询为空返回空，
    /// FTS 出错或结果为空时回退到朴素文件扫描。
    pub async fn search(&self, query: &str) -> Result<Vec<MemorySearchHit>, AppError> {
        let Some(fts_query) = sanitize_fts_query(query) else {
            return Ok(Vec::new());
        };
        let fts_hits = match self.search_fts(&fts_query, query).await {
            Ok(hits) => hits,
            Err(_) => Vec::new(),
        };
        if fts_hits.is_empty() {
            return self.search_files(query).await;
        }
        Ok(fts_hits)
    }

    /// FTS5 全文搜索（BM25 排序）
    async fn search_fts(&self, fts_query: &str, raw_query: &str) -> Result<Vec<MemorySearchHit>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT body, scope, path, bm25(memory_fts) AS rank
            FROM memory_fts
            WHERE memory_fts MATCH ?
            ORDER BY rank
            LIMIT ?
            "#,
        )
        .bind(fts_query)
        .bind(FTS_LIMIT)
        .fetch_all(&self.db.pool)
        .await?;

        let mut hits = Vec::with_capacity(rows.len());
        for row in rows {
            let body: String = row.try_get("body")?;
            let scope: String = row.try_get("scope")?;
            let path: String = row.try_get("path")?;
            let rank: f64 = row.try_get("rank")?;
            hits.push(MemorySearchHit {
                path,
                scope,
                snippet: extract_snippet(&body, raw_query),
                // bm25 越小越优，取负保持「分数越高越相关」的既有语义
                score: -rank,
            });
        }
        Ok(hits)
    }

    /// 回退实现：朴素子串扫描
    async fn search_files(&self, query: &str) -> Result<Vec<MemorySearchHit>, AppError> {
        let mut hits = Vec::new();

        // 搜索全局记忆
        let global_path = self.base_dir.join("global").join("MEMORY.md");
        if global_path.exists() {
            let content = tokio::fs::read_to_string(&global_path).await?;
            if content.contains(query) {
                hits.push(MemorySearchHit {
                    path: global_path.to_string_lossy().to_string(),
                    scope: "global".to_string(),
                    snippet: extract_snippet(&content, query),
                    score: 1.0,
                });
            }
        }

        // 搜索项目记忆
        let projects_dir = self.base_dir.join("projects");
        if projects_dir.exists() {
            let mut entries = tokio::fs::read_dir(&projects_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if path.is_dir() {
                    let memory_path = path.join("MEMORY.md");
                    if memory_path.exists() {
                        let content = tokio::fs::read_to_string(&memory_path).await?;
                        if content.contains(query) {
                            hits.push(MemorySearchHit {
                                path: memory_path.to_string_lossy().to_string(),
                                scope: "project".to_string(),
                                snippet: extract_snippet(&content, query),
                                score: 0.8,
                            });
                        }
                    }
                }
            }
        }

        // 搜索会话记忆
        let sessions_dir = self.base_dir.join("sessions");
        if sessions_dir.exists() {
            let mut entries = tokio::fs::read_dir(&sessions_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if path.is_dir() {
                    // 搜索 checkpoint.md
                    let checkpoint_path = path.join("checkpoint.md");
                    if checkpoint_path.exists() {
                        let content = tokio::fs::read_to_string(&checkpoint_path).await?;
                        if content.contains(query) {
                            hits.push(MemorySearchHit {
                                path: checkpoint_path.to_string_lossy().to_string(),
                                scope: "session".to_string(),
                                snippet: extract_snippet(&content, query),
                                score: 0.6,
                            });
                        }
                    }
                    // 搜索 notes.md
                    let notes_path = path.join("notes.md");
                    if notes_path.exists() {
                        let content = tokio::fs::read_to_string(&notes_path).await?;
                        if content.contains(query) {
                            hits.push(MemorySearchHit {
                                path: notes_path.to_string_lossy().to_string(),
                                scope: "session".to_string(),
                                snippet: extract_snippet(&content, query),
                                score: 0.5,
                            });
                        }
                    }
                }
            }
        }

        // 按分数排序
        hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        Ok(hits)
    }

    /// 将单个 .md 文件索引进 memory_fts（先删旧行再插入，rowid 为路径稳定哈希）
    async fn index_file(&self, path: &Path) -> Result<(), AppError> {
        let content = tokio::fs::read_to_string(path).await?;
        let body: String = content.chars().take(MAX_BODY_CHARS).collect();
        let scope = scope_for_path(&self.base_dir, path).to_string();
        let rowid = stable_hash(&path.to_string_lossy());
        let path_str = path.to_string_lossy().into_owned();

        sqlx::query("DELETE FROM memory_fts WHERE path = ?")
            .bind(&path_str)
            .execute(&self.db.pool)
            .await?;
        sqlx::query(
            "INSERT INTO memory_fts(rowid, body, fingerprint, scope, type, path) VALUES (?, ?, '', ?, 'memory', ?)",
        )
        .bind(rowid)
        .bind(&body)
        .bind(&scope)
        .bind(&path_str)
        .execute(&self.db.pool)
        .await?;
        Ok(())
    }

    /// 读取记忆文件
    pub async fn read(&self, path: &str) -> Result<String, AppError> {
        let path = PathBuf::from(path);
        if !path.exists() {
            return Err(AppError::Validation(format!("文件不存在: {path:?}")));
        }
        let content = tokio::fs::read_to_string(&path).await?;
        Ok(content)
    }

    /// 写入记忆文件
    pub async fn write(&self, path: &str, content: &str) -> Result<(), AppError> {
        let path = PathBuf::from(path);

        // 安全校验：路径必须在 base_dir 下
        if !path.starts_with(&self.base_dir) {
            return Err(AppError::Validation("路径不安全".into()));
        }

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&path, content).await?;
        Ok(())
    }

    /// 获取当前注入的记忆上下文（用于调试）
    pub async fn context_dump(&self) -> Result<Vec<MemoryDump>, AppError> {
        let mut dumps = Vec::new();

        // 全局记忆
        let global_path = self.base_dir.join("global").join("MEMORY.md");
        if global_path.exists() {
            let content = tokio::fs::read_to_string(&global_path).await?;
            dumps.push(MemoryDump {
                path: global_path.to_string_lossy().to_string(),
                scope: "global".to_string(),
                size: content.len(),
                preview: content.chars().take(200).collect(),
            });
        }

        // 项目记忆
        let projects_dir = self.base_dir.join("projects");
        if projects_dir.exists() {
            let mut entries = tokio::fs::read_dir(&projects_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if path.is_dir() {
                    let memory_path = path.join("MEMORY.md");
                    if memory_path.exists() {
                        let content = tokio::fs::read_to_string(&memory_path).await?;
                        dumps.push(MemoryDump {
                            path: memory_path.to_string_lossy().to_string(),
                            scope: "project".to_string(),
                            size: content.len(),
                            preview: content.chars().take(200).collect(),
                        });
                    }
                }
            }
        }

        Ok(dumps)
    }
}

// ── MemoryStore 实现 ──────────────────────────────────────

#[async_trait::async_trait]
impl MemoryStore for MemoryService {
    async fn build_context(
        &self,
        _session_id: &str,
        _agent_id: &str,
    ) -> Result<MemoryContext, AgentError> {
        let mut summary = String::new();

        // 加载全局记忆
        let global_path = self.base_dir.join("global").join("MEMORY.md");
        if global_path.exists() {
            if let Ok(content) = tokio::fs::read_to_string(&global_path).await {
                summary.push_str(&format!("# 全局记忆\n{content}\n\n"));
            }
        }

        // 加载项目记忆
        let projects_dir = self.base_dir.join("projects");
        if projects_dir.exists() {
            if let Ok(mut entries) = tokio::fs::read_dir(&projects_dir).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();
                    if path.is_dir() {
                        let memory_path = path.join("MEMORY.md");
                        if memory_path.exists() {
                            if let Ok(content) = tokio::fs::read_to_string(&memory_path).await {
                                summary.push_str(&format!("# 项目记忆\n{content}\n\n"));
                            }
                        }
                    }
                }
            }
        }

        Ok(MemoryContext {
            summary,
            items: Vec::new(),
        })
    }

    async fn record(
        &self,
        session_id: &str,
        _agent_id: &str,
        exchange: MessageExchange,
    ) -> Result<(), AgentError> {
        // 追加到 sessions/<session_id>/notes.md，随后增量重建该文件的 FTS 索引
        let dir = self.base_dir.join("sessions").join(session_id);
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| AgentError::Internal(e.to_string()))?;
        let notes_path = dir.join("notes.md");
        let entry = format!(
            "\n## user\n{}\n\n## assistant\n{}\n",
            exchange.user_message, exchange.assistant_message
        );
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&notes_path)
            .await
            .map_err(|e| AgentError::Internal(e.to_string()))?;
        file.write_all(entry.as_bytes())
            .await
            .map_err(|e| AgentError::Internal(e.to_string()))?;

        self.index_file(&notes_path)
            .await
            .map_err(|e| AgentError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn search(
        &self,
        query: &str,
        _scope: Option<MemoryScope>,
    ) -> Result<Vec<MemoryItem>, AgentError> {
        let hits = self.search(query).await.map_err(|e| AgentError::Internal(e.to_string()))?;
        Ok(hits.into_iter().map(|h| MemoryItem {
            path: h.path,
            body: h.snippet,
            scope: MemoryScope::Global,
            memory_type: "memory".to_string(),
        }).collect())
    }
}

// ── 返回类型 ──────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemorySearchHit {
    pub path: String,
    pub scope: String,
    pub snippet: String,
    pub score: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemoryDump {
    pub path: String,
    pub scope: String,
    pub size: usize,
    pub preview: String,
}

// ── 辅助函数 ──────────────────────────────────────────────

/// FNV-1a 64 位路径哈希，跨进程/跨版本稳定，用作 memory_fts 的 rowid
fn stable_hash(s: &str) -> i64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    (h & 0x7fff_ffff_ffff_ffff) as i64
}

/// 过滤 FTS5 查询语法特殊字符，将剩余词用引号包成短语。
/// 空查询返回 None，调用方应返回空结果。
fn sanitize_fts_query(query: &str) -> Option<String> {
    let mut cleaned = String::with_capacity(query.len());
    for c in query.chars() {
        if matches!(
            c,
            '"' | '*' | '^' | '(' | ')' | '{' | '}' | '[' | ']'
                | '-' | '+' | ':' | '~' | '!' | '|' | '&' | '<' | '>' | '=' | '/' | '\\'
        ) {
            cleaned.push(' ');
        } else {
            cleaned.push(c);
        }
    }
    let tokens: Vec<&str> = cleaned.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }
    let mut out = String::new();
    for (i, token) in tokens.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push('"');
        out.push_str(token);
        out.push('"');
    }
    Some(out)
}

/// 由相对路径首段推断 memory_fts.scope 列的值
fn scope_for_path(base: &Path, path: &Path) -> &'static str {
    let Some(rel) = path.strip_prefix(base).ok() else {
        return "memory";
    };
    match rel.components().next().and_then(|c| c.as_os_str().to_str()) {
        Some("global") => "global",
        Some("projects") => "project",
        Some("sessions") => "session",
        _ => "memory",
    }
}

/// 递归收集目录下所有 .md 文件
async fn collect_md_files(root: &Path, out: &mut Vec<PathBuf>) -> Result<(), AppError> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let mut entries = tokio::fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let ft = entry.file_type().await?;
            if ft.is_dir() {
                stack.push(path);
            } else if path.extension().map(|e| e == "md").unwrap_or(false) {
                out.push(path);
            }
        }
    }
    Ok(())
}

fn extract_snippet(content: &str, query: &str) -> String {
    if let Some(pos) = content.find(query) {
        let start = pos.saturating_sub(100);
        let end = (pos + query.len() + 100).min(content.len());
        let snippet = &content[start..end];
        format!("...{snippet}...")
    } else {
        content.chars().take(200).collect()
    }
}
