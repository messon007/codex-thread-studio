//! Explicit native session information editing. Store reads never overwrite drafts.
use crate::{
    conversation_pages::Source,
    protocol::Command,
    session_info::{self as data, Edit, Reply, Request, Stored},
};
use adw::prelude::*;
use gtk::glib;
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::sync::mpsc;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket {
    pub dialog: u64,
    pub serial: u64,
}
#[derive(Clone)]
pub struct Input {
    pub source: Source,
    pub title: String,
    pub backend_name: String,
    pub routable: bool,
    pub capture: bool,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Read,
    Save,
    Generated,
}
pub struct Panel {
    pub id: u64,
    pub dialog: adw::Dialog,
    pub input: Input,
    pub on_generate: RefCell<Option<Box<dyn Fn(Ticket, String) -> Result<(), String>>>>,
    pub on_stop: RefCell<Option<Box<dyn Fn(&str) -> Result<(), String>>>>,
    parent: glib::WeakRef<adw::ApplicationWindow>,
    sender: mpsc::Sender<Command>,
    question: gtk::TextView,
    role: gtk::TextView,
    metadata: gtk::Box,
    status: gtk::Label,
    capture: gtk::Label,
    save: gtk::Button,
    copy: gtk::Button,
    reload: gtk::Button,
    generate: gtk::Button,
    stop: gtk::Button,
    serial: Cell<u64>,
    pending: RefCell<HashMap<u64, Kind>>,
    stored: RefCell<Option<Stored>>,
    cancel: RefCell<Arc<AtomicBool>>,
    generation: Cell<Option<Ticket>>,
    turn: RefCell<Option<String>>,
    dirty: Cell<bool>,
    applying: Cell<bool>,
    valid: Cell<bool>,
    allowed: Cell<bool>,
    can_stop: Cell<bool>,
    reload_required: Cell<bool>,
    prompt: Cell<bool>,
    close_after_save: Cell<bool>,
}
fn value(view: &gtk::TextView) -> String {
    let b = view.buffer();
    b.text(&b.start_iter(), &b.end_iter(), true).to_string()
}
fn label(text: &str) -> gtk::Label {
    let l = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .build();
    crate::i18n_ui::bind(&l, "label", text);
    l
}
fn field(body: &gtk::Box, name: &str, view: &gtk::TextView) {
    let title = label(name);
    title.add_css_class("heading");
    body.append(&title);
    let scroll = gtk::ScrolledWindow::builder()
        .min_content_height(110)
        .max_content_height(170)
        .propagate_natural_height(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(view)
        .build();
    scroll.add_css_class("card");
    body.append(&scroll);
}
impl Panel {
    #[cfg(debug_assertions)]
    pub(crate) fn smoke_state(&self) -> serde_json::Value {
        serde_json::json!({"question":value(&self.question),"role":value(&self.role),"loaded":self.stored.borrow().is_some(),"dirty":self.dirty.get(),"generating":self.generation.get().is_some(),"valid":self.valid.get(),"status":self.status.text().to_string(),"saveEnabled":self.save.is_sensitive(),"generateEnabled":self.generate.is_sensitive(),"writing":self.saving(),"reloadRequired":self.reload_required.get()})
    }
    pub fn new(
        parent: &adw::ApplicationWindow,
        sender: mpsc::Sender<Command>,
        input: Input,
        id: u64,
    ) -> Rc<Self> {
        let dialog = adw::Dialog::builder()
            .content_width(700)
            .content_height(740)
            .build();
        crate::i18n_ui::bind(&dialog, "title", "Session information");
        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        let heading = adw::WindowTitle::new(
            &input.title.replace('\0', "�"),
            &input.backend_name.replace('\0', "�"),
        );
        header.set_title_widget(Some(&heading));
        let reload =
            crate::workspace_ui::button("view-refresh-symbolic", "Reload session information");
        let copy = crate::workspace_ui::button("edit-copy-symbolic", "Copy opening question");
        let save = gtk::Button::with_label("Save");
        save.add_css_class("suggested-action");
        crate::i18n_ui::bind(&save, "label", "Save");
        header.pack_start(&reload);
        header.pack_end(&save);
        header.pack_end(&copy);
        toolbar.add_top_bar(&header);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 10);
        for p in ["margin-start", "margin-end", "margin-top", "margin-bottom"] {
            body.set_property(p, 20i32);
        }
        let question = gtk::TextView::builder()
            .wrap_mode(gtk::WrapMode::WordChar)
            .top_margin(8)
            .bottom_margin(8)
            .left_margin(8)
            .right_margin(8)
            .build();
        crate::i18n_ui::accessible_label(&question, "Opening question");
        field(&body, "Opening question", &question);
        let capture = label("Loading session information…");
        capture.add_css_class("caption");
        capture.add_css_class("dim-label");
        body.append(&capture);
        let role = crate::responsibility_ui::view();
        let generate = gtk::Button::with_label("Generate with AI");
        crate::i18n_ui::bind(&generate, "label", "Generate with AI");
        let stop = gtk::Button::with_label("Stop generation");
        crate::i18n_ui::bind(&stop, "label", "Stop generation");
        if input.routable {
            field(&body, "Session responsibility", &role);
            let description=label("Used by Thread Router to decide which requests should be dispatched to this session.");
            description.add_css_class("caption");
            description.add_css_class("dim-label");
            body.append(&description);
            let actions = crate::adaptive::toolbar(8);
            actions.append(&stop);
            actions.append(&generate);
            body.append(&actions);
            let note =
                label("Generation adds a turn to this session. Review the result before saving.");
            note.add_css_class("caption");
            note.add_css_class("dim-label");
            body.append(&note);
        }
        let metadata = gtk::Box::new(gtk::Orientation::Vertical, 8);
        body.append(&metadata);
        let status = label("");
        status.set_selectable(true);
        body.append(&status);
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&body)
            .build();
        toolbar.set_content(Some(&scroll));
        dialog.set_child(Some(&toolbar));
        let panel = Rc::new(Self {
            id,
            dialog,
            input,
            on_generate: Default::default(),
            on_stop: Default::default(),
            parent: parent.downgrade(),
            sender,
            question,
            role,
            metadata,
            status,
            capture,
            save,
            copy,
            reload,
            generate,
            stop,
            serial: Cell::new(0),
            pending: Default::default(),
            stored: Default::default(),
            cancel: RefCell::new(Arc::new(AtomicBool::new(false))),
            generation: Cell::new(None),
            turn: Default::default(),
            dirty: Cell::new(false),
            applying: Cell::new(false),
            valid: Cell::new(true),
            allowed: Cell::new(false),
            can_stop: Cell::new(false),
            reload_required: Cell::new(false),
            prompt: Cell::new(false),
            close_after_save: Cell::new(false),
        });
        panel.render_metadata(Vec::new());
        for view in [&panel.question, &panel.role] {
            let weak = Rc::downgrade(&panel);
            view.buffer().connect_changed(move |_| {
                if let Some(p) = weak.upgrade() {
                    p.changed();
                }
            });
        }
        let weak = Rc::downgrade(&panel);
        panel.copy.connect_clicked(move |button| {
            if let Some(p) = weak.upgrade() {
                button.clipboard().set_text(&value(&p.question));
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.save.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.save(true);
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.reload.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                if p.dirty.get() {
                    p.confirm(false);
                } else {
                    p.begin();
                }
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.generate.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.generate();
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.stop.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                if p.can_stop.get() {
                    if let (Some(turn), Some(action)) =
                        (p.turn.borrow().as_ref(), p.on_stop.borrow().as_ref())
                    {
                        if let Err(error) = action(turn) {
                            p.notice(&error);
                        }
                    }
                }
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.dialog.connect_close_attempt(move |_| {
            if let Some(p) = weak.upgrade() {
                if p.saving() {
                    p.notice("Wait for session information to finish saving");
                } else {
                    p.confirm(true);
                }
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.dialog.connect_closed(move |_| {
            if let Some(p) = weak.upgrade() {
                p.cancel.borrow().store(true, Ordering::Release);
            }
        });
        panel.controls();
        panel
    }
    fn next(&self) -> u64 {
        let serial = self.serial.get() + 1;
        self.serial.set(serial);
        serial
    }
    fn notice(&self, message: &str) {
        crate::i18n_ui::bind(&self.status, "label", message);
    }
    fn error(&self, message: &str) {
        crate::i18n_ui::literal(&self.status, "label", &message.replace('\0', "�"));
    }
    fn saving(&self) -> bool {
        self.pending.borrow().values().any(|k| *k == Kind::Save)
    }
    fn reading(&self) -> bool {
        self.pending.borrow().values().any(|k| *k == Kind::Read)
    }
    fn changed(&self) {
        if self.applying.get() {
            return;
        }
        let stored = self.stored.borrow();
        let question = stored
            .as_ref()
            .and_then(|s| s.message.as_ref())
            .map_or("", |m| m.text.as_str());
        let role = stored.as_ref().map_or("", |s| s.responsibility.as_str());
        self.dirty.set(
            value(&self.question) != question.replace('\0', "�")
                || self.input.routable && value(&self.role) != role.replace('\0', "�"),
        );
        drop(stored);
        self.controls();
    }
    fn controls(&self) {
        let busy = self.reading() || self.saving();
        let loaded = self.stored.borrow().is_some();
        let generation = self.generation.get().is_some();
        self.question
            .set_editable(loaded && self.valid.get() && !busy);
        self.role
            .set_editable(loaded && self.valid.get() && !busy && !generation);
        self.copy.set_sensitive(!value(&self.question).is_empty());
        self.save.set_sensitive(
            loaded
                && self.dirty.get()
                && self.valid.get()
                && !busy
                && !generation
                && !self.reload_required.get(),
        );
        self.reload
            .set_sensitive(self.valid.get() && !busy && !generation);
        self.generate.set_sensitive(
            loaded
                && self.input.routable
                && self.valid.get()
                && self.allowed.get()
                && !busy
                && !generation,
        );
        self.stop.set_visible(generation);
        self.stop
            .set_sensitive(generation && self.can_stop.get() && self.valid.get());
        self.dialog
            .set_can_close(!self.dirty.get() && !self.saving());
    }
    fn send(&self, kind: Kind, request: Request) {
        let serial = self.next();
        self.pending.borrow_mut().insert(serial, kind);
        if self
            .sender
            .try_send(Command::SessionInfo {
                dialog: self.id,
                serial,
                request,
            })
            .is_err()
        {
            self.pending.borrow_mut().remove(&serial);
            self.error("Session information is busy; try again");
            if kind == Kind::Generated {
                self.generation.set(None);
            }
        }
        self.controls();
    }
    pub fn begin(&self) {
        if self.saving() || !self.valid.get() {
            return;
        }
        self.cancel.borrow().store(true, Ordering::Release);
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel.replace(cancel.clone());
        self.pending.borrow_mut().clear();
        self.notice("Loading session information…");
        self.send(
            Kind::Read,
            Request::Read {
                source: self.input.source.clone(),
                capture: self.input.capture,
                cancel,
            },
        );
    }
    fn save(&self, close: bool) {
        if self.saving()
            || self.reading()
            || !self.valid.get()
            || self.reload_required.get()
            || self.generation.get().is_some()
        {
            return;
        }
        let Some(stored) = self.stored.borrow().clone() else {
            return;
        };
        let question = value(&self.question).trim().to_owned();
        let role = value(&self.role).trim().to_owned();
        if question.len() > data::QUESTION_BYTES {
            self.notice("Opening questions support at most 16 KiB of UTF-8 text");
            return;
        }
        if role.chars().count() > data::ROLE_CHARS {
            self.notice("Responsibilities support at most 4,096 characters");
            return;
        }
        let captured_at = glib::DateTime::now_utc()
            .and_then(|d| d.format_iso8601())
            .map(|s| s.to_string())
            .unwrap_or_else(|_| "unknown".into());
        let original_question = stored
            .message
            .as_ref()
            .map_or("", |m| m.text.as_str())
            .replace('\0', "�");
        let changed_question = (question != original_question).then_some(question);
        let changed_role = (self.input.routable
            && role != stored.responsibility.replace('\0', "�"))
        .then_some(role);
        if changed_question.is_none() && changed_role.is_none() {
            self.apply_stored(stored);
            if close {
                self.dialog.force_close();
            }
            self.controls();
            return;
        }
        self.close_after_save.set(close);
        self.notice("Saving session information…");
        self.send(
            Kind::Save,
            Request::Save(Edit {
                session_key: stored.session_key,
                expected_revision: stored.revision,
                question: changed_question,
                responsibility: changed_role,
                captured_at,
                capture_only: false,
                truncated: false,
            }),
        );
    }
    fn apply_stored(&self, stored: Stored) {
        self.applying.set(true);
        self.question.buffer().set_text(
            &stored
                .message
                .as_ref()
                .map_or("", |m| m.text.as_str())
                .replace('\0', "�"),
        );
        self.role
            .buffer()
            .set_text(&stored.responsibility.replace('\0', "�"));
        let message = stored.message.as_ref();
        let source = message
            .filter(|m| !m.text.is_empty())
            .map(|m| m.source.as_str());
        if let Some(source) = source.filter(|s| !matches!(*s, "history" | "manual" | "")) {
            crate::i18n_ui::literal(&self.capture, "label", &source.replace('\0', "�"));
        } else {
            let text = match source {
                Some("history") => "Extracted from history",
                Some("manual") => "Edited manually",
                _ => "Not identified",
            };
            crate::i18n_ui::bind_summary(
                &self.capture,
                "label",
                text,
                &[],
                if message.is_some_and(|m| m.truncated) {
                    &["Truncated"]
                } else {
                    &[]
                },
            );
        }
        self.stored.replace(Some(stored));
        self.dirty.set(false);
        self.reload_required.set(false);
        self.applying.set(false);
    }
    fn render_metadata(&self, fields: Vec<data::Field>) {
        while let Some(child) = self.metadata.first_child() {
            self.metadata.remove(&child);
        }
        let mut all = vec![
            data::Field {
                label: "Backend",
                value: self.input.backend_name.clone(),
            },
            data::Field {
                label: "Session ID",
                value: self.input.source.context.thread.clone(),
            },
        ];
        if !fields.iter().any(|f| f.label == "Project directory") {
            all.push(data::Field {
                label: "Project directory",
                value: self.input.source.directory.clone(),
            });
        }
        all.extend(fields);
        for field in all {
            let row = gtk::Box::new(gtk::Orientation::Vertical, 2);
            let name = label(field.label);
            name.add_css_class("caption");
            name.add_css_class("dim-label");
            let text = gtk::Label::builder()
                .xalign(0.0)
                .wrap(true)
                .wrap_mode(gtk::pango::WrapMode::WordChar)
                .selectable(true)
                .build();
            if field.label == "Status" {
                let translated = match field.value.as_str() {
                    "idle" => Some("Idle"),
                    "active" | "inProgress" | "busy" => Some("Working…"),
                    "disconnected" => Some("Disconnected"),
                    _ => None,
                };
                if let Some(key) = translated {
                    crate::i18n_ui::bind(&text, "label", key);
                } else {
                    crate::i18n_ui::literal(&text, "label", &field.value);
                }
            } else {
                crate::i18n_ui::literal(&text, "label", &field.value);
            }
            row.append(&name);
            row.append(&text);
            self.metadata.append(&row);
        }
    }
    fn generate(&self) {
        if !self.generate.is_sensitive() {
            return;
        }
        let ticket = Ticket {
            dialog: self.id,
            serial: self.next(),
        };
        self.generation.set(Some(ticket));
        self.turn.borrow_mut().take();
        self.can_stop.set(false);
        self.notice("Generating session responsibility…");
        self.controls();
        let result = self
            .on_generate
            .borrow()
            .as_ref()
            .ok_or("Session generation is unavailable".into())
            .and_then(|f| f(ticket, value(&self.question)));
        if let Err(error) = result {
            self.generation.set(None);
            self.error(&error);
            self.controls();
        }
    }
    pub fn acknowledged(&self, ticket: Ticket, result: Result<String, String>) {
        if self.generation.get() != Some(ticket) || !self.valid.get() {
            return;
        }
        match result {
            Ok(turn) => {
                self.turn.replace(Some(turn.clone()));
                self.send(
                    Kind::Generated,
                    Request::Observe {
                        source: self.input.source.clone(),
                        turn,
                        cancel: self.cancel.borrow().clone(),
                    },
                );
            }
            Err(error) => {
                self.generation.set(None);
                self.error(&error);
                self.controls();
            }
        }
    }
    pub fn sync(&self, valid: bool, allowed: bool, active: Option<&str>) {
        self.allowed.set(allowed);
        self.can_stop.set(
            self.turn
                .borrow()
                .as_deref()
                .is_some_and(|turn| data::can_stop(&self.input.source, turn, active)),
        );
        if !valid && self.valid.replace(false) {
            self.cancel.borrow().store(true, Ordering::Release);
            self.generation.set(None);
            self.notice("The session or connection changed; reopen session information");
        }
        self.controls();
    }
    pub fn event(&self, serial: u64, result: Result<Reply, data::Failure>) {
        let Some(kind) = self.pending.borrow_mut().remove(&serial) else {
            return;
        };
        if !self.valid.get() && kind != Kind::Save {
            self.controls();
            return;
        }
        match result {
            Ok(Reply::Loaded(loaded)) if kind == Kind::Read => {
                self.apply_stored(loaded.stored);
                self.render_metadata(loaded.fields);
                if let Some(warning) = loaded.warning {
                    self.error(&warning);
                } else {
                    self.notice("");
                }
            }
            Ok(Reply::Saved(stored)) if kind == Kind::Save => {
                self.apply_stored(stored);
                self.notice("Session information saved");
                if self.close_after_save.replace(false) {
                    self.dialog.force_close();
                }
            }
            Ok(Reply::Generated(text)) if kind == Kind::Generated => {
                self.generation.set(None);
                self.turn.borrow_mut().take();
                self.role.buffer().set_text(&text);
                self.changed();
                self.notice("Session responsibility generated. Review it before saving.");
            }
            Err(error) => {
                if kind == Kind::Generated {
                    self.generation.set(None);
                    self.turn.borrow_mut().take();
                }
                if kind == Kind::Save {
                    self.reload_required.set(error.reload_required);
                    self.close_after_save.set(false);
                }
                self.error(&error.message);
            }
            _ => self.error("Unexpected session information response"),
        }
        self.controls();
    }
    pub fn gateway_failed(&self) {
        self.cancel.borrow().store(true, Ordering::Release);
        if self.saving() {
            self.reload_required.set(true);
        }
        self.pending.borrow_mut().clear();
        self.generation.set(None);
        self.notice("Gateway stopped; reload session information before saving");
        self.controls();
    }
    pub fn allow_window_close(self: &Rc<Self>) -> bool {
        if self.saving() {
            self.dialog.present(self.parent.upgrade().as_ref());
            self.notice("Wait for session information to finish saving");
            false
        } else if self.dirty.get() {
            self.dialog.present(self.parent.upgrade().as_ref());
            self.confirm(true);
            false
        } else {
            true
        }
    }
    fn confirm(self: &Rc<Self>, close: bool) {
        if self.prompt.replace(true) {
            return;
        }
        let alert = adw::AlertDialog::new(
            Some("Unsaved session information"),
            Some("Save your changes before closing?"),
        );
        for (id, text) in [
            ("cancel", "Cancel"),
            ("discard", "Discard"),
            ("save", "Save"),
        ] {
            alert.add_response(id, text);
            crate::i18n_ui::response(&alert, id, text);
        }
        crate::i18n_ui::bind(&alert, "heading", "Unsaved session information");
        crate::i18n_ui::bind(&alert, "body", "Save your changes before closing?");
        alert.set_close_response("cancel");
        alert.set_default_response(Some("cancel"));
        alert.set_response_enabled("save", self.save.is_sensitive());
        let weak = Rc::downgrade(self);
        alert.choose(
            self.parent.upgrade().as_ref(),
            gtk::gio::Cancellable::NONE,
            move |response| {
                let Some(p) = weak.upgrade() else {
                    return;
                };
                p.prompt.set(false);
                match response.as_str() {
                    "save" => p.save(close),
                    "discard" => {
                        p.dirty.set(false);
                        if close {
                            p.dialog.force_close();
                        } else {
                            p.begin();
                        }
                    }
                    _ => {}
                }
            },
        );
    }
}
