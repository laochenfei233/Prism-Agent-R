use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// §19.3.6 指令渐进披露
///
/// AGENTS.md 当目录（~100 行）+ docs/ 分片 + CI 校验
/// progressive 模式下仅注入目录 + 命中分片，prompt 体积显著小于 single

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InstructionMode {
    /// 单文件模式（兼容旧行为）
    Single,
    /// 渐进披露模式（推荐）
    Progressive,
}

/// 指令分片
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstructionShard {
    pub name: String,
    pub path: PathBuf,
    pub content: String,
    pub keywords: Vec<String>,
}

/// 指令管理器
pub struct InstructionManager {
    mode: InstructionMode,
    shards: Vec<InstructionShard>,
    base_path: PathBuf,
}

impl InstructionManager {
    pub fn new(base_path: PathBuf, mode: InstructionMode) -> Self {
        Self {
            mode,
            shards: Vec::new(),
            base_path,
        }
    }

    /// 加载指令分片
    pub fn load_shards(&mut self) -> Result<(), String> {
        let ag_dir = self.base_path.join("docs").join("AGENTS");
        if !ag_dir.exists() {
            return Ok(());
        }

        // 读取目录文件（AGENTS.md）
        let index_path = ag_dir.join("AGENTS.md");
        if index_path.exists() {
            let content = std::fs::read_to_string(&index_path)
                .map_err(|e| format!("读取 AGENTS.md 失败: {e}"))?;
            tracing::info!("Loaded AGENTS.md index ({} bytes)", content.len());
        }

        // 扫描分片文件
        if let Ok(entries) = std::fs::read_dir(&ag_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "md") {
                    let name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();

                    if name == "AGENTS" {
                        continue; // 跳过索引文件
                    }

                    let content = std::fs::read_to_string(&path).unwrap_or_default();

                    let keywords = extract_keywords(&content);

                    self.shards.push(InstructionShard {
                        name,
                        path,
                        content,
                        keywords,
                    });
                }
            }
        }

        tracing::info!("Loaded {} instruction shards", self.shards.len());
        Ok(())
    }

    /// 根据用户输入选择相关分片
    pub fn select_relevant_shards(&self, query: &str, max_shards: usize) -> Vec<&InstructionShard> {
        let query_lower = query.to_lowercase();

        let mut scored: Vec<(&InstructionShard, f32)> = self
            .shards
            .iter()
            .map(|shard| {
                let score = shard
                    .keywords
                    .iter()
                    .filter(|kw| query_lower.contains(&kw.to_lowercase()))
                    .count() as f32;
                (shard, score)
            })
            .filter(|(_, score)| *score > 0.0)
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored
            .into_iter()
            .take(max_shards)
            .map(|(shard, _)| shard)
            .collect()
    }

    /// 构建注入内容
    pub fn build_injection(&self, query: &str) -> String {
        match self.mode {
            InstructionMode::Single => {
                // 单文件模式：返回所有分片内容
                self.shards
                    .iter()
                    .map(|s| s.content.clone())
                    .collect::<Vec<_>>()
                    .join("\n\n---\n\n")
            }
            InstructionMode::Progressive => {
                // 渐进模式：仅返回目录 + 命中分片
                let relevant = self.select_relevant_shards(query, 3);
                if relevant.is_empty() {
                    return String::new();
                }

                let mut output = String::from("## 指令目录\n\n");
                for shard in &self.shards {
                    let marker = if relevant.iter().any(|r| r.name == shard.name) {
                        " (已加载)"
                    } else {
                        ""
                    };
                    output.push_str(&format!("- {}{}\n", shard.name, marker));
                }

                output.push_str("\n---\n\n");

                for shard in relevant {
                    output.push_str(&format!("## {}\n\n{}\n\n", shard.name, shard.content));
                }

                output
            }
        }
    }

    /// 获取注入体积估计（字节）
    pub fn estimate_injection_size(&self, query: &str) -> usize {
        self.build_injection(query).len()
    }
}

/// 从内容中提取关键词
fn extract_keywords(content: &str) -> Vec<String> {
    let mut keywords = Vec::new();

    // 提取标题
    for line in content.lines() {
        if line.starts_with('#') {
            let keyword = line.trim_start_matches('#').trim().to_string();
            if !keyword.is_empty() && keyword.len() < 50 {
                keywords.push(keyword);
            }
        }
    }

    // 提取代码块中的关键词
    let mut in_code = false;
    for line in content.lines() {
        if line.starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            // 提取函数名、结构体名等
            for word in line.split_whitespace() {
                if word.len() > 3 && word.len() < 30 {
                    keywords.push(word.to_string());
                }
            }
        }
    }

    keywords.dedup();
    keywords
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instruction_mode_serialize() {
        let mode = InstructionMode::Progressive;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, "\"Progressive\"");
    }

    #[test]
    fn test_extract_keywords() {
        let content = "# RAG 检索\n\n## HyDE\n\n使用向量检索";
        let keywords = extract_keywords(content);
        assert!(keywords.contains(&"RAG 检索".to_string()));
        assert!(keywords.contains(&"HyDE".to_string()));
    }

    #[test]
    fn test_select_relevant_shards() {
        let mut manager = InstructionManager::new(PathBuf::new(), InstructionMode::Progressive);
        manager.shards.push(InstructionShard {
            name: "rag".into(),
            path: PathBuf::new(),
            content: "RAG 检索相关内容".into(),
            keywords: vec!["RAG".into(), "检索".into(), "向量".into()],
        });
        manager.shards.push(InstructionShard {
            name: "security".into(),
            path: PathBuf::new(),
            content: "安全相关内容".into(),
            keywords: vec!["安全".into(), "认证".into(), "授权".into()],
        });

        let relevant = manager.select_relevant_shards("如何使用 RAG 检索", 3);
        assert_eq!(relevant.len(), 1);
        assert_eq!(relevant[0].name, "rag");
    }
}
