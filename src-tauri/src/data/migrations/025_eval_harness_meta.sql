-- 025_eval_harness_meta.sql
-- §19.3.7 评测 harness 元数据 + §19.3.8 用例审计

-- rag_eval_reports 增 harness_meta
ALTER TABLE rag_eval_reports ADD COLUMN harness_meta TEXT;

-- rag_eval_cases 增 audit_verdict
ALTER TABLE rag_eval_cases ADD COLUMN audit_verdict TEXT;
