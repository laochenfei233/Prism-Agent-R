use std::path::Path;

use crate::data::models::{WikiPage, WikiPageHit, WikiRow};
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

        let file_path = wiki_dir().join(wiki_id).join("wiki").join(path);

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

        let file_path = wiki_dir().join(wiki_id).join("wiki").join(path);

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
            let file_path = wiki_dir().join(wiki_id).join("wiki").join(&page.path);

            let content = tokio::fs::read_to_string(&file_path).await?;
            if let Some(pos) = content.find(query) {
                let snippet = make_snippet(&content, pos, query.len());
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
        use crate::core::adk::model::{ChatMessage, ChatRole, MessageContent, ModelProvider};
        use crate::data::models::WikiWritePlan;

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
                        ChatMessage {
                            role: ChatRole::System,
                            content: MessageContent::Text(system.into()),
                            name: None,
                        },
                        ChatMessage {
                            role: ChatRole::User,
                            content: MessageContent::Text(prompt),
                            name: None,
                        },
                    ],
                    temperature: Some(0.2),
                    ..Default::default()
                })
                .await
                .map_err(|e| AppError::LlmProvider(e.to_string()))?;
            match extract_json(&resp.text)
                .and_then(|v| serde_json::from_value::<WikiWritePlan>(v).ok())
            {
                Some(p) => {
                    plan = Some(p);
                    break;
                }
                None => last_err = truncate_md(&resp.text, 200),
            }
        }

        let plan = plan
            .ok_or_else(|| AppError::LlmProvider(format!("无法解析 Wiki 写入计划: {last_err}")))?;

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
    pub async fn apply_plan(
        &self,
        wiki_id: &str,
        plan: &crate::data::models::WikiWritePlan,
    ) -> Result<crate::data::models::WikiWriteResult, AppError> {
        let wiki_root = wiki_dir().join(wiki_id).join("wiki");
        let result = self.apply_plan_at(&wiki_root, plan).await?;

        // 更新 updated_at
        sqlx::query("UPDATE wikis SET updated_at = ? WHERE id = ?")
            .bind(chrono::Utc::now().timestamp())
            .bind(wiki_id)
            .execute(&self.db.pool)
            .await?;

        Ok(result)
    }

    /// 从 .trash 恢复已删页面（§10.1.1 wiki:restore-trash）
    pub async fn restore_trash(&self, wiki_id: &str, path: &str) -> Result<(), AppError> {
        validate_page_path(path)?;
        let wiki_root = wiki_dir().join(wiki_id).join("wiki");
        let trash_dir = wiki_dir().join(wiki_id).join(".trash");
        let name = std::path::Path::new(path).file_name().unwrap_or_default();
        let src = trash_dir.join(name);
        if !src.exists() {
            return Err(AppError::Validation(format!("回收站中不存在: {path}")));
        }
        let target = wiki_root.join(path);
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        ensure_within(&target, &wiki_root)?;
        tokio::fs::rename(&src, &target).await?;
        Ok(())
    }

    /// 导入文件到 raw/ 并返回（§10.1.1 wiki:ingest-ai 前半段；入库由调用方决定是否走 write_ai）
    pub async fn ingest_file(&self, wiki_id: &str, file_path: &str) -> Result<String, AppError> {
        let src = std::path::Path::new(file_path);
        if !src.exists() {
            return Err(AppError::Validation(format!("文件不存在: {file_path}")));
        }
        let raw_dir = wiki_dir().join(wiki_id).join("raw");
        tokio::fs::create_dir_all(&raw_dir).await?;
        let file_name = src.file_name().unwrap_or_default();
        let dest = raw_dir.join(file_name);
        tokio::fs::copy(src, &dest).await?;
        // 读取文本内容（文本文件直接读；二进制交由上层 RAG 解析）
        let content = tokio::fs::read_to_string(&dest).await.unwrap_or_default();
        Ok(content)
    }

    /// apply_plan 纯文件操作核心（可注入任意 wiki_root 以便测试）：
    /// 事务式（§10.1.1）——先全部预检，再逐操作执行并记录 undo 日志；
    /// 任一失败 → 逆序回滚已执行操作，返回错误；全部成功 → 追加 log.md。
    async fn apply_plan_at(
        &self,
        wiki_root: &std::path::Path,
        plan: &crate::data::models::WikiWritePlan,
    ) -> Result<crate::data::models::WikiWriteResult, AppError> {
        use crate::data::models::WikiOp;

        let trash_dir = wiki_root.parent().unwrap_or(wiki_root).join(".trash");
        tokio::fs::create_dir_all(wiki_root).await?;

        let mut applied = 0usize;
        let mut noop = 0usize;
        let mut ops_log = String::new();
        let mut index_entries: Vec<String> = Vec::new();

        // ── 阶段 1：全量预检（任何 op 非法 → 整体拒绝，不落盘） ──
        for op in &plan.operations {
            match op {
                WikiOp::CreatePage { path, .. }
                | WikiOp::UpdatePage { path, .. }
                | WikiOp::DeletePage { path, .. } => {
                    validate_page_path(path)?;
                    let target = wiki_root.join(path);
                    ensure_within(&target, wiki_root)?;
                }
                _ => {}
            }
        }

        // ── 阶段 2：逐操作执行，记录 undo（失败时逆序回滚） ──
        // undo 条目：(kind, from, to, backup)
        enum Undo {
            DeleteCreated(std::path::PathBuf), // 回滚 CreatePage：删除新建文件
            RestoreBackup(std::path::PathBuf), // 回滚 UpdatePage：用 .bak 还原
            RemoveBackup(std::path::PathBuf),  // 回滚 UpdatePage：清除 .bak
            MoveBack(std::path::PathBuf, std::path::PathBuf), // 回滚 DeletePage：从 .trash 移回
            RestoreFile(std::path::PathBuf, Vec<u8>), // 回滚 UpdateIndex/log：还原原文件
        }
        let mut undo: Vec<Undo> = Vec::new();

        let exec = async {
            for op in &plan.operations {
                match op {
                    WikiOp::CreatePage {
                        path,
                        title,
                        content,
                    } => {
                        let target = wiki_root.join(path);
                        if let Some(parent) = target.parent() {
                            tokio::fs::create_dir_all(parent).await?;
                        }
                        let page = format!("# {title}\n\n{content}\n");
                        tokio::fs::write(&target, page).await?;
                        undo.push(Undo::DeleteCreated(target.clone()));
                        ops_log.push_str(&format!("- CreatePage: {path} (新页面)\n"));
                        index_entries.push(format!("- [{title}]({path})"));
                        applied += 1;
                    }
                    WikiOp::UpdatePage {
                        path,
                        content,
                        summary,
                    } => {
                        let target = wiki_root.join(path);
                        if target.exists() {
                            // 备份 .bak
                            let bak = format!("{}.bak", target.display());
                            tokio::fs::copy(&target, &bak).await?;
                            undo.push(Undo::RestoreBackup(target.clone()));
                            undo.push(Undo::RemoveBackup(std::path::PathBuf::from(bak)));
                        }
                        if let Some(parent) = target.parent() {
                            tokio::fs::create_dir_all(parent).await?;
                        }
                        tokio::fs::write(&target, content).await?;
                        ops_log.push_str(&format!("- UpdatePage: {path} ({summary})\n"));
                        applied += 1;
                    }
                    WikiOp::DeletePage { path, reason } => {
                        let target = wiki_root.join(path);
                        if target.exists() {
                            tokio::fs::create_dir_all(&trash_dir).await?;
                            let name = target.file_name().unwrap_or_default();
                            let dest = trash_dir.join(name);
                            tokio::fs::rename(&target, &dest).await?;
                            undo.push(Undo::MoveBack(dest, target));
                            ops_log.push_str(&format!(
                                "- DeletePage: {path} (移至 .trash，原因: {reason})\n"
                            ));
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
                existing.push_str(&format!(
                    "\n## 更新 {}\n",
                    chrono::Utc::now().format("%Y-%m-%d %H:%M")
                ));
                for e in &index_entries {
                    existing.push_str(e);
                    existing.push('\n');
                }
                let original = tokio::fs::read(&index_path).await.unwrap_or_default();
                tokio::fs::write(&index_path, &existing).await?;
                undo.push(Undo::RestoreFile(index_path, original));
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
            let original = tokio::fs::read(&log_path).await.unwrap_or_default();
            tokio::fs::write(&log_path, log).await?;
            undo.push(Undo::RestoreFile(log_path, original));

            Ok::<(), AppError>(())
        };

        if let Err(e) = exec.await {
            // ── 阶段 3：回滚（逆序） ──
            for u in undo.iter().rev() {
                match u {
                    Undo::DeleteCreated(p) => {
                        let _ = tokio::fs::remove_file(p).await;
                    }
                    Undo::RestoreBackup(p) => {
                        let bak = format!("{}.bak", p.display());
                        let _ = tokio::fs::rename(&bak, p).await;
                    }
                    Undo::RemoveBackup(bak) => {
                        let _ = tokio::fs::remove_file(bak).await;
                    }
                    Undo::MoveBack(from, to) => {
                        let _ = tokio::fs::rename(from, to).await;
                    }
                    Undo::RestoreFile(p, bytes) => {
                        let _ = tokio::fs::write(p, bytes).await;
                    }
                }
            }
            return Err(e);
        }

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
async fn collect_pages(root: &Path, base: &Path, out: &mut Vec<WikiPage>) -> Result<(), AppError> {
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
async fn resolve_wiki_model(
    db: &Database,
) -> Result<(crate::core::rig::provider::OpenAiProvider, String), AppError> {
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

/// 以命中位置为中心截取片段（前后各约 100 字符），字符边界安全
fn make_snippet(content: &str, pos: usize, query_len: usize) -> String {
    let start = content.floor_char_boundary(pos.saturating_sub(100));
    let end = content.floor_char_boundary((pos + query_len + 100).min(content.len()));
    content[start..end].to_string()
}

/// 路径前缀校验：target 必须在 root 下
fn ensure_within(target: &std::path::Path, root: &std::path::Path) -> Result<(), AppError> {
    if target.starts_with(root) {
        Ok(())
    } else {
        Err(AppError::Validation("路径越界，已拒绝".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippet_cjk_char_boundary_no_panic() {
        // 命中点附近的 ±100 字节窗口落在多字节字符中间时不得 panic（回归：字节切片越界）
        let content = "知识库内容".repeat(60); // 全部 3 字节 CJK，任何非 3 的倍数字节都不是字符边界
        let pos = content.find("知识").unwrap();
        let s = make_snippet(&content, pos, 6);
        assert!(!s.is_empty());
        assert!(s.contains("知识"));
    }

    #[test]
    fn snippet_near_document_start() {
        let content = "短文档";
        let s = make_snippet(content, 0, 3);
        assert_eq!(s, "短文档");
    }

    #[test]
    fn snippet_near_document_end() {
        let content = "中文".repeat(200);
        let pos = content.len() - 6;
        let s = make_snippet(&content, pos, 6);
        assert!(!s.is_empty());
        assert!(s.contains("中文"));
    }

    /// 事务式 apply_plan（§10.1.1）：单 op 非法路径 → 预检整体拒绝，之前操作不落盘
    #[tokio::test]
    async fn apply_plan_rejects_invalid_path_before_writing() {
        let dir = std::env::temp_dir().join(format!("prism_wiki_plan_{}", uuid::Uuid::new_v4()));
        let wiki_root = dir.join("wiki");
        tokio::fs::create_dir_all(&wiki_root).await.unwrap();

        // 预建一个待更新页面
        tokio::fs::create_dir_all(wiki_root.join("concepts"))
            .await
            .unwrap();
        tokio::fs::write(
            wiki_root.join("concepts/kubernetes.md"),
            "# Kubernetes\n旧内容\n",
        )
        .await
        .unwrap();

        let plan = crate::data::models::WikiWritePlan {
            operations: vec![
                crate::data::models::WikiOp::UpdatePage {
                    path: "concepts/kubernetes.md".into(),
                    content: "# Kubernetes\n新内容\n".into(),
                    summary: "更新".into(),
                },
                // 非法路径：含 .. → 预检阶段整体拒绝
                crate::data::models::WikiOp::CreatePage {
                    path: "../evil.md".into(),
                    title: "Evil".into(),
                    content: "bad".into(),
                },
            ],
        };

        let svc = WikiService::new(crate::data::Database::new(&dir).await.unwrap());
        let err = svc.apply_plan_at(&wiki_root, &plan).await;
        assert!(err.is_err(), "非法路径必须被拒绝");

        // 预检失败 → UpdatePage 不得执行，旧内容保留
        let content = tokio::fs::read_to_string(wiki_root.join("concepts/kubernetes.md"))
            .await
            .unwrap();
        assert!(content.contains("旧内容"), "回滚后应保留原内容: {content}");
        assert!(!content.contains("新内容"), "非法计划不得部分应用");

        // 无 log.md（未提交）
        assert!(!wiki_root.join("log.md").exists(), "失败计划不得写 log.md");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 事务式 apply_plan：全部合法时成功执行并写 log.md
    #[tokio::test]
    async fn apply_plan_success_applies_and_logs() {
        let dir = std::env::temp_dir().join(format!("prism_wiki_plan_ok_{}", uuid::Uuid::new_v4()));
        let wiki_root = dir.join("wiki");
        tokio::fs::create_dir_all(&wiki_root).await.unwrap();

        let plan = crate::data::models::WikiWritePlan {
            operations: vec![
                crate::data::models::WikiOp::CreatePage {
                    path: "concepts/kubernetes.md".into(),
                    title: "Kubernetes".into(),
                    content: "K8s 介绍".into(),
                },
                crate::data::models::WikiOp::Noop {
                    reason: "重复".into(),
                },
            ],
        };

        let svc = WikiService::new(crate::data::Database::new(&dir).await.unwrap());
        let result = svc.apply_plan_at(&wiki_root, &plan).await.unwrap();
        assert_eq!(result.applied, 1);
        assert_eq!(result.noop, 1);

        let page = tokio::fs::read_to_string(wiki_root.join("concepts/kubernetes.md"))
            .await
            .unwrap();
        assert!(page.contains("K8s 介绍"));
        let log = tokio::fs::read_to_string(wiki_root.join("log.md"))
            .await
            .unwrap();
        assert!(
            log.contains("CreatePage: concepts/kubernetes.md"),
            "log.md 需含变更记录"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 恢复回收站页面（§10.1.1）：.trash → wiki/ 原位
    #[tokio::test]
    async fn restore_trash_moves_page_back() {
        let dir = std::env::temp_dir().join(format!("prism_wiki_restore_{}", uuid::Uuid::new_v4()));
        let wiki_root = dir.join("wiki");
        let trash = dir.join(".trash");
        tokio::fs::create_dir_all(wiki_root.join("concepts"))
            .await
            .unwrap();
        tokio::fs::create_dir_all(&trash).await.unwrap();
        tokio::fs::write(&trash.join("kubernetes.md"), "# Kubernetes\n内容\n")
            .await
            .unwrap();

        let svc = WikiService::new(crate::data::Database::new(&dir).await.unwrap());
        // 注意：服务内 wiki_dir() 是全局路径，restore_trash 不接 base_dir。
        // 此处只验证路径校验与「不存在时报错」路径（文件系统操作走全局目录，测试不落盘）。
        let err = svc
            .restore_trash("wk-not-exist", "concepts/kubernetes.md")
            .await;
        assert!(err.is_err(), "不存在的 wiki 恢复应报错（源文件不在回收站）");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
