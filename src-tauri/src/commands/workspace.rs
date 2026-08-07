use std::io::BufRead;
use std::path::{Path, PathBuf};

use sqlx::SqlitePool;
use tauri::{Emitter, State};

use crate::data::models::{DirTree, WorkspaceInfo};
use crate::utils::error::AppError;

// ── Preferences 键 ────────────────────────────────────────

const KEY_CURRENT_DIR: &str = "workspace.current_dir";
const KEY_RECENT_DIRS: &str = "workspace.recent_dirs";
const KEY_BOUND_AGENT: &str = "workspace.bound_agent_id";

const TREE_IGNORED: &[&str] = &[
    ".git", "node_modules", "target", "dist", "build", "__pycache__", ".venv", "vendor", ".svn",
];

// ── 命令 ──────────────────────────────────────────────────

/// 获取当前工作区信息（preferences 无记录时回退到进程当前目录）
#[tauri::command]
pub async fn workspace_get(state: State<'_, crate::AppState>) -> Result<WorkspaceInfo, AppError> {
    load_workspace_info(&state.db.pool).await
}

/// 设置工作区：更新 current_dir、加入最近目录列表（最多 5 个）、可选写入 agent 绑定，
/// 并 emit `workspace:changed` 事件。
#[tauri::command]
pub async fn workspace_set(
    app: tauri::AppHandle,
    state: State<'_, crate::AppState>,
    path: String,
    agent_id: Option<String>,
) -> Result<WorkspaceInfo, AppError> {
    let canon = std::fs::canonicalize(&path)
        .map_err(|e| AppError::Validation(format!("目录无效 '{path}': {e}")))?;
    if !canon.is_dir() {
        return Err(AppError::Validation(format!("'{path}' 不是目录")));
    }
    let dir = canon.display().to_string();
    let now = chrono::Utc::now().timestamp_millis();

    set_pref(&state.db.pool, KEY_CURRENT_DIR, &dir, now).await?;

    let mut recent: Vec<String> = get_pref(&state.db.pool, KEY_RECENT_DIRS)
        .await
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    recent.retain(|d| d != &dir);
    recent.insert(0, dir.clone());
    recent.truncate(5);
    set_pref(&state.db.pool, KEY_RECENT_DIRS, &serde_json::to_string(&recent)?, now).await?;

    if let Some(agent) = agent_id {
        set_pref(&state.db.pool, KEY_BOUND_AGENT, &agent, now).await?;
    }

    let info = WorkspaceInfo {
        current_dir: dir,
        recent_dirs: recent,
        bound_agent_id: get_pref(&state.db.pool, KEY_BOUND_AGENT).await,
    };

    let _ = app.emit("workspace:changed", info.clone());
    Ok(info)
}

/// 递归读取目录树（忽略常见构建/依赖目录，文件标注 language 与 line_count）
#[tauri::command]
pub async fn workspace_tree(path: String, depth: Option<u8>) -> Result<DirTree, AppError> {
    let depth = depth.unwrap_or(2).clamp(1, 8);
    build_tree(&PathBuf::from(&path), depth)
}

/// 读取文件内容；超过 200KB 只返回前 100 行并标记截断
#[tauri::command]
pub async fn workspace_read_file(
    _state: State<'_, crate::AppState>,
    path: String,
) -> Result<String, AppError> {
    let meta = tokio::fs::metadata(&path).await?;
    if !meta.is_file() {
        return Err(AppError::Validation(format!("'{path}' 不是文件")));
    }

    const LIMIT: u64 = 200 * 1024;
    if meta.len() <= LIMIT {
        return Ok(tokio::fs::read_to_string(&path).await?);
    }

    // 大文件：只读前 100 行，避免整文件载入内存
    let file = std::fs::File::open(&path)?;
    let reader = std::io::BufReader::new(file);
    let mut out = String::new();
    for line in reader.lines().take(100) {
        let line = line?;
        out.push_str(&line);
        out.push('\n');
    }
    out.push_str("\n[内容过大，已截断：文件超过 200KB，仅显示前 100 行]\n");
    Ok(out)
}

/// 用系统默认方式在外部打开文件（Windows: explorer / macOS: open / Linux: xdg-open）
#[tauri::command]
pub async fn workspace_open_file(
    _state: State<'_, crate::AppState>,
    path: String,
    line: Option<u32>,
) -> Result<(), AppError> {
    if !Path::new(&path).exists() {
        return Err(AppError::Validation(format!("文件不存在: {path}")));
    }

    #[cfg(target_os = "windows")]
    {
        let _ = line;
        std::process::Command::new("explorer").arg(&path).spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        let _ = line;
        std::process::Command::new("open").arg(&path).spawn()?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = line;
        std::process::Command::new("xdg-open").arg(&path).spawn()?;
    }
    Ok(())
}

/// 写指令文件（仅允许写入当前工作区目录内的文件）
#[tauri::command]
pub async fn workspace_write_instructions(
    state: State<'_, crate::AppState>,
    path: String,
    content: String,
) -> Result<(), AppError> {
    let workspace = load_workspace_info(&state.db.pool).await?;
    let root = std::fs::canonicalize(&workspace.current_dir)?;

    let target = if Path::new(&path).is_absolute() {
        PathBuf::from(&path)
    } else {
        root.join(&path)
    };

    let parent = target.parent().unwrap_or(Path::new("."));
    let canon_parent = std::fs::canonicalize(parent)
        .map_err(|_| AppError::Forbidden(format!("目标路径无效: {path}")))?;
    if !canon_parent.starts_with(&root) {
        return Err(AppError::Forbidden(format!(
            "只能写入工作区目录内的文件: {}",
            target.display()
        )));
    }

    if let Some(parent_dir) = target.parent() {
        tokio::fs::create_dir_all(parent_dir).await?;
    }
    tokio::fs::write(&target, content).await?;
    Ok(())
}

// ── 共享辅助（agent::context_agent 复用） ────────────────

/// 读取 preferences 中的工作区记录；无 current_dir 记录时返回 None
pub(crate) async fn load_workspace_pref(pool: &SqlitePool) -> Option<WorkspaceInfo> {
    let current_dir = get_pref(pool, KEY_CURRENT_DIR).await?;
    let recent_dirs: Vec<String> = get_pref(pool, KEY_RECENT_DIRS)
        .await
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let bound_agent_id = get_pref(pool, KEY_BOUND_AGENT).await;
    Some(WorkspaceInfo {
        current_dir,
        recent_dirs,
        bound_agent_id,
    })
}

/// 完整加载工作区信息；无记录时回退到进程当前目录
pub(crate) async fn load_workspace_info(pool: &SqlitePool) -> Result<WorkspaceInfo, AppError> {
    if let Some(info) = load_workspace_pref(pool).await {
        return Ok(info);
    }
    Ok(WorkspaceInfo {
        current_dir: default_current_dir(),
        recent_dirs: Vec::new(),
        bound_agent_id: None,
    })
}

fn default_current_dir() -> String {
    std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| ".".into())
}

async fn get_pref(pool: &SqlitePool, key: &str) -> Option<String> {
    sqlx::query_scalar::<_, String>("SELECT value FROM preferences WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
}

async fn set_pref(pool: &SqlitePool, key: &str, value: &str, now: i64) -> Result<(), AppError> {
    sqlx::query("INSERT OR REPLACE INTO preferences (key, value, updated_at) VALUES (?, ?, ?)")
        .bind(key)
        .bind(value)
        .bind(now)
        .execute(pool)
        .await?;
    Ok(())
}

// ── 目录树 ────────────────────────────────────────────────

fn build_tree(dir: &Path, depth: u8) -> Result<DirTree, AppError> {
    if !dir.is_dir() {
        return Err(AppError::Validation(format!("'{}' 不是目录", dir.display())));
    }

    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".into());

    if depth == 0 {
        return Ok(DirTree {
            name,
            path: dir.display().to_string(),
            is_dir: true,
            children: None,
            language: None,
            line_count: None,
        });
    }

    let entries = std::fs::read_dir(dir)
        .map_err(|e| AppError::Internal(format!("读取目录失败: {e}")))?;

    let mut children: Vec<DirTree> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            !TREE_IGNORED.contains(&name.as_str()) && !name.starts_with('.')
        })
        .filter_map(|entry| {
            let path = entry.path();
            let meta = entry.metadata().ok()?;
            let is_dir = meta.is_dir();
            let child_name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            let (language, line_count) = if is_dir {
                (None, None)
            } else {
                let lang = detect_language(&child_name);
                let lines = std::fs::read_to_string(&path)
                    .ok()
                    .map(|c| c.lines().count() as u64);
                (lang, lines)
            };

            if is_dir {
                build_tree(&path, depth - 1).ok()
            } else {
                Some(DirTree {
                    name: child_name,
                    path: path.display().to_string(),
                    is_dir: false,
                    children: None,
                    language,
                    line_count,
                })
            }
        })
        .collect();

    children.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));

    Ok(DirTree {
        name,
        path: dir.display().to_string(),
        is_dir: true,
        children: Some(children),
        language: None,
        line_count: None,
    })
}

fn detect_language(filename: &str) -> Option<String> {
    let ext = Path::new(filename).extension()?.to_str()?;
    match ext {
        "rs" => Some("rust".into()),
        "ts" | "tsx" => Some("typescript".into()),
        "js" | "jsx" => Some("javascript".into()),
        "py" => Some("python".into()),
        "go" => Some("go".into()),
        "java" => Some("java".into()),
        "cpp" | "cc" | "cxx" | "h" | "hpp" => Some("c++".into()),
        "c" => Some("c".into()),
        "rb" => Some("ruby".into()),
        "swift" => Some("swift".into()),
        "kt" | "kts" => Some("kotlin".into()),
        "vue" => Some("vue".into()),
        "svelte" => Some("svelte".into()),
        "css" | "scss" | "less" => Some("css".into()),
        "html" | "htm" => Some("html".into()),
        "json" => Some("json".into()),
        "yaml" | "yml" => Some("yaml".into()),
        "toml" => Some("toml".into()),
        "md" => Some("markdown".into()),
        "sql" => Some("sql".into()),
        "sh" | "bash" => Some("shell".into()),
        _ => None,
    }
}
