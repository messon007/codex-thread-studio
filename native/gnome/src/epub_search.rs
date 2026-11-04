//! Bounded search-page projection; EPUB text remains on the Rust gateway.
use serde::Deserialize;
#[derive(Clone, Debug, Deserialize)]
pub struct Section {
    pub chapter: usize,
    pub href: String,
    pub label: String,
    pub count: usize,
    pub limited: bool,
    pub snippet: String,
}
#[derive(Debug, Deserialize)]
pub struct Issue {
    pub chapter: usize,
    pub label: String,
    pub error: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub book_hash: String,
    pub query: String,
    pub case_sensitive: bool,
    pub start: usize,
    pub scanned: usize,
    pub total: usize,
    pub next: Option<usize>,
    pub sections: Vec<Section>,
    pub issues: Vec<Issue>,
}
impl Page {
    pub fn validate(&self) -> Result<(), String> {
        let end = self
            .start
            .checked_add(self.scanned)
            .ok_or("Invalid EPUB search range")?;
        if self.query.is_empty()
            || self.query.len() > 512
            || self.book_hash.len() > 128
            || self.total == 0
            || self.total > 5000
            || self.scanned == 0
            || self.scanned > 16
            || end > self.total
            || self.next != if end < self.total { Some(end) } else { None }
            || self.sections.len() > 16
            || self.issues.len() > 16
            || self.sections.iter().any(|s| {
                s.chapter < self.start
                    || s.chapter >= end
                    || s.href.len() > 8192
                    || s.label.len() > 4096
                    || s.snippet.len() > 900
                    || s.count == 0
                    || s.count > 10000
            })
            || self
                .sections
                .windows(2)
                .any(|pair| pair[0].chapter >= pair[1].chapter)
            || self.issues.iter().any(|i| {
                i.chapter < self.start
                    || i.chapter >= end
                    || i.label.len() > 4096
                    || i.error.len() > 4096
            })
        {
            return Err("Invalid EPUB search page".into());
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_misaligned_or_oversized_search_pages_and_preserves_literal_query() {
        let raw = serde_json::json!({"bookHash":"hash","query":"needle ","caseSensitive":true,"start":16,"scanned":16,"total":40,"next":32,"sections":[{"chapter":17,"href":"OPS/a.xhtml","label":"Unicode 中文😀","count":2,"limited":false,"snippet":"needle trailing"}],"issues":[]});
        let page: Page = serde_json::from_value(raw.clone()).unwrap();
        page.validate().unwrap();
        assert_eq!(page.query, "needle ");
        for field in ["next", "scanned", "total"] {
            let mut bad = raw.clone();
            bad[field] = serde_json::json!(5001);
            assert!(serde_json::from_value::<Page>(bad)
                .unwrap()
                .validate()
                .is_err());
        }
        let mut bad = raw;
        bad["sections"][0]["chapter"] = serde_json::json!(15);
        assert!(serde_json::from_value::<Page>(bad)
            .unwrap()
            .validate()
            .is_err());
    }
}
