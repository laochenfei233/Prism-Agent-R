-- Provider 配置
CREATE TABLE IF NOT EXISTS providers (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    kind        TEXT NOT NULL,                -- openai|anthropic|google|mimo|dashscope|ollama|custom
    base_url    TEXT,
    api_key_enc TEXT,                         -- AES-GCM 加密后密文
    is_enabled  INTEGER NOT NULL DEFAULT 1,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

-- 模型注册表
CREATE TABLE IF NOT EXISTS models (
    id           TEXT PRIMARY KEY,
    provider_id  TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    model_id     TEXT NOT NULL,               -- 供应商侧模型 ID
    display_name TEXT,
    kind         TEXT NOT NULL DEFAULT 'chat',-- chat|embedding|vision|asr
    max_tokens   INTEGER DEFAULT 8192,
    is_default   INTEGER NOT NULL DEFAULT 0,
    created_at   INTEGER NOT NULL,
    UNIQUE (provider_id, model_id)
);

-- Agent 定义
CREATE TABLE IF NOT EXISTS agents (
    id             TEXT PRIMARY KEY,
    name           TEXT NOT NULL,
    description    TEXT,
    avatar         TEXT,
    system_prompt  TEXT,
    model_id       TEXT REFERENCES models(id),
    plan_model_id  TEXT REFERENCES models(id),
    small_model_id TEXT REFERENCES models(id),
    temperature    REAL DEFAULT 0.7,
    max_tokens     INTEGER DEFAULT 8192,
    disabled_tools TEXT NOT NULL DEFAULT '[]',   -- JSON 数组
    configuration  TEXT NOT NULL DEFAULT '{}',   -- JSON（知识库绑定等）
    order_key      INTEGER NOT NULL DEFAULT 0,
    created_at     INTEGER NOT NULL,
    updated_at     INTEGER NOT NULL
);

-- Agent × MCP 关联
CREATE TABLE IF NOT EXISTS agent_mcp_servers (
    agent_id      TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    mcp_server_id TEXT NOT NULL REFERENCES mcp_servers(id) ON DELETE CASCADE,
    created_at    INTEGER NOT NULL,
    PRIMARY KEY (agent_id, mcp_server_id)
);

-- Agent × 技能关联
CREATE TABLE IF NOT EXISTS agent_skills (
    agent_id   TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    skill_id   TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
    is_enabled INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (agent_id, skill_id)
);

-- 会话
CREATE TABLE IF NOT EXISTS sessions (
    id         TEXT PRIMARY KEY,
    agent_id   TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    title      TEXT,
    pinned     INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_sessions_agent ON sessions(agent_id, updated_at DESC);

-- 消息
CREATE TABLE IF NOT EXISTS messages (
    id           TEXT PRIMARY KEY,
    session_id   TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    role         TEXT NOT NULL,                -- system|user|assistant|tool
    content      TEXT NOT NULL,                -- 文本（assistant 可为空，仅 tool_calls）
    tool_calls   TEXT,                         -- JSON: [{id,name,arguments}]
    tool_call_id TEXT,                         -- 关联 tool 角色消息
    model_id     TEXT,
    usage        TEXT,                         -- JSON: {prompt_tokens,completion_tokens}
    created_at   INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id, created_at);

-- 技能元数据
CREATE TABLE IF NOT EXISTS skills (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    description  TEXT,
    folder_name  TEXT NOT NULL UNIQUE,         -- 磁盘目录名
    source       TEXT NOT NULL,                -- builtin|marketplace|local|zip
    source_url   TEXT,
    namespace    TEXT,
    author       TEXT,
    tags         TEXT NOT NULL DEFAULT '[]',
    content_hash TEXT NOT NULL,
    is_enabled   INTEGER NOT NULL DEFAULT 0,
    created_at   INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL
);

-- MCP 服务器
CREATE TABLE IF NOT EXISTS mcp_servers (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    type       TEXT NOT NULL,                  -- stdio|sse|http|inmemory
    command    TEXT,
    args       TEXT NOT NULL DEFAULT '[]',
    env        TEXT NOT NULL DEFAULT '{}',
    base_url   TEXT,
    headers    TEXT NOT NULL DEFAULT '{}',
    is_active  INTEGER NOT NULL DEFAULT 1,
    timeout_ms INTEGER DEFAULT 30000,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
