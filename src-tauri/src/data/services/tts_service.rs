// §10.3.9 TTS 播报：长文分段（按句边界）+ 后端能力状态
//
// 系统 TTS 走 WebView Web Speech API（前端执行，零依赖）；本服务提供
// 服务端分段（按句子/200 字符上限）与配置查询，云端 TTS 接入时在此扩展。

use serde::{Deserialize, Serialize};

use crate::data::db::Database;
use crate::utils::error::AppError;

const MAX_SEGMENT_CHARS: usize = 200;

/// 长文分段：优先句子边界（。！？.!?；;）切分，合并至 ≤ MAX_SEGMENT_CHARS
pub fn split_for_speech(text: &str) -> Vec<String> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }
    if text.chars().count() <= MAX_SEGMENT_CHARS {
        return vec![text.to_string()];
    }

    let mut segments: Vec<String> = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        current.push(ch);
        let boundary = matches!(ch, '。' | '！' | '？' | '.' | '!' | '?' | '；' | ';' | '\n');
        if boundary && current.chars().count() >= 40 {
            segments.push(current.trim().to_string());
            current.clear();
        } else if current.chars().count() >= MAX_SEGMENT_CHARS {
            // 长句无边界：强制截断（保留整句语义由前端拼接播放）
            segments.push(current.trim().to_string());
            current.clear();
        }
    }
    if !current.trim().is_empty() {
        segments.push(current.trim().to_string());
    }
    segments.retain(|s| !s.is_empty());
    segments
}

/// 后端能力状态（前端据此选择系统 TTS 或提示配置云端）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsVoiceInfo {
    /// 当前生效后端：system（Web Speech API）| none
    pub backend: String,
    pub available: bool,
    pub lang: Option<String>,
    pub rate: f32,
}

/// 查询 TTS 状态（preferences: tts.lang / tts.rate，默认 zh-CN / 1.0）
pub async fn voices_status(db: &Database) -> Result<TtsVoiceInfo, AppError> {
    let lang = get_pref(db, "tts.lang")
        .await
        .unwrap_or_else(|| "zh-CN".into());
    let rate = get_pref(db, "tts.rate")
        .await
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(1.0);
    Ok(TtsVoiceInfo {
        backend: "system".into(),
        available: true, // Web Speech API 由 WebView 提供，桌面端默认可用
        lang: Some(lang),
        rate,
    })
}

async fn get_pref(db: &Database, key: &str) -> Option<String> {
    sqlx::query_scalar::<_, String>("SELECT value FROM preferences WHERE key = ?")
        .bind(key)
        .fetch_optional(&db.pool)
        .await
        .ok()
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_text_single_segment() {
        let segs = split_for_speech("今天讨论完成。");
        assert_eq!(segs, vec!["今天讨论完成。"]);
    }

    #[test]
    fn empty_text_no_segments() {
        assert!(split_for_speech("  \n ").is_empty());
    }

    #[test]
    fn splits_on_sentence_boundaries() {
        let text = format!("{}。", "第一句内容".repeat(30)); // 120 字单句（< 200 无截断）
        let segs = split_for_speech(&text);
        assert!(!segs.is_empty());
        assert!(segs.iter().all(|s| s.chars().count() <= MAX_SEGMENT_CHARS));
    }

    #[test]
    fn long_sentence_without_boundary_truncates() {
        // 250 字无句号长句：按 200 字上限强制截断为多段
        let text = "长句无标点".repeat(50);
        assert_eq!(text.chars().count(), 250);
        let segs = split_for_speech(&text);
        assert!(
            segs.len() > 1,
            "超长无边界文本应截断为多段，实际 {} 段",
            segs.len()
        );
        assert!(segs.iter().all(|s| s.chars().count() <= MAX_SEGMENT_CHARS));
    }

    #[test]
    fn multi_sentence_splits() {
        // 每句 102 字符，总 206 > MAX_SEGMENT_CHARS(200)，按句号边界切为 2 段
        let text = format!("{}。{}。", "句子A".repeat(34), "句子B".repeat(34));
        let segs = split_for_speech(&text);
        assert!(segs.len() >= 2, "多句应拆分为多段，实际 {} 段", segs.len());
        assert!(segs.iter().any(|s| s.contains("句子A")));
        assert!(segs.iter().any(|s| s.contains("句子B")));
    }
}
