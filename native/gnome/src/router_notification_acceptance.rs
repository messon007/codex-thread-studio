//! Drive accepted Router replies through the real controller; the parent fixture
//! records actual GIO transport on its private bus. No provider messages are sent.
use super::*;
impl Ui {
    pub(in crate::app) fn smoke_router_notices(self: &Rc<Self>) -> Result<Value, String> {
        let registry = self
            .router
            .borrow()
            .registry
            .clone()
            .ok_or("Router registry not ready")?;
        let previous_draft = self.composer_text();
        self.composer
            .buffer()
            .set_text("Unsent Router notification draft 中文😀 ");
        let draft = self.composer_text();
        let selected = self.composer_key();
        let baseline_trees = self.router.borrow().jobs.len();
        let rows = |statuses: &[(&str, &str)]| {
            statuses
                .iter()
                .map(|(id, status)| crate::router::Tracked {
                    controller: "codex:router".into(),
                    id: (*id).into(),
                    status: Some((*status).into()),
                    target_turn_id: String::new(),
                })
                .collect::<Vec<_>>()
        };
        let apply = |statuses: Vec<crate::router::Tracked>, stale: bool| {
            let active_ids = statuses
                .iter()
                .filter(|s| {
                    matches!(
                        s.status.as_deref(),
                        Some("routing" | "dispatching" | "running")
                    )
                })
                .map(|s| s.id.clone())
                .collect();
            let history = History {
                controller: "codex:router".into(),
                ids: statuses.iter().map(|s| s.id.clone()).collect(),
                statuses,
                next: None,
                newer: None,
                unread_count: 0,
                active_ids,
                controller_busy: false,
            };
            let serial = {
                let mut book = self.router.borrow_mut();
                book.serial += 1;
                let n = book.serial;
                book.sync = Some(n);
                n
            };
            self.router_event(
                if stale { serial - 1 } else { serial },
                Ok(Reply::Sync {
                    registry: registry.clone(),
                    histories: vec![history],
                    changed: Vec::new(),
                    stamps: HashMap::new(),
                    view: None,
                    tracking: Vec::new(),
                }),
            );
            let mut book = self.router.borrow_mut();
            if stale {
                book.sync = None;
            }
            book.next_poll = Some(Instant::now() + Duration::from_secs(90));
        };
        apply(
            rows(&[
                ("historical", "completed"),
                ("router-done", "running"),
                ("router-error", "dispatching"),
                ("router-question", "routing"),
                ("router-stop", "running"),
            ]),
            false,
        );
        // Displaced active jobs must keep status-only monitoring, independently
        // of which controller or history page is selected.
        {
            let mut book = self.router.borrow_mut();
            book.histories[0].ids.clear();
        }
        if !self
            .router_queue_tracking()
            .iter()
            .any(|(c, id)| c == "codex:router" && id == "router-done")
        {
            return Err("Off-window Router job lost its lightweight watch".into());
        }
        let terminal = || {
            rows(&[
                ("historical", "completed"),
                ("router-done", "completed"),
                ("router-error", "failed"),
                ("router-question", "clarify"),
                ("router-stop", "stopped"),
            ])
        };
        apply(terminal(), false);
        apply(terminal(), false);
        apply(rows(&[("stale-only", "running")]), true);
        apply(rows(&[("stale-only", "completed")]), false);
        if self.router.borrow().jobs.len() != baseline_trees
            || self.composer_text() != draft
            || self.composer_key() != selected
        {
            return Err(
                "Router alerts changed conversation/draft or retained hidden replies".into(),
            );
        }
        self.composer.buffer().set_text(&previous_draft);
        self.router.borrow_mut().next_poll = None;
        Ok(
            json!({"liveRouterResults":true,"terminalHistorySuppressed":true,"duplicateAndStaleSuppressed":true,"stoppedSuppressed":true,"offWindowStatusTracking":true,"hiddenReplyTreesUnchanged":true,"draftRetained":true}),
        )
    }
}
