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
        "读取本地文件内容。参数：path（文件路径）。返回文件文本内容，超过 100KB 截断。"
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "文件路径" }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolOutput, AgentError> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| AgentError::InvalidArgs("file_read: 缺少 path".into()))?;

        let path = Path::new(path);
        if !path.exists() {
            return Ok(ToolOutput::error(format!("文件不存在: {}", path.display())));
        }
        if !path.is_file() {
            return Ok(ToolOutput::error(format!("不是文件: {}", path.display())));
        }

        match tokio::fs::read_to_string(path).await {
            Ok(content) => {
                let truncated = if content.len() > 100_000 {
                    let lines: Vec<&str> = content.lines().take(200).collect();
                    format!("{}\n\n... [截断：文件超过 100KB，仅显示前 200 行]", lines.join("\n"))
                } else {
                    content
                };
                Ok(ToolOutput::text(truncated))
            }
            Err(e) => Ok(ToolOutput::error(format!("读取失败: {e}"))),
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
        "写入本地文件（自动创建父目录）。参数：path（文件路径）、content（文件内容）。返回写入的路径。"
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

        match tokio::fs::write(path, content).await {
            Ok(()) => Ok(ToolOutput::text(format!("已写入: {}", path.display()))),
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
        "列出目录内容。参数：path（目录路径）、depth（递归深度，默认 1，最大 3）。返回文件和目录列表。"
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "目录路径" },
                "depth": { "type": "integer", "minimum": 1, "maximum": 3, "default": 1, "description": "递归深度" }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolOutput, AgentError> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| AgentError::InvalidArgs("file_list: 缺少 path".into()))?;
        let depth = args["depth"].as_u64().unwrap_or(1).min(3) as u8;

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

fn list_dir(dir: &Path, max_depth: u8, current_depth: u8) -> Result<Vec<String>, AgentError> {
    let mut entries = Vec::new();
    if current_depth > max_depth {
        return Ok(entries);
    }

    let read_dir = std::fs::read_dir(dir)
        .map_err(|e| AgentError::Internal(format!("读取目录失败: {e}")))?;

    let mut items: Vec<_> = read_dir
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            !name.starts_with('.') && name != "node_modules" && name != "target"
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
            let size_str = if size < 1024 {
                format!("{}B", size)
            } else if size < 1024 * 1024 {
                format!("{}KB", size / 1024)
            } else {
                format!("{}MB", size / (1024 * 1024))
            };
            entries.push(format!("{prefix}{} ({})", name, size_str));
        }
    }

    Ok(entries)
}
