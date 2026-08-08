-- 024_trace_grading.sql
-- §17.3 Trace Grading 回写：agent_traces 增 grade_score/grade_reason/graded_at

ALTER TABLE agent_traces ADD COLUMN grade_score REAL;
ALTER TABLE agent_traces ADD COLUMN grade_reason TEXT;
ALTER TABLE agent_traces ADD COLUMN graded_at INTEGER;

CREATE INDEX IF NOT EXISTS idx_traces_grade ON agent_traces(agent_id, grade_score);
