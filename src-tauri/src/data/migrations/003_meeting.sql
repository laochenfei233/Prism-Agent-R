-- 会议
CREATE TABLE IF NOT EXISTS meetings (
    id                 TEXT PRIMARY KEY,
    title              TEXT NOT NULL,
    date               TEXT NOT NULL,
    transcript         TEXT NOT NULL DEFAULT '',
    summary            TEXT NOT NULL DEFAULT '',
    participants       TEXT NOT NULL DEFAULT '[]',
    recording_duration INTEGER NOT NULL DEFAULT 0,
    audio_path         TEXT,
    folder_path        TEXT,
    source_lang        TEXT,
    target_lang        TEXT,
    asr_kind           TEXT,
    asr_model          TEXT,
    retranscribed_at   INTEGER,
    created_at         INTEGER NOT NULL,
    updated_at         INTEGER NOT NULL
);

-- 会议转写片段
CREATE TABLE IF NOT EXISTS meeting_transcripts (
    id         TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    "index"    INTEGER NOT NULL,
    text       TEXT NOT NULL,
    is_final   INTEGER NOT NULL DEFAULT 0,
    translated TEXT,
    created_at INTEGER NOT NULL
);

-- ASR 后端配置
CREATE TABLE IF NOT EXISTS asr_configs (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    kind        TEXT NOT NULL,
    base_url    TEXT,
    api_key_enc TEXT,
    model       TEXT,
    lang        TEXT,
    is_default  INTEGER NOT NULL DEFAULT 0,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);
