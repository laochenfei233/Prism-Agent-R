-- 023_agent_exceptions.sql
-- Phase 5: 异常记录表

CREATE TABLE IF NOT EXISTS agent_exceptions (
    id            TEXT PRIMARY KEY,
    session_id    TEXT NOT NULL,
    agent_id      TEXT NOT NULL,
    workflow_id   TEXT,
    run_id        TEXT,
    stage_id      TEXT,
    exception_type TEXT NOT NULL,
    severity      TEXT NOT NULL,
    message       TEXT NOT NULL,
    context       TEXT,
    tool_name     TEXT,
    model_id      TEXT,
    tokens_used   INTEGER,
    cost_used     REAL,
    created_at    INTEGER NOT NULL,
    resolved_at   INTEGER,
    resolved_by   TEXT,
    resolution    TEXT
);

CREATE INDEX IF NOT EXISTS idx_exceptions_session ON agent_exceptions(session_id);
CREATE INDEX IF NOT EXISTS idx_exceptions_agent ON agent_exceptions(agent_id);
CREATE INDEX IF NOT EXISTS idx_exceptions_type ON agent_exceptions(exception_type);
CREATE INDEX IF NOT EXISTS idx_exceptions_severity ON agent_exceptions(severity);
