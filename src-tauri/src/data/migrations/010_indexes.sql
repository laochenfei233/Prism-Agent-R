-- 关键索引（迁移 010）
CREATE INDEX IF NOT EXISTS idx_messages_id ON messages(id);
CREATE INDEX IF NOT EXISTS idx_sessions_pinned_updated ON sessions(pinned, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_workflow_runs_status ON workflow_runs(status);
CREATE INDEX IF NOT EXISTS idx_translate_lang_time ON translate_history(target_lang, created_at);
