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
                // Recover the delimiter
                let byte_offset = sent.as_ptr() as usize - text.as_ptr() as usize;
                let end_pos = byte_offset + sent.len();
                if end_pos < text.len() {
                    text[end_pos..end_pos + 1].to_string()
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
}
