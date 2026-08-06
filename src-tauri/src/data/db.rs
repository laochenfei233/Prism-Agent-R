use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::path::Path;

use crate::utils::error::AppError;

#[derive(Clone)]
pub struct Database {
    pub pool: SqlitePool,
}

impl Database {
    pub async fn new(app_data_dir: &Path) -> Result<Self, AppError> {
        tokio::fs::create_dir_all(app_data_dir).await?;

        let db_path = app_data_dir.join("prism.db");
        let url = format!("sqlite:{}?mode=rwc", db_path.display());

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await?;

        // Run migrations
        sqlx::query(include_str!("migrations/001_init.sql"))
            .execute(&pool)
            .await?;
        sqlx::query(include_str!("migrations/002_rag.sql"))
            .execute(&pool)
            .await?;
        sqlx::query(include_str!("migrations/003_meeting.sql"))
            .execute(&pool)
            .await?;
        sqlx::query(include_str!("migrations/004_workflow.sql"))
            .execute(&pool)
            .await?;
        sqlx::query(include_str!("migrations/005_glossary_memory.sql"))
            .execute(&pool)
            .await?;
        sqlx::query(include_str!("migrations/012_session_fts.sql"))
            .execute(&pool)
            .await?;

        Ok(Self { pool })
    }
}
