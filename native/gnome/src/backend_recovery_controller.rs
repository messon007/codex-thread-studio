//! GTK scheduling around a pure, request-free transport recovery policy.
use super::*;
use crate::backend_recovery::Plan;
#[derive(Default)]
pub(super) struct Recovery {
    book: crate::backend_recovery::Book,
    timers: HashMap<String, glib::SourceId>,
    stopped: bool,
}
impl Recovery {
    pub(super) fn forget(&mut self, backend: &str) {
        if let Some(timer) = self.timers.remove(backend) {
            timer.remove();
        }
        self.book.forget(backend);
    }
    #[cfg(debug_assertions)]
    pub(super) fn pending(&self) -> usize {
        self.timers.len()
    }
    pub(super) fn stop(&mut self) {
        self.stopped = true;
        for timer in self.timers.drain().map(|(_, timer)| timer) {
            timer.remove();
        }
        self.book.clear();
    }
}
impl Drop for Recovery {
    fn drop(&mut self) {
        self.stop();
    }
}
impl Ui {
    pub(super) fn begin_backend_connection(
        &self,
        backend: &str,
        generation: u64,
        previous: Option<u64>,
    ) {
        let mut recovery = self.recovery.borrow_mut();
        if recovery.stopped {
            return;
        }
        if let Some(timer) = recovery.timers.remove(backend) {
            timer.remove();
        }
        match previous {
            Some(old) => recovery.book.started(backend, old, generation),
            None => recovery.book.connecting(backend, generation),
        }
    }
    pub(super) fn backend_disconnected(&self, backend: String, generation: u64, error: String) {
        if self.state.borrow().backend_generations.get(&backend) != Some(&generation) {
            return;
        }
        self.expire_inputs(Some(&backend), Some(generation), None);
        self.annotations_disconnected(Some(&backend));
        self.mcp_disconnected(&backend, generation);
        self.agents_disconnected(&backend, generation);
        self.state.borrow_mut().backend_ready.remove(&backend);
        self.state.borrow_mut().releases.lost(Some(&backend));
        self.state.borrow_mut().shells.disconnected(Some(&backend));
        self.state.borrow_mut().images.disconnected(Some(&backend));
        self.state.borrow_mut().queues.pause_backend(
            &backend,
            "Disconnected. Check the conversation before resuming.",
        );
        self.state.borrow_mut().cache.invalidate(&backend);
        {
            let mut state = self.state.borrow_mut();
            state.pending.retain(|_, request| {
                request.backend != backend || request.generation != generation
            });
            for (key, activity) in &mut state.activity {
                if key.starts_with(&format!("{backend}:")) {
                    activity.stale = true;
                    if activity.active {
                        activity.attention = true;
                    }
                }
            }
        }
        if self.state.borrow().backend == backend
            && self.state.borrow().socket_generation == generation
        {
            self.state.borrow_mut().session_mutation = false;
            self.state.borrow_mut().ready = false;
            self.state.borrow_mut().sending = false;
            self.error(&error);
            self.update_composer();
        } else {
            self.toast_error(&format!("{backend}: {error}"));
        }
    }
    pub(super) fn backend_error(&self, backend: String, generation: u64, error: String) {
        if self.state.borrow().backend_generations.get(&backend) != Some(&generation) {
            return;
        }
        if self.state.borrow().backend == backend {
            self.error(&error);
        } else {
            self.toast_error(&format!("{backend}: {error}"));
        }
    }
    pub(super) fn backend_recovered(&self, backend: &str, generation: u64) {
        let mut recovery = self.recovery.borrow_mut();
        if recovery.stopped {
            return;
        }
        if let Some(timer) = recovery.timers.remove(backend) {
            timer.remove();
        }
        recovery
            .book
            .ready(backend, generation, std::time::Instant::now());
    }
    pub(super) fn schedule_backend_recovery(self: &Rc<Self>, backend: &str, generation: u64) {
        if self.recovery.borrow().stopped
            || self.state.borrow().backend_generations.get(backend) != Some(&generation)
        {
            return;
        }
        let plan =
            self.recovery
                .borrow_mut()
                .book
                .failed(backend, generation, std::time::Instant::now());
        match plan {
            Plan::Retry { delay, attempt } => {
                if self.state.borrow().backend == backend {
                    crate::i18n_ui::bind_format(
                        &self.status,
                        "label",
                        "Disconnected · Reconnecting in {seconds}s ({attempt}/8)",
                        &[
                            ("seconds", &format!("{:.1}", delay.as_secs_f64())),
                            ("attempt", &attempt.to_string()),
                        ],
                    );
                }
                let weak = Rc::downgrade(self);
                let id = backend.to_owned();
                let timer = glib::timeout_add_local_once(delay, move || {
                    if let Some(ui) = weak.upgrade() {
                        ui.retry_backend(&id, generation);
                    }
                });
                self.recovery
                    .borrow_mut()
                    .timers
                    .insert(backend.into(), timer);
            }
            Plan::Exhausted if self.state.borrow().backend == backend => {
                self.notice("Automatic reconnection stopped. Use Reconnect to try again.")
            }
            _ => {}
        }
    }
    fn retry_backend(self: &Rc<Self>, backend: &str, generation: u64) {
        {
            let mut recovery = self.recovery.borrow_mut();
            recovery.timers.remove(backend);
            if recovery.stopped || !recovery.book.take(backend, generation) {
                return;
            }
        }
        let target = {
            let state = self.state.borrow();
            if state.backend_generations.get(backend) != Some(&generation)
                || state.backend_ready.contains(backend)
            {
                return;
            }
            state.backends.iter().find(|b| b.id == backend).cloned()
        };
        let Some(target) = target else {
            return;
        };
        let selected = self.state.borrow().backend == backend;
        let next = if selected {
            self.connect_backend_attempt(target, true, None, Some(generation));
            self.state.borrow().socket_generation
        } else {
            // Keep the selected backend, transcript, draft and workspace intact.
            let next = {
                let mut state = self.state.borrow_mut();
                state.connection_sequence += 1;
                let next = state.connection_sequence;
                state.backend_generations.insert(backend.into(), next);
                next
            };
            self.begin_backend_connection(backend, next, Some(generation));
            self.command(Command::Connect {
                backend: target,
                generation: next,
            });
            next
        };
        if next == generation {
            self.schedule_backend_recovery(backend, generation);
        }
    }
}
