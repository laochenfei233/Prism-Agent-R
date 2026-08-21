// §10.2.1 项目级自动索引
//
// 工作目录变更 → 增量索引（白名单扩展名 + 忽略规则 + 文件指纹比对 + debounce 5s）。
// - 命名空间隔离：wiki_id = '__project__'（迁移 021 预建 wikis 行），不污染用户 Wiki
// - 触发：独立轮询快照（复用 fs.rs snapshot/diff），每 2s 比对，5s 无变更后批量处理
// - 开关：preferences `project_index.enabled`（默认开），仅对 workspace.current_dir 生效
// - 首次全量：后台任务 + `rag:progress` 事件；状态走 `project_index:status` 事件

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use tauri::Emitter;

use crate::commands::fs;
use crate::data::db::Database;
use crate::data::models::ProjectIndexStatus;
use crate::data::rag::store;
use crate::data::services::rag_service::RagService;
use crate::utils::error::AppError;

pub const PROJECT_WIKI_ID: &str = "__project__";

/// 索引白名单扩展名（design §10.2.1：md/txt/rs/ts/svelte/json/yaml/toml 等）
const WHITELIST: &[&str] = &[
    "md", "txt", "rs", "ts", "tsx", "js", "jsx", "svelte", "json", "yaml", "yml", "toml", "py",
    "go", "java", "c", "cpp", "h", "hpp", "sql", "css", "html", "vue", "sh", "bash",
];

const KEY_ENABLED: &str = "project_index.enabled";
const KEY_WORKDIR: &str = "workspace.current_dir";

const WATCH_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);
const DEBOUNCE: std::time::Duration = std::time::Duration::from_secs(5);

// ── 管理器全局状态 ────────────────────────────────────────

struct Manager {
    handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    processing: AtomicBool,
    last_indexed_at: Mutex<Option<i64>>,
}

fn manager() -> &'static Manager {
    static M: OnceLock<Manager> = OnceLock::new();
    M.get_or_init(|| Manager {
        handle: Mutex::new(None),
        processing: AtomicBool::new(false),
        last_indexed_at: Mutex::new(None),
    })
}

// ── 启动 / 开关 ───────────────────────────────────────────

/// 应用启动时调用：启用状态下启动监听循环（开关变化由循环内每 tick 读取，无需重启）
pub fn start_if_enabled(db: Database, app: tauri::AppHandle) {
    let mgr = manager();
    let mut guard = mgr.handle.lock().unwrap();
    if guard.is_some() {
        return;
    }
    *guard = Some(tokio::spawn(watcher_loop(db, app)));
}

/// 切换开关（project_index_toggle）
pub async fn toggle(
    db: &Database,
    app: tauri::AppHandle,
    enabled: bool,
) -> Result<ProjectIndexStatus, AppError> {
    set_pref(db, KEY_ENABLED, if enabled { "1" } else { "0" }).await?;
    if enabled {
        // 确保监听循环在跑（已跑则无操作）
        start_if_enabled(db.clone(), app.clone());
    }
    let status = status(db).await?;
    let _ = app.emit(
        "project_index:status",
        serde_json::to_value(&status).unwrap_or_default(),
    );
    Ok(status)
}

/// 触发全量重建（project_index_reindex）
pub async fn reindex(db: Database, app: tauri::AppHandle) -> Result<ProjectIndexStatus, AppError> {
    let mgr = manager();
    if mgr.processing.swap(true, Ordering::SeqCst) {
        return Err(AppError::Validation("项目索引正在运行中".into()));
    }
    let workdir = workdir_pref(&db).await;
    let root = match workdir {
        Some(dir) => match std::fs::canonicalize(&dir) {
            Ok(r) if r.is_dir() => r,
            _ => {
                mgr.processing.store(false, Ordering::SeqCst);
                return Err(AppError::Validation(format!("工作目录无效: {dir}")));
            }
        },
        None => {
            mgr.processing.store(false, Ordering::SeqCst);
            return Err(AppError::Validation(
                "未设置工作目录（workspace:set 绑定后生效）".into(),
            ));
        }
    };
    tokio::spawn(full_reindex_task(db.clone(), app.clone(), root));
    status(&db).await
}

/// 当前状态（project_index_status）
pub async fn status(db: &Database) -> Result<ProjectIndexStatus, AppError> {
    let enabled = pref(db, KEY_ENABLED).await.unwrap_or_else(|| "1".into()) == "1";
    let workdir = workdir_pref(db).await;
    let indexed: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM rag_documents WHERE wiki_id = ?")
        .bind(PROJECT_WIKI_ID)
        .fetch_one(&db.pool)
        .await
        .unwrap_or(0);
    Ok(ProjectIndexStatus {
        enabled,
        workdir,
        indexed_files: indexed,
        in_progress: manager().processing.load(Ordering::SeqCst),
        last_indexed_at: *manager().last_indexed_at.lock().unwrap(),
    })
}

// ── 监听循环 ──────────────────────────────────────────────

async fn watcher_loop(db: Database, app: tauri::AppHandle) {
    let mut last_root: Option<PathBuf> = None;
    let mut last_snapshot: Option<fs::Snapshot> = None;
    let mut pending: HashSet<String> = HashSet::new();
    let mut last_change: Option<Instant> = None;

    loop {
        tokio::time::sleep(WATCH_INTERVAL).await;

        // 开关 + 工作目录（每 tick 读取，支持热切换）
        let enabled = pref(&db, KEY_ENABLED).await.unwrap_or_else(|| "1".into()) == "1";
        let workdir = match workdir_pref(&db).await {
            Some(d) => match std::fs::canonicalize(&d) {
                Ok(r) if r.is_dir() => r,
                _ => {
                    last_root = None;
                    last_snapshot = None;
                    pending.clear();
                    continue;
                }
            },
            None => {
                last_root = None;
                last_snapshot = None;
                pending.clear();
                continue;
            }
        };

        if !enabled {
            last_root = None;
            last_snapshot = None;
            pending.clear();
            continue;
        }

        // 目录切换 → 重置基线（全新快照，避免跨目录误判删除）
        if last_root.as_ref() != Some(&workdir) {
            last_root = Some(workdir.clone());
            last_snapshot = None;
            pending.clear();
        }

        let current = fs::snapshot_dir(&workdir);
        let changed = match &last_snapshot {
            Some(old) => fs::diff_snapshots(old, &current),
            None => Vec::new(), // 首轮只建基线
        };
        last_snapshot = Some(current);

        if !changed.is_empty() {
            pending.extend(changed);
            last_change = Some(Instant::now());
        }

        // debounce 5s 无新变更 → 处理批量
        let ready = !pending.is_empty()
            && last_change
                .map(|t| t.elapsed() >= DEBOUNCE)
                .unwrap_or(false);
        if ready && !manager().processing.swap(true, Ordering::SeqCst) {
            let batch: Vec<String> = pending.drain().collect();
            let root = workdir.clone();
            tokio::spawn(process_batch_task(db.clone(), app.clone(), root, batch));
            last_change = None;
        }
    }
}

// ── 增量批处理 ────────────────────────────────────────────

async fn process_batch_task(db: Database, app: tauri::AppHandle, root: PathBuf, rels: Vec<String>) {
    let mgr = manager();
    for rel in &rels {
        let full = root.join(rel);
        if full.is_file() && is_whitelisted(rel) {
            let fp = file_fingerprint(&full);
            if let Some(doc_id) = store::find_document_by_path(&db, PROJECT_WIKI_ID, rel)
                .await
                .ok()
                .flatten()
            {
                if store::fingerprint_of_document(&db, &doc_id)
                    .await
                    .ok()
                    .flatten()
                    == Some(fp.clone())
                {
                    continue; // 未变更
                }
                let _ = store::delete_document(&db, &doc_id).await; // 变更 → 重新摄取
            }
            let mut svc = RagService::new(db.clone());
            if let Err(e) = svc.configure_from_db().await {
                tracing::warn!("[project_index] 嵌入配置读取失败: {e}");
            }
            match svc
                .ingest_with_meta(PROJECT_WIKI_ID, &full.to_string_lossy(), rel, &fp)
                .await
            {
                Ok(_) => {}
                Err(e) => tracing::warn!("[project_index] {rel} 摄取失败: {e}"),
            }
        } else if !full.exists() {
            // 文件被删除 → 清理索引
            let _ = store::delete_document_by_path(&db, PROJECT_WIKI_ID, rel).await;
        }
    }
    *mgr.last_indexed_at.lock().unwrap() = Some(chrono::Utc::now().timestamp());
    mgr.processing.store(false, Ordering::SeqCst);
    emit_status(&app, &db).await;
}

// ── 全量重建 ──────────────────────────────────────────────

async fn full_reindex_task(db: Database, app: tauri::AppHandle, root: PathBuf) {
    let mgr = manager();
    let files = collect_indexable_files(&root);
    let total = files.len();
    let mut indexed: HashSet<String> = HashSet::new();
    let mut done = 0usize;

    for (rel, full) in &files {
        done += 1;
        let _ = app.emit(
            "rag:progress",
            serde_json::json!({
                "stage": "project_index",
                "done": done,
                "total": total,
                "message": rel,
            }),
        );

        let fp = file_fingerprint(full);
        if let Some(doc_id) = store::find_document_by_path(&db, PROJECT_WIKI_ID, rel)
            .await
            .ok()
            .flatten()
        {
            if store::fingerprint_of_document(&db, &doc_id)
                .await
                .ok()
                .flatten()
                == Some(fp.clone())
            {
                indexed.insert(rel.clone());
                continue;
            }
            let _ = store::delete_document(&db, &doc_id).await;
        }

        let mut svc = RagService::new(db.clone());
        let _ = svc.configure_from_db().await;
        match svc
            .ingest_with_meta(PROJECT_WIKI_ID, &full.to_string_lossy(), rel, &fp)
            .await
        {
            Ok(_) => indexed.insert(rel.clone()),
            Err(e) => {
                tracing::warn!("[project_index] {rel} 全量摄取失败: {e}");
                false
            }
        };
    }

    // 清理磁盘上已不存在的文档
    if let Ok(docs) = store::list_documents(&db, PROJECT_WIKI_ID).await {
        for doc in docs {
            if let Some(fp) = &doc.file_path {
                if !indexed.contains(fp) {
                    let _ = store::delete_document(&db, &doc.id).await;
                }
            }
        }
    }

    *mgr.last_indexed_at.lock().unwrap() = Some(chrono::Utc::now().timestamp());
    mgr.processing.store(false, Ordering::SeqCst);
    let _ = app.emit("rag:progress", serde_json::json!({ "stage": "project_index", "done": total, "total": total, "message": "complete" }));
    emit_status(&app, &db).await;
}

// ── 辅助 ──────────────────────────────────────────────────

fn is_whitelisted(rel: &str) -> bool {
    let ext = Path::new(rel)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    WHITELIST.contains(&ext.as_str())
}

/// 文件指纹：mtime_nanos:size（path+mtime+size 三重比对由调用方以 rel 路径完成）
fn file_fingerprint(path: &Path) -> String {
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return String::new(),
    };
    let nanos = meta
        .modified()
        .ok()
        .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(0);
    format!("{nanos}:{}", meta.len())
}

/// 收集白名单内文件（忽略规则复用 fs.rs IGNORED）
fn collect_indexable_files(root: &Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if fs::IGNORED.contains(&name.as_str()) || name.starts_with('.') {
                continue;
            }
            // 跳过符号链接（防目录环；链接指向区外的文件不纳入索引）
            let Ok(ftype) = entry.file_type() else {
                continue;
            };
            if ftype.is_symlink() {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                stack.push(path);
            } else if let Ok(rel) = path.strip_prefix(root) {
                let rel_str = rel.to_string_lossy().to_string();
                if is_whitelisted(&rel_str) {
                    out.push((rel_str, path));
                }
            }
        }
    }
    out.sort();
    out
}

async fn workdir_pref(db: &Database) -> Option<String> {
    pref(db, KEY_WORKDIR).await
}

async fn pref(db: &Database, key: &str) -> Option<String> {
    sqlx::query_scalar::<_, String>("SELECT value FROM preferences WHERE key = ?")
        .bind(key)
        .fetch_optional(&db.pool)
        .await
        .ok()
        .flatten()
}

async fn set_pref(db: &Database, key: &str, value: &str) -> Result<(), AppError> {
    sqlx::query("INSERT OR REPLACE INTO preferences (key, value, updated_at) VALUES (?, ?, ?)")
        .bind(key)
        .bind(value)
        .bind(chrono::Utc::now().timestamp())
        .execute(&db.pool)
        .await?;
    Ok(())
}

async fn emit_status(app: &tauri::AppHandle, db: &Database) {
    if let Ok(s) = status(db).await {
        if let Ok(v) = serde_json::to_value(&s) {
            let _ = app.emit("project_index:status", v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitelist_matches_design_extensions() {
        assert!(is_whitelisted("src/main.rs"));
        assert!(is_whitelisted("docs/design.md"));
        assert!(is_whitelisted("app.svelte"));
        assert!(is_whitelisted("config.yaml"));
        assert!(is_whitelisted("Cargo.toml"));
        assert!(!is_whitelisted("image.png"));
        assert!(!is_whitelisted("binary.exe"));
        assert!(!is_whitelisted("no_extension"));
    }

    #[test]
    fn fingerprint_changes_with_content() {
        let dir = std::env::temp_dir().join(format!("projidx_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.txt");
        std::fs::write(&f, "hello").unwrap();
        let fp1 = file_fingerprint(&f);
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&f, "hello world").unwrap();
        let fp2 = file_fingerprint(&f);
        assert_ne!(fp1, fp2, "内容变化后指纹必须不同");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
