use crate::i18n_ui::UiText;
use crate::message_queue::{Entry, Message};
use adw::prelude::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
pub struct Panel {
    pub widget: adw::Clamp,
    body: gtk::Box,
    title: gtk::Label,
    status: gtk::Label,
    toggle: gtk::Button,
    current: RefCell<String>,
    revision: Cell<u64>,
    pub editor: RefCell<Option<Rc<Editor>>>,
    pub on_toggle: RefCell<Option<Box<dyn Fn(String)>>>,
    pub on_edit: RefCell<Option<Box<dyn Fn(String, String)>>>,
    pub on_remove: RefCell<Option<Box<dyn Fn(String, String)>>>,
    pub on_save: RefCell<Option<Box<dyn Fn(String, String, String) -> Result<(), String>>>>,
    pub on_close: RefCell<Option<Box<dyn Fn(String)>>>,
}
pub struct Editor {
    pub dialog: adw::Dialog,
    pub key: String,
    pub id: String,
    text: gtk::TextView,
    save: gtk::Button,
    status: gtk::Label,
}
impl Panel {
    #[cfg(debug_assertions)]
    pub async fn smoke_failed_save(
        &self,
        break_store: impl FnOnce() -> Result<(), String>,
        restore_store: impl FnOnce() -> Result<(), String>,
    ) -> Result<(), String> {
        let button = self
            .body
            .first_child()
            .and_then(|row| row.first_child())
            .and_then(|label| label.next_sibling())
            .and_downcast::<gtk::Button>()
            .ok_or("Queue edit button missing")?;
        button.emit_clicked();
        let editor = self.editor.borrow().clone().ok_or("Queue editor missing")?;
        editor
            .text
            .buffer()
            .set_text("failed edit must be retained");
        break_store()?;
        editor.save.emit_clicked();
        let result = async {
            let start = std::time::Instant::now();
            while !editor.save.is_sensitive() {
                if start.elapsed() > std::time::Duration::from_secs(5) {
                    return Err("Failed queue write did not unlock the editor".to_owned());
                }
                gtk::glib::timeout_future(std::time::Duration::from_millis(25)).await;
            }
            let buffer = editor.text.buffer();
            if buffer.text(&buffer.start_iter(), &buffer.end_iter(), false)
                != "failed edit must be retained"
                || self.editor.borrow().is_none()
            {
                return Err("Failed queue write discarded the editor text".into());
            }
            Ok(())
        }
        .await;
        let restored = restore_store();
        result?;
        restored?;
        editor.text.buffer().set_text("native-queue-one");
        editor.save.emit_clicked();
        let start = std::time::Instant::now();
        while self.editor.borrow().is_some() {
            if start.elapsed() > std::time::Duration::from_secs(5) {
                return Err("Queue edit could not be saved after storage recovered".into());
            }
            gtk::glib::timeout_future(std::time::Duration::from_millis(25)).await;
        }
        Ok(())
    }
    pub fn new() -> Rc<Self> {
        let box_ = gtk::Box::new(gtk::Orientation::Vertical, 4);
        box_.add_css_class("card");
        box_.set_margin_top(8);
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        bar.set_margin_start(12);
        bar.set_margin_end(6);
        bar.set_margin_top(6);
        let title = gtk::Label::builder()
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .width_chars(1)
            .build();
        title.add_css_class("heading");
        bar.append(&title);
        let toggle = gtk::Button::with_label("Pause").ui_text("label", "Pause");
        toggle.add_css_class("flat");
        bar.append(&toggle);
        box_.append(&bar);
        let status = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .margin_start(12)
            .margin_end(12)
            .build();
        status.add_css_class("caption");
        box_.append(&status);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        box_.append(&body);
        let widget = adw::Clamp::builder()
            .maximum_size(1000)
            .tightening_threshold(800)
            .child(&box_)
            .margin_start(24)
            .margin_end(24)
            .visible(false)
            .build();
        crate::appearance_ui::watch_clamp(&widget);
        let ui = Rc::new(Self {
            widget,
            body,
            title,
            status,
            toggle,
            current: RefCell::new(String::new()),
            revision: Cell::new(u64::MAX),
            editor: RefCell::new(None),
            on_toggle: RefCell::new(None),
            on_edit: RefCell::new(None),
            on_remove: RefCell::new(None),
            on_save: RefCell::new(None),
            on_close: RefCell::new(None),
        });
        let weak = Rc::downgrade(&ui);
        ui.toggle.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                if let Some(callback) = ui.on_toggle.borrow().as_ref() {
                    let key = ui.current.borrow().clone();
                    callback(key);
                }
            }
        });
        ui
    }
    pub fn render(self: &Rc<Self>, key: &str, entry: Option<&Entry>) {
        let Some(entry) = entry else {
            self.widget.set_visible(false);
            self.current.replace(key.into());
            self.revision.set(u64::MAX);
            return;
        };
        if self.current.borrow().as_str() == key && self.revision.get() == entry.revision {
            return;
        }
        self.current.replace(key.into());
        self.revision.set(entry.revision);
        self.widget.set_visible(
            !entry.messages.is_empty() || entry.saving.is_some() || entry.error.is_some(),
        );
        crate::i18n_ui::bind_format(
            &self.title,
            "label",
            "Queued messages · {count}/{limit}",
            &[
                ("count", &entry.messages.len().to_string()),
                ("limit", &entry.limit.to_string()),
            ],
        );
        crate::i18n_ui::bind(
            &self.toggle,
            "label",
            if !entry.loaded {
                "Reload"
            } else if entry.paused {
                "Resume"
            } else {
                "Pause"
            },
        );
        self.toggle.set_sensitive(entry.saving.is_none());
        if let Some(error) = &entry.error {
            if error == crate::session_release::QUEUE_PAUSE {
                crate::i18n_ui::bind(&self.status, "label", crate::session_release::QUEUE_PAUSE);
            } else {
                crate::i18n_ui::literal(&self.status, "label", error);
            }
        } else {
            crate::i18n_ui::bind(
                &self.status,
                "label",
                if entry.saving.is_some() {
                    "Saving queue…"
                } else if entry.paused {
                    "Paused"
                } else if entry.preparing.is_some() {
                    "Preparing next message…"
                } else {
                    "Sent after the current turn completes"
                },
            );
        }
        while let Some(child) = self.body.first_child() {
            self.body.remove(&child);
        }
        for message in &entry.messages {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            row.set_margin_start(12);
            row.set_margin_end(6);
            row.set_margin_top(4);
            row.set_margin_bottom(4);
            let snippet = message
                .text
                .chars()
                .take(240)
                .map(|ch| if ch == '\n' || ch == '\r' { ' ' } else { ch })
                .collect::<String>();
            let label = gtk::Label::builder()
                .label(if snippet.is_empty() {
                    "Attached references"
                } else {
                    &snippet
                })
                .xalign(0.0)
                .wrap(true)
                .lines(2)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .width_chars(1)
                .hexpand(true)
                .build();
            if snippet.is_empty() {
                crate::i18n_ui::bind(&label, "label", "Attached references");
            }
            row.append(&label);
            if !message.input.is_empty() {
                let refs = gtk::Label::new(None);
                crate::i18n_ui::bind_format(
                    &refs,
                    "label",
                    "{count} refs",
                    &[("count", &message.input.len().to_string())],
                );
                let names = message
                    .input
                    .iter()
                    .map(|reference| {
                        reference["name"]
                            .as_str()
                            .or_else(|| reference["path"].as_str())
                            .unwrap_or("Reference")
                            .chars()
                            .take(160)
                            .collect::<String>()
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                refs.set_tooltip_text(Some(&names));
                refs.add_css_class("caption");
                row.append(&refs);
            }
            for (icon, tooltip, edit) in [
                ("document-edit-symbolic", "Edit queued message", true),
                ("user-trash-symbolic", "Remove queued message", false),
            ] {
                let button = gtk::Button::builder()
                    .icon_name(icon)
                    .tooltip_text(tooltip)
                    .sensitive(!entry.busy())
                    .build();
                crate::i18n_ui::bind(&button, "tooltip-text", tooltip);
                button.add_css_class("flat");
                row.append(&button);
                let weak = Rc::downgrade(self);
                let key = key.to_owned();
                let id = message.id.clone();
                button.connect_clicked(move |_| {
                    if let Some(ui) = weak.upgrade() {
                        let handler = if edit {
                            ui.on_edit.borrow()
                        } else {
                            ui.on_remove.borrow()
                        };
                        if let Some(callback) = handler.as_ref() {
                            callback(key.clone(), id.clone());
                        }
                    }
                });
            }
            self.body.append(&row);
        }
    }
    pub fn edit(self: &Rc<Self>, parent: &adw::ApplicationWindow, key: &str, message: &Message) {
        let dialog = adw::Dialog::builder()
            .title("Edit queued message")
            .content_width(680)
            .content_height(420)
            .build()
            .ui_text("title", "Edit queued message");
        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(
            &adw::WindowTitle::new("Edit queued message", "References are retained")
                .ui_text("title", "Edit queued message")
                .ui_text("subtitle", "References are retained"),
        ));
        let save = gtk::Button::with_label("Save").ui_text("label", "Save");
        save.add_css_class("suggested-action");
        header.pack_end(&save);
        toolbar.add_top_bar(&header);
        let text = gtk::TextView::builder()
            .wrap_mode(gtk::WrapMode::WordChar)
            .left_margin(16)
            .right_margin(16)
            .top_margin(12)
            .bottom_margin(12)
            .build();
        text.buffer().set_text(&message.text);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        body.append(
            &gtk::ScrolledWindow::builder()
                .child(&text)
                .vexpand(true)
                .hscrollbar_policy(gtk::PolicyType::Never)
                .build(),
        );
        let status = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .margin_start(16)
            .margin_end(16)
            .margin_bottom(12)
            .build();
        body.append(&status);
        toolbar.set_content(Some(&body));
        dialog.set_child(Some(&toolbar));
        let editor = Rc::new(Editor {
            dialog,
            key: key.into(),
            id: message.id.clone(),
            text,
            save,
            status,
        });
        let weak = Rc::downgrade(self);
        let value = Rc::downgrade(&editor);
        editor.save.connect_clicked(move |_| {
            if let (Some(ui), Some(value)) = (weak.upgrade(), value.upgrade()) {
                let buffer = value.text.buffer();
                let text = buffer
                    .text(&buffer.start_iter(), &buffer.end_iter(), false)
                    .to_string();
                if let Some(callback) = ui.on_save.borrow().as_ref() {
                    match callback(value.key.clone(), value.id.clone(), text) {
                        Ok(()) => {
                            value.save.set_sensitive(false);
                            value.dialog.set_can_close(false);
                            crate::i18n_ui::bind(&value.status, "label", "Saving queue…");
                        }
                        Err(error) => value.status.set_text(&error),
                    }
                }
            }
        });
        let weak = Rc::downgrade(self);
        let key = key.to_owned();
        editor.dialog.connect_closed(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.editor.borrow_mut().take();
                if let Some(callback) = ui.on_close.borrow().as_ref() {
                    callback(key.clone());
                }
            }
        });
        self.editor.replace(Some(editor.clone()));
        editor.dialog.present(Some(parent));
        editor.text.grab_focus();
    }
    pub fn saved(&self, key: &str, result: Result<(), &str>) {
        let editor = self.editor.borrow().clone();
        if let Some(editor) = editor.filter(|editor| editor.key == key) {
            editor.dialog.set_can_close(true);
            editor.save.set_sensitive(true);
            match result {
                Ok(()) => {
                    editor.dialog.close();
                }
                Err(error) => editor.status.set_text(error),
            }
        }
    }
}

#[cfg(debug_assertions)]
impl Panel {
    pub async fn smoke_edit(
        &self,
        text: &str,
        capture: impl Fn(&adw::Dialog),
    ) -> Result<(), String> {
        fn button(widget: &gtk::Widget, tooltip: &str) -> Option<gtk::Button> {
            if let Some(button) = widget.downcast_ref::<gtk::Button>() {
                if button.tooltip_text().as_deref() == Some(tooltip) {
                    return Some(button.clone());
                }
            }
            let mut child = widget.first_child();
            while let Some(widget) = child {
                child = widget.next_sibling();
                if let Some(found) = button(&widget, tooltip) {
                    return Some(found);
                }
            }
            None
        }
        button(
            self.body.upcast_ref(),
            &crate::language::tr("Edit queued message"),
        )
        .ok_or("Queue edit control missing")?
        .emit_clicked();
        let editor = self
            .editor
            .borrow()
            .clone()
            .ok_or("Queue editor did not open")?;
        editor.text.buffer().set_text(text);
        crate::window_capture::ready(&editor.dialog.child().ok_or("Queue editor has no content")?)
            .await?;
        capture(&editor.dialog);
        editor.save.emit_clicked();
        let start = std::time::Instant::now();
        while self.editor.borrow().is_some() {
            if start.elapsed() > std::time::Duration::from_secs(5) {
                return Err(format!("Queue edit did not save: {}", editor.status.text()));
            }
            gtk::glib::timeout_future(std::time::Duration::from_millis(25)).await;
        }
        Ok(())
    }
    pub fn smoke_toggle(&self) {
        self.toggle.emit_clicked();
    }
    pub fn smoke_remove_last(&self) -> Result<(), String> {
        self.body
            .last_child()
            .and_then(|row| row.last_child())
            .and_downcast::<gtk::Button>()
            .ok_or("Queue removal control missing")?
            .emit_clicked();
        Ok(())
    }
}
