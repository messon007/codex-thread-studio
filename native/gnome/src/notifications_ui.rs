//! GIO desktop notifications and native preference controls; no secondary UI engine.
use crate::i18n_ui::UiText;
use crate::{
    notifications::{Request, Snapshot},
    protocol::Command,
};
use adw::prelude::*;
use gtk::{gio, glib};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::{Rc, Weak},
};
use tokio::sync::mpsc;
pub struct Runtime {
    app: adw::Application,
    sender: mpsc::Sender<Command>,
    snapshot: RefCell<Option<Snapshot>>,
    serial: Cell<u64>,
    saving: Cell<bool>,
    pending: Cell<Option<bool>>,
    error: RefCell<String>,
    bindings: RefCell<Vec<Weak<Binding>>>,
    requests: RefCell<crate::notifications::Requests>,
    issued: RefCell<VecDeque<String>>,
    dismissed: RefCell<VecDeque<String>>,
    ledger_ready: Cell<bool>,
    ledger_dirty: Cell<bool>,
    timer: RefCell<Option<glib::SourceId>>,
    #[cfg(debug_assertions)]
    pub activations: Cell<u32>,
}
pub struct Binding {
    pub group: adw::PreferencesGroup,
    pub row: adw::SwitchRow,
    reload: gtk::Button,
    reset: gtk::Button,
    status: gtk::Label,
    updating: Cell<bool>,
    pub on_update: RefCell<Option<Box<dyn Fn()>>>,
}
impl Runtime {
    pub fn new(app: &adw::Application, sender: mpsc::Sender<Command>) -> Rc<Self> {
        let ui = Rc::new(Self {
            app: app.clone(),
            sender,
            snapshot: RefCell::new(None),
            serial: Cell::new(0),
            saving: Cell::new(false),
            pending: Cell::new(None),
            error: RefCell::new(String::new()),
            bindings: RefCell::new(vec![]),
            requests: RefCell::new(Default::default()),
            issued: RefCell::new(VecDeque::new()),
            dismissed: RefCell::new(VecDeque::new()),
            ledger_ready: Cell::new(false),
            ledger_dirty: Cell::new(false),
            timer: RefCell::new(None),
            #[cfg(debug_assertions)]
            activations: Cell::new(0),
        });
        let weak = Rc::downgrade(&ui);
        let timer = glib::timeout_add_local(std::time::Duration::from_millis(150), move || {
            let Some(ui) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            ui.flush();
            glib::ControlFlow::Continue
        });
        ui.timer.replace(Some(timer));
        ui
    }
    pub fn busy(&self) -> bool {
        self.saving.get()
    }
    pub fn loaded(&self) -> bool {
        self.snapshot.borrow().is_some()
    }
    pub fn enabled(&self) -> bool {
        self.pending.get() != Some(false) && {
            self.snapshot
                .borrow()
                .as_ref()
                .and_then(|s| s.enabled)
                .unwrap_or(false)
        }
    }
    pub fn bind(self: &Rc<Self>) -> Rc<Binding> {
        let group = adw::PreferencesGroup::builder()
            .title("Notifications")
            .description("GNOME notification settings and Do Not Disturb also apply.")
            .build()
            .ui_text("title", "Notifications")
            .ui_text(
                "description",
                "GNOME notification settings and Do Not Disturb also apply.",
            );
        let row=adw::SwitchRow::builder().title("Desktop notifications").subtitle("Notify when Studio is in the background and work completes or an agent needs input").build().ui_text("title","Desktop notifications").ui_text("subtitle","Notify when Studio is in the background and work completes or an agent needs input");
        group.add(&row);
        let controls = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let reload = gtk::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text("Reload notification settings")
            .valign(gtk::Align::Center)
            .build()
            .ui_text("tooltip-text", "Reload notification settings");
        let reset = gtk::Button::builder()
            .label("Disable")
            .tooltip_text("Replace the invalid saved notification setting with disabled")
            .valign(gtk::Align::Center)
            .build()
            .ui_text("label", "Disable")
            .ui_text(
                "tooltip-text",
                "Replace the invalid saved notification setting with disabled",
            );
        controls.append(&reset);
        controls.append(&reload);
        group.set_header_suffix(Some(&controls));
        let status = gtk::Label::builder()
            .wrap(true)
            .xalign(0.0)
            .margin_top(6)
            .build();
        group.add(&status);
        let binding = Rc::new(Binding {
            group,
            row,
            reload,
            reset,
            status,
            updating: Cell::new(false),
            on_update: RefCell::new(None),
        });
        let weak = Rc::downgrade(self);
        let b = Rc::downgrade(&binding);
        binding.row.connect_active_notify(move |row| {
            if let (Some(ui), Some(b)) = (weak.upgrade(), b.upgrade()) {
                if !b.updating.get() {
                    ui.save(row.is_active());
                }
            }
        });
        let weak = Rc::downgrade(self);
        binding.reload.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.reload();
            }
        });
        let weak = Rc::downgrade(self);
        binding.reset.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.save(false);
            }
        });
        self.bindings.borrow_mut().push(Rc::downgrade(&binding));
        self.repaint();
        binding
    }
    fn repaint(&self) {
        let snapshot = self.snapshot.borrow().clone();
        let error = self.error.borrow().clone();
        self.bindings.borrow_mut().retain(|weak| {
            let Some(b) = weak.upgrade() else {
                return false;
            };
            b.updating.set(true);
            b.row
                .set_active(self.pending.get().unwrap_or_else(|| self.enabled()));
            b.row.set_sensitive(snapshot.is_some() && !self.busy());
            b.reload.set_sensitive(!self.busy());
            b.reset
                .set_visible(snapshot.as_ref().is_some_and(|s| s.enabled.is_none()));
            b.reset.set_sensitive(!self.busy());
            let message = if self.busy() {
                "Saving notification settings…"
            } else if !error.is_empty() {
                &error
            } else if let Some(e) = snapshot
                .as_ref()
                .and_then(|s| s.validation_error.as_deref())
            {
                e
            } else {
                ""
            };
            b.status.set_text(message);
            b.status.set_visible(!message.is_empty());
            b.updating.set(false);
            if let Some(callback) = b.on_update.borrow().as_ref() {
                callback();
            }
            true
        });
    }
    fn save(&self, enabled: bool) {
        if self.busy() {
            return;
        }
        let Some(snapshot) = self.snapshot.borrow().clone() else {
            self.repaint();
            return;
        };
        self.pending.set(Some(enabled));
        self.request(Request::Save {
            revision: snapshot.revision,
            enabled,
        });
    }
    pub fn reload(&self) {
        self.request(Request::Read);
    }
    fn request(&self, request: Request) {
        if self.saving.replace(true) {
            return;
        }
        self.serial.set(self.serial.get() + 1);
        self.error.borrow_mut().clear();
        self.repaint();
        if self
            .sender
            .try_send(Command::NotificationSettings {
                serial: self.serial.get(),
                request,
            })
            .is_err()
        {
            self.saving.set(false);
            self.pending.set(None);
            self.error
                .replace("Settings queue is unavailable; reload and retry".into());
            self.repaint();
        }
    }
    pub fn event(&self, serial: u64, result: Result<Snapshot, String>) {
        if serial != self.serial.get() {
            return;
        }
        self.saving.set(false);
        self.pending.set(None);
        match result {
            Ok(s) => {
                self.snapshot.replace(Some(s));
                self.error.borrow_mut().clear();
                if !self.enabled() {
                    self.withdraw_all();
                }
            }
            Err(e) => {
                self.error.replace(e);
            }
        }
        self.repaint();
    }
    pub fn restore_ids(&self, value: &serde_json::Value) {
        let mut ids = self.issued.borrow_mut();
        if let Some(values) = value.as_array() {
            for id in values
                .iter()
                .take(crate::notification_ids::MAX_IDS)
                .filter_map(|v| v.as_str())
                .filter(|s| crate::notification_ids::valid(s))
            {
                if !ids.iter().any(|s| s == id) && !self.dismissed.borrow().iter().any(|s| s == id)
                {
                    if ids.len() < crate::notification_ids::MAX_IDS {
                        ids.push_back(id.into());
                    } else {
                        // Live notices can arrive before the startup ledger.
                        // Preserve them and retire excess historical entries.
                        self.app.withdraw_notification(id);
                        self.ledger_dirty.set(true);
                    }
                }
            }
        }
        drop(ids);
        self.ledger_ready.set(true);
        if self.loaded() && !self.enabled() {
            self.withdraw_all();
        }
    }
    pub fn flush(&self) {
        if self.ledger_dirty.get() && self.ledger_ready.get() {
            let ids = self.issued.borrow().iter().cloned().collect::<Vec<_>>();
            if self
                .sender
                .try_send(Command::Preferences(
                    serde_json::json!({"notificationIds":ids}),
                ))
                .is_ok()
            {
                self.ledger_dirty.set(false);
            }
        }
    }
    fn withdraw_all(&self) {
        let ids = self.issued.borrow_mut().drain(..).collect::<Vec<_>>();
        for id in &ids {
            self.app.withdraw_notification(id);
        }
        if !ids.is_empty() {
            self.ledger_dirty.set(true);
        }
    }
    pub fn withdraw(&self, id: &str) {
        if !crate::notification_ids::valid(id) {
            return;
        }
        self.app.withdraw_notification(id);
        {
            let mut dismissed = self.dismissed.borrow_mut();
            if !dismissed.iter().any(|s| s == id) {
                dismissed.push_back(id.into());
                if dismissed.len() > crate::notification_ids::MAX_IDS {
                    dismissed.pop_front();
                }
            }
        }
        self.issued.borrow_mut().retain(|s| s != id);
        self.ledger_dirty.set(true);
    }
    pub fn foreground(&self) -> bool {
        self.app.active_window().is_some_and(|w| w.is_active())
    }
    pub fn completion(&self, backend: &str, thread: &str, body: &str, failed: bool) {
        self.notice(
            backend,
            thread,
            if failed {
                "Work failed"
            } else {
                "Work completed"
            },
            body,
        );
    }
    pub fn approval(&self, backend: &str, thread: &str, body: &str) -> Option<String> {
        self.notice(backend, thread, "Agent needs your input", body)
    }
    pub fn claim_request(&self, backend: &str, generation: u64, id: &serde_json::Value) -> bool {
        self.requests.borrow_mut().claim(backend, generation, id)
    }
    fn notice(&self, backend: &str, thread: &str, title: &str, body: &str) -> Option<String> {
        if !self.enabled() || self.foreground() {
            return None;
        }
        let id = crate::notification_ids::tag(backend, thread)?;
        let notification = gio::Notification::new(&crate::language::tr(title));
        let body = body
            .chars()
            .filter(|c| !c.is_control())
            .take(240)
            .collect::<String>();
        notification.set_body(Some(&body));
        notification.set_icon(&gio::ThemedIcon::new(
            "io.github.messon007.codexthreadstudio.Gnome",
        ));
        notification
            .set_default_action_and_target_value("app.show-notification", Some(&id.to_variant()));
        let mut issued = self.issued.borrow_mut();
        self.dismissed.borrow_mut().retain(|s| s != &id);
        if !issued.contains(&id) {
            if issued.len() >= crate::notification_ids::MAX_IDS {
                if let Some(old) = issued.pop_front() {
                    self.app.withdraw_notification(&old);
                }
            }
            issued.push_back(id.clone());
            self.ledger_dirty.set(true);
        }
        drop(issued);
        self.app.send_notification(Some(&id), &notification);
        Some(id)
    }
    pub fn activated(&self, id: &str) {
        self.withdraw(id);
        #[cfg(debug_assertions)]
        self.activations.set(self.activations.get() + 1);
    }
}
#[cfg(debug_assertions)]
impl Binding {
    pub fn refresh(&self) {
        self.reload.emit_clicked();
    }
    pub fn error_text(&self) -> String {
        self.status.text().to_string()
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        if let Some(timer) = self.timer.get_mut().take() {
            timer.remove();
        }
    }
}
