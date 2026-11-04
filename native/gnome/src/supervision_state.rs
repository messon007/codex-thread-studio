//! Native UI ownership and queue gates. No provider work or persistent state.
use crate::{session_search::Context, turn_supervision::Stage, turn_supervision_runtime::View};
use std::collections::BTreeMap;
pub fn scope(
    source: &Source,
    value: serde_json::Value,
) -> Result<Option<(String, String)>, String> {
    if value["backend"] != source.context.backend
        || value["threadId"] != source.context.thread
        || value["turnId"] != source.turn
        || value["cwd"] != source.directory
        || !value
            .as_object()
            .is_some_and(|m| m.len() == 5 && m.contains_key("route"))
    {
        return Err("Supervision ownership response changed its source identity".into());
    }
    let route = &value["route"];
    if route.is_null() {
        return Ok(None);
    }
    let controller = route["controller"].as_str().unwrap_or("");
    let id = route["id"].as_str().unwrap_or("");
    if !route.as_object().is_some_and(|m| m.len() == 2)
        || controller.len() > 320
        || controller.chars().any(char::is_control)
        || controller
            .split_once(':')
            .is_none_or(|(b, t)| b.is_empty() || t.is_empty() || t.len() > 256)
        || id.is_empty()
        || id.len() > 64
        || id.chars().any(char::is_control)
    {
        return Err("Invalid routed supervision ownership".into());
    }
    Ok(Some((controller.into(), id.into())))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub context: Context,
    pub directory: String,
    pub title: String,
    pub turn: String,
    pub controller: Option<String>,
}
impl Source {
    pub fn key(&self) -> String {
        format!("{}:{}", self.context.backend, self.context.thread)
    }
}
pub struct Entry {
    pub source: Source,
    pub request: u64,
    pub issued: bool,
    pub stopping: bool,
    pub view: Option<View>,
    pub reason: String,
    holding: bool,
    retired: bool,
    discovering: bool,
}
impl Entry {
    pub fn discovering(&self) -> bool {
        self.holding && self.discovering && !self.retired
    }
    pub fn status_applies(&self, source: &Source) -> bool {
        self.holding
            || self.source.context == source.context
                && self.source.directory == source.directory
                && self
                    .view
                    .as_ref()
                    .map_or(self.source.turn.as_str(), |v| v.current.as_str())
                    == source.turn
    }
    pub fn holding(&self) -> bool {
        self.holding
    }
    pub fn active(&self) -> bool {
        self.holding
            && !self.stopping
            && !self.retired
            && self
                .view
                .as_ref()
                .is_none_or(|v| !matches!(v.stage, Stage::Stopped | Stage::Complete))
    }
}
#[derive(Default)]
pub struct Book {
    pub entries: BTreeMap<String, Entry>,
}
impl Book {
    pub fn begin(&mut self, source: Source, request: u64) -> Result<(), String> {
        if request == 0
            || source.turn.is_empty()
            || source.turn.len() > 256
            || source.turn.chars().any(char::is_control)
            || source.context.thread.is_empty()
            || source.context.thread.len() > 256
            || source.context.backend.is_empty()
            || source.directory.is_empty()
            || source.directory.len() > 4096
            || source.directory.contains('\0')
        {
            return Err("Invalid supervision source".into());
        }
        let key = source.key();
        if self.entries.get(&key).is_some_and(Entry::holding) {
            return Err("This session already has supervised work or an unsettled receipt".into());
        }
        if !self.entries.contains_key(&key) && self.entries.len() >= 16 {
            let evict = self
                .entries
                .iter()
                .find(|(_, e)| !e.holding)
                .map(|(k, _)| k.clone())
                .ok_or("Too many supervised sessions; stop an existing supervisor first")?;
            self.entries.remove(&evict);
        }
        self.entries.insert(
            key,
            Entry {
                source,
                request,
                issued: false,
                stopping: false,
                view: None,
                reason: String::new(),
                holding: true,
                retired: false,
                discovering: true,
            },
        );
        Ok(())
    }
    pub fn current(&self, context: &Context, request: u64) -> Option<&Entry> {
        self.entries
            .get(&format!("{}:{}", context.backend, context.thread))
            .filter(|e| !e.retired && e.request == request && e.source.context == *context)
    }
    pub fn may_prepare(&self, context: &Context, request: u64, root: &str) -> bool {
        self.current(context, request)
            .is_some_and(|e| e.issued && e.active() && e.source.directory == root)
    }
    pub fn discovering(&self) -> bool {
        self.entries.values().any(Entry::discovering)
    }
    pub fn resolve(
        &mut self,
        context: &Context,
        request: u64,
        controller: Option<String>,
    ) -> Result<(Source, bool), String> {
        let e = self
            .current(context, request)
            .filter(|e| e.discovering())
            .ok_or("Supervision ownership request is no longer current")?;
        if e.source
            .controller
            .as_ref()
            .is_some_and(|hint| Some(hint) != controller.as_ref())
        {
            return Err("Router ownership changed before supervision started".into());
        }
        let e = self
            .entries
            .get_mut(&format!("{}:{}", context.backend, context.thread))
            .unwrap();
        e.source.controller = controller;
        e.discovering = false;
        if e.stopping {
            e.holding = false;
            e.retired = true;
        }
        Ok((e.source.clone(), e.stopping))
    }
    pub fn issued(&mut self, context: &Context, request: u64) -> bool {
        if !self
            .current(context, request)
            .is_some_and(|e| e.active() && !e.discovering)
        {
            return false;
        }
        self.entries
            .get_mut(&format!("{}:{}", context.backend, context.thread))
            .unwrap()
            .issued = true;
        true
    }
    pub fn apply(&mut self, view: View) -> bool {
        if self.current(&view.context, view.request).is_none() {
            return false;
        }
        let e = self
            .entries
            .get_mut(&format!("{}:{}", view.context.backend, view.context.thread))
            .unwrap();
        e.holding = view.holding;
        e.reason = view.reason.clone();
        e.view = Some(view);
        true
    }
    pub fn reject(&mut self, context: &Context, request: u64, reason: &str) -> bool {
        if self.current(context, request).is_none() {
            return false;
        }
        let e = self
            .entries
            .get_mut(&format!("{}:{}", context.backend, context.thread))
            .unwrap();
        e.reason = reason.into();
        e.retired = true;
        e.holding = false;
        e.stopping = true;
        e.discovering = false;
        true
    }
    /// An issued job retains its queue gate until the backend confirms settlement.
    pub fn stopping(&mut self, key: &str, reason: &str) -> Option<(Source, bool)> {
        let e = self.entries.get_mut(key).filter(|e| e.holding)?;
        e.stopping = true;
        e.reason = reason.into();
        if !e.issued && !e.discovering {
            e.retired = true;
            e.holding = false;
        }
        Some((e.source.clone(), e.issued))
    }
    pub fn holds(&self, key: &str) -> bool {
        self.entries.iter().any(|(source, e)| {
            e.holding && (source == key || e.source.controller.as_deref() == Some(key))
        })
    }
    pub fn forget_backend(&mut self, backend: &str) -> Vec<Source> {
        let mut sources = Vec::new();
        for e in self
            .entries
            .values_mut()
            .filter(|e| e.source.context.backend == backend && e.holding)
        {
            sources.push(e.source.clone());
            e.holding = false;
            e.retired = true;
            e.stopping = true;
            e.discovering = false;
            e.reason = "Backend changed or disconnected".into();
        }
        sources
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source() -> Source {
        Source {
            context: Context {
                backend: "codex".into(),
                thread: "target".into(),
                generation: 3,
            },
            directory: "/project".into(),
            title: "Target".into(),
            turn: "running".into(),
            controller: Some("codex:router".into()),
        }
    }
    fn view(request: u64, stage: Stage, holding: bool) -> View {
        View {
            request,
            context: source().context,
            root: "running".into(),
            current: "running".into(),
            stage,
            count: 0,
            reason: "Result".into(),
            warning: String::new(),
            holding,
        }
    }
    fn resolved(b: &mut Book, request: u64) {
        b.resolve(&source().context, request, source().controller)
            .unwrap();
    }
    #[test]
    fn opt_in_holds_source_and_controller_before_options_or_backend_acknowledgement() {
        let mut b = Book::default();
        b.begin(source(), 4).unwrap();
        assert!(b.holds("codex:target") && b.holds("codex:router"));
        assert!(!b.holds("codex:other"));
        assert!(b.begin(source(), 5).is_err());
        assert!(!b.may_prepare(&source().context, 4, "/project"));
        assert!(!b.issued(&source().context, 4));
        resolved(&mut b, 4);
        assert!(b.issued(&source().context, 4));
        assert!(b.may_prepare(&source().context, 4, "/project"));
        assert!(!b.may_prepare(&source().context, 4, "/other"));
    }
    #[test]
    fn old_requests_and_connection_generations_cannot_release_a_new_queue_gate() {
        let mut b = Book::default();
        b.begin(source(), 4).unwrap();
        b.reject(&source().context, 4, "cancelled");
        b.begin(source(), 5).unwrap();
        assert!(!b.apply(view(4, Stage::Stopped, false)));
        assert!(!b.reject(&source().context, 4, "stale error"));
        let mut old = view(5, Stage::Complete, false);
        old.context.generation = 2;
        assert!(!b.apply(old));
        assert!(b.holds("codex:router"));
    }
    #[test]
    fn stop_before_issue_retires_settings_but_stop_after_issue_waits_for_receipt_cleanup() {
        let mut b = Book::default();
        b.begin(source(), 4).unwrap();
        resolved(&mut b, 4);
        assert!(!b.stopping("codex:target", "user stop").unwrap().1);
        assert!(!b.holds("codex:target"));
        assert!(!b.issued(&source().context, 4));
        b.begin(source(), 5).unwrap();
        resolved(&mut b, 5);
        b.issued(&source().context, 5);
        assert!(b.stopping("codex:target", "user stop").unwrap().1);
        assert!(b.holds("codex:router"));
        assert!(!b.may_prepare(&source().context, 5, "/project"));
        b.apply(view(5, Stage::Stopped, true));
        assert!(b.holds("codex:target"));
        b.apply(view(5, Stage::Stopped, false));
        assert!(!b.holds("codex:router"));
    }
    #[test]
    fn disconnected_owners_do_not_revive_from_buffered_updates() {
        let mut b = Book::default();
        b.begin(source(), 4).unwrap();
        resolved(&mut b, 4);
        b.issued(&source().context, 4);
        assert_eq!(b.forget_backend("codex"), vec![source()]);
        assert!(!b.apply(view(4, Stage::Waiting, true)));
        assert!(!b.holds("codex:target"));
        let mut newer = source();
        newer.context.generation = 4;
        b.begin(newer.clone(), 5).unwrap();
        b.resolve(&newer.context, 5, newer.controller.clone())
            .unwrap();
        assert!(b.issued(&newer.context, 5));
    }
    #[test]
    fn completion_releases_gates_and_admission_is_bounded_without_evicting_live_owners() {
        let mut b = Book::default();
        for i in 0..16 {
            let mut s = source();
            s.context.thread = format!("target-{i}");
            b.begin(s, i + 1).unwrap();
        }
        assert!(b.begin(source(), 20).is_err());
        let e = b.entries.values().next().unwrap();
        let mut v = view(e.request, Stage::Complete, false);
        v.context = e.source.context.clone();
        assert!(b.apply(v));
        b.begin(source(), 20).unwrap();
        assert_eq!(b.entries.len(), 16);
        assert!(b.holds("codex:target"));
    }
    #[test]
    fn cold_target_scope_resolves_controller_without_reviving_cancelled_or_old_requests() {
        let mut b = Book::default();
        let mut target = source();
        target.controller = None;
        b.begin(target.clone(), 4).unwrap();
        assert!(b.discovering());
        assert!(!b.holds("codex:router"));
        assert!(!b.issued(&target.context, 4));
        let (linked, stopped) = b
            .resolve(&target.context, 4, Some("codex:router".into()))
            .unwrap();
        assert!(!stopped);
        assert_eq!(linked.controller, source().controller);
        assert!(b.holds("codex:router"));
        assert!(!b.discovering());
        b.reject(&target.context, 4, "done");
        b.begin(target.clone(), 5).unwrap();
        b.stopping("codex:target", "user stop").unwrap();
        assert!(b.discovering());
        assert!(b.holds("codex:target"));
        assert!(b
            .resolve(&target.context, 4, Some("codex:foreign".into()))
            .is_err());
        let (linked, stopped) = b
            .resolve(&target.context, 5, Some("codex:router".into()))
            .unwrap();
        assert!(stopped);
        assert_eq!(linked.controller, source().controller);
        assert!(!b.holds("codex:target"));
        assert!(!b.discovering());
        assert!(!b.issued(&target.context, 5));
    }
    #[test]
    fn scope_response_requires_exact_source_and_does_not_silently_drop_known_router() {
        use serde_json::json;
        let value = json!({"backend":"codex","threadId":"target","turnId":"running","cwd":"/project","route":{"controller":"codex:router","id":"known-job"}});
        assert_eq!(
            scope(&source(), value.clone()).unwrap(),
            Some(("codex:router".into(), "known-job".into()))
        );
        for key in ["backend", "threadId", "turnId", "cwd"] {
            let mut other = value.clone();
            other[key] = "different".into();
            assert!(scope(&source(), other).is_err());
        }
        let mut missing = value.clone();
        missing.as_object_mut().unwrap().remove("route");
        assert!(scope(&source(), missing).is_err());
        let mut null = value;
        null["route"] = serde_json::Value::Null;
        assert!(scope(&source(), null).unwrap().is_none());
        let mut b = Book::default();
        b.begin(source(), 1).unwrap();
        assert!(b.resolve(&source().context, 1, None).is_err());
        assert!(b.discovering());
    }
    #[test]
    fn terminal_status_is_visible_only_for_its_actual_current_turn_and_epoch() {
        let mut b = Book::default();
        b.begin(source(), 4).unwrap();
        resolved(&mut b, 4);
        let mut v = view(4, Stage::Complete, false);
        v.current = "last-accepted".into();
        b.apply(v);
        let e = &b.entries["codex:target"];
        let mut same = source();
        same.turn = "last-accepted".into();
        assert!(e.status_applies(&same));
        for change in ["turn", "epoch", "directory"] {
            let mut newer = same.clone();
            match change {
                "turn" => newer.turn = "new-task".into(),
                "epoch" => newer.context.generation += 1,
                _ => newer.directory = "/other".into(),
            };
            assert!(!e.status_applies(&newer));
        }
    }
}
