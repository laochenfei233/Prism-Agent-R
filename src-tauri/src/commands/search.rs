use crate::utils::crypto::decrypt_key;
use crate::utils::error::AppError;

/// 从 preferences 表加载搜索配置到 HashMap
async fn load_search_config(
    pool: &sqlx::SqlitePool,
) -> Result<std::collections::HashMap<String, String>, AppError> {
    let mut config = std::collections::HashMap::new();

    let keys = [
        "search.provider",
        "search.api_key",
        "search.searxng_url",
        "search.fallback_provider",
    ];

    for key in &keys {
        if let Some(val) =
            sqlx::query_scalar::<_, String>("SELECT value FROM preferences WHERE key = ?")
                .bind(key)
                .fetch_optional(pool)
                .await?
        {
            // api_key 需要解密
            let value = if *key == "search.api_key" {
                decrypt_key(&val).unwrap_or(val)
            } else {
                val
            };
            config.insert(key.to_string(), value);
        }
    }

    Ok(config)
}

/// 获取搜索配置的 HashMap（供 SearchService 使用）
pub async fn get_search_config(
    pool: &sqlx::SqlitePool,
) -> std::collections::HashMap<String, String> {
    load_search_config(pool).await.unwrap_or_default()
}
