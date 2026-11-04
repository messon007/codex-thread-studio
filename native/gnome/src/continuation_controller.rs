//! User-triggered continuation; stale results never overwrite or send a draft.
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
pub(super) struct Flight {
    ticket: u64,
    request: crate::continuation::Request,
    composer_revision: u64,
    pub(super) cancel: Arc<AtomicBool>,
}
impl Ui {
    fn continuation_request(&self) -> Option<crate::continuation::Request> {
        let behavior = self.message_flow.behavior()?;
        if !self.composer_text().trim().is_empty()
            || self.composer_tools.preedit.get()
            || self.router_selected()
            || self.router.borrow().registry.is_none()
        {
            return None;
        }
        let state = self.state.borrow();
        let thread = state.selected.as_ref()?;
        let key = format!("{}:{}", state.backend, thread.id);
        let queue = state.queues.entries.get(&key)?;
        if !state.ready
            || !state.history_verified
            || !state.options_loaded
            || state.loading
            || state.catalog_loading
            || state.session_mutation
            || state.sending
            || state.active_turn.is_some()
            || state.archived
            || state.shells.contains(&key)
            || !queue.loaded
            || queue.busy()
            || !queue.messages.is_empty()
            || queue.flight.is_some()
            || !state.images.images(&key).is_empty()
            || state.images.pending.contains_key(&key)
            || !state.references.get(&key).is_empty()
        {
            return None;
        }
        let backend = state
            .backends
            .iter()
            .find(|b| b.id == state.backend)?
            .clone();
        let model = if !state.options.model.is_empty() {
            state.options.model.clone()
        } else if !state.session_model.is_empty() {
            state.session_model.clone()
        } else {
            state.default_model.clone()
        };
        let mut options = state.options.clone();
        if options.service_tier.is_empty() {
            options.service_tier = state.session_tier.clone();
        }
        Some(crate::continuation::Request {
            context: crate::translation::Context {
                backend,
                socket: state.socket_generation,
                thread: thread.id.clone(),
                cwd: thread.cwd.clone(),
                model,
                ready: true,
            },
            options,
            response: state
                .items
                .iter()
                .rev()
                .find(|i| {
                    matches!(i.role.as_str(), "Assistant" | "Plan")
                        && (!i.body.trim().is_empty() || i.output.is_some())
                })
                .cloned(),
            behavior,
        })
    }
    fn continuation_matches(&self, flight: &Flight) -> bool {
        if self.composer_revision.get() != flight.composer_revision {
            return false;
        }
        self.continuation_request().is_some_and(|r| {
            r.context == flight.request.context
                && r.options == flight.request.options
                && r.behavior == flight.request.behavior
                && match (&r.response, &flight.request.response) {
                    (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                    (None, None) => true,
                    _ => false,
                }
        })
    }
    pub(super) fn render_continuation(&self) {
        crate::i18n_ui::bind(
            &self.continue_button,
            "tooltip-text",
            match self.message_flow.behavior().as_deref() {
                Some("quickSend") => {
                    "Immediately send a random continue prompt · Ctrl/Cmd+Shift+Enter"
                }
                Some("ollamaDraft") => {
                    "Draft the next message with local Ollama · Ctrl/Cmd+Shift+Enter"
                }
                _ => "Draft the next message with the current session model · Ctrl/Cmd+Shift+Enter",
            },
        );
        let can_start = self.continuation_request().is_some();
        let pending = self.continuation.borrow();
        if let Some(flight) = pending.as_ref() {
            if !self.continuation_matches(flight) {
                flight.cancel.store(true, Ordering::Release);
            }
            let stopping = flight.cancel.load(Ordering::Acquire);
            self.continue_button.set_visible(true);
            self.continue_button.set_sensitive(!stopping);
            crate::i18n_ui::bind(
                &self.continue_button,
                "label",
                if stopping {
                    "Stopping draft…"
                } else {
                    "Cancel draft"
                },
            );
        } else {
            self.continue_button.set_visible(!self.router_selected());
            self.continue_button.set_sensitive(can_start);
            crate::i18n_ui::bind(&self.continue_button, "label", "Continue");
        }
    }
    pub(super) fn start_continuation(&self) {
        if self.continuation.borrow().is_some() {
            return;
        }
        let Some(request) = self.continuation_request() else {
            return;
        };
        let ticket = crate::translation::next_ticket();
        let cancel = Arc::new(AtomicBool::new(false));
        let command = Command::Continue {
            ticket,
            request: request.clone(),
            cancel: cancel.clone(),
        };
        self.continuation.replace(Some(Flight {
            ticket,
            request,
            composer_revision: self.composer_revision.get(),
            cancel,
        }));
        if !self.command(command) {
            self.continuation.borrow_mut().take();
        }
        self.render_continuation();
    }
    pub(super) fn continuation_result(
        self: &Rc<Self>,
        ticket: u64,
        result: Result<crate::continuation::Outcome, String>,
    ) {
        if self
            .continuation
            .borrow()
            .as_ref()
            .is_none_or(|p| p.ticket != ticket)
        {
            return;
        }
        let flight = self.continuation.borrow_mut().take().unwrap();
        let valid = !flight.cancel.load(Ordering::Acquire) && self.continuation_matches(&flight);
        if valid {
            match result {
                Ok(result) => {
                    self.composer.buffer().set_text(&result.prompt);
                    self.remember_draft();
                    self.composer_tools.close();
                    let buffer = self.composer.buffer();
                    buffer.place_cursor(&buffer.end_iter());
                    self.composer.grab_focus();
                    if let Some(warning) = result.warning {
                        self.error(&warning);
                    }
                    if result.send {
                        self.submit();
                    } else {
                        self.notice("Continuation draft added to the composer");
                    }
                }
                Err(error) => self.error(&error),
            }
        }
        self.update_composer();
    }
    pub(super) fn connect_continuation(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.continue_button.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                let cancel = ui.continuation.borrow().as_ref().map(|p| p.cancel.clone());
                if let Some(cancel) = cancel {
                    cancel.store(true, Ordering::Release);
                    ui.render_continuation();
                } else {
                    ui.start_continuation();
                }
            }
        });
    }
}
