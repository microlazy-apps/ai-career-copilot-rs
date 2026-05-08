//! Plain-text extraction for resume attachments.
//!
//! We persist the raw bytes on disk and a normalized plain-text copy in
//! SQLite, so the LLM prompt can be enriched on every chat turn without
//! re-parsing PDF/DOCX. Heavy parsing happens once at upload time and is
//! shielded from `panic!` by `catch_unwind` (some PDFs blow up
//! `pdf-extract`'s text layer).

use std::io::{Cursor, Read};
use std::panic::AssertUnwindSafe;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentKind {
    Pdf,
    Docx,
    Text,
    Markdown,
}

impl AttachmentKind {
    pub fn label(&self) -> &'static str {
        match self {
            AttachmentKind::Pdf => "PDF",
            AttachmentKind::Docx => "DOCX",
            AttachmentKind::Text => "TXT",
            AttachmentKind::Markdown => "Markdown",
        }
    }

    pub fn detect(filename: &str, mime: &str) -> Option<Self> {
        let lower = filename.to_lowercase();
        if lower.ends_with(".pdf") || mime == "application/pdf" {
            return Some(AttachmentKind::Pdf);
        }
        if lower.ends_with(".docx")
            || mime == "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        {
            return Some(AttachmentKind::Docx);
        }
        if lower.ends_with(".md") || lower.ends_with(".markdown") || mime == "text/markdown" {
            return Some(AttachmentKind::Markdown);
        }
        if lower.ends_with(".txt") || mime.starts_with("text/") {
            return Some(AttachmentKind::Text);
        }
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractStatus {
    Ok,
    Partial,
    Failed,
}

impl ExtractStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ExtractStatus::Ok => "ok",
            ExtractStatus::Partial => "partial",
            ExtractStatus::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExtractOutcome {
    pub text: String,
    pub status: ExtractStatus,
    pub error: Option<String>,
}

/// Hard cap on stored text length so an enormous resume doesn't blow up
/// the DB row or the LLM prompt. ~32k chars ~= ~10–12k tokens, well below
/// any sensible context window we still need to leave for chat history.
const MAX_TEXT_CHARS: usize = 32_000;

pub fn extract(kind: AttachmentKind, bytes: &[u8]) -> ExtractOutcome {
    let raw = match kind {
        AttachmentKind::Text | AttachmentKind::Markdown => extract_text(bytes),
        AttachmentKind::Pdf => extract_pdf(bytes),
        AttachmentKind::Docx => extract_docx(bytes),
    };

    match raw {
        Ok(text) => {
            let cleaned = normalize(&text);
            let (capped, truncated) = cap_chars(&cleaned, MAX_TEXT_CHARS);
            ExtractOutcome {
                text: capped,
                status: if truncated {
                    ExtractStatus::Partial
                } else {
                    ExtractStatus::Ok
                },
                error: truncated.then(|| {
                    format!("已截断为前 {MAX_TEXT_CHARS} 个字符（原文超出上限）")
                }),
            }
        }
        Err(msg) => ExtractOutcome {
            text: String::new(),
            status: ExtractStatus::Failed,
            error: Some(msg),
        },
    }
}

fn extract_text(bytes: &[u8]) -> Result<String, String> {
    if let Ok(s) = std::str::from_utf8(bytes) {
        return Ok(s.to_string());
    }
    let (decoded, _, had_errors) = encoding_rs::GBK.decode(bytes);
    if had_errors {
        let (decoded2, _, _) = encoding_rs::UTF_8.decode(bytes);
        return Ok(decoded2.into_owned());
    }
    Ok(decoded.into_owned())
}

fn extract_pdf(bytes: &[u8]) -> Result<String, String> {
    let bytes_owned = bytes.to_vec();
    let result = std::panic::catch_unwind(AssertUnwindSafe(move || {
        pdf_extract::extract_text_from_mem(&bytes_owned)
    }));
    match result {
        Ok(Ok(text)) => Ok(text),
        Ok(Err(e)) => Err(format!("PDF 解析失败：{e}")),
        Err(_) => Err("PDF 解析中断（文件可能已损坏或使用了不支持的编码）".to_string()),
    }
}

fn extract_docx(bytes: &[u8]) -> Result<String, String> {
    let cursor = Cursor::new(bytes);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("DOCX 不是合法的 zip：{e}"))?;

    let mut xml = String::new();
    {
        let mut entry = archive
            .by_name("word/document.xml")
            .map_err(|e| format!("DOCX 缺少 word/document.xml：{e}"))?;
        entry
            .read_to_string(&mut xml)
            .map_err(|e| format!("读取 document.xml 失败：{e}"))?;
    }

    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(false);

    let mut out = String::new();
    let mut buf = Vec::new();
    let mut in_text = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = e.name();
                let local = std::str::from_utf8(name.local_name().as_ref())
                    .unwrap_or("")
                    .to_string();
                if local == "t" {
                    in_text = true;
                } else if local == "p" || local == "br" || local == "tab" {
                    out.push('\n');
                }
            }
            Ok(Event::Empty(ref e)) => {
                let name = e.name();
                let local = std::str::from_utf8(name.local_name().as_ref())
                    .unwrap_or("")
                    .to_string();
                if local == "br" || local == "tab" {
                    out.push('\n');
                }
            }
            Ok(Event::Text(t)) if in_text => {
                let raw = t.decode().unwrap_or_default();
                let unescaped = quick_xml::escape::unescape(&raw)
                    .map(|c| c.into_owned())
                    .unwrap_or_else(|_| raw.into_owned());
                out.push_str(&unescaped);
            }
            Ok(Event::End(ref e)) => {
                let name = e.name();
                let local = std::str::from_utf8(name.local_name().as_ref())
                    .unwrap_or("")
                    .to_string();
                if local == "t" {
                    in_text = false;
                } else if local == "p" {
                    out.push('\n');
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("DOCX XML 解析失败：{e}")),
            _ => {}
        }
        buf.clear();
    }

    Ok(out)
}

fn normalize(text: &str) -> String {
    let mut prev_blank = false;
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            if !prev_blank && !out.is_empty() {
                out.push('\n');
                prev_blank = true;
            }
        } else {
            out.push_str(trimmed);
            out.push('\n');
            prev_blank = false;
        }
    }
    out.trim_end().to_string()
}

fn cap_chars(text: &str, max_chars: usize) -> (String, bool) {
    if text.chars().count() <= max_chars {
        return (text.to_string(), false);
    }
    let truncated: String = text.chars().take(max_chars).collect();
    (truncated, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_kinds() {
        assert_eq!(
            AttachmentKind::detect("Resume.PDF", "application/pdf"),
            Some(AttachmentKind::Pdf)
        );
        assert_eq!(
            AttachmentKind::detect("resume.docx", "application/octet-stream"),
            Some(AttachmentKind::Docx)
        );
        assert_eq!(
            AttachmentKind::detect("resume.md", "application/octet-stream"),
            Some(AttachmentKind::Markdown)
        );
        assert_eq!(
            AttachmentKind::detect("resume.txt", "text/plain"),
            Some(AttachmentKind::Text)
        );
        assert_eq!(
            AttachmentKind::detect("logo.png", "image/png"),
            None
        );
    }

    #[test]
    fn normalizes_blank_lines() {
        let raw = "Header\n\n\n\nBody line\n   \nFooter";
        assert_eq!(normalize(raw), "Header\n\nBody line\n\nFooter");
    }

    #[test]
    fn caps_very_long_text() {
        let s: String = "中".repeat(40_000);
        let (out, truncated) = cap_chars(&s, 32_000);
        assert!(truncated);
        assert_eq!(out.chars().count(), 32_000);
    }

    #[test]
    fn extracts_utf8_text() {
        let outcome = extract(AttachmentKind::Text, "你好 world".as_bytes());
        assert_eq!(outcome.status, ExtractStatus::Ok);
        assert_eq!(outcome.text, "你好 world");
    }
}
