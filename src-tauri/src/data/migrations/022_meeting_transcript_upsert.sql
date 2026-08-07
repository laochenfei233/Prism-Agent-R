-- §10.3.4 修复：转写片段按 (meeting_id, "index") 幂等落库
-- 旧实现每次生成新 UUID 作主键导致同 index 重复插入；先清理既有重复（保留每组
-- created_at 最大、平局 rowid 最大的行），再建唯一索引支撑 ON CONFLICT upsert。
DELETE FROM meeting_transcripts
WHERE rowid IN (
    SELECT t.rowid FROM meeting_transcripts t
    WHERE EXISTS (
        SELECT 1 FROM meeting_transcripts m
        WHERE m.meeting_id = t.meeting_id AND m."index" = t."index"
          AND (m.created_at > t.created_at OR (m.created_at = t.created_at AND m.rowid > t.rowid))
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_meeting_transcripts_meeting_index
    ON meeting_transcripts(meeting_id, "index");
