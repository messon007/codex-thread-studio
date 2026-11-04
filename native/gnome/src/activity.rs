//! Background presentation state; loading idle is not a new completion.
use crate::model::text;
use serde_json::Value;
use std::collections::VecDeque;
#[derive(Default)]
pub struct Activity {
    pub active: bool,
    pub attention: bool,
    pub stale: bool,
    finished: VecDeque<String>,
}
impl Activity {
    pub fn finished(&self, turn: &str) -> bool {
        self.finished.iter().any(|id| id == turn)
    }
    pub fn apply(&mut self, method: &str, params: &Value, selected: bool) -> bool {
        let mut completed = false;
        self.stale = false;
        let turn = params
            .pointer("/turn/id")
            .and_then(Value::as_str)
            .or(params["turnId"].as_str())
            .unwrap_or("");
        match method {
            "turn/started" => {
                if turn.is_empty() || !self.finished.iter().any(|id| id == turn) {
                    self.active = true;
                }
            }
            "turn/completed" => {
                let duplicate = !turn.is_empty() && self.finished.iter().any(|id| id == turn);
                if !duplicate && (!turn.is_empty() || self.active) {
                    completed = true;
                    self.attention = !selected;
                }
                self.active = false;
                if !turn.is_empty() && !duplicate {
                    self.finished.push_back(turn.into());
                    if self.finished.len() > 8 {
                        self.finished.pop_front();
                    }
                }
            }
            "thread/status/changed" => {
                let status = params["status"]
                    .as_str()
                    .unwrap_or_else(|| text(&params["status"], "type"));
                self.active = matches!(status, "active" | "running" | "busy" | "retry");
                if matches!(status, "failed" | "systemError") && !selected {
                    self.attention = true;
                }
            }
            _ => {}
        }
        if selected {
            self.attention = false;
        }
        completed
    }
    pub fn read(&mut self, active: bool) {
        self.active = active;
        self.attention = false;
        self.stale = false;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn idle_loading_and_duplicate_completion_do_not_create_attention() {
        let mut activity = Activity::default();
        activity.apply(
            "thread/status/changed",
            &json!({"status":{"type":"idle"}}),
            false,
        );
        assert!(!activity.attention && !activity.active);
        activity.apply("turn/started", &json!({"turn":{"id":"t"}}), false);
        activity.apply("turn/completed", &json!({"turn":{"id":"t"}}), false);
        assert!(activity.attention && !activity.active);
        activity.read(false);
        activity.apply("turn/completed", &json!({"turn":{"id":"t"}}), false);
        activity.apply("turn/started", &json!({"turn":{"id":"t"}}), false);
        assert!(!activity.attention && !activity.active);
    }
    #[test]
    fn selected_completion_is_read_and_background_completion_needs_attention() {
        let mut selected = Activity::default();
        selected.apply("turn/completed", &json!({"turn":{"id":"a"}}), true);
        assert!(!selected.attention);
        let mut background = Activity::default();
        background.apply("turn/completed", &json!({"turn":{"id":"b"}}), false);
        assert!(background.attention);
    }
    #[test]
    fn only_new_live_completions_are_notification_candidates() {
        let mut activity = Activity::default();
        assert!(!activity.apply("turn/completed", &json!({}), false));
        assert!(!activity.apply("turn/started", &json!({}), false));
        assert!(activity.apply("turn/completed", &json!({}), true));
        assert!(!activity.apply("turn/completed", &json!({}), true));
        assert!(activity.apply("turn/completed", &json!({"turn":{"id":"completed"}}), true));
        assert!(!activity.apply("turn/completed", &json!({"turn":{"id":"completed"}}), false));
    }
}
