//! Inspect before adoption; all backend work belongs to the Rust gateway.
use super::*;
use std::rc::Weak;
#[cfg(debug_assertions)]
#[path = "router_recovery_acceptance.rs"]
mod acceptance;
pub(super) struct Recovery {
    pub id: u64,
    pub dialog: adw::Dialog,
    owner: Weak<Panel>,
    backend: String,
    revision: String,
    entry: gtk::Entry,
    selector: gtk::DropDown,
    keys: RefCell<Vec<String>>,
    inspect: gtk::Button,
    adopt: gtk::Button,
    reload: gtk::Button,
    details: gtk::Label,
    status: gtk::Label,
    snapshot: RefCell<Option<config::Recovery>>,
    reviewed: RefCell<Option<config::RecoveryCandidate>>,
    expected: RefCell<Option<String>>,
    busy: Cell<bool>,
    mutating: Cell<bool>,
    applying: Cell<bool>,
}
impl Recovery {
    pub fn new(owner: &Rc<Panel>, backend: String, revision: String) -> Rc<Self> {
        let dialog = adw::Dialog::builder()
            .title("Recover Router controller")
            .content_width(650)
            .content_height(460)
            .build();
        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        let reload = gtk::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text("Reload recovery state")
            .build();
        header.pack_end(&reload);
        toolbar.add_top_bar(&header);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
        body.set_margin_start(20);
        body.set_margin_end(20);
        body.set_margin_bottom(20);
        let caption=gtk::Label::builder().label(format!("Adopt an existing {backend} session from the interrupted setup. Inspect its identity before registering it as the controller.")).wrap(true).xalign(0.0).build();
        body.append(&caption);
        let selector = gtk::DropDown::from_strings(&["Enter an existing session ID"]);
        selector.set_enable_search(true);
        selector.set_tooltip_text(Some("Choose from up to 100 matching sessions in the loaded catalog, or enter an existing session ID."));
        crate::adaptive::string_selector(&selector, 48);
        body.append(&selector);
        let entry = gtk::Entry::builder()
            .placeholder_text("Existing session ID")
            .max_length(256)
            .hexpand(true)
            .build();
        entry.update_property(&[gtk::accessible::Property::Label(
            "Existing controller session ID",
        )]);
        body.append(&entry);
        let inspect = gtk::Button::with_label("Inspect session");
        inspect.set_halign(gtk::Align::Start);
        body.append(&inspect);
        let details = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .selectable(true)
            .build();
        body.append(&details);
        let status = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .selectable(true)
            .build();
        status.add_css_class("dim-label");
        body.append(&status);
        let notice=gtk::Label::builder().label(if backend=="opencode"{"Adoption keeps the active backend and fallback rules. OpenCode controller tools will be disabled."}else{"Adoption keeps the active backend and fallback rules."}).xalign(0.0).wrap(true).build();
        notice.add_css_class("caption");
        body.append(&notice);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
        content.append(
            &gtk::ScrolledWindow::builder()
                .child(&body)
                .vexpand(true)
                .hscrollbar_policy(gtk::PolicyType::Never)
                .build(),
        );
        let adopt = gtk::Button::with_label("Adopt controller");
        adopt.add_css_class("suggested-action");
        adopt.set_halign(gtk::Align::End);
        adopt.set_margin_end(20);
        adopt.set_margin_bottom(16);
        content.append(&adopt);
        toolbar.set_content(Some(&content));
        dialog.set_child(Some(&toolbar));
        let child = Rc::new(Self {
            id: NEXT.fetch_add(1, Ordering::Relaxed),
            dialog,
            owner: Rc::downgrade(owner),
            backend,
            revision,
            entry,
            selector,
            keys: RefCell::new(vec![String::new()]),
            inspect,
            adopt,
            reload,
            details,
            status,
            snapshot: RefCell::new(None),
            reviewed: RefCell::new(None),
            expected: RefCell::new(None),
            busy: Cell::new(false),
            mutating: Cell::new(false),
            applying: Cell::new(false),
        });
        let weak = Rc::downgrade(&child);
        child.reload.connect_clicked(move |_| {
            if let Some(c) = weak.upgrade() {
                c.reload();
            }
        });
        let weak = Rc::downgrade(&child);
        child.inspect.connect_clicked(move |_| {
            if let Some(c) = weak.upgrade() {
                let id = c.entry.text().to_string();
                c.send(false, Some(id));
            }
        });
        let weak = Rc::downgrade(&child);
        child.adopt.connect_clicked(move |_| {
            if let Some(c) = weak.upgrade() {
                let id = c.entry.text().to_string();
                c.send(true, Some(id));
            }
        });
        let weak = Rc::downgrade(&child);
        child.entry.connect_changed(move |_| {
            if let Some(c) = weak.upgrade() {
                c.reviewed.borrow_mut().take();
                c.controls();
            }
        });
        let weak = Rc::downgrade(&child);
        child.selector.connect_selected_notify(move |selector| {
            if let Some(c) = weak.upgrade() {
                if !c.applying.get() {
                    if let Some(id) = c
                        .keys
                        .borrow()
                        .get(selector.selected() as usize)
                        .filter(|id| !id.is_empty())
                    {
                        c.entry.set_text(id);
                    }
                }
            }
        });
        let weak = Rc::downgrade(&child);
        child.dialog.connect_closed(move |_| {
            if let Some(c) = weak.upgrade() {
                if let Some(owner) = c.owner.upgrade() {
                    let current = owner
                        .recovery
                        .borrow()
                        .as_ref()
                        .is_some_and(|live| live.id == c.id);
                    if current {
                        owner.recovery.borrow_mut().take();
                    }
                }
            }
        });
        child.controls();
        child
    }
    pub fn reload(&self) {
        if !self.busy.get() {
            self.send(false, None);
        }
    }
    fn send(&self, mutating: bool, id: Option<String>) {
        if self.busy.get() {
            return;
        }
        let Some(owner) = self.owner.upgrade() else {
            return;
        };
        let request = if mutating {
            let Some(snapshot) = self.snapshot.borrow().clone() else {
                return;
            };
            let Some(id) = id.clone().filter(|id| {
                self.reviewed
                    .borrow()
                    .as_ref()
                    .is_some_and(|c| c.thread_id == *id)
            }) else {
                return;
            };
            Request::Adopt {
                backend: self.backend.clone(),
                revision: self.revision.clone(),
                journal_revision: snapshot.journal_revision,
                thread_id: id,
            }
        } else {
            Request::Recovery {
                backend: self.backend.clone(),
                revision: self.revision.clone(),
                thread_id: id.clone(),
            }
        };
        self.expected.replace(id);
        self.busy.set(true);
        self.mutating.set(mutating);
        self.reviewed.borrow_mut().take();
        self.status.set_text(if mutating {
            "Adopting the inspected session…"
        } else {
            "Reading controller recovery metadata…"
        });
        self.controls();
        owner.send(Pending::Recovery(self.id, mutating), request);
    }
    fn controls(&self) {
        let available = self
            .snapshot
            .borrow()
            .as_ref()
            .is_some_and(|s| matches!(s.state.as_str(), "known" | "uncertain"));
        self.entry.set_sensitive(!self.mutating.get());
        self.selector.set_sensitive(!self.busy.get() && available);
        self.reload.set_sensitive(!self.busy.get());
        self.inspect
            .set_sensitive(available && !self.busy.get() && !self.entry.text().is_empty());
        self.adopt.set_sensitive(
            available
                && !self.busy.get()
                && self
                    .reviewed
                    .borrow()
                    .as_ref()
                    .is_some_and(|c| c.thread_id == self.entry.text()),
        );
        self.dialog.set_can_close(!self.mutating.get());
    }
    pub fn choices(&self) {
        let Some(owner) = self.owner.upgrade() else {
            return;
        };
        let workspace = self
            .snapshot
            .borrow()
            .as_ref()
            .and_then(|s| s.workspace.clone());
        let mut keys = vec![String::new()];
        let mut labels = vec!["Enter an existing session ID".to_owned()];
        for target in owner
            .targets
            .borrow()
            .iter()
            .filter(|t| {
                t.backend == self.backend && workspace.as_deref() == Some(t.thread.cwd.as_str())
            })
            .take(100)
        {
            keys.push(target.thread.id.clone());
            labels.push(format!("{} · {}", target.thread.title, target.thread.id));
        }
        self.applying.set(true);
        let selected = keys
            .iter()
            .position(|id| id == self.entry.text().as_str())
            .unwrap_or(0);
        self.keys.replace(keys);
        self.selector.set_model(Some(&gtk::StringList::new(
            &labels.iter().map(String::as_str).collect::<Vec<_>>(),
        )));
        self.selector.set_selected(selected as u32);
        self.applying.set(false);
    }
    pub fn event(&self, result: Result<Reply, String>) {
        self.busy.set(false);
        self.mutating.set(false);
        match result {
            Ok(Reply::Recovery(snapshot)) => {
                let current = self.entry.text().to_string();
                let expected = self.expected.borrow().clone();
                if let Some(candidate) = snapshot.candidate.clone().filter(|c| {
                    expected.as_deref() == Some(c.thread_id.as_str()) && current == c.thread_id
                }) {
                    self.details.set_text(&format!(
                        "{}\nSession: {}\nWorkspace: {}\nStatus: {}",
                        candidate.title, candidate.thread_id, candidate.workspace, candidate.status
                    ));
                    self.reviewed.replace(Some(candidate));
                    self.status.set_text(
                        "Session verified. Adopt controller to register this exact identity.",
                    );
                } else if expected.is_some() {
                    self.status
                        .set_text("The entered session changed. Inspect it again before adoption.");
                } else {
                    self.details.set_text(&format!(
                        "Backend: {}\nWorkspace: {}\nRecovery: {}",
                        snapshot.backend,
                        snapshot.workspace.as_deref().unwrap_or("Unavailable"),
                        snapshot.state
                    ));
                    self.status.set_text(match snapshot.state.as_str(){"uncertain"=>"Creation was interrupted before its identity was recorded. Select the existing session or enter its ID.","known"=>"The created identity was retained. Inspect it to finish registration.","configured"=>"A controller is already configured. Close recovery and reload Router settings.",_=>"No interrupted controller setup was found. Use Set up controller to create one."});
                    if current.is_empty() {
                        if let Some(id) = &snapshot.thread_id {
                            self.entry.set_text(id);
                        }
                    }
                }
                self.snapshot.replace(Some(snapshot));
                self.choices();
            }
            Err(error) => {
                self.reviewed.borrow_mut().take();
                self.status.set_text(&error);
            }
            _ => self.status.set_text("Unexpected controller recovery reply"),
        }
        self.controls();
    }
    pub fn completed(&self) {
        self.busy.set(false);
        self.mutating.set(false);
        self.controls();
        self.dialog.force_close();
    }
}
