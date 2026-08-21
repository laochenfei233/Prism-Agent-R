use std::sync::Arc;

use tauri::{Emitter, State};

use crate::data::models::*;
use crate::data::services::asr::{AsrBackendConfig, AsrEventSink, AsrModelManager, AsrSegment};
use crate::data::services::meeting::AudioStreamManager;
use crate::data::services::meeting_service::MeetingService;
use crate::utils::error::AppError;
use crate::utils::paths;

/// 模型中转站：AudioStreamManager 实例（AppState 持有）
type SharedAudioStreams = Arc<AudioStreamManager>;

#[tauri::command]
pub async fn asr_list_configs(
    state: State<'_, crate::AppState>,
) -> Result<Vec<AsrConfigDto>, AppError> {
    let svc = MeetingService::new(state.db.clone(), paths::meetings_dir());
    Ok(svc
        .list_asr_configs()
        .await?
        .into_iter()
        .map(|c| AsrConfigDto {
            id: c.id,
            name: c.name,
            kind: c.kind,
            base_url: c.base_url,
            model: c.model,
            lang: c.lang,
            is_default: c.is_default,
            model_path: c.model_path,
            extra: c.extra,
        })
        .collect())
}

#[tauri::command]
pub async fn asr_save_config(
    state: State<'_, crate::AppState>,
    config: AsrConfigInput,
) -> Result<AsrConfigDto, AppError> {
    let svc = MeetingService::new(state.db.clone(), paths::meetings_dir());
    let c = svc.save_asr_config(config).await?;
    Ok(AsrConfigDto {
        id: c.id,
        name: c.name,
        kind: c.kind,
        base_url: c.base_url,
        model: c.model,
        lang: c.lang,
        is_default: c.is_default,
        model_path: c.model_path,
        extra: c.extra,
    })
}

#[tauri::command]
pub async fn asr_delete_config(
    state: State<'_, crate::AppState>,
    id: String,
) -> Result<(), AppError> {
    let svc = MeetingService::new(state.db.clone(), paths::meetings_dir());
    svc.delete_asr_config(&id).await
}

/// 可用后端列表：注册表动态查询（内置 + 运行时注册的自定义后端），不写死
#[tauri::command]
pub async fn asr_backends() -> Result<Vec<AsrBackendInfoDto>, AppError> {
    use crate::data::services::asr::registered_backends;
    let mut out: Vec<AsrBackendInfoDto> = registered_backends()
        .into_iter()
        .map(|kind| {
            let k = crate::data::services::asr::AsrKind::parse(&kind);
            AsrBackendInfoDto {
                kind: kind.clone(),
                name: k.display_name().to_string(),
                description: backend_desc(&kind),
                languages: k.languages(),
            }
        })
        .collect();
    // 未注册到枚举的新后端也能透出（kind_raw 保留原名）
    for kind in [
        "DashScopeFunasr",
        "MiMoHttp",
        "SherpaOnnx",
        "LocalFunasrWs",
        "WhisperApi",
        "Vosk",
        "AzureSpeech",
        "Custom",
    ] {
        if !out.iter().any(|b| b.kind == kind) {
            let k = crate::data::services::asr::AsrKind::parse(kind);
            out.push(AsrBackendInfoDto {
                kind: kind.into(),
                name: k.display_name().into(),
                description: backend_desc(kind),
                languages: k.languages(),
            });
        }
    }
    Ok(out)
}

fn backend_desc(kind: &str) -> String {
    match kind {
        "DashScopeFunasr" => "云端 WebSocket 流式中文识别（新版 api-ws 协议）".into(),
        "MiMoHttp" => "MiMo HTTP 语音识别（OpenAI 兼容）".into(),
        "SherpaOnnx" => "本地离线语音识别（可指定任意模型路径）".into(),
        "LocalFunasrWs" => "连接本地 FunASR WebSocket 服务".into(),
        "WhisperApi" => "OpenAI Whisper 分片上传".into(),
        "Vosk" => "本地轻量离线识别（可指定任意模型路径）".into(),
        "AzureSpeech" => "Azure Speech（待接入）".into(),
        _ => "自定义 ASR 端点（OpenAI 兼容，注册表可扩展）".into(),
    }
}

// ── 模型管理 ──────────────────────────────────────────────

#[tauri::command]
pub async fn asr_model_catalog() -> Result<Vec<AsrModelInfoDto>, AppError> {
    let mgr = AsrModelManager::new(paths::app_data_dir().join("asr_models"));
    Ok(mgr
        .catalog()
        .into_iter()
        .map(|m| AsrModelInfoDto {
            id: m.id,
            name: m.name,
            backend: m.backend,
            category: match m.category {
                crate::data::services::asr::AsrModelCategory::Online => "online".into(),
                crate::data::services::asr::AsrModelCategory::Local => "local".into(),
            },
            size_mb: m.size_mb,
            lang: m.lang,
            url: m.url,
            requires_vad: m.requires_vad,
            user_placed: m.user_placed,
            default_model_id: m.default_model_id,
            requires_api_key: m.requires_api_key,
        })
        .collect())
}

#[tauri::command]
pub async fn asr_model_installed() -> Result<Vec<InstalledAsrModelDto>, AppError> {
    let mgr = AsrModelManager::new(paths::app_data_dir().join("asr_models"));
    Ok(mgr
        .installed()
        .into_iter()
        .map(|m| InstalledAsrModelDto {
            id: m.id,
            path: m.path,
            size_mb: m.size_mb,
            backend: m.backend,
            lang: m.lang,
        })
        .collect())
}

#[tauri::command]
pub async fn asr_model_download(
    app: tauri::AppHandle,
    model_id: String,
) -> Result<serde_json::Value, AppError> {
    let mgr = AsrModelManager::new(paths::app_data_dir().join("asr_models"));
    let progress_app = app.clone();
    let model_id_clone = model_id.clone();
    let path = mgr
        .download(
            &model_id,
            Some(Box::new(move |frac, msg| {
                let _ = progress_app.emit(
                    "asr:model-download-progress",
                    serde_json::json!({
                        "model_id": model_id_clone,
                        "progress": frac,
                        "message": msg,
                    }),
                );
            })),
        )
        .await?;
    Ok(
        serde_json::json!({ "model_id": model_id, "path": path.to_string_lossy(), "status": "downloaded" }),
    )
}

#[tauri::command]
pub async fn asr_model_remove(model_id: String) -> Result<(), AppError> {
    let mgr = AsrModelManager::new(paths::app_data_dir().join("asr_models"));
    mgr.remove(&model_id).await
}

// ── ASR 连通性测试 ────────────────────────────────────────

#[tauri::command]
pub async fn asr_test(config: AsrConfigInput) -> Result<serde_json::Value, AppError> {
    let backend_cfg = AsrBackendConfig::from_input(
        &config.kind,
        config.base_url,
        config.api_key,
        config.model,
        config.lang,
        config.model_path,
        config.extra,
    );
    let backend = crate::data::services::asr::create_asr_backend(&backend_cfg);
    let started = std::time::Instant::now();
    match backend.health_check().await {
        Ok(()) => Ok(
            serde_json::json!({ "ok": true, "latency_ms": started.elapsed().as_millis() as u64, "error": null }),
        ),
        Err(e) => Ok(
            serde_json::json!({ "ok": false, "latency_ms": started.elapsed().as_millis() as u64, "error": e.to_string() }),
        ),
    }
}

// ── 会议录音（ASR 转发） ──────────────────────────────────

/// 启动录音：先创建音频流（规避旧版丢块时序问题），后端健康检查通过后返回。
#[tauri::command]
pub async fn meeting_start_recording(
    app: tauri::AppHandle,
    state: State<'_, crate::AppState>,
    id: String,
    asr_config: Option<AsrConfigInput>,
) -> Result<(), AppError> {
    let streams = get_streams(&state).await;

    // 配置解析：显式传入优先；否则回退到默认配置（is_default=1）；均无则报错
    let cfg = if let Some(c) = &asr_config {
        Some(c.clone())
    } else {
        let svc = MeetingService::new(state.db.clone(), paths::meetings_dir());
        let configs = svc.list_asr_configs().await?;
        configs
            .into_iter()
            .find(|c| c.is_default)
            .map(|c| AsrConfigInput {
                name: c.name,
                kind: c.kind,
                base_url: c.base_url,
                api_key: None,
                model: c.model,
                lang: c.lang,
                is_default: true,
                model_path: c.model_path,
                extra: c.extra,
            })
    };

    let Some(cfg) = cfg else {
        return Err(AppError::Validation(
            "未配置 ASR 后端。请先在「设置 → 语音识别 (ASR)」中添加并设为默认。".into(),
        ));
    };

    let backend_cfg = AsrBackendConfig::from_input(
        &cfg.kind,
        cfg.base_url.clone(),
        cfg.api_key.clone(),
        cfg.model.clone(),
        cfg.lang.clone(),
        cfg.model_path.clone(),
        cfg.extra.clone(),
    );
    let mut backend = crate::data::services::asr::create_asr_backend(&backend_cfg);
    backend
        .health_check()
        .await
        .map_err(crate::utils::error::AppError::from)?;

    // 先建 stream（ASR 消费端）——时序规避核心
    let rx = streams.create_stream(&id).await;

    // 状态机：idle → recording
    let svc = MeetingService::new(state.db.clone(), paths::meetings_dir());
    svc.set_status(&id, "recording").await?;

    // 事件回调 → 增量落库 + 前端事件（meeting:transcript 实时转录）
    let db = state.db.clone();
    let app_handle = app.clone();
    let meeting_id = id.clone();
    let events = AsrEventSink::new(
        move |seg: AsrSegment| {
            let svc = MeetingService::new(db.clone(), paths::meetings_dir());
            let app = app_handle.clone();
            let meeting_id = meeting_id.clone();
            tokio::spawn(async move {
                let _ = svc
                    .update_transcript(
                        &meeting_id,
                        &[TranscriptSegment {
                            index: seg.index as i32,
                            text: seg.text.clone(),
                            is_final: seg.is_final,
                            translated: None,
                            speaker_id: seg.speaker_id,
                        }],
                    )
                    .await;
                let _ = app.emit(
                    "meeting:transcript",
                    serde_json::json!({
                        "meeting_id": meeting_id,
                        "index": seg.index,
                        "text": seg.text,
                        "is_final": seg.is_final,
                        "speaker_id": seg.speaker_id,
                    }),
                );
            });
        },
        |status: String| {
            tracing::info!("[ASR] {status}");
        },
    );

    // 音频流（PCM 块流）→ 后端 start
    let audio = Box::pin(futures::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|chunk| (chunk, rx))
    }));
    let _handle = backend
        .start(audio, events)
        .await
        .map_err(crate::utils::error::AppError::from)?;
    // 会话句柄由前端 stop 时取消（简化：每次 start 覆盖）

    Ok(())
}

/// 推送音频块（前端 Web Audio API → IPC）
/// 双写：① recording.wav 落盘（供离线二次转写/换模型重转）② 推给 ASR 流
#[tauri::command]
pub async fn meeting_audio_chunk(
    state: State<'_, crate::AppState>,
    meeting_id: String,
    pcm_base64: String,
) -> Result<(), AppError> {
    let streams = get_streams(&state).await;
    use base64::Engine;
    let pcm = base64::engine::general_purpose::STANDARD
        .decode(pcm_base64.trim())
        .map_err(|e| AppError::Validation(format!("PCM base64 解码失败: {e}")))?;
    // 双写 ①：录音文件
    let svc = MeetingService::new(state.db.clone(), paths::meetings_dir());
    if let Err(e) = svc.append_recording(&meeting_id, &pcm).await {
        tracing::warn!("[ASR] 录音双写失败（不影响实时转写）: {e}");
    }
    // 双写 ②：ASR 流
    streams.push_chunk(&meeting_id, pcm).await?;
    Ok(())
}

/// 停止录音：移除音频流 + 落库最终转写 + 状态机 recording/paused → ready
#[tauri::command]
pub async fn meeting_stop_recording(
    app: tauri::AppHandle,
    state: State<'_, crate::AppState>,
    id: String,
) -> Result<serde_json::Value, AppError> {
    let streams = get_streams(&state).await;
    streams.drop_stream(&id).await;
    let svc = MeetingService::new(state.db.clone(), paths::meetings_dir());
    let transcript = svc.transcript_text(&id).await?;
    sqlx::query("UPDATE meetings SET transcript = ?, updated_at = ? WHERE id = ?")
        .bind(&transcript)
        .bind(chrono::Utc::now().timestamp())
        .bind(&id)
        .execute(&state.db.pool)
        .await?;
    // 状态机：recording/paused → ready（停止即定稿）
    svc.set_status(&id, "ready").await?;
    let _ = app.emit(
        "meeting:status",
        serde_json::json!({
            "meeting_id": id,
            "status": "ready",
        }),
    );
    Ok(serde_json::json!({ "transcript": transcript }))
}

/// 暂停录音（§10.3 状态机）：recording → paused，音频流保留，前端停止采集
#[tauri::command]
pub async fn meeting_pause_recording(
    app: tauri::AppHandle,
    state: State<'_, crate::AppState>,
    id: String,
) -> Result<(), AppError> {
    let svc = MeetingService::new(state.db.clone(), paths::meetings_dir());
    svc.pause(&id).await?;
    let _ = app.emit(
        "meeting:status",
        serde_json::json!({
            "meeting_id": id,
            "status": "paused",
        }),
    );
    Ok(())
}

/// 恢复录音（§10.3 状态机）：paused → recording
#[tauri::command]
pub async fn meeting_resume_recording(
    app: tauri::AppHandle,
    state: State<'_, crate::AppState>,
    id: String,
) -> Result<(), AppError> {
    let svc = MeetingService::new(state.db.clone(), paths::meetings_dir());
    svc.resume(&id).await?;
    let _ = app.emit(
        "meeting:status",
        serde_json::json!({
            "meeting_id": id,
            "status": "recording",
        }),
    );
    Ok(())
}

/// 取消录音（§10.3 状态机）：任意状态 → cancelled，丢弃音频流并清理转写
#[tauri::command]
pub async fn meeting_cancel_recording(
    app: tauri::AppHandle,
    state: State<'_, crate::AppState>,
    id: String,
) -> Result<(), AppError> {
    let streams = get_streams(&state).await;
    streams.drop_stream(&id).await;
    let svc = MeetingService::new(state.db.clone(), paths::meetings_dir());
    svc.cancel(&id).await?;
    // 清理已落库的转写片段（取消 = 本次录音作废）
    sqlx::query("DELETE FROM meeting_transcripts WHERE meeting_id = ?")
        .bind(&id)
        .execute(&state.db.pool)
        .await?;
    let _ = app.emit(
        "meeting:status",
        serde_json::json!({
            "meeting_id": id,
            "status": "cancelled",
        }),
    );
    Ok(())
}

async fn get_streams(state: &State<'_, crate::AppState>) -> SharedAudioStreams {
    state.audio_streams.clone()
}

// ── DTO 别名 ──────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AsrModelInfoDto {
    pub id: String,
    pub name: String,
    pub backend: String,
    /// 模型类别：online（在线 API）或 local（本地离线）
    pub category: String,
    pub size_mb: u64,
    pub lang: Vec<String>,
    pub url: String,
    pub requires_vad: bool,
    pub user_placed: bool,
    /// 在线模型：默认模型 ID
    pub default_model_id: Option<String>,
    /// 在线模型：是否需要 API Key
    pub requires_api_key: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InstalledAsrModelDto {
    pub id: String,
    pub path: String,
    pub size_mb: u64,
    pub backend: String,
    pub lang: Vec<String>,
}
