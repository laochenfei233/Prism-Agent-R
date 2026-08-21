use serde::Serialize;
use tauri::State;

use crate::utils::crypto::{decrypt_key, encrypt_key};
use crate::utils::error::AppError;

/// 搜索配置结果
#[derive(Serialize)]
pub struct SearchConfigResult {
    pub provider: String,
    pub api_key_set: bool,
    pub searxng_url: Option<String>,
    pub fallback_provider: Option<String>,
}

/// 读取搜索配置
#[tauri::command]
pub async fn search_config(
    state: State<'_, crate::AppState>,
) -> Result<SearchConfigResult, AppError> {
    let pool = &state.db.pool;

    let provider: String =
        sqlx::query_scalar("SELECT value FROM preferences WHERE key = 'search.provider'")
            .fetch_optional(pool)
            .await?
            .unwrap_or_else(|| "noop".to_string());

    let api_key_enc: Option<String> =
        sqlx::query_scalar("SELECT value FROM preferences WHERE key = 'search.api_key'")
            .fetch_optional(pool)
            .await?;

    let searxng_url: Option<String> =
        sqlx::query_scalar("SELECT value FROM preferences WHERE key = 'search.searxng_url'")
            .fetch_optional(pool)
            .await?;

    let fallback: Option<String> =
        sqlx::query_scalar("SELECT value FROM preferences WHERE key = 'search.fallback_provider'")
            .fetch_optional(pool)
            .await?;

    Ok(SearchConfigResult {
        provider,
        api_key_set: api_key_enc.map(|k| !k.is_empty()).unwrap_or(false),
        searxng_url,
        fallback_provider: fallback,
    })
}

/// 保存搜索配置
#[tauri::command]
pub async fn search_config_save(
    state: State<'_, crate::AppState>,
    provider: Option<String>,
    api_key: Option<String>,
    searxng_url: Option<String>,
    fallback_provider: Option<String>,
) -> Result<(), AppError> {
    let pool = &state.db.pool;
    let now = chrono::Utc::now().timestamp_millis();

    if let Some(p) = provider {
        sqlx::query("INSERT OR REPLACE INTO preferences (key, value, updated_at) VALUES ('search.provider', ?, ?)")
            .bind(&p).bind(now).execute(pool).await?;
    }

    if let Some(key) = api_key {
        let stored = if key.is_empty() {
            key
        } else {
            encrypt_key(&key)?
        };
        sqlx::query("INSERT OR REPLACE INTO preferences (key, value, updated_at) VALUES ('search.api_key', ?, ?)")
            .bind(&stored).bind(now).execute(pool).await?;
    }

    if let Some(url) = searxng_url {
        sqlx::query("INSERT OR REPLACE INTO preferences (key, value, updated_at) VALUES ('search.searxng_url', ?, ?)")
            .bind(&url).bind(now).execute(pool).await?;
    }

    if let Some(fb) = fallback_provider {
        if fb.is_empty() {
            sqlx::query("DELETE FROM preferences WHERE key = 'search.fallback_provider'")
                .execute(pool)
                .await?;
        } else {
            sqlx::query("INSERT OR REPLACE INTO preferences (key, value, updated_at) VALUES ('search.fallback_provider', ?, ?)")
                .bind(&fb).bind(now).execute(pool).await?;
        }
    }

    Ok(())
}

/// 搜索测试结果
#[derive(Serialize)]
pub struct SearchTestResult {
    pub success: bool,
    pub provider: String,
    pub first_result_title: Option<String>,
    pub first_result_url: Option<String>,
    pub elapsed_ms: u64,
    pub error: Option<String>,
}

/// 测试搜索连接
#[tauri::command]
pub async fn search_test(state: State<'_, crate::AppState>) -> Result<SearchTestResult, AppError> {
    use crate::core::search::service::SearchService;

    let pool = &state.db.pool;
    let config = load_search_config(pool).await?;
    let service = SearchService::from_config(&config);

    let provider_name = service.primary_provider().to_string();

    if !service.has_real_provider() {
        return Ok(SearchTestResult {
            success: false,
            provider: provider_name,
            first_result_title: None,
            first_result_url: None,
            elapsed_ms: 0,
            error: Some(NoopSearchProvider::hint().to_string()),
        });
    }

    let start = std::time::Instant::now();
    match service.search("prism agent rust", 1).await {
        Ok(hits) => {
            let elapsed = start.elapsed().as_millis() as u64;
            let first = hits.first();
            Ok(SearchTestResult {
                success: true,
                provider: provider_name,
                first_result_title: first.map(|h| h.title.clone()),
                first_result_url: first.map(|h| h.url.clone()),
                elapsed_ms: elapsed,
                error: None,
            })
        }
        Err(e) => Ok(SearchTestResult {
            success: false,
            provider: provider_name,
            first_result_title: None,
            first_result_url: None,
            elapsed_ms: start.elapsed().as_millis() as u64,
            error: Some(e),
        }),
    }
}

use crate::core::search::noop::NoopSearchProvider;

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
