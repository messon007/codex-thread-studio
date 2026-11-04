//! Native controls for shared directory visibility rules; explicit persistence.
use crate::i18n_ui::UiText;
use crate::{
    protocol::Command,
    session_visibility::{Editor, Reply, Request, Snapshot, Values},
};
use adw::prelude::*;
use gtk::gio;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use tokio::sync::mpsc;
#[cfg(debug_assertions)]
#[path = "session_visibility_acceptance.rs"]
mod acceptance;
pub struct Panel {
    pub id: u64,
    pub dialog: adw::PreferencesDialog,
    parent: adw::ApplicationWindow,
    sender: mpsc::Sender<Command>,
    hidden: gtk::TextBuffer,
    rules: gtk::TextBuffer,
    form: gtk::Box,
    fields: [Field; 2],
    editor: RefCell<Editor>,
    replaced: [Cell<bool>; 2],
    modified: [Cell<bool>; 2],
    reload: gtk::Button,
    save: gtk::Button,
    choose: gtk::Button,
    status: gtk::Label,
    settled_status: RefCell<String>,
    snapshot: RefCell<Option<Snapshot>>,
    busy: Cell<bool>,
    updating: Cell<bool>,
    serial: Cell<u64>,
    confirmation: RefCell<Option<adw::AlertDialog>>,
    picker: RefCell<Option<gio::Cancellable>>,
    pub on_applied: RefCell<Option<Box<dyn Fn(Reply)>>>,
}
fn text(buffer: &gtk::TextBuffer) -> String {
    buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), true)
        .to_string()
}
struct Field {
    group: adw::PreferencesGroup,
    view: gtk::TextView,
    scroll: gtk::ScrolledWindow,
    repair: gtk::ListBox,
    replace: gtk::Button,
}
fn editor(title: &str, description: &str) -> Field {
    let group = adw::PreferencesGroup::builder()
        .title(title)
        .description(description)
        .build();
    crate::i18n_ui::bind(&group, "title", title);
    crate::i18n_ui::bind(&group, "description", description);
    let view = gtk::TextView::builder()
        .monospace(true)
        .wrap_mode(gtk::WrapMode::None)
        .top_margin(10)
        .bottom_margin(10)
        .left_margin(12)
        .right_margin(12)
        .accepts_tab(false)
        .build();
    view.update_property(&[gtk::accessible::Property::Label(title)]);
    let scroll = gtk::ScrolledWindow::builder()
        .min_content_height(120)
        .max_content_height(220)
        .propagate_natural_height(true)
        .child(&view)
        .build();
    scroll.add_css_class("card");
    scroll.set_overflow(gtk::Overflow::Hidden);
    let repair_row=adw::ActionRow::builder().title("Stored value needs replacement")
        .subtitle("This value cannot safely appear as editable lines. Replace it in memory, then Save to apply.").build().ui_text("title","Stored value needs replacement").ui_text("subtitle","This value cannot safely appear as editable lines. Replace it in memory, then Save to apply.");
    let replace = gtk::Button::builder()
        .label("Replace…")
        .valign(gtk::Align::Center)
        .build()
        .ui_text("label", "Replace…");
    repair_row.add_suffix(&replace);
    let repair = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .build();
    repair.add_css_class("boxed-list");
    repair.append(&repair_row);
    repair.set_visible(false);
    // Keep the alternatives in one custom group child. An invisible preference
    // row otherwise leaves an empty internal list shadow across the text editor.
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&scroll);
    content.append(&repair);
    group.add(&content);
    Field {
        group,
        view,
        scroll,
        repair,
        replace,
    }
}
impl Panel {
    pub fn new(parent: &adw::ApplicationWindow, sender: mpsc::Sender<Command>) -> Rc<Self> {
        let dialog = adw::PreferencesDialog::builder()
            .title("Session visibility")
            .content_width(700)
            .content_height(710)
            .build()
            .ui_text("title", "Session visibility");
        let page = adw::PreferencesPage::new();
        let form = gtk::Box::new(gtk::Orientation::Vertical, 24);
        let hidden_field=editor("Hidden folders","One folder per line. Hides the folder and its descendants from the session list and Router candidates; this does not delete sessions or change file access.");
        let hidden = hidden_field.view.buffer();
        let choose = gtk::Button::builder()
            .icon_name("folder-open-symbolic")
            .tooltip_text("Add hidden folder")
            .valign(gtk::Align::Center)
            .build()
            .ui_text("tooltip-text", "Add hidden folder");
        hidden_field.group.set_header_suffix(Some(&choose));
        let rules_field=editor("Directory ignore rules","One rule per line: *, ?, ** and [a-z] globs. # starts a comment; ! allows a match. Later matching rules win, including over hidden folders. Example: **/build/ then !/work/build/keep/.");
        let rules = rules_field.view.buffer();
        form.append(&hidden_field.group);
        form.append(&rules_field.group);
        let container = adw::PreferencesGroup::new();
        container.add(&form);
        page.add(&container);
        let actions = adw::PreferencesGroup::new();
        let row = adw::ActionRow::new();
        let status = gtk::Label::builder()
            .wrap(true)
            .xalign(0.0)
            .hexpand(true)
            .build();
        row.add_prefix(&status);
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        buttons.set_valign(gtk::Align::Center);
        let reload = gtk::Button::with_label("Reload").ui_text("label", "Reload");
        let save = gtk::Button::with_label("Save").ui_text("label", "Save");
        save.add_css_class("suggested-action");
        buttons.append(&reload);
        buttons.append(&save);
        row.add_suffix(&buttons);
        actions.add(&row);
        page.add(&actions);
        dialog.add(&page);
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let ui = Rc::new(Self {
            id: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            dialog,
            parent: parent.clone(),
            sender,
            hidden,
            rules,
            form,
            fields: [hidden_field, rules_field],
            editor: RefCell::new(Editor::default()),
            replaced: [Cell::new(false), Cell::new(false)],
            modified: [Cell::new(false), Cell::new(false)],
            reload,
            save,
            choose,
            status,
            settled_status: RefCell::new(String::new()),
            snapshot: RefCell::new(None),
            busy: Cell::new(false),
            updating: Cell::new(false),
            serial: Cell::new(0),
            confirmation: RefCell::new(None),
            picker: RefCell::new(None),
            on_applied: RefCell::new(None),
        });
        for (index, buffer) in [&ui.hidden, &ui.rules].into_iter().enumerate() {
            let weak = Rc::downgrade(&ui);
            buffer.connect_changed(move |_| {
                if let Some(ui) = weak.upgrade() {
                    if !ui.updating.get() {
                        let original = ui.editor.borrow();
                        let original = if index == 0 {
                            original.hidden.as_deref()
                        } else {
                            original.rules.as_deref()
                        };
                        ui.modified[index].set(
                            text(&if index == 0 {
                                ui.hidden.clone()
                            } else {
                                ui.rules.clone()
                            }) != original.unwrap_or(""),
                        );
                        if ui.dirty() {
                            crate::i18n_ui::bind(&ui.status, "label", "Unsaved changes");
                        } else {
                            ui.status.set_text(&ui.settled_status.borrow());
                        }
                        ui.controls();
                    }
                }
            });
        }
        for index in 0..2 {
            let weak = Rc::downgrade(&ui);
            ui.fields[index].replace.connect_clicked(move |_| {
                if let Some(ui) = weak.upgrade() {
                    ui.confirm_replace(index);
                }
            });
        }
        let weak = Rc::downgrade(&ui);
        ui.save.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.save();
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.reload.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                if ui.dirty() {
                    ui.confirm(false);
                } else {
                    ui.request(Request::Read);
                }
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.choose.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.choose_folder();
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.dialog.connect_close_attempt(move |_| {
            if let Some(ui) = weak.upgrade() {
                if ui.busy.get() {
                    ui.dialog.add_toast(adw::Toast::new(&crate::language::tr(
                        "Wait for the settings request to finish",
                    )));
                } else if ui.dirty() {
                    ui.confirm(true);
                } else {
                    ui.dialog.force_close();
                }
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.dialog.connect_closed(move |_| {
            if let Some(ui) = weak.upgrade() {
                let picker = ui.picker.borrow_mut().take();
                if let Some(c) = picker {
                    c.cancel();
                }
                let confirmation = ui.confirmation.borrow_mut().take();
                if let Some(c) = confirmation {
                    c.force_close();
                }
            }
        });
        ui.request(Request::Read);
        ui
    }
    fn values(&self) -> Option<Values> {
        if !(self.available(0) && self.available(1)) {
            return None;
        }
        let snapshot = self.snapshot.borrow();
        let snapshot = snapshot.as_ref()?;
        let unchanged = |index: usize, key: &str| {
            if !self.modified[index].get() && !self.replaced[index].get() {
                serde_json::from_value::<Vec<String>>(snapshot.values[key].clone()).ok()
            } else {
                None
            }
        };
        Some(Values {
            hidden_session_directories: unchanged(0, "hiddenSessionDirectories").unwrap_or_else(
                || {
                    text(&self.hidden)
                        .split('\n')
                        .filter(|p| !p.trim().is_empty())
                        .map(str::to_owned)
                        .collect()
                },
            ),
            session_directory_ignore: unchanged(1, "sessionDirectoryIgnore").unwrap_or_else(|| {
                if text(&self.rules).is_empty() {
                    vec![]
                } else {
                    text(&self.rules).split('\n').map(str::to_owned).collect()
                }
            }),
        })
    }
    fn dirty(&self) -> bool {
        self.replaced.iter().chain(&self.modified).any(Cell::get)
    }
    fn available(&self, index: usize) -> bool {
        self.replaced[index].get()
            || if index == 0 {
                self.editor.borrow().hidden.is_some()
            } else {
                self.editor.borrow().rules.is_some()
            }
    }
    pub fn may_close(&self) -> bool {
        !self.busy.get() && !self.dirty()
    }
    fn controls(&self) {
        let ready =
            self.snapshot.borrow().is_some() && !self.busy.get() && self.picker.borrow().is_none();
        self.form.set_sensitive(ready);
        for (index, field) in self.fields.iter().enumerate() {
            let available = self.available(index);
            field.scroll.set_visible(available);
            field
                .repair
                .set_visible(!available && self.snapshot.borrow().is_some());
            field.view.set_sensitive(ready && available);
        }
        self.choose.set_sensitive(ready && self.available(0));
        self.save
            .set_sensitive(ready && self.dirty() && self.available(0) && self.available(1));
        self.reload
            .set_sensitive(!self.busy.get() && self.picker.borrow().is_none());
        self.dialog.set_can_close(self.may_close());
    }
    fn save(&self) {
        if self.busy.get() {
            return;
        }
        let Some(s) = self.snapshot.borrow().clone() else {
            return;
        };
        let Some(values) = self.values() else {
            return;
        };
        self.request(Request::Save {
            revision: s.revision,
            values,
        });
    }
    fn request(&self, request: Request) {
        if self.busy.replace(true) {
            return;
        }
        let serial = self.serial.get() + 1;
        self.serial.set(serial);
        crate::i18n_ui::bind(&self.status, "label", "Updating session visibility…");
        self.controls();
        if self
            .sender
            .try_send(Command::SessionVisibility {
                dialog: self.id,
                serial,
                request,
            })
            .is_err()
        {
            self.busy.set(false);
            self.settled_status
                .replace("Settings queue is unavailable; retry".into());
            self.status.set_text(&self.settled_status.borrow());
            self.controls();
        }
    }
    pub fn event(self: &Rc<Self>, serial: u64, result: Result<Reply, String>) {
        if serial != self.serial.get() {
            return;
        }
        self.busy.set(false);
        match result {
            Ok(reply) => {
                self.updating.set(true);
                self.hidden
                    .set_text(reply.editor.hidden.as_deref().unwrap_or(""));
                self.rules
                    .set_text(reply.editor.rules.as_deref().unwrap_or(""));
                self.editor.replace(reply.editor.clone());
                for cell in self.replaced.iter().chain(&self.modified) {
                    cell.set(false);
                }
                self.snapshot.replace(Some(reply.snapshot.clone()));
                self.updating.set(false);
                self.settled_status.replace(match reply.filter.as_ref(){Ok(_)=>"Saved".into(),Err(error)=>format!("Saved rules cannot be applied: {error}. Edit or replace the affected values, then Save.")});
                self.status.set_text(&self.settled_status.borrow());
                if let Some(callback) = self.on_applied.borrow().as_ref() {
                    callback(reply);
                }
            }
            Err(e) => {
                self.settled_status.replace(e);
                self.status.set_text(&self.settled_status.borrow());
            }
        }
        self.controls();
    }
    fn confirm(self: &Rc<Self>, close: bool) {
        if let Some(c) = self.confirmation.borrow().as_ref() {
            c.present(Some(&self.parent));
            return;
        }
        let alert = adw::AlertDialog::new(
            Some("Discard visibility changes?"),
            Some("Saved rules will be kept. Sessions and files are unaffected."),
        );
        alert.add_response("cancel", "Keep editing");
        alert.add_response("discard", "Discard changes");
        alert.set_close_response("cancel");
        alert.set_default_response(Some("cancel"));
        alert.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
        let weak = Rc::downgrade(self);
        alert.connect_response(None, move |_, response| {
            if let Some(ui) = weak.upgrade() {
                ui.confirmation.borrow_mut().take();
                if response == "discard" {
                    if close {
                        ui.dialog.force_close();
                    } else {
                        ui.request(Request::Read);
                    }
                }
            }
        });
        self.confirmation.replace(Some(alert.clone()));
        alert.present(Some(&self.parent));
    }
    fn confirm_replace(self: &Rc<Self>, index: usize) {
        if self.busy.get() || self.available(index) {
            return;
        }
        if let Some(alert) = self.confirmation.borrow().as_ref() {
            alert.present(Some(&self.parent));
            return;
        }
        let alert=adw::AlertDialog::new(Some(if index==0{"Replace stored hidden folders?"}else{"Replace stored ignore rules?"}),
            Some("This clears this field in memory. Save applies the replacement; Reload keeps the original stored value. Other settings are preserved."));
        alert.add_response("cancel", "Keep original");
        alert.add_response("replace", "Replace value");
        alert.set_close_response("cancel");
        alert.set_default_response(Some("cancel"));
        alert.set_response_appearance("replace", adw::ResponseAppearance::Destructive);
        let weak = Rc::downgrade(self);
        alert.connect_response(None, move |_, response| {
            if let Some(ui) = weak.upgrade() {
                ui.confirmation.borrow_mut().take();
                if response == "replace" {
                    ui.replaced[index].set(true);
                    if index == 0 {
                        ui.hidden.set_text("");
                    } else {
                        ui.rules.set_text("");
                    }
                    crate::i18n_ui::bind(&ui.status, "label", "Unsaved changes");
                    ui.controls();
                }
            }
        });
        self.confirmation.replace(Some(alert.clone()));
        alert.present(Some(&self.parent));
    }
    fn choose_folder(self: &Rc<Self>) {
        if self.busy.get() || self.picker.borrow().is_some() || !self.available(0) {
            return;
        }
        let picker = gtk::FileDialog::builder()
            .title("Hide sessions from folder")
            .modal(true)
            .build()
            .ui_text("title", "Hide sessions from folder");
        let cancel = gio::Cancellable::new();
        self.picker.replace(Some(cancel.clone()));
        self.controls();
        let weak = Rc::downgrade(self);
        picker.select_folder(Some(&self.parent), Some(&cancel), move |result| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            if ui.picker.borrow_mut().take().is_none() {
                return;
            }
            match result {
                Ok(file) => {
                    if let Some(path) = file.path().and_then(|p| p.to_str().map(str::to_owned)) {
                        if let Err(error) =
                            crate::session_directory_policy::validate(&[path.clone()], &[])
                        {
                            ui.dialog.add_toast(adw::Toast::new(&error));
                            ui.controls();
                            return;
                        }
                        let mut value = text(&ui.hidden);
                        if !value.is_empty() && !value.ends_with('\n') {
                            value.push('\n');
                        }
                        value.push_str(&path);
                        ui.hidden.set_text(&value);
                    } else {
                        ui.dialog.add_toast(adw::Toast::new(&crate::language::tr(
                            "Choose a local folder with a UTF-8 path",
                        )));
                    }
                }
                Err(e)
                    if e.matches(gtk::DialogError::Dismissed)
                        || e.matches(gio::IOErrorEnum::Cancelled) => {}
                Err(e) => ui.dialog.add_toast(adw::Toast::new(&e.to_string())),
            }
            ui.controls();
        });
    }
}
