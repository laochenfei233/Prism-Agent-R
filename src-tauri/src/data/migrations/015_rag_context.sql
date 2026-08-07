-- RAG 扩展：上下文说明 + 引用 meta + 块类型
ALTER TABLE rag_chunks ADD COLUMN context TEXT;
ALTER TABLE rag_chunks ADD COLUMN page_start INTEGER;
ALTER TABLE rag_chunks ADD COLUMN page_end INTEGER;
ALTER TABLE rag_chunks ADD COLUMN section TEXT;
ALTER TABLE rag_chunks ADD COLUMN block_type TEXT NOT NULL DEFAULT 'text';
ALTER TABLE rag_chunks ADD COLUMN char_start INTEGER;
ALTER TABLE rag_chunks ADD COLUMN char_end INTEGER;
ALTER TABLE rag_chunks ADD COLUMN table_json TEXT;
ALTER TABLE rag_chunks ADD COLUMN caption TEXT;
CREATE INDEX IF NOT EXISTS idx_rag_chunks_page ON rag_chunks(wiki_id, page_start);

-- 评测用例
CREATE TABLE IF NOT EXISTS rag_eval_cases (
    id          TEXT PRIMARY KEY,
    wiki_id     TEXT NOT NULL,
    question    TEXT NOT NULL,
    expect      TEXT NOT NULL,
    suite       TEXT NOT NULL DEFAULT 'default',
    created_at  INTEGER NOT NULL
);
