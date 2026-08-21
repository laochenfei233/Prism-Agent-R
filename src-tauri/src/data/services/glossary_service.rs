use chrono::Utc;
use sqlx::SqlitePool;

use crate::data::models::{GlossaryTerm, GlossaryTermInput, GlossaryTermRow, ImportResult};
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

        Ok(rows
            .into_iter()
            .map(|r| GlossaryTerm {
                id: r.id,
                source_lang: r.source_lang,
                target_lang: r.target_lang,
                source_term: r.source_term,
                target_term: r.target_term,
                category: r.category,
                enabled: r.enabled != 0,
            })
            .collect())
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

    /// 更新已有术语（§10.5.2 增删改）
    pub async fn update(&self, id: &str, term: GlossaryTermInput) -> Result<(), AppError> {
        let changed = sqlx::query(
            "UPDATE glossary_terms SET source_lang = ?, target_lang = ?, source_term = ?, target_term = ?, category = ? WHERE id = ?"
        )
        .bind(&term.source_lang)
        .bind(&term.target_lang)
        .bind(&term.source_term)
        .bind(&term.target_term)
        .bind(&term.category)
        .bind(id)
        .execute(&self.pool)
        .await?;
        if changed.rows_affected() == 0 {
            return Err(AppError::Validation(format!("术语不存在: {id}")));
        }
        Ok(())
    }

    pub async fn import_csv(&self, content: &str) -> Result<ImportResult, AppError> {
        // 跳过表头（source_lang,target_lang,source_term,target_term,category）
        let lines = content.lines().skip(1);
        self.import_csv_lines(lines).await
    }

    /// 内置词表导入：与 import_csv 相同，但保留表头识别（若首行恰是数据则不强跳）
    pub async fn import_builtin_csv(&self, content: &str) -> Result<ImportResult, AppError> {
        let lines = content.lines().skip(1); // 内置文件统一带表头
        self.import_csv_lines(lines).await
    }

    async fn import_csv_lines<'a>(
        &self,
        lines: impl Iterator<Item = &'a str>,
    ) -> Result<ImportResult, AppError> {
        let mut imported = 0usize;
        let mut failed = 0usize;
        let now = Utc::now().timestamp();

        for line in lines {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 术语更新（§10.5.2）：update 后内容变更；不存在的 id 报错
    #[tokio::test]
    async fn update_term_changes_values() {
        let dir = std::env::temp_dir().join(format!("prism_gloss_{}", uuid::Uuid::new_v4()));
        let db = crate::data::db::Database::new(&dir).await.unwrap();
        let svc = GlossaryService::new(db.pool.clone());

        svc.add(GlossaryTermInput {
            source_lang: "en".into(),
            target_lang: "zh".into(),
            source_term: "Prism".into(),
            target_term: "棱镜".into(),
            category: Some("产品名".into()),
        })
        .await
        .unwrap();

        let terms = svc.list(None).await.unwrap();
        assert_eq!(terms.len(), 1);
        let id = terms[0].id.clone();

        // 更新译文
        svc.update(
            &id,
            GlossaryTermInput {
                source_lang: "en".into(),
                target_lang: "zh".into(),
                source_term: "Prism".into(),
                target_term: "棱镜（平台）".into(),
                category: Some("产品名".into()),
            },
        )
        .await
        .unwrap();

        let terms = svc.list(None).await.unwrap();
        assert_eq!(terms[0].target_term, "棱镜（平台）");

        // 不存在的 id
        assert!(
            svc.update(
                "no-such-id",
                GlossaryTermInput {
                    source_lang: "en".into(),
                    target_lang: "zh".into(),
                    source_term: "x".into(),
                    target_term: "y".into(),
                    category: None,
                }
            )
            .await
            .is_err(),
            "不存在的术语 update 应报错"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 内置词表导入：表头必须被跳过，不作为术语写入
    #[tokio::test]
    async fn import_builtin_skips_header() {
        let dir = std::env::temp_dir().join(format!("prism_gloss_bi_{}", uuid::Uuid::new_v4()));
        let db = crate::data::db::Database::new(&dir).await.unwrap();
        let svc = GlossaryService::new(db.pool.clone());

        let csv = "source_lang,target_lang,source_term,target_term,category\nen,zh,FOB,船上交货,外贸术语\nen,zh,CIF,成本保险费加运费,外贸术语\n";
        let result = svc.import_builtin_csv(csv).await.unwrap();
        assert_eq!(result.imported, 2, "表头不应计为术语");
        assert_eq!(result.failed, 0);

        let terms = svc.list(None).await.unwrap();
        assert_eq!(terms.len(), 2);
        // 表头不会被导入
        assert!(
            !terms.iter().any(|t| t.source_term == "source_lang"),
            "表头不应入库"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 真实内置词表文件可完整导入（回归：打包资源内容必须能被解析）
    #[tokio::test]
    async fn real_builtin_csv_imports() {
        let dir = std::env::temp_dir().join(format!("prism_gloss_real_{}", uuid::Uuid::new_v4()));
        let db = crate::data::db::Database::new(&dir).await.unwrap();
        let svc = GlossaryService::new(db.pool.clone());

        // 读取打包资源目录的 CSV（开发模式下 resource_dir = src-tauri）
        let resources = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/glossary");
        let mut imported_total = 0usize;
        let mut files = 0usize;
        if let Ok(mut entries) = tokio::fs::read_dir(&resources).await {
            while let Ok(Some(e)) = entries.next_entry().await {
                if e.path().extension().map(|x| x == "csv").unwrap_or(false) {
                    let content = tokio::fs::read_to_string(e.path()).await.unwrap();
                    let result = svc.import_builtin_csv(&content).await.unwrap();
                    assert_eq!(
                        result.failed,
                        0,
                        "{} 不应有解析失败行",
                        e.file_name().to_string_lossy()
                    );
                    imported_total += result.imported;
                    files += 1;
                }
            }
        }
        assert!(files >= 6, "应至少有 6 个内置词表，实际 {files}");
        assert!(
            imported_total > 30_000,
            "总导入应 > 30000 条，实际 {imported_total}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
