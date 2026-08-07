use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use tauri::{Emitter, State};
use tokio::time::{interval, Duration};

use crate::utils::error::AppError;

// ── 轮询监听 ──────────────────────────────────────────────
// 不引入 notify crate：记录目录文件快照（mtime + size），
// 后台任务每 2 秒比对一次，变化时 emit `fs:watcher` 事件。

const WATCH_INTERVAL_SECS: u64 = 2;

pub(crate) type Snapshot = HashMap<String, (i64, u64)>;

struct Watcher {
    handle: tokio::task::JoinHandle<()>,
}

fn watcher_registry() -> &'static Mutex<Option<Watcher>> {
    static W: OnceLock<Mutex<Option<Watcher>>> = OnceLock::new();
    W.get_or_init(|| Mutex::new(None))
}

/// 启用/禁用工作目录变更监听
#[tauri::command]
pub async fn fs_watch(
    app: tauri::AppHandle,
    _state: State<'_, crate::AppState>,
    workdir: String,
    enable: bool,
) -> Result<(), AppError> {
    let mut guard = watcher_registry().lock().unwrap();

    // 无论启用还是切换目录，先停掉旧的监听任务
    if let Some(w) = guard.take() {
        w.handle.abort();
    }

    if !enable {
        return Ok(());
    }

    let root = std::fs::canonicalize(&workdir)
        .map_err(|e| AppError::Validation(format!("目录无效 '{workdir}': {e}")))?;
    if !root.is_dir() {
        return Err(AppError::Validation(format!("'{workdir}' 不是目录")));
    }

    let app = app.clone();
    let handle = tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(WATCH_INTERVAL_SECS));
        let mut last = snapshot_dir(&root);
        loop {
            ticker.tick().await;
            let current = snapshot_dir(&root);
            let changed = diff_snapshots(&last, &current);
            if !changed.is_empty() {
                let _ = app.emit("fs:watcher", serde_json::json!({ "changed_paths": changed }));
                last = current;
            }
        }
    });

    *guard = Some(Watcher { handle });
    Ok(())
}

// ── 快照与差异 ────────────────────────────────────────────
// pub(crate)：§10.2.1 项目级自动索引复用（project_index.rs）

pub(crate) const IGNORED: &[&str] = &[
    ".git", "node_modules", "target", "dist", "build", "__pycache__", ".venv", "vendor", ".svn",
];

pub(crate) fn snapshot_dir(root: &Path) -> Snapshot {
    let mut map = Snapshot::new();
    walk_snapshot(root, root, &mut map);
    map
}

fn walk_snapshot(root: &Path, dir: &Path, map: &mut Snapshot) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if IGNORED.contains(&name.as_str()) || name.starts_with('.') {
            continue;
        }
        // 跳过符号链接（防目录环无限递归；链接指向区外的文件也不纳入快照）
        let Ok(ftype) = entry.file_type() else { continue };
        if ftype.is_symlink() {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();

        if meta.is_dir() {
            walk_snapshot(root, &path, map);
        } else if let Ok(modified) = meta.modified() {
            let nanos = modified
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as i64)
                .unwrap_or(0);
            map.insert(rel, (nanos, meta.len()));
        }
    }
}

pub(crate) fn diff_snapshots(old: &Snapshot, new: &Snapshot) -> Vec<String> {
    let mut changed: Vec<String> = new
        .iter()
        .filter(|(p, sig)| old.get(p.as_str()) != Some(sig))
        .map(|(p, _)| p.clone())
        .collect();

    for p in old.keys() {
        if !new.contains_key(p) {
            changed.push(p.clone());
        }
    }

    changed
}
