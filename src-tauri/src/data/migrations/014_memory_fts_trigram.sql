-- 重建 memory_fts 为 trigram 分词（中文检索优化，迁移 014）
DROP TABLE IF EXISTS memory_fts;

CREATE VIRTUAL TABLE memory_fts USING fts5(
    body,
    fingerprint,
    scope UNINDEXED,
    type UNINDEXED,
    path UNINDEXED,
    tokenize='trigram'
);
