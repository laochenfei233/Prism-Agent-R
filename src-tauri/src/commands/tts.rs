// §10.3.9 TTS 播报 IPC 命令
//
// 系统 TTS 由前端 Web Speech API 执行；后端负责长文分段与状态查询，
// 云端 TTS 后端（DashScope/MiMo）接入时在此扩展（tts:speak 增加云端分支）。

use tauri::State;

use crate::data::services::tts_service::{split_for_speech, TtsVoiceInfo};
use crate::utils::error::AppError;

/// 播报文本：服务端按句分段，返回分段数组供前端队列播放
#[tauri::command]
pub async fn tts_speak(
    _state: State<'_, crate::AppState>,
    text: String,
    lang: Option<String>,
    rate: Option<f32>,
) -> Result<serde_json::Value, AppError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(AppError::Validation("播报文本不能为空".into()));
    }
    if text.chars().count() > 50_000 {
        return Err(AppError::Validation("播报文本过长（上限 5 万字）".into()));
    }
    let segments = split_for_speech(text);
    if segments.is_empty() {
        return Err(AppError::Validation("无可播报内容".into()));
    }
    Ok(serde_json::json!({
        "backend": "system",
        "segments": segments,
        "lang": lang.unwrap_or_else(|| "zh-CN".into()),
        "rate": rate.unwrap_or(1.0),
    }))
}

/// 停止播报（系统 TTS 在前端停止；云端任务接入后在此取消）
#[tauri::command]
pub async fn tts_stop() -> Result<(), AppError> {
    Ok(())
}

/// 可用音色/后端状态
#[tauri::command]
pub async fn tts_voices(state: State<'_, crate::AppState>) -> Result<TtsVoiceInfo, AppError> {
    crate::data::services::tts_service::voices_status(&state.db).await
}
