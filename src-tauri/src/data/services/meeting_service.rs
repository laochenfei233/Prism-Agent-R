use std::path::PathBuf;

use crate::data::db::Database;
use crate::data::models::*;
use crate::utils::error::AppError;

pub struct MeetingService {
    db: Database,
    base_dir: PathBuf,
}

impl MeetingService {
    pub fn new(db: Database, base_dir: PathBuf) -> Self {
        Self { db, base_dir }
    }

    pub async fn create(&self, title: &str, participants: Option<&[String]>) -> Result<Meeting, AppError> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();
        let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let participants_json = serde_json::to_string(participants.unwrap_or(&[])).unwrap_or_default();
        let folder = self.base_dir.join(&id);
        tokio::fs::create_dir_all(&folder).await?;

        sqlx::query(
            "INSERT INTO meetings (id, title, date, transcript, summary, participants, recording_duration, folder_path, created_at, updated_at) VALUES (?1, ?2, ?3, '', '', ?4, 0, ?5, ?6, ?6)"
        )
        .bind(&id).bind(title).bind(&date).bind(&participants_json)
        .bind(folder.to_string_lossy().to_string()).bind(now)
        .execute(&self.db.pool).await?;

        Ok(Meeting { id, title: title.to_string(), date, transcript: String::new(), summary: String::new(), participants: participants.unwrap_or(&[]).to_vec(), recording_duration: 0, created_at: now, updated_at: now })
    }

    pub async fn list(&self) -> Result<Vec<Meeting>, AppError> {
        let rows = sqlx::query_as::<_, MeetingRow>("SELECT * FROM meetings ORDER BY created_at DESC")
            .fetch_all(&self.db.pool).await?;
        Ok(rows.into_iter().map(row_to_meeting).collect())
    }

    pub async fn get(&self, id: &str) -> Result<Meeting, AppError> {
        let row = sqlx::query_as::<_, MeetingRow>("SELECT * FROM meetings WHERE id = ?1")
            .bind(id).fetch_optional(&self.db.pool).await?;
        row.map(row_to_meeting).ok_or_else(|| AppError::Validation(format!("会议不存在: {id}")))
    }

    pub async fn delete(&self, id: &str) -> Result<(), AppError> {
        sqlx::query("DELETE FROM meetings WHERE id = ?1").bind(id).execute(&self.db.pool).await?;
        let folder = self.base_dir.join(id);
        if folder.exists() { let _ = tokio::fs::remove_dir_all(&folder).await; }
        Ok(())
    }

    pub async fn update_transcript(&self, id: &str, segments: &[TranscriptSegment]) -> Result<(), AppError> {
        let now = chrono::Utc::now().timestamp();
        let mut tx = self.db.pool.begin().await?;
        for seg in segments {
            let seg_id = uuid::Uuid::new_v4().to_string();
            // 幂等 upsert：同 (meeting_id, "index") 覆盖更新（迁移 022 唯一索引支撑）
            sqlx::query(
                "INSERT INTO meeting_transcripts (id, meeting_id, \"index\", text, is_final, speaker_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
                 ON CONFLICT(meeting_id, \"index\") DO UPDATE SET text = excluded.text, is_final = excluded.is_final, speaker_id = excluded.speaker_id"
            )
            .bind(&seg_id).bind(id).bind(seg.index).bind(&seg.text)
            .bind(seg.is_final as i32).bind(seg.speaker_id.map(|s| s as i64)).bind(now)
            .execute(&mut *tx).await?;
        }
        sqlx::query("UPDATE meetings SET updated_at = ?1 WHERE id = ?2").bind(now).bind(id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn get_transcript(&self, id: &str) -> Result<Vec<TranscriptSegment>, AppError> {
        let rows = sqlx::query_as::<_, TranscriptSegmentRow>(
            "SELECT * FROM meeting_transcripts WHERE meeting_id = ?1 ORDER BY \"index\" ASC"
        ).bind(id).fetch_all(&self.db.pool).await?;
        Ok(rows.into_iter().map(|r| TranscriptSegment {
            index: r.index,
            text: r.text,
            is_final: r.is_final != 0,
            translated: r.translated,
            speaker_id: r.speaker_id.map(|s| s as u32),
        }).collect())
    }

    /// 转写全文（含说话人前缀，供展示/推送/导出复用）
    pub async fn transcript_text(&self, id: &str) -> Result<String, AppError> {
        let segments = self.get_transcript(id).await?;
        Ok(segments
            .iter()
            .map(|s| format!("{}{}", speaker_prefix(s.speaker_id), s.text))
            .collect::<Vec<_>>()
            .join("\n"))
    }

    /// 生成结构化摘要（§10.3.6）：短转写一次生成；超长（>8K tokens）map-reduce 分段摘要再合并
    pub async fn summary(&self, id: &str) -> Result<String, AppError> {
        let meeting = self.get(id).await?;
        let segments = self.get_transcript(id).await?;
        if segments.is_empty() {
            return Err(AppError::Validation("暂无转写内容，无法生成摘要".into()));
        }
        let transcript: String = segments.iter().map(|s| s.text.clone()).collect::<Vec<_>>().join("\n");
        let participants = meeting.participants.join(", ");

        // 8K tokens ≈ 32K 字符（保守按 4 字符/token）；超过则分段
        const SEGMENT_CHARS: usize = 30_000;
        let full = format!(
            "会议标题：{}\n参会人：{}\n\n转写内容：\n{}",
            meeting.title, participants, transcript
        );

        let resp = if full.chars().count() > SEGMENT_CHARS {
            self.summary_map_reduce(&full).await?
        } else {
            self.summary_once(&full).await?
        };

        // 落库
        sqlx::query("UPDATE meetings SET summary = ?, updated_at = ? WHERE id = ?")
            .bind(&resp)
            .bind(chrono::Utc::now().timestamp())
            .bind(id)
            .execute(&self.db.pool)
            .await?;
        Ok(resp)
    }

    /// 单段摘要（map 阶段的小块 / 整体）
    async fn summary_once(&self, content: &str) -> Result<String, AppError> {
        let prompt = format!(
            "你是一个会议纪要整理助手。根据以下会议内容生成结构化摘要。\n\n{content}\n\n\
             输出 Markdown 格式：\n## 主题\n## 主要讨论\n## 关键决策\n## 待办事项（含负责人）\n## 行动项\n",
        );
        self.call_llm(&prompt).await
    }

    /// map-reduce：按语义边界分块 → 每块摘要 → 合并摘要
    async fn summary_map_reduce(&self, full: &str) -> Result<String, AppError> {
        const CHUNK_CHARS: usize = 28_000;
        // 按段落边界切块（避免截断句子）
        let mut chunks: Vec<String> = Vec::new();
        let mut current = String::new();
        for para in full.split('\n') {
            if current.chars().count() + para.chars().count() + 1 > CHUNK_CHARS && !current.is_empty() {
                chunks.push(std::mem::take(&mut current));
            }
            if !current.is_empty() { current.push('\n'); }
            current.push_str(para);
        }
        if !current.is_empty() { chunks.push(current); }

        // map：每块独立摘要（关键信息抽取）
        let mut partials = Vec::with_capacity(chunks.len());
        for chunk in &chunks {
            let prompt = format!(
                "从以下会议片段中抽取关键信息：主题要点、重要讨论、决策、待办事项（含负责人）、行动项。\n\
                 只输出要点，不要重复原文。\n\n{}",
                truncate_chars(chunk, 28_000)
            );
            partials.push(self.call_llm(&prompt).await?);
        }

        // reduce：合并各块要点为最终结构化摘要
        let joined = partials.join("\n\n---\n\n");
        let prompt = format!(
            "以下是同一会议的多个片段要点。将它们合并为一份完整、去重的结构化会议摘要。\n\n{}",
            truncate_chars(&joined, 28_000)
        );
        self.call_llm(&prompt).await
    }

    /// 摘要后生成翻译稿并保存（§10.3.4：transcript_translated.md）
    pub async fn export_translated_transcript(&self, id: &str, target_lang: &str) -> Result<String, AppError> {
        let segments = self.get_transcript(id).await?;
        if segments.is_empty() {
            return Err(AppError::Validation("暂无转写内容".into()));
        }
        let transcript: String = segments.iter().map(|s| s.text.clone()).collect::<Vec<_>>().join("\n");
        let prompt = format!(
            "将以下会议转写翻译成{}。保留原意、专有名词和术语。输出翻译后的文本。\n\n{}",
            target_lang,
            truncate_chars(&transcript, 20_000)
        );
        let translated = self.call_llm(&prompt).await?;
        // 保存到 meetings/{id}/transcript_translated.md
        let folder = self.base_dir.join(id);
        tokio::fs::create_dir_all(&folder).await?;
        let path = folder.join("transcript_translated.md");
        tokio::fs::write(&path, &translated).await?;
        Ok(path.to_string_lossy().to_string())
    }

    /// 转写清洗（§10.3.6）：修正错别字、补标点、按语义分段
    pub async fn clean_transcript(&self, id: &str) -> Result<String, AppError> {
        let segments = self.get_transcript(id).await?;
        let raw: String = segments.iter().map(|s| s.text.clone()).collect::<Vec<_>>().join("\n");
        if raw.trim().is_empty() {
            return Err(AppError::Validation("暂无转写内容".into()));
        }
        let prompt = format!(
            "你是会议转写清洗助手。修正以下转写中的错别字、补充标点、按语义分段，保留原意。\n\
             只输出清洗后的 Markdown 文本。\n\n{}",
            truncate_chars(&raw, 20_000)
        );
        self.call_llm(&prompt).await
    }

    /// 会议问答（§10.3.6）：上下文 = 标题 + 参会人 + 转写 + 摘要
    pub async fn qa(&self, id: &str, question: &str) -> Result<String, AppError> {
        let meeting = self.get(id).await?;
        let segments = self.get_transcript(id).await?;
        let transcript: String = segments.iter().map(|s| s.text.clone()).collect::<Vec<_>>().join("\n");
        let prompt = format!(
            "基于以下会议内容回答用户问题。\n\n标题：{}\n参会人：{}\n\n转写：\n{}\n\n摘要：\n{}\n\n问题：{}\n\n回答：",
            meeting.title,
            meeting.participants.join(", "),
            truncate_chars(&transcript, 20_000),
            meeting.summary,
            question
        );
        self.call_llm(&prompt).await
    }

    /// 推送给 Agent（§10.3.6）：构建消息注入 Agent 会话
    pub async fn push_to_agent(&self, id: &str, agent_id: &str, session_id: Option<&str>) -> Result<String, AppError> {
        let meeting = self.get(id).await?;
        let segments = self.get_transcript(id).await?;
        let transcript: String = segments.iter().map(|s| s.text.clone()).collect::<Vec<_>>().join("\n");
        let content = format!(
            "[会议纪要推送] 标题：{}\n日期：{}\n参会人：{}\n\n转写：\n{}\n\n摘要：\n{}",
            meeting.title,
            meeting.date,
            meeting.participants.join(", "),
            transcript,
            meeting.summary
        );

        let session = match session_id {
            Some(sid) => sid.to_string(),
            None => {
                let sid = uuid::Uuid::new_v4().to_string();
                let now = chrono::Utc::now().timestamp();
                sqlx::query("INSERT INTO sessions (id, agent_id, title, pinned, created_at, updated_at) VALUES (?, ?, ?, 0, ?, ?)")
                    .bind(&sid).bind(agent_id).bind(format!("会议：{}", meeting.title)).bind(now).bind(now)
                    .execute(&self.db.pool).await?;
                sid
            }
        };
        let msg_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp_millis();
        sqlx::query("INSERT INTO messages (id, session_id, role, content, created_at) VALUES (?, ?, 'user', ?, ?)")
            .bind(&msg_id).bind(&session).bind(&content).bind(now)
            .execute(&self.db.pool).await?;
        Ok(session)
    }

    /// 复用默认模型调用 LLM（prompt 单轮）
    async fn call_llm(&self, prompt: &str) -> Result<String, AppError> {
        use crate::core::adk::model::{ChatMessage, ChatRole, MessageContent, ModelProvider};
        let (provider, _display) = resolve_meeting_model(&self.db).await?;
        let resp = provider
            .generate(crate::core::adk::model::GenerationRequest {
                messages: vec![ChatMessage {
                    role: ChatRole::User,
                    content: MessageContent::Text(prompt.to_string()),
                    name: None,
                }],
                temperature: Some(0.3),
                ..Default::default()
            })
            .await
            .map_err(|e| AppError::LlmProvider(e.to_string()))?;
        Ok(resp.text)
    }

    /// 追加录音（PCM 16kHz mono，WAV 头 + 流式追加写盘）
    pub async fn append_recording(&self, id: &str, pcm: &[u8]) -> Result<(), AppError> {
        let folder = self.base_dir.join(id);
        tokio::fs::create_dir_all(&folder).await?;
        let wav_path = folder.join("recording.wav");
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&wav_path)
            .await?;
        // 首次写入时附带 WAV 头
        let meta = file.metadata().await?;
        if meta.len() == 0 {
            use tokio::io::AsyncWriteExt;
            file.write_all(&pcm_to_wav_header()).await?;
        }
        use tokio::io::AsyncWriteExt;
        file.write_all(pcm).await?;

        // 更新录音时长（按字节推算）
        let seconds = (meta.len() as f64 / 32000.0).round() as i32;
        sqlx::query("UPDATE meetings SET recording_duration = ?, updated_at = ? WHERE id = ?")
            .bind(seconds)
            .bind(chrono::Utc::now().timestamp())
            .bind(id)
            .execute(&self.db.pool)
            .await?;
        Ok(())
    }

    /// 从录音文件重新转写（§10.3.5 离线二次转写）：
    /// 读 recording.wav → PCM 分块 → 走 AsrBackend → 结果替换转写
    pub async fn retranscribe(
        &self,
        id: &str,
        asr_config: &crate::data::services::asr::AsrBackendConfig,
    ) -> Result<Vec<TranscriptSegment>, AppError> {
        use crate::data::services::asr::{create_asr_backend, AsrEventSink};

        let wav_path = self.base_dir.join(id).join("recording.wav");
        if !wav_path.exists() {
            return Err(AppError::Validation("无录音文件，无法重新转写".into()));
        }
        let wav = tokio::fs::read(&wav_path).await?;
        // 跳过 44 字节 WAV 头，剩余为 PCM（16kHz 16bit mono）
        let pcm = if wav.len() > 44 { wav[44..].to_vec() } else { Vec::new() };
        if pcm.is_empty() {
            return Err(AppError::Validation("录音文件为空".into()));
        }

        // 按 2s 一块切分（32000 字节/秒）
        const BLOCK: usize = 64_000;
        let chunks: Vec<Vec<u8>> = pcm.chunks(BLOCK).map(|c| c.to_vec()).collect();

        let mut backend = create_asr_backend(asr_config);
        backend.health_check().await.map_err(crate::utils::error::AppError::from)?;

        // 收集 on_segment 回调结果
        let segments = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::<TranscriptSegment>::new()));
        let segments_out = segments.clone();
        let events = AsrEventSink::new(
            move |seg: crate::data::services::asr::AsrSegment| {
                let segments = segments_out.clone();
                tokio::spawn(async move {
                    segments.lock().await.push(TranscriptSegment {
                        index: seg.index as i32,
                        text: seg.text,
                        is_final: seg.is_final,
                        translated: None,
                        speaker_id: seg.speaker_id,
                    });
                });
            },
            |status: String| tracing::info!("[ASR retranscribe] {status}"),
        );

        // 音频源：静态 PCM 块流
        let audio: crate::data::services::asr::AudioSource =
            Box::pin(futures::stream::iter(chunks));
        let handle = backend.start(audio, events).await.map_err(crate::utils::error::AppError::from)?;

        // 等待后端完成（离线后端流结束后应自行结束；给个上限防止挂死）
        let timeout = tokio::time::Duration::from_secs(600);
        let started = std::time::Instant::now();
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            if started.elapsed() > timeout {
                break;
            }
            // 流结束后通道关闭，后端任务退出；轮询直到无新段且流空
            // 简化：等待 2s 静默（无新增段）视为完成
            let before = segments.lock().await.len();
            tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
            let after = segments.lock().await.len();
            if after == before {
                break;
            }
        }
        let _ = handle;

        let mut segs = segments.lock().await.clone();
        segs.sort_by_key(|s| s.index);
        // 过滤中间结果，只保留 final（或全部保留以防后端不标 final）
        let final_segs: Vec<TranscriptSegment> = segs
            .iter()
            .filter(|s| s.is_final || segs.iter().all(|x| !x.is_final))
            .cloned()
            .collect();

        if final_segs.is_empty() {
            return Err(AppError::Validation("转写无结果，请检查 ASR 配置".into()));
        }

        // 替换转写 + 标记 retranscribed_at
        self.update_transcript(id, &final_segs).await?;
        sqlx::query("UPDATE meetings SET retranscribed_at = ?, updated_at = ? WHERE id = ?")
            .bind(chrono::Utc::now().timestamp())
            .bind(chrono::Utc::now().timestamp())
            .bind(id)
            .execute(&self.db.pool)
            .await?;
        Ok(final_segs)
    }

    /// 导出（§10.3.7）：Markdown 完整模板 / 纯文本；选项控制是否含摘要、翻译
    pub async fn export(
        &self,
        id: &str,
        format: &str,
        include_summary: Option<bool>,
        include_translation: Option<bool>,
    ) -> Result<String, AppError> {
        let meeting = self.get(id).await?;
        let transcript = self.get_transcript(id).await?;
        let inc_summary = include_summary.unwrap_or(true);
        let inc_translation = include_translation.unwrap_or(false);

        // 尝试读取已保存的翻译稿
        let translated = if inc_translation {
            let t_path = self.base_dir.join(id).join("transcript_translated.md");
            tokio::fs::read_to_string(&t_path).await.ok()
        } else {
            None
        };

        match format {
            "markdown" | "md" => {
                let mut out = String::new();
                out.push_str(&format!("# {}\n\n", meeting.title));
                out.push_str(&format!("- **日期**: {}\n", meeting.date));
                out.push_str(&format!("- **参会人**: {}\n", meeting.participants.join(", ")));
                if meeting.recording_duration > 0 {
                    out.push_str(&format!("- **录音时长**: {}s\n", meeting.recording_duration));
                }

                out.push_str("\n---\n\n## 转写\n\n");
                for seg in &transcript {
                    let speaker = speaker_prefix(seg.speaker_id);
                    out.push_str(&format!("{}{}\n\n", speaker, seg.text));
                }

                if inc_summary && !meeting.summary.trim().is_empty() {
                    out.push_str("## 摘要\n\n");
                    out.push_str(&meeting.summary);
                    out.push_str("\n\n");
                }

                if let Some(t) = &translated {
                    out.push_str("## 翻译稿\n\n");
                    out.push_str(t);
                    out.push('\n');
                }
                Ok(out)
            }
            "text" | "txt" => {
                let mut out = String::new();
                out.push_str(&format!("{} ({})\n", meeting.title, meeting.date));
                out.push_str(&format!("参会人: {}\n", meeting.participants.join(", ")));
                out.push_str("----------------------\n");
                for seg in &transcript {
                    out.push_str(&seg.text);
                    out.push('\n');
                }
                if inc_summary && !meeting.summary.trim().is_empty() {
                    out.push_str("\n===== 摘要 =====\n");
                    out.push_str(&meeting.summary);
                    out.push('\n');
                }
                Ok(out)
            }
            _ => Err(AppError::Validation("仅支持 markdown / text 格式".into())),
        }
    }

    pub async fn list_asr_configs(&self) -> Result<Vec<AsrConfig>, AppError> {
        let rows = sqlx::query_as::<_, AsrConfigRow>("SELECT * FROM asr_configs ORDER BY is_default DESC")
            .fetch_all(&self.db.pool).await?;
        Ok(rows.into_iter().map(|r| AsrConfig {
            id: r.id, name: r.name, kind: r.kind, base_url: r.base_url,
            api_key: r.api_key_enc.as_deref().map(crate::commands::settings::decrypt_provider_key),
            model: r.model, lang: r.lang, is_default: r.is_default != 0,
            model_path: r.model_path,
            extra: r.extra.and_then(|e| serde_json::from_str(&e).ok()),
        }).collect())
    }

    pub async fn save_asr_config(&self, input: AsrConfigInput) -> Result<AsrConfig, AppError> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();
        let extra = input.extra.as_ref().map(serde_json::to_string).transpose()?;
        let api_key_enc = input.api_key.as_deref().map(crate::commands::settings::encrypt_provider_key);
        sqlx::query(
            "INSERT INTO asr_configs (id, name, kind, base_url, api_key_enc, model, lang, is_default, model_path, extra, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)"
        )
        .bind(&id).bind(&input.name).bind(&input.kind).bind(&input.base_url)
        .bind(&api_key_enc)
        .bind(&input.model).bind(&input.lang).bind(input.is_default as i32)
        .bind(&input.model_path).bind(&extra).bind(now)
        .execute(&self.db.pool).await?;
        Ok(AsrConfig { id, name: input.name, kind: input.kind, base_url: input.base_url, api_key: input.api_key, model: input.model, lang: input.lang, is_default: input.is_default, model_path: input.model_path, extra: input.extra })
    }

    pub async fn delete_asr_config(&self, id: &str) -> Result<(), AppError> {
        sqlx::query("DELETE FROM asr_configs WHERE id = ?1").bind(id).execute(&self.db.pool).await?;
        Ok(())
    }
}

fn row_to_meeting(r: MeetingRow) -> Meeting {
    let participants: Vec<String> = serde_json::from_str(&r.participants).unwrap_or_default();
    Meeting { id: r.id, title: r.title, date: r.date, transcript: r.transcript, summary: r.summary, participants, recording_duration: r.recording_duration, created_at: r.created_at, updated_at: r.updated_at }
}

/// 16kHz 16bit mono WAV 头（44 字节，data 长度字段为 0，后续追加）
fn pcm_to_wav_header() -> Vec<u8> {
    let sample_rate = 16000u32;
    let channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * channels as u32 * (bits_per_sample / 8) as u32;
    let block_align = channels * (bits_per_sample / 8);

    let mut wav = Vec::with_capacity(44);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&0u32.to_le_bytes()); // 占位，播放器可容忍
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&bits_per_sample.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&0u32.to_le_bytes());
    wav
}

/// 解析默认模型构建 provider（会议 LLM 功能复用）
async fn resolve_meeting_model(db: &Database) -> Result<(crate::core::rig::provider::OpenAiProvider, String), AppError> {
    use crate::data::models::{ModelRow, ProviderRow};
    let model_row = sqlx::query_as::<_, ModelRow>(
        "SELECT id, provider_id, model_id, display_name, kind, max_tokens, is_default, created_at FROM models WHERE is_default = 1 LIMIT 1"
    )
    .fetch_optional(&db.pool)
    .await?
    .ok_or_else(|| AppError::LlmProvider("未配置默认模型，请在设置中添加 Provider 并设置默认模型".into()))?;

    let provider_row = sqlx::query_as::<_, ProviderRow>(
        "SELECT id, name, kind, base_url, api_key_enc, is_enabled, created_at, updated_at FROM providers WHERE id = ?"
    )
    .bind(&model_row.provider_id)
    .fetch_optional(&db.pool)
    .await?
    .ok_or_else(|| AppError::LlmProvider(format!("Provider not found: {}", model_row.provider_id)))?;

    let base_url = provider_row.base_url.unwrap_or_else(|| {
        match provider_row.kind.as_str() {
            "ollama" => "http://localhost:11434/v1".to_string(),
            _ => "https://api.openai.com/v1".to_string(),
        }
    });
    let api_key = provider_row
        .api_key_enc
        .as_deref()
        .map(crate::commands::settings::decrypt_provider_key)
        .unwrap_or_default();
    let display = model_row.display_name.clone().unwrap_or_else(|| model_row.model_id.clone());
    let provider = crate::core::rig::provider::OpenAiProvider::new(
        model_row.provider_id.clone(),
        display.clone(),
        api_key,
        base_url,
        model_row.model_id.clone(),
    );
    Ok((provider, display))
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let head: String = s.chars().take(max).collect();
        format!("{head}…[已截断]")
    }
}

/// 转写片段说话人前缀（§10.3.1）：后端返回 speaker_id 时标注「说话人 N」，否则为空
pub fn speaker_prefix(speaker_id: Option<u32>) -> String {
    match speaker_id {
        Some(n) => format!("[说话人 {n}] "),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 回归：同 index 片段重复落库必须幂等覆盖（迁移 022 唯一索引 + ON CONFLICT）
    #[tokio::test]
    async fn update_transcript_upserts_by_index() {
        let dir = std::env::temp_dir().join(format!("prism_meeting_up_{}", uuid::Uuid::new_v4()));
        let db = crate::data::db::Database::new(&dir).await.unwrap();
        let svc = MeetingService::new(db, std::env::temp_dir());
        let meeting = svc.create("t", None).await.unwrap();

        // 同一 index 0 两次写入（第一次中间结果，第二次定稿）
        svc.update_transcript(&meeting.id, &[TranscriptSegment {
            index: 0, text: "今天天气".into(), is_final: false, translated: None, speaker_id: Some(1),
        }]).await.unwrap();
        svc.update_transcript(&meeting.id, &[TranscriptSegment {
            index: 0, text: "今天天气很好。".into(), is_final: true, translated: None, speaker_id: Some(2),
        }]).await.unwrap();

        let segs = svc.get_transcript(&meeting.id).await.unwrap();
        assert_eq!(segs.len(), 1, "同 index 只能有一行");
        assert_eq!(segs[0].text, "今天天气很好。");
        assert_eq!(segs[0].speaker_id, Some(2));

        // transcript_text 带说话人前缀
        let text = svc.transcript_text(&meeting.id).await.unwrap();
        assert_eq!(text, "[说话人 2] 今天天气很好。");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
