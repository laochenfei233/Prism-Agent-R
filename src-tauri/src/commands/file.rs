use std::io::BufRead;
use std::path::Path;

use crate::data::models::{FileEntry, ParsedFile};
use crate::utils::error::AppError;

// ── 文件/附件域命令 ────────────────────────────────────────

/// 打开系统文件选择对话框（需 tauri-plugin-dialog，当前 Cargo.toml 未启用）。
/// 变通：传入 `path` 直接指定文件路径并校验；未传则返回对话框不可用提示。
#[tauri::command]
pub async fn file_pick(path: Option<String>) -> Result<String, AppError> {
    let Some(p) = path else {
        return Err(AppError::Validation(
            "文件选择需系统对话框支持（未安装 tauri-plugin-dialog），请直接传入 path 参数".into(),
        ));
    };
    if !Path::new(&p).exists() {
        return Err(AppError::Validation(format!("文件不存在: {p}")));
    }
    Ok(p)
}

/// 读取文本文件内容；超过 200KB 只返回前 100 行并标记截断（同 workspace_read_file 风格）
#[tauri::command]
pub async fn file_read_text(path: String) -> Result<String, AppError> {
    read_text_limited(&path).await
}

/// 写入文本文件（自动创建父目录），返回写入路径
#[tauri::command]
pub async fn file_write(path: String, content: String) -> Result<String, AppError> {
    if let Some(parent) = Path::new(&path).parent() {
        if !parent.as_os_str().is_empty() {
            tokio::fs::create_dir_all(parent).await?;
        }
    }
    tokio::fs::write(&path, content).await?;
    Ok(path)
}

/// 列出目录内容：depth 控制递归深度（1 = 仅直接子项，最多 8 层），目录优先排序
#[tauri::command]
pub async fn file_list(path: String, depth: Option<u8>) -> Result<Vec<FileEntry>, AppError> {
    let dir = Path::new(&path);
    if !dir.is_dir() {
        return Err(AppError::Validation(format!("'{path}' 不是目录")));
    }
    list_entries(dir, depth.unwrap_or(1).clamp(1, 8))
}

/// 按扩展名解析文件：md/txt → 文本，json → JSON 对象，图片 → 大小+类型元信息，其他 → 文本或错误
#[tauri::command]
pub async fn file_parse(path: String) -> Result<ParsedFile, AppError> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err(AppError::Validation(format!("文件不存在: {path}")));
    }
    let size = std::fs::metadata(p)?.len();
    let mime = mime_for_ext(p);

    match p
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("json") => {
            let text = tokio::fs::read_to_string(p).await?;
            match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(value) => Ok(ParsedFile {
                    path,
                    kind: "json".into(),
                    content: None,
                    json: Some(value),
                    size,
                    mime,
                }),
                Err(_) => Ok(ParsedFile {
                    path,
                    kind: "text".into(),
                    content: Some(text),
                    json: None,
                    size,
                    mime,
                }),
            }
        }
        Some(ext) if is_image(ext) => Ok(ParsedFile {
            path,
            kind: "image".into(),
            content: None,
            json: None,
            size,
            mime,
        }),
        _ => {
            let text = read_text_limited(&path).await.map_err(|_| {
                AppError::Validation(format!("不支持解析的文件类型或二进制内容: {path}"))
            })?;
            Ok(ParsedFile {
                path,
                kind: "text".into(),
                content: Some(text),
                json: None,
                size,
                mime,
            })
        }
    }
}

// ── 共享辅助（chat_send 附件拼接复用） ────────────────────

/// 读取文本文件；超过 200KB 只返回前 100 行并标记截断
pub(crate) async fn read_text_limited(path: &str) -> Result<String, AppError> {
    let meta = tokio::fs::metadata(path).await?;
    if !meta.is_file() {
        return Err(AppError::Validation(format!("'{path}' 不是文件")));
    }

    const LIMIT: u64 = 200 * 1024;
    if meta.len() <= LIMIT {
        return Ok(tokio::fs::read_to_string(path).await?);
    }

    let file = std::fs::File::open(path)?;
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

/// 读取附件文本；读取失败时返回占位说明，避免单个坏附件中断整个消息发送
pub(crate) async fn read_attachment_text(path: &str) -> String {
    match read_text_limited(path).await {
        Ok(text) => text,
        Err(e) => format!("(无法读取附件文本: {e})"),
    }
}

fn list_entries(dir: &Path, depth: u8) -> Result<Vec<FileEntry>, AppError> {
    let mut out = Vec::new();
    let entries =
        std::fs::read_dir(dir).map_err(|e| AppError::Internal(format!("读取目录失败: {e}")))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let meta = entry.metadata().ok();
        let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
        let size = meta.map(|m| m.len()).unwrap_or(0);
        out.push(FileEntry {
            path: path.display().to_string(),
            name: entry.file_name().to_string_lossy().to_string(),
            is_dir,
            size,
        });
        if is_dir && depth > 1 {
            out.extend(list_entries(&path, depth - 1)?);
        }
    }
    out.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.path.cmp(&b.path)));
    Ok(out)
}

fn is_image(ext: &str) -> bool {
    matches!(
        ext,
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg" | "ico"
    )
}

fn mime_for_ext(p: &Path) -> Option<String> {
    let ext = p.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "png" => "image/png".to_string(),
        "jpg" | "jpeg" => "image/jpeg".to_string(),
        "gif" => "image/gif".to_string(),
        "bmp" => "image/bmp".to_string(),
        "webp" => "image/webp".to_string(),
        "svg" => "image/svg+xml".to_string(),
        "ico" => "image/x-icon".to_string(),
        "json" => "application/json".to_string(),
        "md" => "text/markdown".to_string(),
        "txt" | "log" => "text/plain".to_string(),
        "pdf" => "application/pdf".to_string(),
        _ => "application/octet-stream".to_string(),
    })
}
