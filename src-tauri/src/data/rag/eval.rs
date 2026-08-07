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
    /// 表格解析准确（LLM-as-Judge 或结构化比对，此处占位）
    pub table_acc: f32,
    /// OCR 无漏字（占位：需扫描件样本）
    pub ocr_completeness: f32,
    /// 图表正确理解（LLM-as-Judge，占位）
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
        out.push(EvalCase { id, wiki_id, question, expect, suite });
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
pub async fn fetch_chunk_meta(db: &Database, chunk_ids: &[String]) -> Result<Vec<ChunkMeta>, AppError> {
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

/// 表格解析准确：期望单元格在 table_json 中的逐格匹配率（结构化比对，无 LLM 成本）。
/// 无期望单元格时退化为「命中 table 块」的布尔得分。
pub fn table_cell_match_rate(table_json: Option<&str>, expected: Option<&[Vec<String>]>) -> Option<f32> {
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
        serde_json::Value::Object(map) => map.values().filter_map(|v| v.as_str().map(String::from)).collect(),
        _ => Vec::new(),
    };

    match expected {
        None => Some(if cells.is_empty() { 0.0 } else { 1.0 }),
        Some(rows) => {
            let expected_flat: Vec<&String> = rows.iter().flatten().collect();
            if expected_flat.is_empty() {
                return Some(if cells.is_empty() { 0.0 } else { 1.0 });
            }
            let matched = expected_flat.iter().filter(|e| cells.iter().any(|c| c.contains(e.as_str()))).count();
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
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT COUNT(*) FROM wikis WHERE id = ?").bind(wiki_id).fetch_one(&db.pool).await?;
    if exists.unwrap_or(0) == 0 {
        return Err(AppError::Validation(format!("知识库不存在: {wiki_id}")));
    }
    Ok(())
}

pub async fn list_docs(db: &Database, wiki_id: &str) -> Result<Vec<RagDocumentRow>, AppError> {
    Ok(crate::data::rag::store::list_documents(db, wiki_id).await?)
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
            metrics: Metrics { recall_at_k: 0.75, page_acc: 0.5, ..Default::default() },
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
}
