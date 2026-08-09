// preferences 键值表类型化读写辅助
//
// 统一入口：读时带默认值回退（无记录 → 返回默认），写时 INSERT OR REPLACE。
// 各服务（workspace/rag/tts/translate/project_index 等）逐步迁移到本模块，
// 消除重复的 SELECT/INSERT SQL。

use sqlx::SqlitePool;

use crate::utils::error::AppError;

pub async fn get_str(pool: &SqlitePool, key: &str, default: &str) -> String {
    sqlx::query_scalar::<_, String>("SELECT value FROM preferences WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| default.to_string())
}

pub async fn get_bool(pool: &SqlitePool, key: &str, default: bool) -> bool {
    sqlx::query_scalar::<_, String>("SELECT value FROM preferences WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .and_then(|v| match v.as_str() {
            "true" | "1" => Some(true),
            "false" | "0" => Some(false),
            _ => None,
        })
        .unwrap_or(default)
}

pub async fn get_i64(pool: &SqlitePool, key: &str, default: i64) -> i64 {
    sqlx::query_scalar::<_, String>("SELECT value FROM preferences WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(default)
}

pub async fn get_f64(pool: &SqlitePool, key: &str, default: f64) -> f64 {
    sqlx::query_scalar::<_, String>("SELECT value FROM preferences WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(default)
}

/// 写入（INSERT OR REPLACE），value 为字符串形式
pub async fn set(pool: &SqlitePool, key: &str, value: &str) -> Result<(), AppError> {
    sqlx::query("INSERT OR REPLACE INTO preferences (key, value, updated_at) VALUES (?, ?, ?)")
        .bind(key)
        .bind(value)
        .bind(chrono::Utc::now().timestamp_millis())
        .execute(pool)
        .await?;
    Ok(())
}

/// 删除（回退默认值）
pub async fn remove(pool: &SqlitePool, key: &str) -> Result<(), AppError> {
    sqlx::query("DELETE FROM preferences WHERE key = ?")
        .bind(key)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::db::Database;

    fn temp_db() -> Database {
        let dir = std::env::temp_dir().join(format!("prism-prefs-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(Database::new(&dir))
            .expect("init db");
        // 目录保留（系统临时目录），避免连接池尚未建立时删除导致 unable to open
        db
    }

    #[test]
    fn defaults_when_missing() {
        let db = temp_db();
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            assert_eq!(get_str(&db.pool, "nonexistent", "d").await, "d");
            assert_eq!(get_bool(&db.pool, "nonexistent", true).await, true);
            assert_eq!(get_i64(&db.pool, "nonexistent", 42).await, 42);
            assert_eq!(get_f64(&db.pool, "nonexistent", 1.5).await, 1.5);
        });
    }

    #[test]
    fn roundtrip() {
        let db = temp_db();
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            set(&db.pool, "rag.chunk_size", "1200").await.unwrap();
            assert_eq!(get_i64(&db.pool, "rag.chunk_size", 1000).await, 1200);
            set(&db.pool, "reflection.enabled", "true").await.unwrap();
            assert_eq!(get_bool(&db.pool, "reflection.enabled", false).await, true);
            remove(&db.pool, "rag.chunk_size").await.unwrap();
            assert_eq!(get_i64(&db.pool, "rag.chunk_size", 1000).await, 1000);
        });
    }
}
