use super::*;
use crate::i18n_ui::{self, Part};
use crate::{
    command_tools::{self, Access},
    composer_tools_ui::Context,
};
impl Ui {
    pub(super) fn command_context(&self) -> Option<Context> {
        let state = self.state.borrow();
        if !state.ready
            || state.loading
            || state.session_mutation
            || (state.selected.is_some() && !state.history_verified)
        {
            return None;
        }
        let backend = state
            .backends
            .iter()
            .find(|backend| backend.id == state.backend)?;
        Some(Context {
            key: format!(
                "{}:{}",
                state.backend,
                state
                    .selected
                    .as_ref()
                    .map(|thread| thread.id.as_str())
                    .unwrap_or("")
            ),
            backend: state.backend.clone(),
            generation: state.socket_generation,
            root: state
                .selected
                .as_ref()
                .map(|thread| thread.cwd.clone())
                .unwrap_or_default(),
            opencode: backend.kind == "opencode",
            target: None,
        })
    }
    pub(super) fn open_commands(self: &Rc<Self>) {
        if let Some(palette) = self.palette.borrow().clone() {
            palette.present(&self.window);
            return;
        }
        let Some(context) = self.command_context() else {
            self.error("Wait for the backend and session to load");
            return;
        };
        let palette = crate::command_palette::Palette::new(context.clone(), self.sender.clone());
        let weak = Rc::downgrade(self);
        *palette.on_action.borrow_mut() = Some(Box::new(move |name| {
            if let Some(ui) = weak.upgrade() {
                if ui.command_context().as_ref() != Some(&context) {
                    ui.error("The session changed; reopen Commands");
                    return;
                }
                if let Err(error) = ui.run_command(&name, None) {
                    ui.error(&error);
                }
            }
        }));
        let weak = Rc::downgrade(self);
        palette.dialog.connect_closed(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.palette.borrow_mut().take();
            }
        });
        self.palette.replace(Some(palette.clone()));
        palette.present(&self.window);
    }
    fn clear_command_text(&self, original: Option<&str>) {
        if original.is_some_and(|original| self.composer_text() == original) {
            self.composer.buffer().set_text("");
        }
    }
    pub(super) fn run_command(
        self: &Rc<Self>,
        name: &str,
        original: Option<String>,
    ) -> Result<(), String> {
        if !command_tools::known(name) {
            return Err("Unknown command".into());
        }
        let context = self
            .command_context()
            .ok_or("Wait for the backend and session to load")?;
        {
            let state = self.state.borrow();
            if !matches!(name, "new" | "router") && state.selected.is_none() {
                return Err("Select a session first".into());
            }
            if matches!(
                name,
                "compact" | "review" | "rename" | "fork" | "archive" | "delete"
            ) && (state.sending
                || state.active_turn.is_some()
                || state.shells.contains(&context.key)
                || self.queue_busy(&state))
            {
                return Err("Finish or stop the current operation first".into());
            }
            if context.opencode
                && matches!(name, "fast" | "archive" | "mcp" | "agent" | "subagents")
            {
                return Err("This command is available for Codex sessions".into());
            }
        }
        self.composer_tools.close();
        if matches!(name, "compact" | "review") {
            return self.start_operation(name, original);
        }
        match name {
            "router" => {
                self.clear_command_text(original.as_deref());
                self.open_router_settings();
            }
            "agent" | "subagents" => {
                self.clear_command_text(original.as_deref());
                self.open_agents(context);
            }
            "mcp" => {
                self.clear_command_text(original.as_deref());
                self.open_mcp(context);
            }
            "model" => {
                if !self.model.is_sensitive() {
                    return Err("Model settings are not ready".into());
                }
                self.clear_command_text(original.as_deref());
                self.model.grab_focus();
                self.model.emit_activate();
            }
            "fast" => {
                if !self.fast.is_sensitive() {
                    return Err("This model does not expose Fast mode".into());
                }
                self.clear_command_text(original.as_deref());
                self.fast.emit_clicked();
            }
            "permissions" => {
                self.clear_command_text(original.as_deref());
                self.open_permissions(context);
            }
            "status" => {
                self.clear_command_text(original.as_deref());
                self.show_session_status();
            }
            "skills" => {
                self.clear_command_text(original.as_deref());
                let buffer = self.composer.buffer();
                let mut end = buffer.end_iter();
                let prefix =
                    if end.offset() > 0 && !self.composer_text().ends_with(char::is_whitespace) {
                        " $"
                    } else {
                        "$"
                    };
                buffer.insert(&mut end, prefix);
                buffer.place_cursor(&end);
                self.composer.grab_focus();
            }
            "diff" => {
                let metadata = self.state.borrow_mut().command_data.get(&context.key);
                let text = metadata.diff.ok_or("No turn diff has been reported for this session; Git Review is available from the workspace toolbar")??;
                if text.document.bytes() == 0 {
                    return Err("This session's current turn diff is empty".into());
                }
                self.clear_command_text(original.as_deref());
                self.source.show_prepared((*text).clone());
                self.show_tool("source");
            }
            "rename" => {
                self.clear_command_text(original.as_deref());
                self.rename_session();
            }
            "fork" => {
                self.clear_command_text(original.as_deref());
                self.fork_session();
            }
            "new" => {
                self.clear_command_text(original.as_deref());
                self.new_session();
            }
            "archive" => {
                self.clear_command_text(original.as_deref());
                self.archive_session();
            }
            "delete" => {
                self.clear_command_text(original.as_deref());
                self.delete_session();
            }
            "copy" => {
                let text = self
                    .state
                    .borrow()
                    .items
                    .iter()
                    .rev()
                    .find(|item| item.role == "Assistant")
                    .map(|item| item.body.clone())
                    .ok_or("No assistant response is loaded")?;
                self.composer.clipboard().set_text(&text);
                self.clear_command_text(original.as_deref());
                self.toast
                    .add_toast(adw::Toast::new("Copied latest assistant response"));
            }
            _ => return Err("This command is still being ported".into()),
        }
        Ok(())
    }
    fn show_session_status(self: &Rc<Self>) {
        if let Some(dialog) = self.status_dialog.borrow().as_ref() {
            dialog.present(Some(&self.window));
            return;
        }
        let mut state = self.state.borrow_mut();
        let key = state
            .selected
            .as_ref()
            .map(|thread| format!("{}:{}", state.backend, thread.id))
            .unwrap_or_default();
        let usage = state.command_data.get(&key).usage;
        let field = |key: &'static str, value: Part| {
            vec![
                Part::message(key, &[]),
                Part::literal(": "),
                value,
                Part::literal("\n"),
            ]
        };
        let optional = |value: Option<&str>, fallback: &'static str| {
            value.map_or_else(|| Part::message(fallback, &[]), Part::literal)
        };
        let mut parts = Vec::new();
        for (key, value) in [
            ("Backend", Part::literal(state.backend.clone())),
            (
                "Session",
                optional(state.selected.as_ref().map(|t| t.id.as_str()), "None"),
            ),
            (
                "Project",
                optional(state.selected.as_ref().map(|t| t.cwd.as_str()), "None"),
            ),
            (
                "Model",
                Part::literal(if state.options.model.is_empty() {
                    &state.session_model
                } else {
                    &state.options.model
                }),
            ),
            (
                "Effort",
                if state.options.effort.is_empty() {
                    Part::message("Inherit session", &[])
                } else {
                    Part::literal(&state.options.effort)
                },
            ),
            (
                "Service tier",
                Part::literal(state.options.effective_tier(&state.session_tier)),
            ),
            (
                "Active turn",
                optional(state.active_turn.as_deref(), "None"),
            ),
            (
                "Message permissions",
                Part::message(
                    state
                        .policies
                        .get(&key)
                        .map(|p| p.access.label())
                        .unwrap_or("Inherit session"),
                    &[],
                ),
            ),
            ("Gateway", Part::literal(state.gateway_version.clone())),
        ] {
            parts.extend(field(key, value));
        }
        parts.extend([
            Part::literal("\n"),
            Part::message("Token usage", &[]),
            Part::literal("\n"),
            optional(usage.as_deref(), "No data reported"),
        ]);
        drop(state);
        let dialog = adw::Dialog::builder()
            .title("Session status")
            .content_width(620)
            .content_height(400)
            .build();
        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&adw::HeaderBar::new());
        let label = gtk::Label::builder()
            .selectable(true)
            .xalign(0.0)
            .yalign(0.0)
            .wrap(true)
            .margin_start(20)
            .margin_end(20)
            .margin_top(12)
            .margin_bottom(20)
            .build();
        i18n_ui::bind(&dialog, "title", "Session status");
        i18n_ui::bind_parts(&label, "label", parts, "");
        toolbar.set_content(Some(&gtk::ScrolledWindow::builder().child(&label).build()));
        dialog.set_child(Some(&toolbar));
        let weak = Rc::downgrade(self);
        dialog.connect_closed(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.status_dialog.borrow_mut().take();
            }
        });
        self.status_dialog.replace(Some(dialog.clone()));
        dialog.present(Some(&self.window));
    }
    fn open_permissions(self: &Rc<Self>, context: Context) {
        if let Some(dialog) = self.permission_dialog.borrow().as_ref() {
            dialog.present(Some(&self.window));
            return;
        }
        let dialog = adw::Dialog::builder()
            .title("Message permissions")
            .content_width(590)
            .content_height(430)
            .build()
            .ui_text("title", "Message permissions");
        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(
            &adw::WindowTitle::new("Message permissions", "Selected session")
                .ui_text("title", "Message permissions")
                .ui_text("subtitle", "Selected session"),
        ));
        toolbar.add_top_bar(&header);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
        body.set_margin_start(20);
        body.set_margin_end(20);
        body.set_margin_bottom(20);
        let note_key = if context.opencode {
            "OpenCode permissions use project configuration and runtime approvals. Requests can be allowed once, always allowed, or denied."
        } else {
            "Applies to new messages and queued follow-ups in this Studio run. Existing turns and server commands keep their own permissions. Restarting Studio restores session inheritance."
        };
        let note = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .build()
            .ui_text("label", note_key);
        body.append(&note);
        let feedback = gtk::Label::builder().xalign(0.0).wrap(true).build();
        feedback.add_css_class("caption");
        if !context.opencode {
            let current = match self.state.borrow().policies.get(&context.key) {
                None => Some(Access::Inherit),
                Some(policy) if policy.root == context.root => Some(policy.access),
                _ => None,
            };
            let current_label = gtk::Label::builder().xalign(0.0).wrap(true).build();
            if let Some(access) = current {
                i18n_ui::bind_parts(
                    &current_label,
                    "label",
                    vec![
                        Part::message("Current", &[]),
                        Part::literal(": "),
                        Part::message(access.label(), &[]),
                    ],
                    "",
                );
            } else {
                i18n_ui::bind(
                    &current_label,
                    "label",
                    "The project changed. Choose message permissions again.",
                );
            }
            current_label.add_css_class("caption");
            body.append(&current_label);
            let list = gtk::ListBox::new();
            list.add_css_class("boxed-list");
            list.set_selection_mode(gtk::SelectionMode::None);
            for (access, detail) in [
                (Access::Inherit, "Use the backend session's permissions"),
                (Access::ReadOnly, "Read files; request approval when needed"),
                (
                    Access::WorkspaceWrite,
                    "Write the selected project; network disabled",
                ),
                (
                    Access::FullAccess,
                    "Disable sandbox restrictions for future messages",
                ),
            ] {
                let row = adw::ActionRow::builder()
                    .title(access.label())
                    .subtitle(detail)
                    .build()
                    .ui_text("title", access.label())
                    .ui_text("subtitle", detail);
                let button = gtk::Button::with_label(if current == Some(access) {
                    "Selected"
                } else {
                    "Use"
                });
                i18n_ui::bind(
                    &button,
                    "label",
                    if current == Some(access) {
                        "Selected"
                    } else {
                        "Use"
                    },
                );
                button.set_valign(gtk::Align::Center);
                row.add_suffix(&button);
                list.append(&row);
                let weak = Rc::downgrade(self);
                let scope = context.clone();
                let sheet = dialog.clone();
                let feedback = feedback.clone();
                button.connect_clicked(move |_| {
                    let Some(ui) = weak.upgrade() else { return };
                    if access != Access::FullAccess {
                        match ui.save_policy(&scope, access) {
                            Ok(()) => { sheet.close(); }
                            Err(error) => i18n_ui::literal(&feedback, "label", &error),
                        }
                        return;
                    }
                    let confirm = adw::MessageDialog::builder()
                        .transient_for(&ui.window)
                        .heading("Use full access for future messages?")
                        .body("This disables sandbox restrictions for this session's new messages and queued follow-ups.")
                        .build()
                        .ui_text("heading", "Use full access for future messages?")
                        .ui_text("body", "This disables sandbox restrictions for this session's new messages and queued follow-ups.");
                    confirm.add_responses(&[("cancel", "Cancel"), ("use", "Use full access")]);
                    i18n_ui::response(&confirm, "cancel", "Cancel");
                    i18n_ui::response(&confirm, "use", "Use full access");
                    confirm.set_response_appearance("use", adw::ResponseAppearance::Destructive);
                    confirm.set_default_response(Some("cancel"));
                    confirm.set_close_response("cancel");
                    let weak = Rc::downgrade(&ui);
                    let scope = scope.clone();
                    let sheet = sheet.clone();
                    let feedback = feedback.clone();
                    confirm.connect_response(None, move |_, response| {
                        if response == "use" {
                            if let Some(ui) = weak.upgrade() {
                                match ui.save_policy(&scope, access) {
                                    Ok(()) => { sheet.close(); }
                                    Err(error) => i18n_ui::literal(&feedback, "label", &error),
                                }
                            }
                        }
                    });
                    confirm.present();
                });
            }
            body.append(&list);
        }
        body.append(&feedback);
        toolbar.set_content(Some(&body));
        dialog.set_child(Some(&toolbar));
        let weak = Rc::downgrade(self);
        dialog.connect_closed(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.permission_dialog.borrow_mut().take();
            }
        });
        self.permission_dialog.replace(Some(dialog.clone()));
        dialog.present(Some(&self.window));
    }
    fn save_policy(&self, context: &Context, access: Access) -> Result<(), String> {
        if self.command_context().as_ref() != Some(context) || context.opencode {
            return Err("The session changed; reopen Permissions".into());
        }
        self.state
            .borrow_mut()
            .policies
            .set(&context.key, &context.root, access)?;
        self.supervision_invalidate(&context.key);
        self.toast.add_toast(adw::Toast::new(
            "Message permissions updated for this session",
        ));
        self.update_composer();
        Ok(())
    }
    pub(super) fn operation_pending(state: &State, key: &str) -> bool {
        state.shells.contains(key)||state.pending.values().any(|request|matches!(&request.action,Pending::Operation{thread,..} if format!("{}:{}",request.backend,thread)==key))
    }
    pub(super) fn start_operation(
        self: &Rc<Self>,
        name: &str,
        original: Option<String>,
    ) -> Result<(), String> {
        if self.router_selected() {
            return Err("Open the target conversation for this operation".into());
        }
        let context = self
            .command_context()
            .ok_or("Wait for the backend and session to load")?;
        let thread = self
            .state
            .borrow()
            .selected
            .clone()
            .ok_or("Select a session first")?;
        {
            let state = self.state.borrow();
            if state.sending
                || state.active_turn.is_some()
                || state.shells.contains(&context.key)
                || state.archived
                || self.queue_busy(&state)
            {
                return Err("Finish or stop the current operation first".into());
            }
        }
        let mut params = json!({"threadId":thread.id});
        let method = match name {
            "shell" => {
                let command = original
                    .as_deref()
                    .and_then(command_tools::shell)
                    .filter(|command| !command.is_empty())
                    .ok_or("Enter a shell command after !")?;
                if command.len() > 256 * 1024 {
                    return Err("Shell commands must be 256 KiB or smaller".into());
                }
                let state = self.state.borrow();
                if !state.images.images(&context.key).is_empty()
                    || !state.references.get(&context.key).is_empty()
                {
                    return Err("Shell commands cannot include image or reference attachments; remove them first".into());
                }
                params["command"] = json!(command);
                if context.opencode {
                    let model = if !state.options.model.is_empty() {
                        &state.options.model
                    } else if !state.session_model.is_empty() {
                        &state.session_model
                    } else {
                        &state.default_model
                    };
                    if !model.contains('/') {
                        return Err(
                            "Choose an OpenCode model before running a shell command".into()
                        );
                    }
                    params["model"] = json!(model);
                }
                "thread/shellCommand"
            }
            "compact" => "thread/compact/start",
            "review" => {
                params["target"] = json!({"type":"uncommittedChanges"});
                params["delivery"] = json!("inline");
                "review/start"
            }
            _ => return Err("Unknown session operation".into()),
        };
        self.pause_selected_queue("Paused for a session command; resume when ready");
        if name == "shell" {
            self.state
                .borrow_mut()
                .shells
                .begin(&context.key, context.generation)?;
            self.queue_manual_start(&thread.id);
        }
        self.state.borrow_mut().sending = true;
        self.update_composer();
        self.rpc(
            Pending::Operation {
                thread: thread.id,
                name: match name {
                    "shell" => "shell",
                    "compact" => "compact",
                    _ => "review",
                },
                original,
            },
            method,
            params,
        );
        Ok(())
    }
    pub(super) fn operation_response(
        &self,
        backend: &str,
        thread: &str,
        name: &str,
        original: Option<String>,
        payload: &Value,
        prepared: protocol::Prepared,
        visible: bool,
    ) {
        let key = format!("{backend}:{thread}");
        let success = payload.get("error").is_none();
        {
            let mut state = self.state.borrow_mut();
            state.cache.0.update(&key, |view| {
                view.sending = false;
                view.valid = false;
            });
            if visible {
                state.sending = false;
            }
            if !success {
                state.shells.request_failed(&key);
            }
            if success
                && original
                    .as_ref()
                    .is_some_and(|text| state.drafts.get(&key) == Some(text))
            {
                state.drafts.remove(&key);
            }
        }
        if visible {
            if success {
                self.clear_command_text(original.as_deref());
                self.present_reply(prepared.reply_items);
            } else {
                self.error(
                    payload
                        .pointer("/error/message")
                        .and_then(Value::as_str)
                        .unwrap_or("Session command failed"),
                );
            }
        }
        if !success {
            if let Some(entry) = self.state.borrow_mut().queues.entries.get_mut(&key) {
                entry.pause("Session command failed; check the conversation before resuming");
            }
            if visible {
                self.read_session(thread, true);
            } else {
                self.toast.add_toast(adw::Toast::new(&format!(
                    "{backend}: session command failed"
                )));
            }
        }
        if success && payload.pointer("/result/turn/id").is_some() {
            self.complete_manual_reply(backend, thread, payload);
            if payload
                .pointer("/result/turn/status")
                .and_then(Value::as_str)
                .is_some_and(|status| matches!(status, "completed" | "failed" | "interrupted"))
            {
                self.state.borrow_mut().shells.forget(&key);
            }
        }
        if success
            && name == "review"
            && payload
                .pointer("/result/turn/status")
                .and_then(Value::as_str)
                .is_none_or(|status| status == "inProgress")
        {
            if let Some(turn) = payload.pointer("/result/turn/id").and_then(Value::as_str) {
                let mut state = self.state.borrow_mut();
                if visible
                    && !state
                        .activity
                        .get(&key)
                        .is_some_and(|activity| activity.finished(turn))
                {
                    state.active_turn = Some(turn.into());
                }
            }
        }
        if visible {
            self.update_composer();
        }
    }
    pub(super) fn observe_operations(
        &self,
        backend: &str,
        generation: u64,
        payload: &Value,
        prepared: &protocol::Prepared,
    ) {
        let method = model::text(payload, "method");
        if !matches!(method, "turn/started" | "turn/completed") && !prepared.shell_item {
            return;
        }
        let params = &payload["params"];
        let thread = model::text(params, "threadId");
        if thread.is_empty() {
            return;
        }
        let key = format!("{backend}:{thread}");
        let mut state = self.state.borrow_mut();
        if prepared.shell_item && !state.shells.contains(&key) {
            let _ = state.shells.begin(&key, generation);
        }
        let finished = state.shells.observe(&key, generation, method, params);
        if finished {
            state.cache.0.update(&key, |view| view.sending = false);
        }
    }
}

#[cfg(debug_assertions)]
async fn until(mut ready: impl FnMut() -> bool) -> Result<(), String> {
    let began = std::time::Instant::now();
    while !ready() {
        if began.elapsed() > Duration::from_secs(7) {
            return Err("Native command operation timed out".into());
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    Ok(())
}
#[cfg(debug_assertions)]
fn permission_button(widget: &gtk::Widget, title: &str) -> Option<gtk::Button> {
    fn button(widget: &gtk::Widget) -> Option<gtk::Button> {
        if let Some(button) = widget.downcast_ref::<gtk::Button>() {
            if matches!(button.label().as_deref(), Some("Use" | "Selected")) {
                return Some(button.clone());
            }
        }
        let mut child = widget.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            if let Some(found) = button(&widget) {
                return Some(found);
            }
        }
        None
    }
    if let Some(row) = widget.downcast_ref::<adw::ActionRow>() {
        if row.title() == title {
            return button(widget);
        }
    }
    let mut child = widget.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        if let Some(found) = permission_button(&widget, title) {
            return Some(found);
        }
    }
    None
}
#[cfg(debug_assertions)]
async fn choose_policy(ui: &Rc<Ui>, access: Access) -> Result<(), String> {
    let context = ui.command_context().ok_or("No permission context")?;
    ui.open_permissions(context);
    glib::timeout_future(Duration::from_millis(30)).await;
    let dialog = ui
        .permission_dialog
        .borrow()
        .clone()
        .ok_or("Permissions did not open")?;
    permission_button(dialog.upcast_ref(), access.label())
        .ok_or("Permission choice missing")?
        .emit_clicked();
    until(|| ui.permission_dialog.borrow().is_none()).await
}
#[cfg(debug_assertions)]
fn confirmation() -> Option<adw::MessageDialog> {
    gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|window| window.downcast::<adw::MessageDialog>().ok())
        .find(|dialog| dialog.heading().as_deref() == Some("Use full access for future messages?"))
}
#[cfg(debug_assertions)]
pub(super) async fn smoke(
    ui: &Rc<Ui>,
    directory: &std::path::Path,
    capture: impl Fn(&adw::Dialog, &str),
    window_capture: impl Fn(&str),
) -> Result<Value, String> {
    until(|| ui.send.is_sensitive() && !ui.state.borrow().loading).await?;
    let key = ui.composer_key().ok_or("No command test session")?;
    let draft = ui.composer_text();
    ui.composer
        .buffer()
        .set_text("Keep this command-palette draft 中文😀");
    ui.open_commands();
    let palette = ui
        .palette
        .borrow()
        .clone()
        .ok_or("Command palette did not open")?;
    palette.smoke_query("permissions");
    palette.smoke_query("status");
    until(|| palette.smoke_has("status") && !palette.smoke_has("permissions")).await?;
    let previous = palette.smoke_identity();
    crate::window_capture::ready(&palette.dialog.child().ok_or("Palette has no content")?).await?;
    capture(&palette.dialog, "commands.png");
    palette.smoke_pick("status")?;
    until(|| ui.palette.borrow().is_none() && ui.status_dialog.borrow().is_some()).await?;
    if ui.composer_text() != "Keep this command-palette draft 中文😀" {
        return Err("Opening a palette command changed the draft".into());
    }
    let dialog = ui.status_dialog.borrow().clone().unwrap();
    crate::window_capture::ready(&dialog.child().ok_or("Status dialog has no content")?).await?;
    capture(&dialog, "command-status.png");
    dialog.close();
    until(|| ui.status_dialog.borrow().is_none()).await?;
    ui.open_commands();
    let newer = ui.palette.borrow().clone().unwrap();
    until(|| newer.smoke_has("status")).await?;
    ui.event(Event::CommandPalette {
        dialog: previous.0,
        serial: previous.1,
        result: Ok(vec![Arc::new(crate::composer_refs::Suggestion {
            kind: crate::composer_refs::Kind::Command,
            name: "obsolete".into(),
            path: "command:obsolete".into(),
            title: "Obsolete".into(),
            detail: String::new(),
        })]),
    });
    if newer.smoke_has("obsolete") || !newer.smoke_has("status") {
        return Err("An old palette response changed its replacement".into());
    }
    newer.dialog.close();
    until(|| ui.palette.borrow().is_none()).await?;
    ui.composer.buffer().set_text("/sta");
    until(|| ui.composer_tools.smoke_has("status")).await?;
    ui.composer_tools.smoke_pick("status")?;
    until(|| ui.status_dialog.borrow().is_some()).await?;
    if !ui.composer_text().is_empty() {
        return Err("Inline slash command was sent as text".into());
    }
    let status_dialog = ui.status_dialog.borrow().clone().unwrap();
    status_dialog.close();
    until(|| ui.status_dialog.borrow().is_none()).await?;

    super::mcp_commands::smoke(ui, directory, &capture).await?;

    choose_policy(ui, Access::ReadOnly).await?;
    let context = ui.command_context().unwrap();
    ui.open_permissions(context.clone());
    glib::timeout_future(Duration::from_millis(30)).await;
    let dialog = ui.permission_dialog.borrow().clone().unwrap();
    crate::window_capture::ready(&dialog.child().ok_or("Permissions dialog has no content")?)
        .await?;
    capture(&dialog, "permissions.png");
    permission_button(dialog.upcast_ref(), Access::FullAccess.label())
        .ok_or("Full access choice missing")?
        .emit_clicked();
    until(|| confirmation().is_some()).await?;
    confirmation().unwrap().response("cancel");
    if ui
        .state
        .borrow()
        .policies
        .get(&key)
        .map(|policy| policy.access)
        != Some(Access::ReadOnly)
    {
        return Err("Cancelling full access changed permissions".into());
    }
    permission_button(dialog.upcast_ref(), Access::FullAccess.label())
        .unwrap()
        .emit_clicked();
    until(|| confirmation().is_some()).await?;
    confirmation().unwrap().response("use");
    until(|| ui.permission_dialog.borrow().is_none()).await?;
    if ui
        .state
        .borrow()
        .policies
        .get(&key)
        .map(|policy| policy.access)
        != Some(Access::FullAccess)
    {
        return Err("Confirmed full access was not scoped to the session".into());
    }
    choose_policy(ui, Access::WorkspaceWrite).await?;
    let scope = ui.command_context().unwrap();
    ui.open_permissions(scope);
    let stale = ui.permission_dialog.borrow().clone().unwrap();
    let original_session = ui.state.borrow().selected.clone().unwrap();
    let other = ui
        .state
        .borrow()
        .threads
        .iter()
        .find(|thread| thread.id == "native-cache-thread")
        .cloned()
        .ok_or("Permission sibling missing")?;
    ui.select(other);
    until(|| !ui.state.borrow().loading).await?;
    permission_button(stale.upcast_ref(), Access::ReadOnly.label())
        .ok_or("Stale permission choice missing")?
        .emit_clicked();
    if ui
        .state
        .borrow()
        .policies
        .get(&key)
        .map(|policy| policy.access)
        != Some(Access::WorkspaceWrite)
        || ui
            .state
            .borrow()
            .policies
            .get(&ui.composer_key().unwrap())
            .is_some()
    {
        return Err("A stale permissions dialog changed another session".into());
    }
    stale.close();
    until(|| ui.permission_dialog.borrow().is_none()).await?;
    ui.select(original_session);
    until(|| !ui.state.borrow().loading && ui.send.is_sensitive()).await?;
    ui.composer.buffer().set_text("native-command-policy");
    ui.submit();
    until(|| !ui.state.borrow().sending && ui.state.borrow().active_turn.is_none()).await?;

    ui.composer.buffer().set_text("!native-shell-hold");
    until(|| ui.shell_hint.is_visible() && ui.send.label().as_deref() == Some("Run")).await?;
    ui.submit();
    until(|| {
        !ui.state.borrow().sending
            && ui.state.borrow().active_turn.is_some()
            && ui.state.borrow().shells.contains(&key)
    })
    .await?;
    window_capture("shell-command.png");
    ui.composer
        .buffer()
        .set_text("Must not send a new message during shell execution");
    ui.submit();
    if ui.send.is_sensitive() {
        return Err("Shell acknowledgement enabled a conflicting send".into());
    }
    ui.composer.buffer().set_text("!native-shell-queue");
    if ui.enqueue().is_ok() {
        return Err("Shell syntax was accepted into the message queue".into());
    }
    ui.composer
        .buffer()
        .set_text("native-command-policy-queued");
    until(|| ui.queue_button.is_sensitive()).await?;
    ui.queue_button.emit_clicked();
    until(|| {
        ui.state.borrow().queues.entries[&key].saving.is_none()
            && ui.state.borrow().queues.entries[&key].messages.len() == 1
    })
    .await?;
    choose_policy(ui, Access::ReadOnly).await?;
    let selected = ui.state.borrow().selected.clone().unwrap();
    let sibling = ui
        .state
        .borrow()
        .threads
        .iter()
        .find(|thread| thread.id == "native-cache-thread")
        .cloned()
        .ok_or("Command sibling missing")?;
    ui.select(sibling);
    until(|| !ui.state.borrow().loading).await?;
    if ui
        .state
        .borrow()
        .shells
        .contains(&ui.composer_key().unwrap())
    {
        return Err("Shell state leaked into another session".into());
    }
    std::fs::write(directory.join("release-native-shell.flag"), b"finish")
        .map_err(|error| error.to_string())?;
    until(|| {
        !ui.state.borrow().shells.contains(&key)
            && ui.state.borrow().queues.entries[&key].messages.is_empty()
            && ui.state.borrow().queues.entries[&key].flight.is_none()
    })
    .await?;
    ui.select(selected);
    until(|| !ui.state.borrow().loading && ui.send.is_sensitive()).await?;
    ui.composer.buffer().set_text("!native-shell-fail");
    ui.submit();
    until(|| !ui.state.borrow().sending && !ui.state.borrow().loading && ui.send.is_sensitive())
        .await?;
    if ui.composer_text() != "!native-shell-fail" || ui.state.borrow().shells.contains(&key) {
        return Err("Failed shell command discarded its draft or left a busy session".into());
    }
    ui.composer.buffer().set_text("/compact");
    ui.submit();
    until(|| !ui.state.borrow().sending && ui.composer_text().is_empty()).await?;
    ui.composer.buffer().set_text("/review");
    ui.submit();
    until(|| {
        !ui.state.borrow().sending
            && ui.state.borrow().active_turn.is_none()
            && ui.composer_text().is_empty()
    })
    .await?;
    if !ui
        .state
        .borrow()
        .items
        .iter()
        .any(|item| item.body.contains("Native review result"))
    {
        return Err("Review result did not reach the native transcript".into());
    }
    until(|| ui.state.borrow_mut().command_data.get(&key).diff.is_some()).await?;
    let metadata = ui.state.borrow_mut().command_data.get(&key);
    if !metadata
        .usage
        .as_deref()
        .is_some_and(|usage| usage.contains("4242"))
    {
        return Err("Token usage was not routed to its session".into());
    }
    ui.run_command("status", None)?;
    let status_dialog = ui.status_dialog.borrow().clone().unwrap();
    crate::window_capture::ready(
        &status_dialog
            .child()
            .ok_or("Status dialog has no content")?,
    )
    .await?;
    capture(&status_dialog, "command-status-usage.png");
    status_dialog.close();
    ui.run_command("diff", None)?;
    if !ui.source.visible_text().contains("native-turn-diff-中文😀")
        || ui.source.visible_text().contains("wrong-session")
    {
        return Err("The conversation diff opened the wrong data".into());
    }
    window_capture("command-diff.png");
    ui.tools.set_visible(false);
    ui.update_workspace();
    ui.composer.buffer().set_text("/review");
    ui.submit();
    let review_turn = format!("native-review-{}", ui.state.borrow().sequence);
    until(|| {
        ui.state.borrow().sending
            && ui
                .state
                .borrow()
                .activity
                .get(&key)
                .is_some_and(|activity| activity.finished(&review_turn))
    })
    .await?;
    ui.composer
        .buffer()
        .set_text("native-command-policy-review");
    until(|| ui.queue_button.is_sensitive()).await?;
    ui.queue_button.emit_clicked();
    until(|| {
        !ui.state.borrow().sending
            && ui.state.borrow().queues.entries[&key].messages.is_empty()
            && ui.state.borrow().queues.entries[&key].flight.is_none()
            && ui.state.borrow().active_turn.is_none()
    })
    .await?;

    choose_policy(ui, Access::Inherit).await?;
    ui.composer.buffer().set_text(&draft);
    Ok(
        json!({"ok":true,"palette":true,"palettePreservesDraft":true,"recreatedPaletteIsolation":true,"inlineSlash":true,"permissionConfirmation":true,"stalePermissionsIsolation":true,"permissionInheritance":true,"futureQueuePolicy":true,"shellMode":true,"shellLifecycle":true,"offscreenShell":true,"shellFailureRetention":true,"shellQueueGuard":true,"compact":true,"reviewEarlyCompletion":true,"queuedAfterEarlyCompletion":true,"scopedTurnDiff":true,"tokenUsage":true,"mcpInventory":true}),
    )
}

#[cfg(debug_assertions)]
pub(super) async fn smoke_opencode(
    ui: &Rc<Ui>,
    directory: &std::path::Path,
    capture: impl Fn(&adw::Dialog, &str),
) -> Result<(), String> {
    let key = ui.composer_key().ok_or("No OpenCode command session")?;
    let seed = ui.state.borrow().queues.entries[&key].messages.clone();
    ui.open_commands();
    let palette = ui
        .palette
        .borrow()
        .clone()
        .ok_or("OpenCode palette did not open")?;
    until(|| palette.smoke_has("review")).await?;
    if palette.smoke_has("fast") || palette.smoke_has("archive") {
        return Err("OpenCode palette offered unsupported commands".into());
    }
    palette.dialog.close();
    until(|| ui.palette.borrow().is_none()).await?;
    let scope = ui.command_context().unwrap();
    ui.open_permissions(scope);
    let dialog = ui.permission_dialog.borrow().clone().unwrap();
    crate::window_capture::ready(&dialog.child().ok_or("Permissions dialog has no content")?)
        .await?;
    capture(&dialog, "opencode-permissions.png");
    if permission_button(dialog.upcast_ref(), "Full access").is_some() {
        return Err("Codex sandbox choices appeared for OpenCode".into());
    }
    dialog.close();
    until(|| ui.permission_dialog.borrow().is_none()).await?;
    if !ui.state.borrow().options.model.contains('/') {
        ui.model.set_selected(1);
        until(|| ui.state.borrow().options.model.contains('/')).await?;
    }
    ui.composer.buffer().set_text("!native-opencode-shell");
    ui.submit();
    until(|| !ui.state.borrow().sending && !ui.state.borrow().shells.contains(&key)).await?;
    if !ui
        .state
        .borrow()
        .items
        .iter()
        .any(|item| item.body.contains("Native OpenCode shell output"))
    {
        return Err("OpenCode shell response was not rendered".into());
    }
    for command in ["compact", "review"] {
        ui.composer.buffer().set_text(&format!("/{command}"));
        ui.submit();
        until(|| {
            !ui.state.borrow().sending
                && ui.composer_text().is_empty()
                && ui.state.borrow().active_turn.is_none()
        })
        .await?;
    }
    if ui.state.borrow().queues.entries[&key]
        .messages
        .iter()
        .map(|message| message.id.clone())
        .collect::<Vec<_>>()
        != seed
            .iter()
            .map(|message| message.id.clone())
            .collect::<Vec<_>>()
        || !ui.state.borrow().queues.entries[&key].paused
    {
        return Err("OpenCode commands changed the paused queue".into());
    }
    std::fs::write(directory.join("opencode-commands.json"),serde_json::to_vec_pretty(&json!({"ok":true,"capabilities":true,"projectPermissions":true,"shellModel":true,"shellReply":true,"compact":true,"review":true,"pausedQueueRetained":true})).unwrap()).map_err(|error|error.to_string())?;
    Ok(())
}
