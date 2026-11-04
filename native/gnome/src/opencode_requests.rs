//! OpenCode request identity is owned by one authenticated project/connection.
//! Keep only routing metadata; a failed mutation is never automatically retried.
use serde_json::Value;
use std::collections::{HashMap, VecDeque};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Route {
    pub session: String,
    pub directory: String,
    claimed: bool,
}
#[derive(Default)]
pub struct Book {
    pending: HashMap<String, Route>,
    resolved: VecDeque<(String, String, String)>,
    resolution_sequence: u64,
}
pub fn directory(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 4096
        && std::path::Path::new(value).is_absolute()
        && !value.chars().any(char::is_control)
}
fn identity(value: &str) -> bool {
    !value.is_empty() && value.len() <= 240 && !value.chars().any(char::is_control)
}
impl Book {
    pub fn snapshot(&self) -> u64 {
        self.resolution_sequence
    }
    pub fn snapshot_valid(&self, marker: u64) -> bool {
        self.resolution_sequence
            .saturating_sub(self.resolved.len() as u64)
            <= marker
    }
    pub fn ask(
        &mut self,
        kind: &str,
        properties: &Value,
        cwd: &str,
    ) -> Result<Option<String>, String> {
        let id = properties["id"]
            .as_str()
            .filter(|id| identity(id))
            .ok_or("Invalid OpenCode request identity")?;
        let session = properties["sessionID"]
            .as_str()
            .filter(|id| identity(id))
            .ok_or("Invalid OpenCode request session")?;
        if !directory(cwd) {
            return Err("OpenCode request has no originating project; open its session to recover pending requests".into());
        }
        if serde_json::to_vec(properties)
            .map_err(|_| "Invalid OpenCode request metadata")?
            .len()
            > 256 * 1024
        {
            return Err(
                "OpenCode request metadata exceeds 256 KiB; inspect the source conversation".into(),
            );
        }
        let key = format!("{kind}:{id}");
        if self
            .resolved
            .iter()
            .any(|(k, s, d)| k == &key && s == session && d == cwd)
        {
            return Ok(None);
        }
        if let Some(previous) = self.pending.get(&key) {
            if previous.directory != cwd || previous.session != session {
                return Err("OpenCode reused a pending request ID in a different scope".into());
            }
            return Ok((!previous.claimed).then_some(key));
        }
        if self.pending.len() >= 256 {
            return Err("OpenCode pending request inventory exceeds 256 entries".into());
        }
        self.pending.insert(
            key.clone(),
            Route {
                session: session.into(),
                directory: cwd.into(),
                claimed: false,
            },
        );
        Ok(Some(key))
    }
    pub fn resolve(&mut self, kind: &str, properties: &Value, cwd: &str) -> Option<String> {
        let id = properties["requestID"].as_str().filter(|id| identity(id))?;
        let session = properties["sessionID"].as_str().filter(|id| identity(id))?;
        if !directory(cwd) {
            return None;
        }
        let key = format!("{kind}:{id}");
        if self
            .pending
            .get(&key)
            .is_some_and(|r| r.directory != cwd || r.session != session)
        {
            return None;
        }
        self.pending.remove(&key);
        if !self
            .resolved
            .iter()
            .any(|(k, s, d)| k == &key && s == session && d == cwd)
        {
            self.resolution_sequence = self.resolution_sequence.saturating_add(1);
            self.resolved
                .push_back((key.clone(), session.into(), cwd.into()));
            if self.resolved.len() > 512 {
                self.resolved.pop_front();
            }
        }
        Some(key)
    }
    pub fn claim(&mut self, id: &Value) -> Result<Route, String> {
        let key = id.as_str().ok_or("Invalid OpenCode interaction identity")?;
        let route = self
            .pending
            .get_mut(key)
            .ok_or("OpenCode request is no longer pending; refresh its source session")?;
        if route.claimed {
            return Err(
                "This OpenCode response was already submitted; refresh before trying again".into(),
            );
        }
        route.claimed = true;
        Ok(route.clone())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn replies_keep_origin_and_cannot_be_claimed_twice_or_rebound() {
        let mut b = Book::default();
        let p = json!({"id":"p","sessionID":"s"});
        assert_eq!(
            b.ask("permission", &p, "/original 中文").unwrap(),
            Some("permission:p".into())
        );
        assert!(b.ask("permission", &p, "/different").is_err());
        let r = b.claim(&json!("permission:p")).unwrap();
        assert_eq!(r.directory, "/original 中文");
        assert!(b.claim(&json!("permission:p")).is_err());
        assert!(b.ask("permission", &p, "/original 中文").unwrap().is_none());
    }
    #[test]
    fn resolved_snapshots_and_foreign_events_cannot_resurrect_or_close_a_request() {
        let mut b = Book::default();
        let ask = json!({"id":"q","sessionID":"s"});
        let done = json!({"requestID":"q","sessionID":"s"});
        b.ask("question", &ask, "/project").unwrap();
        assert!(b.resolve("question", &done, "/other").is_none());
        assert!(b
            .resolve(
                "question",
                &json!({"requestID":"q","sessionID":"foreign"}),
                "/project"
            )
            .is_none());
        assert!(b.resolve("question", &done, "/project").is_some());
        assert!(b.ask("question", &ask, "/project").unwrap().is_none());
        assert!(b.claim(&json!("question:q")).is_err());
        let unseen = json!({"requestID":"unseen","sessionID":"s"});
        b.resolve("question", &unseen, "/project");
        assert!(b
            .ask(
                "question",
                &json!({"id":"unseen","sessionID":"s"}),
                "/project"
            )
            .unwrap()
            .is_none());
    }
    #[test]
    fn bounds_reject_unscoped_or_oversized_requests_without_eviction() {
        let mut b = Book::default();
        let p = json!({"id":"p","sessionID":"s"});
        assert!(b.ask("permission", &p, "").is_err());
        assert!(b
            .ask(
                "permission",
                &json!({"id":"large","sessionID":"s","metadata":"x".repeat(262145)}),
                "/p"
            )
            .is_err());
        for i in 0..256 {
            b.ask(
                "permission",
                &json!({"id":i.to_string(),"sessionID":"s"}),
                "/p",
            )
            .unwrap();
        }
        assert!(b.ask("permission", &p, "/p").is_err());
        assert!(b.claim(&json!("permission:0")).is_ok());
    }
    #[test]
    fn snapshots_cannot_outlive_evicted_resolution_evidence() {
        let mut b = Book::default();
        let old = b.snapshot();
        for i in 0..513 {
            b.resolve(
                "question",
                &json!({"requestID":i.to_string(),"sessionID":"s"}),
                "/p",
            );
        }
        assert!(!b.snapshot_valid(old));
        assert!(b.snapshot_valid(b.snapshot()));
        assert_eq!(b.resolved.len(), 512);
    }
}
