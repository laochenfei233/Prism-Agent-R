use futures::{SinkExt, StreamExt};

use super::*;

// ── 内置后端注册 ──────────────────────────────────────────
// 新增后端 = 实现 AsrBackend + 在此 register 一行；不修改工厂 match。

pub fn builtin_register() {
    register_backend("DashScopeFunasr", |cfg| Box::new(DashScopeFunasrBackend::new(cfg)));
    register_backend("MiMoHttp", |cfg| Box::new(OpenAiCompatibleBackend::new(cfg)));
    register_backend("Custom", |cfg| Box::new(OpenAiCompatibleBackend::new(cfg)));
    register_backend("WhisperApi", |cfg| Box::new(WhisperApiBackend::new(cfg)));
    // SherpaOnnx：启用 `sherpa-native` feature 时走真实本地推理；否则骨架（校验模型路径，提示安装）
    #[cfg(feature = "sherpa-native")]
    register_backend("SherpaOnnx", |cfg| Box::new(SherpaOnnxBackend::new(cfg)));
    #[cfg(not(feature = "sherpa-native"))]
    register_backend("SherpaOnnx", |cfg| Box::new(LocalNativeBackend::sherpa(cfg)));
    register_backend("LocalFunasrWs", |cfg| Box::new(LocalFunasrWsBackend::new(cfg)));
    // Vosk：启用 `vosk-native` feature 时走真实本地推理；否则骨架（校验模型路径，提示安装）
    #[cfg(feature = "vosk-native")]
    register_backend("Vosk", |cfg| Box::new(VoskBackend::new(cfg)));
    #[cfg(not(feature = "vosk-native"))]
    register_backend("Vosk", |cfg| Box::new(LocalNativeBackend::vosk(cfg)));
    register_backend("AzureSpeech", |cfg| Box::new(AzureSpeechBackend::new(cfg)));
}

/// 按配置构建后端：
/// 1. 精确匹配注册名（kind_raw，用户自定义后端优先）
/// 2. 回退枚举 kind 名
/// 3. 最后回退 Custom（OpenAI 兼容），绝不拒绝未知 kind
pub fn create_asr_backend(cfg: &AsrBackendConfig) -> Box<dyn AsrBackend> {
    if let Some(f) = lookup_factory(&cfg.kind_raw) {
        return f(cfg);
    }
    if let Some(f) = lookup_factory(cfg.kind.as_str()) {
        return f(cfg);
    }
    Box::new(OpenAiCompatibleBackend::new(cfg))
}

// ── OpenAiCompatibleBackend（MiMo / Custom：HTTP /chat/completions 音频） ──

/// MiMo ASR / 自定义 OpenAI 兼容端点（对齐 huiji MiMoAsrService 实测协议）：
/// POST {base_url}/chat/completions，音频以 data URL 内联（input_audio 格式），
/// 3s 合并一次缓冲，返回全量文本。
pub struct OpenAiCompatibleBackend {
    base_url: String,
    api_key: String,
    model: String,
    use_api_key_header: bool,
    langs: Vec<String>,
}

impl OpenAiCompatibleBackend {
    pub fn new(cfg: &AsrBackendConfig) -> Self {
        // MiMo 官方端点用 `api-key` 头（huiji 实测）；其他兼容端点用 Bearer
        let use_api_key_header = cfg
            .base_url
            .as_deref()
            .map(|u| u.contains("xiaomi"))
            .unwrap_or(false);
        Self {
            base_url: cfg.base_url.clone().unwrap_or_else(|| "https://api.xiaomimimo.com/v1".into()),
            api_key: cfg.api_key.clone().unwrap_or_default(),
            model: cfg.model.clone().unwrap_or_else(|| "mimo-v2.5-asr".into()),
            use_api_key_header,
            langs: cfg.lang.clone().map(|l| vec![l]).unwrap_or_else(|| AsrKind::MiMoHttp.languages()),
        }
    }

    async fn transcribe_chunk(&self, wav: Vec<u8>) -> Result<String, AsrError> {
        // §10.3.3② MiMo 协议：audio_url + data URL 内联（与 prism-agent MiMoAsrService 一致）
        let data_url = format!("data:audio/wav;base64,{}", base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &wav));
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let body = serde_json::json!({
            "model": self.model,
            "messages": [{
                "role": "user",
                "content": [{ "type": "audio_url", "audio_url": { "url": data_url } }]
            }],
            "asr_options": { "language": "auto" },
            "max_tokens": 1024,
        });

        let mut req = reqwest::Client::new().post(&url).header("Content-Type", "application/json");
        if !self.api_key.is_empty() {
            if self.use_api_key_header {
                req = req.header("api-key", self.api_key.clone());
            } else {
                req = req.header("Authorization", format!("Bearer {}", self.api_key));
            }
        }

        let resp = req.json(&body).send().await.map_err(|e| AsrError::Network(e.to_string()))?;
        let status = resp.status();
        if status.as_u16() == 401 {
            return Err(AsrError::Unauthorized);
        }
        if status.as_u16() == 429 {
            return Err(AsrError::QuotaExceeded);
        }
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(AsrError::Protocol(format!("HTTP {status}: {text}")));
        }
        let data: serde_json::Value = resp.json().await.map_err(|e| AsrError::Protocol(e.to_string()))?;
        Ok(data["choices"][0]["message"]["content"].as_str().unwrap_or_default().to_string())
    }
}

#[async_trait::async_trait]
impl AsrBackend for OpenAiCompatibleBackend {
    fn kind(&self) -> AsrKind { AsrKind::MiMoHttp }

    fn languages(&self) -> &[String] { &self.langs }

    async fn health_check(&self) -> Result<(), AsrError> {
        // 轻量探测：GET /models（兼容端点多支持）。鉴权头与写入路径保持一致
        let url = format!("{}/models", self.base_url.trim_end_matches('/'));
        let mut req = reqwest::Client::new().get(&url);
        if !self.api_key.is_empty() {
            if self.use_api_key_header {
                req = req.header("api-key", self.api_key.clone());
            } else {
                req = req.header("Authorization", format!("Bearer {}", self.api_key));
            }
        }
        let resp = req.send().await.map_err(|e| AsrError::Network(e.to_string()))?;
        if resp.status().is_success() {
            Ok(())
        } else if resp.status().as_u16() == 401 {
            Err(AsrError::Unauthorized)
        } else {
            Err(AsrError::Protocol(format!("HTTP {}", resp.status())))
        }
    }

    async fn start(
        &mut self,
        mut audio: AudioSource,
        events: AsrEventSink,
    ) -> Result<AsrSessionHandle, AsrError> {
        // 3s 缓冲：16kHz 16bit 单声道 = 32000 字节/秒
        const CHUNK_SECS: usize = 3;
        const BYTES_PER_SEC: usize = 32000;
        const BUF_LEN: usize = CHUNK_SECS * BYTES_PER_SEC;

        let handle = AsrSessionHandle::new();
        let cancel = handle.token();
        let backend = Self {
            base_url: self.base_url.clone(),
            api_key: self.api_key.clone(),
            model: self.model.clone(),
            use_api_key_header: self.use_api_key_header,
            langs: self.langs.clone(),
        };

        tokio::spawn(async move {
            let mut buf: Vec<u8> = Vec::with_capacity(BUF_LEN);
            let mut index: u64 = 0;
            let mut prev_full = String::new();

            while let Some(chunk) = audio.next().await {
                if cancel.is_cancelled() { break; }
                buf.extend_from_slice(&chunk);
                if buf.len() >= BUF_LEN {
                    let wav = pcm_to_wav(&buf);
                    let full = match backend.transcribe_chunk(wav).await {
                        Ok(t) => t,
                        Err(e) => {
                            events.status(&format!("识别错误: {e}"));
                            buf.drain(..);
                            continue;
                        }
                    };
                    // 与上一段做差集：new = full[len(prev):]
                    let new_text = diff_text(&full, &prev_full);
                    if !new_text.is_empty() {
                        events.segment(AsrSegment {
                            index,
                            text: new_text,
                            is_final: true,
                            start_ms: 0,
                            end_ms: 0,
                            language: None,
                            confidence: None,
                            speaker_id: None,
                        });
                        index += 1;
                    }
                    prev_full = full;
                    buf.drain(..);
                }
            }
            // 尾部剩余音频（<3s）
            if !buf.is_empty() {
                let wav = pcm_to_wav(&buf);
                if let Ok(full) = backend.transcribe_chunk(wav).await {
                    let new_text = diff_text(&full, &prev_full);
                    if !new_text.is_empty() {
                        events.segment(AsrSegment {
                            index,
                            text: new_text,
                            is_final: true,
                            start_ms: 0,
                            end_ms: 0,
                            language: None,
                            confidence: None,
                            speaker_id: None,
                        });
                    }
                }
            }
        });

        Ok(handle)
    }

    async fn stop(&mut self) -> Result<Vec<AsrSegment>, AsrError> {
        Ok(Vec::new()) // 流式后端：结果已通过回调推送
    }
}

// ── WhisperApiBackend（OpenAI Whisper：multipart 分片上传） ──

pub struct WhisperApiBackend {
    base_url: String,
    api_key: String,
    model: String,
    langs: Vec<String>,
}

impl WhisperApiBackend {
    pub fn new(cfg: &AsrBackendConfig) -> Self {
        Self {
            base_url: cfg.base_url.clone().unwrap_or_else(|| "https://api.openai.com/v1".into()),
            api_key: cfg.api_key.clone().unwrap_or_default(),
            model: cfg.model.clone().unwrap_or_else(|| "whisper-1".into()),
            langs: cfg.lang.clone().map(|l| vec![l]).unwrap_or_else(|| AsrKind::WhisperApi.languages()),
        }
    }

    async fn transcribe_wav(&self, wav: Vec<u8>) -> Result<String, AsrError> {
        let url = format!("{}/audio/transcriptions", self.base_url.trim_end_matches('/'));
        let part = reqwest::multipart::Part::bytes(wav)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| AsrError::Protocol(e.to_string()))?;
        let form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("model", self.model.clone())
            .text("response_format", "text");

        let mut req = reqwest::Client::new().post(&url).multipart(form);
        if !self.api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", self.api_key));
        }
        let resp = req.send().await.map_err(|e| AsrError::Network(e.to_string()))?;
        let status = resp.status();
        if status.as_u16() == 401 { return Err(AsrError::Unauthorized); }
        if status.as_u16() == 429 { return Err(AsrError::QuotaExceeded); }
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(AsrError::Protocol(format!("HTTP {status}: {text}")));
        }
        Ok(resp.text().await.unwrap_or_default().trim().to_string())
    }
}

#[async_trait::async_trait]
impl AsrBackend for WhisperApiBackend {
    fn kind(&self) -> AsrKind { AsrKind::WhisperApi }

    fn languages(&self) -> &[String] { &self.langs }

    async fn health_check(&self) -> Result<(), AsrError> {
        let url = format!("{}/models", self.base_url.trim_end_matches('/'));
        let mut req = reqwest::Client::new().get(&url);
        if !self.api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", self.api_key));
        }
        let resp = req.send().await.map_err(|e| AsrError::Network(e.to_string()))?;
        if resp.status().is_success() { Ok(()) }
        else if resp.status().as_u16() == 401 { Err(AsrError::Unauthorized) }
        else { Err(AsrError::Protocol(format!("HTTP {}", resp.status()))) }
    }

    async fn start(
        &mut self,
        mut audio: AudioSource,
        events: AsrEventSink,
    ) -> Result<AsrSessionHandle, AsrError> {
        // 15s 分片 + 1s 重叠（= 480000 字节/15s）
        const SLICE_BYTES: usize = 15 * 32000;
        const OVERLAP_BYTES: usize = 1 * 32000;

        let handle = AsrSessionHandle::new();
        let cancel = handle.token();
        let backend = Self {
            base_url: self.base_url.clone(),
            api_key: self.api_key.clone(),
            model: self.model.clone(),
            langs: self.langs.clone(),
        };

        tokio::spawn(async move {
            let mut buf: Vec<u8> = Vec::new();
            let mut index: u64 = 0;
            let mut prev_text = String::new();

            while let Some(chunk) = audio.next().await {
                if cancel.is_cancelled() { break; }
                buf.extend_from_slice(&chunk);
                if buf.len() >= SLICE_BYTES {
                    // 取整片前 15s，保留尾部 1s 重叠
                    let take = SLICE_BYTES;
                    let slice = buf[..take].to_vec();
                    buf.drain(..take - OVERLAP_BYTES.min(take));
                    if let Ok(text) = backend.transcribe_wav(pcm_to_wav(&slice)).await {
                        if !text.is_empty() {
                            // 重叠 1s ≈ 20 字（中文），去重后发送，避免拼接文本重复（§10.3.3⑤）
                            let text = dedup_overlap(&prev_text, &text, 20);
                            if !text.is_empty() {
                                events.segment(AsrSegment {
                                    index, text: text.clone(), is_final: true,
                                    start_ms: 0, end_ms: 0,
                                    language: None, confidence: None, speaker_id: None,
                                });
                                index += 1;
                            }
                            prev_text = text;
                        }
                    }
                }
            }
            if !buf.is_empty() {
                if let Ok(text) = backend.transcribe_wav(pcm_to_wav(&buf)).await {
                    if !text.is_empty() {
                        let text = dedup_overlap(&prev_text, &text, 20);
                        if !text.is_empty() {
                            events.segment(AsrSegment {
                                index, text, is_final: true,
                                start_ms: 0, end_ms: 0,
                                language: None, confidence: None, speaker_id: None,
                            });
                        }
                    }
                }
            }
        });

        Ok(handle)
    }

    async fn stop(&mut self) -> Result<Vec<AsrSegment>, AsrError> {
        Ok(Vec::new())
    }
}

// ── DashScopeFunasrBackend（WebSocket 双工流式，新版 api-ws 协议） ──
// 协议对齐 huiji FunASRRealtimeService 实测：
//   wss://dashscope.aliyuncs.com/api-ws/v1/inference + Authorization: Bearer
//   run-task（含 task_id）→ task-started → result-generated（payload.output.sentence）
//   → finish-task → task-finished

pub struct DashScopeFunasrBackend {
    api_key: String,
    model: String,
    langs: Vec<String>,
}

impl DashScopeFunasrBackend {
    pub fn new(cfg: &AsrBackendConfig) -> Self {
        Self {
            api_key: cfg.api_key.clone().unwrap_or_default(),
            model: cfg.model.clone().unwrap_or_else(|| "fun-asr-realtime".into()),
            langs: vec!["zh".into()],
        }
    }
}

#[async_trait::async_trait]
impl AsrBackend for DashScopeFunasrBackend {
    fn kind(&self) -> AsrKind { AsrKind::DashScopeFunasr }

    fn languages(&self) -> &[String] { &self.langs }

    async fn health_check(&self) -> Result<(), AsrError> {
        if self.api_key.is_empty() {
            return Err(AsrError::Unauthorized);
        }
        // WebSocket 无法轻量探测，仅校验 key 非空
        Ok(())
    }

    async fn start(
        &mut self,
        mut audio: AudioSource,
        events: AsrEventSink,
    ) -> Result<AsrSessionHandle, AsrError> {
        use tokio_tungstenite::connect_async;
        use tokio_tungstenite::tungstenite::Message as WsMessage;

        const URL: &str = "wss://dashscope.aliyuncs.com/api-ws/v1/inference";
        let api_key = self.api_key.clone();
        let model = self.model.clone();

        let handle = AsrSessionHandle::new();
        let cancel = handle.token();

        tokio::spawn(async move {
            // 鉴权：DashScope 要求 Authorization: Bearer header；
            // tokio-tungstenite 通过 IntoClientRequest 自定义 request 携带 header（huiji 同方案）。
            let mut request = match tokio_tungstenite::tungstenite::client::IntoClientRequest::into_client_request(URL) {
                Ok(req) => req,
                Err(e) => { events.status(&format!("构造请求失败: {e}")); return; }
            };
            if !api_key.is_empty() {
                let auth = format!("Bearer {api_key}");
                if let Ok(val) = http::HeaderValue::from_str(&auth) {
                    request.headers_mut().insert(http::header::AUTHORIZATION, val);
                }
            }
            let (mut ws, _resp) = match connect_async(request).await {
                Ok(v) => v,
                Err(e) => {
                    events.status(&format!("WebSocket 连接失败: {e}"));
                    return;
                }
            };

            // 1. run-task（含 task_id）
            let task_id = uuid::Uuid::new_v4().to_string();
            let run_task = serde_json::json!({
                "header": { "action": "run-task", "task_id": task_id, "streaming": "duplex" },
                "payload": {
                    "task_group": "audio",
                    "task": "asr",
                    "function": "recognition",
                    "model": model,
                    "parameters": {
                        "format": "pcm",
                        "sample_rate": 16000,
                        "speaker_diarization_enabled": true
                    },
                    "input": {}
                }
            });
            if ws.send(WsMessage::Text(run_task.to_string().into())).await.is_err() {
                events.status("发送 run-task 失败");
                return;
            }
            events.status("connecting");

            let mut index: u64 = 0;
            let mut task_started = false;
            let mut finished = false;
            let mut task_finished_received = false;

            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        let finish = serde_json::json!({
                            "header": { "action": "finish-task", "task_id": task_id },
                            "payload": { "input": {} }
                        });
                        let _ = ws.send(WsMessage::Text(finish.to_string().into())).await;
                        let _ = ws.close(None).await;
                        break;
                    }
                    chunk = audio.next() => {
                        match chunk {
                            Some(pcm) => {
                                if task_started {
                                    // 二进制帧直接发送（新版协议音频为裸 PCM 二进制）
                                    if ws.send(WsMessage::Binary(pcm.into())).await.is_err() {
                                        events.status("发送音频失败");
                                        break;
                                    }
                                } else {
                                    // task 未启动：音频由 AudioStreamManager pending 缓冲，
                                    // 此处直接丢弃（消费端会在 task-started 后继续推送）
                                }
                            }
                            None => {
                                let finish = serde_json::json!({
                                    "header": { "action": "finish-task", "task_id": task_id },
                                    "payload": { "input": {} }
                                });
                                let _ = ws.send(WsMessage::Text(finish.to_string().into())).await;
                                finished = true;
                            }
                        }
                    }
                }

                if finished { break; }

                // 非阻塞读结果
                while let Ok(Some(msg)) = tokio::time::timeout(std::time::Duration::from_millis(10), ws.next()).await {
                    match msg {
                        Ok(WsMessage::Text(text)) => {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                                let event = v["header"]["action"].as_str().or_else(|| v["header"]["event"].as_str()).unwrap_or("");
                                match event {
                                    "task-started" => {
                                        task_started = true;
                                        events.status("recognizing");
                                    }
                                    "result-generated" => {
                                        let payload = &v["payload"];
                                        let output = &payload["output"];
                                        // 两种格式：output.sentence 或 output.text
                                        let sentence = &output["sentence"];
                                        let (seg_text, is_end, speaker_id) = if sentence.is_object() {
                                            (
                                                sentence["text"].as_str().unwrap_or("").to_string(),
                                                sentence["sentence_end"].as_bool().unwrap_or(false),
                                                sentence["speaker_id"].as_str().map(String::from),
                                            )
                                        } else {
                                            (
                                                output["text"].as_str().unwrap_or("").to_string(),
                                                output["sentence_end"].as_bool().unwrap_or(false),
                                                output["speaker_id"].as_str().map(String::from),
                                            )
                                        };
                                        if !seg_text.is_empty() {
                                            events.segment(AsrSegment {
                                                index,
                                                text: seg_text,
                                                is_final: is_end,
                                                start_ms: 0,
                                                end_ms: 0,
                                                language: None,
                                                confidence: None,
                                                speaker_id: speaker_id.and_then(|s| s.parse().ok()),
                                            });
                                            if is_end { index += 1; }
                                        }
                                    }
                                    "task-finished" => {
                                        events.status("stopped");
                                        finished = true;
                                        task_finished_received = true;
                                        break;
                                    }
                                    "task-failed" | "failed" | "error" => {
                                        events.status(&format!("ASR 错误: {text}"));
                                    }
                                    _ => {}
                                }
                            }
                        }
                        Ok(WsMessage::Close(_)) => break,
                        Err(e) => {
                            events.status(&format!("WebSocket 错误: {e}"));
                            break;
                        }
                        _ => {}
                    }
                }
            }

            // 排空阶段：finish-task 已发送后，服务端仍会推送尾部定稿与 task-finished。
            // 若直接退出循环，尾部 result-generated 会丢失（§10.3.3 协议：需读到 task-finished 为止）。
            if finished && !task_finished_received {
                while let Ok(Some(msg)) = tokio::time::timeout(std::time::Duration::from_secs(5), ws.next()).await {
                    match msg {
                        Ok(WsMessage::Text(text)) => {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                                let event = v["header"]["action"].as_str().or_else(|| v["header"]["event"].as_str()).unwrap_or("");
                                match event {
                                    "result-generated" => {
                                        let output = &v["payload"]["output"];
                                        let sentence = &output["sentence"];
                                        let (seg_text, is_end, speaker_id) = if sentence.is_object() {
                                            (
                                                sentence["text"].as_str().unwrap_or("").to_string(),
                                                sentence["sentence_end"].as_bool().unwrap_or(false),
                                                sentence["speaker_id"].as_str().map(String::from),
                                            )
                                        } else {
                                            (
                                                output["text"].as_str().unwrap_or("").to_string(),
                                                output["sentence_end"].as_bool().unwrap_or(false),
                                                output["speaker_id"].as_str().map(String::from),
                                            )
                                        };
                                        if !seg_text.is_empty() {
                                            events.segment(AsrSegment {
                                                index,
                                                text: seg_text,
                                                is_final: is_end,
                                                start_ms: 0,
                                                end_ms: 0,
                                                language: None,
                                                confidence: None,
                                                speaker_id: speaker_id.and_then(|s| s.parse().ok()),
                                            });
                                            if is_end { index += 1; }
                                        }
                                    }
                                    "task-finished" => {
                                        events.status("stopped");
                                        break;
                                    }
                                    "task-failed" | "failed" | "error" => {
                                        events.status(&format!("ASR 错误: {text}"));
                                    }
                                    _ => {}
                                }
                            }
                        }
                        Ok(WsMessage::Close(_)) => break,
                        Err(_) => break,
                        _ => {}
                    }
                }
            }
        });

        Ok(handle)
    }

    async fn stop(&mut self) -> Result<Vec<AsrSegment>, AsrError> {
        Ok(Vec::new())
    }
}

// ── LocalFunasrWsBackend（本地 FunASR WebSocket） ─────────

pub struct LocalFunasrWsBackend {
    base_url: String,
    langs: Vec<String>,
}

impl LocalFunasrWsBackend {
    pub fn new(cfg: &AsrBackendConfig) -> Self {
        Self {
            base_url: cfg.base_url.clone().unwrap_or_else(|| "ws://localhost:10095".into()),
            langs: vec!["zh".into()],
        }
    }
}

#[async_trait::async_trait]
impl AsrBackend for LocalFunasrWsBackend {
    fn kind(&self) -> AsrKind { AsrKind::LocalFunasrWs }

    fn languages(&self) -> &[String] { &self.langs }

    async fn health_check(&self) -> Result<(), AsrError> {
        use tokio_tungstenite::connect_async;
        let url = format!("{}", self.base_url);
        connect_async(&url)
            .await
            .map(|_| ())
            .map_err(|e| AsrError::Network(e.to_string()))
    }

    async fn start(
        &mut self,
        mut audio: AudioSource,
        events: AsrEventSink,
    ) -> Result<AsrSessionHandle, AsrError> {
        use tokio_tungstenite::connect_async;
        use tokio_tungstenite::tungstenite::Message as WsMessage;

        let url = self.base_url.clone();
        let handle = AsrSessionHandle::new();
        let cancel = handle.token();

        tokio::spawn(async move {
            let (mut ws, _resp) = match connect_async(&url).await {
                Ok(v) => v,
                Err(e) => { events.status(&format!("连接失败: {e}")); return; }
            };
            let init = serde_json::json!({
                "mode": "2pass", "chunk_size": [5,10,5],
                "wav_name": "meeting", "is_speaking": true, "itn": true
            });
            let _ = ws.send(WsMessage::Text(init.to_string().into())).await;
            events.status("connected");

            let mut index: u64 = 0;
            while let Some(chunk) = audio.next().await {
                if cancel.is_cancelled() { break; }
                if ws.send(WsMessage::Binary(chunk.into())).await.is_err() { break; }
                // 读离线结果
                while let Ok(Some(msg)) = tokio::time::timeout(std::time::Duration::from_millis(5), ws.next()).await {
                    if let Ok(WsMessage::Text(text)) = msg {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                            let seg_text = v["text"].as_str().unwrap_or("").to_string();
                            let is_final = v["is_final"].as_bool().unwrap_or(false);
                            if !seg_text.is_empty() {
                                events.segment(AsrSegment {
                                    index, text: seg_text, is_final,
                                    start_ms: 0, end_ms: 0,
                                    language: None, confidence: None, speaker_id: None,
                                });
                                if is_final { index += 1; }
                            }
                        }
                    }
                }
            }
        });

        Ok(handle)
    }

    async fn stop(&mut self) -> Result<Vec<AsrSegment>, AsrError> {
        Ok(Vec::new())
    }
}

// ── SherpaOnnxBackend（真实本地推理：SenseVoice + Silero VAD） ──
// 使用 sherpa-rs crate（onnxruntime 由 download-binaries feature 自动下载）。
// 仅当启用 `sherpa-native` feature 时编译（需本机 libclang + 网络下载 onnxruntime）；
// 默认构建下 SherpaOnnx 由 LocalNativeBackend 骨架承接（见 builtin_register）。
// 模型：model_path 指向含 model.int8.onnx + tokens.txt 的任意目录（自动递归查找），
// 不限定内置清单；Silero VAD 可选（silero_vad.onnx 与模型同目录或上级目录）。

#[cfg(feature = "sherpa-native")]
pub struct SherpaOnnxBackend {
    model_path: Option<String>,
    langs: Vec<String>,
}

#[cfg(feature = "sherpa-native")]
impl SherpaOnnxBackend {
    pub fn new(cfg: &AsrBackendConfig) -> Self {
        Self {
            model_path: cfg.model_path.clone(),
            langs: cfg.lang.clone().map(|l| vec![l]).unwrap_or_else(|| AsrKind::SherpaOnnx.languages()),
        }
    }

    /// 定位模型文件：递归查找 model.onnx / model.int8.onnx / model_quant.onnx
    fn find_model(&self, base: &std::path::Path) -> Option<std::path::PathBuf> {
        let candidates = ["model.int8.onnx", "model.onnx", "model_quant.onnx"];
        find_file_recursive(base, &candidates, 3)
    }

    /// 定位 tokens.txt
    fn find_tokens(&self, base: &std::path::Path) -> Option<std::path::PathBuf> {
        find_file_recursive(base, &["tokens.txt", "tokens.json"], 3)
    }

    /// 定位 Silero VAD（可选）
    fn find_vad(&self, base: &std::path::Path) -> Option<std::path::PathBuf> {
        find_file_recursive(base, &["silero_vad.onnx"], 3)
    }

    fn resolve(&self) -> Result<(std::path::PathBuf, std::path::PathBuf, Option<std::path::PathBuf>), AsrError> {
        let Some(path) = &self.model_path else {
            return Err(AsrError::ModelNotFound("SherpaOnnx 未配置模型路径（model_path）".into()));
        };
        let dir = std::path::Path::new(path);
        if !dir.exists() {
            return Err(AsrError::ModelNotFound(format!("模型目录不存在: {path}")));
        }
        let model = self.find_model(dir).ok_or_else(|| {
            AsrError::ModelNotFound(format!("模型目录中未找到 model.onnx/model.int8.onnx（{path}）"))
        })?;
        let tokens = self.find_tokens(dir).ok_or_else(|| {
            AsrError::ModelNotFound(format!("模型目录中未找到 tokens.txt/tokens.json（{path}）"))
        })?;
        let vad = self.find_vad(dir);
        Ok((model, tokens, vad))
    }
}

#[cfg(feature = "sherpa-native")]
#[async_trait::async_trait]
impl AsrBackend for SherpaOnnxBackend {
    fn kind(&self) -> AsrKind { AsrKind::SherpaOnnx }

    fn languages(&self) -> &[String] { &self.langs }

    async fn health_check(&self) -> Result<(), AsrError> {
        self.resolve().map(|_| ())
    }

    async fn start(
        &mut self,
        mut audio: AudioSource,
        events: AsrEventSink,
    ) -> Result<AsrSessionHandle, AsrError> {
        let (model_path, tokens_path, vad_path) = self.resolve()?;
        let langs = self.langs.clone();
        let handle = AsrSessionHandle::new();
        let cancel = handle.token();

        tokio::task::spawn_blocking(move || {
            // 1. 初始化 SenseVoice 识别器（对齐 huiji sherpa_adapter 配置）
            let recognizer = match sherpa_rs::sense_voice::SenseVoiceRecognizer::new(
                sherpa_rs::sense_voice::SenseVoiceConfig {
                    model: model_path.to_string_lossy().into_owned(),
                    language: "auto".into(),
                    use_itn: true,
                    provider: Some("cpu".into()),
                    num_threads: Some(4),
                    debug: false,
                    tokens: tokens_path.to_string_lossy().into_owned(),
                },
            ) {
                Ok(r) => r,
                Err(e) => {
                    events.status(&format!("SenseVoice 初始化失败: {e}"));
                    return;
                }
            };

            // 2. 可选 Silero VAD
            let mut vad = vad_path.as_ref().and_then(|vp| {
                sherpa_rs::silero_vad::SileroVad::new(
                    sherpa_rs::silero_vad::SileroVadConfig {
                        model: vp.to_string_lossy().into_owned(),
                        min_silence_duration: 0.5,
                        min_speech_duration: 0.25,
                        max_speech_duration: 30.0,
                        threshold: 0.5,
                        sample_rate: 16000,
                        window_size: 512,
                        provider: Some("cpu".into()),
                        num_threads: Some(2),
                        debug: false,
                    },
                    60.0,
                )
                .ok()
            });

            let mut recognizer = recognizer;
            let mut sample_index: u64 = 0;
            let mut prev_text = String::new();

            // 3. 异步块流 → 阻塞消费（用 futures executor 轮询）
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    events.status(&format!("runtime 创建失败: {e}"));
                    return;
                }
            };

            rt.block_on(async {
                while let Some(pcm) = audio.next().await {
                    if cancel.is_cancelled() { break; }
                    // PCM i16 LE → f32 [-1,1]
                    let samples: Vec<f32> = pcm
                        .chunks_exact(2)
                        .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
                        .collect();
                    if samples.is_empty() { continue; }

                    // VAD 模式：喂 VAD 提取语音段；无 VAD：累积后整段识别
                    if let Some(vad) = vad.as_mut() {
                        vad.accept_waveform(samples);
                        while !vad.is_empty() {
                            let segment = vad.front();
                            vad.pop();
                            if segment.samples.is_empty() { continue; }
                            let result = recognizer.transcribe(16000, &segment.samples);
                            if !result.text.trim().is_empty() {
                                // 差集：只发新内容
                                let new_text = if result.text.starts_with(&prev_text) {
                                    result.text[prev_text.len()..].trim().to_string()
                                } else {
                                    result.text.clone()
                                };
                                if !new_text.is_empty() {
                                    events.segment(AsrSegment {
                                        index: sample_index,
                                        text: new_text,
                                        is_final: true,
                                        start_ms: 0,
                                        end_ms: 0,
                                        language: Some(result.lang.clone()),
                                        confidence: None,
                                        speaker_id: None,
                                    });
                                    sample_index += 1;
                                }
                                prev_text = result.text.clone();
                            }
                        }
                    } else {
                        // 无 VAD：5s 累积一次整段识别（约 80000 样本）
                        let mut acc = samples;
                        while acc.len() >= 16000 * 5 {
                            let seg: Vec<f32> = acc.drain(..16000 * 5).collect();
                            let result = recognizer.transcribe(16000, &seg);
                            if !result.text.trim().is_empty() {
                                events.segment(AsrSegment {
                                    index: sample_index,
                                    text: result.text.clone(),
                                    is_final: true,
                                    start_ms: 0,
                                    end_ms: 0,
                                    language: Some(result.lang.clone()),
                                    confidence: None,
                                    speaker_id: None,
                                });
                                sample_index += 1;
                            }
                        }
                    }
                }
            });
            let _ = langs;
        });

        Ok(handle)
    }

    async fn stop(&mut self) -> Result<Vec<AsrSegment>, AsrError> {
        Ok(Vec::new())
    }
}

// ── VoskBackend（真实本地推理，§10.3.3⑥） ──
// 使用 vosk crate（vosk-sys 静态链接 libvosk）。仅当启用 `vosk-native` feature 时编译
// （需本机预装 Vosk 库，无自动下载，故默认关闭）；默认构建下 Vosk 由 LocalNativeBackend 骨架承接。
// 模型：model_path 指向 Vosk 模型目录（含 conf/model.conf + am/final.mdl 等）。

#[cfg(feature = "vosk-native")]
pub struct VoskBackend {
    model_path: Option<String>,
    langs: Vec<String>,
}

#[cfg(feature = "vosk-native")]
impl VoskBackend {
    pub fn new(cfg: &AsrBackendConfig) -> Self {
        Self {
            model_path: cfg.model_path.clone(),
            langs: cfg.lang.clone().map(|l| vec![l]).unwrap_or_else(|| AsrKind::Vosk.languages()),
        }
    }
}

#[cfg(feature = "vosk-native")]
#[async_trait::async_trait]
impl AsrBackend for VoskBackend {
    fn kind(&self) -> AsrKind { AsrKind::Vosk }

    fn languages(&self) -> &[String] { &self.langs }

    async fn health_check(&self) -> Result<(), AsrError> {
        let Some(path) = &self.model_path else {
            return Err(AsrError::ModelNotFound("Vosk 未配置模型路径，请在配置中指定 model_path".into()));
        };
        let dir = std::path::Path::new(path);
        if !dir.exists() {
            return Err(AsrError::ModelNotFound(format!("模型目录不存在: {path}")));
        }
        // Vosk 模型关键文件：am/final.mdl 与 conf/model.conf
        if !dir.join("am").join("final.mdl").exists() || !dir.join("conf").join("model.conf").exists() {
            return Err(AsrError::ModelNotFound(format!(
                "{} 目录不是有效的 Vosk 模型（缺少 am/final.mdl 或 conf/model.conf）",
                dir.display()
            )));
        }
        Ok(())
    }

    async fn start(
        &mut self,
        mut audio: AudioSource,
        events: AsrEventSink,
    ) -> Result<AsrSessionHandle, AsrError> {
        self.health_check().await?;
        let model_path = self.model_path.clone().unwrap();
        let langs = self.langs.clone();
        let handle = AsrSessionHandle::new();
        let cancel = handle.token();

        tokio::spawn(async move {
            // 模型加载失败直接报状态（无句柄可回传，只能通过事件提示）
            let Some(model) = vosk::Model::new(&model_path) else {
                events.status("Vosk 模型加载失败");
                return;
            };
            let Some(mut recognizer) = vosk::Recognizer::new(&model, 16000.0) else {
                events.status("Vosk Recognizer 创建失败");
                return;
            };

            let mut index: u64 = 0;
            let mut seg_buf: Vec<i16> = Vec::new();
            while let Some(chunk) = audio.next().await {
                if cancel.is_cancelled() { break; }
                // PCM 块：16kHz 16bit 小端 → i16 样本
                let samples: Vec<i16> = chunk
                    .chunks_exact(2)
                    .map(|b| i16::from_le_bytes([b[0], b[1]]))
                    .collect();
                seg_buf.extend_from_slice(&samples);
                // 攒够 ~200ms 再喂（减少调用次数），vosk 内部有端点检测
                if seg_buf.len() >= 3200 {
                    match recognizer.accept_waveform(&seg_buf) {
                        Ok(vosk::DecodingState::Finalized) => {
                            let result = recognizer.result();
                            let text = match &result {
                                vosk::CompleteResult::Single(s) => s.text.trim().to_string(),
                                vosk::CompleteResult::Multiple(m) => m
                                    .alternatives
                                    .first()
                                    .map(|a| a.text.trim().to_string())
                                    .unwrap_or_default(),
                            };
                            if !text.is_empty() {
                                events.segment(AsrSegment {
                                    index, text: text.clone(), is_final: true,
                                    start_ms: 0, end_ms: 0,
                                    language: langs.first().cloned(), confidence: None, speaker_id: None,
                                });
                                index += 1;
                            }
                        }
                        Ok(_) => {
                            // Running：中间结果（灰显渲染）
                            let partial = recognizer.partial_result();
                            let text = partial.partial.trim().to_string();
                            if !text.is_empty() {
                                events.segment(AsrSegment {
                                    index, text, is_final: false,
                                    start_ms: 0, end_ms: 0,
                                    language: langs.first().cloned(), confidence: None, speaker_id: None,
                                });
                            }
                        }
                        Err(_) => events.status("Vosk 解码错误"),
                    }
                    seg_buf.clear();
                }
            }
            // 流结束：取最终结果
            let final_result = recognizer.final_result();
            let text = match &final_result {
                vosk::CompleteResult::Single(s) => s.text.trim().to_string(),
                vosk::CompleteResult::Multiple(m) => m
                    .alternatives
                    .first()
                    .map(|a| a.text.trim().to_string())
                    .unwrap_or_default(),
            };
            if !text.is_empty() {
                events.segment(AsrSegment {
                    index, text, is_final: true,
                    start_ms: 0, end_ms: 0,
                    language: langs.first().cloned(), confidence: None, speaker_id: None,
                });
            }
            events.status("stopped");
        });

        Ok(handle)
    }

    async fn stop(&mut self) -> Result<Vec<AsrSegment>, AsrError> {
        Ok(Vec::new())
    }
}

#[cfg(feature = "sherpa-native")]
fn find_file_recursive(dir: &std::path::Path, names: &[&str], depth: usize) -> Option<std::path::PathBuf> {
    if depth == 0 {
        return None;
    }
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if let Some(found) = find_file_recursive(&p, names, depth - 1) {
                return Some(found);
            }
        } else if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
            if names.contains(&name) {
                return Some(p);
            }
        }
    }
    None
}

// ── AzureSpeechBackend（§10.3.3 ⑦：WebSocket 双工流式，支持说话人分离） ──
// 端点: wss://{region}.stt.speech.microsoft.com/speech/recognition/conversation/cognitiveservices/v1
// 鉴权: Header `Ocp-Apim-Subscription-Key`
// 协议: 首帧 speech.config JSON → 二进制音频帧（16kHz PCM）→ 结束发 {"end":true}
// 接收: speechHypothesis（中间结果）/ speechFragment（定稿，含 speakerId 说话人分离）
// region 由 base_url 提供，key 由 api_key 提供

pub struct AzureSpeechBackend {
    base_url: String,
    api_key: String,
    langs: Vec<String>,
}

impl AzureSpeechBackend {
    pub fn new(cfg: &AsrBackendConfig) -> Self {
        Self {
            base_url: cfg.base_url.clone().unwrap_or_else(|| "https://eastasia.stt.speech.microsoft.com".into()),
            api_key: cfg.api_key.clone().unwrap_or_default(),
            langs: cfg.lang.clone().map(|l| vec![l]).unwrap_or_else(|| AsrKind::AzureSpeech.languages()),
        }
    }
}

#[async_trait::async_trait]
impl AsrBackend for AzureSpeechBackend {
    fn kind(&self) -> AsrKind { AsrKind::AzureSpeech }

    fn languages(&self) -> &[String] { &self.langs }

    async fn health_check(&self) -> Result<(), AsrError> {
        if self.api_key.is_empty() {
            return Err(AsrError::Unauthorized);
        }
        if !self.base_url.contains(".stt.speech.microsoft.com") {
            return Err(AsrError::Protocol("base_url 应为 https://{region}.stt.speech.microsoft.com".into()));
        }
        Ok(())
    }

    async fn start(
        &mut self,
        mut audio: AudioSource,
        events: AsrEventSink,
    ) -> Result<AsrSessionHandle, AsrError> {
        // WS 流式：边收音频边出结果（录音场景增量；重转写场景静态流同样适用）
        let handle = AsrSessionHandle::new();
        let cancel = handle.token();
        let base_url = self.base_url.clone();
        let api_key = self.api_key.clone();
        let lang = self.langs.first().cloned().unwrap_or_else(|| "zh-CN".into());

        tokio::spawn(async move {
            use tokio_tungstenite::connect_async;
            use tokio_tungstenite::tungstenite::Message as WsMessage;

            let url = format!(
                "{}/speech/recognition/conversation/cognitiveservices/v1?language={}&format=detailed&profanity=raw",
                base_url.trim_end_matches('/').replace("https://", "wss://"),
                lang
            );

            // 鉴权：Azure 要求 Ocp-Apim-Subscription-Key header（IntoClientRequest 携带自定义 header）
            let mut request =
                match tokio_tungstenite::tungstenite::client::IntoClientRequest::into_client_request(&url) {
                    Ok(req) => req,
                    Err(e) => {
                        events.status(&format!("构造请求失败: {e}"));
                        return;
                    }
                };
            if let Ok(val) = http::HeaderValue::from_str(&api_key) {
                request.headers_mut().insert("Ocp-Apim-Subscription-Key", val);
            }
            let (mut ws, _resp) = match connect_async(request).await {
                Ok(v) => v,
                Err(e) => {
                    events.status(&format!("Azure WebSocket 连接失败: {e}"));
                    return;
                }
            };

            // 1. 首帧 speech.config（含客户端标识）
            let config = serde_json::json!({
                "context": { "system": { "name": "PrismAgent", "version": "0.1.0" } }
            });
            if ws.send(WsMessage::Text(config.to_string().into())).await.is_err() {
                events.status("发送 speech.config 失败");
                return;
            }
            events.status("connecting");

            let mut index: u64 = 0;

            // 文本帧处理（主循环与排空循环共用）
            // 返回 true 表示应结束（endDetected/Close 语义）
            let mut handle_frame = |text: String, finished: &mut bool| {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                    let ty = v["type"].as_str().unwrap_or("");
                    match ty {
                        // 中间结果（持续修正）
                        "speechHypothesis" => {
                            let t = v["text"].as_str().unwrap_or("");
                            if !t.is_empty() {
                                events.segment(AsrSegment {
                                    index,
                                    text: t.to_string(),
                                    is_final: false,
                                    start_ms: v["offset"].as_u64().unwrap_or(0) / 10_000,
                                    end_ms: 0,
                                    language: Some(lang.clone()),
                                    confidence: None,
                                    speaker_id: None,
                                });
                            }
                        }
                        // 定稿（含说话人分离 speakerId）
                        "speechFragment" => {
                            let t = v["text"].as_str().unwrap_or("");
                            if !t.is_empty() {
                                let speaker_id = v["speakerId"]
                                    .as_u64()
                                    .or_else(|| v["speakerId"].as_str().and_then(|s| s.parse().ok()))
                                    .map(|s| s as u32);
                                events.segment(AsrSegment {
                                    index,
                                    text: t.to_string(),
                                    is_final: true,
                                    start_ms: v["offset"].as_u64().unwrap_or(0) / 10_000,
                                    end_ms: v["duration"].as_u64().unwrap_or(0) / 10_000,
                                    language: Some(lang.clone()),
                                    confidence: None,
                                    speaker_id,
                                });
                                index += 1;
                            }
                        }
                        "speech.endDetected" | "turn.end" => {
                            *finished = true;
                        }
                        "speech.phrase" => {
                            // 兼容非 conversation 端点返回（标准 STT 短语）
                            let t = v["result"]["DisplayText"].as_str().unwrap_or("");
                            if !t.is_empty() {
                                events.segment(AsrSegment {
                                    index,
                                    text: t.to_string(),
                                    is_final: true,
                                    start_ms: 0,
                                    end_ms: 0,
                                    language: Some(lang.clone()),
                                    confidence: None,
                                    speaker_id: None,
                                });
                                index += 1;
                            }
                            *finished = true;
                        }
                        _ => {}
                    }
                }
            };

            let mut finished = false;
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        let _ = ws.send(WsMessage::Text(r#"{"end":true}"#.into())).await;
                        let _ = ws.close(None).await;
                        break;
                    }
                    chunk = audio.next() => {
                        match chunk {
                            Some(pcm) => {
                                if ws.send(WsMessage::Binary(pcm.into())).await.is_err() {
                                    events.status("发送音频失败");
                                    break;
                                }
                            }
                            None => {
                                // 音频流结束：通知服务端语音结束，然后排空读——
                                // 最终定稿（speechFragment）在 end 之后到达，立即退出会丢尾部结果
                                let _ = ws.send(WsMessage::Text(r#"{"end":true}"#.into())).await;
                                let drain_deadline = std::time::Instant::now()
                                    + std::time::Duration::from_secs(3);
                                while std::time::Instant::now() < drain_deadline {
                                    match tokio::time::timeout(
                                        std::time::Duration::from_millis(200),
                                        ws.next(),
                                    ).await {
                                        Ok(Some(Ok(WsMessage::Text(text)))) => {
                                            handle_frame(text, &mut finished);
                                            if finished { break; }
                                        }
                                        Ok(Some(Ok(WsMessage::Close(_)))) | Ok(None) => break,
                                        Ok(Some(Err(e))) => {
                                            events.status(&format!("Azure WebSocket 错误: {e}"));
                                            break;
                                        }
                                        _ => {} // 超时未到 deadline → 继续等
                                    }
                                }
                                finished = true;
                            }
                        }
                    }
                }
                if finished { break; }

                // 非阻塞读结果
                while let Ok(Some(msg)) = tokio::time::timeout(
                    std::time::Duration::from_millis(10),
                    ws.next(),
                ).await {
                    match msg {
                        Ok(WsMessage::Text(text)) => handle_frame(text, &mut finished),
                        Ok(WsMessage::Close(_)) => {
                            finished = true;
                            break;
                        }
                        Err(e) => {
                            events.status(&format!("Azure WebSocket 错误: {e}"));
                            break;
                        }
                        _ => {}
                    }
                }
            }
            events.status("stopped");
        });

        Ok(handle)
    }

    async fn stop(&mut self) -> Result<Vec<AsrSegment>, AsrError> {
        Ok(Vec::new())
    }
}

// ── 本地推理后端骨架（SherpaOnnx / Vosk / AzureSpeech） ────
// 本地推理引擎（sherpa-rs / vosk-rs）依赖大体积原生库，接入时机见设计 §10.3.9 备注。
// 此处提供可运行的骨架：校验用户指定的任意 model_path，识别相关文件是否就绪，
// 给出可读错误；模型下载/路径由 model_manager 与用户配置共同驱动，不写死目录。

pub struct LocalNativeBackend {
    kind: AsrKind,
    model_path: Option<String>,
    langs: Vec<String>,
    /// 期望存在的模型相关文件（相对 model_path 或任意存在的子文件）
    expected: &'static [&'static str],
}

impl LocalNativeBackend {
    pub fn sherpa(cfg: &AsrBackendConfig) -> Self {
        Self {
            kind: AsrKind::SherpaOnnx,
            model_path: cfg.model_path.clone(),
            langs: cfg.lang.clone().map(|l| vec![l]).unwrap_or_else(|| AsrKind::SherpaOnnx.languages()),
            expected: &["model.int8.onnx", "model.onnx", "model_quant.onnx", "tokens.txt"],
        }
    }

    pub fn vosk(cfg: &AsrBackendConfig) -> Self {
        Self {
            kind: AsrKind::Vosk,
            model_path: cfg.model_path.clone(),
            langs: cfg.lang.clone().map(|l| vec![l]).unwrap_or_else(|| AsrKind::Vosk.languages()),
            expected: &["conf/model.conf", "am/final.mdl"],
        }
    }

    fn check_ready(&self) -> Result<(), AsrError> {
        let Some(path) = &self.model_path else {
            return Err(AsrError::ModelNotFound(format!(
                "{} 未配置模型路径，请在配置中指定 model_path（或先下载模型）",
                self.kind.display_name()
            )));
        };
        let dir = std::path::Path::new(path);
        if !dir.exists() {
            return Err(AsrError::ModelNotFound(format!("模型目录不存在: {path}")));
        }
        // 目录内递归找任一期望文件（兼容解压后的子目录结构，参考 huiji _detectModelPath）
        let found = walk_find(dir, self.expected, 3);
        if !found {
            return Err(AsrError::ModelNotFound(format!(
                "{} 目录中未找到模型文件（期望: {}），可能下载不完整",
                self.kind.display_name(),
                self.expected.join(" / ")
            )));
        }
        Ok(())
    }
}

fn walk_find(dir: &std::path::Path, candidates: &[&str], depth: usize) -> bool {
    if depth == 0 {
        return false;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if walk_find(&p, candidates, depth - 1) {
                return true;
            }
        } else if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
            if candidates.contains(&name) {
                return true;
            }
        }
    }
    false
}

#[async_trait::async_trait]
impl AsrBackend for LocalNativeBackend {
    fn kind(&self) -> AsrKind { self.kind }

    fn languages(&self) -> &[String] { &self.langs }

    async fn health_check(&self) -> Result<(), AsrError> {
        self.check_ready()
    }

    async fn start(
        &mut self,
        _audio: AudioSource,
        _events: AsrEventSink,
    ) -> Result<AsrSessionHandle, AsrError> {
        self.check_ready()?;
        Err(AsrError::NotImplemented(format!(
            "{} 本地推理引擎尚未接入（需链接原生库），请使用云端后端",
            self.kind.display_name()
        )))
    }

    async fn stop(&mut self) -> Result<Vec<AsrSegment>, AsrError> {
        Ok(Vec::new())
    }
}

// ── 工具函数 ──────────────────────────────────────────────

/// PCM (16kHz 16bit mono LE) → WAV 文件字节（含 44 字节头）
pub fn pcm_to_wav(pcm: &[u8]) -> Vec<u8> {
    let sample_rate = 16000u32;
    let channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * channels as u32 * (bits_per_sample / 8) as u32;
    let block_align = channels * (bits_per_sample / 8);
    let data_len = pcm.len() as u32;

    let mut wav = Vec::with_capacity(44 + pcm.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());          // PCM
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&bits_per_sample.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    wav.extend_from_slice(pcm);
    wav
}

/// 差集：new = full[len(prev):]（词边界不精确时整体覆盖）
fn diff_text(full: &str, prev: &str) -> String {
    if prev.is_empty() {
        return full.to_string();
    }
    if full.len() > prev.len() && full.starts_with(prev) {
        full[prev.len()..].trim_start().to_string()
    } else {
        full.to_string()
    }
}

/// Whisper 分片重叠去重（§10.3.3⑤）：新片文本开头若与上一片文本尾部重复，
/// 去掉重复部分后返回。去重阈值 = 重叠 1s 音频对应的文本长度（约 20 字）。
fn dedup_overlap(prev: &str, next: &str, max_overlap_chars: usize) -> String {
    if prev.is_empty() || next.is_empty() {
        return next.to_string();
    }
    let prev_tail: String = prev.chars().rev().take(max_overlap_chars).collect::<Vec<_>>().into_iter().rev().collect();
    let next_head_len = next.chars().take(max_overlap_chars).count();
    let next_head: String = next.chars().take(next_head_len).collect();
    // 找 prev_tail 与 next_head 的最长公共后缀/前缀匹配
    let mut best = 0;
    let prev_chars: Vec<char> = prev_tail.chars().collect();
    let next_chars: Vec<char> = next_head.chars().collect();
    for overlap in 1..=prev_chars.len().min(next_chars.len()) {
        let p_tail = &prev_chars[prev_chars.len() - overlap..];
        let n_head = &next_chars[..overlap];
        if p_tail == n_head {
            best = overlap;
        }
    }
    if best > 0 {
        next.chars().skip(best).collect()
    } else {
        next.to_string()
    }
}

/// 将音频缓冲（PCM 块列表）拼成单个 AudioSource 流（用于离线二次转写）
pub fn pcm_chunks_to_source(chunks: Vec<PcmChunk>) -> AudioSource {
    Box::pin(futures::stream::iter(chunks))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedup_overlap_removes_repeated_head() {
        // 上一片尾部「继续讨论。今天」与新片开头重复 → 去重后不重复
        let prev = "我们先讨论需求。继续讨论。今天";
        let next = "今天天气不错，我们继续。";
        let out = dedup_overlap(prev, next, 20);
        assert_eq!(out, "天气不错，我们继续。", "重叠的「今天」应被去除");
    }

    #[test]
    fn dedup_overlap_no_match_keeps_all() {
        let prev = "完全不同的内容";
        let next = "新话题开始了";
        assert_eq!(dedup_overlap(prev, next, 20), "新话题开始了");
    }

    #[test]
    fn dedup_overlap_empty_prev() {
        assert_eq!(dedup_overlap("", "第一条文本", 20), "第一条文本");
    }
}
