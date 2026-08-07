use std::process::Command;

use base64::Engine;
use sqlx::SqlitePool;

use crate::data::models::{OcrBlock, OcrProviderInfo, OcrResult};
use crate::utils::error::AppError;

/// OCR 服务（§10.5.3）：
/// - 多模态 LLM（OpenAI 兼容 /chat/completions 传 base64 图片）——默认在线
/// - tesseract 本地检测（离线降级）
/// provider 优先级：显式指定 > 在线（默认模型）> 本地 tesseract
pub struct OcrService {
    pool: SqlitePool,
}

impl OcrService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn recognize(
        &self,
        image_path: &str,
        lang: Option<&str>,
        provider: Option<&str>,
    ) -> Result<OcrResult, AppError> {
        let lang = lang.unwrap_or("auto").to_string();

        match provider {
            Some("tesseract") => self.recognize_tesseract(image_path, &lang),
            Some("local") => self.recognize_tesseract(image_path, &lang),
            _ => {
                // 默认在线多模态 LLM；失败时降级 tesseract
                match self.recognize_llm(image_path, &lang).await {
                    Ok(r) => Ok(r),
                    Err(e) => {
                        tracing::warn!("LLM OCR 失败（{e}），降级 tesseract");
                        match self.recognize_tesseract(image_path, &lang) {
                            Ok(r) => Ok(r),
                            Err(t_err) => Err(AppError::Internal(format!(
                                "LLM OCR 失败: {e}；tesseract 也不可用: {t_err}"
                            ))),
                        }
                    }
                }
            }
        }
    }

    /// 多模态 LLM OCR：base64 图片 → /chat/completions
    async fn recognize_llm(&self, image_path: &str, lang: &str) -> Result<OcrResult, AppError> {
        use crate::data::models::{ModelRow, ProviderRow};

        let model_row = sqlx::query_as::<_, ModelRow>(
            "SELECT id, provider_id, model_id, display_name, kind, max_tokens, is_default, created_at FROM models WHERE is_default = 1 LIMIT 1"
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::LlmProvider("未配置默认模型，无法执行 OCR".into()))?;

        let provider_row = sqlx::query_as::<_, ProviderRow>(
            "SELECT id, name, kind, base_url, api_key_enc, is_enabled, created_at, updated_at FROM providers WHERE id = ?"
        )
        .bind(&model_row.provider_id)
        .fetch_optional(&self.pool)
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

        // 读取图片 → base64 data URL
        let bytes = tokio::fs::read(image_path).await?;
        let mime = mime_guess::from_path(image_path).first_or_octet_stream().to_string();
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let data_url = format!("data:{mime};base64,{b64}");

        let lang_hint = if lang == "auto" { "自动检测" } else { lang };
        let body = serde_json::json!({
            "model": model_row.model_id,
            "messages": [{
                "role": "user",
                "content": [
                    { "type": "text", "text": format!("请识别这张图片中的全部文字（语言：{lang_hint}）。只输出识别出的文字内容，保留换行；如果是表格请用 Markdown 表格输出。") },
                    { "type": "image_url", "image_url": { "url": data_url } }
                ]
            }],
            "max_tokens": 2048,
        });

        let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
        let mut req = reqwest::Client::new().post(&url).header("Content-Type", "application/json");
        if !api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {api_key}"));
        }
        let resp = req
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("OCR 请求失败: {e}")))?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::LlmProvider(format!("OCR HTTP {status}: {text}")));
        }
        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("OCR 解析失败: {e}")))?;
        let text = data["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or_default()
            .trim()
            .to_string();
        if text.is_empty() {
            return Err(AppError::Internal("OCR 未识别到文字".into()));
        }

        Ok(OcrResult {
            text: text.clone(),
            lang: lang.to_string(),
            provider: "llm".into(),
            blocks: vec![OcrBlock {
                text,
                bbox: (0.0, 0.0, 1.0, 1.0),
                confidence: 0.9,
                kind: "text".into(),
            }],
        })
    }

    /// tesseract 本地 OCR（检测可执行文件；缺失返回可读错误）
    fn recognize_tesseract(&self, image_path: &str, lang: &str) -> Result<OcrResult, AppError> {
        let tesseract = find_executable("tesseract")
            .ok_or_else(|| AppError::Internal("未检测到 tesseract，请安装或使用在线 OCR".into()))?;

        let lang_arg = if lang == "auto" { "chi_sim+eng" } else { lang };
        let output = Command::new(&tesseract)
            .arg(image_path)
            .arg("stdout")
            .arg("-l")
            .arg(lang_arg)
            .output()
            .map_err(|e| AppError::Internal(format!("tesseract 执行失败: {e}")))?;
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if text.is_empty() {
            return Err(AppError::Internal("tesseract 未识别到文字".into()));
        }
        Ok(OcrResult {
            text: text.clone(),
            lang: lang.to_string(),
            provider: "tesseract".into(),
            blocks: vec![OcrBlock {
                text,
                bbox: (0.0, 0.0, 1.0, 1.0),
                confidence: 0.8,
                kind: "text".into(),
            }],
        })
    }

    pub async fn providers(&self) -> Vec<OcrProviderInfo> {
        let llm_ok = {
            let has_model: Option<i64> = sqlx::query_scalar("SELECT COUNT(*) FROM models WHERE is_default = 1")
                .fetch_one(&self.pool)
                .await
                .unwrap_or(Some(0));
            has_model.unwrap_or(0) > 0
        };
        vec![
            OcrProviderInfo { name: "llm".into(), kind: "api".into(), available: llm_ok },
            OcrProviderInfo { name: "tesseract".into(), kind: "local".into(), available: find_executable("tesseract").is_some() },
        ]
    }
}

/// 在 PATH 中查找可执行文件
fn find_executable(name: &str) -> Option<std::path::PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(if cfg!(windows) { format!("{name}.exe") } else { name.into() });
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}
