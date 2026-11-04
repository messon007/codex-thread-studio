//! App-wide locale and scoped, explicit preference controls.
use crate::{
    language::{Request, Snapshot},
    protocol::Command,
};
use adw::prelude::*;
use gtk::glib;
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};
use tokio::sync::mpsc;
struct Pending {
    serial: u64,
    save: bool,
    owner: Weak<Binding>,
}
pub struct Runtime {
    sender: mpsc::Sender<Command>,
    snapshot: RefCell<Option<Snapshot>>,
    serial: Cell<u64>,
    pending: RefCell<Option<Pending>>,
    error: RefCell<String>,
    bindings: RefCell<Vec<Weak<Binding>>>,
    feedback: Rc<crate::write_feedback::Queue>,
    pub on_changed: RefCell<Option<Box<dyn Fn()>>>,
}
pub struct Binding {
    pub group: adw::PreferencesGroup,
    row: adw::ComboRow,
    model: gtk::StringList,
    save: gtk::Button,
    reload: gtk::Button,
    status: gtk::Label,
    dirty: Cell<bool>,
    updating: Cell<bool>,
    saving: Cell<bool>,
    pub on_changed: RefCell<Option<Box<dyn Fn()>>>,
}
impl Runtime {
    pub fn new(
        sender: mpsc::Sender<Command>,
        feedback: Rc<crate::write_feedback::Queue>,
    ) -> Rc<Self> {
        Rc::new(Self {
            sender,
            feedback,
            snapshot: RefCell::default(),
            serial: Cell::new(0),
            pending: RefCell::default(),
            error: RefCell::default(),
            bindings: RefCell::default(),
            on_changed: RefCell::default(),
        })
    }
    pub fn bind(self: &Rc<Self>) -> Rc<Binding> {
        let group = adw::PreferencesGroup::new();
        crate::i18n_ui::bind(&group, "title", "Interface language");
        let model = gtk::StringList::new(&[
            &crate::language::tr("Use system language"),
            "简体中文",
            "English",
        ]);
        let row = adw::ComboRow::builder().model(&model).build();
        crate::i18n_ui::bind(&row, "title", "Language");
        group.add(&row);
        let save = gtk::Button::new();
        crate::i18n_ui::bind(&save, "label", "Apply language");
        let reload = gtk::Button::builder()
            .icon_name("view-refresh-symbolic")
            .build();
        crate::i18n_ui::bind(&reload, "tooltip-text", "Reload language");
        let controls = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        controls.append(&reload);
        controls.append(&save);
        group.set_header_suffix(Some(&controls));
        let status = gtk::Label::builder().xalign(0.0).wrap(true).build();
        status.add_css_class("caption");
        group.add(&status);
        let binding = Rc::new(Binding {
            group,
            row,
            model,
            save,
            reload,
            status,
            dirty: Cell::new(false),
            updating: Cell::new(false),
            saving: Cell::new(false),
            on_changed: RefCell::default(),
        });
        let weak = Rc::downgrade(&binding);
        let runtime = Rc::downgrade(self);
        binding.row.connect_selected_notify(move |_| {
            if let (Some(binding), Some(runtime)) = (weak.upgrade(), runtime.upgrade()) {
                if !binding.updating.get() {
                    binding.dirty.set(true);
                    runtime.refresh(&binding, false);
                }
            }
        });
        let weak = Rc::downgrade(&binding);
        let runtime = Rc::downgrade(self);
        binding.save.connect_clicked(move |_| {
            if let (Some(binding), Some(runtime)) = (weak.upgrade(), runtime.upgrade()) {
                runtime.request(Some(&binding), true);
            }
        });
        let weak = Rc::downgrade(&binding);
        let runtime = Rc::downgrade(self);
        binding.reload.connect_clicked(move |_| {
            if let (Some(binding), Some(runtime)) = (weak.upgrade(), runtime.upgrade()) {
                runtime.request(Some(&binding), false);
            }
        });
        self.bindings.borrow_mut().retain(|b| b.strong_count() > 0);
        self.bindings.borrow_mut().push(Rc::downgrade(&binding));
        self.refresh(&binding, true);
        binding
    }
    fn request(&self, binding: Option<&Rc<Binding>>, save: bool) {
        if self.pending.borrow().is_some() {
            return;
        }
        let request = if save {
            let Some(snapshot) = self.snapshot.borrow().clone() else {
                return;
            };
            let Some(binding) = binding else {
                return;
            };
            Request::Save {
                revision: snapshot.revision,
                language: match binding.row.selected() {
                    1 => "zh-CN",
                    2 => "en-US",
                    _ => "system",
                }
                .into(),
            }
        } else {
            Request::Read
        };
        let serial = self.serial.get().wrapping_add(1);
        self.serial.set(serial);
        self.error.borrow_mut().clear();
        self.pending.replace(Some(Pending {
            serial,
            save,
            owner: binding.map_or_else(Weak::new, Rc::downgrade),
        }));
        self.refresh_all(None);
        if self
            .sender
            .try_send(Command::Language { serial, request })
            .is_err()
        {
            self.event(serial, Err("Language settings are unavailable".into()));
        }
    }
    pub fn event(&self, serial: u64, result: Result<Snapshot, String>) {
        let owner = if serial == 0 {
            if self.pending.borrow().is_some() || self.snapshot.borrow().is_some() {
                return;
            }
            None
        } else {
            if self
                .pending
                .borrow()
                .as_ref()
                .is_none_or(|p| p.serial != serial)
            {
                return;
            }
            self.pending
                .borrow_mut()
                .take()
                .and_then(|p| p.owner.upgrade())
        };
        match result {
            Ok(snapshot) => {
                let system = glib::language_names()
                    .first()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "en-US".into());
                if let Err(error) = crate::language::set(&snapshot.language, &system) {
                    self.error.replace(error);
                } else {
                    self.snapshot.replace(Some(snapshot));
                    self.error.borrow_mut().clear();
                    crate::i18n_ui::apply();
                    if let Some(changed) = self.on_changed.borrow().as_ref() {
                        changed();
                    }
                }
            }
            Err(error) => {
                self.error.replace(error);
            }
        }
        self.feedback.finished("language");
        let force = if self.error.borrow().is_empty() {
            owner.as_ref()
        } else {
            None
        };
        self.refresh_all(force);
    }
    fn refresh_all(&self, force: Option<&Rc<Binding>>) {
        let bindings = self
            .bindings
            .borrow()
            .iter()
            .filter_map(Weak::upgrade)
            .collect::<Vec<_>>();
        for binding in bindings {
            self.refresh(&binding, force.is_some_and(|f| Rc::ptr_eq(f, &binding)));
        }
    }
    fn refresh(&self, binding: &Binding, force: bool) {
        let pending = self.pending.borrow();
        let snapshot = self.snapshot.borrow();
        let error = self.error.borrow();
        binding.updating.set(true);
        let selected = binding.row.selected();
        let system = crate::language::tr("Use system language");
        if binding.model.string(0).as_deref() != Some(&system) {
            // Do not replace the open popup's items whenever a choice changes.
            // Only the system label is translated; the autonyms stay stable.
            binding.model.splice(0, 1, &[&system]);
            binding.row.set_selected(selected);
        }
        if (force || !binding.dirty.get()) && snapshot.is_some() {
            binding
                .row
                .set_selected(match snapshot.as_ref().unwrap().language.as_str() {
                    "zh-CN" => 1,
                    "en-US" => 2,
                    _ => 0,
                });
            binding.dirty.set(false);
        }
        binding.updating.set(false);
        binding.saving.set(pending.as_ref().is_some_and(|p| p.save));
        binding
            .row
            .set_sensitive(snapshot.is_some() && pending.is_none());
        binding.reload.set_sensitive(pending.is_none());
        binding
            .save
            .set_sensitive(snapshot.is_some() && pending.is_none() && binding.dirty.get());
        crate::i18n_ui::bind(
            &binding.status,
            "label",
            if !error.is_empty() {
                &error
            } else if pending.as_ref().is_some_and(|p| p.save) {
                "Saving language…"
            } else if snapshot.is_none() {
                "Loading language…"
            } else if binding.dirty.get() {
                "Apply language"
            } else {
                "Language saved"
            },
        );
        if let Some(changed) = binding.on_changed.borrow().as_ref() {
            changed();
        }
    }
    pub fn allow_window_close(&self) -> bool {
        if self.pending.borrow().as_ref().is_some_and(|p| p.save) {
            self.feedback.waiting("language", "Saving language…");
            false
        } else {
            true
        }
    }
    pub fn gateway_failed(&self) {
        let serial = self.pending.borrow().as_ref().map(|p| p.serial);
        if let Some(serial) = serial {
            self.event(serial, Err("Language settings are unavailable".into()));
        }
    }
}
impl Binding {
    #[cfg(debug_assertions)]
    pub fn smoke_select(&self, index: u32) {
        self.row.set_selected(index);
    }
    #[cfg(debug_assertions)]
    pub fn smoke_apply(&self) {
        self.save.emit_clicked();
    }
    #[cfg(debug_assertions)]
    pub fn smoke_reload(&self) {
        self.reload.emit_clicked();
    }
    #[cfg(debug_assertions)]
    pub fn smoke_selected(&self) -> u32 {
        self.row.selected()
    }
    #[cfg(debug_assertions)]
    pub fn smoke_status(&self) -> String {
        self.status.text().into()
    }
    pub fn dirty(&self) -> bool {
        self.dirty.get()
    }
    pub fn saving(&self) -> bool {
        self.saving.get()
    }
    pub fn discard(&self) {
        self.dirty.set(false);
    }
}
#[cfg(debug_assertions)]
impl Runtime {
    pub fn smoke_language(&self) -> Option<String> {
        self.snapshot.borrow().as_ref().map(|s| s.language.clone())
    }
}
