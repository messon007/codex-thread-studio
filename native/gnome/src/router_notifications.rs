//! Bounded transition admission for Router alerts; never infer completion from
//! controller SDK activity or replay terminal history as a new result.
use crate::router::{History, Registry, Tracked};
use std::collections::{BTreeSet, HashSet, VecDeque};
const MAX_ACTIVE: usize = 256; // Gateway: 64 controllers, four active jobs each.
const MAX_FINISHED: usize = 512;
type Identity = (String, String);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Completed,
    Failed,
    Clarify,
}
#[derive(Debug)]
pub struct Alert {
    pub controller: String,
    pub kind: Kind,
}
#[derive(Default)]
pub struct Tracker {
    active: BTreeSet<Identity>,
    finished: VecDeque<Identity>,
    finished_keys: HashSet<Identity>,
}
impl Tracker {
    fn identity(controller: &str, id: &str) -> Option<Identity> {
        let (backend, thread) = controller.split_once(':')?;
        if id.is_empty()
            || id.len() > 64
            || id.chars().any(char::is_control)
            || crate::notification_ids::tag(backend, thread).is_none()
        {
            return None;
        }
        Some((controller.into(), id.into()))
    }
    fn watch(&mut self, controller: &str, id: &str) {
        let Some(key) = Self::identity(controller, id) else {
            return;
        };
        if !self.finished_keys.contains(&key)
            && !self.active.contains(&key)
            && self.active.len() < MAX_ACTIVE
        {
            self.active.insert(key);
        }
    }
    pub fn observe(&mut self, row: &Tracked, accepted: bool) -> Option<Alert> {
        let key = Self::identity(&row.controller, &row.id)?;
        let kind = match row.status.as_deref() {
            Some("routing" | "dispatching" | "running") => {
                self.watch(&row.controller, &row.id);
                return None;
            }
            Some("completed") => Some(Kind::Completed),
            Some("failed") => Some(Kind::Failed),
            Some("clarify") => Some(Kind::Clarify),
            Some("stopped") | None => None,
            _ => return None,
        };
        let observed = self.active.remove(&key);
        if self.finished_keys.contains(&key) {
            return None;
        }
        self.finished_keys.insert(key.clone());
        self.finished.push_back(key);
        if self.finished.len() > MAX_FINISHED {
            if let Some(old) = self.finished.pop_front() {
                self.finished_keys.remove(&old);
            }
        }
        if observed || accepted {
            kind.map(|kind| Alert {
                controller: row.controller.clone(),
                kind,
            })
        } else {
            None
        }
    }
    /// Latest summaries already exist for every controller. Observe previous
    /// watches first, then seed active jobs; an initial terminal result is history.
    pub fn sync(
        &mut self,
        registry: &Registry,
        histories: &[History],
        tracked: &[Tracked],
    ) -> Vec<Alert> {
        self.active
            .retain(|(controller, _)| registry.contains(controller));
        let mut alerts = Vec::new();
        for row in histories.iter().flat_map(|h| &h.statuses).chain(tracked) {
            if registry.contains(&row.controller) {
                if let Some(alert) = self.observe(row, false) {
                    alerts.push(alert);
                }
            }
        }
        for h in histories {
            if registry.contains(&h.controller) {
                for id in &h.active_ids {
                    self.watch(&h.controller, id);
                }
            }
        }
        alerts
    }
    /// Only old active jobs outside the latest summary window need an additional
    /// status-only read. Queue gates retain priority within the shared 128 budget.
    pub fn tracking(&self, histories: &[History], existing: &mut Vec<Identity>) {
        for key in &self.active {
            if existing.len() >= 128 {
                break;
            }
            let in_latest = histories
                .iter()
                .any(|h| h.controller == key.0 && h.ids.contains(&key.1));
            if !in_latest && !existing.contains(key) {
                existing.push(key.clone());
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn row(id: &str, status: Option<&str>) -> Tracked {
        Tracked {
            controller: "codex:router".into(),
            id: id.into(),
            status: status.map(Into::into),
            target_turn_id: String::new(),
        }
    }
    #[test]
    fn history_replay_stops_deletions_and_repeated_results_do_not_alert() {
        let mut t = Tracker::default();
        assert!(t
            .observe(&row("history", Some("completed")), false)
            .is_none());
        assert!(t.observe(&row("history", Some("running")), false).is_none());
        assert!(t.active.is_empty());
        for status in [Some("stopped"), None] {
            t.observe(&row("active", Some("running")), false);
            assert!(t.observe(&row("active", status), false).is_none());
        }
        for (i, status, kind) in [
            ("one", "completed", Kind::Completed),
            ("two", "failed", Kind::Failed),
            ("three", "clarify", Kind::Clarify),
        ] {
            t.observe(&row(i, Some("routing")), true);
            t.observe(&row(i, Some("running")), false);
            assert_eq!(t.observe(&row(i, Some(status)), false).unwrap().kind, kind);
            assert!(t.observe(&row(i, Some(status)), true).is_none());
        }
        assert_eq!(
            t.observe(&row("immediate", Some("clarify")), true)
                .unwrap()
                .kind,
            Kind::Clarify
        );
    }
    #[test]
    fn summaries_track_background_controllers_without_rich_replies() {
        let registry = Registry {
            revision: String::new(),
            controller_backend: "codex".into(),
            controllers: std::collections::BTreeMap::from([("codex".into(), "router".into())]),
        };
        let history = |status: &str| History {
            controller: "codex:router".into(),
            ids: vec!["job".into()],
            statuses: vec![row("job", Some(status))],
            next: None,
            newer: None,
            unread_count: 0,
            active_ids: Vec::new(),
            controller_busy: false,
        };
        let mut t = Tracker::default();
        assert!(t.sync(&registry, &[history("running")], &[]).is_empty());
        let mut requests = Vec::new();
        t.tracking(&[history("running")], &mut requests);
        assert!(
            requests.is_empty(),
            "Latest summaries must not cause duplicate status reads"
        );
        t.tracking(&[], &mut requests);
        assert_eq!(requests, vec![("codex:router".into(), "job".into())]);
        let alerts = t.sync(&registry, &[], &[row("job", Some("completed"))]);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].kind, Kind::Completed);
        assert!(t.sync(&registry, &[history("completed")], &[]).is_empty());
        t.observe(&row("removed", Some("running")), false);
        let mut empty = registry;
        empty.controllers.clear();
        assert!(t.sync(&empty, &[], &[]).is_empty());
        assert!(t.active.is_empty());
    }
    #[test]
    fn watches_are_bounded_and_queue_gates_keep_priority() {
        let mut t = Tracker::default();
        for i in 0..300 {
            t.observe(&row(&format!("active-{i}"), Some("running")), false);
        }
        assert_eq!(t.active.len(), MAX_ACTIVE);
        let mut existing = vec![("opencode:router".into(), "queue-gate".into()); 127];
        t.tracking(&[], &mut existing);
        assert_eq!(existing.len(), 128);
        assert_eq!(existing[0].1, "queue-gate");
        for i in 0..1000 {
            t.observe(&row(&format!("finished-{i}"), Some("completed")), false);
        }
        assert_eq!(t.finished.len(), MAX_FINISHED);
        assert!(t
            .observe(&row("bad\0id", Some("completed")), true)
            .is_none());
    }
}
