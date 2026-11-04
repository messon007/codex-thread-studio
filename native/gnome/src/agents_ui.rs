use crate::{agents, composer_tools_ui::Context, model::Thread};
use adw::prelude::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(1);
type Load = Box<dyn Fn(agents::Step)>;
type Open = Box<dyn Fn(Thread)>;
type OpenAction = Rc<RefCell<Option<Open>>>;
pub struct Panel {
    pub dialog: adw::Dialog,
    pub context: Context,
    pub id: u64,
    pub revision: Cell<u64>,
    pub pending: Cell<u64>,
    pub on_load: RefCell<Option<Load>>,
    pub on_open: OpenAction,
    pub discovery: RefCell<Option<agents::Discovery>>,
    selected: String,
    tree: Rc<RefCell<Option<agents::Tree>>>,
    model: gtk::StringList,
    feedback: gtk::Label,
    reload: gtk::Button,
}
impl Panel {
    pub fn new(context: Context, selected: String) -> Rc<Self> {
        let dialog = adw::Dialog::builder()
            .title("Agents")
            .content_width(650)
            .content_height(560)
            .build();
        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        let reload = gtk::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text("Reload agent tree")
            .build();
        header.pack_end(&reload);
        toolbar.add_top_bar(&header);
        let model = gtk::StringList::new(&[]);
        let tree: Rc<RefCell<Option<agents::Tree>>> = Rc::new(RefCell::new(None));
        let on_open: OpenAction = Rc::new(RefCell::new(None));
        let factory = gtk::SignalListItemFactory::new();
        let records = tree.clone();
        let action = on_open.clone();
        factory.connect_setup(move |_, object| {
            let item = object.downcast_ref::<gtk::ListItem>().unwrap();
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            row.set_margin_top(10);
            row.set_margin_bottom(10);
            row.set_margin_start(18);
            row.set_margin_end(18);
            let text = gtk::Box::new(gtk::Orientation::Vertical, 4);
            text.set_hexpand(true);
            for index in 0..3 {
                let label = gtk::Label::builder()
                    .xalign(0.0)
                    .wrap(true)
                    .wrap_mode(gtk::pango::WrapMode::WordChar)
                    .build();
                if index == 0 {
                    label.add_css_class("heading");
                } else {
                    label.add_css_class("dim-label");
                    label.add_css_class("caption");
                }
                text.append(&label);
            }
            row.append(&text);
            let open = gtk::Button::builder()
                .icon_name("go-next-symbolic")
                .tooltip_text("Open agent session")
                .valign(gtk::Align::Center)
                .build();
            row.append(&open);
            let weak = item.downgrade();
            let records = records.clone();
            let action = action.clone();
            open.connect_clicked(move |_| {
                let Some(item) = weak.upgrade() else {
                    return;
                };
                let index = item
                    .item()
                    .and_downcast::<gtk::StringObject>()
                    .and_then(|index| index.string().parse::<usize>().ok());
                let target = {
                    let records = records.borrow();
                    records.as_ref().and_then(|tree| {
                        index
                            .and_then(|index| tree.rows.get(index))
                            .filter(|row| row.agent.thread.id != tree.selected)
                            .map(|row| row.agent.thread.clone())
                    })
                };
                if let Some(target) = target {
                    if let Some(open) = action.borrow().as_ref() {
                        open(target);
                    }
                }
            });
            item.set_child(Some(&row));
        });
        let records = tree.clone();
        factory.connect_bind(move |_, object| {
            let item = object.downcast_ref::<gtk::ListItem>().unwrap();
            let index = item
                .item()
                .and_downcast::<gtk::StringObject>()
                .and_then(|index| index.string().parse::<usize>().ok());
            let data = {
                let records = records.borrow();
                records.as_ref().and_then(|tree| {
                    index.and_then(|index| tree.rows.get(index)).map(|row| {
                        (
                            row.agent.clone(),
                            row.depth,
                            row.agent.thread.id == tree.selected,
                        )
                    })
                })
            };
            if let (Some((agent, depth, current)), Some(row)) = (data, item.child()) {
                // Keep nesting bounded visually; the exact depth remains readable.
                row.set_margin_start(18 + (depth.min(6) * 16) as i32);
                let mut child = row.first_child().and_then(|text| text.first_child());
                let kind = if depth == 0 {
                    "Main agent".to_owned()
                } else {
                    format!("Subagent · level {depth}")
                };
                let role = if agent.role.is_empty() {
                    kind
                } else {
                    format!("{kind} · {}", agent.role)
                };
                let status = if current {
                    "Current session".to_owned()
                } else {
                    match agent.thread.activity.as_str() {
                        "active" => "Working".into(),
                        "idle" => "Idle".into(),
                        "notLoaded" => "Not loaded".into(),
                        "" => String::new(),
                        other => other.to_owned(),
                    }
                };
                for text in [&agent.thread.title, &role, &status] {
                    if let Some(widget) = child.take() {
                        child = widget.next_sibling();
                        if let Some(label) = widget.downcast_ref::<gtk::Label>() {
                            label.set_text(text);
                            label.set_visible(!text.is_empty());
                        }
                    }
                }
                if let Some(open) = row.last_child().and_downcast::<gtk::Button>() {
                    open.set_sensitive(!current);
                    let accessible = if current {
                        format!("Current agent session: {}", agent.thread.title)
                    } else {
                        format!("Open agent session: {}", agent.thread.title)
                    };
                    open.update_property(&[gtk::accessible::Property::Label(&accessible)]);
                    open.set_tooltip_text(Some(if current {
                        "Current agent session"
                    } else {
                        "Open agent session"
                    }));
                }
                row.set_tooltip_text(Some(&format!("{}\n{}", agent.thread.id, agent.thread.cwd)));
            }
        });
        let list = gtk::ListView::new(
            Some(gtk::NoSelection::new(Some(model.clone()))),
            Some(factory),
        );
        list.add_css_class("boxed-list");
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        body.append(
            &gtk::Label::builder()
                .label(format!("Backend: {}", context.backend))
                .xalign(0.0)
                .margin_start(18)
                .margin_end(18)
                .margin_top(8)
                .build(),
        );
        body.append(
            &gtk::ScrolledWindow::builder()
                .child(&list)
                .vexpand(true)
                .hscrollbar_policy(gtk::PolicyType::Never)
                .build(),
        );
        let feedback = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .selectable(true)
            .margin_start(18)
            .margin_end(18)
            .margin_bottom(12)
            .build();
        body.append(&feedback);
        toolbar.set_content(Some(&body));
        dialog.set_child(Some(&toolbar));
        let panel = Rc::new(Self {
            dialog,
            context,
            id: NEXT.fetch_add(1, Ordering::Relaxed),
            revision: Cell::new(0),
            pending: Cell::new(0),
            on_load: RefCell::new(None),
            on_open,
            discovery: RefCell::new(None),
            selected,
            tree,
            model,
            feedback,
            reload: reload.clone(),
        });
        let weak = Rc::downgrade(&panel);
        reload.connect_clicked(move |_| {
            if let Some(panel) = weak.upgrade() {
                panel.begin();
            }
        });
        panel
    }
    pub fn begin(&self) {
        self.revision.set(self.revision.get() + 1);
        self.pending.set(0);
        self.discovery.replace(Some(agents::Discovery::default()));
        self.tree.borrow_mut().take();
        self.model.splice(0, self.model.n_items(), &[]);
        self.reload.set_sensitive(false);
        self.feedback.set_text("Resolving agent ancestry…");
        if let Some(load) = self.on_load.borrow().as_ref() {
            load(agents::Step::Read(self.selected.clone()));
        }
    }
    pub fn progress(&self, count: usize) {
        self.feedback
            .set_text(&format!("Scanning agent catalog · {count} sessions…"));
    }
    pub fn computing(&self) {
        self.pending.set(0);
        self.feedback.set_text("Building agent tree…");
    }
    pub fn error(&self, error: &str) {
        self.pending.set(0);
        self.feedback.set_text(error);
        self.reload.set_sensitive(true);
    }
    pub fn apply(&self, tree: agents::Tree) {
        let count = tree.rows.len();
        let status = match &tree.partial {
            Some(reason) => format!("{count} agents · {reason}"),
            None if count == 1 => "This session has no subagents.".into(),
            None => format!("{count} agents"),
        };
        self.tree.replace(Some(tree));
        let rows = (0..count)
            .map(|index| index.to_string())
            .collect::<Vec<_>>();
        let rows = rows.iter().map(String::as_str).collect::<Vec<_>>();
        self.model.splice(0, self.model.n_items(), &rows);
        self.pending.set(0);
        self.feedback.set_text(&status);
        self.reload.set_sensitive(true);
    }
    #[cfg(debug_assertions)]
    pub fn smoke_ids(&self) -> Vec<String> {
        self.tree
            .borrow()
            .as_ref()
            .map(|tree| {
                tree.rows
                    .iter()
                    .map(|row| row.agent.thread.id.clone())
                    .collect()
            })
            .unwrap_or_default()
    }
    #[cfg(debug_assertions)]
    pub fn smoke_status(&self) -> String {
        self.feedback.text().into()
    }
    #[cfg(debug_assertions)]
    pub fn smoke_open(&self, id: &str) -> Result<(), String> {
        fn find(widget: &gtk::Widget, id: &str) -> Option<gtk::Button> {
            if widget
                .tooltip_text()
                .is_some_and(|tooltip| tooltip.starts_with(&format!("{id}\n")))
            {
                return widget.last_child().and_downcast::<gtk::Button>();
            }
            let mut child = widget.first_child();
            while let Some(widget) = child {
                child = widget.next_sibling();
                if let Some(found) = find(&widget, id) {
                    return Some(found);
                }
            }
            None
        }
        let button =
            find(self.dialog.upcast_ref(), id).ok_or("Agent navigation button is not mapped")?;
        if !button.is_sensitive() {
            return Err("The current agent must not be reopened".into());
        }
        button.emit_clicked();
        Ok(())
    }
}
