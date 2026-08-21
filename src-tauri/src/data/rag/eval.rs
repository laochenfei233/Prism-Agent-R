use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::data::db::Database;
use crate::data::models::RagDocumentRow;
use crate::utils::error::AppError;

// ── 评测用例（§10.2.5） ───────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalCase {
    pub id: String,
    pub wiki_id: String,
    pub question: String,
    pub expect: EvalExpect,
    pub suite: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EvalExpect {
    #[serde(default)]
    pub chunk_ids: Vec<String>,
    #[serde(default)]
    pub pages: Vec<u32>,
    pub section: Option<String>,
    #[serde(default)]
    pub answer_keywords: Vec<String>,
    pub has_table: Option<bool>,
    /// 表格期望单元格（结构化比对，§10.2.5 table_acc）
    #[serde(default)]
    pub table_expected: Option<Vec<Vec<String>>>,
    /// 人工转录参考文本（OCR 无漏字评测，§10.2.5 ocr_completeness，字符召回率）
    pub ocr_reference: Option<String>,
    /// 图表期望语义描述（LLM-as-Judge 5 分制，§10.2.5 chart_acc）
    pub chart_expected: Option<String>,
}

// ── 五维评测结果 ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalReport {
    pub suite: String,
    pub case_count: usize,
    pub metrics: Metrics,
    pub cases: Vec<CaseResult>,
    pub created_at: i64,
}

/// 评测命中 chunk 的元数据（三维度评测的数据源）
#[derive(Debug, Clone)]
pub struct ChunkMeta {
    pub id: String,
    pub block_type: String,
    pub table_json: Option<String>,
    pub caption: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Metrics {
    /// 检索片段命中：期望 chunk_ids ∩ 检索 top-k / 期望总数
    pub recall_at_k: f32,
    /// 页码定位正确：回答引用页码与期望 pages 一致的比例
    pub page_acc: f32,
    /// 表格解析准确：table_json 与期望单元格的逐格匹配率（结构化比对）
    pub table_acc: f32,
    /// OCR 无漏字：OCR 文本与人工转录的字符召回率（编辑距离）
    pub ocr_completeness: f32,
    /// 图表正确理解：图注与期望语义一致性（LLM-as-Judge 5 分制 / 5）
    pub chart_acc: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseResult {
    pub id: String,
    pub question: String,
    pub passed: bool,
    pub hit_count: usize,
    pub detail: String,
}

// ── 评测集持久化 ──────────────────────────────────────────

/// 从 rag_eval_cases 表加载评测集
pub async fn load_cases(db: &Database, suite: Option<&str>) -> Result<Vec<EvalCase>, AppError> {
    let rows = if let Some(s) = suite {
        sqlx::query("SELECT id, wiki_id, question, expect, suite, created_at FROM rag_eval_cases WHERE suite = ? ORDER BY created_at")
            .bind(s)
            .fetch_all(&db.pool)
            .await?
    } else {
        sqlx::query("SELECT id, wiki_id, question, expect, suite, created_at FROM rag_eval_cases ORDER BY created_at")
            .fetch_all(&db.pool)
            .await?
    };
    let mut out = Vec::new();
    for row in rows {
        let id: String = row.try_get("id")?;
        let wiki_id: String = row.try_get("wiki_id")?;
        let question: String = row.try_get("question")?;
        let expect_json: String = row.try_get("expect")?;
        let suite: String = row.try_get("suite")?;
        let expect: EvalExpect = serde_json::from_str(&expect_json).unwrap_or_default();
        out.push(EvalCase {
            id,
            wiki_id,
            question,
            expect,
            suite,
        });
    }
    Ok(out)
}

/// 添加评测用例
pub async fn add_case(db: &Database, case: &EvalCase) -> Result<(), AppError> {
    let now = chrono::Utc::now().timestamp();
    let expect_json = serde_json::to_string(&case.expect)?;
    sqlx::query(
        "INSERT OR REPLACE INTO rag_eval_cases (id, wiki_id, question, expect, suite, created_at) VALUES (?, ?, ?, ?, ?, ?)"
    )
    .bind(&case.id)
    .bind(&case.wiki_id)
    .bind(&case.question)
    .bind(&expect_json)
    .bind(&case.suite)
    .bind(now)
    .execute(&db.pool)
    .await?;
    Ok(())
}

/// 保存一次评测报告（rag:eval 完成后落库，供趋势对比）
pub async fn save_report(db: &Database, report: &EvalReport) -> Result<(), AppError> {
    let id = uuid::Uuid::new_v4().to_string();
    let metrics = serde_json::to_string(&report.metrics)?;
    sqlx::query(
        "INSERT INTO rag_eval_reports (id, suite, case_count, metrics, created_at) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(&id)
    .bind(&report.suite)
    .bind(report.case_count as i64)
    .bind(&metrics)
    .bind(report.created_at)
    .execute(&db.pool)
    .await?;
    Ok(())
}

/// 历史评测报告（趋势对比）：按 suite 分组倒序，每组最近 50 次
pub async fn list_reports(db: &Database) -> Result<Vec<EvalReport>, AppError> {
    let rows = sqlx::query(
        "SELECT suite, case_count, metrics, created_at FROM rag_eval_reports ORDER BY created_at DESC LIMIT 200"
    )
    .fetch_all(&db.pool)
    .await?;

    let mut out = Vec::new();
    for row in rows {
        let suite: String = row.try_get("suite")?;
        let case_count: i64 = row.try_get("case_count")?;
        let metrics_json: String = row.try_get("metrics")?;
        let created_at: i64 = row.try_get("created_at")?;
        let metrics: Metrics = serde_json::from_str(&metrics_json).unwrap_or_default();
        out.push(EvalReport {
            suite,
            case_count: case_count as usize,
            metrics,
            cases: Vec::new(),
            created_at,
        });
    }
    Ok(out)
}

/// 拉取命中 chunk 的元数据（block_type/table_json/caption/content），三维度评测数据源
pub async fn fetch_chunk_meta(
    db: &Database,
    chunk_ids: &[String],
) -> Result<Vec<ChunkMeta>, AppError> {
    if chunk_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query(
        "SELECT id, block_type, table_json, caption, content FROM rag_chunks WHERE id IN (SELECT value FROM json_each(?1))"
    )
    .bind(serde_json::to_string(chunk_ids)?)
    .fetch_all(&db.pool)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(ChunkMeta {
            id: row.try_get("id")?,
            block_type: row.try_get("block_type")?,
            table_json: row.try_get("table_json")?,
            caption: row.try_get("caption")?,
            content: row.try_get("content")?,
        });
    }
    Ok(out)
}

// ── 三维度评测实现（§10.2.5） ─────────────────────────────

/// 运行评测核心（命令层 rag_eval 与 CI 门槛共用，§10.2.5）：
/// 检索类指标直接度量；table_acc 结构化比对；ocr_completeness 字符召回率；chart_acc LLM-as-Judge。
/// 返回报告（不含落库；落库由调用方 save_report）。
pub async fn run_eval(
    db: &Database,
    cases: Vec<EvalCase>,
    top_k: usize,
    suite: String,
) -> Result<EvalReport, AppError> {
    let now = chrono::Utc::now().timestamp();
    let mut report = EvalReport {
        suite,
        case_count: cases.len(),
        metrics: Default::default(),
        cases: Vec::new(),
        created_at: now,
    };
    if cases.is_empty() {
        return Ok(report);
    }

    // RagService 循环外复用（避免重复构建嵌入器）
    let mut svc = crate::data::services::rag_service::RagService::new(db.clone());
    svc.configure_from_db().await?;

    let mut hit_total = 0usize;
    let mut hit_denom = 0usize;
    let mut page_ok = 0usize;
    let mut page_denom = 0usize;
    let mut table_sum = 0.0f32;
    let mut table_denom = 0usize;
    let mut ocr_sum = 0.0f32;
    let mut ocr_denom = 0usize;
    let mut chart_sum = 0.0f32;
    let mut chart_denom = 0usize;
    let mut chart_model: Option<std::sync::Arc<dyn crate::core::adk::model::ModelProvider>> = None;

    for case in &cases {
        let hits = svc
            .search(&case.wiki_id, &case.question, top_k)
            .await
            .unwrap_or_default();
        let hit_ids: Vec<String> = hits.iter().map(|h| h.chunk_id.clone()).collect();
        let metas = fetch_chunk_meta(db, &hit_ids).await.unwrap_or_default();

        // recall@k
        let expected = &case.expect.chunk_ids;
        let matched = expected.iter().filter(|c| hit_ids.contains(c)).count();
        hit_denom += expected.len();
        hit_total += matched;

        // page_acc
        if !case.expect.pages.is_empty() {
            page_denom += 1;
            if hits.iter().any(|h| {
                h.page_start
                    .map(|p| case.expect.pages.contains(&(p as u32)))
                    .unwrap_or(false)
            }) {
                page_ok += 1;
            }
        }

        // table_acc：期望命中 table 块（has_table）或给出期望单元格时评测
        if case.expect.has_table == Some(true) || case.expect.table_expected.is_some() {
            table_denom += 1;
            let table_json = metas
                .iter()
                .find(|m| m.block_type == "table")
                .and_then(|m| m.table_json.as_deref());
            if let Some(rate) =
                table_cell_match_rate(table_json, case.expect.table_expected.as_deref())
            {
                table_sum += rate;
            }
        }

        // ocr_completeness：有参考转录文本时，对 top 命中 chunk 内容做字符召回率
        if let Some(ref_text) = &case.expect.ocr_reference {
            ocr_denom += 1;
            if let Some(first) = metas.first() {
                if let Some(rate) = ocr_char_recall(&first.content, ref_text) {
                    ocr_sum += rate;
                }
            }
        }

        // chart_acc：期望图注语义 → LLM-as-Judge 5 分制（无模型时维度记 0，降级不报错）
        if let Some(exp) = &case.expect.chart_expected {
            chart_denom += 1;
            let caption = metas
                .iter()
                .find(|m| m.block_type == "image")
                .and_then(|m| m.caption.as_deref());
            if let Some(cap) = caption {
                if chart_model.is_none() {
                    chart_model = crate::data::services::rag_service::resolve_rerank_model(db)
                        .await
                        .ok();
                }
                if let Some(model) = &chart_model {
                    let judge = crate::core::rig::judge::AgentJudge::new(model.clone());
                    let criteria = vec!["语义一致性".to_string()];
                    let task = format!("判断图表图注是否与期望语义一致。期望：{exp}");
                    if let Ok(res) = judge.evaluate(&task, cap, &criteria).await {
                        chart_sum += (res.score / 5.0).clamp(0.0, 1.0);
                    }
                }
            }
        }

        // 关键词覆盖
        let kw_miss: Vec<String> = case
            .expect
            .answer_keywords
            .iter()
            .filter(|kw| !hits.iter().any(|h| h.quote.contains(kw.as_str())))
            .cloned()
            .collect();
        let passed = (expected.is_empty() || matched > 0) && kw_miss.is_empty();

        let detail = if kw_miss.is_empty() {
            format!("recall {matched}/{}", expected.len())
        } else {
            format!("缺少关键词: {}", kw_miss.join(", "))
        };
        report.cases.push(CaseResult {
            id: case.id.clone(),
            question: case.question.clone(),
            passed,
            hit_count: matched,
            detail,
        });
    }

    report.metrics.recall_at_k = if hit_denom > 0 {
        hit_total as f32 / hit_denom as f32
    } else {
        0.0
    };
    report.metrics.page_acc = if page_denom > 0 {
        page_ok as f32 / page_denom as f32
    } else {
        0.0
    };
    report.metrics.table_acc = if table_denom > 0 {
        table_sum / table_denom as f32
    } else {
        0.0
    };
    report.metrics.ocr_completeness = if ocr_denom > 0 {
        ocr_sum / ocr_denom as f32
    } else {
        0.0
    };
    report.metrics.chart_acc = if chart_denom > 0 {
        chart_sum / chart_denom as f32
    } else {
        0.0
    };
    Ok(report)
}

/// 表格解析准确：期望单元格在 table_json 中的逐格匹配率（结构化比对，无 LLM 成本）。
/// 无期望单元格时退化为「命中 table 块」的布尔得分。
pub fn table_cell_match_rate(
    table_json: Option<&str>,
    expected: Option<&[Vec<String>]>,
) -> Option<f32> {
    let json = table_json?;
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let cells: Vec<String> = match &value {
        serde_json::Value::Array(arr) => arr
            .iter()
            .flat_map(|r| match r {
                serde_json::Value::Array(row) => row
                    .iter()
                    .filter_map(|c| c.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>(),
                _ => Vec::new(),
            })
            .collect(),
        serde_json::Value::Object(map) => map
            .values()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        _ => Vec::new(),
    };

    match expected {
        None => Some(if cells.is_empty() { 0.0 } else { 1.0 }),
        Some(rows) => {
            let expected_flat: Vec<&String> = rows.iter().flatten().collect();
            if expected_flat.is_empty() {
                return Some(if cells.is_empty() { 0.0 } else { 1.0 });
            }
            let matched = expected_flat
                .iter()
                .filter(|e| cells.iter().any(|c| c.contains(e.as_str())))
                .count();
            Some(matched as f32 / expected_flat.len() as f32)
        }
    }
}

/// OCR 无漏字：OCR 文本与人工转录的字符召回率（编辑距离）。
/// recall = (ref_len - edit_dist) / ref_len；无参考文本返回 None（维度不适用）。
pub fn ocr_char_recall(ocr_text: &str, reference: &str) -> Option<f32> {
    if reference.is_empty() {
        return None;
    }
    let dist = levenshtein_chars(ocr_text, reference);
    let ref_len = reference.chars().count() as f32;
    Some(((ref_len - dist as f32) / ref_len).clamp(0.0, 1.0))
}

/// 字符级编辑距离（中文场景按字符而非字节计）
pub fn levenshtein_chars(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

/// 校验 wiki 存在
pub async fn ensure_wiki(db: &Database, wiki_id: &str) -> Result<(), AppError> {
    let exists: Option<i64> = sqlx::query_scalar("SELECT COUNT(*) FROM wikis WHERE id = ?")
        .bind(wiki_id)
        .fetch_one(&db.pool)
        .await?;
    if exists.unwrap_or(0) == 0 {
        return Err(AppError::Validation(format!("知识库不存在: {wiki_id}")));
    }
    Ok(())
}

pub async fn list_docs(db: &Database, wiki_id: &str) -> Result<Vec<RagDocumentRow>, AppError> {
    crate::data::rag::store::list_documents(db, wiki_id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levenshtein_identical() {
        assert_eq!(levenshtein_chars("今天天气很好", "今天天气很好"), 0);
    }

    #[test]
    fn levenshtein_one_char() {
        assert_eq!(levenshtein_chars("今天天气很好", "今天天气很不好"), 1);
    }

    #[test]
    fn levenshtein_empty() {
        assert_eq!(levenshtein_chars("", ""), 0);
        assert_eq!(levenshtein_chars("abc", ""), 3);
    }

    #[test]
    fn ocr_recall_perfect() {
        let r = ocr_char_recall("今天天气很好", "今天天气很好").unwrap();
        assert!((r - 1.0).abs() < 1e-6);
    }

    #[test]
    fn ocr_recall_no_reference() {
        assert!(ocr_char_recall("任意文本", "").is_none());
    }

    #[test]
    fn table_match_exact_cells() {
        let json = r#"[["姓名","年龄"],["张三","30"]]"#;
        let expected = vec![vec!["张三".to_string(), "30".to_string()]];
        let rate = table_cell_match_rate(Some(json), Some(&expected)).unwrap();
        assert_eq!(rate, 1.0);
    }

    #[test]
    fn table_match_partial() {
        let json = r#"[["姓名","年龄"],["张三","30"]]"#;
        let expected = vec![vec!["张三".to_string(), "31".to_string()]];
        let rate = table_cell_match_rate(Some(json), Some(&expected)).unwrap();
        assert_eq!(rate, 0.5);
    }

    #[test]
    fn table_match_no_json() {
        assert!(table_cell_match_rate(None, None).is_none());
    }

    /// 集成：临时库跑全部迁移（含 019_rag_eval_reports）+ 报告落库/读取闭环
    #[tokio::test]
    async fn report_roundtrip_persists() {
        let dir = std::env::temp_dir().join(format!("prism_eval_rt_{}", uuid::Uuid::new_v4()));
        let db = crate::data::db::Database::new(&dir).await.unwrap();

        let report = EvalReport {
            suite: "default".into(),
            case_count: 2,
            metrics: Metrics {
                recall_at_k: 0.75,
                page_acc: 0.5,
                ..Default::default()
            },
            cases: Vec::new(),
            created_at: 1_720_000_000,
        };
        save_report(&db, &report).await.unwrap();

        let reports = list_reports(&db).await.unwrap();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].suite, "default");
        assert_eq!(reports[0].case_count, 2);
        assert!((reports[0].metrics.recall_at_k - 0.75).abs() < 1e-6);
        assert!((reports[0].metrics.page_acc - 0.5).abs() < 1e-6);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 集成：json_each 运行时可用性 + fetch_chunk_meta 返回 block_type/table_json/caption
    #[tokio::test]
    async fn fetch_chunk_meta_reads_block_meta() {
        let dir = std::env::temp_dir().join(format!("prism_eval_meta_{}", uuid::Uuid::new_v4()));
        let db = crate::data::db::Database::new(&dir).await.unwrap();
        let now = chrono::Utc::now().timestamp();

        // 满足 rag_chunks 外键（wiki → document → chunk）
        sqlx::query(
            "INSERT INTO wikis (id, name, created_at, updated_at) VALUES ('wk-1', 't', 0, 0)",
        )
        .execute(&db.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO rag_documents (id, wiki_id, name, mime_type, size, chunk_count, status, created_at, updated_at) \
             VALUES ('doc-1', 'wk-1', 't.md', 'text/markdown', 10, 1, 'ready', 0, 0)"
        )
        .execute(&db.pool).await.unwrap();

        let chunk_id = "ch-meta-1";
        sqlx::query(
            "INSERT INTO rag_chunks (id, document_id, wiki_id, \"index\", content, block_type, table_json, caption, created_at) \
             VALUES (?1, 'doc-1', 'wk-1', 0, '单元格内容', 'table', ?2, NULL, ?3)"
        )
        .bind(chunk_id)
        .bind(r#"[["姓名","年龄"],["张三","30"]]"#)
        .bind(now)
        .execute(&db.pool)
        .await
        .unwrap();

        let metas = fetch_chunk_meta(&db, &[chunk_id.to_string()])
            .await
            .unwrap();
        assert_eq!(metas.len(), 1);
        assert_eq!(metas[0].block_type, "table");
        let rate =
            table_cell_match_rate(metas[0].table_json.as_deref(), Some(&[vec!["张三".into()]]))
                .unwrap();
        assert_eq!(rate, 1.0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// CI 回归门槛（§10.2.5）：自包含 golden set → run_eval → 指标必须 ≥ 基线。
    /// 检索/页码/表格三维度零 LLM 成本，cargo test 在 CI 中即执行门槛，低于基线阻止合并。
    #[tokio::test]
    async fn eval_gate_meets_baselines() {
        use crate::data::rag::embedding::{embedding_to_bytes, Embedder, LocalEmbedder};
        use crate::data::rag::store;

        let dir = std::env::temp_dir().join(format!("prism_eval_gate_{}", uuid::Uuid::new_v4()));
        let db = crate::data::db::Database::new(&dir).await.unwrap();

        sqlx::query(
            "INSERT INTO wikis (id, name, created_at, updated_at) VALUES ('wk-eval', 't', 0, 0)",
        )
        .execute(&db.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO rag_documents (id, wiki_id, name, mime_type, size, chunk_count, status, created_at, updated_at) \
             VALUES ('doc-gate', 'wk-eval', '财务报告.md', 'text/markdown', 100, 2, 'ready', 0, 0)"
        )
        .execute(&db.pool).await.unwrap();

        // golden set 文档：一个文本 chunk + 一个表格 chunk（页码 1）
        let text_chunk = "Q2 2023 收入增长 3%，主要来自企业客户续费。上一季度为 1.2 亿。";
        let table_chunk = "季度营收明细：Q2 2023 营收 1.2 亿，Q1 2023 营收 1.1 亿。";

        let embedder = LocalEmbedder::default();
        let emb = embedder
            .embed_batch(&[text_chunk.to_string(), table_chunk.to_string()])
            .await
            .unwrap();

        store::insert_chunks(
            &db,
            "doc-gate",
            "wk-eval",
            &[
                (text_chunk.to_string(), Some(embedding_to_bytes(&emb[0]))),
                (table_chunk.to_string(), Some(embedding_to_bytes(&emb[1]))),
            ],
            None,
            Some(&[(Some(1), Some(1)), (Some(1), Some(1))]),
        )
        .await
        .unwrap();
        let ch_text = chunk_ids_first(&db, 0).await;
        let ch_table = chunk_ids_first(&db, 1).await;
        // 表格 chunk 标注 block_type + table_json
        sqlx::query("UPDATE rag_chunks SET block_type = 'table', table_json = ? WHERE id = ?")
            .bind(r#"[["季度","营收"],["Q2 2023","1.2 亿"]]"#)
            .bind(&ch_table)
            .execute(&db.pool)
            .await
            .unwrap();

        let cases = vec![
            EvalCase {
                id: "ev-gate-1".into(),
                wiki_id: "wk-eval".into(),
                question: "Q2 2023 收入增长是多少？".into(),
                expect: EvalExpect {
                    chunk_ids: vec![ch_text.clone()],
                    pages: vec![1],
                    answer_keywords: vec!["3%".into()],
                    ..Default::default()
                },
                suite: "ci".into(),
            },
            EvalCase {
                id: "ev-gate-2".into(),
                wiki_id: "wk-eval".into(),
                question: "各季度营收明细？".into(),
                expect: EvalExpect {
                    chunk_ids: vec![ch_table.clone()],
                    has_table: Some(true),
                    table_expected: Some(vec![vec!["Q2 2023".into(), "1.2 亿".into()]]),
                    ..Default::default()
                },
                suite: "ci".into(),
            },
        ];

        let report = run_eval(&db, cases, 5, "ci".into()).await.unwrap();

        // 基线：检索/页码/表格三维度必须 ≥ 0.8（自包含数据下应稳定命中）
        assert!(
            report.metrics.recall_at_k >= 0.8,
            "recall_at_k={} 低于基线 0.8",
            report.metrics.recall_at_k
        );
        assert!(
            report.metrics.page_acc >= 0.8,
            "page_acc={} 低于基线 0.8",
            report.metrics.page_acc
        );
        assert!(
            report.metrics.table_acc >= 0.8,
            "table_acc={} 低于基线 0.8",
            report.metrics.table_acc
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// insert_chunks 返回插入数量而非 id，测试按 index 回查 chunk id
    async fn chunk_ids_first(db: &Database, index: i32) -> String {
        sqlx::query_scalar::<_, String>(
            "SELECT id FROM rag_chunks WHERE wiki_id = 'wk-eval' AND \"index\" = ? ORDER BY rowid",
        )
        .bind(index)
        .fetch_one(&db.pool)
        .await
        .unwrap()
    }
}
