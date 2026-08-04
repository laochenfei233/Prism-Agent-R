-- 翻译历史
CREATE TABLE IF NOT EXISTS translate_history (
    id           TEXT PRIMARY KEY,
    source_text  TEXT NOT NULL,
    source_lang  TEXT NOT NULL,
    target_lang  TEXT NOT NULL,
    translated   TEXT NOT NULL,
    created_at   INTEGER NOT NULL
);

-- 翻译术语表
CREATE TABLE IF NOT EXISTS glossary_terms (
    id           TEXT PRIMARY KEY,
    source_lang  TEXT NOT NULL,
    target_lang  TEXT NOT NULL,
    source_term  TEXT NOT NULL,
    target_term  TEXT NOT NULL,
    category     TEXT,
    enabled      INTEGER NOT NULL DEFAULT 1,
    created_at   INTEGER NOT NULL,
    UNIQUE (source_lang, target_lang, source_term)
);

-- 记忆全文索引（FTS5）
CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(
    body, fingerprint, scope UNINDEXED, type UNINDEXED, path UNINDEXED
);

-- 键值偏好设置
CREATE TABLE IF NOT EXISTS preferences (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);
