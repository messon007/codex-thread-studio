use super::*;
use crate::{
    supervision_state::{self as display, Source},
    turn_supervision::Stage,
    turn_supervision_host::{Request, Update},
    turn_supervision_runtime::Start,
};
pub(super) enum Purpose {
    Start,
    Prepare { id: u64, text: String },
}
pub(super) struct OptionsRead {
    source: Source,
    request: u64,
    purpose: Purpose,
}
impl Ui {
    fn supervision_serial(&self) -> u64 {
        let mut s = self.state.borrow_mut();
        s.sequence += 1;
        s.sequence
    }
    pub(super) fn connect_supervision(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        *self.supervision_menu.on_action.borrow_mut() = Some(Box::new(move |source, stop| {
            if let Some(ui) = weak.upgrade() {
                if stop {
                    ui.stop_supervision(&source.key(), "Supervision stopped by user");
                } else if let Err(error) = ui.start_supervision(source) {
                    ui.error(&error);
                }
            }
        }));
    }
    pub(super) fn supervision_holds(&self, key: &str) -> bool {
        let book = self.supervision.borrow();
        book.holds(key) || book.discovering() && self.router_managed(key)
    }
    fn supervision_sources(&self) -> Vec<Source> {
        if self.router.borrow().registry.is_none() {
            return Vec::new();
        }
        let state = self.state.borrow();
        let Some(thread) = &state.selected else {
            return Vec::new();
        };
        let key = format!("{}:{}", state.backend, thread.id);
        let mut sources = Vec::new();
        if self.router_managed(&key) {
            for display in self.router.borrow().jobs.values().filter(|d| {
                d.job.controller == key
                    && !d.job.browser_history
                    && d.job.status == "running"
                    && !d.job.target_turn_id.is_empty()
            }) {
                if let Some(target) = &display.job.target {
                    if state
                        .backends
                        .iter()
                        .any(|b| b.id == target.backend && b.kind == "codex")
                        && state.backend_ready.contains(&target.backend)
                    {
                        if let Some(generation) = state.backend_generations.get(&target.backend) {
                            sources.push(Source {
                                context: crate::session_search::Context {
                                    backend: target.backend.clone(),
                                    thread: target.id.clone(),
                                    generation: *generation,
                                },
                                directory: target.cwd.clone(),
                                title: target.title.clone(),
                                turn: display.job.target_turn_id.clone(),
                                controller: Some(key.clone()),
                            });
                        }
                    }
                }
            }
        } else if state.ready
            && state.history_verified
            && !state.loading
            && !state.archived
            && !state.session_mutation
            && state.options_loaded
            && state
                .backends
                .iter()
                .any(|b| b.id == state.backend && b.kind == "codex")
        {
            if let Some(turn) = &state.active_turn {
                sources.push(Source {
                    context: crate::session_search::Context {
                        backend: state.backend.clone(),
                        thread: thread.id.clone(),
                        generation: state.socket_generation,
                    },
                    directory: thread.cwd.clone(),
                    title: thread.title.clone(),
                    turn: turn.clone(),
                    controller: None,
                });
            }
        }
        sources
    }
    pub(super) fn render_supervision(&self) {
        let key = self.composer_key().unwrap_or_default();
        let book = self.supervision.borrow();
        let active = book.entries.values().any(display::Entry::holding);
        // This global cue survives navigation away from the supervised source.
        self.supervision_cue.set_visible(active);
        if active {
            self.supervision_menu
                .button
                .add_css_class("suggested-action");
        } else {
            self.supervision_menu
                .button
                .remove_css_class("suggested-action");
        }
        let current = book.entries.iter().find(|(k, e)| {
            e.holding()
                && (k.as_str() == key || e.source.controller.as_deref() == Some(key.as_str()))
        });
        self.supervision_status.set_visible(current.is_some());
        if let Some((_, entry)) = current {
            let stage = if entry.stopping {
                "Supervision stopped"
            } else {
                match entry.view.as_ref().map(|v| v.stage) {
                    None => "Loading…",
                    Some(Stage::Waiting) => "Waiting",
                    Some(Stage::Evaluating) => "Evaluating",
                    Some(Stage::Preparing) => "Preparing",
                    Some(Stage::Sending) => "Sending…",
                    Some(Stage::Complete) => "Supervision complete",
                    Some(Stage::Stopped) => "Supervision stopped",
                }
            };
            crate::i18n_ui::bind_parts(
                &self.supervision_status,
                "label",
                vec![
                    crate::i18n_ui::Part::message("Turn supervision", &[]),
                    crate::i18n_ui::Part::message(stage, &[]),
                    crate::i18n_ui::Part::literal(format!(
                        "{}/5",
                        entry.view.as_ref().map_or(0, |v| v.count)
                    )),
                ],
                " · ",
            );
            self.supervision_status
                .set_tooltip_text(Some(&entry.source.title));
        }
        drop(book);
        let mut rows = std::collections::BTreeMap::new();
        for source in self.supervision_sources() {
            rows.insert(
                source.key(),
                crate::supervision_ui::Row {
                    source,
                    status: String::new(),
                    action: "start",
                },
            );
        }
        for (key, e) in &self.supervision.borrow().entries {
            if e.holding() || rows.contains_key(key) {
                if !e.holding() && !e.status_applies(&rows.get(key).unwrap().source) {
                    continue;
                }
                let caption = if e.stopping {
                    "Supervision stopped"
                } else {
                    match e.view.as_ref().map(|v| v.stage) {
                        None => "Loading…",
                        Some(Stage::Waiting) => "Waiting",
                        Some(Stage::Evaluating) => "Evaluating",
                        Some(Stage::Preparing) => "Preparing",
                        Some(Stage::Sending) => "Sending…",
                        Some(Stage::Complete) => "Supervision complete",
                        Some(Stage::Stopped) => "Supervision stopped",
                    }
                };
                let mut status = crate::language::tr(caption);
                if let Some(view) = &e.view {
                    status.push_str(&format!(" · {}/5", view.count));
                    if !view.warning.is_empty() {
                        status.push_str(&format!("\n{}", view.warning));
                    }
                }
                if !e.reason.is_empty() {
                    status.push_str(&format!("\n{}", e.reason));
                }
                let action = if e.holding() {
                    if e.stopping {
                        "settling"
                    } else {
                        "stop"
                    }
                } else {
                    "start"
                };
                rows.insert(
                    key.clone(),
                    crate::supervision_ui::Row {
                        source: if e.holding() {
                            e.source.clone()
                        } else {
                            rows.get(key).unwrap().source.clone()
                        },
                        status,
                        action,
                    },
                );
            }
        }
        self.supervision_menu.render(rows.into_values().collect());
    }
    fn supervision_send(&self, source: &Source, request: Request) -> bool {
        let state = self.state.borrow();
        if state.backend_generations.get(&source.context.backend)
            != Some(&source.context.generation)
            || !state.backend_ready.contains(&source.context.backend)
        {
            drop(state);
            self.error("Backend changed or disconnected");
            return false;
        }
        drop(state);
        self.command(Command::Supervision {
            backend: source.context.backend.clone(),
            generation: source.context.generation,
            request,
        })
    }
    fn supervision_read_options(&self, source: Source, request: u64, purpose: Purpose) -> bool {
        let serial = self.supervision_serial();
        let key = source.key();
        if self.supervision_options.borrow().len() >= 32 {
            return false;
        }
        self.supervision_options.borrow_mut().insert(
            serial,
            OptionsRead {
                source,
                request,
                purpose,
            },
        );
        if !self.command(Command::SupervisionOptions { serial, key }) {
            self.supervision_options.borrow_mut().remove(&serial);
            return false;
        }
        true
    }
    fn start_supervision(&self, source: Source) -> Result<(), String> {
        if !self.supervision_sources().iter().any(|s| {
            s.context == source.context
                && s.directory == source.directory
                && s.turn == source.turn
                && s.controller == source.controller
        }) {
            return Err("No running turn to supervise".into());
        }
        {
            let state = self.state.borrow();
            let busy = |key: &str| {
                state.queues.entries.get(key).is_some_and(|e| {
                    e.preparing.is_some()
                        || e.flight.as_ref().is_some_and(|f| !f.acknowledged)
                        || (!e.router && e.flight.is_some())
                })
            };
            if busy(&source.key()) || source.controller.as_deref().is_some_and(busy) {
                return Err("Wait for the current queued message before supervising".into());
            }
        }
        let request = self.supervision_serial();
        self.supervision
            .borrow_mut()
            .begin(source.clone(), request)?;
        self.state.borrow_mut().queues.dirty = true;
        if !self.command(Command::Http{tag:format!("native-supervision-scope:{request}"),method:reqwest::Method::POST,path:"/studio/native/router/supervision-scope".into(),body:Some(json!({"backend":source.context.backend,"threadId":source.context.thread,"turnId":source.turn,"cwd":source.directory})),document:None}) {
            self.supervision.borrow_mut().reject(
                &source.context,
                request,
                "Cannot read supervision ownership",
            );
            return Err("Cannot read supervision ownership".into());
        }
        self.update_composer();
        Ok(())
    }
    fn pause_supervision_queues(&self, source: &Source, reason: &str) {
        let mut state = self.state.borrow_mut();
        for key in std::iter::once(source.key()).chain(source.controller.clone()) {
            if let Some(entry) = state.queues.entries.get_mut(&key) {
                entry.pause(reason);
            }
        }
        state.queues.dirty = true;
    }
    pub(super) fn supervision_scope_result(&self, request: u64, result: Result<Value, String>) {
        let pending = self
            .supervision
            .borrow()
            .entries
            .values()
            .find(|e| e.request == request && e.discovering())
            .map(|e| e.source.clone());
        let Some(source) = pending else {
            return;
        };
        let result = result
            .and_then(|value| display::scope(&source, value))
            .and_then(|route| {
                if self
                    .state
                    .borrow()
                    .backend_generations
                    .get(&source.context.backend)
                    != Some(&source.context.generation)
                    || !self
                        .state
                        .borrow()
                        .backend_ready
                        .contains(&source.context.backend)
                {
                    return Err("Backend changed or disconnected".into());
                }
                let resolved = self.supervision.borrow_mut().resolve(
                    &source.context,
                    request,
                    route.as_ref().map(|(controller, _)| controller.clone()),
                )?;
                if let Some((controller, id)) = &route.filter(|_| !resolved.1) {
                    if self
                        .state
                        .borrow()
                        .queues
                        .entries
                        .get(controller)
                        .is_some_and(|e| {
                            e.preparing.is_some()
                                || e.flight
                                    .as_ref()
                                    .is_some_and(|f| !f.acknowledged || f.id != *id)
                                || e.router_wait.as_ref().is_some_and(|pending| pending != id)
                        })
                    {
                        return Err("Another queued Router task is already being submitted".into());
                    }
                }
                Ok(resolved)
            });
        match result {
            Ok((resolved, true)) => {
                self.pause_supervision_queues(&resolved, "Supervision stopped by user")
            }
            Ok((resolved, false)) => {
                if !self.supervision_read_options(resolved.clone(), request, Purpose::Start) {
                    self.supervision.borrow_mut().reject(
                        &resolved.context,
                        request,
                        "Cannot read source settings",
                    );
                    self.pause_supervision_queues(&resolved, "Cannot read source settings");
                }
            }
            Err(error) => {
                let source = self
                    .supervision
                    .borrow()
                    .current(&source.context, request)
                    .map(|e| e.source.clone())
                    .unwrap_or(source);
                self.supervision
                    .borrow_mut()
                    .reject(&source.context, request, &error);
                self.pause_supervision_queues(&source, &error);
                self.error(&error);
            }
        }
        self.state.borrow_mut().queues.dirty = true;
        self.update_composer();
    }
    pub(super) fn stop_supervision(&self, key: &str, reason: &str) {
        let pending = {
            let book = self.supervision.borrow();
            book.entries
                .get(key)
                .filter(|e| e.holding())
                .map(|e| (e.source.clone(), e.issued))
        };
        let Some((source, issued)) = pending else {
            return;
        };
        if issued
            && !self.supervision_send(
                &source,
                Request::Stop {
                    thread: source.context.thread.clone(),
                },
            )
        {
            return;
        }
        self.supervision.borrow_mut().stopping(key, reason);
        self.supervision_options
            .borrow_mut()
            .retain(|_, r| r.source.key() != key);
        self.pause_supervision_queues(&source, reason);
        self.update_composer();
    }
    pub(super) fn supervision_human(&self, key: &str) {
        let keys = self
            .supervision
            .borrow()
            .entries
            .iter()
            .filter(|(k, e)| {
                e.holding() && (k.as_str() == key || e.source.controller.as_deref() == Some(key))
            })
            .map(|(k, _)| k.clone())
            .collect::<Vec<_>>();
        for key in keys {
            self.stop_supervision(&key, "Stopped by user activity");
        }
    }
    pub(super) fn supervision_backend_lost(&self, backend: &str) {
        let sources = self.supervision.borrow_mut().forget_backend(backend);
        self.supervision_options
            .borrow_mut()
            .retain(|_, r| r.source.context.backend != backend);
        for source in sources {
            self.pause_supervision_queues(&source, "Backend changed or disconnected");
        }
    }
    pub(super) fn supervision_invalidate(&self, key: &str) {
        let source = self
            .supervision
            .borrow()
            .entries
            .get(key)
            .filter(|e| e.holding() && e.issued)
            .map(|e| e.source.clone());
        if let Some(source) = source {
            self.supervision_send(
                &source,
                Request::Invalidate {
                    thread: source.context.thread.clone(),
                },
            );
        }
    }
    pub(super) fn supervision_options_changed(
        &self,
        key: &str,
        options: &crate::turn_options::Options,
    ) {
        self.supervision_invalidate(key);
        let source = self
            .supervision
            .borrow()
            .entries
            .get(key)
            .filter(|e| e.active() && e.issued)
            .map(|e| e.source.clone());
        if let Some(source) = source {
            let model = self.supervision_model(&source, options);
            self.supervision_send(
                &source,
                Request::Options {
                    thread: source.context.thread.clone(),
                    model,
                    effort: options.effort.clone(),
                },
            );
        }
    }
    fn supervision_model(&self, source: &Source, options: &crate::turn_options::Options) -> String {
        if !options.model.is_empty() {
            return options.model.clone();
        }
        let state = self.state.borrow();
        if state.backend == source.context.backend
            && state
                .selected
                .as_ref()
                .is_some_and(|t| t.id == source.context.thread)
        {
            return if state.session_model.is_empty() {
                state.default_model.clone()
            } else {
                state.session_model.clone()
            };
        }
        state
            .cache
            .0
            .peek(&source.key())
            .map(|v| v.session_model.clone())
            .unwrap_or_default()
    }
    pub(super) fn supervision_options_result(&self, serial: u64, result: Result<Value, String>) {
        let Some(read) = self.supervision_options.borrow_mut().remove(&serial) else {
            return;
        };
        let valid = self
            .supervision
            .borrow()
            .current(&read.source.context, read.request)
            .is_some_and(display::Entry::active)
            && self
                .state
                .borrow()
                .backend_generations
                .get(&read.source.context.backend)
                == Some(&read.source.context.generation)
            && self
                .state
                .borrow()
                .backend_ready
                .contains(&read.source.context.backend);
        if !valid {
            return;
        }
        let result = result.and_then(|value| {
            serde_json::from_value::<crate::turn_options::Options>(value)
                .map_err(|_| "Invalid supervised source options".into())
        });
        let mut options = match result {
            Ok(o) => o,
            Err(error) => {
                match read.purpose {
                    Purpose::Prepare { id, .. } => {
                        self.supervision_send(
                            &read.source,
                            Request::Rejected {
                                id,
                                reason: error.clone(),
                            },
                        );
                    }
                    Purpose::Start => {
                        self.supervision.borrow_mut().reject(
                            &read.source.context,
                            read.request,
                            &error,
                        );
                        self.pause_supervision_queues(&read.source, &error);
                    }
                }
                self.error(&error);
                self.update_composer();
                return;
            }
        };
        {
            let state = self.state.borrow();
            if state.backend == read.source.context.backend
                && state
                    .selected
                    .as_ref()
                    .is_some_and(|t| t.id == read.source.context.thread)
                && state.options_loaded
            {
                options = state.options.clone();
            }
        }
        let model = self.supervision_model(&read.source, &options);
        let request = match read.purpose {
            Purpose::Start => {
                let backend = self
                    .state
                    .borrow()
                    .backends
                    .iter()
                    .find(|b| b.id == read.source.context.backend && b.kind == "codex")
                    .cloned();
                let Some(backend) = backend else {
                    return;
                };
                Request::Start(Start {
                    request: read.request,
                    context: crate::translation::Context {
                        backend,
                        socket: read.source.context.generation,
                        thread: read.source.context.thread.clone(),
                        cwd: read.source.directory.clone(),
                        model,
                        ready: true,
                    },
                    turn: read.source.turn.clone(),
                    effort: options.effort,
                })
            }
            Purpose::Prepare { id, text } => {
                if !self.supervision.borrow().may_prepare(
                    &read.source.context,
                    read.request,
                    &read.source.directory,
                ) {
                    return;
                }
                let mut params = json!({"threadId":read.source.context.thread,"input":[{"type":"text","text":text}]});
                options.apply(&mut params, true);
                let policy = self.state.borrow().policies.apply(
                    &read.source.key(),
                    &read.source.directory,
                    true,
                    &mut params,
                );
                if let Err(error) = policy {
                    self.supervision_send(&read.source, Request::Rejected { id, reason: error });
                    return;
                }
                self.supervision_send(
                    &read.source,
                    Request::Options {
                        thread: read.source.context.thread.clone(),
                        model,
                        effort: options.effort,
                    },
                );
                Request::Prepared { id, params }
            }
        };
        let starting = matches!(request, Request::Start(_));
        if self.supervision_send(&read.source, request) {
            if starting {
                self.supervision
                    .borrow_mut()
                    .issued(&read.source.context, read.request);
            }
        } else {
            self.supervision.borrow_mut().reject(
                &read.source.context,
                read.request,
                "Backend request was not sent",
            );
            self.pause_supervision_queues(&read.source, "Backend request was not sent");
        }
        self.update_composer();
    }
    pub(super) fn supervision_update(&self, backend: String, generation: u64, update: Update) {
        match update {
            Update::Changed(view) => {
                let source = self
                    .supervision
                    .borrow()
                    .current(&view.context, view.request)
                    .map(|e| e.source.clone());
                let Some(source) = source else {
                    return;
                };
                let stage = view.stage;
                let holding = view.holding;
                let reason = view.reason.clone();
                let old = self
                    .supervision
                    .borrow()
                    .entries
                    .get(&source.key())
                    .and_then(|e| e.view.as_ref().map(|v| v.stage));
                if !self.supervision.borrow_mut().apply(view) {
                    return;
                }
                if stage == Stage::Stopped {
                    self.pause_supervision_queues(&source, &reason);
                }
                if stage == Stage::Complete && !holding {
                    let mut state = self.state.borrow_mut();
                    if let Some(e) = state
                        .queues
                        .entries
                        .get_mut(&source.key())
                        .filter(|e| e.flight.is_none())
                    {
                        e.active = false;
                        e.released = true;
                        e.touch();
                    }
                    state.queues.dirty = true;
                }
                if matches!(stage, Stage::Complete | Stage::Stopped) && old != Some(stage) {
                    self.toast.add_toast(adw::Toast::new(&format!(
                        "{} · {}: {}",
                        source.title,
                        crate::language::tr(if stage == Stage::Complete {
                            "Supervision complete"
                        } else {
                            "Supervision stopped"
                        }),
                        reason
                    )));
                }
                self.state.borrow_mut().queues.dirty = true;
            }
            Update::Prepare {
                id,
                request,
                context,
                root,
                text,
            } => {
                let source = self
                    .supervision
                    .borrow()
                    .current(&context, request)
                    .filter(|_| {
                        self.supervision
                            .borrow()
                            .may_prepare(&context, request, &root)
                    })
                    .map(|e| e.source.clone());
                if let Some(source) = source {
                    if self.supervision_read_options(source, request, Purpose::Prepare { id, text })
                    {
                        self.update_composer();
                        return;
                    }
                }
                self.command(Command::Supervision {
                    backend,
                    generation,
                    request: Request::Rejected {
                        id,
                        reason: "Supervision source changed before preparation".into(),
                    },
                });
            }
            Update::Rejected {
                request,
                thread,
                message,
            } => {
                if let (Some(request), Some(thread)) = (request, thread) {
                    let context = crate::session_search::Context {
                        backend,
                        generation,
                        thread,
                    };
                    let source = self
                        .supervision
                        .borrow()
                        .current(&context, request)
                        .map(|e| e.source.clone());
                    if let Some(source) = source {
                        self.supervision
                            .borrow_mut()
                            .reject(&context, request, &message);
                        self.pause_supervision_queues(&source, &message);
                    }
                }
                self.error(&message);
            }
            Update::Follow {
                receipt, routed, ..
            } => {
                if let Err(error) = routed {
                    self.error(&error);
                }
                let key = format!("{}:{}", receipt.context.backend, receipt.context.thread);
                let mut state = self.state.borrow_mut();
                let finished = state
                    .activity
                    .get(&key)
                    .is_some_and(|a| a.finished(&receipt.turn));
                if !finished {
                    state.activity.entry(key.clone()).or_default().apply(
                        "turn/started",
                        &json!({"turn":{"id":receipt.turn}}),
                        false,
                    );
                }
                state.cache.0.update(&key, |v| {
                    v.active_turn = (!finished).then(|| receipt.turn.clone());
                    v.valid = false;
                });
                if state.backend == receipt.context.backend
                    && state
                        .selected
                        .as_ref()
                        .is_some_and(|t| t.id == receipt.context.thread)
                {
                    state.active_turn = (!finished).then(|| receipt.turn.clone());
                }
                if let Some(e) = state.queues.entries.get_mut(&key) {
                    e.active = !finished;
                    e.released = finished;
                    e.touch();
                }
                state.queues.dirty = true;
                drop(state);
                self.router.borrow_mut().next_poll = Some(std::time::Instant::now());
            }
        }
        self.update_composer();
    }
}
