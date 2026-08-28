use async_trait::async_trait;
use regex::Regex;
use serde_json::json;
use std::path::{Path, PathBuf};

use super::error::AgentError;
use super::model::ToolOutput;
use super::tool::ToolExecutor;

// ── 通用遍历辅助 ─────────────────────────────────────────

const IGNORE_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    ".next",
    "dist",
    "build",
    "__pycache__",
    ".venv",
    ".mimocode",
    ".claude",
    ".opencode",
    ".codex",
];

/// 递归收集目录下的文件（跳过忽略目录与隐藏项）
fn collect_files(root: &Path, out: &mut Vec<PathBuf>, max_files: usize) {
    if out.len() >= max_files {
        return;
    }
    let Ok(read_dir) = std::fs::read_dir(root) else {
        return;
    };
    for entry in read_dir.filter_map(|e| e.ok()) {
        if out.len() >= max_files {
            return;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || IGNORE_DIRS.contains(&name.as_str()) {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_files(&entry.path(), out, max_files);
        } else if file_type.is_file() {
            out.push(entry.path());
        }
    }
}

/// 将 glob 模式转为正则（支持 `**`、`*`、`?`、`[...]`）
fn glob_to_regex(pattern: &str) -> Result<Regex, AgentError> {
    let mut re = String::from("^");
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '*' => {
                if i + 1 < chars.len() && chars[i + 1] == '*' {
                    // `**`：跨目录通配；`**/` 允许匹配任意前缀目录
                    if i + 2 < chars.len() && chars[i + 2] == '/' {
                        re.push_str("(?:.*/)?");
                        i += 3;
                        continue;
                    }
                    re.push_str(".*");
                    i += 2;
                } else {
                    re.push_str("[^/]*");
                    i += 1;
                }
            }
            '?' => {
                re.push_str("[^/]");
                i += 1;
            }
            '[' => {
                // 透传字符类，直到匹配的 ']'
                let mut j = i + 1;
                let mut class = String::from("[");
                while j < chars.len() && chars[j] != ']' {
                    class.push(chars[j]);
                    j += 1;
                }
                if j < chars.len() {
                    class.push(']');
                    re.push_str(&class);
                    i = j + 1;
                } else {
                    // 未闭合的 '[' 按字面处理
                    re.push_str("\\[");
                    i += 1;
                }
            }
            c => {
                if r"\.+()|{}^$".contains(c) {
                    re.push('\\');
                }
                re.push(c);
                i += 1;
            }
        }
    }
    re.push('$');
    Regex::new(&re).map_err(|e| AgentError::InvalidArgs(format!("无效的 glob 模式: {e}")))
}

// ── Grep Tool ────────────────────────────────────────────

pub struct GrepTool;

#[async_trait]
impl ToolExecutor for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "使用正则表达式搜索目录下文件内容，返回匹配的文件路径、行号与文本。参数：pattern（正则表达式）、path（目录或文件，默认当前目录）、file_pattern（可选，文件名 glob 过滤，如 *.rs）、max_results（默认 100，最大 500）。"
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "正则表达式" },
                "path": { "type": "string", "default": ".", "description": "搜索根目录或文件" },
                "file_pattern": { "type": "string", "description": "文件名 glob 过滤，如 *.rs" },
                "max_results": { "type": "integer", "minimum": 1, "maximum": 500, "default": 100, "description": "最大结果数" }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolOutput, AgentError> {
        let pattern = args["pattern"]
            .as_str()
            .ok_or_else(|| AgentError::InvalidArgs("grep: 缺少 pattern".into()))?;
        let root = args["path"].as_str().unwrap_or(".");
        let max_results = args["max_results"].as_u64().unwrap_or(100).min(500) as usize;
        let file_re = match args["file_pattern"].as_str() {
            Some(fp) => Some(glob_to_regex(fp)?),
            None => None,
        };

        let regex = Regex::new(pattern)
            .map_err(|e| AgentError::InvalidArgs(format!("无效的正则表达式: {e}")))?;
        let root = Path::new(root);
        if !root.exists() {
            return Ok(ToolOutput::error(format!("路径不存在: {}", root.display())));
        }

        let mut files = Vec::new();
        if root.is_file() {
            files.push(root.to_path_buf());
        } else {
            collect_files(root, &mut files, 10_000);
        }

        let mut hits = Vec::new();
        let mut total = 0usize;
        'outer: for file in files {
            if let Some(re) = &file_re {
                let name = file
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                if !re.is_match(&name) {
                    continue;
                }
            }
            let Ok(content) = std::fs::read_to_string(&file) else {
                continue;
            };
            for (idx, line) in content.lines().enumerate() {
                if regex.is_match(line) {
                    let snippet = line.chars().take(300).collect::<String>();
                    hits.push(format!("{}:{}:{}", file.display(), idx + 1, snippet));
                    total += 1;
                    if total >= max_results {
                        break 'outer;
                    }
                }
            }
        }

        if hits.is_empty() {
            return Ok(ToolOutput::text(format!("未找到匹配「{pattern}」的内容")));
        }
        let mut out = format!("匹配 {} 处（pattern: {pattern}）:\n", hits.len());
        out.push_str(&hits.join("\n"));
        if total >= max_results {
            out.push_str(&format!("\n... 已截断，仅显示前 {max_results} 条"));
        }
        Ok(ToolOutput::text(out))
    }
}

// ── Glob Tool ────────────────────────────────────────────

pub struct GlobTool;

#[async_trait]
impl ToolExecutor for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "按文件名模式匹配遍历目录，返回匹配的路径列表。参数：pattern（glob 模式，如 **/*.rs）、path（根目录，默认当前目录）、max_results（默认 200，最大 1000）。"
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "glob 模式，支持 ** * ? [..]" },
                "path": { "type": "string", "default": ".", "description": "根目录" },
                "max_results": { "type": "integer", "minimum": 1, "maximum": 1000, "default": 200, "description": "最大结果数" }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolOutput, AgentError> {
        let pattern = args["pattern"]
            .as_str()
            .ok_or_else(|| AgentError::InvalidArgs("glob: 缺少 pattern".into()))?;
        let root = args["path"].as_str().unwrap_or(".");
        let max_results = args["max_results"].as_u64().unwrap_or(200).min(1000) as usize;

        let regex = glob_to_regex(pattern)?;
        let root = Path::new(root);
        if !root.exists() {
            return Ok(ToolOutput::error(format!("路径不存在: {}", root.display())));
        }

        let mut files = Vec::new();
        if root.is_file() {
            files.push(root.to_path_buf());
        } else {
            collect_files(root, &mut files, 20_000);
        }

        let mut matched = Vec::new();
        for file in files {
            if matched.len() >= max_results {
                break;
            }
            let rel = file
                .strip_prefix(root)
                .unwrap_or(&file)
                .to_string_lossy()
                .replace('\\', "/");
            if regex.is_match(&rel) {
                matched.push(file.to_string_lossy().into_owned());
            }
        }

        if matched.is_empty() {
            return Ok(ToolOutput::text(format!("未找到匹配「{pattern}」的文件")));
        }
        let mut out = format!("匹配 {} 个文件（pattern: {pattern}）:\n", matched.len());
        out.push_str(&matched.join("\n"));
        if matched.len() >= max_results {
            out.push_str(&format!("\n... 已截断，仅显示前 {max_results} 条"));
        }
        Ok(ToolOutput::text(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_star() {
        let re = glob_to_regex("*.rs").unwrap();
        assert!(re.is_match("main.rs"));
        assert!(!re.is_match("src/main.rs"));
    }

    #[test]
    fn glob_double_star() {
        let re = glob_to_regex("**/*.rs").unwrap();
        assert!(re.is_match("main.rs"));
        assert!(re.is_match("src/main.rs"));
        assert!(re.is_match("a/b/c/main.rs"));
    }

    #[test]
    fn glob_question() {
        let re = glob_to_regex("?.txt").unwrap();
        assert!(re.is_match("a.txt"));
        assert!(!re.is_match("ab.txt"));
    }
}
