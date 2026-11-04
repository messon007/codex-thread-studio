//! Presentation-neutral EPUB metadata and bounded resource transport.
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub fn supports(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|v| v.to_str())
        .is_some_and(|v| v.eq_ignore_ascii_case("epub"))
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    pub root: String,
    pub path: String,
    pub book_hash: String,
    pub cfi: String,
    pub chapter_label: String,
    pub progress: f64,
    pub font_scale: f64,
    pub theme: String,
    pub flow: String,
    pub toc_open: bool,
    #[serde(default)]
    pub updated_at: i64,
}
#[derive(Debug, Deserialize)]
pub struct Chapter {
    pub id: String,
    pub href: String,
    pub label: String,
    pub linear: bool,
}
#[derive(Debug, Deserialize)]
pub struct Heading {
    pub label: String,
    pub href: String,
    pub depth: usize,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Book {
    pub document_id: String,
    pub root: String,
    pub path: String,
    pub hash: String,
    pub title: String,
    pub author: String,
    pub language: String,
    pub direction: String,
    pub layout: String,
    pub spine_node_index: usize,
    pub spine: Vec<Chapter>,
    pub toc: Vec<Heading>,
    pub toc_limited: bool,
    pub saved: Option<Reading>,
}
impl Book {
    pub fn reading(&self, dark: bool) -> Reading {
        self.saved.clone().unwrap_or_else(|| Reading {
            root: self.root.clone(),
            path: self.path.clone(),
            book_hash: self.hash.clone(),
            cfi: String::new(),
            chapter_label: String::new(),
            progress: 0.0,
            font_scale: 1.0,
            theme: if dark { "dark" } else { "light" }.into(),
            flow: "scrolled".into(),
            toc_open: false,
            updated_at: 0,
        })
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.document_id.len() > 64
            || !matches!(self.direction.as_str(), "default" | "ltr" | "rtl")
            || self.document_id.is_empty()
            || self.root.len() > 4096
            || self.path.len() > 8192
            || self.hash.len() > 128
            || self.title.len() > 4096
            || self.author.len() > 4096
            || self.language.len() > 4096
            || self.spine_node_index > 10000
            || self.spine.is_empty()
            || self.spine.len() > 5000
            || self.toc.len() > 2000
            || self
                .spine
                .iter()
                .any(|s| s.href.len() > 8192 || s.label.len() > 4096 || s.id.len() > 512)
            || self
                .toc
                .iter()
                .any(|h| h.href.len() > 8192 || h.label.len() > 4096 || h.depth > 16)
        {
            return Err("EPUB metadata exceeds its presentation budget".into());
        }
        if let Some(saved) = &self.saved {
            if saved.book_hash != self.hash
                || saved.cfi.len() > 8192
                || !saved.progress.is_finite()
                || !(0.0..=1.0).contains(&saved.progress)
                || !(0.75..=2.0).contains(&saved.font_scale)
                || !matches!(saved.theme.as_str(), "light" | "sepia" | "dark")
                || !matches!(saved.flow.as_str(), "scrolled" | "paginated")
            {
                return Err("Invalid EPUB reading state".into());
            }
        }
        Ok(())
    }
}
pub fn cfi_chapter(cfi: &str) -> Option<usize> {
    let base = cfi.strip_prefix("epubcfi(")?.split('!').next()?;
    let number = base
        .split('/')
        .nth(2)?
        .split('[')
        .next()?
        .parse::<usize>()
        .ok()?;
    (number >= 2 && number % 2 == 0).then(|| number / 2 - 1)
}
#[derive(Debug)]
pub struct Asset {
    pub mime: String,
    pub bytes: Vec<u8>,
}
fn asset(mut bytes: Vec<u8>) -> Result<Asset, String> {
    if bytes.len() < 12 || &bytes[..8] != b"STEPUB01" {
        return Err("Invalid EPUB resource response".into());
    }
    let size = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    if size == 0
        || size > 128
        || size > bytes.len() - 12
        || bytes.len() - 12 - size > 16 * 1024 * 1024
    {
        return Err("EPUB resource exceeds its presentation budget".into());
    }
    let mime = std::str::from_utf8(&bytes[12..12 + size])
        .map_err(|_| "Invalid EPUB media type")?
        .to_owned();
    if !mime.bytes().all(|b| b.is_ascii_graphic()) {
        return Err("Invalid EPUB media type".into());
    }
    let data = bytes.split_off(12 + size);
    Ok(Asset { mime, bytes: data })
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Selection {
    pub excerpt: String,
    pub cfi_range: String,
    pub href: String,
}
pub fn selection(raw: &str) -> Result<Selection, String> {
    if raw.len() > 65536 {
        return Err("EPUB selection is too large".into());
    }
    let selection: Selection =
        serde_json::from_str(raw).map_err(|_| "Select text in this EPUB before commenting")?;
    if selection.excerpt.trim().is_empty()
        || selection.excerpt.len() > 16384
        || selection.cfi_range.len() > 8192
        || selection.href.len() > 8192
        || (!selection.cfi_range.is_empty() && cfi_chapter(&selection.cfi_range).is_none())
    {
        return Err("EPUB selection exceeds its bounds".into());
    }
    Ok(selection)
}
#[derive(Debug)]
pub enum Reply {
    Open(Book),
    Search(crate::epub_search::Page),
    Asset(Asset),
    Saved,
    Closed,
}
#[derive(Debug)]
pub struct Response {
    pub generation: u64,
    pub serial: u64,
    pub kind: &'static str,
    pub result: Result<Reply, String>,
}
pub async fn execute(
    api: &crate::gateway::Api,
    kind: &'static str,
    body: Value,
) -> Result<Reply, String> {
    if kind == "resume" {
        return Box::pin(crate::viewer_resume::epub(api, body)).await;
    }
    if kind == "asset" {
        let bytes = api.binary("/studio/native/epub", &body).await?;
        return crate::worker::compute(move || asset(bytes).map(Reply::Asset)).await;
    }
    let mut attempt = 0;
    let value = loop {
        let (status, value) = api
            .json_status(reqwest::Method::POST, "/studio/native/epub", &body)
            .await?;
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS && attempt < 3 {
            attempt += 1;
            tokio::time::sleep(std::time::Duration::from_millis(80 * attempt)).await;
            continue;
        }
        if !status.is_success() {
            return Err(crate::gateway::response_error(status, &value));
        }
        break value;
    };
    crate::worker::compute(move || match kind {
        "search" => {
            let page: crate::epub_search::Page =
                serde_json::from_value(value).map_err(|_| "Invalid EPUB search response")?;
            page.validate()?;
            Ok(Reply::Search(page))
        }
        "open" => {
            let book: Book =
                serde_json::from_value(value).map_err(|_| "Invalid EPUB book response")?;
            book.validate()?;
            Ok(Reply::Open(book))
        }
        "close" => {
            if let Some(error) = value["saveError"].as_str() {
                Err(error.to_owned())
            } else {
                Ok(Reply::Closed)
            }
        }
        _ => Ok(Reply::Saved),
    })
    .await
}
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Message {
    Anchor {
        href: String,
        matched: bool,
    },
    Ready {
        href: String,
    },
    LaidOut {
        href: String,
    },
    Position {
        href: String,
        cfi: String,
        fraction: f64,
    },
    Edge {
        href: String,
        direction: i32,
    },
}
pub fn message(value: String) -> Result<Message, String> {
    if value.len() > 16 * 1024 {
        return Err("EPUB message exceeds its budget".into());
    }
    let message: Message =
        serde_json::from_str(&value).map_err(|_| "Invalid EPUB content message")?;
    match &message {
        Message::Ready { href } | Message::LaidOut { href } | Message::Edge { href, .. }
            if href.len() > 8192 =>
        {
            return Err("Invalid EPUB content address".into())
        }
        Message::Position {
            href,
            cfi,
            fraction,
        } if href.len() > 8192
            || cfi.len() > 8192
            || !fraction.is_finite()
            || !(0.0..=1.0).contains(fraction) =>
        {
            return Err("Invalid EPUB reading position".into())
        }
        Message::Edge { direction, .. } if ![-1, 1].contains(direction) => {
            return Err("Invalid EPUB navigation".into())
        }
        _ => {}
    }
    Ok(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_and_assertion_free_cfis_keep_their_original_spine_positions() {
        assert_eq!(cfi_chapter("epubcfi(/6/4[chapter-two]!/4/2/1:9)"), Some(1));
        assert_eq!(cfi_chapter("epubcfi(/6/2!/4/2/1:0)"), Some(0));
        for value in [
            "",
            "https://example.com",
            "epubcfi(/6/0!/4/2)",
            "epubcfi(/6/3!/4/2)",
        ] {
            assert_eq!(cfi_chapter(value), None);
        }
        assert!(message(serde_json::json!({"kind":"position","href":"studio-epub://test/ch.xhtml","cfi":"epubcfi(/6/2!/4/2)","fraction":2.0}).to_string()).is_err());
    }
    #[test]
    fn resource_wire_rejects_truncation_and_unbounded_mime() {
        let mut bytes = b"STEPUB01".to_vec();
        bytes.extend_from_slice(&8u32.to_le_bytes());
        bytes.extend_from_slice(b"text/cssbody{}");
        let parsed = asset(bytes.clone()).unwrap();
        assert_eq!(parsed.mime, "text/css");
        assert_eq!(parsed.bytes, b"body{}");
        bytes[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(asset(bytes).is_err());
        assert!(asset(vec![]).is_err());
    }
}
