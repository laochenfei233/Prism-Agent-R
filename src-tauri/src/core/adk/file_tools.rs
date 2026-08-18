use async_trait::async_trait;
use serde_json::json;
use std::path::Path;

use super::error::AgentError;
use super::model::ToolOutput;
use super::tool::ToolExecutor;

// ── File Read Tool ────────────────────────────────────────

pub struct FileReadTool;

#[async_trait]
impl ToolExecutor for FileReadTool {
    fn name(&self) -> &str {
        "file_read"
    }

    fn description(&self) -> &str {
        "读取本地文件内容。参数：path（文件路径），offset（起始行号，默认 0），limit（最大行数，默认不限制）。支持行范围读取，超过 1MB 自动截断。二进制文件返回文件信息而非内容。"
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "文件路径" },
                "offset": { "type": "integer", "minimum": 0, "default": 0, "description": "起始行号（从 0 开始）" },
                "limit": { "type": "integer", "minimum": 1, "default": 0, "description": "最大行数（0=不限制）" }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolOutput, AgentError> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| AgentError::InvalidArgs("file_read: 缺少 path".into()))?;
        let offset = args["offset"].as_u64().unwrap_or(0) as usize;
        let limit = args["limit"].as_u64().unwrap_or(0) as usize;

        let path = Path::new(path);
        if !path.exists() {
            return Ok(ToolOutput::error(format!("文件不存在: {}", path.display())));
        }
        if !path.is_file() {
            return Ok(ToolOutput::error(format!("不是文件: {}", path.display())));
        }

        // 检查文件大小
        let metadata = tokio::fs::metadata(path).await?;
        let file_size = metadata.len();

        // 二进制文件检测：读取前 512 字节检查是否有 null 字节
        if file_size > 0 {
            let peek = tokio::fs::read(path).await.unwrap_or_default();
            let check_len = peek.len().min(512);
            if peek[..check_len].contains(&0) {
                let size_str = format_size(file_size);
                return Ok(ToolOutput::text(format!(
                    "二进制文件: {} ({})，无法以文本方式读取",
                    path.display(), size_str
                )));
            }
        }

        match tokio::fs::read_to_string(path).await {
            Ok(content) => {
                let lines: Vec<&str> = content.lines().collect();
                let total_lines = lines.len();

                // 应用行范围
                let start = offset.min(total_lines);
                let end = if limit > 0 {
                    (start + limit).min(total_lines)
                } else {
                    total_lines
                };
                let selected: Vec<&str> = lines[start..end].to_vec();

                // 如果内容过大（>1MB），截断
                let truncated;
                let mut result = selected.join("\n");
                if result.len() > 1_000_000 {
                    result = result[..1_000_000].to_string();
                    truncated = true;
                } else {
                    truncated = false;
                }

                let header = if start > 0 || limit > 0 {
                    format!("// 文件: {} (共 {} 行，显示 {}-{})\n", path.display(), total_lines, start + 1, end)
                } else {
                    format!("// 文件: {} (共 {} 行)\n", path.display(), total_lines)
                };

                let footer = if truncated {
                    "\n\n... [内容超过 1MB，已截断]"
                } else if end < total_lines {
                    &format!("\n\n... [仅显示第 {}-{} 行，共 {} 行]", start + 1, end, total_lines)
                } else {
                    ""
                };

                Ok(ToolOutput::text(format!("{}{}{}", header, result, footer)))
            }
            Err(e) => {
                // 尝试作为二进制文件返回基本信息
                let size_str = format_size(file_size);
                Ok(ToolOutput::error(format!("读取失败 ({}): {e}", size_str)))
            }
        }
    }
}

// ── File Write Tool ───────────────────────────────────────

pub struct FileWriteTool;

#[async_trait]
impl ToolExecutor for FileWriteTool {
    fn name(&self) -> &str {
        "file_write"
    }

    fn description(&self) -> &str {
        "写入本地文件（自动创建父目录，覆盖已有文件）。参数：path（文件路径）、content（文件内容）。返回写入的路径和字节数。"
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "文件路径" },
                "content": { "type": "string", "description": "文件内容" }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolOutput, AgentError> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| AgentError::InvalidArgs("file_write: 缺少 path".into()))?;
        let content = args["content"]
            .as_str()
            .ok_or_else(|| AgentError::InvalidArgs("file_write: 缺少 content".into()))?;

        let path = Path::new(path);
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }

        let bytes = content.as_bytes().len();
        match tokio::fs::write(path, content).await {
            Ok(()) => Ok(ToolOutput::text(format!("已写入: {} ({})", path.display(), format_size(bytes as u64)))),
            Err(e) => Ok(ToolOutput::error(format!("写入失败: {e}"))),
        }
    }
}

// ── File List Tool ────────────────────────────────────────

pub struct FileListTool;

#[async_trait]
impl ToolExecutor for FileListTool {
    fn name(&self) -> &str {
        "file_list"
    }

    fn description(&self) -> &str {
        "列出目录内容。参数：path（目录路径）、depth（递归深度，默认 1，最大 5）。返回文件和目录列表，含大小信息。"
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "目录路径" },
                "depth": { "type": "integer", "minimum": 1, "maximum": 5, "default": 1, "description": "递归深度" }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolOutput, AgentError> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| AgentError::InvalidArgs("file_list: 缺少 path".into()))?;
        let depth = args["depth"].as_u64().unwrap_or(1).min(5) as u8;

        let dir = Path::new(path);
        if !dir.is_dir() {
            return Ok(ToolOutput::error(format!("不是目录: {}", dir.display())));
        }

        let entries = list_dir(dir, depth, 0)?;
        if entries.is_empty() {
            return Ok(ToolOutput::text("目录为空".to_string()));
        }
        Ok(ToolOutput::text(entries.join("\n")))
    }
}

// ── Helpers ───────────────────────────────────────────────

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{}B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{}KB", bytes / 1024)
    } else {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn list_dir(dir: &Path, max_depth: u8, current_depth: u8) -> Result<Vec<String>, AgentError> {
    let mut entries = Vec::new();
    if current_depth > max_depth {
        return Ok(entries);
    }

    let read_dir = std::fs::read_dir(dir)
        .map_err(|e| AgentError::Internal(format!("读取目录失败: {e}")))?;

    let ignore_names = [".git", "node_modules", "target", ".next", "dist", "build", "__pycache__", ".venv", ".mimocode", ".claude", ".opencode", ".codex"];

    let mut items: Vec<_> = read_dir
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            !name.starts_with('.') && !ignore_names.contains(&name.as_str())
        })
        .collect();

    items.sort_by(|a, b| {
        let a_is_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let b_is_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
        b_is_dir.cmp(&a_is_dir).then_with(|| a.file_name().cmp(&b.file_name()))
    });

    for item in items {
        let name = item.file_name().to_string_lossy().to_string();
        let is_dir = item.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let prefix = if current_depth > 0 { "  ".repeat(current_depth as usize) } else { String::new() };

        if is_dir {
            entries.push(format!("{prefix}{}/", name));
            entries.extend(list_dir(&item.path(), max_depth, current_depth + 1)?);
        } else {
            let size = item.metadata().map(|m| m.len()).unwrap_or(0);
            entries.push(format!("{prefix}{} ({})", name, format_size(size)));
        }
    }

    Ok(entries)
}
