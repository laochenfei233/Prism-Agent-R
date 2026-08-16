pub mod backends;
pub mod model_manager;

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use backends::{builtin_register, create_asr_backend};
pub use model_manager::{AsrModelCategory, AsrModelInfo, AsrModelManager, InstalledAsrModel};

// ── ASR 错误 ──────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum AsrError {
    #[error("未授权：API Key 无效")]
    Unauthorized,
    #[error("配额不足或限流")]
    QuotaExceeded,
    #[error("网络错误: {0}")]
    Network(String),
    #[error("模型文件缺失: {0}")]
    ModelNotFound(String),
    #[error("协议错误: {0}")]
    Protocol(String),
    #[error("后端未实现: {0}")]
    NotImplemented(String),
}

impl From<AsrError> for crate::utils::error::AppError {
    fn from(e: AsrError) -> Self {
        crate::utils::error::AppError::Internal(e.to_string())
    }
}

// ── ASR 数据类型 ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrSegment {
    pub index: u64,
    pub text: String,
    /// false = 中间结果（会持续修正），true = 定稿
    pub is_final: bool,
    pub start_ms: u64,
    pub end_ms: u64,
    pub language: Option<String>,
    pub confidence: Option<f32>,
    /// 说话人分离（支持的后端提供）
    pub speaker_id: Option<u32>,
}

/// 内置后端标识。
///
/// 注意：这是**预置快照**，不是封闭集合——任何未列出的 kind 字符串
/// 一律解析为 `Custom`，再通过后端注册表（见 `register_backend`）
/// 按名称匹配到具体实现；未注册的 Custom 会落到 OpenAI 兼容实现。
#[derive(Debug, Clone, Serialize, Deserialize, Copy, PartialEq, Eq)]
pub enum AsrKind {
    DashScopeFunasr,
    MiMoHttp,
    SherpaOnnx,
    LocalFunasrWs,
    WhisperApi,
    Vosk,
    AzureSpeech,
    Custom,
}

impl AsrKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            AsrKind::DashScopeFunasr => "DashScopeFunasr",
            AsrKind::MiMoHttp => "MiMoHttp",
            AsrKind::SherpaOnnx => "SherpaOnnx",
            AsrKind::LocalFunasrWs => "LocalFunasrWs",
            AsrKind::WhisperApi => "WhisperApi",
            AsrKind::Vosk => "Vosk",
            AsrKind::AzureSpeech => "AzureSpeech",
            AsrKind::Custom => "Custom",
        }
    }

    /// 宽松解析：已知枚举命中枚举，未知一律 Custom（不 panic、不拒绝）
    pub fn from_str(s: &str) -> Self {
        match s {
            "DashScopeFunasr" => AsrKind::DashScopeFunasr,
            "MiMoHttp" => AsrKind::MiMoHttp,
            "SherpaOnnx" => AsrKind::SherpaOnnx,
            "LocalFunasrWs" => AsrKind::LocalFunasrWs,
            "WhisperApi" => AsrKind::WhisperApi,
            "Vosk" => AsrKind::Vosk,
            "AzureSpeech" => AsrKind::AzureSpeech,
            _ => AsrKind::Custom,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            AsrKind::DashScopeFunasr => "DashScope FunASR",
            AsrKind::MiMoHttp => "MiMo ASR",
            AsrKind::SherpaOnnx => "Sherpa ONNX",
            AsrKind::LocalFunasrWs => "本地 FunASR WS",
            AsrKind::WhisperApi => "Whisper API",
            AsrKind::Vosk => "Vosk",
            AsrKind::AzureSpeech => "Azure Speech",
            AsrKind::Custom => "自定义兼容端点",
        }
    }

    pub fn languages(&self) -> Vec<String> {
        match self {
            AsrKind::DashScopeFunasr => vec!["zh".into()],
            AsrKind::MiMoHttp => vec!["zh".into(), "en".into()],
            AsrKind::SherpaOnnx => vec!["zh".into(), "en".into(), "ja".into(), "ko".into()],
            AsrKind::LocalFunasrWs => vec!["zh".into()],
            AsrKind::WhisperApi => vec!["zh".into(), "en".into(), "ja".into(), "ko".into()],
            AsrKind::Vosk => vec!["zh".into(), "en".into()],
            AsrKind::AzureSpeech => vec!["zh".into(), "en".into()],
            AsrKind::Custom => vec!["zh".into(), "en".into()],
        }
    }
}

// ── 后端动态注册表 ────────────────────────────────────────
// 不框死后端：新增后端 = 实现 AsrBackend + register_backend("MyKind", factory) 一行。
// 注册表全局静态，编译期由 backends.rs 的 builtin 注册填满；运行时也可追加。

/// 后端工厂：由注册名 + 配置构建具体后端
pub type BackendFactory = fn(&AsrBackendConfig) -> Box<dyn AsrBackend>;

static BACKEND_REGISTRY: OnceLock<std::sync::RwLock<HashMap<String, BackendFactory>>> = OnceLock::new();

fn registry() -> &'static std::sync::RwLock<HashMap<String, BackendFactory>> {
    BACKEND_REGISTRY.get_or_init(|| {
        let m = std::sync::RwLock::new(HashMap::new());
        // 内置注册（在 backends.rs 的 builtin_register 中填充）
        m
    })
}

/// 注册一个后端（幂等：同名覆盖）。示例：
/// `register_backend("MyGroqAsr", |cfg| Box::new(MyBackend::new(cfg)));`
pub fn register_backend(name: &str, factory: BackendFactory) {
    registry().write().unwrap().insert(name.to_string(), factory);
}

/// 查询已注册的后端名（供 UI 展示可用后端）
pub fn registered_backends() -> Vec<String> {
    let mut names: Vec<String> = registry().read().unwrap().keys().cloned().collect();
    names.sort();
    names
}

/// 按名称查工厂；未注册返回 None（调用方回退 Custom）
pub(crate) fn lookup_factory(name: &str) -> Option<BackendFactory> {
    registry().read().unwrap().get(name).copied()
}

// ── 音频源与事件回调 ──────────────────────────────────────

/// 音频块：16kHz 16bit 单声道 PCM（小端）
pub type PcmChunk = Vec<u8>;

/// 音频源：异步块流（与设计文档 AudioSource 同思路）
pub type AudioSource = Pin<Box<dyn futures::Stream<Item = PcmChunk> + Send>>;

/// 事件回调（增量转写 / 状态变化）
#[derive(Clone)]
pub struct AsrEventSink {
    pub on_segment: std::sync::Arc<dyn Fn(AsrSegment) + Send + Sync>,
    pub on_status: std::sync::Arc<dyn Fn(String) + Send + Sync>,
}

impl AsrEventSink {
    pub fn new(
        on_segment: impl Fn(AsrSegment) + Send + Sync + 'static,
        on_status: impl Fn(String) + Send + Sync + 'static,
    ) -> Self {
        Self {
            on_segment: std::sync::Arc::new(on_segment),
            on_status: std::sync::Arc::new(on_status),
        }
    }

    pub fn segment(&self, seg: AsrSegment) {
        (self.on_segment)(seg);
    }

    pub fn status(&self, status: &str) {
        (self.on_status)(status.to_string());
    }
}

// ── ASR 后端 Trait ────────────────────────────────────────

#[async_trait::async_trait]
pub trait AsrBackend: Send + Sync {
    /// 后端类型标识（用于配置与 UI 展示）
    fn kind(&self) -> AsrKind;

    /// 健康检查（启动会议前调用，失败则 UI 提前提示）
    async fn health_check(&self) -> Result<(), AsrError>;

    /// 开始识别：接收 16kHz PCM 音频块流，结果通过回调推送。
    /// 返回一个句柄，调用方可通过它停止。
    async fn start(
        &mut self,
        audio: AudioSource,
        events: AsrEventSink,
    ) -> Result<AsrSessionHandle, AsrError>;

    /// 停止识别，返回最终结果（离线后端）
    async fn stop(&mut self) -> Result<Vec<AsrSegment>, AsrError>;

    /// 支持的语言列表
    fn languages(&self) -> &[String];
}

/// ASR 会话句柄：停止录音时调用 cancel 结束流
#[derive(Clone)]
pub struct AsrSessionHandle {
    cancel: tokio_util::sync::CancellationToken,
}

impl AsrSessionHandle {
    pub fn new() -> Self {
        Self { cancel: tokio_util::sync::CancellationToken::new() }
    }

    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    pub fn token(&self) -> tokio_util::sync::CancellationToken {
        self.cancel.clone()
    }
}

impl Default for AsrSessionHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// 后端配置（来自 asr_configs 表 + 用户自定义字段）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrBackendConfig {
    /// 后端标识（可以是内置枚举名，也可以是任意自定义名）
    pub kind: AsrKind,
    /// 原始 kind 字符串（保留用户输入，供注册表精确匹配）
    pub kind_raw: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub lang: Option<String>,
    /// 本地模型路径（sherpa/vosk 等本地后端使用，不限定内置目录）
    pub model_path: Option<String>,
    /// 额外参数（任意 JSON，供自定义后端扩展，不框死字段）
    pub extra: Option<serde_json::Value>,
}

impl AsrBackendConfig {
    pub fn from_input(
        kind: &str,
        base_url: Option<String>,
        api_key: Option<String>,
        model: Option<String>,
        lang: Option<String>,
        model_path: Option<String>,
        extra: Option<serde_json::Value>,
    ) -> Self {
        Self {
            kind: AsrKind::from_str(kind),
            kind_raw: kind.to_string(),
            base_url,
            api_key,
            model,
            lang,
            model_path,
            extra,
        }
    }
}
