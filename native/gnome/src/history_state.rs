//! Bounded history/live reconciliation; snapshots are immutable and keyed by item.
use crate::model::{self, Item};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
};

#[derive(Clone, Debug, Default)]
pub struct Lifecycle {
    pub active_turn: Option<String>,
    pub active_shell: bool,
}
pub fn lifecycle(turns: &[Value]) -> Result<Lifecycle, String> {
    let mut result = Lifecycle::default();
    for turn in turns {
        let id = model::text(turn, "id");
        if id.is_empty() || id.len() > 256 || id.chars().any(char::is_control) {
            return Err("Invalid turn identity".into());
        }
        if !matches!(
            model::text(turn, "status"),
            "inProgress" | "completed" | "interrupted" | "failed"
        ) {
            return Err("Backend returned an invalid turn state".into());
        }
        if turn["status"] != "inProgress" {
            continue;
        }
        if result.active_turn.is_some() {
            return Err("Backend reported more than one active turn".into());
        }
        result.active_turn = Some(id.into());
        result.active_shell = turn["items"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|item| item["type"] == "commandExecution" && item["source"] == "userShell");
    }
    Ok(result)
}

#[derive(Debug)]
pub struct Changes {
    ids: VecDeque<String>,
    pub lifecycle: bool,
    pub keep_active: bool,
}
impl Default for Changes {
    fn default() -> Self {
        Self {
            ids: VecDeque::new(),
            lifecycle: false,
            keep_active: true,
        }
    }
}
impl Changes {
    pub fn begin(&mut self) {
        self.ids.clear();
        self.lifecycle = false;
    }
    pub fn settled(&mut self) {
        *self = Self::default();
    }
    fn touch(&mut self, id: &str) {
        if id.is_empty() || id.len() > 256 || self.ids.iter().any(|v| v == id) {
            return;
        }
        self.ids.push_back(id.into());
        if self.ids.len() > model::MAX_VISIBLE_ITEMS {
            self.ids.pop_front();
        }
    }
    pub fn weight(&self) -> usize {
        self.ids.capacity() * std::mem::size_of::<String>()
            + self.ids.iter().map(String::capacity).sum::<usize>()
    }
}
/// Apply already prepared events to a live or cached view. No text parsing or GTK.
pub fn observe(
    items: &mut Vec<Arc<Item>>,
    active: &mut Option<String>,
    changes: &mut Changes,
    loading: bool,
    method: &str,
    params: &Value,
    prepared: Option<&Arc<Item>>,
) -> bool {
    match method {
        "turn/started" => {
            *active = params
                .pointer("/turn/id")
                .and_then(Value::as_str)
                .map(str::to_owned);
            changes.lifecycle |= loading;
        }
        "turn/completed" => {
            *active = None;
            changes.lifecycle |= loading;
        }
        "thread/status/changed"
            if params.pointer("/status/type").and_then(Value::as_str) == Some("idle") =>
        {
            *active = None;
            changes.lifecycle |= loading;
        }
        "item/started" | "item/completed" | "turn/plan/updated" => {
            let Some(item) = prepared else {
                return false;
            };
            let mut item = item.clone();
            if item.turn_id.is_empty() {
                if let Some(turn) = active {
                    Arc::make_mut(&mut item).turn_id = turn.clone();
                }
            }
            if loading {
                changes.touch(&item.id);
            }
            if let Some(old) = items.iter_mut().find(|old| old.id == item.id) {
                if item.role == "Assistant"
                    && old.turn_id == item.turn_id
                    && old.origin == item.origin
                {
                    if let Some(rich) = old.rich.as_ref().filter(|rich| {
                        old.body
                            .get(..rich.source_bytes)
                            .is_some_and(|prefix| item.body.starts_with(prefix))
                    }) {
                        // Preserve mapped prefix blocks through completion. The
                        // streaming lane parses any remaining final text using
                        // these immutable blocks; server metadata stays current.
                        let rich = rich.clone();
                        let current = Arc::make_mut(&mut item);
                        current.rich = Some(rich);
                        current.markup = None;
                    }
                }
                *old = item;
            } else {
                items.push(item);
            }
            model::trim_items(items);
        }
        "item/agentMessage/delta"
        | "item/commandExecution/outputDelta"
        | "item/reasoning/summaryTextDelta" => {
            let id = model::text(params, "itemId");
            // A reconnect cannot recreate text that the server never persisted.
            // Wait for a complete item rather than append across a known gap.
            if !changes.keep_active && !changes.ids.iter().any(|v| v == id) {
                return false;
            }
            let Some(item) = items.iter_mut().find(|item| item.id == id) else {
                return false;
            };
            if loading {
                changes.touch(id);
            }
            Arc::make_mut(item).append(model::text(params, "delta"));
        }
        _ => return false,
    }
    true
}

/// Atomic pointer reconciliation with the owning view. Parsing and I/O have
/// already completed on workers; this bounded O(n) commit avoids a second race.
pub fn merge(
    base: Vec<Arc<Item>>,
    live: &[Arc<Item>],
    reported: Lifecycle,
    current_active: Option<&str>,
    changes: &Changes,
) -> (Vec<Arc<Item>>, Lifecycle) {
    let reported_turn = reported.active_turn.clone();
    let active = if changes.lifecycle {
        current_active.map(str::to_owned)
    } else {
        reported.active_turn
    };
    let touched = changes
        .ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let keep = live
        .iter()
        .filter(|item| {
            touched.contains(item.id.as_str())
                || (changes.keep_active && active.as_deref() == Some(item.turn_id.as_str()))
        })
        .map(|item| (item.id.as_str(), item))
        .collect::<HashMap<_, _>>();
    let known = base
        .iter()
        .map(|item| item.id.as_str())
        .collect::<HashSet<_>>();
    let mut before = HashMap::<Option<&str>, Vec<Arc<Item>>>::new();
    let mut anchor = None;
    for item in live.iter().rev() {
        if known.contains(item.id.as_str()) {
            anchor = Some(item.id.as_str());
        } else if keep.contains_key(item.id.as_str()) {
            before.entry(anchor).or_default().push(item.clone());
        }
    }
    let mut output = Vec::with_capacity(base.len() + live.len());
    for item in &base {
        if let Some(mut rows) = before.remove(&Some(item.id.as_str())) {
            rows.reverse();
            output.extend(rows);
        }
        output.push(keep.get(item.id.as_str()).copied().unwrap_or(item).clone());
    }
    if let Some(mut rows) = before.remove(&None) {
        rows.reverse();
        output.extend(rows);
    }
    model::trim_items(&mut output);
    let shell = reported.active_shell && active.is_some() && active == reported_turn;
    (
        output,
        Lifecycle {
            active_turn: active,
            active_shell: shell,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn item(id: &str, text: &str) -> Arc<Item> {
        let mut item = Item::from_value(&json!({"id":id,"type":"agentMessage","text":text}));
        item.turn_id = "turn".into();
        Arc::new(item)
    }
    #[test]
    fn persisted_snapshot_never_duplicates_observed_deltas_or_erases_partial_text() {
        let mut live = vec![item("a", "first ")];
        let mut active = Some("turn".into());
        let mut changes = Changes::default();
        observe(
            &mut live,
            &mut active,
            &mut changes,
            true,
            "item/agentMessage/delta",
            &json!({"itemId":"a","delta":"second 中文😀"}),
            None,
        );
        let (rows, state) = merge(
            vec![item("a", "first second 中文😀")],
            &live,
            Lifecycle {
                active_turn: Some("turn".into()),
                active_shell: false,
            },
            active.as_deref(),
            &changes,
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].body, "first second 中文😀");
        assert!(state.active_turn.is_some());
        let (rows, _) = merge(
            Vec::new(),
            &live,
            Lifecycle {
                active_turn: Some("turn".into()),
                active_shell: false,
            },
            active.as_deref(),
            &changes,
        );
        assert_eq!(rows[0].body, "first second 中文😀");
    }
    #[test]
    fn terminal_event_wins_over_an_earlier_busy_snapshot_and_keeps_new_items() {
        let mut live = Vec::new();
        let mut active = Some("turn".into());
        let mut changes = Changes::default();
        observe(
            &mut live,
            &mut active,
            &mut changes,
            true,
            "item/completed",
            &json!({}),
            Some(&item("done", "final")),
        );
        observe(
            &mut live,
            &mut active,
            &mut changes,
            true,
            "turn/completed",
            &json!({}),
            None,
        );
        let (rows, state) = merge(
            Vec::new(),
            &live,
            Lifecycle {
                active_turn: Some("turn".into()),
                active_shell: true,
            },
            active.as_deref(),
            &changes,
        );
        assert_eq!(rows[0].body, "final");
        assert!(state.active_turn.is_none());
        assert!(!state.active_shell);
    }
    #[test]
    fn missing_active_items_stay_before_their_known_following_item() {
        let live = vec![item("a", "A"), item("b", "partial B"), item("c", "C")];
        let (rows, _) = merge(
            vec![live[0].clone(), live[2].clone()],
            &live,
            Lifecycle {
                active_turn: Some("turn".into()),
                active_shell: false,
            },
            None,
            &Changes::default(),
        );
        assert_eq!(
            rows.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
            ["a", "b", "c"]
        );
    }
    #[test]
    fn reconnect_gaps_do_not_extend_an_old_cached_prefix() {
        let mut changes = Changes {
            keep_active: false,
            ..Default::default()
        };
        let mut live = vec![item("a", "stale prefix")];
        let mut active = None;
        assert!(!observe(
            &mut live,
            &mut active,
            &mut changes,
            true,
            "item/agentMessage/delta",
            &json!({"itemId":"a","delta":"missing middle"}),
            None
        ));
        let (rows, _) = merge(
            Vec::new(),
            &live,
            Lifecycle {
                active_turn: Some("turn".into()),
                active_shell: false,
            },
            None,
            &changes,
        );
        assert!(rows.is_empty());
        observe(
            &mut live,
            &mut active,
            &mut changes,
            true,
            "item/completed",
            &json!({}),
            Some(&item("a", "complete")),
        );
        let (rows, _) = merge(Vec::new(), &live, Lifecycle::default(), None, &changes);
        assert_eq!(rows[0].body, "complete");
    }
    #[test]
    fn lifecycle_requires_known_statuses_and_one_identifiable_active_turn() {
        for turns in [
            json!([{"id":"t"}]),
            json!([{"id":"t","status":"future-status"}]),
            json!([{"id":"","status":"inProgress"}]),
            json!([{"id":"a","status":"inProgress"},{"id":"b","status":"inProgress"}]),
        ] {
            assert!(lifecycle(turns.as_array().unwrap()).is_err());
        }
        let state=lifecycle(&[json!({"id":"shell","status":"inProgress","items":[{"type":"commandExecution","source":"userShell"}]})]).unwrap();
        assert!(state.active_shell);
        let mut changes = Changes::default();
        changes.lifecycle = true;
        let (_, state) = merge(
            Vec::new(),
            &[],
            state,
            Some("different-model-turn"),
            &changes,
        );
        assert!(!state.active_shell);
    }
}
