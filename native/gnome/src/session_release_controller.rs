use super::*;
use crate::session_release::{Phase, QUEUE_PAUSE};

impl Ui {
    pub(super) fn can_release_session(&self) -> bool {
        let state = self.state.borrow();
        let Some(thread) = &state.selected else {
            return false;
        };
        let key = format!("{}:{}", state.backend, thread.id);
        state.ready
            && state.history_verified
            && !state.loading
            && !state.catalog_loading
            && !state.session_mutation
            && !state.sending
            && state.active_turn.is_none()
            && !state.archived
            && !state.shells.contains(&key)
            && !self.queue_busy(&state)
            && !state.releases.blocked(&key)
            && state
                .backends
                .iter()
                .any(|backend| backend.id == state.backend && backend.kind == "codex")
            && !self.router_managed(&key)
            && !self
                .supervision
                .borrow()
                .entries
                .get(&key)
                .is_some_and(|entry| entry.holding())
    }
    pub(super) fn release_session(self: &Rc<Self>) {
        if !self.can_release_session() {
            return;
        }
        let (backend, thread, generation) = {
            let state = self.state.borrow();
            (
                state.backend.clone(),
                state.selected.as_ref().unwrap().id.clone(),
                state.socket_generation,
            )
        };
        let key = format!("{backend}:{thread}");
        let started = self
            .state
            .borrow_mut()
            .releases
            .begin(&backend, &thread, generation);
        let ticket = match started {
            Ok(ticket) => ticket,
            Err(error) => {
                self.error(&error);
                return;
            }
        };
        {
            let mut state = self.state.borrow_mut();
            state.history_verified = false;
            if let Some(entry) = state.queues.entries.get_mut(&key) {
                entry.pause(QUEUE_PAUSE);
                entry.connected = false;
            }
            state.queues.dirty = true;
        }
        self.update_composer();
        self.rpc(
            Pending::Release {
                thread: thread.clone(),
                ticket,
            },
            "thread/unsubscribe",
            json!({"threadId":thread}),
        );
    }
    pub(super) fn release_result(
        &self,
        backend: &str,
        generation: u64,
        thread: &str,
        ticket: u64,
        payload: &Value,
    ) {
        let confirmed = payload.get("error").is_none()
            && matches!(
                payload.pointer("/result/status").and_then(Value::as_str),
                Some("unsubscribed" | "notSubscribed" | "notLoaded")
            );
        let key = format!("{backend}:{thread}");
        {
            let mut state = self.state.borrow_mut();
            if !state.releases.finish(&key, generation, ticket, confirmed) {
                return;
            }
            state.cache.0.update(&key, |view| {
                view.valid = false;
                view.history_verified = false;
                view.loading = false;
                view.sending = false;
            });
            if state.backend == backend
                && state
                    .selected
                    .as_ref()
                    .is_some_and(|selected| selected.id == thread)
            {
                state.history_verified = false;
                state.loading = false;
            }
        }
        if !confirmed {
            let message: String = payload
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("The backend did not confirm session release; reload before sending")
                .chars()
                .take(1024)
                .collect();
            self.error(&message);
        }
        self.update_composer();
    }
    pub(super) fn render_release(&self) {
        let key = self.composer_key().unwrap_or_default();
        let phase = self.state.borrow().releases.phase(&key);
        if let Some(phase) = phase {
            let mut state = self.state.borrow_mut();
            state.history_verified = false;
            state.loading = false;
            if let Some(entry) = state.queues.entries.get_mut(&key) {
                // Rendering must not invalidate and rebuild every queued row
                // on unrelated status updates once this source is paused.
                if !entry.paused || entry.connected {
                    entry.pause(QUEUE_PAUSE);
                    entry.connected = false;
                }
            }
            drop(state);
            crate::i18n_ui::bind(
                &self.status,
                "label",
                match phase {
                    Phase::Pending => "Releasing session…",
                    Phase::Released => "Session released · Reload to send messages again",
                    Phase::Unconfirmed => "Release unconfirmed · Reload to reconnect this session",
                },
            );
        }
        if let Some(action) = self
            .window
            .lookup_action("release-session")
            .and_downcast::<gio::SimpleAction>()
        {
            action.set_enabled(self.can_release_session());
        }
    }
}
