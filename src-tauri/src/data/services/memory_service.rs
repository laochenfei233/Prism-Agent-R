use std::path::PathBuf;

use crate::core::adk::memory::{MemoryContext, MemoryItem, MemoryScope, MemoryStore, MessageExchange};
use crate::core::adk::error::AgentError;
use crate::data::Database;
use crate::utils::error::AppError;

// ── 记忆服务 ──────────────────────────────────────────────

pub struct MemoryService {
    #[allow(dead_code)]
    db: Database,
    base_dir: PathBuf,
}

impl MemoryService {
    pub fn new(db: Database, base_dir: PathBuf) -> Self {
        Self { db, base_dir }
    }

    /// 搜索记忆
    pub async fn search(&self, query: &str) -> Result<Vec<MemorySearchHit>, AppError> {
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
        _session_id: &str,
        _agent_id: &str,
        _exchange: MessageExchange,
    ) -> Result<(), AgentError> {
        // MVP 阶段不自动记录，由 checkpoint-writer 处理
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
