//! Gateway process notices are separate from a socket's own generation.
use serde_json::Value;

#[derive(Default)]
pub struct Codex {
    generation: Option<u64>,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Notice {
    Forward,
    Ignore,
    Failed(String),
    Gap(Option<u64>),
}
impl Codex {
    pub fn observe(&mut self, payload: &Value) -> Notice {
        if payload.get("id").is_some() {
            return Notice::Forward;
        }
        if payload["method"] == "studio/appServer/lagged" {
            // This reports a gap in the whole stream, not a new process. Even
            // a legacy/unknown count invalidates the cached conversation state.
            return Notice::Gap(payload["params"]["skipped"].as_u64());
        }
        if payload["method"] != "studio/appServer/status" {
            return Notice::Forward;
        }
        if let Some(generation) = payload["params"]["generation"].as_u64() {
            if self.generation.is_some_and(|current| generation < current) {
                return Notice::Ignore;
            }
            self.generation = Some(generation);
        }
        if !matches!(
            payload["params"]["state"].as_str(),
            Some("error" | "stopped")
        ) {
            return Notice::Forward;
        }
        let source = payload["params"]["message"]
            .as_str()
            .filter(|s| !s.is_empty())
            .or_else(|| {
                payload["params"]["reason"]
                    .as_str()
                    .filter(|s| !s.is_empty())
            })
            .unwrap_or("Codex app-server is unavailable");
        // Keep an oversized upstream diagnostic out of GTK layout. Source
        // identifiers and diagnostic text remain literal, never interpreted.
        let mut end = source.len().min(4096);
        while !source.is_char_boundary(end) {
            end -= 1;
        }
        let mut message = source[..end].to_owned();
        if end < source.len() {
            message.push('…');
        }
        Notice::Failed(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn status(state: &str, generation: u64) -> Value {
        json!({"method":"studio/appServer/status","params":{"state":state,"generation":generation,"reason":"exited with status 23"}})
    }
    #[test]
    fn an_explicit_event_gap_invalidates_state_without_advancing_process_generation() {
        let mut lifecycle = Codex::default();
        assert_eq!(lifecycle.observe(&status("ready", 5)), Notice::Forward);
        for count in [0, 17, u64::MAX] {
            assert_eq!(lifecycle.observe(&json!({"method":"studio/appServer/lagged","params":{"skipped":count,"generation":999}})), Notice::Gap(Some(count)));
        }
        for value in [json!(null), json!(-1), json!("17")] {
            assert_eq!(
                lifecycle.observe(
                    &json!({"method":"studio/appServer/lagged","params":{"skipped":value}})
                ),
                Notice::Gap(None)
            );
        }
        assert_eq!(
            lifecycle.observe(
                &json!({"method":"studio/appServer/lagged","id":1,"params":{"skipped":9}})
            ),
            Notice::Forward
        );
        assert_eq!(lifecycle.observe(&status("ready", 5)), Notice::Forward);
        assert_eq!(lifecycle.observe(&status("stopped", 4)), Notice::Ignore);
    }
    #[test]
    fn current_process_failure_is_terminal_but_queued_older_notices_are_ignored() {
        let mut lifecycle = Codex::default();
        assert_eq!(lifecycle.observe(&status("starting", 5)), Notice::Forward);
        assert_eq!(lifecycle.observe(&status("ready", 5)), Notice::Forward);
        assert_eq!(lifecycle.observe(&status("stopped", 4)), Notice::Ignore);
        assert_eq!(lifecycle.observe(&status("ready", 4)), Notice::Ignore);
        assert_eq!(lifecycle.observe(&status("error", 4)), Notice::Ignore);
        assert_eq!(lifecycle.observe(&status("starting", 6)), Notice::Forward);
        assert_eq!(lifecycle.observe(&status("stopped", 5)), Notice::Ignore);
        assert_eq!(
            lifecycle.observe(&status("stopped", 6)),
            Notice::Failed("exited with status 23".into())
        );
    }
    #[test]
    fn start_error_without_a_process_generation_and_legacy_status_are_supported() {
        let mut lifecycle = Codex::default();
        assert_eq!(
            lifecycle
                .observe(&json!({"method":"studio/appServer/status","params":{"state":"ready"}})),
            Notice::Forward
        );
        assert_eq!(lifecycle.observe(&json!({"method":"studio/appServer/status","params":{"state":"error","message":"Cannot start /literal/{command} 中文😀"}})), Notice::Failed("Cannot start /literal/{command} 中文😀".into()));
        assert_eq!(
            lifecycle
                .observe(&json!({"method":"studio/appServer/status","params":{"state":"stopped"}})),
            Notice::Failed("Codex app-server is unavailable".into())
        );
        assert_eq!(lifecycle.observe(&json!({"method":"studio/appServer/status","id":1,"params":{"state":"stopped","generation":99}})), Notice::Forward);
        assert_eq!(lifecycle.observe(&json!({"method":"thread/status/changed","params":{"state":"stopped","generation":100}})), Notice::Forward);
        assert_eq!(lifecycle.observe(&status("ready", 1)), Notice::Forward);
    }
    #[test]
    fn diagnostic_is_bounded_on_a_unicode_boundary() {
        let message = format!("x{}", "😀".repeat(2000));
        assert_eq!(Codex::default().observe(&json!({"method":"studio/appServer/status","params":{"state":"error","message":message}})), Notice::Failed(format!("x{}…", "😀".repeat(1023))));
        assert_eq!(Codex::default().observe(&json!({"method":"studio/appServer/status","params":{"state":"stopped","message":"","reason":"😀".repeat(2000)}})), Notice::Failed(format!("{}…", "😀".repeat(1024))));
    }
}
