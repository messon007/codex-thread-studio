//! Virtualized native Session Map; the shared gateway remains authoritative.
use crate::{
    protocol::Command,
    session_map::{self as data, types::*, Anchor, Snapshot},
    session_map_ai::{self as ai, Work},
    translation::Context,
};
use adw::prelude::*;
use gtk::glib;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::sync::mpsc;

pub struct Capture {
    pub context: Context,
    pub effort: String,
    pub source: Vec<Arc<crate::model::Item>>,
}
pub struct Panel {
    pub widget: gtk::Box,
    pub context: RefCell<Option<Box<dyn Fn() -> Option<Context>>>>,
    pub capture: RefCell<Option<Box<dyn Fn() -> Option<Capture>>>>,
    pub on_close: RefCell<Option<Box<dyn Fn()>>>,
    parent: glib::WeakRef<adw::ApplicationWindow>,
    sender: mpsc::Sender<Command>,
    anchor: RefCell<Option<(Anchor, u64)>>,
    snapshot: RefCell<Option<Arc<Snapshot>>>,
    serial: Cell<u64>,
    busy: Cell<bool>,
    generating: Cell<bool>,
    uncertain: Cell<bool>,
    external_change: Cell<bool>,
    sync_feedback: RefCell<Option<String>>,
    stop: RefCell<Option<Arc<AtomicBool>>>,
    rows: gtk::StringList,
    goal: gtk::Label,
    done: gtk::Label,
    trail: gtk::Label,
    status: gtk::Label,
    progress: gtk::Label,
    create: gtk::Button,
    generate: gtk::Button,
    reload: gtk::Button,
    undo: gtk::Button,
    delete: gtk::Button,
    cancel: gtk::Button,
    dialog: RefCell<Option<adw::Dialog>>,
}
fn literal(label: &gtk::Label, text: &str) {
    crate::i18n_ui::literal(label, "label", &text.replace('\0', "�"));
}
#[cfg(debug_assertions)]
#[path = "session_map_ui_acceptance.rs"]
mod acceptance;
impl Panel {
    pub fn new(parent: &adw::ApplicationWindow, sender: mpsc::Sender<Command>) -> Rc<Self> {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let (header, title) = crate::workspace_ui::header("Session Map");
        crate::i18n_ui::bind(&title, "label", "Session Map");
        let create = gtk::Button::with_label("Create Map");
        crate::i18n_ui::bind(&create, "label", "Create Map");
        let generate = gtk::Button::with_label("Generate with AI");
        crate::i18n_ui::bind(&generate, "label", "Generate with AI");
        let reload = crate::workspace_ui::button("view-refresh-symbolic", "Reload Map");
        let undo = crate::workspace_ui::button("edit-undo-symbolic", "Undo Map change");
        let delete = crate::workspace_ui::button("user-trash-symbolic", "Delete Map");
        let cancel = crate::workspace_ui::button("process-stop-symbolic", "Cancel Map generation");
        let close = crate::workspace_ui::button("window-close-symbolic", "Close Session Map");
        header.append(&reload);
        header.append(&close);
        widget.append(&header);
        let actions = crate::adaptive::toolbar(6);
        actions.set_margin_start(12);
        actions.set_margin_end(12);
        for b in [&create, &generate, &undo, &delete, &cancel] {
            actions.append(b);
        }
        widget.append(&actions);
        let summary = gtk::Box::new(gtk::Orientation::Vertical, 8);
        summary.set_margin_start(18);
        summary.set_margin_end(18);
        summary.set_margin_top(12);
        summary.set_margin_bottom(12);
        let label = || {
            gtk::Label::builder()
                .xalign(0.0)
                .wrap(true)
                .wrap_mode(gtk::pango::WrapMode::WordChar)
                .selectable(true)
                .build()
        };
        let goal = label();
        goal.add_css_class("heading");
        let done = label();
        let trail = label();
        trail.add_css_class("dim-label");
        summary.append(&goal);
        summary.append(&done);
        summary.append(&trail);
        widget.append(&summary);
        let rows = gtk::StringList::new(&[]);
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, object| {
            let item = object.downcast_ref::<gtk::ListItem>().unwrap();
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.set_margin_top(6);
            row.set_margin_bottom(6);
            row.set_margin_end(12);
            let title = gtk::Button::builder()
                .has_frame(false)
                .hexpand(true)
                .halign(gtk::Align::Fill)
                .build();
            let text = gtk::Label::builder()
                .wrap(true)
                .wrap_mode(gtk::pango::WrapMode::WordChar)
                .xalign(0.0)
                .build();
            title.set_child(Some(&text));
            let done = crate::workspace_ui::button("object-select-symbolic", "Mark done");
            row.append(&title);
            row.append(&done);
            item.set_child(Some(&row));
        });
        let list = gtk::ListView::new(
            Some(gtk::NoSelection::new(Some(rows.clone()))),
            Some(factory.clone()),
        );
        list.add_css_class("navigation-sidebar");
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&list)
            .build();
        widget.append(&scroll);
        let status = crate::workspace_ui::footer();
        let progress = crate::workspace_ui::footer();
        progress.add_css_class("dim-label");
        widget.append(&progress);
        status.set_wrap(true);
        status.set_ellipsize(gtk::pango::EllipsizeMode::None);
        widget.append(&status);
        let p = Rc::new(Self {
            widget,
            context: RefCell::new(None),
            capture: RefCell::new(None),
            on_close: RefCell::new(None),
            parent: parent.downgrade(),
            sender,
            anchor: RefCell::new(None),
            snapshot: RefCell::new(None),
            serial: Cell::new(0),
            busy: Cell::new(false),
            generating: Cell::new(false),
            uncertain: Cell::new(false),
            external_change: Cell::new(false),
            sync_feedback: RefCell::new(None),
            stop: RefCell::new(None),
            rows,
            goal,
            done,
            trail,
            status,
            progress,
            create,
            generate,
            reload,
            undo,
            delete,
            cancel,
            dialog: RefCell::new(None),
        });
        let weak = Rc::downgrade(&p);
        factory.connect_bind(move |_, object| {
            if let Some(p) = weak.upgrade() {
                p.bind_row(object);
            }
        });
        // Resolve the current ListItem when clicked, never a recycled row's old index.
        let weak = Rc::downgrade(&p);
        factory.connect_setup(move |_, object| {
            let item = object.downcast_ref::<gtk::ListItem>().unwrap();
            let row = item.child().unwrap();
            let title = row.first_child().and_downcast::<gtk::Button>().unwrap();
            let done = title.next_sibling().and_downcast::<gtk::Button>().unwrap();
            for (button, complete) in [(title, false), (done, true)] {
                let item = item.downgrade();
                let weak = weak.clone();
                button.connect_clicked(move |_| {
                    if let (Some(p), Some(item)) = (weak.upgrade(), item.upgrade()) {
                        p.change_item(&item, complete);
                    }
                });
            }
        });
        let weak = Rc::downgrade(&p);
        p.reload.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.sync_feedback.borrow_mut().take();
                p.load();
            }
        });
        let weak = Rc::downgrade(&p);
        p.create.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.create_dialog();
            }
        });
        let weak = Rc::downgrade(&p);
        p.generate.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.generate();
            }
        });
        let weak = Rc::downgrade(&p);
        p.undo.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.mutate(false);
            }
        });
        let weak = Rc::downgrade(&p);
        p.delete.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.confirm_delete();
            }
        });
        let weak = Rc::downgrade(&p);
        p.cancel.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.cancel();
                literal(&p.status, &crate::language::tr("Cancelling…"));
            }
        });
        let weak = Rc::downgrade(&p);
        close.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                if let Some(f) = p.on_close.borrow().as_ref() {
                    f();
                }
            }
        });
        p.render();
        p
    }
    fn current(&self) -> Option<Context> {
        self.context.borrow().as_ref().and_then(|f| f())
    }
    fn cancel(&self) {
        if let Some(stop) = self.stop.borrow().as_ref() {
            stop.store(true, Ordering::Release);
        }
    }
    pub fn sync(self: &Rc<Self>) {
        let next = self.current().map(|c| {
            (
                Anchor {
                    backend: c.backend.id,
                    thread: c.thread,
                },
                c.socket,
            )
        });
        if *self.anchor.borrow() != next {
            self.cancel();
            self.serial.set(self.serial.get() + 1);
            self.busy.set(false);
            self.stop.borrow_mut().take();
            self.uncertain.set(false);
            self.external_change.set(false);
            self.sync_feedback.borrow_mut().take();
            self.anchor.replace(next);
            self.snapshot.borrow_mut().take();
            let dialog = self.dialog.borrow_mut().take();
            if let Some(dialog) = dialog {
                dialog.close();
            }
            self.render();
            if self.widget.is_mapped() {
                self.load();
            }
        }
    }
    pub fn open(self: &Rc<Self>) {
        self.sync();
        self.load();
    }
    fn send(&self, work: Work, ai: bool) {
        if self.busy.get() {
            return;
        }
        if !matches!(&work, Work::Store(data::Request::Load(_))) {
            self.sync_feedback.borrow_mut().take();
        }
        self.generating.set(ai);
        let serial = self.serial.get() + 1;
        self.serial.set(serial);
        let stop = Arc::new(AtomicBool::new(false));
        self.stop.replace(Some(stop.clone()));
        self.busy.set(true);
        if self
            .sender
            .try_send(Command::SessionMap { serial, work, stop })
            .is_err()
        {
            self.busy.set(false);
            literal(&self.status, "Native worker is unavailable");
        } else {
            literal(
                &self.status,
                &crate::language::tr(if ai { "Generating…" } else { "Loading…" }),
            );
        }
        self.controls();
    }
    fn load(&self) {
        if let Some((anchor, _)) = self.anchor.borrow().clone() {
            self.send(Work::Store(data::Request::Load(anchor)), false);
        } else {
            self.render();
        }
    }
    pub fn event(self: &Rc<Self>, serial: u64, result: Result<ai::Reply, data::Failure>) {
        self.sync();
        if serial != self.serial.get() {
            return;
        }
        self.busy.set(false);
        let refresh = self.external_change.replace(false);
        self.stop.borrow_mut().take();
        match result {
            Ok(reply) => {
                self.uncertain.set(reply.reload_required);
                self.snapshot.replace(reply.snapshot);
                self.render();
                if let Some(warning) = reply.warning {
                    literal(&self.status, &warning);
                }
            }
            Err(failure) => {
                if failure.conflict || failure.uncertain {
                    self.uncertain.set(true);
                }
                literal(
                    &self.status,
                    &format!(
                        "{}{}",
                        failure.message,
                        if self.uncertain.get() {
                            " · Reload Map before making changes"
                        } else {
                            ""
                        }
                    ),
                );
                self.controls();
            }
        }
        if refresh {
            self.load();
        }
    }
    pub fn synchronized(
        self: &Rc<Self>,
        backend: &str,
        generation: u64,
        thread: &str,
        result: Result<Option<Arc<Snapshot>>, data::Failure>,
        message: &str,
    ) {
        self.sync();
        if !self
            .anchor
            .borrow()
            .as_ref()
            .is_some_and(|(a, g)| a.backend == backend && a.thread == thread && *g == generation)
        {
            return;
        }
        // Preserve feedback even when a concurrent local read/write owns the
        // pane. Its completion reloads the authoritative snapshot.
        self.sync_feedback.replace(Some(match &result {
            Err(e) => e.message.clone(),
            _ => message.into(),
        }));
        if self.busy.get() {
            self.external_change.set(true);
            return;
        }
        match result {
            Ok(Some(snapshot)) => {
                if self.snapshot.borrow().as_ref().is_some_and(|s| {
                    s.map.id != snapshot.map.id || s.map.revision >= snapshot.map.revision
                }) {
                    return;
                }
                self.snapshot.replace(Some(snapshot));
                self.render();
                literal(&self.status, message);
            }
            Ok(None) => literal(&self.status, message),
            Err(error) => {
                literal(&self.status, &error.message);
                if error.conflict || error.uncertain {
                    self.load();
                }
            }
        }
    }
    pub fn external_update(
        self: &Rc<Self>,
        backend: &str,
        generation: u64,
        thread: &str,
        message: &str,
    ) {
        self.sync();
        if !self
            .anchor
            .borrow()
            .as_ref()
            .is_some_and(|(a, g)| a.backend == backend && a.thread == thread && *g == generation)
        {
            return;
        }
        self.sync_feedback.replace(Some(message.into()));
        if self.busy.get() {
            self.external_change.set(true);
        } else {
            self.load();
        }
    }
    fn controls(&self) {
        let busy = self.busy.get();
        let present = self.snapshot.borrow().is_some();
        let anchored = self.anchor.borrow().is_some();
        let writable = !busy && !self.uncertain.get();
        self.create.set_visible(!present);
        self.create.set_sensitive(writable && anchored);
        self.generate.set_visible(present);
        self.generate.set_sensitive(
            writable
                && self
                    .current()
                    .is_some_and(|c| c.ready && c.backend.kind == "codex"),
        );
        crate::i18n_ui::bind(
            &self.generate,
            "label",
            if self
                .snapshot
                .borrow()
                .as_ref()
                .is_some_and(|s| !s.rows.is_empty())
            {
                "Update with AI"
            } else {
                "Generate with AI"
            },
        );
        self.undo.set_sensitive(writable && present);
        self.delete.set_sensitive(writable && present);
        self.reload.set_sensitive(!busy && anchored);
        self.cancel.set_visible(busy && self.generating.get());
    }
    fn render(&self) {
        let snapshot = self.snapshot.borrow();
        let keys = snapshot
            .as_ref()
            .map(|s| (0..s.rows.len()).map(|i| i.to_string()).collect::<Vec<_>>())
            .unwrap_or_default();
        let refs = keys.iter().map(String::as_str).collect::<Vec<_>>();
        self.rows.splice(0, self.rows.n_items(), &refs);
        if let Some(s) = snapshot.as_ref() {
            literal(&self.goal, &s.map.goal);
            literal(&self.done, &s.map.definition_of_done);
            literal(
                &self.trail,
                &s.trail
                    .iter()
                    .map(|&i| s.map.items[i].title.as_str())
                    .collect::<Vec<_>>()
                    .join(" › "),
            );
            literal(
                &self.progress,
                &format!("{} / {} · rev {}", s.done, s.rows.len(), s.map.revision),
            );
            literal(&self.status, "");
        } else {
            literal(
                &self.goal,
                &crate::language::tr(if self.anchor.borrow().is_some() {
                    "Create a Session Map to track this conversation"
                } else {
                    "Select a conversation to view its Map"
                }),
            );
            literal(&self.done, "");
            literal(&self.trail, "");
            literal(&self.status, "");
            literal(&self.progress, "");
        }
        drop(snapshot);
        if let Some(feedback) = self.sync_feedback.borrow().as_ref() {
            literal(&self.status, feedback);
        }
        self.controls();
    }
    fn item_index(&self, item: &gtk::ListItem) -> Option<usize> {
        item.item()
            .and_downcast::<gtk::StringObject>()
            .and_then(|v| v.string().parse().ok())
    }
    fn bind_row(&self, object: &glib::Object) {
        let item = object.downcast_ref::<gtk::ListItem>().unwrap();
        let snapshot = self.snapshot.borrow();
        let Some(s) = snapshot.as_ref() else {
            return;
        };
        let Some(row) = self.item_index(item).and_then(|i| s.rows.get(i)) else {
            return;
        };
        let data = &s.map.items[row.index];
        let container = item.child().unwrap();
        container.set_margin_start(12 + (row.depth.min(8) * 14) as i32);
        let button = container
            .first_child()
            .and_downcast::<gtk::Button>()
            .unwrap();
        let title = button.child().and_downcast::<gtk::Label>().unwrap();
        literal(&title, &data.title);
        button.set_tooltip_text(Some(&data.summary.replace('\0', "�")));
        if s.map.current_item_id.as_deref() == Some(&data.id) {
            title.add_css_class("accent");
            title.add_css_class("heading");
        } else {
            title.remove_css_class("accent");
            title.remove_css_class("heading");
        }
        let done = button.next_sibling().and_downcast::<gtk::Button>().unwrap();
        done.set_sensitive(data.state != "done");
        done.set_icon_name("object-select-symbolic");
        done.set_tooltip_text(Some(&crate::language::tr(if data.state == "done" {
            "Done"
        } else {
            "Mark done"
        })));
    }
    fn change_item(&self, item: &gtk::ListItem, done: bool) {
        if self.busy.get() || self.uncertain.get() {
            return;
        }
        let snapshot = self.snapshot.borrow().clone();
        let Some(s) = snapshot else {
            return;
        };
        let Some(row) = self.item_index(item).and_then(|i| s.rows.get(i)) else {
            return;
        };
        let id = s.map.items[row.index].id.clone();
        let op = if done {
            MapOperation::SetState {
                item_id: id,
                state: "done".into(),
            }
        } else {
            MapOperation::SetCurrent { item_id: Some(id) }
        };
        self.send(
            Work::Store(data::Request::Apply {
                anchor: Anchor {
                    backend: s.map.backend.clone(),
                    thread: s.map.thread_id.clone(),
                },
                map_id: s.map.id.clone(),
                revision: s.map.revision,
                actor: data::Actor::User,
                source_turn: None,
                operations: vec![op],
            }),
            false,
        );
    }
    fn mutate(&self, delete: bool) {
        let snapshot = self.snapshot.borrow().clone();
        if let Some(s) = snapshot {
            let anchor = Anchor {
                backend: s.map.backend.clone(),
                thread: s.map.thread_id.clone(),
            };
            let map_id = s.map.id.clone();
            let revision = s.map.revision;
            self.send(
                Work::Store(if delete {
                    data::Request::Delete {
                        anchor,
                        map_id,
                        revision,
                    }
                } else {
                    data::Request::Undo {
                        anchor,
                        map_id,
                        revision,
                    }
                }),
                false,
            );
        }
    }
    fn confirm_delete(self: &Rc<Self>) {
        if self.busy.get() || self.uncertain.get() {
            return;
        }
        let Some(s) = self.snapshot.borrow().clone() else {
            return;
        };
        let dialog = adw::AlertDialog::builder()
            .heading("Delete Map?")
            .body("The conversation will be kept.")
            .build();
        dialog.add_responses(&[("cancel", "Cancel"), ("delete", "Delete Map")]);
        dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        dialog.set_close_response("cancel");
        dialog.set_default_response(Some("cancel"));
        let weak = Rc::downgrade(self);
        let frozen = self.anchor.borrow().clone();
        dialog.connect_response(None, move |_, response| {
            if response == "delete" {
                if let Some(p) = weak.upgrade() {
                    if *p.anchor.borrow() == frozen
                        && p.snapshot.borrow().as_ref().is_some_and(|v| {
                            v.map.id == s.map.id && v.map.revision == s.map.revision
                        })
                    {
                        p.mutate(true);
                    }
                }
            }
        });
        if let Some(parent) = self.parent.upgrade() {
            dialog.present(Some(&parent));
        }
    }
    fn generate(&self) {
        if self.busy.get() || self.uncertain.get() {
            return;
        }
        let captured = self.capture.borrow().as_ref().and_then(|f| f());
        let snapshot = self.snapshot.borrow().clone();
        if let (Some(c), Some(snapshot)) = (captured, snapshot) {
            self.send(
                Work::Generate {
                    context: c.context,
                    snapshot,
                    source: c.source,
                    effort: c.effort,
                },
                true,
            );
        }
    }
    fn create_dialog(self: &Rc<Self>) {
        if self.busy.get() || self.uncertain.get() || self.snapshot.borrow().is_some() {
            return;
        }
        let Some(context) = self.current() else {
            return;
        };
        if let Some(dialog) = self.dialog.borrow().as_ref() {
            if let Some(parent) = self.parent.upgrade() {
                dialog.present(Some(&parent));
            }
            return;
        }
        let dialog = adw::Dialog::builder()
            .title("Create Session Map")
            .content_width(520)
            .content_height(380)
            .build();
        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&adw::HeaderBar::new());
        let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
        body.set_margin_start(24);
        body.set_margin_end(24);
        body.set_margin_top(12);
        body.set_margin_bottom(24);
        let goal = gtk::Entry::builder()
            .placeholder_text("Goal (optional: infer from conversation)")
            .max_length(4096)
            .build();
        let done = gtk::Entry::builder()
            .placeholder_text("Completion definition (optional)")
            .max_length(8192)
            .build();
        let structure = gtk::DropDown::from_strings(&["Hierarchy", "Path", "Flow", "Blank"]);
        let submit = gtk::Button::with_label("Create Map");
        submit.add_css_class("suggested-action");
        let notice = gtk::Label::builder()
            .wrap(true)
            .xalign(0.0)
            .label("AI generation uses an isolated task and leaves this conversation unchanged.")
            .build();
        for child in [
            goal.clone().upcast::<gtk::Widget>(),
            done.clone().upcast(),
            structure.clone().upcast(),
            notice.upcast(),
            submit.clone().upcast(),
        ] {
            body.append(&child);
        }
        toolbar.set_content(Some(&body));
        dialog.set_child(Some(&toolbar));
        let weak = Rc::downgrade(self);
        let d = dialog.downgrade();
        submit.connect_clicked(move |_| {
            let Some(p) = weak.upgrade() else {
                return;
            };
            let captured = p.capture.borrow().as_ref().and_then(|f| f());
            let Some(c) = captured else {
                return;
            };
            if c.context.backend.id != context.backend.id
                || c.context.thread != context.thread
                || c.context.socket != context.socket
            {
                return;
            }
            if goal.text().trim().is_empty() && c.context.backend.kind != "codex" {
                literal(&p.status, "OpenCode sessions require an explicit Map goal");
                return;
            }
            let request = CreateMapRequest {
                backend: context.backend.id.clone(),
                thread_id: context.thread.clone(),
                goal: goal.text().to_string(),
                definition_of_done: done.text().to_string(),
                structure: ["hierarchy", "path", "flow", "blank"]
                    [structure.selected().min(3) as usize]
                    .into(),
                items: vec![],
            };
            p.send(
                Work::Create {
                    context: c.context,
                    request,
                    source: c.source,
                    effort: c.effort,
                },
                true,
            );
            if let Some(d) = d.upgrade() {
                d.close();
            }
        });
        let weak = Rc::downgrade(self);
        dialog.connect_closed(move |_| {
            if let Some(p) = weak.upgrade() {
                p.dialog.borrow_mut().take();
            }
        });
        self.dialog.replace(Some(dialog.clone()));
        if let Some(parent) = self.parent.upgrade() {
            dialog.present(Some(&parent));
        }
    }
}
