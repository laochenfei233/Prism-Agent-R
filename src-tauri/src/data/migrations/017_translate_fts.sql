-- 翻译历史全文搜索
CREATE VIRTUAL TABLE IF NOT EXISTS translate_fts USING fts5(
    source_text,
    translated,
    source_lang UNINDEXED,
    target_lang UNINDEXED,
    content='translate_history',
    content_rowid='rowid',
    tokenize='unicode61'
);

-- 同步触发器
CREATE TRIGGER IF NOT EXISTS translate_ai AFTER INSERT ON translate_history BEGIN
    INSERT INTO translate_fts(rowid, source_text, translated, source_lang, target_lang)
    VALUES (new.rowid, new.source_text, new.translated, new.source_lang, new.target_lang);
END;

CREATE TRIGGER IF NOT EXISTS translate_ad AFTER DELETE ON translate_history BEGIN
    INSERT INTO translate_fts(translate_fts, rowid, source_text, translated, source_lang, target_lang)
    VALUES ('delete', old.rowid, old.source_text, old.translated, old.source_lang, old.target_lang);
END;

CREATE TRIGGER IF NOT EXISTS translate_au AFTER UPDATE ON translate_history BEGIN
    INSERT INTO translate_fts(translate_fts, rowid, source_text, translated, source_lang, target_lang)
    VALUES ('delete', old.rowid, old.source_text, old.translated, old.source_lang, old.target_lang);
    INSERT INTO translate_fts(rowid, source_text, translated, source_lang, target_lang)
    VALUES (new.rowid, new.source_text, new.translated, new.source_lang, new.target_lang);
END;
