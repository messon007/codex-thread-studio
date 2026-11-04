//! Portable PDF navigation data and validation. Never execute PDF actions.
use serde::{Deserialize, Serialize};
pub const MAX_LINKS: usize = 256;
pub const MAX_URL: usize = 4096;
pub const MAX_REGIONS: usize = 512;
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
impl Rect {
    pub fn valid(&self) -> bool {
        [self.x, self.y, self.width, self.height]
            .iter()
            .all(|v| v.is_finite())
            && self.x >= 0.0
            && self.y >= 0.0
            && self.width > 0.0
            && self.height > 0.0
            && self.x + self.width <= 1.0002
            && self.y + self.height <= 1.0002
    }
    #[allow(dead_code)] // Used by the native GTK client, not the gateway decoder.
    pub fn contains(&self, point: [f64; 2]) -> bool {
        point.iter().all(|v| v.is_finite())
            && point[0] >= self.x
            && point[0] <= self.x + self.width
            && point[1] >= self.y
            && point[1] <= self.y + self.height
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Target {
    Page { page: usize },
    Url { url: String },
}
impl Target {
    pub fn valid(&self, pages: usize) -> bool {
        match self {
            Self::Page { page } => *page < pages,
            Self::Url { url } => web_url(url).as_deref() == Some(url),
        }
    }
    #[allow(dead_code)] // Native presentation for this shared transport type.
    pub fn label(&self) -> String {
        match self {
            Self::Page { page } => format!("Go to page {}", page.saturating_add(1)),
            Self::Url { url } => url.clone(),
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Link {
    pub rects: Vec<Rect>,
    pub target: Target,
}
impl Link {
    pub fn valid(&self, pages: usize) -> bool {
        !self.rects.is_empty()
            && self.rects.len() <= 8
            && self.rects.iter().all(Rect::valid)
            && self.target.valid(pages)
    }
}
pub fn web_url(raw: &str) -> Option<String> {
    if raw.is_empty() || raw.len() > MAX_URL || raw.chars().any(|c| c.is_control()) {
        return None;
    }
    let url = url::Url::parse(raw).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return None;
    }
    let value = url.to_string();
    (value.len() <= MAX_URL).then_some(value)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn web_targets_are_explicit_http_urls_without_credentials_or_controls() {
        assert_eq!(
            web_url("HTTPS://example.com/中文#section"),
            Some("https://example.com/%E4%B8%AD%E6%96%87#section".into())
        );
        for raw in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "mailto:a@example.com",
            "data:text/plain,test",
            "../other.pdf",
            "//example.com/a",
            "https://user:secret@example.com",
            "https://example.com/\npath",
            "",
        ] {
            assert!(web_url(raw).is_none(), "accepted {raw}");
        }
        let rect = Rect {
            x: 0.1,
            y: 0.2,
            width: 0.3,
            height: 0.1,
        };
        assert!(rect.valid() && rect.contains([0.2, 0.25]));
        assert!(!rect.contains([f64::NAN, 0.25]));
        assert!(!Target::Page { page: 3 }.valid(3));
        assert!(!Target::Url {
            url: "https://EXAMPLE.com".into()
        }
        .valid(3));
    }
}
