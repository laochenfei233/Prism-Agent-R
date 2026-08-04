-- 工作流定义
CREATE TABLE IF NOT EXISTS workflows (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT,
    definition  TEXT NOT NULL,                 -- JSON: {stages:[...]}
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

-- 工作流运行
CREATE TABLE IF NOT EXISTS workflow_runs (
    id          TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    status      TEXT NOT NULL DEFAULT 'running', -- running|done|failed|cancelled
    inputs      TEXT NOT NULL DEFAULT '{}',
    outputs     TEXT,
    error       TEXT,
    created_at  INTEGER NOT NULL,
    finished_at INTEGER
);

-- 阶段模板
CREATE TABLE IF NOT EXISTS stage_templates (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    role          TEXT NOT NULL,
    description   TEXT,
    prompt_template TEXT NOT NULL,
    tools         TEXT NOT NULL DEFAULT '[]',
    max_iterations INTEGER DEFAULT 10,
    source        TEXT NOT NULL DEFAULT 'user',   -- builtin | user
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL
);
