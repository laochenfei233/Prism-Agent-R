use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::{Mutex, OnceLock};

use tauri::State;

use crate::data::models::LspServerInfo;
use crate::utils::error::AppError;

// ── 候选定义 ──────────────────────────────────────────────

struct Candidate {
    id: &'static str,
    cmd: &'static str,
    langs: &'static [&'static str],
    manifests: &'static [&'static str],
    install_hint: &'static str,
}

const CANDIDATES: &[Candidate] = &[
    Candidate {
        id: "rust-analyzer",
        cmd: "rust-analyzer",
        langs: &["rust"],
        manifests: &["Cargo.toml"],
        install_hint: "rustup component add rust-analyzer",
    },
    Candidate {
        id: "typescript-language-server",
        cmd: "typescript-language-server",
        langs: &["typescript", "javascript"],
        manifests: &["package.json"],
        install_hint: "npm install -g typescript-language-server",
    },
    Candidate {
        id: "pyright",
        cmd: "pyright-langserver",
        langs: &["python"],
        manifests: &["pyproject.toml", "requirements.txt"],
        install_hint: "npm install -g pyright",
    },
    Candidate {
        id: "gopls",
        cmd: "gopls",
        langs: &["go"],
        manifests: &["go.mod"],
        install_hint: "go install golang.org/x/tools/gopls@latest",
    },
];

fn candidate(id: &str) -> Option<&'static Candidate> {
    CANDIDATES.iter().find(|c| c.id == id)
}

// ── 进程内运行注册表 ──────────────────────────────────────

struct RunningServer {
    child: tokio::process::Child,
    cmd: String,
}

fn registry() -> &'static Mutex<HashMap<String, RunningServer>> {
    static REG: OnceLock<Mutex<HashMap<String, RunningServer>>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(HashMap::new()))
}

// ── 命令 ──────────────────────────────────────────────────

/// 按项目清单文件（Cargo.toml / package.json / pyproject.toml / go.mod）检测 LSP 候选
#[tauri::command]
pub async fn lsp_detect(workdir: String) -> Result<Vec<LspServerInfo>, AppError> {
    let dir = Path::new(&workdir);
    let mut servers = Vec::new();

    for cand in CANDIDATES {
        let detected = cand.manifests.iter().any(|m| dir.join(m).exists());
        if detected {
            servers.push(LspServerInfo {
                id: cand.id.into(),
                cmd: cand.cmd.into(),
                status: "stopped".into(),
                langs: cand.langs.iter().map(|s| s.to_string()).collect(),
                index_file_count: None,
                last_error: None,
                install_hint: Some(cand.install_hint.into()),
            });
        }
    }

    Ok(servers)
}

/// 返回已启动的 LSP 服务器（自动剔除已退出的进程）
#[tauri::command]
pub async fn lsp_list() -> Result<Vec<LspServerInfo>, AppError> {
    let mut guard = registry().lock().unwrap();
    let mut servers = Vec::new();
    let mut dead = Vec::new();

    for (id, server) in guard.iter() {
        if server.child.id().is_some_and(|pid| pid_alive(pid)) {
            servers.push(LspServerInfo {
                id: id.clone(),
                cmd: server.cmd.clone(),
                status: "running".into(),
                langs: candidate(id)
                    .map(|c| c.langs.iter().map(|s| s.to_string()).collect())
                    .unwrap_or_default(),
                index_file_count: None,
                last_error: None,
                install_hint: None,
            });
        } else {
            dead.push(id.clone());
        }
    }

    for id in dead {
        guard.remove(&id);
    }

    Ok(servers)
}

/// 启动 LSP 服务器子进程（stdio，进程 spawn + pid 记录，不发真实诊断）
#[tauri::command]
pub async fn lsp_start(
    _state: State<'_, crate::AppState>,
    server_id: String,
    workdir: String,
) -> Result<LspServerInfo, AppError> {
    let cand = candidate(&server_id)
        .ok_or_else(|| AppError::Validation(format!("未知的 LSP 服务器: {server_id}")))?;

    let dir = Path::new(&workdir);
    if !dir.is_dir() {
        return Err(AppError::Validation(format!("工作目录无效: {workdir}")));
    }

    if !binary_exists(cand.cmd) {
        return Err(AppError::Validation(format!(
            "未找到可执行文件 '{}'，无法启动 {server_id}。请先安装：{}",
            cand.cmd, cand.install_hint
        )));
    }

    let mut child = tokio::process::Command::new(cand.cmd)
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| AppError::Internal(format!("启动 '{}' 失败: {e}", cand.cmd)))?;

    // 短暂健康检查：立即退出的进程视为启动失败
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    if let Ok(Some(status)) = child.try_wait() {
        return Err(AppError::Validation(format!(
            "'{}' 启动后立即退出 (code: {status})",
            cand.cmd
        )));
    }

    registry().lock().unwrap().insert(
        server_id.clone(),
        RunningServer {
            child,
            cmd: cand.cmd.into(),
        },
    );

    Ok(LspServerInfo {
        id: server_id,
        cmd: cand.cmd.into(),
        status: "running".into(),
        langs: cand.langs.iter().map(|s| s.to_string()).collect(),
        index_file_count: None,
        last_error: None,
        install_hint: None,
    })
}

/// 结束 LSP 服务器进程（幂等：未运行也返回成功）
#[tauri::command]
pub async fn lsp_stop(server_id: String) -> Result<(), AppError> {
    let removed = { registry().lock().unwrap().remove(&server_id) };
    if let Some(mut server) = removed {
        let _ = server.child.kill().await;
        let _ = server.child.wait().await;
    }
    Ok(())
}

// ── 辅助 ──────────────────────────────────────────────────

fn binary_exists(cmd: &str) -> bool {
    let probe = if cfg!(windows) { "where.exe" } else { "which" };
    std::process::Command::new(probe)
        .arg(cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn pid_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        std::process::Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}
