use crate::data::models::*;
use crate::data::services::meeting_service::MeetingService;
use crate::utils::error::AppError;
use crate::utils::paths;
use tauri::State;

#[tauri::command]
pub async fn meeting_create(
    state: State<'_, crate::AppState>,
    title: String,
    participants: Option<Vec<String>>,
) -> Result<MeetingDto, AppError> {
    let svc = MeetingService::new(state.db.clone(), paths::meetings_dir());
    let m = svc.create(&title, participants.as_deref()).await?;
    Ok(meeting_to_dto(m))
}

#[tauri::command]
pub async fn meeting_list(state: State<'_, crate::AppState>) -> Result<Vec<MeetingDto>, AppError> {
    let svc = MeetingService::new(state.db.clone(), paths::meetings_dir());
    Ok(svc.list().await?.into_iter().map(meeting_to_dto).collect())
}

#[tauri::command]
pub async fn meeting_get(
    state: State<'_, crate::AppState>,
    id: String,
) -> Result<MeetingDto, AppError> {
    let svc = MeetingService::new(state.db.clone(), paths::meetings_dir());
    Ok(meeting_to_dto(svc.get(&id).await?))
}

#[tauri::command]
pub async fn meeting_delete(state: State<'_, crate::AppState>, id: String) -> Result<(), AppError> {
    let svc = MeetingService::new(state.db.clone(), paths::meetings_dir());
    svc.delete(&id).await
}

#[tauri::command]
pub async fn meeting_update_transcript(
    state: State<'_, crate::AppState>,
    id: String,
    segments: Vec<TranscriptSegmentDto>,
) -> Result<(), AppError> {
    let svc = MeetingService::new(state.db.clone(), paths::meetings_dir());
    let segs: Vec<TranscriptSegment> = segments
        .into_iter()
        .map(|s| TranscriptSegment {
            index: s.index,
            text: s.text,
            is_final: s.is_final,
            translated: s.translated,
            speaker_id: s.speaker_id,
        })
        .collect();
    svc.update_transcript(&id, &segs).await
}

#[tauri::command]
pub async fn meeting_get_transcript(
    state: State<'_, crate::AppState>,
    id: String,
) -> Result<Vec<TranscriptSegmentDto>, AppError> {
    let svc = MeetingService::new(state.db.clone(), paths::meetings_dir());
    Ok(svc
        .get_transcript(&id)
        .await?
        .into_iter()
        .map(|s| TranscriptSegmentDto {
            index: s.index,
            text: s.text,
            is_final: s.is_final,
            translated: s.translated,
            speaker_id: s.speaker_id,
        })
        .collect())
}

#[tauri::command]
pub async fn meeting_summary(
    state: State<'_, crate::AppState>,
    id: String,
) -> Result<String, AppError> {
    let svc = MeetingService::new(state.db.clone(), paths::meetings_dir());
    svc.summary(&id).await
}

#[tauri::command]
pub async fn meeting_export(
    state: State<'_, crate::AppState>,
    id: String,
    format: String,
    include_summary: Option<bool>,
    include_translation: Option<bool>,
) -> Result<String, AppError> {
    let svc = MeetingService::new(state.db.clone(), paths::meetings_dir());
    svc.export(&id, &format, include_summary, include_translation)
        .await
}

/// §10.3.4 摘要后生成翻译稿并保存 transcript_translated.md
#[tauri::command]
pub async fn meeting_export_translation(
    state: State<'_, crate::AppState>,
    id: String,
    target_lang: String,
) -> Result<String, AppError> {
    let svc = MeetingService::new(state.db.clone(), paths::meetings_dir());
    svc.export_translated_transcript(&id, &target_lang).await
}

/// §10.3.6 转写清洗
#[tauri::command]
pub async fn meeting_clean(
    state: State<'_, crate::AppState>,
    id: String,
) -> Result<String, AppError> {
    let svc = MeetingService::new(state.db.clone(), paths::meetings_dir());
    svc.clean_transcript(&id).await
}

/// §10.3.6 会议问答
#[tauri::command]
pub async fn meeting_qa(
    state: State<'_, crate::AppState>,
    id: String,
    question: String,
) -> Result<String, AppError> {
    let svc = MeetingService::new(state.db.clone(), paths::meetings_dir());
    svc.qa(&id, &question).await
}

/// §10.3.6 推送 Agent
#[tauri::command]
pub async fn meeting_push_to_agent(
    state: State<'_, crate::AppState>,
    meeting_id: String,
    agent_id: String,
    session_id: Option<String>,
) -> Result<String, AppError> {
    let svc = MeetingService::new(state.db.clone(), paths::meetings_dir());
    svc.push_to_agent(&meeting_id, &agent_id, session_id.as_deref())
        .await
}

fn meeting_to_dto(m: Meeting) -> MeetingDto {
    MeetingDto {
        id: m.id,
        title: m.title,
        date: m.date,
        transcript: m.transcript,
        summary: m.summary,
        participants: m.participants,
        recording_duration: m.recording_duration,
        status: m.status,
        created_at: m.created_at,
        updated_at: m.updated_at,
    }
}
