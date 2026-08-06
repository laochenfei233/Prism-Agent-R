use tauri::State;

use crate::data::models::SkillDto;
use crate::data::services::skill_service::{InstalledSkill, LocalSkill, SkillSearchHit, SkillService};
use crate::utils::error::AppError;

// ── 技能命令 ──────────────────────────────────────────────

#[tauri::command]
pub async fn skill_list(
    state: State<'_, crate::AppState>,
) -> Result<Vec<SkillDto>, AppError> {
    let svc = SkillService::new(state.db.clone());
    svc.list().await
}

#[tauri::command]
pub async fn skill_install(
    state: State<'_, crate::AppState>,
    source: String,
    source_url: Option<String>,
) -> Result<InstalledSkill, AppError> {
    let svc = SkillService::new(state.db.clone());

    // 根据 source 类型解析路径
    let folder_path = if source.starts_with("local:") {
        source[6..].to_string()
    } else if source.starts_with("github:") {
        // GitHub 仓库：需要 clone 后定位技能目录
        // MVP 阶段简化处理，假设 source 本身就是本地路径
        return Err(AppError::Validation("GitHub 安装暂未实现，请使用本地路径".into()));
    } else {
        source
    };

    let source_type = source_url.as_deref().unwrap_or("local");
    svc.install(&folder_path, Some(source_type)).await
}

#[tauri::command]
pub async fn skill_uninstall(
    state: State<'_, crate::AppState>,
    id: String,
) -> Result<(), AppError> {
    let svc = SkillService::new(state.db.clone());
    svc.uninstall(&id).await
}

#[tauri::command]
pub async fn skill_toggle(
    state: State<'_, crate::AppState>,
    agent_id: String,
    skill_id: String,
    enabled: bool,
) -> Result<(), AppError> {
    let svc = SkillService::new(state.db.clone());
    svc.toggle(&agent_id, &skill_id, enabled).await
}

#[tauri::command]
pub async fn skill_search_market(
    _state: State<'_, crate::AppState>,
    _query: String,
) -> Result<Vec<SkillSearchHit>, AppError> {
    // MVP 阶段返回空列表，Phase 2 实现三源搜索
    Ok(Vec::new())
}

#[tauri::command]
pub async fn skill_list_local(
    state: State<'_, crate::AppState>,
    workdir: String,
) -> Result<Vec<LocalSkill>, AppError> {
    let svc = SkillService::new(state.db.clone());
    svc.list_local(&workdir).await
}
