//! Shared preference decoding and bounded lifecycle deduplication; no GUI dependencies.
use serde::Deserialize;
use std::collections::VecDeque;
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub revision: String,
    pub enabled: Option<bool>,
    #[serde(default)]
    pub validation_error: Option<String>,
}
#[derive(Debug)]
pub enum Request {
    Read,
    Save { revision: String, enabled: bool },
}
pub async fn execute(api: &crate::gateway::Api, request: Request) -> Result<Snapshot, String> {
    let (method, body) = match request {
        Request::Read => (reqwest::Method::GET, None),
        Request::Save { revision, enabled } => (
            reqwest::Method::PATCH,
            Some(serde_json::json!({"expectedRevision":revision,"enabled":enabled})),
        ),
    };
    let value = api
        .json(method, "/studio/native/notifications", body.as_ref())
        .await?;
    crate::worker::compute(move || {
        let snapshot: Snapshot =
            serde_json::from_value(value).map_err(|_| "Invalid notification settings response")?;
        if snapshot.revision.len() != 64
            || !snapshot.revision.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("Invalid notification settings revision".into());
        }
        Ok(snapshot)
    })
    .await
}
#[derive(Default)]
pub struct Requests {
    seen: VecDeque<(String, u64, String)>,
}
impl Requests {
    pub fn claim(&mut self, backend: &str, generation: u64, id: &serde_json::Value) -> bool {
        let id = if let Some(s) = id
            .as_str()
            .filter(|s| s.len() <= 256 && !s.chars().any(char::is_control))
        {
            format!("s:{s}")
        } else if id.is_number() {
            format!("n:{id}")
        } else {
            return false;
        };
        let key = (backend.to_owned(), generation, id);
        if self.seen.contains(&key) {
            return false;
        }
        self.seen.push_back(key);
        if self.seen.len() > 256 {
            self.seen.pop_front();
        }
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn request_duplicates_are_scoped_by_backend_and_connection() {
        let mut book = Requests::default();
        assert!(book.claim("codex", 1, &json!(1)));
        assert!(!book.claim("codex", 1, &json!(1)));
        assert!(book.claim("codex", 2, &json!(1)));
        assert!(book.claim("opencode", 1, &json!(1)));
        assert!(book.claim("codex", 1, &json!("1")));
        for i in 0..300 {
            book.claim("codex", 3, &json!(i));
        }
        assert_eq!(book.seen.len(), 256);
        assert!(!book.claim("codex", 3, &json!({"bad":true})));
    }
}
