-- 会话标题 FTS（轻量级，标题短文本）— 迁移 012
CREATE VIRTUAL TABLE IF NOT EXISTS sessions_fts USING fts5(
    title,
    session_id UNINDEXED,
    content='sessions',
    content_rowid='rowid',
    tokenize='unicode61'
);

-- 同步触发器：插入时同步索引
CREATE TRIGGER IF NOT EXISTS sessions_ai AFTER INSERT ON sessions BEGIN
    INSERT INTO sessions_fts(rowid, title, session_id)
    VALUES (new.rowid, new.title, new.id);
END;

-- 同步触发器：标题更新时同步索引
CREATE TRIGGER IF NOT EXISTS sessions_au AFTER UPDATE OF title ON sessions BEGIN
    INSERT INTO sessions_fts(sessions_fts, rowid, title, session_id)
    VALUES ('delete', old.rowid, old.title, old.id);
    INSERT INTO sessions_fts(rowid, title, session_id)
    VALUES (new.rowid, new.title, new.id);
END;

-- 回填现有数据
INSERT INTO sessions_fts(rowid, title, session_id)
SELECT rowid, title, id FROM sessions WHERE title IS NOT NULL AND title != '';
