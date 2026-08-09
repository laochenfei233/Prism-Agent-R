/// Split text into chunks with paragraph → sentence → fixed-window fallback.
pub fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![];
    }

    // If the whole text fits in one chunk, return it as-is
    if text.len() <= chunk_size {
        return vec![text.to_string()];
    }

    let mut chunks: Vec<String> = Vec::new();

    // 1. Try paragraph-based splitting
    let paragraphs: Vec<&str> = text.split("\n\n").collect();
    if paragraphs.len() > 1 {
        let mut current = String::new();
        for para in paragraphs {
            if !current.is_empty() && current.len() + para.len() + 2 > chunk_size {
                chunks.push(current.trim().to_string());
                // Keep overlap from end of previous chunk
                if overlap > 0 && !chunks.last().unwrap().is_empty() {
                    let prev = chunks.last().unwrap();
                    let overlap_start = prev.len().saturating_sub(overlap);
                    let overlap_start = prev.floor_char_boundary(overlap_start);
                    current = prev[overlap_start..].to_string();
                    if !current.is_empty() {
                        current.push_str("\n\n");
                    }
                } else {
                    current = String::new();
                }
            }
            if !current.is_empty() {
                current.push_str("\n\n");
            }
            current.push_str(para);
        }
        if !current.trim().is_empty() {
            chunks.push(current.trim().to_string());
        }
        if !chunks.is_empty() {
            return chunks;
        }
    }

    // 2. Try sentence-based splitting
    let sentence_endings = ['。', '！', '？', '.', '!', '?'];
    let sentences: Vec<&str> = text
        .split(|c: char| sentence_endings.contains(&c))
        .collect();

    if sentences.len() > 1 {
        let mut current = String::new();
        for (i, sent) in sentences.iter().enumerate() {
            let sep = if i < sentences.len() - 1 {
                // Recover the delimiter（多字节标点必须按字符取，不能按字节切片）
                let byte_offset = sent.as_ptr() as usize - text.as_ptr() as usize;
                let end_pos = byte_offset + sent.len();
                if end_pos < text.len() {
                    text[end_pos..].chars().next().unwrap_or_default().to_string()
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            let candidate = if current.is_empty() {
                format!("{}{}", sent, sep)
            } else {
                format!("{}{}{}", current, sent, sep)
            };

            if candidate.len() > chunk_size && !current.is_empty() {
                chunks.push(current.trim().to_string());
                if overlap > 0 {
                    let prev = chunks.last().unwrap();
                    let overlap_start = prev.len().saturating_sub(overlap);
                    let overlap_start = prev.floor_char_boundary(overlap_start);
                    current = prev[overlap_start..].to_string();
                } else {
                    current = String::new();
                }
                current.push_str(sent);
                current.push_str(&sep);
            } else {
                current = candidate;
            }
        }
        if !current.trim().is_empty() {
            chunks.push(current.trim().to_string());
        }
        if !chunks.is_empty() {
            return chunks;
        }
    }

    // 3. Fixed-window fallback
    let bytes = text.as_bytes();
    let mut start = 0;
    while start < bytes.len() {
        let end = std::cmp::min(start + chunk_size, bytes.len());
        // Try to avoid splitting UTF-8 chars
        let mut actual_end = end;
        while actual_end > start && !text.is_char_boundary(actual_end) {
            actual_end -= 1;
        }
        chunks.push(String::from_utf8_lossy(&bytes[start..actual_end]).to_string());
        if end >= bytes.len() {
            break;
        }
        start = end.saturating_sub(overlap);
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text() {
        assert_eq!(chunk_text("", 100, 20), Vec::<String>::new());
    }

    #[test]
    fn short_text() {
        assert_eq!(chunk_text("hello", 100, 20), vec!["hello"]);
    }

    #[test]
    fn paragraph_split() {
        let text = "Para one.\n\nPara two.\n\nPara three.";
        let chunks = chunk_text(text, 20, 0);
        assert!(chunks.len() >= 2);
    }

    #[test]
    fn fixed_window() {
        let text = "abcdefghijklmnopqrstuvwxyz";
        let chunks = chunk_text(text, 10, 3);
        assert!(chunks.len() > 1);
        assert!(chunks[0].len() <= 10);
    }

    #[test]
    fn chinese_sentence_split_no_panic() {
        // 无 \n\n 的中文长文本：句子分割路径恢复分隔符时不得对多字节标点做 1 字节切片
        let text = "今天天气很好。我们一起去公园散步。然后回家吃饭休息。最后按时睡觉。";
        let chunks = chunk_text(text, 50, 0);
        assert!(chunks.len() >= 1);
        assert!(chunks.iter().all(|c| !c.is_empty()));
        // 分隔符必须保留在 chunk 中（不得丢句号）
        assert!(chunks.iter().any(|c| c.contains('。')));
    }

    #[test]
    fn chinese_overlap_no_panic() {
        // overlap 截取点落在多字节字符中间时不得 panic（段落路径）
        let text = "第一段内容描述。\n\n第二段内容描述。\n\n第三段内容描述。\n\n第四段内容描述。";
        let chunks = chunk_text(text, 30, 5);
        assert!(chunks.len() >= 1);
        assert!(chunks.iter().all(|c| !c.is_empty()));
    }

    #[test]
    fn chinese_sentence_overlap_no_panic() {
        // overlap 截取点落在多字节字符中间时不得 panic（句子路径）
        let text = "今天天气很好。我们一起去公园散步。然后回家吃饭休息。最后按时睡觉。";
        let chunks = chunk_text(text, 40, 7);
        assert!(chunks.len() >= 1);
        assert!(chunks.iter().all(|c| !c.is_empty()));
    }
}
