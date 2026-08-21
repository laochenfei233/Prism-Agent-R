use std::process::Command;

use base64::Engine;
use sqlx::SqlitePool;

use crate::data::models::{OcrBlock, OcrProviderInfo, OcrResult};
use crate::utils::error::AppError;

/// OCR 服务（§10.5.3）：
/// - 多模态 LLM（OpenAI 兼容 /chat/completions 传 base64 图片）——默认在线
/// - tesseract 本地检测（离线降级）
///
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

    /// 识别入口（data URL 或路径统一入口）：resolve 字节后分发
    pub async fn recognize_input(
        &self,
        input: &str,
        lang: Option<&str>,
        provider: Option<&str>,
    ) -> Result<OcrResult, AppError> {
        let (bytes, mime) = decode_image_input(input)?;
        // LLM 走 data URL；tesseract 走临时文件
        if matches!(provider, Some("tesseract") | Some("local")) {
            let tmp = write_temp_image(&bytes, &mime)?;
            let r = self
                .recognize(tmp.to_string_lossy().as_ref(), lang, provider)
                .await;
            let _ = std::fs::remove_file(&tmp);
            r
        } else {
            let lang = lang.unwrap_or("auto").to_string();
            let data_url = format!(
                "data:{mime};base64,{}",
                base64::engine::general_purpose::STANDARD.encode(&bytes)
            );
            // 降级链：llm → tesseract（临时文件）
            match self.recognize_llm_from_data_url(&data_url, &lang).await {
                Ok(r) => Ok(r),
                Err(e) => {
                    tracing::warn!("LLM OCR 失败（{e}），降级 tesseract");
                    let tmp = write_temp_image(&bytes, &mime)?;
                    let r = self.recognize_tesseract(tmp.to_string_lossy().as_ref(), &lang);
                    let _ = std::fs::remove_file(&tmp);
                    r.map_err(|t_err| {
                        AppError::Internal(format!(
                            "LLM OCR 失败: {e}；tesseract 也不可用: {t_err}"
                        ))
                    })
                }
            }
        }
    }

    /// 多模态 LLM OCR：base64 图片 → /chat/completions
    async fn recognize_llm(&self, image_path: &str, lang: &str) -> Result<OcrResult, AppError> {
        let bytes = tokio::fs::read(image_path).await?;
        let mime = mime_guess::from_path(image_path)
            .first_or_octet_stream()
            .to_string();
        let data_url = format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&bytes)
        );
        self.recognize_llm_from_data_url(&data_url, lang).await
    }

    async fn recognize_llm_from_data_url(
        &self,
        data_url: &str,
        lang: &str,
    ) -> Result<OcrResult, AppError> {
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

        let base_url = provider_row
            .base_url
            .unwrap_or_else(|| match provider_row.kind.as_str() {
                "ollama" => "http://localhost:11434/v1".to_string(),
                _ => "https://api.openai.com/v1".to_string(),
            });
        let api_key = provider_row
            .api_key_enc
            .as_deref()
            .map(crate::commands::settings::decrypt_provider_key)
            .unwrap_or_default();

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
        let mut req = reqwest::Client::new()
            .post(&url)
            .header("Content-Type", "application/json");
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
            let has_model: Option<i64> =
                sqlx::query_scalar("SELECT COUNT(*) FROM models WHERE is_default = 1")
                    .fetch_one(&self.pool)
                    .await
                    .unwrap_or(Some(0));
            has_model.unwrap_or(0) > 0
        };
        vec![
            OcrProviderInfo {
                name: "llm".into(),
                kind: "api".into(),
                available: llm_ok,
            },
            OcrProviderInfo {
                name: "tesseract".into(),
                kind: "local".into(),
                available: find_executable("tesseract").is_some(),
            },
        ]
    }
}

/// 图片输入解析：data URL（data:image/png;base64,xxx）或磁盘路径 → (字节, mime)
/// 供前端直接传 FileReader 的 data URL，规避 WebView file.name 不是磁盘路径的问题。
fn decode_image_input(input: &str) -> Result<(Vec<u8>, String), AppError> {
    if let Some(rest) = input.strip_prefix("data:") {
        let (mime, b64) = rest
            .split_once(';')
            .ok_or_else(|| AppError::Validation("data URL 缺少 mime 分隔".into()))?;
        let b64 = b64
            .strip_prefix("base64,")
            .ok_or_else(|| AppError::Validation("data URL 非 base64".into()))?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .map_err(|e| AppError::Validation(format!("data URL base64 解码失败: {e}")))?;
        Ok((bytes, mime.to_string()))
    } else {
        let bytes = std::fs::read(input)
            .map_err(|e| AppError::Internal(format!("图片路径不可读: {input}: {e}")))?;
        let mime = mime_guess::from_path(input)
            .first_or_octet_stream()
            .to_string();
        Ok((bytes, mime))
    }
}

/// 在 PATH 中查找可执行文件
fn find_executable(name: &str) -> Option<std::path::PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.into()
        });
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// 将图片字节写为临时文件（tesseract 需要磁盘路径），返回路径
fn write_temp_image(bytes: &[u8], mime: &str) -> Result<std::path::PathBuf, AppError> {
    let ext = match mime {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        _ => "png",
    };
    let path = std::env::temp_dir().join(format!("prism_ocr_{}.{ext}", uuid::Uuid::new_v4()));
    std::fs::write(&path, bytes)
        .map_err(|e| AppError::Internal(format!("临时图片写入失败: {e}")))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_data_url_png() {
        // 1x1 透明 PNG
        let png = [0x89u8, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        let b64 = base64::engine::general_purpose::STANDARD.encode(png);
        let url = format!("data:image/png;base64,{b64}");
        let (bytes, mime) = decode_image_input(&url).unwrap();
        assert_eq!(bytes, png);
        assert_eq!(mime, "image/png");
    }

    #[test]
    fn decode_path_reads_file() {
        let dir = std::env::temp_dir().join(format!("prism_ocr_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.png");
        std::fs::write(&path, b"PNGDATA").unwrap();
        let (bytes, mime) = decode_image_input(&path.to_string_lossy()).unwrap();
        assert_eq!(bytes, b"PNGDATA");
        assert_eq!(mime, "image/png");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn decode_invalid_path_errors() {
        assert!(decode_image_input("C:\\nonexistent\\x.png").is_err());
    }
}
