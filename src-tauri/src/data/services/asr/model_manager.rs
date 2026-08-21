use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::utils::error::AppError;

/// 下载进度回调：上报 0.0~1.0 进度与阶段描述
pub type DownloadProgressCallback = Box<dyn Fn(f32, &str) + Send + Sync>;

// ── 模型信息 ──────────────────────────────────────────────

/// 模型类别：在线模型（API 链接）vs 本地模型（下载后离线运行）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AsrModelCategory {
    /// 在线模型：通过 API 调用，无需下载，需配置 API Key 和地址
    Online,
    /// 本地模型：下载后离线运行，需要本地路径
    Local,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrModelInfo {
    pub id: String,
    pub name: String,
    pub backend: String,
    /// 模型类别：online（在线 API）或 local（本地离线）
    pub category: AsrModelCategory,
    pub size_mb: u64,
    pub lang: Vec<String>,
    /// 在线模型：API 地址模板；本地模型：下载 URL
    pub url: String,
    pub sha256: String,
    pub requires_vad: bool,
    /// 是否用户自放置的模型（磁盘扫描发现，非内置清单）
    pub user_placed: bool,
    /// 在线模型：默认模型 ID（如 whisper-1）
    pub default_model_id: Option<String>,
    /// 在线模型：是否需要 API Key
    pub requires_api_key: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledAsrModel {
    pub id: String,
    pub path: String,
    pub size_mb: u64,
    pub backend: String,
    pub lang: Vec<String>,
}

/// 内置预置清单（仅"推荐下载"，不构成封闭集合；磁盘上的任何模型目录都会被扫描识别）
/// URL 为可下载的 tar.bz2 直链（sherpa-onnx 官方 releases）
pub fn builtin_catalog() -> Vec<AsrModelInfo> {
    vec![
        // ═══════════════════════════════════════════════════════
        // 在线模型（API 调用，无需下载）
        // ═══════════════════════════════════════════════════════
        AsrModelInfo {
            id: "online-whisper-api".into(),
            name: "OpenAI Whisper API".into(),
            backend: "WhisperApi".into(),
            category: AsrModelCategory::Online,
            size_mb: 0,
            lang: vec!["zh".into(), "en".into(), "ja".into(), "ko".into()],
            url: "https://api.openai.com/v1/audio/transcriptions".into(),
            sha256: "".into(),
            requires_vad: false,
            user_placed: false,
            default_model_id: Some("whisper-1".into()),
            requires_api_key: true,
        },
        AsrModelInfo {
            id: "online-dashscope-funasr".into(),
            name: "阿里云 DashScope FunASR".into(),
            backend: "DashScopeFunasr".into(),
            category: AsrModelCategory::Online,
            size_mb: 0,
            lang: vec!["zh".into(), "en".into()],
            url: "wss://dashscope.aliyuncs.com/api-ws/v1/inference".into(),
            sha256: "".into(),
            requires_vad: false,
            user_placed: false,
            default_model_id: Some("paraformer-v2".into()),
            requires_api_key: true,
        },
        AsrModelInfo {
            id: "online-mimo-http".into(),
            name: "MiMo HTTP 语音识别".into(),
            backend: "MiMoHttp".into(),
            category: AsrModelCategory::Online,
            size_mb: 0,
            lang: vec!["zh".into(), "en".into()],
            url: "https://api.xiaomimimo.com/v1/audio/transcriptions".into(),
            sha256: "".into(),
            requires_vad: false,
            user_placed: false,
            default_model_id: Some("mimo-asr".into()),
            requires_api_key: true,
        },
        AsrModelInfo {
            id: "online-azure-speech".into(),
            name: "Azure Speech Services".into(),
            backend: "AzureSpeech".into(),
            category: AsrModelCategory::Online,
            size_mb: 0,
            lang: vec!["zh".into(), "en".into(), "ja".into(), "ko".into()],
            url: "https://{region}.stt.speech.microsoft.com/speech/recognition/conversation/cognitiveservices/v1".into(),
            sha256: "".into(),
            requires_vad: false,
            user_placed: false,
            default_model_id: None,
            requires_api_key: true,
        },
        AsrModelInfo {
            id: "online-custom-api".into(),
            name: "自定义 ASR API".into(),
            backend: "Custom".into(),
            category: AsrModelCategory::Online,
            size_mb: 0,
            lang: vec!["zh".into()],
            url: "".into(),
            sha256: "".into(),
            requires_vad: false,
            user_placed: false,
            default_model_id: None,
            requires_api_key: true,
        },
        // ═══════════════════════════════════════════════════════
        // 本地模型（下载后离线运行）
        // ═══════════════════════════════════════════════════════
        AsrModelInfo {
            id: "sherpa-sensevoice-small".into(),
            name: "SenseVoice-Small (中文流式)".into(),
            backend: "SherpaOnnx".into(),
            category: AsrModelCategory::Local,
            size_mb: 228,
            lang: vec!["zh".into(), "en".into(), "ja".into(), "ko".into(), "yue".into()],
            url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17.tar.bz2".into(),
            sha256: "".into(),
            requires_vad: false,
            user_placed: false,
            default_model_id: None,
            requires_api_key: false,
        },
        AsrModelInfo {
            id: "sherpa-paraformer-large".into(),
            name: "Paraformer-Large (中文)".into(),
            backend: "SherpaOnnx".into(),
            category: AsrModelCategory::Local,
            size_mb: 230,
            lang: vec!["zh".into()],
            url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-paraformer-zh-2023-09-14.tar.bz2".into(),
            sha256: "".into(),
            requires_vad: true,
            user_placed: false,
            default_model_id: None,
            requires_api_key: false,
        },
        AsrModelInfo {
            id: "sherpa-paraformer-zh-en".into(),
            name: "Paraformer 中英混合".into(),
            backend: "SherpaOnnx".into(),
            category: AsrModelCategory::Local,
            size_mb: 245,
            lang: vec!["zh".into(), "en".into()],
            url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-paraformer-zh-en-2023-09-14.tar.bz2".into(),
            sha256: "".into(),
            requires_vad: true,
            user_placed: false,
            default_model_id: None,
            requires_api_key: false,
        },
        AsrModelInfo {
            id: "sherpa-whisper-tiny".into(),
            name: "Whisper tiny (中英蒸馏)".into(),
            backend: "SherpaOnnx".into(),
            category: AsrModelCategory::Local,
            size_mb: 80,
            lang: vec!["zh".into(), "en".into()],
            url: "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-tiny-zh-en".into(),
            sha256: "".into(),
            requires_vad: false,
            user_placed: false,
            default_model_id: None,
            requires_api_key: false,
        },
        AsrModelInfo {
            id: "sherpa-whisper-base".into(),
            name: "Whisper base (多语言)".into(),
            backend: "SherpaOnnx".into(),
            category: AsrModelCategory::Local,
            size_mb: 150,
            lang: vec!["zh".into(), "en".into(), "ja".into(), "ko".into()],
            url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-base.tar.bz2".into(),
            sha256: "".into(),
            requires_vad: false,
            user_placed: false,
            default_model_id: None,
            requires_api_key: false,
        },
        AsrModelInfo {
            id: "sherpa-whisper-small".into(),
            name: "Whisper small (多语言)".into(),
            backend: "SherpaOnnx".into(),
            category: AsrModelCategory::Local,
            size_mb: 460,
            lang: vec!["zh".into(), "en".into(), "ja".into(), "ko".into()],
            url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-small.tar.bz2".into(),
            sha256: "".into(),
            requires_vad: false,
            user_placed: false,
            default_model_id: None,
            requires_api_key: false,
        },
        AsrModelInfo {
            id: "sherpa-whisper-medium".into(),
            name: "Whisper medium (多语言)".into(),
            backend: "SherpaOnnx".into(),
            category: AsrModelCategory::Local,
            size_mb: 1500,
            lang: vec!["zh".into(), "en".into(), "ja".into(), "ko".into()],
            url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-medium.tar.bz2".into(),
            sha256: "".into(),
            requires_vad: false,
            user_placed: false,
            default_model_id: None,
            requires_api_key: false,
        },
        AsrModelInfo {
            id: "sherpa-paraformer-zh-hf".into(),
            name: "Paraformer 中文 (HuggingFace)".into(),
            backend: "SherpaOnnx".into(),
            category: AsrModelCategory::Local,
            size_mb: 390,
            lang: vec!["zh".into()],
            url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-paraformer-zh-hf.tar.bz2".into(),
            sha256: "".into(),
            requires_vad: true,
            user_placed: false,
            default_model_id: None,
            requires_api_key: false,
        },
        AsrModelInfo {
            id: "funasr-punc-transformer".into(),
            name: "FunASR 标点恢复模型".into(),
            backend: "SherpaOnnx".into(),
            category: AsrModelCategory::Local,
            size_mb: 120,
            lang: vec!["zh".into()],
            url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-punc-ct-pnc-zh-en.tar.bz2".into(),
            sha256: "".into(),
            requires_vad: false,
            user_placed: false,
            default_model_id: None,
            requires_api_key: false,
        },
        AsrModelInfo {
            id: "vosk-cn-small".into(),
            name: "Vosk 中文小模型".into(),
            backend: "Vosk".into(),
            category: AsrModelCategory::Local,
            size_mb: 42,
            lang: vec!["zh".into()],
            url: "https://alphacephei.com/vosk/models/vosk-model-small-cn-0.22.zip".into(),
            sha256: "".into(),
            requires_vad: false,
            user_placed: false,
            default_model_id: None,
            requires_api_key: false,
        },
        AsrModelInfo {
            id: "vosk-en-small".into(),
            name: "Vosk 英文小模型".into(),
            backend: "Vosk".into(),
            category: AsrModelCategory::Local,
            size_mb: 40,
            lang: vec!["en".into()],
            url: "https://alphacephei.com/vosk/models/vosk-model-small-en-us-0.15.zip".into(),
            sha256: "".into(),
            requires_vad: false,
            user_placed: false,
            default_model_id: None,
            requires_api_key: false,
        },
        AsrModelInfo {
            id: "vosk-cn-big".into(),
            name: "Vosk 中文大模型".into(),
            backend: "Vosk".into(),
            category: AsrModelCategory::Local,
            size_mb: 1800,
            lang: vec!["zh".into()],
            url: "https://alphacephei.com/vosk/models/vosk-model-cn-0.22.zip".into(),
            sha256: "".into(),
            requires_vad: false,
            user_placed: false,
            default_model_id: None,
            requires_api_key: false,
        },
        AsrModelInfo {
            id: "vosk-en-big".into(),
            name: "Vosk 英文大模型".into(),
            backend: "Vosk".into(),
            category: AsrModelCategory::Local,
            size_mb: 1800,
            lang: vec!["en".into()],
            url: "https://alphacephei.com/vosk/models/vosk-model-en-us-0.22-lgraph.zip".into(),
            sha256: "".into(),
            requires_vad: false,
            user_placed: false,
            default_model_id: None,
            requires_api_key: false,
        },
        AsrModelInfo {
            id: "vosk-ja-small".into(),
            name: "Vosk 日文小模型".into(),
            backend: "Vosk".into(),
            category: AsrModelCategory::Local,
            size_mb: 38,
            lang: vec!["ja".into()],
            url: "https://alphacephei.com/vosk/models/vosk-model-small-ja-0.22.zip".into(),
            sha256: "".into(),
            requires_vad: false,
            user_placed: false,
            default_model_id: None,
            requires_api_key: false,
        },
    ]
}

// ── 模型管理器 ────────────────────────────────────────────

pub struct AsrModelManager {
    models_dir: PathBuf,
}

impl AsrModelManager {
    pub fn new(models_dir: PathBuf) -> Self {
        Self { models_dir }
    }

    /// 完整目录：内置预置清单 + 磁盘扫描发现的用户模型（不框死，任意模型目录可识别）
    pub fn catalog(&self) -> Vec<AsrModelInfo> {
        let mut out = builtin_catalog();
        // 扫描 models_dir 下的自定义模型目录（未知 id 追加；已知 id 标记 user_placed）
        if let Ok(entries) = std::fs::read_dir(&self.models_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let id = entry.file_name().to_string_lossy().to_string();
                let backend = detect_backend(&path);
                if backend.is_none() {
                    continue; // 目录里没有任何可识别的模型文件，跳过
                }
                let backend = backend.unwrap();
                if let Some(existing) = out.iter_mut().find(|m| m.id == id) {
                    existing.user_placed = true;
                } else {
                    out.push(AsrModelInfo {
                        id: id.clone(),
                        name: id,
                        backend: backend.clone(),
                        category: AsrModelCategory::Local,
                        size_mb: dir_size_mb(&path),
                        lang: default_langs(&backend),
                        url: String::new(),
                        sha256: String::new(),
                        requires_vad: false,
                        user_placed: true,
                        default_model_id: None,
                        requires_api_key: false,
                    });
                }
            }
        }
        out
    }

    /// 已安装模型列表（目录存在且有可识别模型文件 = 已安装）
    pub fn installed(&self) -> Vec<InstalledAsrModel> {
        let mut out = Vec::new();
        if !self.models_dir.exists() {
            return out;
        }
        if let Ok(entries) = std::fs::read_dir(&self.models_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let id = entry.file_name().to_string_lossy().to_string();
                let Some(backend) = detect_backend(&path) else {
                    continue;
                };
                out.push(InstalledAsrModel {
                    id,
                    path: path.to_string_lossy().to_string(),
                    size_mb: dir_size_mb(&path),
                    backend,
                    lang: Vec::new(),
                });
            }
        }
        out
    }

    /// 按 id 取模型目录（内置清单下载目标或用户放置目录）
    pub fn model_dir(&self, model_id: &str) -> PathBuf {
        self.models_dir.join(model_id)
    }

    /// 下载模型（§10.3.1）：HTTP 下载 tar.bz2 → 解压到 models/{id}/。
    /// 进度通过 progress 回调上报（0.0~1.0）。sha256 非空时下载后校验。
    pub async fn download(
        &self,
        model_id: &str,
        progress: Option<DownloadProgressCallback>,
    ) -> Result<PathBuf, AppError> {
        let info = builtin_catalog()
            .into_iter()
            .find(|m| m.id == model_id)
            .ok_or_else(|| AppError::Validation(format!("未知模型: {model_id}")))?;

        let target = self.models_dir.join(&info.id);
        tokio::fs::create_dir_all(&target).await?;

        // 已存在模型文件 → 直接返回
        if detect_backend(&target).is_some() {
            return Ok(target);
        }

        let report = |frac: f32, msg: &str| {
            if let Some(cb) = &progress {
                cb(frac, msg);
            }
        };

        report(0.02, "开始下载");
        let tmp_path = target.join("model.tar.bz2");

        // 1. 下载
        let client = reqwest::Client::new();
        let mut resp = client
            .get(&info.url)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("模型下载失败: {e}")))?;
        if !resp.status().is_success() {
            return Err(AppError::Internal(format!(
                "模型下载 HTTP {}",
                resp.status()
            )));
        }
        let total = resp.content_length().unwrap_or(0);
        let mut downloaded: u64 = 0;
        let mut file = tokio::fs::File::create(&tmp_path).await?;
        use tokio::io::AsyncWriteExt;
        while let Some(chunk) = resp
            .chunk()
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?
        {
            downloaded += chunk.len() as u64;
            file.write_all(&chunk).await?;
            if total > 0 {
                report(0.02 + 0.68 * (downloaded as f32 / total as f32), "下载中");
            }
        }
        file.flush().await?;
        drop(file);
        report(0.7, "下载完成，校验中");

        // 2. sha256 校验（清单填写了校验和时）
        if !info.sha256.is_empty() {
            let bytes = tokio::fs::read(&tmp_path).await?;
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(&bytes);
            let actual = format!("{:x}", hasher.finalize());
            if actual != info.sha256 {
                let _ = tokio::fs::remove_file(&tmp_path).await;
                return Err(AppError::Validation(
                    "模型校验和不匹配（下载可能损坏），请重试".to_string(),
                ));
            }
        }

        // 3. 解压 tar.bz2（解到临时目录再移入，避免半解压污染）
        report(0.75, "解压中");
        let extract_tmp = target.join(".extract");
        if extract_tmp.exists() {
            let _ = tokio::fs::remove_dir_all(&extract_tmp).await;
        }
        tokio::fs::create_dir_all(&extract_tmp).await?;
        extract_tar_bz2(&tmp_path, &extract_tmp)?;

        // 解压出的内容移到 models/{id}/ 根（兼容 tar 包内单目录结构）
        move_extracted(&extract_tmp, &target).await?;
        let _ = tokio::fs::remove_file(&tmp_path).await;
        let _ = tokio::fs::remove_dir_all(&extract_tmp).await;

        if detect_backend(&target).is_none() {
            return Err(AppError::Validation(
                "解压后未找到模型文件，可能下载了错误包".into(),
            ));
        }

        report(1.0, "完成");
        Ok(target)
    }

    /// 删除模型
    pub async fn remove(&self, model_id: &str) -> Result<(), AppError> {
        let path = self.models_dir.join(model_id);
        if path.exists() {
            tokio::fs::remove_dir_all(&path).await?;
        }
        Ok(())
    }
}

/// 解压 tar.bz2 到目标目录
fn extract_tar_bz2(archive: &Path, dest: &Path) -> Result<(), AppError> {
    let file = std::fs::File::open(archive)?;
    let decoder = bzip2::read::BzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    // 安全解压：禁止绝对路径与 .. 穿越
    archive.set_preserve_permissions(false);
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        if path.is_absolute()
            || path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(AppError::Validation(format!(
                "压缩包内含不安全路径: {}",
                path.display()
            )));
        }
        entry.unpack(dest.join(&path))?;
    }
    Ok(())
}

/// 将解压内容移到目标根目录（兼容 tar 包内嵌一层目录）
async fn move_extracted(src: &Path, dest: &Path) -> Result<(), AppError> {
    let entries: Vec<PathBuf> = {
        let mut v = Vec::new();
        if let Ok(mut rd) = tokio::fs::read_dir(src).await {
            while let Ok(Some(e)) = rd.next_entry().await {
                v.push(e.path());
            }
        }
        v
    };
    if entries.len() == 1 && entries[0].is_dir() {
        // 单目录包裹：把内部内容提升一层
        let inner = &entries[0];
        let inner_entries: Vec<PathBuf> = {
            let mut v = Vec::new();
            if let Ok(mut rd) = tokio::fs::read_dir(inner).await {
                while let Ok(Some(e)) = rd.next_entry().await {
                    v.push(e.path());
                }
            }
            v
        };
        for p in inner_entries {
            let name = p
                .file_name()
                .ok_or_else(|| AppError::Internal("文件名无效".into()))?;
            tokio::fs::rename(&p, dest.join(name)).await?;
        }
    } else {
        for p in entries {
            let name = p
                .file_name()
                .ok_or_else(|| AppError::Internal("文件名无效".into()))?;
            tokio::fs::rename(&p, dest.join(name)).await?;
        }
    }
    Ok(())
}

// ── 模型识别辅助 ──────────────────────────────────────────

/// 识别模型目录属于哪个后端（支持解压后的子目录结构）
fn detect_backend(dir: &Path) -> Option<String> {
    let sherpa = [
        "model.int8.onnx",
        "model.onnx",
        "model_quant.onnx",
        "tokens.txt",
    ];
    let vosk = ["conf/model.conf", "am/final.mdl"];
    if walk_has(dir, &sherpa, 3) {
        Some("SherpaOnnx".into())
    } else if walk_has(dir, &vosk, 2) {
        Some("Vosk".into())
    } else {
        None
    }
}

fn walk_has(dir: &Path, candidates: &[&str], depth: usize) -> bool {
    if depth == 0 {
        return false;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if walk_has(&p, candidates, depth - 1) {
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

fn default_langs(backend: &str) -> Vec<String> {
    match backend {
        "SherpaOnnx" => vec!["zh".into(), "en".into()],
        "Vosk" => vec!["zh".into(), "en".into()],
        _ => vec!["zh".into()],
    }
}

fn dir_size_mb(path: &Path) -> u64 {
    let mut total = 0u64;
    if path.is_file() {
        total += path.metadata().map(|m| m.len()).unwrap_or(0);
    } else if path.is_dir() {
        let mut stack = vec![path.to_path_buf()];
        while let Some(dir) = stack.pop() {
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        stack.push(p);
                    } else if let Ok(md) = p.metadata() {
                        total += md.len();
                    }
                }
            }
        }
    }
    total.div_ceil(1_048_576) // MB（向上取整）
}
