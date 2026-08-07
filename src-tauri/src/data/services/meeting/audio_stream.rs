use std::collections::{HashMap, VecDeque};

use tokio::sync::mpsc;

use crate::data::services::asr::PcmChunk;
use crate::utils::error::AppError;

/// 块通道容量（每块 100ms ≈ 3200 字节，缓冲 10s）
const CHANNEL_CAP: usize = 100;

/// 音频流管理器：renderer 推送 PCM 块 → 缓冲 → ASR 消费端。
///
/// ⚠️ 时序规避（旧版实测缺陷）：renderer 的 startRecording() 立即发送 IPC chunks，
/// 但主进程的 stream 在 start 命令 handler 里才创建 → 早期 chunks 被丢弃。
/// 本设计：`pending` Map 缓冲先到的块，`create_stream()` 时 flush 给新消费者。
pub struct AudioStreamManager {
    /// meeting_id → 已创建消费通道（ASR 消费端）
    sources: tokio::sync::Mutex<HashMap<String, mpsc::Sender<PcmChunk>>>,
    /// meeting_id → 未创建 stream 前的缓冲
    pending: tokio::sync::Mutex<HashMap<String, VecDeque<PcmChunk>>>,
}

impl AudioStreamManager {
    pub fn new() -> Self {
        Self {
            sources: tokio::sync::Mutex::new(HashMap::new()),
            pending: tokio::sync::Mutex::new(HashMap::new()),
        }
    }

    /// 推送一块 PCM（16kHz 16bit mono）
    pub async fn push_chunk(&self, meeting_id: &str, pcm: PcmChunk) -> Result<(), AppError> {
        // 先尝试直接发给已创建的 stream
        let sources = self.sources.lock().await;
        if let Some(tx) = sources.get(meeting_id) {
            if tx.try_send(pcm.clone()).is_ok() {
                return Ok(());
            }
            // 通道满：改走 pending（避免阻塞）
            drop(sources);
            self.buffer(meeting_id, pcm).await;
            return Ok(());
        }
        drop(sources);
        // 尚无 stream → 缓冲
        self.buffer(meeting_id, pcm).await;
        Ok(())
    }

    /// 为 meeting 创建消费通道（ASR 消费端）。若此前有缓冲块，立即 flush。
    pub async fn create_stream(&self, meeting_id: &str) -> mpsc::Receiver<PcmChunk> {
        let (tx, rx) = mpsc::channel(CHANNEL_CAP);

        // 1. 注册通道
        self.sources.lock().await.insert(meeting_id.to_string(), tx.clone());

        // 2. flush 缓冲
        let pending = self.pending.lock().await.remove(meeting_id).unwrap_or_default();
        for chunk in pending {
            if tx.try_send(chunk).is_err() {
                break;
            }
        }

        rx
    }

    /// 停止消费：移除通道 + 清理缓冲
    pub async fn drop_stream(&self, meeting_id: &str) {
        self.sources.lock().await.remove(meeting_id);
        self.pending.lock().await.remove(meeting_id);
    }

    async fn buffer(&self, meeting_id: &str, pcm: PcmChunk) {
        let mut pending = self.pending.lock().await;
        let queue = pending.entry(meeting_id.to_string()).or_default();
        // 防止无消费端时无限增长（保留最近 ~30s）
        const MAX_PENDING: usize = 300;
        if queue.len() >= MAX_PENDING {
            queue.pop_front();
        }
        queue.push_back(pcm);
    }
}

impl Default for AudioStreamManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 将缓冲的 PCM 块转成离线 AudioSource（用于二次转写 / 换模型）
pub fn buffered_to_source(chunks: Vec<PcmChunk>) -> crate::data::services::asr::AudioSource {
    crate::data::services::asr::backends::pcm_chunks_to_source(chunks)
}
