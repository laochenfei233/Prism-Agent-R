-- 030_agent_orchestrator.sql
-- Agent Orchestrator：agents 表新增 is_orchestrator 标记

ALTER TABLE agents ADD COLUMN is_orchestrator INTEGER NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_agents_orchestrator ON agents(is_orchestrator);
