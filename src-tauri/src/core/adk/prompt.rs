use crate::data::models::AgentDto;
use crate::data::Database;
use crate::utils::error::AppError;



// ── Prompt Builder ────────────────────────────────────────

pub struct PromptBuilder {
    db: Database,
}

impl PromptBuilder {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn build_system_prompt(
        &self,
        agent: &AgentDto,
        _session_id: &str,
        enabled_skills: &[String],
    ) -> Result<String, AppError> {
        let mut parts = Vec::new();

        // Agent system prompt
        if let Some(sp) = &agent.system_prompt {
            parts.push(sp.clone());
        }

        // Inject enabled skills
        for skill_id in enabled_skills {
            if let Some(content) = self.load_skill_content(skill_id).await? {
                parts.push(format!("\n---\n# Skill\n{}\n", content));
            }
        }

        // Inject project memory
        if let Some(memory) = self.load_project_memory().await? {
            parts.push(format!("\n---\n# Project Memory\n{}\n", memory));
        }

        // Inject global memory
        if let Some(memory) = self.load_global_memory().await? {
            parts.push(format!("\n---\n# Global Memory\n{}\n", memory));
        }

        Ok(parts.join("\n"))
    }

    async fn load_skill_content(&self, skill_id: &str) -> Result<Option<String>, AppError> {
        let row = sqlx::query_scalar::<_, String>(
            "SELECT folder_name FROM skills WHERE id = ? AND is_enabled = 1"
        )
        .bind(skill_id)
        .fetch_optional(&self.db.pool)
        .await?;

        if let Some(folder_name) = row {
            let skill_path = crate::utils::paths::skill_dir().join(&folder_name).join("SKILL.md");
            if skill_path.exists() {
                let content = tokio::fs::read_to_string(&skill_path).await?;
                return Ok(Some(content));
            }
        }
        Ok(None)
    }

    async fn load_project_memory(&self) -> Result<Option<String>, AppError> {
        let memory_path = std::env::current_dir()
            .unwrap_or_default()
            .join(".prism")
            .join("memory.md");
        if memory_path.exists() {
            let content = tokio::fs::read_to_string(&memory_path).await?;
            return Ok(Some(content));
        }
        Ok(None)
    }

    async fn load_global_memory(&self) -> Result<Option<String>, AppError> {
        let memory_path = crate::utils::paths::memory_dir().join("global").join("MEMORY.md");
        if memory_path.exists() {
            let content = tokio::fs::read_to_string(&memory_path).await?;
            return Ok(Some(content));
        }
        Ok(None)
    }
}
