-- 031_approval_governance.sql
-- Agent 审批治理：持久化审批请求与作用域化授权规则

CREATE TABLE IF NOT EXISTS agent_approval_requests (
    id                  TEXT PRIMARY KEY,
    kind                TEXT NOT NULL,
    status              TEXT NOT NULL,
    session_id          TEXT,
    parent_session_id   TEXT,
    agent_id            TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    parent_agent_id     TEXT,
    child_agent_id      TEXT,
    tool_name           TEXT,
    arguments           TEXT,
    task_summary        TEXT,
    capability_summary  TEXT,
    risk_level          TEXT NOT NULL,
    reason              TEXT,
    decision            TEXT,
    decision_reason     TEXT,
    decided_by          TEXT,
    decided_at          INTEGER,
    expires_at          INTEGER,
    created_at          INTEGER NOT NULL,
    updated_at          INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_approval_requests_agent
    ON agent_approval_requests(agent_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_approval_requests_status
    ON agent_approval_requests(status, created_at);

CREATE TABLE IF NOT EXISTS agent_approval_rules (
    id                     TEXT PRIMARY KEY,
    agent_id               TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    tool_name              TEXT NOT NULL,
    normalized_args_hash   TEXT NOT NULL,
    scope_hash             TEXT NOT NULL,
    max_uses               INTEGER,
    use_count              INTEGER NOT NULL DEFAULT 0,
    expires_at             INTEGER,
    created_at             INTEGER NOT NULL,
    updated_at             INTEGER NOT NULL,
    UNIQUE(agent_id, tool_name, normalized_args_hash)
);

CREATE INDEX IF NOT EXISTS idx_approval_rules_scope
    ON agent_approval_rules(agent_id, tool_name, normalized_args_hash);
