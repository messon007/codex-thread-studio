//! Explicit-save native shared-folder editor. No filesystem I/O on the GTK thread.
use crate::i18n_ui::UiText;
use crate::{
    protocol::Command,
    shared_directories::{self, Request, Snapshot},
};
use adw::prelude::*;
use gtk::gio;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use tokio::sync::mpsc;
#[cfg(debug_assertions)]
#[path = "shared_directories_acceptance.rs"]
mod acceptance;

pub struct Panel {
    pub id: u64,
    pub dialog: adw::PreferencesDialog,
    parent: adw::ApplicationWindow,
    sender: mpsc::Sender<Command>,
    snapshot: RefCell<Option<Snapshot>>,
    paths: RefCell<Vec<String>>,
    rows: RefCell<Vec<adw::ActionRow>>,
    group: adw::PreferencesGroup,
    input: adw::EntryRow,
    add: gtk::Button,
    choose: gtk::Button,
    reload: gtk::Button,
    save: gtk::Button,
    status: gtk::Label,
    busy: Cell<bool>,
    serial: Cell<u64>,
    picker: RefCell<Option<gio::Cancellable>>,
    confirmation: RefCell<Option<adw::AlertDialog>>,
    pub on_applied: RefCell<Option<Box<dyn Fn()>>>,
}
impl Panel {
    pub fn new(parent: &adw::ApplicationWindow, sender: mpsc::Sender<Command>) -> Rc<Self> {
        let dialog = adw::PreferencesDialog::builder()
            .title("Shared folders")
            .content_width(680)
            .content_height(620)
            .build()
            .ui_text("title", "Shared folders");
        let page = adw::PreferencesPage::new();
        let group = adw::PreferencesGroup::builder().title("Folders")
            .description("Folders outside the project are read-only. Unavailable folders remain saved and appear in Files when available. Changes take effect after Save.").build().ui_text("title","Folders").ui_text("description","Folders outside the project are read-only. Unavailable folders remain saved and appear in Files when available. Changes take effect after Save.");
        page.add(&group);
        let entry_group = adw::PreferencesGroup::new();
        let input = adw::EntryRow::builder()
            .title("Absolute folder path")
            .build()
            .ui_text("title", "Absolute folder path");
        let add = gtk::Button::builder()
            .icon_name("list-add-symbolic")
            .tooltip_text("Add folder")
            .valign(gtk::Align::Center)
            .build()
            .ui_text("tooltip-text", "Add folder");
        let choose = gtk::Button::builder()
            .icon_name("folder-open-symbolic")
            .tooltip_text("Choose folder")
            .valign(gtk::Align::Center)
            .build()
            .ui_text("tooltip-text", "Choose folder");
        input.add_suffix(&choose);
        input.add_suffix(&add);
        entry_group.add(&input);
        page.add(&entry_group);
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
        let panel = Rc::new(Self {
            id: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            dialog,
            parent: parent.clone(),
            sender,
            snapshot: RefCell::new(None),
            paths: RefCell::new(vec![]),
            rows: RefCell::new(vec![]),
            group,
            input,
            add,
            choose,
            reload,
            save,
            status,
            busy: Cell::new(false),
            serial: Cell::new(0),
            picker: RefCell::new(None),
            confirmation: RefCell::new(None),
            on_applied: RefCell::new(None),
        });
        let weak = Rc::downgrade(&panel);
        panel.add.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.add_input();
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.input.connect_entry_activated(move |_| {
            if let Some(p) = weak.upgrade() {
                p.add_input();
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.input.connect_changed(move |_| {
            if let Some(p) = weak.upgrade() {
                p.changed();
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.choose.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.choose_folder();
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.save.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.save();
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.reload.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                if p.dirty() {
                    p.confirm(false);
                } else {
                    p.request(Request::Read);
                }
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.dialog.connect_close_attempt(move |_| {
            if let Some(p) = weak.upgrade() {
                if p.busy.get() {
                    p.notice("Wait for the settings request to finish");
                } else if p.dirty() {
                    p.confirm(true);
                } else {
                    p.dialog.force_close();
                }
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.dialog.connect_closed(move |_| {
            if let Some(p) = weak.upgrade() {
                if let Some(c) = p.picker.borrow_mut().take() {
                    c.cancel();
                }
                if let Some(c) = p.confirmation.borrow_mut().take() {
                    c.force_close();
                }
            }
        });
        panel.request(Request::Read);
        panel
    }
    fn dirty(&self) -> bool {
        self.snapshot
            .borrow()
            .as_ref()
            .is_some_and(|s| s.directories != *self.paths.borrow())
            || !self.input.text().is_empty()
    }
    pub fn may_close(&self) -> bool {
        !self.busy.get() && !self.dirty()
    }
    fn notice(&self, text: &str) {
        self.dialog.add_toast(adw::Toast::new(text));
    }
    fn controls(&self) {
        let ready =
            self.snapshot.borrow().is_some() && !self.busy.get() && self.picker.borrow().is_none();
        self.group.set_sensitive(ready);
        self.input.set_sensitive(ready);
        self.add.set_sensitive(
            ready && !self.input.text().is_empty() && self.paths.borrow().len() < 256,
        );
        self.choose
            .set_sensitive(ready && self.paths.borrow().len() < 256);
        // A typed path must be added explicitly so Save never silently omits it.
        self.save
            .set_sensitive(ready && self.dirty() && self.input.text().is_empty());
        self.reload
            .set_sensitive(!self.busy.get() && self.picker.borrow().is_none());
        self.dialog.set_can_close(self.may_close());
    }
    fn changed(&self) {
        self.status.set_text(if self.dirty() {
            "Unsaved changes"
        } else {
            "Saved"
        });
        self.controls();
    }
    fn add_input(self: &Rc<Self>) {
        if self.busy.get() || self.snapshot.borrow().is_none() {
            return;
        }
        let path = self.input.text().to_string();
        if self.add_path(path) {
            self.input.set_text("");
            self.changed();
        }
    }
    fn add_path(self: &Rc<Self>, path: String) -> bool {
        let mut paths = self.paths.borrow().clone();
        if paths.contains(&path) {
            self.notice("This folder is already listed");
            return false;
        }
        paths.push(path);
        if let Err(e) = shared_directories::validate(&paths) {
            self.notice(&e);
            return false;
        }
        self.paths.replace(paths);
        self.render();
        self.changed();
        true
    }
    fn render(self: &Rc<Self>) {
        for row in self.rows.borrow_mut().drain(..) {
            self.group.remove(&row);
        }
        for (index, path) in self.paths.borrow().iter().enumerate() {
            let row = adw::ActionRow::builder().title(path).title_lines(2).build();
            row.set_use_markup(false);
            row.set_tooltip_text(Some(path));
            row.set_subtitle_selectable(true);
            let remove = gtk::Button::builder()
                .icon_name("list-remove-symbolic")
                .tooltip_text("Remove folder")
                .valign(gtk::Align::Center)
                .build()
                .ui_text("tooltip-text", "Remove folder");
            let weak = Rc::downgrade(self);
            remove.connect_clicked(move |_| {
                if let Some(p) = weak.upgrade() {
                    if p.busy.get() || index >= p.paths.borrow().len() {
                        return;
                    }
                    p.paths.borrow_mut().remove(index);
                    p.render();
                    p.changed();
                }
            });
            row.add_suffix(&remove);
            self.group.add(&row);
            self.rows.borrow_mut().push(row);
        }
    }
    fn save(&self) {
        if self.busy.get() || !self.input.text().is_empty() {
            return;
        }
        let Some(s) = self.snapshot.borrow().clone() else {
            return;
        };
        self.request(Request::Save {
            revision: s.revision,
            directories: self.paths.borrow().clone(),
        });
    }
    fn request(&self, request: Request) {
        if self.busy.replace(true) {
            return;
        }
        let serial = self.serial.get() + 1;
        self.serial.set(serial);
        crate::i18n_ui::bind(&self.status, "label", "Updating shared folders…");
        self.controls();
        if self
            .sender
            .try_send(Command::SharedDirectories {
                dialog: self.id,
                serial,
                request,
            })
            .is_err()
        {
            self.busy.set(false);
            crate::i18n_ui::bind(
                &self.status,
                "label",
                "Settings queue is unavailable; retry",
            );
            self.controls();
        }
    }
    pub fn event(self: &Rc<Self>, serial: u64, result: Result<Snapshot, String>) {
        if serial != self.serial.get() {
            return;
        }
        self.busy.set(false);
        match result {
            Ok(s) => {
                self.paths.replace(s.directories.clone());
                self.snapshot.replace(Some(s));
                self.input.set_text("");
                self.render();
                crate::i18n_ui::bind(&self.status, "label", "Saved");
                if let Some(callback) = self.on_applied.borrow().as_ref() {
                    callback();
                }
            }
            Err(e) => self.status.set_text(&e),
        }
        self.controls();
    }
    fn confirm(self: &Rc<Self>, close: bool) {
        if let Some(c) = self.confirmation.borrow().as_ref() {
            c.present(Some(&self.parent));
            return;
        }
        let alert = adw::AlertDialog::new(
            Some("Discard shared-folder changes?"),
            Some("The saved folder list will be kept. Other settings are unaffected."),
        );
        alert.add_response("cancel", "Keep editing");
        alert.add_response("discard", "Discard changes");
        alert.set_close_response("cancel");
        alert.set_default_response(Some("cancel"));
        alert.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
        let weak = Rc::downgrade(self);
        alert.connect_response(None, move |_, response| {
            if let Some(p) = weak.upgrade() {
                p.confirmation.borrow_mut().take();
                if response == "discard" {
                    if close {
                        p.dialog.force_close();
                    } else {
                        p.request(Request::Read);
                    }
                }
            }
        });
        self.confirmation.replace(Some(alert.clone()));
        alert.present(Some(&self.parent));
    }
    fn choose_folder(self: &Rc<Self>) {
        if self.busy.get() || self.picker.borrow().is_some() {
            return;
        }
        let picker = gtk::FileDialog::builder()
            .title("Add shared folder")
            .modal(true)
            .build()
            .ui_text("title", "Add shared folder");
        let cancel = gio::Cancellable::new();
        self.picker.replace(Some(cancel.clone()));
        self.controls();
        let weak = Rc::downgrade(self);
        let request = cancel.clone();
        picker.select_folder(Some(&self.parent), Some(&cancel), move |result| {
            let Some(p) = weak.upgrade() else { return };
            if p.picker.borrow().as_ref() != Some(&request) {
                return;
            }
            p.picker.borrow_mut().take();
            match result {
                Ok(file) => {
                    if let Some(path) = file.path().and_then(|p| p.to_str().map(str::to_owned)) {
                        p.add_path(path);
                    } else {
                        p.notice("Choose a local folder with a UTF-8 path");
                    }
                }
                Err(e)
                    if e.matches(gtk::DialogError::Dismissed)
                        || e.matches(gio::IOErrorEnum::Cancelled) => {}
                Err(e) => p.notice(&e.to_string()),
            }
            p.controls();
        });
    }
}
