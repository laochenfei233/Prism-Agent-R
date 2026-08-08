-- 027_orchestrator_sessions.sql
-- Phase 5 §27.2: 自主编排会话持久化（崩溃可恢复）

CREATE TABLE IF NOT EXISTS orchestrator_sessions (
    id            TEXT PRIMARY KEY,
    user_request  TEXT NOT NULL,
    spec          TEXT,
    plan          TEXT,
    status        TEXT NOT NULL,
    cycle_count   INTEGER NOT NULL DEFAULT 0,
    max_cycles    INTEGER NOT NULL DEFAULT 5,
    history       TEXT,
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_orchestrator_status ON orchestrator_sessions(status);
CREATE INDEX IF NOT EXISTS idx_orchestrator_updated ON orchestrator_sessions(updated_at);
