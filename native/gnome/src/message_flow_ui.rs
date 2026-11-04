//! Shared composer controls with explicit saves and retained conflicting edits.
use crate::{
    i18n_ui::UiText,
    message_flow::{Request, Snapshot, Values},
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
    epoch: Cell<u64>,
    feedback: Rc<crate::write_feedback::Queue>,
    pub on_changed: RefCell<Option<Box<dyn Fn(u8)>>>,
}
pub struct Binding {
    pub on_changed: RefCell<Option<Box<dyn Fn()>>>,
    pub group: adw::PreferencesGroup,
    depth: adw::ComboRow,
    behavior: adw::ComboRow,
    depth_model: gtk::StringList,
    behavior_model: gtk::StringList,
    save: gtk::Button,
    reload: gtk::Button,
    status: gtk::Label,
    revision: RefCell<String>,
    dirty: Cell<bool>,
    updating: Cell<bool>,
    saving: Cell<bool>,
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
            epoch: Cell::new(0),
            on_changed: RefCell::default(),
        })
    }
    pub fn epoch(&self) -> u64 {
        self.epoch.get()
    }
    pub fn behavior(&self) -> Option<String> {
        self.snapshot
            .borrow()
            .as_ref()
            .map(|s| s.values.continue_behavior.clone())
    }
    pub fn limit(&self) -> Option<usize> {
        self.snapshot
            .borrow()
            .as_ref()
            .map(|s| s.values.queue_depth as usize)
    }
    pub fn bind(self: &Rc<Self>) -> Rc<Binding> {
        let group = adw::PreferencesGroup::new().ui_text("title", "Message flow");
        let depth_model = gtk::StringList::new(&["1 message", "2 messages", "3 messages"]);
        let behavior_model = gtk::StringList::new(&[
            "Session model draft · review first",
            "Ollama draft · review first",
            "Quick send · random prompt",
        ]);
        let depth = adw::ComboRow::builder()
            .model(&depth_model)
            .build()
            .ui_text("title", "Message queue depth");
        let behavior = adw::ComboRow::builder()
            .model(&behavior_model)
            .build()
            .ui_text("title", "Continue behavior");
        group.add(&depth);
        group.add(&behavior);
        group.add(&gtk::Label::builder().wrap(true).xalign(0.0).css_classes(["caption","dim-label"]).build().ui_text("label","Draft modes fill Chat for review using an isolated task; Quick send immediately sends a short continue phrase."));
        let save = gtk::Button::new().ui_text("label", "Apply message flow");
        let reload = gtk::Button::from_icon_name("view-refresh-symbolic")
            .ui_text("tooltip-text", "Reload message flow");
        let controls = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        controls.append(&reload);
        controls.append(&save);
        group.set_header_suffix(Some(&controls));
        let status = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .css_classes(["caption"])
            .build();
        group.add(&status);
        let binding = Rc::new(Binding {
            on_changed: RefCell::default(),
            group,
            depth,
            behavior,
            depth_model,
            behavior_model,
            save,
            reload,
            status,
            revision: RefCell::default(),
            dirty: Cell::new(false),
            updating: Cell::new(false),
            saving: Cell::new(false),
        });
        for row in [&binding.depth, &binding.behavior] {
            let weak = Rc::downgrade(&binding);
            let runtime = Rc::downgrade(self);
            row.connect_selected_notify(move |_| {
                if let (Some(b), Some(r)) = (weak.upgrade(), runtime.upgrade()) {
                    if !b.updating.get() {
                        b.dirty.set(true);
                        r.refresh(&b, false);
                    }
                }
            });
        }
        for (button, save) in [(&binding.save, true), (&binding.reload, false)] {
            let weak = Rc::downgrade(&binding);
            let runtime = Rc::downgrade(self);
            button.connect_clicked(move |_| {
                if let (Some(b), Some(r)) = (weak.upgrade(), runtime.upgrade()) {
                    r.request(&b, save);
                }
            });
        }
        self.bindings.borrow_mut().retain(|b| b.strong_count() > 0);
        self.bindings.borrow_mut().push(Rc::downgrade(&binding));
        self.refresh(&binding, true);
        self.request(&binding, false);
        binding
    }
    fn request(&self, binding: &Rc<Binding>, save: bool) {
        if self.pending.borrow().is_some() {
            return;
        }
        let request = if save {
            if binding.revision.borrow().is_empty() {
                return;
            }
            Request::Save {
                revision: binding.revision.borrow().clone(),
                values: Values {
                    queue_depth: (binding.depth.selected() + 1) as u8,
                    continue_behavior: match binding.behavior.selected() {
                        1 => "ollamaDraft",
                        2 => "quickSend",
                        _ => "sessionModelDraft",
                    }
                    .into(),
                },
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
            owner: Rc::downgrade(binding),
        }));
        self.refresh_all(None);
        if self
            .sender
            .try_send(Command::MessageFlow { serial, request })
            .is_err()
        {
            self.event(serial, Err("Message-flow settings are unavailable".into()));
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
                let depth = snapshot.values.queue_depth;
                self.snapshot.replace(Some(snapshot));
                self.error.borrow_mut().clear();
                self.epoch.set(self.epoch.get().wrapping_add(1));
                if let Some(changed) = self.on_changed.borrow().as_ref() {
                    changed(depth);
                }
            }
            Err(error) => {
                self.error.replace(error);
            }
        }
        self.feedback.finished("message-flow");
        self.refresh_all(if self.error.borrow().is_empty() {
            owner.as_ref()
        } else {
            None
        });
    }
    fn refresh_all(&self, force: Option<&Rc<Binding>>) {
        for b in self.bindings.borrow().iter().filter_map(Weak::upgrade) {
            self.refresh(&b, force.is_some_and(|f| Rc::ptr_eq(f, &b)));
        }
    }
    fn refresh(&self, b: &Binding, force: bool) {
        let pending = self.pending.borrow();
        let snapshot = self.snapshot.borrow();
        let error = self.error.borrow();
        b.updating.set(true);
        for (model, labels, row) in [
            (
                &b.depth_model,
                ["1 message", "2 messages", "3 messages"],
                &b.depth,
            ),
            (
                &b.behavior_model,
                [
                    "Session model draft · review first",
                    "Ollama draft · review first",
                    "Quick send · random prompt",
                ],
                &b.behavior,
            ),
        ] {
            let selected = row.selected();
            for (i, key) in labels.iter().enumerate() {
                let text = crate::language::tr(key);
                if model.string(i as u32).as_deref() != Some(text.as_str()) {
                    model.splice(i as u32, 1, &[&text]);
                }
            }
            row.set_selected(selected);
        }
        if let Some(s) = snapshot.as_ref().filter(|_| force || !b.dirty.get()) {
            b.depth.set_selected((s.values.queue_depth - 1) as u32);
            b.behavior
                .set_selected(match s.values.continue_behavior.as_str() {
                    "ollamaDraft" => 1,
                    "quickSend" => 2,
                    _ => 0,
                });
            b.revision.replace(s.revision.clone());
            b.dirty.set(false);
        }
        b.updating.set(false);
        b.saving.set(pending.as_ref().is_some_and(|p| p.save));
        b.depth
            .set_sensitive(snapshot.is_some() && pending.is_none());
        b.behavior
            .set_sensitive(snapshot.is_some() && pending.is_none());
        b.reload.set_sensitive(pending.is_none());
        b.save
            .set_sensitive(!b.revision.borrow().is_empty() && pending.is_none() && b.dirty.get());
        crate::i18n_ui::bind(
            &b.status,
            "label",
            if !error.is_empty() {
                error.as_str()
            } else if pending.as_ref().is_some_and(|p| p.save) {
                "Saving message flow…"
            } else if snapshot.is_none() || pending.is_some() {
                "Loading message flow…"
            } else if b.dirty.get() {
                "Apply message flow"
            } else {
                "Message flow saved"
            },
        );
        if let Some(changed) = b.on_changed.borrow().as_ref() {
            changed();
        }
    }
    pub fn refresh_language(&self) {
        self.refresh_all(None);
    }
    pub fn allow_window_close(&self) -> bool {
        if self.pending.borrow().as_ref().is_some_and(|p| p.save) {
            self.feedback
                .waiting("message-flow", "Saving message flow…");
            false
        } else {
            true
        }
    }
    pub fn gateway_failed(&self) {
        let serial = self.pending.borrow().as_ref().map(|p| p.serial);
        if let Some(serial) = serial {
            self.event(serial, Err("Message-flow settings are unavailable".into()));
        }
    }
}
impl Binding {
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
