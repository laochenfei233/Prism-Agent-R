use std::path::PathBuf;

use crate::data::models::{SkillDto, SkillRow};
use crate::data::Database;
use crate::utils::error::AppError;

// ── 技能服务 ──────────────────────────────────────────────

pub struct SkillService {
    db: Database,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InstalledSkill {
    pub id: String,
    pub name: String,
    pub folder_name: String,
    pub source: String,
    pub is_enabled: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillSearchHit {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: String,
    pub tags: Vec<String>,
    pub author: Option<String>,
    pub installed: bool,
}

impl SkillService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// 列出所有技能
    pub async fn list(&self) -> Result<Vec<SkillDto>, AppError> {
        let rows = sqlx::query_as::<_, SkillRow>(
            "SELECT id, name, description, folder_name, source, source_url, namespace, author, tags, content_hash, is_enabled, created_at, updated_at FROM skills ORDER BY created_at"
        )
        .fetch_all(&self.db.pool)
        .await?;

        Ok(rows.into_iter().map(|r| SkillDto {
            id: r.id,
            name: r.name,
            description: r.description,
            folder_name: r.folder_name,
            source: r.source,
            is_enabled: r.is_enabled != 0,
        }).collect())
    }

    /// 安装技能（从本地目录加载 SKILL.md）
    pub async fn install(
        &self,
        folder_path: &str,
        source: Option<&str>,
    ) -> Result<InstalledSkill, AppError> {
        let path = PathBuf::from(folder_path);
        let skill_md = path.join("SKILL.md");

        if !skill_md.exists() {
            return Err(AppError::Validation("目录中未找到 SKILL.md".into()));
        }

        // 读取并解析 SKILL.md frontmatter
        let content = tokio::fs::read_to_string(&skill_md).await?;
        let (metadata, _) = parse_frontmatter(&content);

        let folder_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let name = metadata.get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| folder_name.clone());

        let description = metadata.get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let author = metadata.get("author")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let tags = metadata.get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect::<Vec<_>>())
            .unwrap_or_default();

        let content_hash = compute_hash(&content);

        // 检查是否已安装
        let existing = sqlx::query_scalar::<_, String>(
            "SELECT id FROM skills WHERE folder_name = ?"
        )
        .bind(&folder_name)
        .fetch_optional(&self.db.pool)
        .await?;

        let now = chrono::Utc::now().timestamp_millis();
        let skill_dir = crate::utils::paths::skill_dir();

        if let Some(id) = existing {
            // 更新
            sqlx::query(
                "UPDATE skills SET name = ?, description = ?, content_hash = ?, updated_at = ? WHERE id = ?"
            )
            .bind(&name)
            .bind(&description)
            .bind(&content_hash)
            .bind(now)
            .bind(&id)
            .execute(&self.db.pool)
            .await?;

            Ok(InstalledSkill {
                id,
                name,
                folder_name,
                source: source.unwrap_or("local").to_string(),
                is_enabled: true,
            })
        } else {
            // 新建
            let id = uuid::Uuid::new_v4().to_string();

            sqlx::query(
                "INSERT INTO skills (id, name, description, folder_name, source, source_url, namespace, author, tags, content_hash, is_enabled, created_at, updated_at) VALUES (?, ?, ?, ?, ?, NULL, NULL, ?, ?, ?, 1, ?, ?)"
            )
            .bind(&id)
            .bind(&name)
            .bind(&description)
            .bind(&folder_name)
            .bind(source.unwrap_or("local"))
            .bind(&author)
            .bind(serde_json::to_string(&tags).unwrap_or_else(|_| "[]".into()))
            .bind(&content_hash)
            .bind(now)
            .bind(now)
            .execute(&self.db.pool)
            .await?;

            // 复制技能目录到应用数据
            let target_dir = skill_dir.join(&folder_name);
            if !target_dir.exists() {
                tokio::fs::create_dir_all(&target_dir).await?;
                copy_dir_all(&path, &target_dir).await?;
            }

            Ok(InstalledSkill {
                id,
                name,
                folder_name,
                source: source.unwrap_or("local").to_string(),
                is_enabled: true,
            })
        }
    }

    /// 卸载技能
    pub async fn uninstall(&self, id: &str) -> Result<(), AppError> {
        let folder_name = sqlx::query_scalar::<_, String>(
            "SELECT folder_name FROM skills WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.db.pool)
        .await?
        .ok_or_else(|| AppError::Validation(format!("技能不存在: {id}")))?;

        sqlx::query("DELETE FROM skills WHERE id = ?")
            .bind(id)
            .execute(&self.db.pool)
            .await?;

        // 删除磁盘文件
        let skill_dir = crate::utils::paths::skill_dir().join(&folder_name);
        if skill_dir.exists() {
            let _ = tokio::fs::remove_dir_all(&skill_dir).await;
        }

        Ok(())
    }

    /// 切换技能启用状态
    pub async fn toggle(
        &self,
        agent_id: &str,
        skill_id: &str,
        enabled: bool,
    ) -> Result<(), AppError> {
        let now = chrono::Utc::now().timestamp_millis();

        if enabled {
            sqlx::query(
                "INSERT OR REPLACE INTO agent_skills (agent_id, skill_id, is_enabled, created_at) VALUES (?, ?, 1, ?)"
            )
            .bind(agent_id)
            .bind(skill_id)
            .bind(now)
            .execute(&self.db.pool)
            .await?;
        } else {
            sqlx::query("DELETE FROM agent_skills WHERE agent_id = ? AND skill_id = ?")
                .bind(agent_id)
                .bind(skill_id)
                .execute(&self.db.pool)
                .await?;
        }

        Ok(())
    }

    /// 获取 Agent 启用的技能 ID 列表
    pub async fn enabled_skills(&self, agent_id: &str) -> Result<Vec<String>, AppError> {
        let ids = sqlx::query_scalar::<_, String>(
            "SELECT skill_id FROM agent_skills WHERE agent_id = ? AND is_enabled = 1"
        )
        .bind(agent_id)
        .fetch_all(&self.db.pool)
        .await?;

        Ok(ids)
    }

    /// 搜索市场（简化版本，返回空列表）
    pub async fn search_market(&self, _query: &str) -> Result<Vec<SkillSearchHit>, AppError> {
        // MVP 阶段返回空列表，Phase 2 实现三源搜索
        Ok(Vec::new())
    }

    /// 列出本地技能（指定目录下的 SKILL.md）
    pub async fn list_local(&self, workdir: &str) -> Result<Vec<LocalSkill>, AppError> {
        let mut skills = Vec::new();
        let workdir = PathBuf::from(workdir);

        // 搜索 .claude/skills 和 .agents/skills 目录
        for subdir in &[".claude/skills", ".agents/skills", ".opencode/skills"] {
            let skills_dir = workdir.join(subdir);
            if skills_dir.exists() {
                let mut entries = tokio::fs::read_dir(&skills_dir).await?;
                while let Some(entry) = entries.next_entry().await? {
                    let path = entry.path();
                    if path.is_dir() {
                        let skill_md = path.join("SKILL.md");
                        if skill_md.exists() {
                            let content = tokio::fs::read_to_string(&skill_md).await?;
                            let (metadata, _) = parse_frontmatter(&content);
                            let folder_name = path.file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("unknown")
                                .to_string();
                            let name = metadata.get("name")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| folder_name.clone());
                            let description = metadata.get("description")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            skills.push(LocalSkill {
                                name,
                                description,
                                path: path.to_string_lossy().to_string(),
                            });
                        }
                    }
                }
            }
        }

        Ok(skills)
    }
}

// ── 本地技能 ──────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LocalSkill {
    pub name: String,
    pub description: Option<String>,
    pub path: String,
}

// ── Frontmatter 解析 ─────────────────────────────────────

fn parse_frontmatter(content: &str) -> (serde_json::Value, &str) {
    if let Some(rest) = content.strip_prefix("---") {
        if let Some(end) = rest.find("---") {
            let yaml_str = &rest[..end];
            let body = &rest[end + 3..];

            // 简单 YAML 解析（支持基本 key: value 和 key: [array]）
            let mut map = serde_json::Map::new();
            for line in yaml_str.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((key, value)) = line.split_once(':') {
                    let key = key.trim().to_string();
                    let value = value.trim();
                    if value.starts_with('[') && value.ends_with(']') {
                        // 数组
                        let items: Vec<serde_json::Value> = value[1..value.len()-1]
                            .split(',')
                            .map(|s| serde_json::Value::String(s.trim().trim_matches('"').to_string()))
                            .collect();
                        map.insert(key, serde_json::Value::Array(items));
                    } else if value == "true" {
                        map.insert(key, serde_json::Value::Bool(true));
                    } else if value == "false" {
                        map.insert(key, serde_json::Value::Bool(false));
                    } else if let Ok(n) = value.parse::<f64>() {
                        map.insert(key, serde_json::Value::Number(serde_json::Number::from_f64(n).unwrap()));
                    } else {
                        map.insert(key, serde_json::Value::String(value.trim_matches('"').to_string()));
                    }
                }
            }
            return (serde_json::Value::Object(map), body);
        }
    }
    (serde_json::Value::Object(serde_json::Map::new()), content)
}

// ── 哈希计算 ──────────────────────────────────────────────

fn compute_hash(content: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

// ── 目录复制 ──────────────────────────────────────────────

async fn copy_dir_all(src: &PathBuf, dst: &PathBuf) -> Result<(), std::io::Error> {
    tokio::fs::create_dir_all(dst).await?;
    let mut entries = tokio::fs::read_dir(src).await?;
    while let Some(entry) = entries.next_entry().await? {
        let ty = entry.file_type().await?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            Box::pin(copy_dir_all(&src_path, &dst_path)).await?;
        } else {
            tokio::fs::copy(&src_path, &dst_path).await?;
        }
    }
    Ok(())
}
