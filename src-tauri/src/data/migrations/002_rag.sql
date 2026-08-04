-- Wiki 知识库
CREATE TABLE IF NOT EXISTS wikis (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT,
    schema      TEXT,                          -- SCHEMA.md 内容
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

-- RAG 文档
CREATE TABLE IF NOT EXISTS rag_documents (
    id          TEXT PRIMARY KEY,
    wiki_id     TEXT NOT NULL REFERENCES wikis(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    mime_type   TEXT NOT NULL,
    size        INTEGER NOT NULL DEFAULT 0,
    chunk_count INTEGER NOT NULL DEFAULT 0,
    status      TEXT NOT NULL DEFAULT 'pending', -- pending|chunking|embedding|ready|error
    error_msg   TEXT,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_rag_docs_wiki ON rag_documents(wiki_id);

-- RAG 分块
CREATE TABLE IF NOT EXISTS rag_chunks (
    id          TEXT PRIMARY KEY,
    document_id TEXT NOT NULL REFERENCES rag_documents(id) ON DELETE CASCADE,
    wiki_id     TEXT NOT NULL REFERENCES wikis(id) ON DELETE CASCADE,
    "index"     INTEGER NOT NULL,
    content     TEXT NOT NULL,
    embedding   BLOB,                          -- f32 小端打包
    created_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_rag_chunks_doc ON rag_chunks(document_id);
CREATE INDEX IF NOT EXISTS idx_rag_chunks_wiki ON rag_chunks(wiki_id);
