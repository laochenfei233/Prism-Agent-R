-- 消息全文搜索（迁移 009）
CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
    content,
    session_id UNINDEXED,
    content='messages',
    content_rowid='rowid',
    tokenize='unicode61'
);

CREATE TRIGGER IF NOT EXISTS messages_ai AFTER INSERT ON messages BEGIN
    INSERT INTO messages_fts(rowid, content, session_id)
    VALUES (new.rowid, new.content, new.session_id);
END;

CREATE TRIGGER IF NOT EXISTS messages_ad AFTER DELETE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, content, session_id)
    VALUES ('delete', old.rowid, old.content, old.session_id);
END;

CREATE TRIGGER IF NOT EXISTS messages_au AFTER UPDATE OF content ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, content, session_id)
    VALUES ('delete', old.rowid, old.content, old.session_id);
    INSERT INTO messages_fts(rowid, content, session_id)
    VALUES (new.rowid, new.content, new.session_id);
END;

-- 回填
INSERT INTO messages_fts(rowid, content, session_id)
SELECT rowid, content, session_id FROM messages WHERE content IS NOT NULL AND content != '';
