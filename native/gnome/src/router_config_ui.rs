use crate::{
    model::Backend,
    protocol::Command,
    router_config::{self as config, Fallback, Reply, Request, Settings, Snapshot, Target},
};
use adw::prelude::*;
use gtk::glib;
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
use tokio::sync::mpsc;
static NEXT: AtomicU64 = AtomicU64::new(1);
#[path = "router_recovery_ui.rs"]
mod recovery;
#[derive(Clone)]
enum Pending {
    Read,
    Controller,
    Recovery(u64, bool),
    Targets,
    Save(Settings),
    Role(String),
}
pub struct Panel {
    pub dialog: adw::Dialog,
    pub id: u64,
    pub role_context: Option<crate::composer_tools_ui::Context>,
    pub role_valid: RefCell<Option<Box<dyn Fn() -> bool>>>,
    window: adw::ApplicationWindow,
    sender: mpsc::Sender<Command>,
    backends: Vec<Backend>,
    backend: gtk::DropDown,
    controller: gtk::Label,
    setup: gtk::Button,
    recover: gtk::Button,
    recovery: RefCell<Option<Rc<recovery::Recovery>>>,
    catalog_status: gtk::Label,
    targets: RefCell<Arc<Vec<Target>>>,
    keys: RefCell<Vec<String>>,
    fallbacks: Vec<(gtk::DropDown, gtk::TextView)>,
    role: gtk::TextView,
    role_save: gtk::Button,
    save: gtk::Button,
    reload: gtk::Button,
    feedback: gtk::Label,
    snapshot: RefCell<Option<Snapshot>>,
    role_revision: RefCell<String>,
    confirmation: RefCell<Option<adw::AlertDialog>>,
    pending: RefCell<HashMap<u64, Pending>>,
    serial: Cell<u64>,
    applying: Cell<bool>,
    config_dirty: Cell<bool>,
    config_conflict: Cell<bool>,
    role_dirty: Cell<bool>,
}
fn dropdown() -> gtk::DropDown {
    let dropdown = gtk::DropDown::builder()
        .enable_search(true)
        .hexpand(true)
        .build();
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, object| {
        let item = object.downcast_ref::<gtk::ListItem>().unwrap();
        item.set_child(Some(
            &gtk::Label::builder()
                .xalign(0.0)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .max_width_chars(64)
                .width_chars(20)
                .build(),
        ));
    });
    factory.connect_bind(|_, object| {
        let item = object.downcast_ref::<gtk::ListItem>().unwrap();
        if let (Some(value), Some(label)) = (
            item.item().and_downcast::<gtk::StringObject>(),
            item.child().and_downcast::<gtk::Label>(),
        ) {
            label.set_text(&value.string());
            label.set_tooltip_text(Some(&value.string()));
        }
    });
    dropdown.set_factory(Some(&factory));
    dropdown.set_list_factory(Some(&factory));
    dropdown
}
fn text() -> gtk::TextView {
    let view = gtk::TextView::builder()
        .wrap_mode(gtk::WrapMode::WordChar)
        .top_margin(8)
        .bottom_margin(8)
        .left_margin(8)
        .right_margin(8)
        .build();
    view.set_accepts_tab(false);
    view
}
fn text_scroll(view: &gtk::TextView, height: i32) -> gtk::ScrolledWindow {
    gtk::ScrolledWindow::builder()
        .child(view)
        .min_content_height(height)
        .max_content_height(height)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build()
}
fn value(view: &gtk::TextView) -> String {
    let buffer = view.buffer();
    buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), true)
        .into()
}
fn heading(body: &gtk::Box, title: &str) {
    let label = gtk::Label::builder()
        .label(title)
        .xalign(0.0)
        .margin_top(12)
        .build();
    label.add_css_class("heading");
    body.append(&label);
}
impl Panel {
    pub fn new(
        window: &adw::ApplicationWindow,
        sender: mpsc::Sender<Command>,
        backends: Vec<Backend>,
        role_context: Option<crate::composer_tools_ui::Context>,
        title: String,
    ) -> Rc<Self> {
        let dialog = adw::Dialog::builder()
            .title("Thread Router settings")
            .content_width(690)
            .content_height(680)
            .build();
        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        let reload = gtk::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text("Reload Router settings")
            .build();
        header.pack_end(&reload);
        toolbar.add_top_bar(&header);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        body.set_margin_start(20);
        body.set_margin_end(20);
        body.set_margin_bottom(20);
        heading(&body, "Routing controller");
        let backend = dropdown();
        let labels = backends
            .iter()
            .map(|backend| backend.name.as_str())
            .collect::<Vec<_>>();
        backend.set_model(Some(&gtk::StringList::new(&labels)));
        backend.update_property(&[gtk::accessible::Property::Label("Router backend")]);
        body.append(&backend);
        let controller = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .selectable(true)
            .build();
        controller.add_css_class("dim-label");
        body.append(&controller);
        let setup = gtk::Button::with_label("Set up controller");
        setup.set_halign(gtk::Align::Start);
        let recover = gtk::Button::with_label("Recover existing controller…");
        let controller_actions = crate::adaptive::toolbar(8);
        controller_actions.set_align(0.0);
        controller_actions.append(&setup);
        controller_actions.append(&recover);
        body.append(&controller_actions);
        let role = crate::responsibility_ui::view();
        role.update_property(&[gtk::accessible::Property::Label("Session responsibility")]);
        let role_save = gtk::Button::with_label("Save responsibility");
        if let Some(context) = &role_context {
            heading(&body, "Session responsibility");
            body.append(
                &gtk::Label::builder()
                    .label(format!("{title} · {}", context.backend))
                    .xalign(0.0)
                    .wrap(true)
                    .wrap_mode(gtk::pango::WrapMode::WordChar)
                    .build(),
            );
            body.append(&text_scroll(&role, 100));
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            row.set_halign(gtk::Align::End);
            row.append(&role_save);
            body.append(&row);
        }
        heading(&body, "Fallback sessions");
        let catalog_status = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .selectable(true)
            .build();
        catalog_status.add_css_class("dim-label");
        catalog_status.add_css_class("caption");
        body.append(&catalog_status);
        body.append(&gtk::Label::builder().label("When no regular session matches, use a fallback only when its condition applies.").xalign(0.0).wrap(true).wrap_mode(gtk::pango::WrapMode::WordChar).build());
        let mut fallbacks = Vec::new();
        for index in 0..3 {
            let target = dropdown();
            target.update_property(&[gtk::accessible::Property::Label(&format!(
                "Fallback target {}",
                index + 1
            ))]);
            let condition = text();
            condition.update_property(&[gtk::accessible::Property::Label(&format!(
                "Fallback condition {}",
                index + 1
            ))]);
            let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
            card.set_margin_top(8);
            card.append(&target);
            card.append(&text_scroll(&condition, 70));
            body.append(&card);
            fallbacks.push((target, condition));
        }
        let scroll = gtk::ScrolledWindow::builder()
            .child(&body)
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();
        let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
        content.append(&scroll);
        let feedback = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .selectable(true)
            .margin_start(20)
            .margin_end(20)
            .build();
        content.append(&feedback);
        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        footer.set_halign(gtk::Align::End);
        footer.set_margin_end(20);
        footer.set_margin_bottom(16);
        let save = gtk::Button::with_label("Save Router settings");
        save.add_css_class("suggested-action");
        footer.append(&save);
        content.append(&footer);
        toolbar.set_content(Some(&content));
        dialog.set_child(Some(&toolbar));
        let panel = Rc::new(Self {
            dialog,
            id: NEXT.fetch_add(1, Ordering::Relaxed),
            role_context,
            role_valid: RefCell::new(None),
            window: window.clone(),
            sender,
            backends,
            backend: backend.clone(),
            controller,
            setup: setup.clone(),
            recover: recover.clone(),
            recovery: RefCell::new(None),
            catalog_status,
            targets: RefCell::new(Arc::new(Vec::new())),
            keys: RefCell::new(vec![String::new()]),
            fallbacks,
            role: role.clone(),
            role_save: role_save.clone(),
            save: save.clone(),
            reload: reload.clone(),
            feedback,
            snapshot: RefCell::new(None),
            role_revision: RefCell::new(String::new()),
            confirmation: RefCell::new(None),
            pending: RefCell::new(HashMap::new()),
            serial: Cell::new(0),
            applying: Cell::new(false),
            config_dirty: Cell::new(false),
            config_conflict: Cell::new(false),
            role_dirty: Cell::new(false),
        });
        let weak = Rc::downgrade(&panel);
        backend.connect_selected_notify(move |_| {
            if let Some(panel) = weak.upgrade() {
                if !panel.applying.get() {
                    panel.config_dirty.set(true);
                    panel.controller_label();
                    panel.controls();
                }
            }
        });
        for (dropdown, condition) in &panel.fallbacks {
            let weak = Rc::downgrade(&panel);
            dropdown.connect_selected_notify(move |_| {
                if let Some(panel) = weak.upgrade() {
                    if !panel.applying.get() {
                        panel.config_dirty.set(true);
                        panel.controls();
                    }
                }
            });
            let weak = Rc::downgrade(&panel);
            condition.buffer().connect_changed(move |_| {
                if let Some(panel) = weak.upgrade() {
                    if !panel.applying.get() {
                        panel.config_dirty.set(true);
                        panel.controls();
                    }
                }
            });
        }
        let weak = Rc::downgrade(&panel);
        role.buffer().connect_changed(move |_| {
            if let Some(panel) = weak.upgrade() {
                if !panel.applying.get() {
                    panel.role_dirty.set(true);
                    panel.controls();
                }
            }
        });
        let weak = Rc::downgrade(&panel);
        setup.connect_clicked(move |_| {
            if let Some(panel) = weak.upgrade() {
                panel.setup_controller();
            }
        });
        let weak = Rc::downgrade(&panel);
        recover.connect_clicked(move |_| {
            if let Some(panel) = weak.upgrade() {
                panel.open_recovery();
            }
        });
        let weak = Rc::downgrade(&panel);
        save.connect_clicked(move |_| {
            if let Some(panel) = weak.upgrade() {
                panel.save_config();
            }
        });
        let weak = Rc::downgrade(&panel);
        role_save.connect_clicked(move |_| {
            if let Some(panel) = weak.upgrade() {
                panel.save_role();
            }
        });
        let weak = Rc::downgrade(&panel);
        reload.connect_clicked(move |_| {
            if let Some(panel) = weak.upgrade() {
                panel.confirm_discard(true);
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.dialog.connect_close_attempt(move |_| {
            if let Some(panel) = weak.upgrade() {
                panel.confirm_discard(false);
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.dialog.connect_closed(move |_| {
            if let Some(panel) = weak.upgrade() {
                let _ = panel
                    .sender
                    .try_send(Command::RouterConfigCancel { dialog: panel.id });
            }
        });
        panel.controls();
        panel
    }
    pub fn begin(self: &Rc<Self>) {
        if self.saving() {
            return;
        }
        let _ = self
            .sender
            .try_send(Command::RouterConfigCancel { dialog: self.id });
        self.pending.borrow_mut().clear();
        self.feedback
            .set_text("Loading Router settings and session targets…");
        self.catalog_status
            .set_text("Discovering fallback sessions…");
        self.send(Pending::Read, Request::Read);
        self.send(Pending::Targets, Request::Targets(self.backends.clone()));
    }
    fn send(self: &Rc<Self>, pending: Pending, request: Request) {
        let serial = self.serial.get() + 1;
        self.serial.set(serial);
        if self
            .sender
            .try_send(Command::RouterConfig {
                dialog: self.id,
                serial,
                request,
            })
            .is_err()
        {
            self.feedback.set_text("The backend is busy; try again");
            if let Pending::Recovery(id, _) = pending {
                let child = self.recovery.borrow().clone().filter(|c| c.id == id);
                if let Some(child) = child {
                    child.event(Err("Native recovery workers are busy; try again".into()));
                }
            }
            return;
        }
        self.pending.borrow_mut().insert(serial, pending);
        self.controls();
        let weak = Rc::downgrade(self);
        glib::timeout_add_local_once(std::time::Duration::from_secs(45), move || {
            if let Some(panel) = weak.upgrade() {
                let expired = panel.pending.borrow_mut().remove(&serial);
                if let Some(expired) = expired {
                    if let Pending::Recovery(id, _) = expired {
                        if let Some(child) = panel.recovery.borrow().clone().filter(|c| c.id == id)
                        {
                            child.event(Err("Recovery result is unknown. Reload settings before retrying; creation was not repeated.".into()));
                        }
                    }
                    panel
                        .feedback
                        .set_text("Router operation timed out; reload before retrying a save");
                    panel.controls();
                }
            }
        });
    }
    fn saving(&self) -> bool {
        self.pending.borrow().values().any(|pending| {
            matches!(
                pending,
                Pending::Save(_)
                    | Pending::Role(_)
                    | Pending::Controller
                    | Pending::Recovery(_, true)
            )
        })
    }
    fn controls(&self) {
        let loaded = self.snapshot.borrow().is_some();
        let loading = self
            .pending
            .borrow()
            .values()
            .any(|pending| matches!(pending, Pending::Read));
        let saving = self.saving();
        let supported = self
            .backends
            .get(self.backend.selected() as usize)
            .is_some_and(|backend| matches!(backend.kind.as_str(), "codex" | "opencode"));
        self.setup
            .set_sensitive(loaded && supported && !saving && !loading);
        self.setup.set_tooltip_text(None);
        self.recover
            .set_sensitive(loaded && supported && !saving && !loading);
        self.recover.set_visible(
            self.backends
                .get(self.backend.selected() as usize)
                .is_some_and(|b| {
                    self.snapshot
                        .borrow()
                        .as_ref()
                        .is_some_and(|s| !s.controllers.contains_key(&b.id))
                }),
        );
        self.backend.set_sensitive(loaded && !loading);
        self.role.set_sensitive(loaded && !loading);
        for (target, condition) in &self.fallbacks {
            target.set_sensitive(loaded && !loading);
            condition.set_sensitive(loaded && !loading);
            if let Some(scroll) = condition.parent() {
                let selected = self
                    .keys
                    .borrow()
                    .get(target.selected() as usize)
                    .is_some_and(|key| !key.is_empty());
                scroll.set_visible(selected);
            }
        }
        self.save.set_sensitive(
            loaded && self.config_dirty.get() && !self.config_conflict.get() && !saving && !loading,
        );
        self.role_save
            .set_sensitive(loaded && self.role_dirty.get() && !saving && !loading);
        self.reload.set_sensitive(!saving);
        self.dialog
            .set_can_close(!saving && !self.config_dirty.get() && !self.role_dirty.get());
    }
    fn settings(&self) -> Result<Settings, String> {
        if self
            .fallbacks
            .iter()
            .any(|(_, text)| text.buffer().char_count() > 4096)
        {
            return Err("Each fallback condition supports at most 4,096 characters".into());
        }
        let backend = self
            .backends
            .get(self.backend.selected() as usize)
            .ok_or("Choose a Router backend")?
            .id
            .clone();
        let keys = self.keys.borrow();
        let fallbacks = self
            .fallbacks
            .iter()
            .filter_map(|(dropdown, condition)| {
                keys.get(dropdown.selected() as usize)
                    .filter(|key| !key.is_empty())
                    .map(|key| Fallback {
                        session_key: key.clone(),
                        condition: value(condition),
                    })
            })
            .collect();
        let settings = Settings { backend, fallbacks };
        settings.validate()?;
        Ok(settings)
    }
    fn selected_keys(&self) -> Vec<String> {
        let keys = self.keys.borrow();
        self.fallbacks
            .iter()
            .map(|(dropdown, _)| {
                keys.get(dropdown.selected() as usize)
                    .cloned()
                    .unwrap_or_default()
            })
            .collect()
    }
    fn targets(&self, selected: Vec<String>) {
        let snapshot = self.snapshot.borrow();
        let controllers = snapshot
            .as_ref()
            .map(|snapshot| {
                snapshot
                    .controllers
                    .iter()
                    .map(|(backend, id)| format!("{backend}:{id}"))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut keys = vec![String::new()];
        let mut labels = vec!["No fallback".to_owned()];
        for target in self.targets.borrow().iter() {
            let key = target.key();
            if !controllers.contains(&key) {
                keys.push(key);
                labels.push(format!(
                    "[{}] {} — {}",
                    target.backend, target.thread.title, target.thread.cwd
                ));
            }
        }
        for key in &selected {
            if !key.is_empty() && !keys.contains(key) {
                keys.push(key.clone());
                labels.push(format!("Unavailable · {key}"));
            }
        }
        drop(snapshot);
        self.applying.set(true);
        *self.keys.borrow_mut() = keys;
        let rows = labels.iter().map(String::as_str).collect::<Vec<_>>();
        let model = gtk::StringList::new(&rows);
        for (index, (dropdown, _)) in self.fallbacks.iter().enumerate() {
            dropdown.set_model(Some(&model));
            let position = selected
                .get(index)
                .and_then(|key| {
                    self.keys
                        .borrow()
                        .iter()
                        .position(|candidate| candidate == key)
                })
                .unwrap_or(0);
            dropdown.set_selected(position as u32);
        }
        self.applying.set(false);
    }
    fn apply_config(&self) {
        let fields = {
            let snapshot = self.snapshot.borrow();
            snapshot.as_ref().map(|snapshot| {
                (
                    snapshot.controller_backend.clone(),
                    snapshot.fallbacks.clone(),
                )
            })
        };
        let Some((backend, fallbacks)) = fields else {
            return;
        };
        self.applying.set(true);
        self.backend.set_selected(
            self.backends
                .iter()
                .position(|candidate| candidate.id == backend)
                .map(|index| index as u32)
                .unwrap_or(gtk::INVALID_LIST_POSITION),
        );
        for (index, (_, condition)) in self.fallbacks.iter().enumerate() {
            condition.buffer().set_text(
                fallbacks
                    .get(index)
                    .map(|entry| entry.condition.as_str())
                    .unwrap_or(config::DEFAULT_CONDITION),
            );
        }
        self.applying.set(false);
        let mut keys = fallbacks
            .iter()
            .map(|entry| entry.session_key.clone())
            .collect::<Vec<_>>();
        keys.resize(3, String::new());
        self.targets(keys);
        self.config_dirty.set(false);
        self.config_conflict.set(false);
        self.controller_label();
    }
    fn controller_label(&self) {
        let configured = self
            .backends
            .get(self.backend.selected() as usize)
            .is_some_and(|backend| {
                self.snapshot
                    .borrow()
                    .as_ref()
                    .is_some_and(|snapshot| snapshot.controllers.contains_key(&backend.id))
            });
        self.setup.set_label(if configured {
            "Verify controller"
        } else {
            "Set up controller"
        });
        let text = self
            .backends
            .get(self.backend.selected() as usize)
            .map(|backend| {
                self.snapshot
                    .borrow()
                    .as_ref()
                    .and_then(|snapshot| snapshot.controllers.get(&backend.id))
                    .map(|id| format!("Controller session: {id}"))
                    .unwrap_or_else(|| {
                        "No controller session is configured for this backend.".into()
                    })
            })
            .unwrap_or_else(|| "The saved Router backend is unavailable.".into());
        self.controller.set_text(&text);
    }
    fn apply_role(&self) {
        let Some(context) = &self.role_context else {
            return;
        };
        let fields = {
            let snapshot = self.snapshot.borrow();
            snapshot.as_ref().map(|snapshot| {
                snapshot
                    .openings
                    .get(&context.key)
                    .map(|opening| {
                        (
                            opening.responsibility.clone(),
                            opening.revision.clone(),
                            opening
                                .opening_message
                                .chars()
                                .take(180)
                                .collect::<String>(),
                        )
                    })
                    .unwrap_or_else(|| {
                        (
                            String::new(),
                            snapshot.empty_opening_revision.clone(),
                            String::new(),
                        )
                    })
            })
        };
        if let Some((text, revision, excerpt)) = fields {
            self.applying.set(true);
            self.role.buffer().set_text(&text);
            self.role
                .set_tooltip_text((!excerpt.is_empty()).then_some(excerpt.as_str()));
            *self.role_revision.borrow_mut() = revision;
            self.applying.set(false);
            self.role_dirty.set(false);
        }
    }
    fn setup_controller(self: &Rc<Self>) {
        if self.saving() {
            return;
        }
        let Some(backend) = self
            .backends
            .get(self.backend.selected() as usize)
            .filter(|backend| matches!(backend.kind.as_str(), "codex" | "opencode"))
        else {
            return;
        };
        let revision = self
            .snapshot
            .borrow()
            .as_ref()
            .map(|snapshot| snapshot.revision.clone());
        let Some(revision) = revision else {
            return;
        };
        self.feedback
            .set_text("Setting up or verifying the Router controller…");
        self.send(
            Pending::Controller,
            Request::Controller {
                revision,
                backend: backend.id.clone(),
            },
        );
    }
    fn open_recovery(self: &Rc<Self>) {
        if self.saving() {
            return;
        }
        if let Some(child) = self.recovery.borrow().clone() {
            child.dialog.present(Some(&self.dialog));
            return;
        }
        let Some(backend) = self
            .backends
            .get(self.backend.selected() as usize)
            .filter(|b| matches!(b.kind.as_str(), "codex" | "opencode"))
        else {
            return;
        };
        let Some(revision) = self.snapshot.borrow().as_ref().map(|s| s.revision.clone()) else {
            return;
        };
        let child = recovery::Recovery::new(self, backend.id.clone(), revision);
        self.recovery.replace(Some(child.clone()));
        child.dialog.present(Some(&self.dialog));
        child.reload();
        self.send(Pending::Targets, Request::Targets(self.backends.clone()));
    }
    #[cfg(debug_assertions)]
    pub fn smoke_target_keys(&self) -> Option<Vec<String>> {
        (self.pending.borrow().is_empty() && self.snapshot.borrow().is_some())
            .then(|| self.targets.borrow().iter().map(Target::key).collect())
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_recovery(
        self: &Rc<Self>,
        directory: &std::path::Path,
        restore: bool,
        capture: impl Fn(&adw::Dialog, &str),
    ) -> Result<serde_json::Value, String> {
        let start = std::time::Instant::now();
        while !self.pending.borrow().is_empty() || self.snapshot.borrow().is_none() {
            if start.elapsed() > std::time::Duration::from_secs(12) {
                return Err("Router recovery settings did not load".into());
            }
            glib::timeout_future(std::time::Duration::from_millis(20)).await;
        }
        if restore {
            if self
                .snapshot
                .borrow()
                .as_ref()
                .unwrap()
                .controllers
                .get("codex")
                .map(String::as_str)
                != Some("router")
            {
                return Err("Adopted identity did not survive restart".into());
            }
            self.setup.emit_clicked();
            let start = std::time::Instant::now();
            while self.saving() {
                if start.elapsed() > std::time::Duration::from_secs(12) {
                    return Err("Adopted controller verification did not finish".into());
                }
                glib::timeout_future(std::time::Duration::from_millis(20)).await;
            }
            if !self.feedback.text().contains("ready") {
                return Err(self.feedback.text().to_string());
            }
            self.dialog.force_close();
            return Ok(serde_json::json!({"ok":true,"restored":true,"verified":true}));
        }
        self.fallbacks[0]
            .1
            .buffer()
            .set_text("Keep unsaved fallback condition 中文😀");
        self.role
            .buffer()
            .set_text("Keep unsaved responsibility 中文😀");
        let settings = self.settings()?;
        let role = value(&self.role);
        self.recover.emit_clicked();
        let child = self
            .recovery
            .borrow()
            .clone()
            .ok_or("Recovery dialog missing")?;
        let mut result = child.smoke(directory, capture).await?;
        if self.settings()? != settings
            || value(&self.role) != role
            || !self.config_dirty.get()
            || !self.role_dirty.get()
        {
            return Err("Controller adoption changed unsaved settings or responsibility".into());
        }
        let mut concurrent = self.snapshot.borrow().clone().unwrap();
        concurrent.fallbacks[0].condition = "Changed in another client".into();
        let serial = self.serial.get() + 1;
        self.serial.set(serial);
        self.pending
            .borrow_mut()
            .insert(serial, Pending::Controller);
        self.event(
            serial,
            Ok(Reply::Controller {
                snapshot: concurrent,
                thread_id: "router".into(),
                warning: None,
            }),
        );
        if !self.config_conflict.get()
            || self.save.is_sensitive()
            || self.settings()? != settings
            || value(&self.role) != role
        {
            return Err("Controller acknowledgement silently rebased unsaved settings over concurrent edits".into());
        }
        result["concurrentConfigGuard"] = serde_json::json!(true);
        self.config_dirty.set(false);
        self.role_dirty.set(false);
        self.dialog.force_close();
        Ok(result)
    }
    fn save_config(self: &Rc<Self>) {
        if self.config_conflict.get() {
            self.feedback.set_text("Router settings changed in another client. Reload and review before saving; local edits are retained.");
            return;
        }
        if self.saving() {
            return;
        }
        let settings = match self.settings() {
            Ok(settings) => settings,
            Err(error) => {
                self.feedback.set_text(&error);
                return;
            }
        };
        let revision = self
            .snapshot
            .borrow()
            .as_ref()
            .map(|snapshot| snapshot.revision.clone());
        if let Some(revision) = revision {
            self.send(
                Pending::Save(settings.clone()),
                Request::Save { revision, settings },
            );
        }
    }
    fn save_role(self: &Rc<Self>) {
        if self.saving() {
            return;
        }
        if self.role.buffer().char_count() > crate::session_info::ROLE_CHARS as i32 {
            self.feedback
                .set_text("Responsibilities support at most 4,096 characters");
            return;
        }
        if !self
            .role_valid
            .borrow()
            .as_ref()
            .is_some_and(|valid| valid())
        {
            self.feedback.set_text("The session or connection changed; reopen Router settings before editing its responsibility");
            return;
        }
        let Some(context) = &self.role_context else {
            return;
        };
        let text = value(&self.role);
        self.send(
            Pending::Role(text.clone()),
            Request::Responsibility {
                key: context.key.clone(),
                revision: self.role_revision.borrow().clone(),
                text,
            },
        );
    }
    pub fn event(&self, serial: u64, result: Result<Reply, String>) {
        let Some(pending) = self.pending.borrow_mut().remove(&serial) else {
            return;
        };
        let recovery_id = match &pending {
            Pending::Recovery(id, _) => Some(*id),
            _ => None,
        };
        let recovered = matches!(&result, Ok(Reply::Controller { .. })) && recovery_id.is_some();
        match (pending, result) {
            (
                Pending::Controller | Pending::Recovery(_, true),
                Ok(Reply::Controller {
                    snapshot,
                    thread_id,
                    warning,
                }),
            ) => {
                let conflict = self.config_dirty.get()
                    && self.snapshot.borrow().as_ref().is_some_and(|old| {
                        old.controller_backend != snapshot.controller_backend
                            || old.fallbacks != snapshot.fallbacks
                    });
                if conflict {
                    self.config_conflict.set(true);
                }
                self.snapshot.replace(Some(snapshot));
                if !self.config_dirty.get() {
                    self.apply_config();
                }
                if !self.role_dirty.get() {
                    self.apply_role();
                }
                self.controller_label();
                self.targets(self.selected_keys());
                let mut message =
                    warning.unwrap_or_else(|| format!("Router controller ready: {thread_id}"));
                if conflict {
                    message.push_str("\nOther Router settings changed while the controller was registered. Reload and review before saving; local edits are retained.");
                }
                self.feedback.set_text(&message);
            }
            (Pending::Read, Ok(Reply::Snapshot(snapshot))) => {
                self.snapshot.replace(Some(snapshot));
                self.apply_config();
                self.apply_role();
                self.feedback.set_text("Router settings loaded");
            }
            (Pending::Targets, Ok(Reply::Targets { targets, issues })) => {
                let selected = self.selected_keys();
                self.targets.replace(targets);
                if let Some(child) = self.recovery.borrow().clone() {
                    child.choices();
                }
                self.targets(selected);
                self.catalog_status.set_text(&if issues.is_empty() {
                    format!(
                        "{} session choices",
                        self.keys.borrow().len().saturating_sub(1)
                    )
                } else {
                    issues.join("\n")
                });
            }
            (Pending::Save(sent), Ok(Reply::Snapshot(snapshot))) => {
                let matches = self.settings().ok().as_ref() == Some(&sent);
                self.snapshot.replace(Some(snapshot));
                if matches {
                    self.apply_config();
                }
                if !self.role_dirty.get() {
                    self.apply_role();
                }
                self.feedback.set_text("Router settings saved");
            }
            (Pending::Role(sent), Ok(Reply::Responsibility(saved))) => {
                if self
                    .role_context
                    .as_ref()
                    .is_some_and(|context| context.key == saved.session_key)
                {
                    *self.role_revision.borrow_mut() = saved.opening.revision.clone();
                    if let Some(snapshot) = self.snapshot.borrow_mut().as_mut() {
                        snapshot.openings.insert(saved.session_key, saved.opening);
                    }
                    self.role_dirty.set(value(&self.role) != sent);
                    self.feedback.set_text("Responsibility saved");
                }
            }
            (Pending::Targets, Err(error)) => self.catalog_status.set_text(&error),
            (Pending::Recovery(id, _), result) => {
                if let Some(child) = self.recovery.borrow().clone().filter(|c| c.id == id) {
                    child.event(result);
                }
            }
            (_, Err(error)) => self.feedback.set_text(&error),
            _ => self.feedback.set_text("Unexpected Router response"),
        }
        if recovered {
            let child = self
                .recovery
                .borrow()
                .clone()
                .filter(|c| Some(c.id) == recovery_id);
            if let Some(child) = child {
                child.completed();
            }
        }
        self.controls();
    }
    fn confirm_discard(self: &Rc<Self>, reload: bool) {
        if self.saving() {
            self.feedback.set_text("Wait for the save to finish");
            return;
        }
        if !self.config_dirty.get() && !self.role_dirty.get() {
            if reload {
                self.begin();
            } else {
                self.dialog.force_close();
            }
            return;
        }
        let previous = self.confirmation.borrow().clone();
        if let Some(previous) = previous {
            previous.present(Some(&self.window));
            return;
        }
        let confirm = adw::AlertDialog::builder()
            .heading("Discard unsaved Router changes?")
            .body("Changes in this dialog have not been saved.")
            .build();
        confirm.add_responses(&[("cancel", "Keep editing"), ("discard", "Discard changes")]);
        confirm.set_default_response(Some("cancel"));
        confirm.set_close_response("cancel");
        confirm.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
        let weak = Rc::downgrade(self);
        confirm.connect_response(None, move |_, response| {
            if let Some(panel) = weak.upgrade() {
                panel.confirmation.borrow_mut().take();
            }
            if response == "discard" {
                if let Some(panel) = weak.upgrade() {
                    panel.config_dirty.set(false);
                    panel.role_dirty.set(false);
                    panel.apply_config();
                    panel.apply_role();
                    panel.controls();
                    if reload {
                        panel.begin();
                    } else {
                        panel.dialog.force_close();
                    }
                }
            }
        });
        self.confirmation.replace(Some(confirm.clone()));
        confirm.present(Some(&self.window));
    }
    pub fn allow_window_close(&self) -> bool {
        if self.saving() || self.role_dirty.get() || self.config_dirty.get() {
            self.feedback
                .set_text("Save or discard Router changes before closing Studio");
            self.dialog.present(Some(&self.window));
            false
        } else {
            true
        }
    }
}

#[cfg(debug_assertions)]
async fn until(mut ready: impl FnMut() -> bool) -> Result<(), String> {
    let start = std::time::Instant::now();
    while !ready() {
        if start.elapsed() > std::time::Duration::from_secs(9) {
            return Err("Native Router settings operation timed out".into());
        }
        glib::timeout_future(std::time::Duration::from_millis(20)).await;
    }
    Ok(())
}
#[cfg(debug_assertions)]
impl Panel {
    fn confirmation(&self) -> Option<adw::AlertDialog> {
        self.confirmation.borrow().clone()
    }
    fn respond_confirmation(&self, response: &str) -> Result<(), String> {
        fn find(widget: &gtk::Widget, label: &str) -> Option<gtk::Button> {
            if let Some(button) = widget.downcast_ref::<gtk::Button>() {
                if button.label().as_deref() == Some(label) {
                    return Some(button.clone());
                }
            }
            let mut child = widget.first_child();
            while let Some(widget) = child {
                child = widget.next_sibling();
                if let Some(button) = find(&widget, label) {
                    return Some(button);
                }
            }
            None
        }
        let dialog = self
            .confirmation()
            .ok_or("Router discard confirmation was not shown")?;
        let label = if response == "discard" {
            "Discard changes"
        } else {
            "Keep editing"
        };
        find(dialog.upcast_ref(), label)
            .ok_or("Router discard response button was not found")?
            .emit_clicked();
        Ok(())
    }
    pub fn smoke_cleanup(&self) {
        if let Some(dialog) = self.confirmation() {
            dialog.force_close();
        }
        self.config_dirty.set(false);
        self.role_dirty.set(false);
        self.pending.borrow_mut().clear();
        self.dialog.force_close();
    }
    pub fn smoke_diagnostics(&self) -> String {
        format!(
            "feedback={}, configDirty={}, roleDirty={}, pending={}, confirmation={}",
            self.feedback.text(),
            self.config_dirty.get(),
            self.role_dirty.get(),
            self.pending.borrow().len(),
            self.confirmation().is_some()
        )
    }
    fn select_target(&self, index: usize, key: &str) -> Result<(), String> {
        let position = self
            .keys
            .borrow()
            .iter()
            .position(|candidate| candidate == key)
            .ok_or_else(|| {
                format!(
                    "Router target {key} was not discovered; catalog: {}; keys: {:?}",
                    self.catalog_status.text(),
                    self.keys.borrow()
                )
            })?;
        self.fallbacks[index].0.set_selected(position as u32);
        Ok(())
    }
    pub async fn smoke_test(
        self: &Rc<Self>,
        directory: &std::path::Path,
        capture: impl Fn(&adw::Dialog, &str),
    ) -> Result<serde_json::Value, String> {
        until(|| {
            self.snapshot.borrow().is_some()
                && self.pending.borrow().is_empty()
                && self
                    .keys
                    .borrow()
                    .iter()
                    .any(|key| key == "opencode:native-oc-thread")
        })
        .await?;
        if value(&self.role) != "Original responsibility" {
            return Err("The selected session responsibility was not loaded".into());
        }
        self.setup.emit_clicked();
        until(|| !self.saving()).await?;
        if !self
            .feedback
            .text()
            .contains("Router controller ready: native-managed-router-id")
        {
            return Err(format!(
                "Configured Router verification failed: {}",
                self.feedback.text()
            ));
        }
        self.role
            .buffer()
            .set_text("Native saved responsibility 中文😀");
        self.role_save.emit_clicked();
        self.role
            .buffer()
            .set_text("Native newer responsibility 中文😀");
        until(|| !self.saving()).await?;
        if value(&self.role) != "Native newer responsibility 中文😀" || !self.role_dirty.get() {
            return Err("An asynchronous role save overwrote newer editor text".into());
        }
        self.role_save.emit_clicked();
        until(|| !self.saving() && !self.role_dirty.get()).await?;
        self.select_target(0, "codex:native-cache-thread")?;
        self.select_target(1, "codex:native-cache-thread")?;
        self.fallbacks[0]
            .1
            .buffer()
            .set_text("Fallback 中文 condition");
        self.fallbacks[1].1.buffer().set_text("Other condition");
        self.save.emit_clicked();
        if !self.feedback.text().contains("twice") {
            return Err("Duplicate fallback targets were not rejected".into());
        }
        self.select_target(1, "opencode:native-oc-thread")?;
        let opencode = self
            .backends
            .iter()
            .position(|backend| backend.id == "opencode")
            .ok_or("No OpenCode Router backend")?;
        self.backend.set_selected(opencode as u32);
        self.setup.emit_clicked();
        until(|| !self.saving()).await?;
        if !self
            .feedback
            .text()
            .contains("Router controller ready: native-managed-opencode-id")
            || self.backend.selected() != opencode as u32
            || !self.config_dirty.get()
        {
            return Err(format!(
                "OpenCode verification overwrote controller form edits: {}",
                self.feedback.text()
            ));
        }
        self.save.emit_clicked();
        self.fallbacks[0]
            .1
            .buffer()
            .set_text("Newer fallback 中文 condition");
        until(|| !self.saving()).await?;
        if value(&self.fallbacks[0].1) != "Newer fallback 中文 condition"
            || !self.config_dirty.get()
        {
            return Err("Router save discarded a newer fallback edit".into());
        }
        self.save.emit_clicked();
        until(|| !self.saving() && !self.config_dirty.get()).await?;
        // A completed model write does not imply that the dialog painted it.
        // Observe its normal GTK frame before taking the debug screenshot.
        crate::window_capture::ready(&self.dialog.child().ok_or("Router dialog has no content")?)
            .await?;
        capture(&self.dialog, "router-settings.png");
        let path = directory.join("config/codex-thread-studio/settings.json");
        let mut preferences: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        if preferences["queueDepth"] != 3
            || preferences["contentWidth"] != "wide"
            || preferences["router"]["controllers"]["codex"] != "native-managed-router-id"
        {
            return Err(
                "Scoped Router settings replaced unrelated preferences or a controller ID".into(),
            );
        }
        preferences["router"]["fallbacks"][0]["condition"] = "External Router change".into();
        std::fs::write(&path, serde_json::to_vec(&preferences).unwrap())
            .map_err(|e| e.to_string())?;
        self.fallbacks[0]
            .1
            .buffer()
            .set_text("Keep after Router conflict");
        self.save.emit_clicked();
        until(|| !self.saving() && self.feedback.text().contains("changed")).await?;
        if value(&self.fallbacks[0].1) != "Keep after Router conflict" || !self.config_dirty.get() {
            return Err("A stale Router save discarded editor input".into());
        }
        self.reload.emit_clicked();
        until(|| self.confirmation().is_some()).await?;
        self.respond_confirmation("cancel")?;
        until(|| self.confirmation().is_none()).await?;
        if value(&self.fallbacks[0].1) != "Keep after Router conflict" {
            return Err("Cancelling reload discarded Router edits".into());
        }
        self.window.close();
        if !self.window.is_visible() {
            return Err("Studio closed with dirty Router settings".into());
        }
        if self.confirmation().is_some() {
            self.respond_confirmation("cancel")?;
            until(|| self.confirmation().is_none()).await?;
        }
        self.reload.emit_clicked();
        until(|| self.confirmation().is_some()).await?;
        self.respond_confirmation("discard")?;
        until(|| self.confirmation().is_none()).await?;
        until(|| self.pending.borrow().is_empty() && !self.config_dirty.get()).await?;
        if value(&self.fallbacks[0].1) != "External Router change" {
            return Err("Reload did not read the authoritative Router configuration".into());
        }
        *self.role_revision.borrow_mut() = "stale-responsibility-editor".into();
        self.role
            .buffer()
            .set_text("Keep after responsibility conflict");
        self.role_save.emit_clicked();
        until(|| !self.saving() && self.feedback.text().contains("changed")).await?;
        if value(&self.role) != "Keep after responsibility conflict" || !self.role_dirty.get() {
            return Err("A failed responsibility save discarded editor text".into());
        }
        self.reload.emit_clicked();
        until(|| self.confirmation().is_some()).await?;
        self.respond_confirmation("discard")?;
        until(|| self.pending.borrow().is_empty() && !self.role_dirty.get()).await?;
        if value(&self.role) != "Native newer responsibility 中文😀" {
            return Err("Reload lost the saved responsibility".into());
        }
        Ok(
            serde_json::json!({"ok":true,"opencodeControllerRecovery":true,"controllerRecovery":true,"crossBackendTargets":true,"responsibilityMetadata":true,"fallbackOrder":true,"duplicateFallback":true,"controllerIdentity":true,"unrelatedPreferences":true,"newerRoleRetention":true,"newerConfigRetention":true,"staleConfigRetention":true,"staleRoleRetention":true,"discardCancellation":true,"parentCloseGuard":true}),
        )
    }
    pub async fn smoke_scope_failure(self: &Rc<Self>) -> Result<(), String> {
        self.role
            .buffer()
            .set_text("Must not save to a changed session");
        self.role_save.emit_clicked();
        if !self
            .feedback
            .text()
            .contains("session or connection changed")
            || self.saving()
        {
            return Err("A stale responsibility editor wrote to another session".into());
        }
        self.dialog.close();
        until(|| self.confirmation().is_some()).await?;
        self.respond_confirmation("discard")?;
        Ok(())
    }
}
