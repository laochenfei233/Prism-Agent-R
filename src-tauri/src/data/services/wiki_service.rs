use std::path::Path;

use crate::data::models::{WikiRow, WikiPage, WikiPageHit};
use crate::data::Database;
use crate::utils::error::AppError;
use crate::utils::paths::wiki_dir;

// ── Wiki 服务 ──────────────────────────────────────────────

pub struct WikiService {
    db: Database,
}

impl WikiService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// 创建知识库（DB 插入 + 创建磁盘目录）
    pub async fn create_wiki(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> Result<WikiRow, AppError> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();

        sqlx::query("INSERT INTO wikis (id, name, description, created_at, updated_at) VALUES (?, ?, ?, ?, ?)")
            .bind(&id)
            .bind(name)
            .bind(description)
            .bind(now)
            .bind(now)
            .execute(&self.db.pool)
            .await?;

        // 创建磁盘目录
        let wiki_page_dir = wiki_dir().join(&id).join("wiki");
        tokio::fs::create_dir_all(&wiki_page_dir).await?;

        let row = sqlx::query_as::<_, WikiRow>("SELECT * FROM wikis WHERE id = ?")
            .bind(&id)
            .fetch_one(&self.db.pool)
            .await?;

        Ok(row)
    }

    /// 列出所有知识库
    pub async fn list_wikis(&self) -> Result<Vec<WikiRow>, AppError> {
        let rows = sqlx::query_as::<_, WikiRow>("SELECT * FROM wikis ORDER BY updated_at DESC")
            .fetch_all(&self.db.pool)
            .await?;
        Ok(rows)
    }

    /// 获取单个知识库
    pub async fn get_wiki(&self, id: &str) -> Result<WikiRow, AppError> {
        let row = sqlx::query_as::<_, WikiRow>("SELECT * FROM wikis WHERE id = ?")
            .bind(id)
            .fetch_one(&self.db.pool)
            .await?;
        Ok(row)
    }

    /// 删除知识库（DB 删除 + 磁盘清理）
    pub async fn delete_wiki(&self, id: &str) -> Result<(), AppError> {
        sqlx::query("DELETE FROM wikis WHERE id = ?")
            .bind(id)
            .execute(&self.db.pool)
            .await?;

        // 清理磁盘目录
        let wiki_root = wiki_dir().join(id);
        if wiki_root.exists() {
            tokio::fs::remove_dir_all(&wiki_root).await?;
        }

        Ok(())
    }

    /// 读取 wiki 页面
    pub async fn read_page(&self, wiki_id: &str, path: &str) -> Result<String, AppError> {
        validate_page_path(path)?;

        let file_path = wiki_dir()
            .join(wiki_id)
            .join("wiki")
            .join(path);

        // 安全校验：确保路径在 wiki 目录下
        let wiki_base = wiki_dir().join(wiki_id).join("wiki");
        if !file_path.starts_with(&wiki_base) {
            return Err(AppError::Validation("路径不安全".into()));
        }

        if !file_path.exists() {
            return Err(AppError::Validation(format!("页面不存在: {path}")));
        }

        let content = tokio::fs::read_to_string(&file_path).await?;
        Ok(content)
    }

    /// 写入 wiki 页面
    pub async fn write_page(
        &self,
        wiki_id: &str,
        path: &str,
        content: &str,
    ) -> Result<(), AppError> {
        validate_page_path(path)?;

        let file_path = wiki_dir()
            .join(wiki_id)
            .join("wiki")
            .join(path);

        // 安全校验：确保路径在 wiki 目录下
        let wiki_base = wiki_dir().join(wiki_id).join("wiki");
        if !file_path.starts_with(&wiki_base) {
            return Err(AppError::Validation("路径不安全".into()));
        }

        // 确保父目录存在
        if let Some(parent) = file_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(&file_path, content).await?;

        // 更新 updated_at
        let now = chrono::Utc::now().timestamp();
        sqlx::query("UPDATE wikis SET updated_at = ? WHERE id = ?")
            .bind(now)
            .bind(wiki_id)
            .execute(&self.db.pool)
            .await?;

        Ok(())
    }

    /// 列出页面
    pub async fn list_pages(&self, wiki_id: &str) -> Result<Vec<WikiPage>, AppError> {
        let wiki_page_dir = wiki_dir().join(wiki_id).join("wiki");

        if !wiki_page_dir.exists() {
            return Ok(Vec::new());
        }

        let mut pages = Vec::new();
        collect_pages(&wiki_page_dir, &wiki_page_dir, &mut pages).await?;
        pages.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(pages)
    }

    /// 全文搜索页面
    pub async fn search_pages(
        &self,
        wiki_id: &str,
        query: &str,
    ) -> Result<Vec<WikiPageHit>, AppError> {
        let pages = self.list_pages(wiki_id).await?;
        let mut hits = Vec::new();

        for page in pages {
            let file_path = wiki_dir()
                .join(wiki_id)
                .join("wiki")
                .join(&page.path);

            let content = tokio::fs::read_to_string(&file_path).await?;
            if let Some(pos) = content.find(query) {
                let start = pos.saturating_sub(100);
                let end = (pos + query.len() + 100).min(content.len());
                let snippet = &content[start..end];
                hits.push(WikiPageHit {
                    path: page.path,
                    title: page.title,
                    snippet: format!("...{snippet}..."),
                    score: 1.0,
                });
            }
        }

        Ok(hits)
    }

    /// LLM 自动更新（§10.1.1）：组装上下文 → LLM 生成 WikiWritePlan → 解析（失败重试 1 次）
    /// preview=true 仅返回计划不执行；否则执行并返回变更摘要
    pub async fn write_ai(
        &self,
        wiki_id: &str,
        info: &str,
        preview: bool,
    ) -> Result<serde_json::Value, AppError> {
        use crate::data::models::WikiWritePlan;
        use crate::core::adk::model::{ChatMessage, ChatRole, MessageContent, ModelProvider};

        // 1. 组装上下文（SCHEMA + index 前 200 行 + 页面清单摘要 + 新信息）
        let wiki_root = wiki_dir().join(wiki_id).join("wiki");
        let index_text = read_if_exists(&wiki_root.join("index.md")).await;
        let pages = self.list_pages(wiki_id).await?;
        let page_summary: String = pages
            .iter()
            .map(|p| format!("- {} (标题: {})", p.path, p.title))
            .take(50)
            .collect::<Vec<_>>()
            .join("\n");

        let system = "你是知识库管理员。根据现有页面结构与新信息，输出一个 Wiki 写入计划（JSON）。\
             \n可用操作：{\"operations\": [{\"op\":\"create_page\",\"path\":\"concepts/xx.md\",\"title\":\"xx\",\"content\":\"...\"}, \
             {\"op\":\"update_page\",\"path\":\"...\",\"content\":\"全文\",\"summary\":\"变更说明\"}, \
             {\"op\":\"delete_page\",\"path\":\"...\",\"reason\":\"...\"}, \
             {\"op\":\"update_index\",\"entries\":[\"- [xx](concepts/xx.md)\"]}, \
             {\"op\":\"noop\",\"reason\":\"...\"}]}\n\
             path 必须相对 wiki/ 根且以 .md 结尾，不含 ..。信息与现有内容重复时用 noop。只输出 JSON。";

        let user = format!(
            "现有页面索引（index.md 前 200 行）：\n{}\n\n页面清单：\n{}\n\n要写入的新知识：\n{}",
            truncate_md(&index_text, 200),
            page_summary,
            info
        );

        let (provider, _display) = resolve_wiki_model(&self.db).await?;
        let mut last_err = String::new();
        let mut plan: Option<WikiWritePlan> = None;

        for attempt in 0..2 {
            let prompt = if attempt == 0 {
                user.clone()
            } else {
                format!("{user}\n\n上次输出解析失败：{last_err}\n请严格按 JSON 格式重新输出。")
            };
            let resp = provider
                .generate(crate::core::adk::model::GenerationRequest {
                    messages: vec![
                        ChatMessage { role: ChatRole::System, content: MessageContent::Text(system.into()), name: None },
                        ChatMessage { role: ChatRole::User, content: MessageContent::Text(prompt), name: None },
                    ],
                    temperature: Some(0.2),
                    ..Default::default()
                })
                .await
                .map_err(|e| AppError::LlmProvider(e.to_string()))?;
            match extract_json(&resp.text).and_then(|v| serde_json::from_value::<WikiWritePlan>(v).ok()) {
                Some(p) => { plan = Some(p); break; }
                None => last_err = truncate_md(&resp.text, 200),
            }
        }

        let plan = plan.ok_or_else(|| AppError::LlmProvider(format!("无法解析 Wiki 写入计划: {last_err}")))?;

        // 2. 校验操作数上限（防 LLM 失控）
        if plan.operations.len() > 10 {
            return Err(AppError::Validation("操作数超过 10 个，已截断拒绝".into()));
        }

        if preview {
            return Ok(serde_json::json!({ "plan": plan }));
        }

        // 3. 执行
        let result = self.apply_plan(wiki_id, &plan).await?;
        let _ = result; // 由 apply_plan 返回摘要
        Ok(serde_json::json!({ "plan": plan, "result": "applied" }))
    }

    /// 执行 WikiWritePlan（§10.1.1 执行流程：逐操作落盘 + log.md 追加）
    pub async fn apply_plan(&self, wiki_id: &str, plan: &crate::data::models::WikiWritePlan) -> Result<crate::data::models::WikiWriteResult, AppError> {
        use crate::data::models::WikiOp;

        let wiki_root = wiki_dir().join(wiki_id).join("wiki");
        let trash_dir = wiki_dir().join(wiki_id).join(".trash");
        tokio::fs::create_dir_all(&wiki_root).await?;

        let mut applied = 0usize;
        let mut noop = 0usize;
        let mut ops_log = String::new();
        let mut index_entries: Vec<String> = Vec::new();

        for op in &plan.operations {
            match op {
                WikiOp::CreatePage { path, title, content } => {
                    validate_page_path(path)?;
                    let target = wiki_root.join(path);
                    ensure_within(&target, &wiki_root)?;
                    if let Some(parent) = target.parent() {
                        tokio::fs::create_dir_all(parent).await?;
                    }
                    let page = format!("# {title}\n\n{content}\n");
                    tokio::fs::write(&target, page).await?;
                    ops_log.push_str(&format!("- CreatePage: {path} (新页面)\n"));
                    index_entries.push(format!("- [{title}]({path})"));
                    applied += 1;
                }
                WikiOp::UpdatePage { path, content, summary } => {
                    validate_page_path(path)?;
                    let target = wiki_root.join(path);
                    ensure_within(&target, &wiki_root)?;
                    if target.exists() {
                        // 备份 .bak
                        let _ = tokio::fs::copy(&target, format!("{}.bak", target.display())).await;
                    }
                    if let Some(parent) = target.parent() {
                        tokio::fs::create_dir_all(parent).await?;
                    }
                    tokio::fs::write(&target, content).await?;
                    ops_log.push_str(&format!("- UpdatePage: {path} ({summary})\n"));
                    applied += 1;
                }
                WikiOp::DeletePage { path, reason } => {
                    validate_page_path(path)?;
                    let target = wiki_root.join(path);
                    ensure_within(&target, &wiki_root)?;
                    if target.exists() {
                        tokio::fs::create_dir_all(&trash_dir).await?;
                        let name = target.file_name().unwrap_or_default();
                        let dest = trash_dir.join(name);
                        tokio::fs::rename(&target, &dest).await?;
                        ops_log.push_str(&format!("- DeletePage: {path} (移至 .trash，原因: {reason})\n"));
                        applied += 1;
                    }
                }
                WikiOp::UpdateIndex { entries } => {
                    index_entries.extend(entries.clone());
                    applied += 1;
                }
                WikiOp::Noop { reason } => {
                    noop += 1;
                    ops_log.push_str(&format!("- Noop: {reason}\n"));
                }
            }
        }

        // index.md 追加
        if !index_entries.is_empty() {
            let index_path = wiki_root.join("index.md");
            let mut existing = read_if_exists(&index_path).await;
            if existing.trim().is_empty() {
                existing = "# Index\n".into();
            }
            if !existing.ends_with('\n') {
                existing.push('\n');
            }
            existing.push_str(&format!("\n## 更新 {}\n", chrono::Utc::now().format("%Y-%m-%d %H:%M")));
            for e in index_entries {
                existing.push_str(&e);
                existing.push('\n');
            }
            tokio::fs::write(&index_path, existing).await?;
        }

        // log.md 变更记录
        let log_path = wiki_root.join("log.md");
        let mut log = read_if_exists(&log_path).await;
        if log.trim().is_empty() {
            log = "# Log\n".into();
        }
        if !log.ends_with('\n') {
            log.push('\n');
        }
        log.push_str(&format!(
            "\n## [{}Z] ai-write | Wiki Updated\n\nSource: 对话导入 · 触发: write_ai\nOps:\n{}Result: {applied} ops applied, {noop} noop\n",
            chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S"),
            ops_log
        ));
        tokio::fs::write(&log_path, log).await?;

        // 更新 updated_at
        sqlx::query("UPDATE wikis SET updated_at = ? WHERE id = ?")
            .bind(chrono::Utc::now().timestamp())
            .bind(wiki_id)
            .execute(&self.db.pool)
            .await?;

        Ok(crate::data::models::WikiWriteResult {
            applied,
            noop,
            summary: format!("{applied} ops applied, {noop} noop"),
            log_appended: true,
        })
    }
}

// ── 辅助函数 ──────────────────────────────────────────────

/// 验证页面路径安全性
fn validate_page_path(path: &str) -> Result<(), AppError> {
    if path.contains("..") {
        return Err(AppError::Validation("路径不能包含 '..'".into()));
    }
    if path.starts_with('/') || path.starts_with('\\') {
        return Err(AppError::Validation("路径不能以 '/' 或 '\\' 开头".into()));
    }
    Ok(())
}

/// 递归收集所有 .md 文件
async fn collect_pages(
    root: &Path,
    base: &Path,
    out: &mut Vec<WikiPage>,
) -> Result<(), AppError> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let mut entries = tokio::fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let ft = entry.file_type().await?;
            if ft.is_dir() {
                stack.push(path);
            } else if path.extension().map(|e| e == "md").unwrap_or(false) {
                let metadata = tokio::fs::metadata(&path).await?;
                let rel_path = path
                    .strip_prefix(base)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                let title = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                out.push(WikiPage {
                    path: rel_path,
                    title,
                    size: metadata.len() as usize,
                });
            }
        }
    }
    Ok(())
}

// ── AI 写入辅助 ───────────────────────────────────────────

/// 解析默认模型构建 provider（Wiki AI 功能复用）
async fn resolve_wiki_model(db: &Database) -> Result<(crate::core::rig::provider::OpenAiProvider, String), AppError> {
    use crate::data::models::{ModelRow, ProviderRow};
    let model_row = sqlx::query_as::<_, ModelRow>(
        "SELECT id, provider_id, model_id, display_name, kind, max_tokens, is_default, created_at FROM models WHERE is_default = 1 LIMIT 1"
    )
    .fetch_optional(&db.pool)
    .await?
    .ok_or_else(|| AppError::LlmProvider("未配置默认模型，请在设置中添加 Provider 并设置默认模型".into()))?;

    let provider_row = sqlx::query_as::<_, ProviderRow>(
        "SELECT id, name, kind, base_url, api_key_enc, is_enabled, created_at, updated_at FROM providers WHERE id = ?"
    )
    .bind(&model_row.provider_id)
    .fetch_optional(&db.pool)
    .await?
    .ok_or_else(|| AppError::LlmProvider(format!("Provider not found: {}", model_row.provider_id)))?;

    let base_url = provider_row.base_url.unwrap_or_else(|| {
        match provider_row.kind.as_str() {
            "ollama" => "http://localhost:11434/v1".to_string(),
            _ => "https://api.openai.com/v1".to_string(),
        }
    });
    let api_key = provider_row
        .api_key_enc
        .as_deref()
        .map(crate::commands::settings::decrypt_provider_key)
        .unwrap_or_default();
    let display = model_row.display_name.clone().unwrap_or_else(|| model_row.model_id.clone());
    let provider = crate::core::rig::provider::OpenAiProvider::new(
        model_row.provider_id.clone(),
        display.clone(),
        api_key,
        base_url,
        model_row.model_id.clone(),
    );
    Ok((provider, display))
}

async fn read_if_exists(path: &std::path::Path) -> String {
    tokio::fs::read_to_string(path).await.unwrap_or_default()
}

/// 截断到 max 行
fn truncate_md(s: &str, max_lines: usize) -> String {
    s.lines().take(max_lines).collect::<Vec<_>>().join("\n")
}

/// 宽松提取 JSON 对象（支持 ``` 围栏）
fn extract_json(text: &str) -> Option<serde_json::Value> {
    let cleaned = text.trim();
    let body = if cleaned.starts_with("```") {
        let end = cleaned.rfind("```").unwrap_or(cleaned.len());
        &cleaned[cleaned.find('\n').map(|i| i + 1).unwrap_or(0)..end]
    } else {
        cleaned
    };
    let start = body.find('{')?;
    let stop = body[start..].rfind('}')? + start;
    serde_json::from_str(&body[start..=stop]).ok()
}

/// 路径前缀校验：target 必须在 root 下
fn ensure_within(target: &std::path::Path, root: &std::path::Path) -> Result<(), AppError> {
    if target.starts_with(root) {
        Ok(())
    } else {
        Err(AppError::Validation("路径越界，已拒绝".into()))
    }
}
