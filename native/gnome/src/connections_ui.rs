//! Native connection/account metadata, refreshed only by an explicit action.
use crate::{connections::Snapshot, i18n_ui::UiText, protocol::Command};
use adw::prelude::*;
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::sync::mpsc;

#[derive(Clone, Default, PartialEq, Eq)]
pub struct Runtime {
    pub ready: HashSet<String>,
    pub known: HashSet<String>,
    pub stopped: bool,
}
impl Runtime {
    fn status(&self, id: &str) -> &'static str {
        if self.stopped {
            "Gateway stopped"
        } else if self.ready.contains(id) {
            "Connected"
        } else if self.known.contains(id) {
            "Not connected"
        } else {
            "Connect on demand"
        }
    }
}
pub struct Panel {
    pub id: u64,
    pub dialog: adw::Dialog,
    sender: mpsc::Sender<Command>,
    page: adw::PreferencesPage,
    groups: RefCell<Vec<adw::PreferencesGroup>>,
    event_labels: RefCell<HashMap<String, gtk::Label>>,
    runtime: RefCell<Runtime>,
    refresh: gtk::Button,
    notice: gtk::Label,
    serial: Cell<u64>,
    pending: Cell<Option<u64>>,
    cancel: RefCell<Arc<AtomicBool>>,
    loaded: Cell<bool>,
}
fn detail_row(key: &str, value: &str) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .subtitle_selectable(true)
        .subtitle_lines(0)
        .build();
    // ActionRow's subtitle may be constructed before the inherited markup
    // property. Disable markup before assigning literal provider data.
    row.set_use_markup(false);
    row.set_subtitle(value);
    row.ui_text("title", key)
}
fn row(group: &impl IsA<adw::PreferencesGroup>, key: &str, value: &str) {
    let row = detail_row(key, value);
    group.as_ref().add(&row);
}
fn detail(parent: &adw::ExpanderRow, key: &str, value: &str) {
    let row = detail_row(key, value);
    parent.add_row(&row);
}
impl Panel {
    pub fn new(sender: mpsc::Sender<Command>, id: u64) -> Rc<Self> {
        let dialog = adw::Dialog::builder()
            .content_width(720)
            .content_height(700)
            .build()
            .ui_text("title", "Connections & accounts");
        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        let refresh =
            crate::workspace_ui::button("view-refresh-symbolic", "Refresh connection information");
        header.pack_start(&refresh);
        toolbar.add_top_bar(&header);
        let page = adw::PreferencesPage::new();
        let notice = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .margin_start(20)
            .margin_end(20)
            .margin_top(8)
            .margin_bottom(8)
            .build();
        let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        body.append(&notice);
        body.append(&page);
        page.set_vexpand(true);
        toolbar.set_content(Some(&body));
        dialog.set_child(Some(&toolbar));
        let panel = Rc::new(Self {
            id,
            dialog,
            sender,
            page,
            groups: Default::default(),
            event_labels: Default::default(),
            runtime: Default::default(),
            refresh,
            notice,
            serial: Cell::new(0),
            pending: Cell::new(None),
            cancel: RefCell::new(Arc::new(AtomicBool::new(false))),
            loaded: Cell::new(false),
        });
        let weak = Rc::downgrade(&panel);
        panel.refresh.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.read();
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.dialog.connect_closed(move |_| {
            if let Some(p) = weak.upgrade() {
                p.cancel.borrow().store(true, Ordering::Release);
                p.serial.set(p.serial.get() + 1);
                p.pending.set(None);
            }
        });
        panel
    }
    fn notice(&self, key: &str) {
        crate::i18n_ui::bind(&self.notice, "label", key);
        self.notice.set_visible(!key.is_empty());
    }
    fn error(&self, text: &str) {
        crate::i18n_ui::literal(&self.notice, "label", &text.replace('\0', "�"));
        self.notice.set_visible(true);
    }
    fn controls(&self) {
        self.refresh
            .set_sensitive(self.pending.get().is_none() && !self.runtime.borrow().stopped);
    }
    pub fn sync(&self, runtime: Runtime) {
        if *self.runtime.borrow() != runtime {
            if runtime.stopped {
                self.cancel.borrow().store(true, Ordering::Release);
                self.serial.set(self.serial.get() + 1);
                self.pending.set(None);
                self.notice("Gateway stopped. Use Reconnect to restart it.");
            }
            self.runtime.replace(runtime);
            for (id, label) in self.event_labels.borrow().iter() {
                crate::i18n_ui::bind(label, "label", self.runtime.borrow().status(id));
            }
        }
        self.controls();
    }
    pub fn read(&self) {
        if self.pending.get().is_some() || self.runtime.borrow().stopped {
            return;
        }
        self.cancel.borrow().store(true, Ordering::Release);
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel.replace(cancel.clone());
        let serial = self.serial.get() + 1;
        self.serial.set(serial);
        self.pending.set(Some(serial));
        self.notice("Loading connection information…");
        if self
            .sender
            .try_send(Command::Connections {
                dialog: self.id,
                serial,
                cancel,
            })
            .is_err()
        {
            self.pending.set(None);
            self.notice("Connection information is busy; retry");
        }
        self.controls();
    }
    pub fn event(&self, serial: u64, result: Result<Snapshot, String>) {
        if self.pending.get() != Some(serial) || self.runtime.borrow().stopped {
            return;
        }
        self.pending.set(None);
        match result {
            Ok(snapshot) => {
                self.render(snapshot);
                self.loaded.set(true);
                self.notice("");
            }
            Err(error) => self.error(&error),
        }
        self.controls();
    }
    fn group(&self, key: &str) -> adw::PreferencesGroup {
        let group = adw::PreferencesGroup::new().ui_text("title", key);
        self.page.add(&group);
        self.groups.borrow_mut().push(group.clone());
        group
    }
    fn render(&self, snapshot: Snapshot) {
        for group in self.groups.borrow_mut().drain(..) {
            self.page.remove(&group);
        }
        self.event_labels.borrow_mut().clear();
        let backends = self.group("Backends");
        let config = self.group("Local backend configuration");
        let application = self.group("Application");
        row(&application, "Application", "Codex Thread Studio · GNOME");
        row(&application, "Version", env!("CARGO_PKG_VERSION"));
        row(
            &application,
            "Gateway version",
            if snapshot.gateway_version.is_empty() {
                "—"
            } else {
                &snapshot.gateway_version
            },
        );
        if let Some(error) = &snapshot.gateway_error {
            row(&application, "Error", error);
        }
        let mode = adw::ActionRow::builder()
            .use_markup(false)
            .build()
            .ui_text("title", "Runtime mode")
            .ui_text("subtitle", "Local workspace");
        application.add(&mode);
        row(&config, "Configuration file", &snapshot.config_path);
        if let Some(error) = &snapshot.configuration_error {
            row(&config, "Configuration error", error);
        }
        let hint = adw::ActionRow::builder()
            .use_markup(false)
            .build()
            .ui_text("title", "Restart Studio after editing this file.");
        config.add(&hint);
        for backend in snapshot.backends {
            let d = &backend.descriptor;
            let info = &backend.info;
            let item = adw::ExpanderRow::builder()
                .expanded(backend.error.is_some())
                .build();
            item.set_use_markup(false);
            item.set_title(&d.name.replace('\0', "�"));
            item.set_subtitle(&d.id);
            let label = gtk::Label::new(None);
            label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            label.set_max_width_chars(22);
            label.add_css_class("dim-label");
            crate::i18n_ui::bind(&label, "tooltip-text", "Session events");
            crate::i18n_ui::bind(&label, "label", self.runtime.borrow().status(&d.id));
            item.add_suffix(&label);
            self.event_labels.borrow_mut().insert(d.id.clone(), label);
            let fallback = |value: &str| {
                if value.is_empty() {
                    "—".to_owned()
                } else {
                    value.to_owned()
                }
            };
            detail(&item, "Executable", &fallback(&info.binary));
            detail(
                &item,
                "Backend version",
                info.backend_version
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or("—"),
            );
            detail(
                &item,
                "Protocol",
                if info.protocol.is_empty() {
                    &d.protocol
                } else {
                    &info.protocol
                },
            );
            detail(
                &item,
                "Transport",
                if info.transport.is_empty() {
                    &d.transport
                } else {
                    &info.transport
                },
            );
            let environment = adw::ActionRow::builder()
                .use_markup(false)
                .build()
                .ui_text("title", "Execution environment");
            match info.execution_environment.as_str() {
                "local" => {
                    crate::i18n_ui::bind(&environment, "subtitle", "Local");
                }
                "wsl" => {
                    environment.set_subtitle(&format!(
                        "WSL · {}",
                        info.wsl_distribution.as_deref().unwrap_or("—")
                    ));
                }
                _ => environment.set_subtitle(&fallback(&info.execution_environment)),
            };
            environment.set_subtitle_selectable(true);
            item.add_row(&environment);
            detail(&item, "Host platform", &fallback(&info.host_platform));
            if let Some(reachable) = info.reachable {
                let health = adw::ActionRow::builder()
                    .build()
                    .ui_text("title", "Backend availability")
                    .ui_text(
                        "subtitle",
                        if reachable {
                            "Available"
                        } else {
                            "Unavailable"
                        },
                    );
                item.add_row(&health);
            }
            if let Some(error) = &backend.error {
                detail(&item, "Error", error);
            }
            backends.add(&item);
        }
    }
    #[cfg(debug_assertions)]
    pub(crate) fn smoke_state(&self) -> serde_json::Value {
        serde_json::json!({"loaded":self.loaded.get(),"pending":self.pending.get().is_some(),"serial":self.serial.get(),"status":self.notice.text().to_string(),"refreshEnabled":self.refresh.is_sensitive(),"events":self.event_labels.borrow().iter().map(|(id,label)|(id.clone(),label.text().to_string())).collect::<HashMap<_,_>>()})
    }
}
