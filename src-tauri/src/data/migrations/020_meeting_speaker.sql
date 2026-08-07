-- §10.3.1 说话人分离：转写片段 speaker_id（DashScope speaker_diarization_enabled 已开启）
ALTER TABLE meeting_transcripts ADD COLUMN speaker_id INTEGER;
