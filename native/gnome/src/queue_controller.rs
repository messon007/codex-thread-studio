use super::*;
use crate::message_queue::{self as q, Context, Entry, Flight, Reply, Request};
impl Ui {
    pub(super) fn history_ready(state: &State, key: &str) -> bool {
        if state.releases.blocked(key) {
            return false;
        }
        if state.pending.values().any(|request| matches!(&request.action,Pending::Select(thread,_) if format!("{}:{thread}",request.backend)==key)) {return false;}
        if state
            .selected
            .as_ref()
            .is_some_and(|thread| format!("{}:{}", state.backend, thread.id) == key)
        {
            return state.history_verified && !state.loading;
        }
        state
            .cache
            .0
            .peek(key)
            .is_none_or(|view| view.history_verified && !view.loading)
    }
    pub(super) fn complete_manual_reply(&self, backend: &str, thread: &str, payload: &Value) {
        let mut status = payload
            .pointer("/result/turn/status")
            .and_then(Value::as_str)
            .unwrap_or("");
        let key = format!("{backend}:{thread}");
        if self.router_managed(&key) {
            return;
        }
        let mut state = self.state.borrow_mut();
        if !matches!(status, "completed" | "failed" | "interrupted") {
            let turn = payload
                .pointer("/result/turn/id")
                .and_then(Value::as_str)
                .unwrap_or("");
            if !turn.is_empty()
                && state
                    .activity
                    .get(&key)
                    .is_some_and(|activity| activity.finished(turn))
            {
                status = "completed";
            } else {
                return;
            }
        }
        if state
            .queues
            .entries
            .get(&key)
            .is_some_and(|entry| entry.flight.is_some())
        {
            return;
        }
        let params = json!({"threadId":thread,"turn":{"id":payload.pointer("/result/turn/id").and_then(Value::as_str).unwrap_or(""),"status":status}});
        if let Some(entry) = state.queues.entries.get_mut(&key) {
            entry.observe("turn/completed", &params);
            // A first follow-up may have been staged after completion but
            // before its late acknowledgement. Preserve pauses while clearing
            // that provisional active flag from the authoritative completion.
            entry.active = false;
            entry.released = status == "completed";
        }
        let selected = state.backend == backend
            && state
                .selected
                .as_ref()
                .is_some_and(|selected| selected.id == thread);
        state
            .activity
            .entry(key.clone())
            .or_default()
            .apply("turn/completed", &params, selected);
        state.cache.0.update(&key, |view| view.active_turn = None);
        if selected {
            state.active_turn = None;
        }
        state.queues.dirty = true;
    }
    pub(super) fn queue_gateway_failed(&self, error: &str) {
        let pending = {
            let mut state = self.state.borrow_mut();
            state.backend_ready.clear();
            let mut pending = Vec::new();
            for (key, entry) in &mut state.queues.entries {
                entry.connected = false;
                entry.pause(error);
                if let Some(saving) = &entry.saving {
                    pending.push((key.clone(), saving.serial));
                }
                entry.preparing = None;
                if !entry.router {
                    entry.flight = None;
                }
            }
            pending
        };
        for (key, serial) in pending {
            self.queue_result(key, serial, Err(error.into()));
        }
    }
    pub(super) fn queue_send_failed(&self, backend: &str, thread: &str) {
        let mut state = self.state.borrow_mut();
        if let Some(entry) = state.queues.entries.get_mut(&format!("{backend}:{thread}")) {
            entry.pause("Send failed. Check the conversation before resuming queued messages.");
        }
    }
    pub(super) fn queue_busy(&self, state: &State) -> bool {
        state
            .selected
            .as_ref()
            .and_then(|thread| {
                state
                    .queues
                    .entries
                    .get(&format!("{}:{}", state.backend, thread.id))
            })
            .is_some_and(Entry::busy)
    }
    pub(super) fn connect_queue(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.queue_button.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                if let Err(error) = ui.enqueue() {
                    ui.error(&error);
                }
            }
        });
        let weak = Rc::downgrade(self);
        *self.queue.on_toggle.borrow_mut() = Some(Box::new(move |key| {
            if let Some(ui) = weak.upgrade() {
                ui.toggle_queue(&key);
            }
        }));
        let weak = Rc::downgrade(self);
        *self.queue.on_remove.borrow_mut() = Some(Box::new(move |key, id| {
            if let Some(ui) = weak.upgrade() {
                if let Err(error) = ui.change_queue(&key, &id, None) {
                    ui.error(&error);
                }
            }
        }));
        let weak = Rc::downgrade(self);
        *self.queue.on_edit.borrow_mut() = Some(Box::new(move |key, id| {
            if let Some(ui) = weak.upgrade() {
                if ui.queue.editor.borrow().is_some() {
                    return;
                }
                let message = {
                    let mut state = ui.state.borrow_mut();
                    state.queues.entries.get_mut(&key).and_then(|entry| {
                        if entry.busy() {
                            return None;
                        }
                        let value = entry
                            .messages
                            .iter()
                            .find(|message| message.id == id)
                            .cloned();
                        entry.editing = value.is_some();
                        entry.touch();
                        value
                    })
                };
                if let Some(message) = message {
                    ui.queue.edit(&ui.window, &key, &message);
                }
                ui.update_composer();
            }
        }));
        let weak = Rc::downgrade(self);
        *self.queue.on_save.borrow_mut() = Some(Box::new(move |key, id, text| {
            weak.upgrade()
                .ok_or_else(|| "Studio closed".to_owned())?
                .change_queue(&key, &id, Some(text))
        }));
        let weak = Rc::downgrade(self);
        *self.queue.on_close.borrow_mut() = Some(Box::new(move |key| {
            if let Some(ui) = weak.upgrade() {
                let mut state = ui.state.borrow_mut();
                if let Some(entry) = state.queues.entries.get_mut(&key) {
                    entry.editing = false;
                    entry.touch();
                }
                state.queues.dirty = true;
                drop(state);
                ui.update_composer();
            }
        }));
    }
    fn selected_queue(&self) -> Option<Context> {
        let state = self.state.borrow();
        let thread = state.selected.as_ref()?;
        let kind = state
            .backends
            .iter()
            .find(|backend| backend.id == state.backend)?
            .kind
            .clone();
        Some(Context {
            backend: state.backend.clone(),
            thread: thread.id.clone(),
            root: thread.cwd.clone(),
            kind,
            generation: state.socket_generation,
        })
    }
    pub(super) fn load_queue(&self, force: bool) {
        let Some(context) = self.selected_queue() else {
            return;
        };
        let key = context.key();
        let router = self.router_managed(&key);
        let serial = {
            let mut state = self.state.borrow_mut();
            if !state.queues.entries.contains_key(&key) && state.queues.entries.len() >= 128 {
                let candidate = state
                    .queues
                    .entries
                    .iter()
                    .filter(|(_, entry)| {
                        (entry.paused || entry.messages.is_empty())
                            && !entry.busy()
                            && entry.flight.is_none()
                            && !entry.editing
                    })
                    .min_by_key(|(_, entry)| entry.load_serial)
                    .map(|(key, _)| key.clone());
                if let Some(candidate) = candidate {
                    state.queues.entries.remove(&candidate);
                } else {
                    return;
                }
            }
            let serial = state.queues.serial();
            let ready = state.ready;
            let active = state.active_turn.is_some();
            let entry = state
                .queues
                .entries
                .entry(key.clone())
                .or_insert_with(|| Entry::new(context.clone(), 0));
            entry.router = router
                || entry.router
                    && (!entry.messages.is_empty()
                        || entry.flight.is_some()
                        || entry.router_wait.is_some());
            if entry.context.generation != context.generation {
                entry.pause("Connection changed. Check the conversation before resuming.");
                if !router {
                    entry.flight = None;
                }
                entry.preparing = None;
            }
            entry.context = context;
            entry.connected = ready;
            if entry.flight.is_none() && !router {
                entry.active = active;
            }
            entry.touch();
            if entry.saving.is_some() || entry.loaded && !force || entry.load_serial != 0 && !force
            {
                return;
            }
            entry.load_serial = serial;
            entry.flow_epoch = self.message_flow.epoch();
            serial
        };
        self.queue_request(&key, serial, Request::Read);
    }
    fn queue_request(&self, key: &str, serial: u64, request: Request) -> bool {
        if self.command(Command::Queue {
            key: key.into(),
            serial,
            request,
        }) {
            true
        } else {
            self.queue_result(
                key.into(),
                serial,
                Err("Queue request was not started".into()),
            );
            false
        }
    }
    pub(super) fn enqueue(&self) -> Result<(), String> {
        let key = self.composer_key().ok_or("Choose a session")?;
        let router = self.router_managed(&key);
        let router_gate = router.then(|| self.router_queue_gate(&key)).flatten();
        if !self.state.borrow().images.images(&key).is_empty() {
            return Err("Image messages cannot be queued. Send or steer them directly.".into());
        }
        if self.state.borrow().images.pending.contains_key(&key) {
            return Err("Wait for the images to load before queuing".into());
        }
        let references = self.state.borrow().references.get(&key);
        let original = self.composer_text();
        if router && original.len() > 64 * 1024 {
            return Err("Router queued messages support text up to 64 KiB".into());
        }
        if router && self.router_files_preparing() {
            return Err("Wait for reference files to finish preparing before queuing".into());
        }
        if router
            && self
                .router
                .borrow()
                .registry
                .as_ref()
                .and_then(|registry| registry.active())
                .as_deref()
                != Some(key.as_str())
        {
            return Err("Select the active Router before queuing".into());
        }
        if crate::command_tools::shell(&original).is_some() {
            return Err("Shell commands cannot be queued".into());
        }
        let text = original.trim().to_owned();
        let mut message = q::Message {
            id: glib::uuid_string_random().into(),
            text,
            input: Arc::new(
                references
                    .iter()
                    .map(|reference| reference.input.clone())
                    .collect(),
            ),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64,
            bytes: 0,
        };
        message.validate()?;
        message.bytes = message.text.len()
            + references
                .iter()
                .map(|reference| 2 * reference.bytes)
                .sum::<usize>()
            + 256;
        let context = self.selected_queue().ok_or("Choose a session")?;
        let key = context.key();
        let (serial, request) = {
            let mut state = self.state.borrow_mut();
            if !state.ready
                || state.loading
                || !state.history_verified
                || state.archived
                || state.session_mutation
            {
                return Err("Wait for the session to become ready".into());
            }
            let bytes = state
                .queues
                .entries
                .values()
                .flat_map(|entry| entry.messages.iter())
                .map(|message| message.bytes)
                .sum::<usize>();
            if bytes + message.bytes > 32 * 1024 * 1024 {
                return Err(
                    "The native queue cache is full; finish or remove pending messages first"
                        .into(),
                );
            }
            let active = if router {
                router_gate.is_some()
            } else {
                state.active_turn.is_some() || state.sending
            };
            let serial = state.queues.serial();
            let entry = state
                .queues
                .entries
                .get_mut(&key)
                .ok_or("Queue is loading")?;
            if !entry.loaded || entry.messages.len() >= entry.limit {
                return Err("Queue is full or still loading".into());
            }
            if entry.messages.is_empty() {
                entry.paused = false;
                entry.error = None;
                entry.active = active;
                entry.released = !active;
                if router {
                    entry.router_wait = router_gate;
                }
            }
            let mut messages = entry.messages.clone();
            messages.push(Arc::new(message));
            let request = entry.store(serial, messages, Some(original), false)?;
            state.queue_references.insert(
                (key.clone(), serial),
                references
                    .iter()
                    .map(|reference| reference.id.clone())
                    .collect(),
            );
            (serial, request)
        };
        if !self.queue_request(&key, serial, request) {
            return Err("Queue was not saved".into());
        }
        self.update_composer();
        Ok(())
    }
    fn change_queue(&self, key: &str, id: &str, text: Option<String>) -> Result<(), String> {
        let (serial, request) = {
            let mut state = self.state.borrow_mut();
            let serial = state.queues.serial();
            let entry = state
                .queues
                .entries
                .get_mut(key)
                .ok_or("Queue is no longer loaded")?;
            let index = entry
                .messages
                .iter()
                .position(|message| message.id == id)
                .ok_or("Queued message has changed; reopen it")?;
            let mut messages = entry.messages.clone();
            if let Some(text) = text {
                let message = Arc::make_mut(&mut messages[index]);
                let old = message.text.len();
                message.text = text.trim().into();
                message.validate()?;
                message.bytes = message.bytes.saturating_sub(old) + message.text.len();
            } else {
                messages.remove(index);
            }
            (serial, entry.store(serial, messages, None, false)?)
        };
        if !self.queue_request(key, serial, request) {
            return Err("Queue was not saved".into());
        }
        self.update_composer();
        Ok(())
    }
    fn toggle_queue(&self, key: &str) {
        let mut state = self.state.borrow_mut();
        let ready = state
            .queues
            .entries
            .get(key)
            .is_some_and(|entry| state.backend_ready.contains(&entry.context.backend));
        let operating = Self::operation_pending(&state, key);
        let history_ready = Self::history_ready(&state, key);
        let Some(entry) = state.queues.entries.get_mut(key) else {
            return;
        };
        if !entry.loaded {
            drop(state);
            self.load_queue(true);
            return;
        }
        if entry.paused {
            if !entry.router && !history_ready {
                entry.pause("Refresh this conversation before resuming the queue.");
                drop(state);
                self.update_composer();
                return;
            }
            if operating {
                entry.pause("Wait for the current session command before resuming");
                drop(state);
                self.update_composer();
                return;
            }
            if !ready {
                entry.pause("Reconnect this backend before resuming the queue.");
                drop(state);
                self.update_composer();
                return;
            }
            entry.paused = false;
            entry.error = None;
            entry.released = true;
            entry.connected = true;
            entry.touch();
        } else {
            entry.pause("");
        }
        state.queues.dirty = true;
        drop(state);
        self.update_composer();
    }
    pub(super) fn pause_selected_queue(&self, reason: &str) {
        if let Some(context) = self.selected_queue() {
            let mut state = self.state.borrow_mut();
            if let Some(entry) = state.queues.entries.get_mut(&context.key()) {
                entry.pause(reason);
            }
            state.queues.dirty = true;
        }
    }
    pub(super) fn queue_manual_start(&self, thread: &str) {
        let mut state = self.state.borrow_mut();
        let key = format!("{}:{thread}", state.backend);
        if let Some(entry) = state.queues.entries.get_mut(&key) {
            entry.active = true;
            entry.released = false;
            entry.touch();
        }
    }
    pub(super) fn observe_queue(&self, backend: &str, generation: u64, payload: &Value) {
        let method = model::text(payload, "method");
        if !matches!(
            method,
            "studio/appServer/status"
                | "turn/started"
                | "turn/completed"
                | "thread/status/changed"
                | "error"
        ) {
            return;
        }

        let params = &payload["params"];
        let mut state = self.state.borrow_mut();
        if method == "studio/appServer/status"
            && matches!(model::text(params, "state"), "error" | "stopped")
        {
            state.queues.pause_backend(
                backend,
                "Disconnected. Check the conversation before resuming.",
            );
        }
        let key = format!("{backend}:{}", model::text(params, "threadId"));
        if self.router_managed(&key) {
            return;
        }
        if let Some(entry) = state
            .queues
            .entries
            .get_mut(&key)
            .filter(|entry| entry.context.generation == generation)
        {
            entry.observe(method, params);
            state.queues.dirty = true;
        }
    }
    pub(super) fn queue_result(&self, key: String, serial: u64, result: Result<Reply, String>) {
        let mut clear = None;
        let mut start = None;
        let mut editor_result = None;
        let mut notification = None;
        let accepted = matches!(&result, Ok(Reply::Saved));
        {
            let mut state = self.state.borrow_mut();
            let history_ready = Self::history_ready(&state, &key);
            let supervised = self.supervision_holds(&key);
            let Some(entry) = state.queues.entries.get_mut(&key) else {
                return;
            };
            match result {
                Ok(Reply::Read { messages, limit }) if serial == entry.load_serial => {
                    entry.messages = messages;
                    entry.limit = if entry.flow_epoch == self.message_flow.epoch() {
                        limit
                    } else {
                        self.message_flow.limit().unwrap_or(limit)
                    };
                    entry.loaded = true;
                    entry.error = if entry.messages.is_empty() {
                        None
                    } else {
                        Some("Restored queue. Check the conversation before resuming.".into())
                    };
                    entry.paused = true;
                    entry.touch();
                }
                Ok(Reply::Saved)
                    if entry
                        .saving
                        .as_ref()
                        .is_some_and(|saving| saving.serial == serial) =>
                {
                    clear = entry.stored(serial, Ok(()));
                    editor_result = Some(Ok(()));
                }
                Ok(Reply::Prepared(options)) if entry.preparing == Some(serial) => {
                    entry.preparing = None;
                    if !supervised && entry.can_prepare() && (entry.router || history_ready) {
                        let message = entry.messages[0].clone();
                        {
                            entry.flight = Some(Flight {
                                id: message.id.clone(),
                                generation: entry.context.generation,
                                turn: None,
                                completed: false,
                                acknowledged: false,
                                persisted: false,
                            });
                            entry.released = false;
                            start = Some((entry.context.clone(), message, options));
                        }
                    }
                    entry.touch();
                }
                Err(error) => {
                    if entry
                        .saving
                        .as_ref()
                        .is_some_and(|saving| saving.serial == serial)
                    {
                        entry.stored(serial, Err(error.clone()));
                        notification = Some(error.clone());
                        editor_result = Some(Err(error));
                    } else if entry.preparing == Some(serial) {
                        entry.preparing = None;
                        entry.pause(&error);
                    } else if entry.load_serial == serial {
                        entry.loaded = false;
                        entry.pause(&error);
                    }
                }
                _ => {}
            }
            if let Some(ids) = state.queue_references.remove(&(key.clone(), serial)) {
                if accepted {
                    state.references.remove(&key, &ids);
                }
            }
            state.queues.dirty = true;
            state.queues.prune(&key);
        }
        if let Some(error) = notification {
            self.error(&error);
        }
        if let Some(original) = clear {
            let current = self
                .selected_queue()
                .is_some_and(|context| context.key() == key);
            if current && self.composer_text() == original {
                self.composer.buffer().set_text("");
            }
            let mut state = self.state.borrow_mut();
            if state.drafts.get(&key) == Some(&original) {
                state.drafts.remove(&key);
            }
        }
        if let Some(result) = editor_result {
            self.queue
                .saved(&key, result.as_ref().map(|_| ()).map_err(String::as_str));
        }
        if let Some((context, message, options)) = start {
            self.dispatch_queue(context, message, options);
        }
        self.update_composer();
    }
    fn dispatch_queue(
        &self,
        context: Context,
        message: Arc<q::Message>,
        mut options: crate::turn_options::Options,
    ) {
        if self.supervision_holds(&context.key()) {
            if let Some(entry) = self
                .state
                .borrow_mut()
                .queues
                .entries
                .get_mut(&context.key())
            {
                entry.flight = None;
                entry.released = true;
                entry.touch();
            }
            return;
        }
        if self.router_managed(&context.key()) {
            if self.composer_key().as_deref() == Some(context.key().as_str()) {
                options = self.state.borrow().options.clone();
            }
            self.router_dispatch_queue(context, message, options);
            return;
        }
        if self.router.borrow().registry.is_none() {
            if let Some(entry) = self
                .state
                .borrow_mut()
                .queues
                .entries
                .get_mut(&context.key())
            {
                entry.pause(
                    "Thread Router follow-ups are not connected yet; open the target conversation",
                );
            }
            return;
        }
        let visible = self
            .selected_queue()
            .is_some_and(|selected| selected.key() == context.key());
        if visible {
            options = self.state.borrow().options.clone();
        }
        let mut input = Vec::new();
        if !message.text.is_empty() {
            input.push(json!({"type":"text","text":message.text}));
        }
        input.extend(message.input.iter().cloned());
        let mut params = json!({"threadId":context.thread,"input":input});
        options.apply(&mut params, context.kind == "codex");
        let policy_result = self.state.borrow().policies.apply(
            &context.key(),
            &context.root,
            context.kind == "codex",
            &mut params,
        );
        if let Err(error) = policy_result {
            self.queue_ack(
                &context.key(),
                &message.id,
                &context.backend,
                context.generation,
                &json!({"error":{"message":error}}),
            );
            return;
        }

        let id = {
            let mut state = self.state.borrow_mut();
            state.sequence += 1;
            let id = state.sequence;
            state.pending.insert(
                id,
                PendingRequest {
                    backend: context.backend.clone(),
                    generation: context.generation,
                    action: Pending::Queue {
                        key: context.key(),
                        thread: context.thread.clone(),
                        id: message.id.clone(),
                    },
                },
            );
            id
        };
        if !self.command(Command::Rpc {
            images: Vec::new(),
            backend: context.backend.clone(),
            generation: context.generation,
            id,
            method: "turn/start".into(),
            params,
            root: (!context.root.is_empty()).then_some(context.root.clone()),
        }) {
            self.state.borrow_mut().pending.remove(&id);
            self.queue_ack(
                &context.key(),
                &message.id,
                &context.backend,
                context.generation,
                &json!({"error":{"message":"Backend request was not sent"}}),
            );
        }
    }
    pub(super) fn queue_ack(
        &self,
        key: &str,
        id: &str,
        _backend: &str,
        generation: u64,
        payload: &Value,
    ) {
        let mut store = None;
        let mut active = None;
        let mut finished = false;
        {
            let mut state = self.state.borrow_mut();
            let serial = state.queues.serial();
            let Some(entry) = state.queues.entries.get_mut(key) else {
                return;
            };
            if !entry
                .flight
                .as_ref()
                .is_some_and(|flight| flight.id == id && flight.generation == generation)
            {
                return;
            }
            if let Some(error) = payload.get("error") {
                entry.flight = None;
                entry.pause(&format!(
                    "Queue paused. Check the conversation before resuming: {}",
                    error["message"].as_str().unwrap_or("Send failed")
                ));
            } else {
                let flight = entry.flight.as_mut().unwrap();
                flight.acknowledged = true;
                let status = payload
                    .pointer("/result/turn/status")
                    .and_then(Value::as_str)
                    .unwrap_or("inProgress");
                if status != "inProgress" {
                    finished = true;
                    flight.completed = true;
                    entry.active = false;
                    entry.released = status == "completed";
                    if status != "completed" {
                        entry.paused = true;
                        entry.error=Some(payload.pointer("/result/turn/error/message").and_then(Value::as_str).unwrap_or("Queued turn did not complete; check the conversation before resuming").chars().take(2048).collect());
                    }
                }
                if entry.context.kind == "codex" {
                    if let Some(turn) = payload.pointer("/result/turn/id").and_then(Value::as_str) {
                        if flight.turn.is_none() {
                            flight.turn = Some(turn.into());
                        }
                        if !flight.completed {
                            entry.active = true;
                            active = Some(turn.to_owned());
                        }
                    }
                }
                let messages = entry
                    .messages
                    .iter()
                    .enumerate()
                    .filter(|(index, message)| *index != 0 || message.id != id)
                    .map(|(_, message)| message)
                    .cloned()
                    .collect();
                match entry.store(serial, messages, None, true) {
                    Ok(request) => store = Some((serial, request)),
                    Err(error) => entry.pause(&error),
                }
            }
            state.queues.dirty = true;
            if let Some(turn) = active {
                if state
                    .selected
                    .as_ref()
                    .is_some_and(|thread| format!("{}:{}", state.backend, thread.id) == key)
                {
                    state.active_turn = Some(turn);
                }
            } else if finished
                && state
                    .selected
                    .as_ref()
                    .is_some_and(|thread| format!("{}:{}", state.backend, thread.id) == key)
            {
                state.active_turn = None;
            }
        }
        if let Some((serial, request)) = store {
            self.queue_request(key, serial, request);
        }
        self.update_composer();
    }
    pub(super) fn pump_queues(&self) {
        let requests = {
            let mut state = self.state.borrow_mut();
            state.queues.dirty = false;
            let keys = state
                .queues
                .entries
                .iter()
                .filter(|(key, entry)| {
                    !self.supervision_holds(key)
                        && self.router.borrow().registry.is_some()
                        && (if entry.router {
                            self.router_managed(key) && self.router_queue_ready(key)
                        } else {
                            !self.router_managed(key)
                        })
                        && entry.can_prepare()
                        && (entry.router || Self::history_ready(&state, key))
                        && !Self::operation_pending(&state, key)
                })
                .map(|(key, _)| key.clone())
                .collect::<Vec<_>>();
            keys.into_iter()
                .filter_map(|key| {
                    let serial = state.queues.serial();
                    let context = &state.queues.entries.get(&key)?.context;
                    let valid = state.backend_generations.get(&context.backend)
                        == Some(&context.generation);
                    let entry = state.queues.entries.get_mut(&key)?;
                    if !valid {
                        entry.pause("Connection changed. Check before resuming.");
                        return None;
                    }
                    entry.preparing = Some(serial);
                    entry.touch();
                    Some((key, serial))
                })
                .collect::<Vec<_>>()
        };
        for (key, serial) in requests {
            self.queue_request(&key, serial, Request::Prepare);
        }
        self.update_composer();
    }
    pub(super) fn queue_may_close(&self) -> bool {
        if self
            .state
            .borrow()
            .queues
            .entries
            .values()
            .any(|entry| entry.saving.is_some())
        {
            self.error("Wait for the queue change to finish saving");
            return false;
        }
        let editor = self.queue.editor.borrow().clone();
        if let Some(editor) = editor {
            editor.dialog.present(Some(&self.window));
            self.error("Finish or cancel the queued-message edit before closing");
            return false;
        }
        true
    }
}

#[cfg(debug_assertions)]
pub(super) async fn smoke_opencode(ui: &Rc<Ui>, directory: &std::path::Path) -> Result<(), String> {
    async fn until(test: impl Fn() -> bool) -> Result<(), String> {
        let start = std::time::Instant::now();
        while !test() {
            if start.elapsed() > Duration::from_secs(6) {
                return Err("OpenCode queue did not reach its expected state".into());
            }
            glib::timeout_future(Duration::from_millis(25)).await;
        }
        Ok(())
    }
    let key = "opencode:native-oc-thread";
    until(|| {
        ui.state
            .borrow()
            .queues
            .entries
            .get(key)
            .is_some_and(|entry| entry.loaded && !entry.busy())
    })
    .await?;
    let restored = ui.state.borrow().queues.entries[key]
        .messages
        .first()
        .cloned()
        .ok_or("Restored OpenCode queue missing")?;
    ui.change_queue(key, &restored.id, None)?;
    until(|| {
        ui.state.borrow().queues.entries[key].messages.is_empty()
            && !ui.state.borrow().queues.entries[key].busy()
    })
    .await?;
    ui.composer.buffer().set_text("native-oc-queue-hold");
    ui.submit();
    until(|| ui.state.borrow().active_turn.is_some() && !ui.state.borrow().sending).await?;
    for (text, size) in [("native-oc-queue-first", 1), ("native-oc-queue-second", 2)] {
        ui.composer.buffer().set_text(text);
        ui.queue_button.emit_clicked();
        until(|| {
            ui.state.borrow().queues.entries[key].messages.len() == size
                && !ui.state.borrow().queues.entries[key].busy()
        })
        .await?;
    }
    std::fs::write(directory.join("release-native-oc-queue.flag"), "test")
        .map_err(|error| error.to_string())?;
    until(|| {
        ui.state.borrow().queues.entries[key].messages.is_empty()
            && ui.state.borrow().queues.entries[key].flight.is_none()
            && !ui.state.borrow().queues.entries[key].busy()
            && ui.state.borrow().active_turn.is_none()
    })
    .await?;
    let (serial, request) = {
        let mut state = ui.state.borrow_mut();
        let serial = state.queues.serial();
        let entry = state.queues.entries.get_mut(key).unwrap();
        entry.paused = true;
        (serial, entry.store(serial, vec![restored], None, false)?)
    };
    ui.queue_request(key, serial, request);
    until(|| {
        ui.state.borrow().queues.entries[key].messages.len() == 1
            && !ui.state.borrow().queues.entries[key].busy()
    })
    .await?;
    std::fs::write(directory.join("opencode-queue.json"),serde_json::to_vec_pretty(&json!({"ok":true,"serverOwnedIds":true,"authoritativeIdle":true,"earlyCompletion":true,"durableDequeue":true,"restoredQueueRetained":true})).unwrap()).map_err(|error|error.to_string())?;
    Ok(())
}

#[cfg(debug_assertions)]
pub(super) async fn smoke(
    ui: &Rc<Ui>,
    directory: &std::path::Path,
    capture: impl Fn(&str),
    capture_dialog: impl Fn(&adw::Dialog, &str),
) -> Result<Value, String> {
    async fn until(test: impl Fn() -> bool) -> Result<(), String> {
        let start = std::time::Instant::now();
        while !test() {
            if start.elapsed() > Duration::from_secs(6) {
                return Err("Native queue did not reach the expected state".into());
            }
            glib::timeout_future(Duration::from_millis(25)).await;
        }
        Ok(())
    }
    until(|| {
        ui.state.borrow().ready
            && !ui.state.borrow().loading
            && ui.send.is_sensitive()
            && ui.settings.borrow().is_none()
    })
    .await?;
    let key = ui.selected_queue().ok_or("No queue session")?.key();
    until(|| {
        ui.state
            .borrow()
            .queues
            .entries
            .get(&key)
            .is_some_and(|entry| entry.loaded && entry.limit == 3)
    })
    .await?;
    ui.composer.buffer().set_text("native-queue-hold");
    ui.submit();
    until(|| ui.state.borrow().active_turn.is_some() && !ui.state.borrow().sending).await?;
    ui.composer.buffer().set_text("native-queue-one-original");
    ui.queue_button.emit_clicked();
    ui.composer.buffer().set_text("newer unsent draft");
    until(|| {
        ui.state.borrow().queues.entries[&key].messages.len() == 1
            && ui.state.borrow().queues.entries[&key].saving.is_none()
    })
    .await?;
    if ui.composer_text() != "newer unsent draft" {
        return Err("Saving a queued message cleared newer composer text".into());
    }
    ui.composer.buffer().set_text("native-queue-two");
    ui.queue_button.emit_clicked();
    until(|| {
        ui.state.borrow().queues.entries[&key].messages.len() == 2
            && ui.state.borrow().queues.entries[&key].saving.is_none()
    })
    .await?;
    ui.queue
        .smoke_edit("native-queue-one", |dialog| {
            capture_dialog(dialog, "queue-editor.png")
        })
        .await?;
    let database = directory.join("config/codex-thread-studio/studio.sqlite3");
    let backup = directory.join("queue-storage-backup.sqlite3");
    ui.queue
        .smoke_failed_save(
            || {
                std::fs::rename(&database, &backup).map_err(|error| error.to_string())?;
                std::fs::create_dir(&database).map_err(|error| error.to_string())
            },
            || {
                if database.is_dir() {
                    std::fs::remove_dir(&database).map_err(|error| error.to_string())?;
                }
                std::fs::rename(&backup, &database).map_err(|error| error.to_string())
            },
        )
        .await?;
    if ui.state.borrow().queues.entries[&key].paused {
        ui.queue.smoke_toggle();
    }
    if ui.state.borrow().queues.entries[&key].messages[0].text != "native-queue-one" {
        return Err("Queue edit changed the wrong message".into());
    }
    ui.queue.smoke_remove_last()?;
    until(|| {
        ui.state.borrow().queues.entries[&key].messages.len() == 1
            && ui.state.borrow().queues.entries[&key].saving.is_none()
    })
    .await?;
    for (text, size) in [("native-queue-two", 2), ("native-queue-three", 3)] {
        ui.composer.buffer().set_text(text);
        ui.queue_button.emit_clicked();
        until(|| {
            ui.state.borrow().queues.entries[&key].messages.len() == size
                && ui.state.borrow().queues.entries[&key].saving.is_none()
        })
        .await?;
    }
    ui.composer.buffer().set_text("overflow draft must be kept");
    if ui.enqueue().is_ok() || ui.composer_text() != "overflow draft must be kept" {
        return Err("Queue depth limit discarded an unsent draft".into());
    }
    capture("queue.png");
    ui.queue.smoke_toggle();
    if !ui.state.borrow().queues.entries[&key].paused {
        return Err("Queue pause did not take effect".into());
    }
    std::fs::write(directory.join("release-native-queue-hold.flag"), "test")
        .map_err(|error| error.to_string())?;
    until(|| ui.state.borrow().active_turn.is_none()).await?;
    glib::timeout_future(Duration::from_millis(180)).await;
    if ui.state.borrow().queues.entries[&key].messages.len() != 3 {
        return Err("A paused queue advanced".into());
    }
    ui.composer
        .buffer()
        .set_text("Independent unsent queue-test draft");
    ui.queue.smoke_toggle();
    until(|| {
        ui.state.borrow().queues.entries[&key].messages.len() == 2
            && ui.state.borrow().queues.entries[&key]
                .flight
                .as_ref()
                .is_some_and(|flight| flight.persisted)
    })
    .await?;
    let backend_index = ui
        .state
        .borrow()
        .backends
        .iter()
        .position(|backend| backend.kind == "opencode")
        .ok_or("Missing background queue backend")?;
    ui.backend.set_selected(backend_index as u32);
    until(|| {
        ui.state.borrow().backend == "opencode"
            && ui.state.borrow().ready
            && !ui.state.borrow().loading
            && !ui.state.borrow().catalog_loading
    })
    .await?;
    std::fs::write(directory.join("release-native-queue-one.flag"), "test")
        .map_err(|error| error.to_string())?;
    until(|| {
        ui.state.borrow().queues.entries[&key].messages.is_empty()
            && ui.state.borrow().queues.entries[&key].flight.is_none()
            && ui.state.borrow().queues.entries[&key].saving.is_none()
    })
    .await?;
    let index = ui
        .state
        .borrow()
        .backends
        .iter()
        .position(|backend| backend.id == "codex")
        .unwrap();
    ui.backend.set_selected(index as u32);
    until(|| {
        ui.state.borrow().backend == "codex"
            && ui.state.borrow().ready
            && !ui.state.borrow().loading
            && ui.send.is_sensitive()
    })
    .await?;
    if ui.composer_text() != "Independent unsent queue-test draft" {
        return Err("Background queue dispatch overwrote the current draft".into());
    }
    ui.composer.buffer().set_text("native-queue-final-hold");
    ui.submit();
    until(|| ui.state.borrow().active_turn.is_some() && !ui.state.borrow().sending).await?;
    ui.composer.buffer().set_text("native-queue-after-restart");
    ui.queue_button.emit_clicked();
    until(|| {
        ui.state.borrow().queues.entries[&key].messages.len() == 1
            && ui.state.borrow().queues.entries[&key].saving.is_none()
    })
    .await?;
    ui.interrupt();
    until(|| ui.state.borrow().active_turn.is_none()).await?;
    if !ui.state.borrow().queues.entries[&key].paused {
        return Err("Stop did not pause queued follow-ups".into());
    }
    Ok(
        json!({"ok":true,"durableEnqueue":true,"newDraftRetained":true,"editRemove":true,"failedSaveRetention":true,"depthLimit":true,"pauseResume":true,"backgroundDispatch":true,"earlyCompletion":true,"stopPauses":true,"restartPending":true}),
    )
}
