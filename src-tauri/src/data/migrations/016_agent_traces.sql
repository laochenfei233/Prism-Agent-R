-- Agent 执行轨迹
CREATE TABLE IF NOT EXISTS agent_traces (
    id           TEXT PRIMARY KEY,
    session_id   TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    agent_id     TEXT NOT NULL,
    trace_id     TEXT NOT NULL,
    started_at   INTEGER NOT NULL,
    finished_at  INTEGER,
    steps        TEXT NOT NULL DEFAULT '[]',
    total_tokens TEXT NOT NULL DEFAULT '{}',
    total_cost   REAL NOT NULL DEFAULT 0,
    outcome      TEXT NOT NULL,
    created_at   INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_agent_traces_session ON agent_traces(session_id, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_agent_traces_agent ON agent_traces(agent_id, started_at DESC);
