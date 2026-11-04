use super::*;
use crate::{
    composer_refs::{self, Kind, Suggestion},
    composer_tools_ui::{Context, Query},
};
impl Ui {
    pub(super) fn connect_reference_backend(&self, id: &str) -> Result<(), String> {
        let connection = {
            let mut state = self.state.borrow_mut();
            if state.backend_ready.contains(id) {
                return Ok(());
            }
            let backend = state
                .backends
                .iter()
                .find(|backend| backend.id == id)
                .cloned()
                .ok_or("The target backend is no longer configured")?;
            if state.backend == id {
                return Err("Wait for the controller backend to reconnect".into());
            }
            state.connection_sequence += 1;
            let generation = state.connection_sequence;
            state.backend_generations.insert(id.to_owned(), generation);
            state.pending.retain(|_, request| request.backend != id);
            state.cache.invalidate(id);
            (backend, generation)
        };
        self.begin_backend_connection(id, connection.1, None);
        if self.command(Command::Connect {
            backend: connection.0,
            generation: connection.1,
        }) {
            Ok(())
        } else {
            self.recovery.borrow_mut().forget(id);
            let mut state = self.state.borrow_mut();
            if state.backend_generations.get(id) == Some(&connection.1) {
                state.backend_generations.remove(id);
            }
            Err("The target metadata connection could not be started".into())
        }
    }
    pub(super) fn reference_context(&self) -> Option<Context> {
        let state = self.state.borrow();
        if !state.ready || state.loading || state.archived || state.session_mutation {
            return None;
        }
        let thread = state.selected.as_ref()?;
        let key = format!("{}:{}", state.backend, thread.id);
        let target = if let Some(session) = self.router_reference_target(&key) {
            let backend = state
                .backends
                .iter()
                .find(|backend| backend.id == session.backend)?;
            Some(crate::composer_tools_ui::ReferenceTarget {
                generation: *state.backend_generations.get(&session.backend)?,
                opencode: backend.kind == "opencode",
                session,
            })
        } else {
            None
        };
        Some(Context {
            key,
            backend: state.backend.clone(),
            generation: state.socket_generation,
            root: thread.cwd.clone(),
            opencode: state
                .backends
                .iter()
                .any(|backend| backend.id == state.backend && backend.kind == "opencode"),
            target,
        })
    }
    pub(super) fn connect_references(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.composer.buffer().connect_changed(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.composer_revision
                    .set(ui.composer_revision.get().wrapping_add(1));
                if let Some(flight) = ui.continuation.borrow().as_ref() {
                    flight
                        .cancel
                        .store(true, std::sync::atomic::Ordering::Release);
                }
                ui.schedule_references();
            }
        });
        let weak = Rc::downgrade(self);
        self.composer.buffer().connect_mark_set(move |_, _, mark| {
            if mark.name().as_deref() == Some("insert") {
                if let Some(ui) = weak.upgrade() {
                    ui.schedule_references();
                }
            }
        });
        let weak = Rc::downgrade(self);
        self.composer.connect_preedit_changed(move |_, text| {
            if let Some(ui) = weak.upgrade() {
                ui.composer_tools.preedit.set(!text.is_empty());
                if !text.is_empty() {
                    ui.composer_tools.close();
                    ui.composer_revision
                        .set(ui.composer_revision.get().wrapping_add(1));
                    if let Some(flight) = ui.continuation.borrow().as_ref() {
                        flight
                            .cancel
                            .store(true, std::sync::atomic::Ordering::Release);
                    }
                }
                ui.schedule_references();
            }
        });
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, _, state| {
            weak.upgrade()
                .map(|ui| ui.composer_tools.key(key, state))
                .unwrap_or(glib::Propagation::Proceed)
        });
        self.composer.add_controller(keys);
        let weak = Rc::downgrade(self);
        *self.composer_tools.on_pick.borrow_mut() = Some(Box::new(move |query, choice| {
            if let Some(ui) = weak.upgrade() {
                if let Err(error) = ui.insert_reference(query, choice) {
                    ui.error(&error);
                }
            }
        }));
        let weak = Rc::downgrade(self);
        *self.composer_tools.on_remove.borrow_mut() = Some(Box::new(move |key, id| {
            if let Some(ui) = weak.upgrade() {
                ui.state.borrow_mut().references.remove(&key, &[id]);
                ui.update_composer();
            }
        }));
    }
    fn schedule_references(self: &Rc<Self>) {
        if self.composer_tools.scheduled.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            if let Some(ui) = weak.upgrade() {
                ui.composer_tools.scheduled.set(false);
                ui.update_composer();
                ui.references_changed();
            }
        });
    }
    pub(super) fn references_changed(self: &Rc<Self>) {
        let Some(query) = self
            .composer_tools
            .begin(self.reference_context(), &self.composer.buffer())
        else {
            return;
        };
        let caret = self
            .composer
            .iter_location(&self.composer.buffer().iter_at_offset(query.trigger.end));
        let (x, y) = self.composer.buffer_to_window_coords(
            gtk::TextWindowType::Widget,
            caret.x(),
            caret.y(),
        );
        let point = self
            .composer_tools
            .popover
            .parent()
            .and_then(|parent| {
                self.composer
                    .compute_point(&parent, &gtk::graphene::Point::new(x as f32, y as f32))
            })
            .unwrap_or_else(|| gtk::graphene::Point::new(x as f32, y as f32));
        self.composer_tools
            .popover
            .set_pointing_to(Some(&gtk::gdk::Rectangle::new(
                point.x() as i32,
                point.y() as i32,
                caret.width().max(1),
                caret.height(),
            )));
        self.composer_tools.popover.present();
        let weak = Rc::downgrade(self);
        glib::timeout_add_local_once(Duration::from_millis(120), move || {
            if let Some(ui) = weak.upgrade() {
                if ui.composer_tools.current(query.serial)
                    && ui.reference_context().as_ref() == Some(&query.context)
                {
                    if query.trigger.kind == Kind::Command {
                        ui.tools_reply(
                            query.serial,
                            Ok(Some(crate::command_tools::catalog(query.context.opencode))),
                        );
                    } else if query.trigger.kind == Kind::File {
                        let mut params = json!({"query":query.trigger.query,"roots":[query.context.reference_root()],"cancellationToken":format!("native-{}",query.serial)});
                        if query.context.reference_opencode() {
                            params["cwd"] = query.context.reference_root().into();
                        }
                        ui.reference_rpc(&query, "fuzzyFileSearch", params);
                    } else {
                        let mut params =
                            json!({"cwds":[query.context.reference_root()],"forceReload":false});
                        if query.context.reference_opencode() {
                            params["cwd"] = query.context.reference_root().into();
                        }
                        ui.reference_rpc(&query, "skills/list", params);
                        if !query.context.reference_opencode() {
                            ui.reference_rpc(
                                &query,
                                "plugin/list",
                                json!({"cwds":[query.context.reference_root()],"forceRefetch":false}),
                            );
                        }
                    }
                }
            }
        });
    }
    // Only metadata reads can use a backend other than the visible controller.
    // Owner and target scopes both participate in Context equality on reply.
    fn reference_rpc(&self, query: &Query, method: &str, params: Value) {
        if !matches!(method, "fuzzyFileSearch" | "skills/list" | "plugin/list")
            || self.reference_context().as_ref() != Some(&query.context)
            || !self.composer_tools.current(query.serial)
        {
            return;
        }
        let id = {
            let mut state = self.state.borrow_mut();
            state.sequence += 1;
            state.sequence
        };
        let backend = query.context.reference_backend().to_owned();
        let generation = query.context.reference_generation();
        if self.command(Command::Rpc {
            backend: backend.clone(),
            generation,
            id,
            method: method.into(),
            params,
            root: Some(query.context.reference_root().to_owned()),
            images: Vec::new(),
        }) {
            self.state.borrow_mut().pending.insert(
                id,
                PendingRequest {
                    backend,
                    generation,
                    action: Pending::Tools(query.serial),
                },
            );
        } else {
            self.composer_tools
                .error(query.serial, "Reference discovery could not be started");
        }
    }
    pub(super) fn tools_reply(
        &self,
        serial: u64,
        result: Result<Option<Arc<Vec<Arc<Suggestion>>>>, String>,
    ) {
        if !self.composer_tools.current(serial) {
            return;
        }
        let query = self.composer_tools.query.borrow().clone();
        let Some(query) =
            query.filter(|query| self.reference_context().as_ref() == Some(&query.context))
        else {
            return;
        };
        match result {
            Ok(source) => {
                if let Some(source) = source {
                    self.composer_tools.sources.borrow_mut().push(source);
                }
                self.command(Command::FilterTools {
                    serial: {
                        self.composer_tools
                            .filter_ticket
                            .set(self.composer_tools.filter_ticket.get() + 1);
                        self.composer_tools.filter_ticket.get()
                    },
                    sources: self.composer_tools.sources.borrow().clone(),
                    query: query.trigger.query,
                });
            }
            Err(error) => self.composer_tools.error(serial, &error),
        }
    }
    pub(super) fn insert_reference(
        self: &Rc<Self>,
        query: Query,
        choice: Arc<Suggestion>,
    ) -> Result<(), String> {
        if !self.composer_tools.current(query.serial)
            || self.reference_context().as_ref() != Some(&query.context)
        {
            return Err("The reference belongs to a previous conversation; select it again".into());
        }
        if choice.kind == Kind::Command {
            let original = self.composer_text();
            if original.trim() != format!("/{}", query.trigger.query) {
                return Err("The composer changed; select the command again".into());
            }
            return self.run_command(&choice.name, Some(original));
        }
        let buffer = self.composer.buffer();
        let start = buffer.iter_at_offset(query.trigger.start);
        let end = buffer.iter_at_offset(query.trigger.end);
        let current = buffer.text(&start, &end, false);
        let before = if query.trigger.kind == Kind::File {
            format!("@{}", query.trigger.query)
        } else {
            format!("${}", query.trigger.query)
        };
        if current != before || buffer.cursor_position() != query.trigger.end {
            return Err("The composer changed; choose the reference again".into());
        }
        let structured_files = query.context.reference_opencode() || self.router_selected();
        self.state.borrow_mut().references.add(
            &query.context.key,
            glib::uuid_string_random().into(),
            &choice,
            query.context.reference_root(),
            structured_files,
        )?;
        self.composer_tools.close();
        buffer.begin_user_action();
        let mut start = buffer.iter_at_offset(query.trigger.start);
        let mut end = buffer.iter_at_offset(query.trigger.end);
        buffer.delete(&mut start, &mut end);
        buffer.insert(&mut start, &composer_refs::replacement(&choice));
        buffer.end_user_action();
        self.composer.grab_focus();
        self.update_composer();
        Ok(())
    }
}

#[cfg(debug_assertions)]
async fn until(mut condition: impl FnMut() -> bool) -> Result<(), String> {
    let started = std::time::Instant::now();
    while !condition() {
        if started.elapsed() > Duration::from_secs(7) {
            return Err("Native reference operation timed out".into());
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
#[cfg(debug_assertions)]
pub(super) async fn pick(ui: &Rc<Ui>, text: &str, name: &str) -> Result<(), String> {
    ui.composer.grab_focus();
    ui.composer.buffer().set_text(text);
    until(|| ui.composer_tools.smoke_has(name)).await?;
    ui.composer_tools.smoke_pick(name)?;
    Ok(())
}
#[cfg(debug_assertions)]
pub(super) async fn smoke(
    ui: &Rc<Ui>,
    directory: &std::path::Path,
    capture: impl Fn(&str),
) -> Result<Value, String> {
    until(|| ui.send.is_sensitive() && !ui.state.borrow().loading && !ui.state.borrow().sending)
        .await?;
    let key = ui.composer_key().ok_or("No reference session")?;
    let draft = ui.composer_text();
    std::fs::write(
        directory.join("project/nested/说明 note.md"),
        "# Reference file\n\n中文😀\n",
    )
    .map_err(|error| error.to_string())?;
    ui.composer.grab_focus();
    ui.composer.buffer().set_text("中文😀 @说明");
    until(|| ui.composer_tools.smoke_has("nested/说明 note.md")).await?;
    ui.composer_tools.smoke_keyboard_insert()?;

    if ui.composer_text() != "中文😀 \"nested/说明 note.md\" "
        || !ui.state.borrow().references.get(&key).is_empty()
    {
        return Err("Codex file insertion did not preserve scalar offsets and quoting".into());
    }
    ui.composer.buffer().set_text("@stale");
    glib::timeout_future(Duration::from_millis(200)).await;
    ui.composer.buffer().set_text("@note");
    until(|| ui.composer_tools.smoke_has("nested/note.md")).await?;
    glib::timeout_future(Duration::from_millis(550)).await;
    if ui.composer_tools.smoke_has("old-stale-result.md") {
        return Err("A late file search replaced a newer query".into());
    }
    ui.composer_tools
        .key(gtk::gdk::Key::Escape, gtk::gdk::ModifierType::empty());

    ui.composer.buffer().set_text("$native");
    until(|| {
        ui.composer_tools.smoke_has("native-build") && ui.composer_tools.smoke_has("native-plugin")
    })
    .await?;
    if ui.composer_tools.smoke_has("native-disabled") {
        return Err("Disabled skill appeared in native completion".into());
    }
    glib::timeout_future(Duration::from_millis(60)).await;
    ui.composer_tools
        .smoke_capture(&directory.join("references-picker.png"))?;
    ui.composer_tools.smoke_pick("native-build")?;
    pick(ui, "$native-build $native", "native-plugin").await?;
    if ui.state.borrow().references.get(&key).len() != 2 {
        return Err("Skill/plugin inputs were not retained".into());
    }
    ui.composer.buffer().set_text("native-ref-reject");
    ui.submit();
    until(|| !ui.state.borrow().sending && !ui.state.borrow().loading && ui.send.is_sensitive())
        .await?;
    if ui.state.borrow().references.get(&key).len() != 2
        || ui.composer_text() != "native-ref-reject"
    {
        return Err("Failed reference send discarded input".into());
    }
    ui.composer.buffer().set_text("native-ref-late");
    ui.submit();
    pick(ui, "Newer $native-second", "native-second").await?;
    ui.composer
        .buffer()
        .set_text("Newer reference draft 中文😀");
    let selected = ui.state.borrow().selected.clone().unwrap();
    let sibling = ui
        .state
        .borrow()
        .threads
        .iter()
        .find(|thread| thread.id == "native-cache-thread")
        .cloned()
        .ok_or("Reference sibling missing")?;
    ui.select(sibling);
    until(||!ui.state.borrow().loading&&!ui.state.borrow().pending.values().any(|request|matches!(&request.action,Pending::Send{thread,..} if thread==&selected.id))).await?;
    if ui.state.borrow().references.get(&key).len() != 1
        || !ui
            .state
            .borrow()
            .references
            .get(&ui.composer_key().unwrap())
            .is_empty()
    {
        return Err(
            "Offscreen reference acknowledgement affected newer or other-session inputs".into(),
        );
    }
    ui.select(selected);
    until(|| !ui.state.borrow().loading && ui.send.is_sensitive()).await?;
    if ui.composer_text() != "Newer reference draft 中文😀" {
        return Err("Offscreen reference send cleared a newer draft".into());
    }
    glib::timeout_future(Duration::from_millis(50)).await;
    capture("references.png");
    ui.composer.buffer().set_text("native-ref-active");
    // Keep the newer skill for the queued message, not the holding turn.
    let newer = ui.state.borrow().references.get(&key);
    ui.state.borrow_mut().references.remove(
        &key,
        &newer.iter().map(|item| item.id.clone()).collect::<Vec<_>>(),
    );
    ui.submit();
    until(|| !ui.state.borrow().sending && ui.state.borrow().active_turn.is_some()).await?;
    pick(ui, "$native-second", "native-second").await?;
    ui.composer.buffer().set_text("native-ref-queued");
    ui.queue_button.emit_clicked();
    until(|| {
        ui.state.borrow().references.get(&key).is_empty()
            && ui
                .state
                .borrow()
                .queues
                .entries
                .get(&key)
                .is_some_and(|entry| entry.saving.is_none() && entry.messages.len() == 1)
    })
    .await?;
    if ui.state.borrow().queues.entries[&key].messages[0].input[0]["type"] != "skill" {
        return Err("Queued skill reference was dropped".into());
    }
    ui.interrupt();
    until(|| ui.state.borrow().active_turn.is_none()).await?;
    ui.queue.smoke_toggle();
    until(|| {
        ui.state
            .borrow()
            .queues
            .entries
            .get(&key)
            .is_some_and(|entry| entry.messages.is_empty() && entry.flight.is_none())
            && ui.state.borrow().active_turn.is_none()
    })
    .await?;
    if !ui
        .state
        .borrow()
        .items
        .iter()
        .any(|item| item.body.contains("Worker-projected command result"))
    {
        return Err("Completed reference reply was not rendered".into());
    }
    // Exercise removing a retained reference through its actual native button.
    pick(ui, "$native-build", "native-build").await?;
    ui.composer_tools.smoke_remove_first()?;
    if !ui.state.borrow().references.get(&key).is_empty() {
        return Err("Native reference remove button did not work".into());
    }
    ui.composer.buffer().set_text(&draft);
    Ok(
        json!({"ok":true,"unicodeOffsets":true,"keyboardSelection":true,"imeKeyGuard":true,"quotedFilePath":true,"staleQueryIsolation":true,"skillsAndPlugins":true,"disabledSkillsHidden":true,"failedSendRetention":true,"newerReferencesRetained":true,"sessionIsolation":true,"queuedReferences":true,"replyProjection":true,"remove":true}),
    )
}

#[cfg(debug_assertions)]
pub(super) async fn smoke_opencode(ui: &Rc<Ui>, directory: &std::path::Path) -> Result<(), String> {
    let key = ui.composer_key().ok_or("No OpenCode reference session")?;
    let seed = ui.state.borrow().queues.entries[&key].messages.clone();
    ui.queue.smoke_remove_last()?;
    until(|| {
        ui.state.borrow().queues.entries[&key].messages.is_empty()
            && ui.state.borrow().queues.entries[&key].saving.is_none()
    })
    .await?;
    pick(ui, "@说明", "nested/说明 note.md").await?;
    if ui.state.borrow().references.get(&key).len() != 1 {
        return Err("OpenCode file input missing".into());
    }
    ui.composer.buffer().set_text("native-oc-file-reference");
    ui.submit();
    until(|| {
        !ui.state.borrow().sending
            && ui.state.borrow().active_turn.is_none()
            && ui.state.borrow().references.get(&key).is_empty()
    })
    .await?;
    pick(ui, "$native-build", "native-build").await?;
    ui.composer.buffer().set_text("native-oc-command");
    ui.submit();
    until(|| {
        !ui.state.borrow().sending
            && ui.state.borrow().active_turn.is_none()
            && ui.state.borrow().references.get(&key).is_empty()
    })
    .await?;
    if !ui
        .state
        .borrow()
        .items
        .iter()
        .any(|item| item.body.contains("Returned without SSE duplication"))
    {
        return Err("OpenCode command response was not projected without SSE".into());
    }
    ui.composer.buffer().set_text("native-oc-ref-hold");
    ui.submit();
    until(|| !ui.state.borrow().sending && ui.state.borrow().active_turn.is_some()).await?;
    pick(ui, "$native-build", "native-build").await?;
    ui.composer.buffer().set_text("native-oc-ref-queued");
    ui.queue_button.emit_clicked();
    until(|| {
        ui.state.borrow().references.get(&key).is_empty()
            && ui.state.borrow().queues.entries[&key].messages.len() == 1
            && ui.state.borrow().queues.entries[&key].saving.is_none()
    })
    .await?;
    std::fs::write(directory.join("release-native-oc-ref.flag"), b"finish")
        .map_err(|error| error.to_string())?;
    until(|| {
        ui.state.borrow().queues.entries[&key].messages.is_empty()
            && ui.state.borrow().queues.entries[&key].flight.is_none()
            && ui.state.borrow().active_turn.is_none()
    })
    .await?;
    let (serial, request) = {
        let mut state = ui.state.borrow_mut();
        let serial = state.queues.serial();
        let entry = state.queues.entries.get_mut(&key).unwrap();
        entry.paused = true;
        (serial, entry.store(serial, seed, None, false)?)
    };
    ui.command(Command::Queue {
        key: key.clone(),
        serial,
        request,
    });
    until(|| ui.state.borrow().queues.entries[&key].saving.is_none()).await?;
    std::fs::write(directory.join("opencode-references.json"),serde_json::to_vec_pretty(&json!({"ok":true,"fileParts":true,"skillCommands":true,"serverOwnedIdentity":true,"replyWithoutSse":true,"queuedSkillCompletion":true,"seedQueueRetained":true})).unwrap()).map_err(|error|error.to_string())?;
    Ok(())
}
