//! Worker-prepared, session-scoped command metadata. Raw diff/usage JSON never
//! needs to be parsed, indexed or serialized by GTK.
use crate::{
    byte_cache::{ByteCache, Weight},
    workspace::Text,
};
use serde_json::Value;
use std::{num::NonZeroUsize, sync::Arc};
use studio_markdown_native::text::TextDocument;

#[derive(Clone, Debug)]
pub enum Data {
    Diff(Result<Arc<Text>, String>),
    Usage(Arc<str>),
}
#[derive(Debug)]
pub struct Update {
    pub thread: String,
    pub data: Data,
}

/// Called on the protocol CPU worker before a payload reaches the UI.
pub fn prepare(payload: &mut Value) -> Option<Update> {
    let method = payload["method"].as_str()?;
    if !matches!(method, "turn/diff/updated" | "thread/tokenUsage/updated") {
        return None;
    }
    let is_diff = method == "turn/diff/updated";
    let thread = payload.pointer("/params/threadId")?.as_str()?.to_owned();
    if thread.is_empty() {
        return None;
    }
    let params = &mut payload["params"];
    let data = if is_diff {
        let source = params["diff"].take();
        let source = source.as_str().unwrap_or("");
        let result = if source.len() > 8 * 1024 * 1024
            || source.bytes().filter(|byte| *byte == b'\n').count() > 500_000
        {
            Err("The turn diff exceeds the native viewer budget (8 MiB / 500,000 lines); inspect its files in Git Review".into())
        } else {
            let diff = studio_markdown_native::unified_diff::Index::new(source);
            TextDocument::new(source.to_owned()).map(|document| {
                Arc::new(Text {
                    path: "Conversation turn diff".into(),
                    language: "diff".into(),
                    document,
                    preview: None,
                    html: None,
                    diff: Some(diff),
                    binary: false,
                    truncated: false,
                    file: None,
                })
            })
        };
        Data::Diff(result)
    } else {
        let usage = if params.get("tokenUsage").is_some() {
            params["tokenUsage"].take()
        } else if params.get("usage").is_some() {
            params["usage"].take()
        } else {
            params.clone()
        };
        let mut text = serde_json::to_string_pretty(&usage).unwrap_or_default();
        if text.len() > 8 * 1024 {
            let mut end = 8 * 1024;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            text.truncate(end);
            text.push_str("\n… Usage metadata truncated.");
        }
        // Retain routing identity only; the bounded string above is the display.
        *params = serde_json::json!({"threadId":thread});
        Data::Usage(text.into())
    };
    Some(Update { thread, data })
}

#[derive(Clone, Default)]
pub struct Session {
    pub diff: Option<Result<Arc<Text>, String>>,
    pub usage: Option<Arc<str>>,
}
impl Weight for Session {
    fn weight(&self) -> usize {
        self.usage.as_ref().map_or(0, |text| text.len())
            + self.diff.as_ref().map_or(0, |diff| match diff {
                Ok(text) => text
                    .document
                    .bytes()
                    .saturating_add(text.diff.as_ref().map_or(0, |diff| diff.bytes()))
                    .saturating_add(
                        text.document
                            .lines()
                            .saturating_mul(std::mem::size_of::<usize>()),
                    ),
                Err(error) => error.len(),
            })
    }
}
pub struct Book(ByteCache<String, Session>);
impl Default for Book {
    fn default() -> Self {
        Self(ByteCache::new(
            NonZeroUsize::new(128).unwrap(),
            32 * 1024 * 1024,
        ))
    }
}
impl Book {
    pub fn get(&mut self, key: &str) -> Session {
        self.0
            .update(key, |session| session.clone())
            .unwrap_or_default()
    }
    pub fn observe(&mut self, backend: &str, update: &Update) {
        let key = format!("{backend}:{}", update.thread);
        let mut session = self.get(&key);
        match &update.data {
            Data::Diff(diff) => session.diff = Some(diff.clone()),
            Data::Usage(usage) => session.usage = Some(usage.clone()),
        }
        self.0.insert(key, session);
    }
    pub fn forget(&mut self, key: &str) {
        self.0.remove(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn worker_projection_is_scoped_and_replaces_diff_without_losing_usage() {
        let mut book = Book::default();
        let mut raw = json!({"method":"thread/tokenUsage/updated","params":{"threadId":"t","tokenUsage":{"total":{"totalTokens":42},"modelContextWindow":256000}}});
        book.observe("a", &prepare(&mut raw).unwrap());
        assert!(raw.pointer("/params/tokenUsage").is_none());
        let mut raw = json!({"method":"turn/diff/updated","params":{"threadId":"t","turnId":"turn","diff":"--- a/中文\n+++ b/中文\n+😀\n"}});
        book.observe("a", &prepare(&mut raw).unwrap());
        assert!(raw["params"]["diff"].is_null());
        let current = book.get("a:t");
        assert!(current.usage.unwrap().contains("42"));
        assert!(current
            .diff
            .unwrap()
            .unwrap()
            .document
            .window(1, None, false)
            .unwrap()
            .text
            .contains("+😀"));
        assert!(book.get("b:t").diff.is_none());
        assert!(book.get("a:other").usage.is_none());
        let mut raw = json!({"method":"turn/diff/updated","params":{"threadId":"t","diff":""}});
        book.observe("a", &prepare(&mut raw).unwrap());
        assert_eq!(book.get("a:t").diff.unwrap().unwrap().document.bytes(), 0);
        book.forget("a:t");
        assert!(book.get("a:t").usage.is_none());
    }
    #[test]
    fn over_budget_metadata_stays_bounded_and_malformed_notifications_do_not_route() {
        let mut raw = json!({"method":"turn/diff/updated","params":{"threadId":"t","diff":"\n".repeat(500_001)}});
        assert!(matches!(
            prepare(&mut raw).unwrap().data,
            Data::Diff(Err(_))
        ));
        let mut raw = json!({"method":"thread/tokenUsage/updated","params":{"threadId":"t","usage":"中".repeat(10_000)}});
        let Data::Usage(text) = prepare(&mut raw).unwrap().data else {
            panic!()
        };
        assert!(text.len() < 8300);
        let mut raw = json!({"method":"turn/diff/updated","params":{"diff":"oops"}});
        assert!(prepare(&mut raw).is_none());
    }
}
