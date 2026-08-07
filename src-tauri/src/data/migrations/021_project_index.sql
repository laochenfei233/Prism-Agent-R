-- §10.2.1 项目级自动索引：隔离命名空间 + 文件指纹（path+mtime+size）
INSERT OR IGNORE INTO wikis (id, name, description, created_at, updated_at)
VALUES ('__project__', '项目索引', '工作目录自动索引（§10.2.1），隔离命名空间，不污染用户 Wiki', 0, 0);

-- rag_documents 扩展：file_path（相对工作目录）+ fingerprint（mtime_nanos:size）
ALTER TABLE rag_documents ADD COLUMN file_path TEXT;
ALTER TABLE rag_documents ADD COLUMN fingerprint TEXT;
CREATE INDEX IF NOT EXISTS idx_rag_docs_proj_path ON rag_documents(wiki_id, file_path);
