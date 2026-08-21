use tauri::State;

use crate::data::models::{GlossaryTermDto, GlossaryTermInput, ImportResultDto};
use crate::data::services::glossary_service::GlossaryService;
use crate::utils::error::AppError;

#[tauri::command]
pub async fn glossary_list(
    state: State<'_, crate::AppState>,
    lang_pair: Option<String>,
) -> Result<Vec<GlossaryTermDto>, AppError> {
    let svc = GlossaryService::new(state.db.pool.clone());
    let terms = svc.list(lang_pair.as_deref()).await?;
    Ok(terms
        .into_iter()
        .map(|t| GlossaryTermDto {
            id: t.id,
            source_lang: t.source_lang,
            target_lang: t.target_lang,
            source_term: t.source_term,
            target_term: t.target_term,
            category: t.category,
            enabled: t.enabled,
        })
        .collect())
}

#[tauri::command]
pub async fn glossary_add(
    state: State<'_, crate::AppState>,
    term: GlossaryTermInput,
) -> Result<(), AppError> {
    let svc = GlossaryService::new(state.db.pool.clone());
    svc.add(term).await
}

#[tauri::command]
pub async fn glossary_remove(
    state: State<'_, crate::AppState>,
    id: String,
) -> Result<(), AppError> {
    let svc = GlossaryService::new(state.db.pool.clone());
    svc.remove(&id).await
}

/// §10.5.2 更新已有术语
#[tauri::command]
pub async fn glossary_update(
    state: State<'_, crate::AppState>,
    id: String,
    term: GlossaryTermInput,
) -> Result<(), AppError> {
    let svc = GlossaryService::new(state.db.pool.clone());
    svc.update(&id, term).await
}

#[tauri::command]
pub async fn glossary_import_csv(
    state: State<'_, crate::AppState>,
    path: String,
) -> Result<ImportResultDto, AppError> {
    let content = tokio::fs::read_to_string(&path).await?;
    let svc = GlossaryService::new(state.db.pool.clone());
    let result = svc.import_csv(&content).await?;
    Ok(ImportResultDto {
        imported: result.imported,
        failed: result.failed,
    })
}

/// 内置词表清单（打包资源 resources/glossary/*.csv）
#[tauri::command]
pub async fn glossary_builtin_list(
    app: tauri::AppHandle,
) -> Result<Vec<crate::data::models::BuiltinGlossaryDto>, AppError> {
    let dir = builtin_glossary_dir(&app)?;
    let mut out = Vec::new();
    if let Ok(mut entries) = tokio::fs::read_dir(&dir).await {
        let mut names: Vec<String> = Vec::new();
        while let Ok(Some(e)) = entries.next_entry().await {
            if e.path().extension().map(|x| x == "csv").unwrap_or(false) {
                if let Some(n) = e.file_name().to_str() {
                    names.push(n.to_string());
                }
            }
        }
        names.sort();
        for name in names {
            let label = builtin_label(&name);
            out.push(crate::data::models::BuiltinGlossaryDto {
                file: name,
                label: label.0.to_string(),
                description: label.1.to_string(),
            });
        }
    }
    Ok(out)
}

/// 一键导入内置词库（打包资源 resources/glossary/{file}.csv）
#[tauri::command]
pub async fn glossary_import_builtin(
    app: tauri::AppHandle,
    state: State<'_, crate::AppState>,
    file: String,
) -> Result<ImportResultDto, AppError> {
    // 防路径穿越：仅允许白名单文件名
    let dir = builtin_glossary_dir(&app)?;
    let path = dir.join(&file);
    let base = std::path::PathBuf::from(&file);
    if base.components().count() != 1 || !file.ends_with(".csv") {
        return Err(AppError::Validation("非法文件名".into()));
    }
    if !path.exists() {
        return Err(AppError::Validation(format!("内置词表不存在: {file}")));
    }
    let content = tokio::fs::read_to_string(&path).await?;
    let svc = GlossaryService::new(state.db.pool.clone());
    let result = svc.import_builtin_csv(&content).await?;
    Ok(ImportResultDto {
        imported: result.imported,
        failed: result.failed,
    })
}

/// 内置词表目录：resource_dir()/glossary
fn builtin_glossary_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, AppError> {
    use tauri::Manager;
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|e| AppError::Internal(format!("无法获取资源目录: {e}")))?;
    Ok(resource_dir.join("glossary"))
}

/// 词表文件 → (显示名, 描述)
fn builtin_label(file: &str) -> (&'static str, &'static str) {
    match file {
        "microsoft_terms_zh-CN.csv" => (
            "微软通用术语",
            "Microsoft Terminology Collection，33K+ 条通用词汇",
        ),
        "foreign_trade_terms.csv" => ("外贸术语", "INCOTERMS 2020 + 外贸单证/结算/物流"),
        "hs_codes_chapters.csv" => ("HS 编码章节", "HS 协调制度 01-99 章商品分类"),
        "nutrition_supplements.csv" => ("营养品术语", "营养成分/剂型/法规/宣称"),
        "toys_terms.csv" => ("玩具术语", "EN71/ASTM F963/GB 6675 安全 + 分类"),
        "mechanical_terms.csv" => ("机械术语", "机械设备/零件/材料/工艺"),
        "ecommerce_terms.csv" => ("电商术语", "电商平台/运营/物流/售后"),
        _ => ("内置词表", ""),
    }
}
