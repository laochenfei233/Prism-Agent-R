use tauri::State;

use crate::data::models::{IngestResultDto, RagDocumentDto, RagHitDto};
use crate::data::services::rag_service::{EmbeddingConfig, RagService, embedding_status};
use crate::utils::error::AppError;

#[tauri::command]
pub async fn rag_ingest(
    state: State<'_, crate::AppState>,
    wiki_id: String,
    file_path: String,
) -> Result<IngestResultDto, AppError> {
    let mut svc = RagService::new(state.db.clone());
    svc.configure_from_db().await?;
    let result = svc.ingest(&wiki_id, &file_path).await?;
    Ok(IngestResultDto {
        document_id: result.document_id,
        chunk_count: result.chunk_count,
        status: result.status,
    })
}

#[tauri::command]
pub async fn rag_search(
    state: State<'_, crate::AppState>,
    wiki_id: String,
    query: String,
    top_k: Option<usize>,
) -> Result<Vec<RagHitDto>, AppError> {
    let mut svc = RagService::new(state.db.clone());
    svc.configure_from_db().await?;
    let hits = svc.search(&wiki_id, &query, top_k.unwrap_or(5)).await?;
    Ok(hits
        .into_iter()
        .map(|h| RagHitDto {
            chunk_id: h.chunk_id,
            document_title: h.document_title,
            page_start: h.page_start,
            page_end: h.page_end,
            section: h.section,
            quote: h.quote,
            score: h.score,
        })
        .collect())
}

#[tauri::command]
pub async fn rag_list_documents(
    state: State<'_, crate::AppState>,
    wiki_id: String,
) -> Result<Vec<RagDocumentDto>, AppError> {
    let mut svc = RagService::new(state.db.clone());
    svc.configure_from_db().await?;
    svc.list_documents(&wiki_id).await
}

#[tauri::command]
pub async fn rag_delete_document(
    state: State<'_, crate::AppState>,
    doc_id: String,
) -> Result<(), AppError> {
    let mut svc = RagService::new(state.db.clone());
    svc.configure_from_db().await?;
    svc.delete_document(&doc_id).await
}

/// 设置嵌入器配置：mode = "local" | "api"
#[tauri::command]
pub async fn rag_embedding_config(
    state: State<'_, crate::AppState>,
    mode: String,
    provider_id: Option<String>,
    model: Option<String>,
    dim: Option<usize>,
) -> Result<serde_json::Value, AppError> {
    let mut svc = RagService::new(state.db.clone());
    let cfg = EmbeddingConfig { mode, provider_id, model, dim };
    svc.set_config(&cfg).await?;
    embedding_status(&state.db).await
}

/// 当前嵌入器状态
#[tauri::command]
pub async fn rag_embedding_status(
    state: State<'_, crate::AppState>,
) -> Result<serde_json::Value, AppError> {
    embedding_status(&state.db).await
}

/// §10.2.2 Contextual Retrieval 开关
#[tauri::command]
pub async fn rag_contextual_config(
    state: State<'_, crate::AppState>,
    enabled: bool,
) -> Result<serde_json::Value, AppError> {
    let svc = RagService::new(state.db.clone());
    svc.set_contextual(enabled).await?;
    svc.contextual_status().await
}

/// 当前 Contextual Retrieval 状态
#[tauri::command]
pub async fn rag_contextual_status(
    state: State<'_, crate::AppState>,
) -> Result<serde_json::Value, AppError> {
    let svc = RagService::new(state.db.clone());
    svc.contextual_status().await
}

/// §10.2.2 reranker 开关
#[tauri::command]
pub async fn rag_rerank_config(
    state: State<'_, crate::AppState>,
    enabled: bool,
) -> Result<serde_json::Value, AppError> {
    let svc = RagService::new(state.db.clone());
    svc.set_rerank(enabled).await?;
    svc.rerank_status().await
}

/// 当前 reranker 状态
#[tauri::command]
pub async fn rag_rerank_status(
    state: State<'_, crate::AppState>,
) -> Result<serde_json::Value, AppError> {
    let svc = RagService::new(state.db.clone());
    svc.rerank_status().await
}

// ── §10.2.5 RAG 多维评测 ─────────────────────────────────

/// 运行评测（rag:eval）：检索类指标直接度量；表格结构化比对；OCR 字符召回率；图表 LLM-as-Judge。
/// 完成后落库 rag_eval_reports，供 rag:eval-report 趋势对比。
#[tauri::command]
pub async fn rag_eval(
    state: State<'_, crate::AppState>,
    wiki_id: Option<String>,
    suite: Option<String>,
    top_k: Option<usize>,
) -> Result<crate::data::rag::eval::EvalReport, AppError> {
    use crate::data::rag::eval::{
        fetch_chunk_meta, load_cases, ocr_char_recall, save_report, table_cell_match_rate,
        EvalReport,
    };

    let now = chrono::Utc::now().timestamp();
    let mut cases = load_cases(&state.db, suite.as_deref()).await?;
    if let Some(wid) = &wiki_id {
        cases.retain(|c| &c.wiki_id == wid);
    }
    if cases.is_empty() {
        let report = EvalReport {
            suite: suite.unwrap_or_else(|| "default".into()),
            case_count: 0,
            metrics: Default::default(),
            cases: Vec::new(),
            created_at: now,
        };
        save_report(&state.db, &report).await?;
        return Ok(report);
    }

    let top_k = top_k.unwrap_or(5);
    let mut report = EvalReport {
        suite: suite.unwrap_or_else(|| "default".into()),
        case_count: cases.len(),
        metrics: Default::default(),
        cases: Vec::new(),
        created_at: now,
    };

    // 检索类指标直接度量：逐用例 hybrid_search（RagService 循环外复用，避免重复构建嵌入器）
    let mut svc = RagService::new(state.db.clone());
    svc.configure_from_db().await?;

    let mut hit_total = 0usize;
    let mut hit_denom = 0usize;
    let mut page_ok = 0usize;
    let mut page_denom = 0usize;

    // 三维度（§10.2.5）：table_acc 结构化比对 / ocr_completeness 字符召回率 / chart_acc LLM-as-Judge
    let mut table_sum = 0.0f32;
    let mut table_denom = 0usize;
    let mut ocr_sum = 0.0f32;
    let mut ocr_denom = 0usize;
    let mut chart_sum = 0.0f32;
    let mut chart_denom = 0usize;
    let mut chart_model: Option<std::sync::Arc<dyn crate::core::adk::model::ModelProvider>> = None;

    for case in &cases {
        let hits = svc.search(&case.wiki_id, &case.question, top_k).await.unwrap_or_default();
        let hit_ids: Vec<String> = hits.iter().map(|h| h.chunk_id.clone()).collect();
        let metas = fetch_chunk_meta(&state.db, &hit_ids).await.unwrap_or_default();

        // recall@k
        let expected = &case.expect.chunk_ids;
        let matched = expected.iter().filter(|c| hit_ids.contains(c)).count();
        hit_denom += expected.len();
        hit_total += matched;

        // page_acc
        if !case.expect.pages.is_empty() {
            page_denom += 1;
            if hits.iter().any(|h| h.page_start.map(|p| case.expect.pages.contains(&(p as u32))).unwrap_or(false)) {
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
            if let Some(rate) = table_cell_match_rate(table_json, case.expect.table_expected.as_deref()) {
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
                    chart_model = crate::data::services::rag_service::resolve_rerank_model(&state.db).await.ok();
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
        report.cases.push(crate::data::rag::eval::CaseResult {
            id: case.id.clone(),
            question: case.question.clone(),
            passed,
            hit_count: matched,
            detail,
        });
    }

    report.metrics.recall_at_k = if hit_denom > 0 { hit_total as f32 / hit_denom as f32 } else { 0.0 };
    report.metrics.page_acc = if page_denom > 0 { page_ok as f32 / page_denom as f32 } else { 0.0 };
    report.metrics.table_acc = if table_denom > 0 { table_sum / table_denom as f32 } else { 0.0 };
    report.metrics.ocr_completeness = if ocr_denom > 0 { ocr_sum / ocr_denom as f32 } else { 0.0 };
    report.metrics.chart_acc = if chart_denom > 0 { chart_sum / chart_denom as f32 } else { 0.0 };

    // 落库（趋势数据源）
    save_report(&state.db, &report).await?;
    Ok(report)
}

/// 添加评测用例（rag:eval-add）
#[tauri::command]
pub async fn rag_eval_add(
    state: State<'_, crate::AppState>,
    case: serde_json::Value,
) -> Result<String, AppError> {
    use crate::data::rag::eval::{add_case, ensure_wiki, EvalCase, EvalExpect};

    let id = case["id"].as_str().unwrap_or_default();
    let id = if id.is_empty() { uuid::Uuid::new_v4().to_string() } else { id.to_string() };
    let wiki_id = case["wiki_id"].as_str().ok_or_else(|| AppError::Validation("缺少 wiki_id".into()))?;
    let question = case["question"].as_str().ok_or_else(|| AppError::Validation("缺少 question".into()))?;
    let suite = case["suite"].as_str().unwrap_or("default");
    let expect: EvalExpect = if let Some(e) = case.get("expect") {
        serde_json::from_value(e.clone())?
    } else {
        EvalExpect::default()
    };

    ensure_wiki(&state.db, wiki_id).await?;
    add_case(&state.db, &EvalCase {
        id: id.clone(),
        wiki_id: wiki_id.into(),
        question: question.into(),
        expect,
        suite: suite.into(),
    })
    .await?;
    Ok(id)
}

/// 历史评测报告（rag:eval-report）
#[tauri::command]
pub async fn rag_eval_report(
    state: State<'_, crate::AppState>,
) -> Result<Vec<crate::data::rag::eval::EvalReport>, AppError> {
    crate::data::rag::eval::list_reports(&state.db).await
}
