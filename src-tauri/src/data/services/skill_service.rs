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
    /// 统一小写来源标识：skills.sh / claude-plugins.dev / clawhub.ai / local
    pub source: String,
    /// 安装指令：skills.sh:xxx / github:owner/repo[/path] / zip / local:path
    pub install_source: String,
    pub tags: Vec<String>,
    pub author: Option<String>,
    /// clawhub 提供，用于排序
    pub stars: Option<u64>,
    pub url: Option<String>,
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

        Ok(rows
            .into_iter()
            .map(|r| SkillDto {
                id: r.id,
                name: r.name,
                description: r.description,
                folder_name: r.folder_name,
                source: r.source,
                is_enabled: r.is_enabled != 0,
            })
            .collect())
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

        let folder_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let name = metadata
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| folder_name.clone());

        let description = metadata
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let author = metadata
            .get("author")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let tags = metadata
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let content_hash = compute_hash(&content);

        // 检查是否已安装
        let existing =
            sqlx::query_scalar::<_, String>("SELECT id FROM skills WHERE folder_name = ?")
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
        let folder_name =
            sqlx::query_scalar::<_, String>("SELECT folder_name FROM skills WHERE id = ?")
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
            "SELECT skill_id FROM agent_skills WHERE agent_id = ? AND is_enabled = 1",
        )
        .bind(agent_id)
        .fetch_all(&self.db.pool)
        .await?;

        Ok(ids)
    }

    /// 搜索市场（三源并发：skills.sh / claude-plugins.dev / clawhub.ai）
    pub async fn search_market(&self, query: &str) -> Result<Vec<SkillSearchHit>, AppError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(|e| AppError::Internal(format!("HTTP client: {e}")))?;

        let encoded = urlencoding::encode(query);

        let url_skills = format!("https://skills.sh/api/search?q={encoded}");
        let url_plugins = format!("https://claude-plugins.dev/api/skills?q={encoded}");
        let url_claw = format!("https://clawhub.ai/api/v1/search?query={encoded}");

        let (skills_res, plugins_res, claw_res) = tokio::join!(
            fetch_json(&client, &url_skills),
            fetch_json(&client, &url_plugins),
            fetch_json(&client, &url_claw),
        );

        let mut all_hits: Vec<SkillSearchHit> = Vec::new();

        // Parse skills.sh
        if let Some(val) = skills_res {
            if let Some(arr) = val.as_array() {
                for item in arr {
                    let name = item["name"].as_str().unwrap_or("").to_string();
                    if name.is_empty() {
                        continue;
                    }
                    all_hits.push(SkillSearchHit {
                        id: format!("skills-sh:{}", name),
                        name: name.clone(),
                        description: item["description"].as_str().unwrap_or("").to_string(),
                        source: "skills.sh".into(),
                        install_source: format!("skills.sh:{}", name),
                        tags: item["tags"]
                            .as_array()
                            .map(|a| {
                                a.iter()
                                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                    .collect()
                            })
                            .unwrap_or_default(),
                        author: item["author"].as_str().map(|s| s.to_string()),
                        stars: None,
                        url: item["url"]
                            .as_str()
                            .map(|s| s.to_string())
                            .or_else(|| item["download_url"].as_str().map(|s| s.to_string())),
                        installed: false,
                    });
                }
            }
        }

        // Parse claude-plugins.dev
        if let Some(val) = plugins_res {
            if let Some(arr) = val.as_array() {
                for item in arr {
                    let name = item["name"].as_str().unwrap_or("").to_string();
                    if name.is_empty() {
                        continue;
                    }
                    all_hits.push(SkillSearchHit {
                        id: format!("claude-plugins:{}", name),
                        name: name.clone(),
                        description: item["description"].as_str().unwrap_or("").to_string(),
                        source: "claude-plugins.dev".into(),
                        install_source: item["github"]
                            .as_str()
                            .map(|g| format!("github:{}", g))
                            .unwrap_or_else(|| format!("github:{}", name)),
                        tags: item["tags"]
                            .as_array()
                            .map(|a| {
                                a.iter()
                                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                    .collect()
                            })
                            .unwrap_or_default(),
                        author: item["author"].as_str().map(|s| s.to_string()),
                        stars: None,
                        url: item["url"].as_str().map(|s| s.to_string()),
                        installed: false,
                    });
                }
            }
        }

        // Parse clawhub.ai
        if let Some(val) = claw_res {
            if let Some(arr) = val.as_array() {
                for item in arr {
                    let name = item["name"].as_str().unwrap_or("").to_string();
                    if name.is_empty() {
                        continue;
                    }
                    all_hits.push(SkillSearchHit {
                        id: format!("clawhub:{}", name),
                        name: name.clone(),
                        description: item["description"].as_str().unwrap_or("").to_string(),
                        source: "clawhub.ai".into(),
                        install_source: item["github"]
                            .as_str()
                            .map(|g| format!("github:{}", g))
                            .or_else(|| item["download_url"].as_str().map(|u| format!("zip:{}", u)))
                            .or_else(|| item["install_url"].as_str().map(|u| format!("zip:{}", u)))
                            .unwrap_or_else(|| "zip".into()),
                        tags: item["tags"]
                            .as_array()
                            .map(|a| {
                                a.iter()
                                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                    .collect()
                            })
                            .unwrap_or_default(),
                        author: item["author"].as_str().map(|s| s.to_string()),
                        stars: parse_clawhub_stars(item),
                        url: item["url"].as_str().map(|s| s.to_string()).or_else(|| {
                            item["github"]
                                .as_str()
                                .map(|s| format!("https://github.com/{s}"))
                        }),
                        installed: false,
                    });
                }
            }
        }

        // 查数据库标记已安装
        let installed_names: Vec<String> =
            sqlx::query_scalar::<_, String>("SELECT name FROM skills")
                .fetch_all(&self.db.pool)
                .await?
                .into_iter()
                .map(|n| n.to_lowercase().replace(' ', ""))
                .collect();

        for hit in &mut all_hits {
            let normalized = hit.name.to_lowercase().replace(' ', "");
            hit.installed = installed_names.contains(&normalized);
        }

        // 去重：按归一化名称合并
        let mut map = std::collections::HashMap::new();
        for hit in all_hits {
            let key = normalize_name(&hit.name);
            map.entry(key)
                .and_modify(|existing: &mut SkillSearchHit| {
                    if !hit.description.is_empty() && existing.description.is_empty() {
                        existing.description = hit.description.clone();
                    }
                    if !hit.tags.is_empty() && existing.tags.is_empty() {
                        existing.tags = hit.tags.clone();
                    }
                    if hit.author.is_some() && existing.author.is_none() {
                        existing.author = hit.author.clone();
                    }
                    if !hit.install_source.is_empty() && existing.install_source.is_empty() {
                        existing.install_source = hit.install_source.clone();
                    }
                    if hit.url.is_some() && existing.url.is_none() {
                        existing.url = hit.url.clone();
                    }
                    if hit.stars.unwrap_or(0) > existing.stars.unwrap_or(0) {
                        existing.stars = hit.stars;
                    }
                    if hit.installed {
                        existing.installed = true;
                    }
                })
                .or_insert(hit);
        }

        let mut results: Vec<SkillSearchHit> = map.into_values().collect();
        results.sort_by(|a, b| {
            b.installed
                .cmp(&a.installed)
                .then_with(|| b.stars.unwrap_or(0).cmp(&a.stars.unwrap_or(0)))
                .then_with(|| b.description.is_empty().cmp(&a.description.is_empty()))
        });

        Ok(results)
    }

    /// 从市场安装指令安装技能（解析 install_source 前缀）
    pub async fn install_from_source(&self, source: &str) -> Result<InstalledSkill, AppError> {
        let lower = source.to_lowercase();
        if lower.starts_with("local:") {
            let folder_path = source[6..].trim();
            if folder_path.is_empty() {
                return Err(AppError::Validation("local: 缺少本地路径".into()));
            }
            self.install(folder_path, Some("local")).await
        } else if lower.starts_with("github:") {
            let spec = source[7..].trim();
            if spec.is_empty() {
                return Err(AppError::Validation("github: 缺少 owner/repo".into()));
            }
            self.install_from_github(spec).await
        } else if lower.starts_with("skills.sh:") {
            Err(AppError::Validation(
                "skills.sh 在线安装暂不支持，请手动下载后使用 local: 本地路径安装".into(),
            ))
        } else if lower.starts_with("zip:") {
            Err(AppError::Validation(
                "zip 在线安装暂不支持，请手动下载后使用 local: 本地路径安装".into(),
            ))
        } else {
            // 无前缀：视为本地路径
            self.install(source, Some("local")).await
        }
    }

    /// 从 GitHub 仓库安装（浅克隆后定位 SKILL.md），失败返回可读错误、不 panic
    async fn install_from_github(&self, spec: &str) -> Result<InstalledSkill, AppError> {
        let mut parts = spec.split('/').filter(|s| !s.is_empty());
        let owner = parts
            .next()
            .ok_or_else(|| AppError::Validation("github: 缺少 owner".into()))?;
        let repo = parts
            .next()
            .ok_or_else(|| AppError::Validation("github: 缺少 repo".into()))?;
        let sub_path = {
            let rest: Vec<&str> = parts.collect();
            if rest.is_empty() {
                None
            } else {
                Some(rest.join("/"))
            }
        };

        let tmp_root = std::env::temp_dir().join(format!("prism_skill_{owner}_{repo}"));
        let _ = tokio::fs::remove_dir_all(&tmp_root).await;

        let repo_url = format!("https://github.com/{owner}/{repo}.git");
        let status = tokio::process::Command::new("git")
            .args(["clone", "--depth", "1", &repo_url])
            .arg(&tmp_root)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await;

        let cleanup = || async {
            let _ = tokio::fs::remove_dir_all(&tmp_root).await;
        };

        match status {
            Ok(st) if st.success() => {}
            Ok(_) => {
                cleanup().await;
                return Err(AppError::Internal(format!(
                    "Git 克隆失败（{repo_url}），请手动 clone 后使用 local: 路径安装"
                )));
            }
            Err(e) => {
                cleanup().await;
                return Err(AppError::Internal(format!(
                    "未检测到 git 命令，无法从 GitHub 安装：{e}。请手动 clone 后使用 local: 路径安装"
                )));
            }
        }

        // 去掉 .git，仅保留技能内容
        let _ = tokio::fs::remove_dir_all(tmp_root.join(".git")).await;

        let skill_dir = match &sub_path {
            Some(p) => tmp_root.join(p),
            None => tmp_root.clone(),
        };

        let result = self
            .install(skill_dir.to_str().unwrap_or(""), Some("github"))
            .await;
        cleanup().await;
        result
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
                            let folder_name = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("unknown")
                                .to_string();
                            let name = metadata
                                .get("name")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| folder_name.clone());
                            let description = metadata
                                .get("description")
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
                        let items: Vec<serde_json::Value> = value[1..value.len() - 1]
                            .split(',')
                            .map(|s| {
                                serde_json::Value::String(s.trim().trim_matches('"').to_string())
                            })
                            .collect();
                        map.insert(key, serde_json::Value::Array(items));
                    } else if value == "true" {
                        map.insert(key, serde_json::Value::Bool(true));
                    } else if value == "false" {
                        map.insert(key, serde_json::Value::Bool(false));
                    } else if let Ok(n) = value.parse::<f64>() {
                        map.insert(
                            key,
                            serde_json::Value::Number(serde_json::Number::from_f64(n).unwrap()),
                        );
                    } else {
                        map.insert(
                            key,
                            serde_json::Value::String(value.trim_matches('"').to_string()),
                        );
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

// ── 名称归一化（去重用） ──────────────────────────────────

fn normalize_name(name: &str) -> String {
    let mut s = name.to_lowercase().replace(' ', "");
    for suffix in &["-agent", "-skill", "agent", "skill"] {
        if let Some(stripped) = s.strip_suffix(suffix) {
            if !stripped.is_empty() {
                s = stripped.to_string();
            }
        }
    }
    s
}

// ── HTTP 请求辅助 ─────────────────────────────────────────

async fn fetch_json(client: &reqwest::Client, url: &str) -> Option<serde_json::Value> {
    let resp = client.get(url).send().await.ok()?;
    resp.json::<serde_json::Value>().await.ok()
}

// ── clawhub stars 解析 ────────────────────────────────────
// clawhub API 结构多变：stars 可能为顶层字段、stats 子对象或字符串

fn parse_clawhub_stars(item: &serde_json::Value) -> Option<u64> {
    for key in [
        "stars",
        "star_count",
        "stargazers_count",
        "stats/stars",
        "stats/star_count",
        "stats/stargazers",
    ] {
        let v = item.pointer(&format!("/{key}"));
        let v = match v {
            Some(v) => v,
            None => continue,
        };
        if let Some(n) = v.as_u64() {
            return Some(n);
        }
        if let Some(f) = v.as_f64() {
            if f > 0.0 && f.is_finite() {
                return Some(f as u64);
            }
        }
        if let Some(s) = v.as_str() {
            if let Ok(n) = s.trim().parse::<u64>() {
                return Some(n);
            }
        }
    }
    None
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
