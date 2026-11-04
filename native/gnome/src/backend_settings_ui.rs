//! Native JSON configuration editor, explicit validation/save and conflict retention.
use crate::{
    backend_settings::{Reply, Request, Snapshot},
    i18n_ui::UiText,
    protocol::Command,
};
use adw::prelude::*;
use sourceview::prelude::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use tokio::sync::mpsc;
pub struct Panel {
    pub id: u64,
    pub dialog: adw::Dialog,
    sender: mpsc::Sender<Command>,
    buffer: sourceview::Buffer,
    editor: sourceview::View,
    path: gtk::Label,
    status: gtk::Label,
    reload: gtk::Button,
    validate: gtk::Button,
    save: gtk::Button,
    snapshot: RefCell<Option<Snapshot>>,
    pending: Cell<Option<u64>>,
    serial: Cell<u64>,
    updating: Cell<bool>,
    edit_revision: Cell<u64>,
    submitted_revision: Cell<u64>,
    confirmation: RefCell<Option<adw::AlertDialog>>,
    _styles: Rc<crate::content_widgets::Styles>,
    pub on_changed: RefCell<Option<Box<dyn Fn()>>>,
}
impl Panel {
    pub fn new(sender: mpsc::Sender<Command>) -> Rc<Self> {
        let dialog = adw::Dialog::builder()
            .content_width(880)
            .content_height(680)
            .can_close(false)
            .build()
            .ui_text("title", "Backend configuration");
        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        let title = adw::WindowTitle::new("", "")
            .ui_text("title", "Backend configuration")
            .ui_text(
                "subtitle",
                "Restart Studio to use saved backend configuration",
            );
        header.set_title_widget(Some(&title));
        let reload = gtk::Button::builder()
            .icon_name("view-refresh-symbolic")
            .build()
            .ui_text("tooltip-text", "Reload backend configuration");
        let validate = gtk::Button::new().ui_text("label", "Validate");
        let save = gtk::Button::new().ui_text("label", "Save");
        save.add_css_class("suggested-action");
        header.pack_start(&reload);
        header.pack_end(&save);
        header.pack_end(&validate);
        toolbar.add_top_bar(&header);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let path = gtk::Label::builder()
            .selectable(true)
            .xalign(0.0)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .margin_start(16)
            .margin_end(16)
            .margin_top(8)
            .build();
        path.add_css_class("caption");
        body.append(&path);
        let buffer = sourceview::Buffer::new(None::<&gtk::TextTagTable>);
        buffer.set_language(
            sourceview::LanguageManager::default()
                .language("json")
                .as_ref(),
        );
        buffer.set_enable_undo(true);
        let editor = sourceview::View::with_buffer(&buffer);
        editor.add_css_class("studio-code");
        editor.set_monospace(true);
        editor.set_show_line_numbers(true);
        editor.set_auto_indent(true);
        editor.set_indent_width(2);
        editor.set_tab_width(2);
        editor.set_insert_spaces_instead_of_tabs(true);
        editor.set_left_margin(12);
        editor.set_right_margin(12);
        editor.set_top_margin(12);
        editor.set_bottom_margin(12);
        body.append(
            &gtk::ScrolledWindow::builder()
                .vexpand(true)
                .child(&editor)
                .build(),
        );
        let status = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .margin_start(16)
            .margin_end(16)
            .margin_bottom(12)
            .build();
        status.add_css_class("caption");
        body.append(&status);
        toolbar.set_content(Some(&body));
        dialog.set_child(Some(&toolbar));
        let styles = crate::content_widgets::Styles::new();
        styles.watch_buffer(&buffer);
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let panel = Rc::new(Self {
            id: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            dialog,
            sender,
            buffer,
            editor,
            path,
            status,
            reload,
            validate,
            save,
            snapshot: RefCell::new(None),
            pending: Cell::new(None),
            serial: Cell::new(0),
            updating: Cell::new(false),
            edit_revision: Cell::new(0),
            submitted_revision: Cell::new(0),
            confirmation: RefCell::new(None),
            _styles: styles,
            on_changed: RefCell::new(None),
        });
        let weak = Rc::downgrade(&panel);
        panel.buffer.connect_changed(move |_| {
            if let Some(panel) = weak.upgrade().filter(|panel| !panel.updating.get()) {
                panel
                    .edit_revision
                    .set(panel.edit_revision.get().wrapping_add(1));
                panel.status.remove_css_class("error");
                crate::i18n_ui::bind(&panel.status, "label", "Unsaved changes");
                panel.controls();
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.buffer.connect_modified_changed(move |_| {
            if let Some(panel) = weak.upgrade() {
                if !panel.updating.get()
                    && panel.pending.get().is_none()
                    && !panel.buffer.is_modified()
                    && panel.snapshot.borrow().is_some()
                {
                    panel.message("Saved. Restart Studio to apply backend changes.");
                }
                panel.controls();
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.validate.connect_clicked(move |_| {
            if let Some(panel) = weak.upgrade() {
                panel.request(Request::Validate(panel.text()));
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.save.connect_clicked(move |_| {
            if let Some(panel) = weak.upgrade() {
                let snapshot = panel.snapshot.borrow().clone();
                if let Some(snapshot) = snapshot {
                    panel.request(Request::Save {
                        revision: snapshot.revision,
                        source: panel.text(),
                    });
                }
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.reload.connect_clicked(move |_| {
            if let Some(panel) = weak.upgrade() {
                if panel.buffer.is_modified() {
                    let weak = Rc::downgrade(&panel);
                    panel.confirm("Reload backend configuration?", move || {
                        if let Some(panel) = weak.upgrade() {
                            panel.request(Request::Read);
                        }
                    });
                } else {
                    panel.request(Request::Read);
                }
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.dialog.connect_close_attempt(move |_| {
            if let Some(panel) = weak.upgrade() {
                if panel.pending.get().is_some() {
                    panel.message("Wait for the configuration request to finish");
                } else if panel.buffer.is_modified() {
                    let weak = Rc::downgrade(&panel);
                    panel.confirm("Discard backend configuration changes?", move || {
                        if let Some(panel) = weak.upgrade() {
                            panel.dialog.force_close();
                        }
                    });
                } else {
                    panel.dialog.force_close();
                }
            }
        });
        panel.request(Request::Read);
        panel
    }
    fn text(&self) -> String {
        self.buffer
            .text(&self.buffer.start_iter(), &self.buffer.end_iter(), true)
            .to_string()
    }
    pub fn may_close(&self) -> bool {
        self.pending.get().is_none() && !self.buffer.is_modified()
    }
    fn controls(&self) {
        let ready = self.pending.get().is_none();
        self.editor
            .set_editable(ready && self.snapshot.borrow().is_some());
        self.reload.set_sensitive(ready);
        self.validate
            .set_sensitive(ready && self.snapshot.borrow().is_some());
        self.save.set_sensitive(
            ready
                && self
                    .snapshot
                    .borrow()
                    .as_ref()
                    .is_some_and(|snapshot| self.buffer.is_modified() || !snapshot.exists),
        );
        self.dialog.set_can_close(self.may_close());
        if let Some(changed) = self.on_changed.borrow().as_ref() {
            changed();
        }
    }
    fn message(&self, message: &str) {
        crate::i18n_ui::bind(&self.status, "label", message);
    }
    fn confirm(self: &Rc<Self>, heading: &str, action: impl Fn() + 'static) {
        if let Some(dialog) = self.confirmation.borrow().as_ref() {
            dialog.present(Some(&self.dialog));
            return;
        }
        let dialog = adw::AlertDialog::new(None, None)
            .ui_text("heading", heading)
            .ui_text("body", "Unsaved configuration edits will be discarded.");
        dialog.add_responses(&[("cancel", "Keep editing"), ("discard", "Discard")]);
        crate::i18n_ui::response(&dialog, "cancel", "Keep editing");
        crate::i18n_ui::response(&dialog, "discard", "Discard");
        dialog.set_close_response("cancel");
        dialog.set_default_response(Some("cancel"));
        dialog.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
        let weak = Rc::downgrade(self);
        dialog.connect_response(None, move |_, response| {
            if let Some(panel) = weak.upgrade() {
                panel.confirmation.borrow_mut().take();
                if response == "discard" {
                    action();
                }
            }
        });
        dialog.present(Some(&self.dialog));
        self.confirmation.replace(Some(dialog));
    }
    fn request(&self, request: Request) {
        if self.pending.get().is_some() {
            return;
        }
        let serial = self.serial.get().wrapping_add(1);
        self.serial.set(serial);
        self.pending.set(Some(serial));
        self.submitted_revision.set(self.edit_revision.get());
        self.status.remove_css_class("error");
        self.message(match request {
            Request::Read => "Loading backend configuration…",
            Request::Validate(_) => "Validating backend configuration…",
            Request::Save { .. } => "Saving backend configuration…",
        });
        self.controls();
        if self
            .sender
            .try_send(Command::BackendConfig {
                dialog: self.id,
                serial,
                request,
            })
            .is_err()
        {
            self.event(
                serial,
                Err("Backend configuration queue is unavailable".into()),
            );
        }
    }
    pub fn event(&self, serial: u64, reply: Result<Reply, String>) {
        if self.pending.get() != Some(serial) {
            return;
        }
        self.pending.set(None);
        match reply {
            Ok(Reply::Read(snapshot)) => {
                if self.edit_revision.get() != self.submitted_revision.get() {
                    self.message("Configuration was edited while loading. Reload to review the current file.");
                    self.controls();
                    return;
                }
                self.updating.set(true);
                self.buffer.set_enable_undo(false);
                self.buffer.set_text(&snapshot.source);
                self.buffer.set_enable_undo(true);
                self.buffer.place_cursor(&self.buffer.start_iter());
                self.buffer.set_modified(false);
                self.updating.set(false);
                self.path.set_text(&snapshot.path);
                self.check(snapshot.validation_error.as_deref(), snapshot.backend_count);
                self.snapshot.replace(Some(snapshot));
            }
            Ok(Reply::Saved(snapshot)) => {
                self.snapshot.replace(Some(snapshot));
                self.status.remove_css_class("error");
                if self.edit_revision.get() == self.submitted_revision.get() {
                    self.buffer.set_modified(false);
                    self.message("Saved. Restart Studio to apply backend changes.");
                } else {
                    self.message("Configuration saved; newer edits are still unsaved.");
                }
            }
            Ok(Reply::Checked(check)) => {
                if self.edit_revision.get() == self.submitted_revision.get() {
                    self.check(check.validation_error.as_deref(), check.backend_count)
                } else {
                    self.message("Configuration changed since validation. Validate again.");
                }
            }
            Err(error) => {
                self.status.add_css_class("error");
                crate::i18n_ui::literal(&self.status, "label", &error);
            }
        }
        self.controls();
    }
    fn check(&self, error: Option<&str>, count: Option<usize>) {
        if let Some(error) = error {
            self.status.add_css_class("error");
            crate::i18n_ui::literal(&self.status, "label", error);
        } else {
            self.status.remove_css_class("error");
            crate::i18n_ui::bind_format(
                &self.status,
                "label",
                "Valid configuration · {count} enabled backends",
                &[("count", &count.unwrap_or_default().to_string())],
            );
        }
    }
    pub fn gateway_failed(&self, error: &str) {
        self.serial.set(self.serial.get().wrapping_add(1));
        if let Some(serial) = self.pending.get() {
            self.event(serial, Err(error.into()));
        }
    }
}

#[cfg(debug_assertions)]
impl Panel {
    pub fn smoke_ready(&self) -> bool {
        self.pending.get().is_none() && self.snapshot.borrow().is_some()
    }
    pub fn smoke_text(&self) -> String {
        self.text()
    }
    pub fn smoke_status(&self) -> String {
        self.status.text().to_string()
    }
    pub fn smoke_edit(&self, text: &str) {
        self.buffer.begin_user_action();
        let (mut start, mut end) = self.buffer.bounds();
        self.buffer.delete(&mut start, &mut end);
        self.buffer.insert(&mut start, text);
        self.buffer.end_user_action();
    }
    pub fn smoke_validate(&self) {
        self.validate.emit_clicked();
    }
    pub fn smoke_save(&self) {
        self.save.emit_clicked();
    }
    pub fn smoke_reload(&self) {
        self.reload.emit_clicked();
    }
    pub fn smoke_confirm(&self, response: &str) -> Result<(), String> {
        let dialog = self
            .confirmation
            .borrow()
            .clone()
            .ok_or("Configuration confirmation missing")?;
        fn find(widget: &gtk::Widget, label: &str) -> Option<gtk::Button> {
            if let Some(button) = widget
                .downcast_ref::<gtk::Button>()
                .filter(|button| button.label().as_deref() == Some(label))
            {
                return Some(button.clone());
            }
            let mut child = widget.first_child();
            while let Some(current) = child {
                child = current.next_sibling();
                if let Some(button) = find(&current, label) {
                    return Some(button);
                }
            }
            None
        }
        let label = crate::language::tr(if response == "discard" {
            "Discard"
        } else {
            "Keep editing"
        });
        find(dialog.upcast_ref(), &label)
            .ok_or("Configuration confirmation action missing")?
            .emit_clicked();
        Ok(())
    }
    pub fn smoke_undo(&self) {
        self.buffer.undo();
    }
    pub fn smoke_redo(&self) {
        self.buffer.redo();
    }
    pub fn smoke_scheme(&self) -> Option<String> {
        self.buffer
            .style_scheme()
            .map(|scheme| scheme.id().to_string())
    }
}
