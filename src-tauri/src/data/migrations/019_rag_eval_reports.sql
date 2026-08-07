-- §10.2.5 评测报告落库（rag:eval-report 趋势数据源）
CREATE TABLE IF NOT EXISTS rag_eval_reports (
    id         TEXT PRIMARY KEY,
    suite      TEXT NOT NULL DEFAULT 'default',
    case_count INTEGER NOT NULL DEFAULT 0,
    metrics    TEXT NOT NULL DEFAULT '{}',   -- JSON: {recall_at_k, page_acc, table_acc, ocr_completeness, chart_acc}
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_rag_eval_reports_suite ON rag_eval_reports(suite, created_at DESC);
