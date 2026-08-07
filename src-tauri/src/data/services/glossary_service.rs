use chrono::Utc;
use sqlx::SqlitePool;

use crate::data::models::{
    GlossaryTerm, GlossaryTermInput, GlossaryTermRow, ImportResult,
};
use crate::utils::error::AppError;

pub struct GlossaryService {
    pub pool: SqlitePool,
}

impl GlossaryService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(&self, lang_pair: Option<&str>) -> Result<Vec<GlossaryTerm>, AppError> {
        let rows = if let Some(lp) = lang_pair {
            let parts: Vec<&str> = lp.splitn(2, '-').collect();
            if parts.len() == 2 {
                sqlx::query_as::<_, GlossaryTermRow>(
                    "SELECT id, source_lang, target_lang, source_term, target_term, category, enabled, created_at
                     FROM glossary_terms WHERE source_lang = ? AND target_lang = ? ORDER BY source_term",
                )
                .bind(parts[0])
                .bind(parts[1])
                .fetch_all(&self.pool)
                .await?
            } else {
                sqlx::query_as::<_, GlossaryTermRow>(
                    "SELECT id, source_lang, target_lang, source_term, target_term, category, enabled, created_at
                     FROM glossary_terms ORDER BY source_term",
                )
                .fetch_all(&self.pool)
                .await?
            }
        } else {
            sqlx::query_as::<_, GlossaryTermRow>(
                "SELECT id, source_lang, target_lang, source_term, target_term, category, enabled, created_at
                 FROM glossary_terms ORDER BY source_term",
            )
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows.into_iter().map(|r| GlossaryTerm {
            id: r.id,
            source_lang: r.source_lang,
            target_lang: r.target_lang,
            source_term: r.source_term,
            target_term: r.target_term,
            category: r.category,
            enabled: r.enabled != 0,
        }).collect())
    }

    pub async fn add(&self, term: GlossaryTermInput) -> Result<(), AppError> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();
        sqlx::query(
            "INSERT OR REPLACE INTO glossary_terms (id, source_lang, target_lang, source_term, target_term, category, enabled, created_at)
             VALUES (?, ?, ?, ?, ?, ?, 1, ?)",
        )
        .bind(&id)
        .bind(&term.source_lang)
        .bind(&term.target_lang)
        .bind(&term.source_term)
        .bind(&term.target_term)
        .bind(&term.category)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn remove(&self, id: &str) -> Result<(), AppError> {
        sqlx::query("DELETE FROM glossary_terms WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn import_csv(&self, content: &str) -> Result<ImportResult, AppError> {
        let mut imported = 0usize;
        let mut failed = 0usize;
        let now = Utc::now().timestamp();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.splitn(5, ',').collect();
            if parts.len() < 4 {
                failed += 1;
                continue;
            }

            let id = uuid::Uuid::new_v4().to_string();
            let category = if parts.len() >= 5 {
                Some(parts[4].trim().to_string())
            } else {
                None
            };

            let result = sqlx::query(
                "INSERT OR REPLACE INTO glossary_terms (id, source_lang, target_lang, source_term, target_term, category, enabled, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, 1, ?)",
            )
            .bind(&id)
            .bind(parts[0].trim())
            .bind(parts[1].trim())
            .bind(parts[2].trim())
            .bind(parts[3].trim())
            .bind(&category)
            .bind(now)
            .execute(&self.pool)
            .await;

            match result {
                Ok(_) => imported += 1,
                Err(_) => failed += 1,
            }
        }

        Ok(ImportResult { imported, failed })
    }
}
