-- 023_web_search_cache.sql
-- 网络搜索结果缓存（1 小时有效，避免重复计费与延迟）

CREATE TABLE IF NOT EXISTS web_search_cache (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    query_hash    TEXT NOT NULL,
    provider      TEXT NOT NULL,
    query         TEXT NOT NULL,
    results_json  TEXT NOT NULL,
    created_at    INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_search_cache_hash ON web_search_cache(query_hash);
