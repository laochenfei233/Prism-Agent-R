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
            .after_connect(|conn, _meta| {
                Box::pin(async move {
                    sqlx::query("PRAGMA journal_mode = WAL").execute(&mut *conn).await?;
                    sqlx::query("PRAGMA synchronous = NORMAL").execute(&mut *conn).await?;
                    sqlx::query("PRAGMA foreign_keys = ON").execute(&mut *conn).await?;
                    sqlx::query("PRAGMA busy_timeout = 5000").execute(&mut *conn).await?;
                    sqlx::query("PRAGMA cache_size = -20000").execute(&mut *conn).await?;
                    sqlx::query("PRAGMA temp_store = MEMORY").execute(&mut *conn).await?;
                    Ok(())
                })
            })
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
        sqlx::query(include_str!("migrations/009_message_search.sql"))
            .execute(&pool)
            .await?;
        sqlx::query(include_str!("migrations/010_indexes.sql"))
            .execute(&pool)
            .await?;
        sqlx::query(include_str!("migrations/012_session_fts.sql"))
            .execute(&pool)
            .await?;

        Ok(Self { pool })
    }
}
