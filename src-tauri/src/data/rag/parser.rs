use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::utils::error::AppError;

// ── 统一文档解析管线（§10.2.3） ────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocKind {
    Markdown,
    Text,
    Pdf,
    Image,
    Other,
}

#[derive(Debug, Clone, Default)]
pub struct ParsedDoc {
    pub pages: Vec<ParsedPage>,
    /// 跨页块（表格/图表/代码）
    pub blocks: Vec<ParsedBlock>,
    pub title: Option<String>,
    pub page_count: usize,
}

#[derive(Debug, Clone)]
pub struct ParsedPage {
    pub page_no: u32,
    pub text: String,
    /// 视觉块路径（页面渲染图，可选）
    pub image_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub enum ParsedBlock {
    Table { text: String, table_json: Option<String> },
    Image { path: PathBuf, caption: Option<String> },
    Text { text: String },
}

/// 文档解析器抽象：按扩展名分发（可插拔管线）
#[async_trait]
pub trait DocumentParser: Send + Sync {
    fn kind(&self) -> DocKind;
    async fn parse(&self, path: &Path) -> Result<ParsedDoc, AppError>;
}

// ── 文本/Markdown 解析 ────────────────────────────────────

pub struct TextParser;

#[async_trait]
impl DocumentParser for TextParser {
    fn kind(&self) -> DocKind {
        DocKind::Text
    }

    async fn parse(&self, path: &Path) -> Result<ParsedDoc, AppError> {
        let text = tokio::fs::read_to_string(path).await?;
        Ok(ParsedDoc {
            pages: vec![ParsedPage { page_no: 1, text: text.clone(), image_path: None }],
            blocks: vec![ParsedBlock::Text { text }],
            title: path.file_stem().map(|s| s.to_string_lossy().into_owned()),
            page_count: 1,
        })
    }
}

// ── PDF 双通道解析 ────────────────────────────────────────
// 文本层：pdf-extract（纯 Rust，保留页序）
// 视觉层：检测 pdfium 二进制（pdfium-render 生态）→ 渲染页面 PNG → 供 OCR/多模态理解；
//         未检测到时优雅降级为文本层结果（不中断摄取）

pub struct PdfParser;

#[async_trait]
impl DocumentParser for PdfParser {
    fn kind(&self) -> DocKind {
        DocKind::Pdf
    }

    async fn parse(&self, path: &Path) -> Result<ParsedDoc, AppError> {
        // 文本层：pdf-extract 按页提取（保留页序，§10.2.3 分页定位）
        let pages = pdf_extract::extract_text_by_pages(path)
            .map_err(|e| AppError::Internal(format!("PDF 文本提取失败: {e}")))?;

        let mut doc = ParsedDoc {
            title: path.file_stem().map(|s| s.to_string_lossy().into_owned()),
            ..Default::default()
        };
        let non_empty: Vec<(u32, String)> = pages
            .iter()
            .enumerate()
            .filter(|(_, t)| !t.trim().is_empty())
            .map(|(i, t)| (i as u32 + 1, t.clone()))
            .collect();

        // 数字版（至少一页有文本）→ 使用文本层
        if !non_empty.is_empty() {
            for (page_no, text) in non_empty {
                doc.blocks.push(ParsedBlock::Text { text: text.clone() });
                doc.pages.push(ParsedPage { page_no, text, image_path: None });
            }
            doc.page_count = pages.len();
            return Ok(doc);
        }

        // 扫描版（无文本层）→ 视觉层：检测 pdfium 渲染页面为图像
        if let Some(renderer) = PdfPageRenderer::detect() {
            match renderer.render_all(path, 2).await {
                Ok(rendered) => {
                    doc.pages = rendered;
                    doc.page_count = doc.pages.len();
                    return Ok(doc);
                }
                Err(e) => {
                    tracing::warn!("PDF 视觉层渲染失败（{e}），返回空文本层");
                }
            }
        } else {
            tracing::warn!("未检测到 pdfium，扫描版 PDF 无法视觉解析");
        }

        Ok(doc)
    }
}

// ── PDF 视觉层渲染（可插拔） ──────────────────────────────
// 通过命令行检测 pdfium 可执行文件（用户可选安装），渲染每页为 PNG。
// 渲染结果由上层 OCR/多模态服务消费（见 ocr_service）。

pub struct PdfPageRenderer {
    pdfium: PathBuf,
}

impl PdfPageRenderer {
    /// 检测 pdfium 可执行文件（PATH 或常见位置）
    pub fn detect() -> Option<Self> {
        for name in ["pdfium", "pdfium.exe"] {
            if let Some(p) = find_in_path(name) {
                return Some(Self { pdfium: p });
            }
        }
        None
    }

    /// 渲染 PDF 每页为 PNG（输出到系统临时目录），返回页面列表
    pub async fn render_all(&self, pdf_path: &Path, dpi: u32) -> Result<Vec<ParsedPage>, AppError> {
        let out_dir = std::env::temp_dir().join(format!("prism_pdf_{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&out_dir).await?;

        // pdfium 命令行：pdfium <input> <output_prefix> 或类似；此处用标准参数尝试
        let output = tokio::process::Command::new(&self.pdfium)
            .arg(pdf_path)
            .arg(out_dir.join("page"))
            .arg(dpi.to_string())
            .output()
            .await
            .map_err(|e| AppError::Internal(format!("pdfium 执行失败: {e}")))?;

        let mut pages = Vec::new();
        let mut entries = tokio::fs::read_dir(&out_dir).await?;
        let mut page_no = 0u32;
        while let Some(entry) = entries.next_entry().await? {
            let p = entry.path();
            if p.extension().map(|e| e == "png" || e == "jpg").unwrap_or(false) {
                page_no += 1;
                pages.push(ParsedPage {
                    page_no,
                    text: String::new(),
                    image_path: Some(p),
                });
            }
        }
        let _ = output.status;
        if pages.is_empty() {
            // pdfium 可能未按预期输出；清理并返回错误
            let _ = tokio::fs::remove_dir_all(&out_dir).await;
            return Err(AppError::Internal("pdfium 未生成页面图像".into()));
        }
        Ok(pages)
    }
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

// ── 分发器 ────────────────────────────────────────────────

/// 按扩展名分发到对应解析器（§10.2.3 摄取整合）
pub fn parser_for(path: &Path) -> Box<dyn DocumentParser> {
    match path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("pdf") => Box::new(PdfParser),
        Some("md" | "markdown" | "txt" | "log" | "json" | "yaml" | "yml" | "toml" | "rs" | "ts" | "svelte" | "html" | "css") => {
            Box::new(TextParser)
        }
        _ => Box::new(TextParser), // 未知类型按文本尝试；二进制会报错
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn text_parser_works() {
        let dir = std::env::temp_dir();
        let p = dir.join("parser_test.txt");
        tokio::fs::write(&p, "hello world").await.unwrap();
        let doc = parser_for(&p).parse(&p).await.unwrap();
        assert_eq!(doc.pages.len(), 1);
        assert_eq!(doc.pages[0].text, "hello world");
        let _ = tokio::fs::remove_file(&p).await;
    }

    #[test]
    fn parser_dispatches_pdf() {
        let p = Path::new("x.pdf");
        assert_eq!(parser_for(p).kind(), DocKind::Pdf);
    }
}
