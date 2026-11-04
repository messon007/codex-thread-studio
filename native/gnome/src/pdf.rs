//! Validated PDF presentation data; decoding and document work stay in Rust workers.
pub fn supports(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|v| v.to_str())
        .is_some_and(|v| v.eq_ignore_ascii_case("pdf"))
}
use serde::Deserialize;
use serde_json::Value;
#[derive(Debug, Deserialize)]
pub struct Heading {
    pub title: String,
    pub page: usize,
    pub depth: usize,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub document_id: String,
    pub name: String,
    pub path: String,
    pub hash: String,
    pub page: usize,
    pub pages: usize,
    pub text: String,
    pub text_limited: bool,
    pub width: f32,
    pub height: f32,
    pub outline: Vec<Heading>,
    pub outline_limited: bool,
    #[serde(default)]
    pub links: Vec<crate::pdf_navigation::Link>,
    #[serde(default)]
    pub links_limited: bool,
    #[serde(default)]
    pub links_unavailable: bool,
}
#[derive(Debug, Deserialize)]
pub struct Hit {
    pub page: usize,
    pub snippet: String,
}
#[derive(Debug, Deserialize)]
pub struct Search {
    pub hits: Vec<Hit>,
    pub next: Option<usize>,
    pub partial: bool,
}
pub use crate::pdf_navigation::Rect;
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Selection {
    pub document_id: String,
    pub hash: String,
    pub page: usize,
    pub excerpt: String,
    pub range: [usize; 2],
    pub rects: Vec<Rect>,
}
fn selection(value: Value) -> Result<Reply, String> {
    let s: Selection =
        serde_json::from_value(value).map_err(|_| "Invalid PDF selection response")?;
    if s.document_id.is_empty()
        || s.document_id.len() > 64
        || s.hash.len() > 128
        || s.page > i32::MAX as usize
        || s.excerpt.trim().is_empty()
        || s.excerpt.len() > 16384
        || s.excerpt.contains('\0')
        || s.range[0] >= s.range[1]
        || s.range[1] > 128000
        || s.range[1] - s.range[0] > 16384
        || s.rects.len() > 128
        || s.rects.iter().any(|r| !r.valid())
    {
        return Err("PDF selection exceeds its presentation budget".into());
    }
    Ok(Reply::Selection(s))
}
#[derive(Debug)]
pub enum Reply {
    Page {
        meta: Page,
        image: crate::images::Frame,
    },
    Search(Search),
    Selection(Selection),
    Closed,
}
#[derive(Debug)]
pub struct Response {
    pub generation: u64,
    pub serial: u64,
    pub kind: &'static str,
    pub result: Result<Reply, String>,
}
fn parse(bytes: Vec<u8>) -> Result<Reply, String> {
    if bytes.len() < 12 || &bytes[..8] != b"STPDF001" {
        return Err("Invalid PDF page response".into());
    }
    let size = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    if size > 2 * 1024 * 1024 || size > bytes.len() - 12 {
        return Err("PDF metadata exceeds its budget".into());
    }
    let meta: Page =
        serde_json::from_slice(&bytes[12..12 + size]).map_err(|_| "Invalid PDF page metadata")?;
    if meta.document_id.is_empty()
        || meta.document_id.len() > 64
        || meta.name.len() > 4096
        || meta.path.len() > 8192
        || meta.hash.len() > 128
        || meta.pages == 0
        || meta.pages > i32::MAX as usize
        || meta.page >= meta.pages
        || meta.text.len() > 256 * 1024
        || meta.text.contains('\0')
        || meta.name.contains('\0')
        || meta.outline.len() > 500
        || meta.links.len() > crate::pdf_navigation::MAX_LINKS
        || meta.links.iter().any(|link| !link.valid(meta.pages))
        || meta
            .links
            .iter()
            .map(|link| link.rects.len())
            .sum::<usize>()
            > crate::pdf_navigation::MAX_REGIONS
        || !meta.width.is_finite()
        || !meta.height.is_finite()
        || meta.width <= 0.0
        || meta.height <= 0.0
        || meta.width > 100000.0
        || meta.height > 100000.0
        || meta.outline.iter().any(|h| {
            h.page >= meta.pages || h.depth > 16 || h.title.len() > 2048 || h.title.contains('\0')
        })
    {
        return Err("PDF page exceeds its presentation budget".into());
    }
    let image = crate::images::Frame::parse(bytes[12 + size..].to_vec())?;
    Ok(Reply::Page { meta, image })
}
pub async fn execute(api: &crate::gateway::Api, kind: &str, body: Value) -> Result<Reply, String> {
    if kind == "resume" {
        return Box::pin(crate::viewer_resume::pdf(api, body)).await;
    }
    if kind == "close" {
        let mut attempt = 0;
        loop {
            let (status, value) = api
                .json_status(reqwest::Method::POST, "/studio/native/pdf", &body)
                .await?;
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS && attempt < 2 {
                attempt += 1;
                tokio::time::sleep(std::time::Duration::from_millis(40 * attempt)).await;
                continue;
            }
            return if status.is_success() {
                Ok(Reply::Closed)
            } else {
                Err(crate::gateway::response_error(status, &value))
            };
        }
    }
    if kind == "open" || kind == "page" {
        let bytes = api.binary("/studio/native/pdf", &body).await?;
        return crate::worker::compute(move || parse(bytes)).await;
    }
    let value = api
        .json(reqwest::Method::POST, "/studio/native/pdf", Some(&body))
        .await?;
    let selecting = kind == "select";
    crate::worker::compute(move || {
        if selecting {
            return selection(value);
        }
        let found: Search =
            serde_json::from_value(value).map_err(|_| "Invalid PDF search response")?;
        if found.hits.len() > 100
            || found.next.is_some_and(|n| n > i32::MAX as usize)
            || found.hits.iter().any(|h| {
                h.page > i32::MAX as usize || h.snippet.len() > 2048 || h.snippet.contains('\0')
            })
        {
            return Err("PDF search exceeds its presentation budget".into());
        }
        Ok(Reply::Search(found))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_transport_rejects_invalid_regions_and_unbounded_ranges() {
        let value = serde_json::json!({"documentId":"snapshot","hash":"hash","page":1,"excerpt":"Unicode 中文😀","range":[10,20],"rects":[{"x":0.1,"y":0.2,"width":0.3,"height":0.02}]});
        assert!(
            matches!(selection(value.clone()),Ok(Reply::Selection(s)) if s.excerpt.ends_with('😀'))
        );
        for (key, invalid) in [
            ("range", serde_json::json!([10, 128001])),
            (
                "rects",
                serde_json::json!([{"x":0.9,"y":0.2,"width":0.3,"height":0.02}]),
            ),
            ("excerpt", serde_json::json!("x".repeat(16385))),
        ] {
            let mut bad = value.clone();
            bad[key] = invalid;
            assert!(selection(bad).is_err(), "accepted invalid {key}");
        }
    }
    fn frame(meta: &Value) -> Vec<u8> {
        let metadata = serde_json::to_vec(meta).unwrap();
        let mut wire = b"STPDF001".to_vec();
        wire.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
        wire.extend_from_slice(&metadata);
        wire.extend_from_slice(b"STIMG001");
        for number in [1u32, 1, 1, 1] {
            wire.extend_from_slice(&number.to_le_bytes());
        }
        wire.extend_from_slice(&[255, 255, 255, 255]);
        wire
    }
    fn metadata() -> Value {
        serde_json::json!({
            "documentId":"private-snapshot", "name":"测试.pdf", "path":"测试.pdf",
            "hash":"test", "page":69999, "pages":70000, "text":"Unicode: 你好",
            "textLimited":false, "width":612, "height":792,
            "outline":[{"title":"章节", "page":69999, "depth":0}], "outlineLimited":false
        })
    }
    #[test]
    fn large_page_numbers_and_unicode_survive_validated_raster_transport() {
        let Reply::Page { meta, image } = parse(frame(&metadata())).unwrap() else {
            panic!("Expected a PDF page");
        };
        assert_eq!(meta.page, 69999);
        assert_eq!(meta.text, "Unicode: 你好");
        assert_eq!(image.pixels.as_ref(), &[255, 255, 255, 255]);
    }
    #[test]
    fn validates_bounded_page_links_without_arbitrary_actions() {
        let mut raw = metadata();
        let valid = serde_json::json!({"rects":[{"x":0.1,"y":0.2,"width":0.3,"height":0.1}],"target":{"kind":"url","url":"https://example.com/"}});
        raw["links"] = serde_json::json!([valid.clone()]);
        assert!(parse(frame(&raw)).is_ok());
        for target in [
            serde_json::json!({"kind":"url","url":"file:///etc/passwd"}),
            serde_json::json!({"kind":"page","page":70000}),
            serde_json::json!({"kind":"launch","path":"other.pdf"}),
        ] {
            raw["links"][0]["target"] = target;
            assert!(parse(frame(&raw)).is_err());
        }
        raw["links"] = serde_json::json!(vec![valid; 257]);
        assert!(parse(frame(&raw)).is_err());
    }
    #[test]
    fn rejects_invalid_navigation_and_incomplete_or_oversized_frames() {
        let valid = metadata();
        for (key, value) in [
            ("pages", serde_json::json!(0)),
            ("page", serde_json::json!(70000)),
            ("height", serde_json::json!(-1)),
            ("text", serde_json::json!("bad\0text")),
            (
                "outline",
                serde_json::json!([{"title":"bad", "page":70000, "depth":0}]),
            ),
        ] {
            let mut meta = valid.clone();
            meta[key] = value;
            assert!(parse(frame(&meta)).is_err(), "accepted invalid {key}");
        }
        let mut wire = frame(&valid);
        wire.pop();
        assert!(parse(wire).is_err());
        let mut wire = frame(&valid);
        wire[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse(wire).is_err());
        assert!(parse(vec![]).is_err());
    }
}
