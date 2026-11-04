use crate::{composer_tools_ui::Context, mcp};
use adw::prelude::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(1);
type Load = Box<dyn Fn(Option<String>)>;

pub struct Panel {
    pub dialog: adw::Dialog,
    pub context: Context,
    pub id: u64,
    pub revision: Cell<u64>,
    pub pending: Cell<u64>,
    pub on_load: RefCell<Option<Load>>,
    inventory: Rc<RefCell<mcp::Inventory>>,
    model: gtk::StringList,
    feedback: gtk::Label,
    reload: gtk::Button,
}
impl Panel {
    pub fn new(context: Context) -> Rc<Self> {
        let dialog = adw::Dialog::builder()
            .title("MCP servers")
            .content_width(640)
            .content_height(520)
            .build();
        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        let reload = gtk::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text("Reload server status")
            .build();
        header.pack_end(&reload);
        toolbar.add_top_bar(&header);
        let model = gtk::StringList::new(&[]);
        let inventory = Rc::new(RefCell::new(mcp::Inventory::default()));
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, item| {
            let item = item.downcast_ref::<gtk::ListItem>().unwrap();
            let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
            row.set_margin_start(18);
            row.set_margin_end(18);
            row.set_margin_top(10);
            row.set_margin_bottom(10);
            for index in 0..3 {
                let label = gtk::Label::builder()
                    .xalign(0.0)
                    .wrap(true)
                    .selectable(true)
                    .build();
                if index == 0 {
                    label.add_css_class("heading");
                } else {
                    label.add_css_class("dim-label");
                }
                row.append(&label);
            }
            item.set_child(Some(&row));
        });
        let records = inventory.clone();
        factory.connect_bind(move |_, item| {
            let item = item.downcast_ref::<gtk::ListItem>().unwrap();
            let index = item
                .item()
                .and_downcast::<gtk::StringObject>()
                .and_then(|value| value.string().parse::<usize>().ok());
            let server = index.and_then(|index| records.borrow().servers.get(index).cloned());
            if let (Some(server), Some(row)) = (server, item.child()) {
                let mut child = row.first_child();
                for text in [&server.name, &server.detail, &server.status] {
                    if let Some(widget) = child.take() {
                        child = widget.next_sibling();
                        if let Some(label) = widget.downcast_ref::<gtk::Label>() {
                            label.set_text(text);
                        }
                    }
                }
            }
        });
        let selection = gtk::NoSelection::new(Some(model.clone()));
        let list = gtk::ListView::new(Some(selection), Some(factory));
        list.add_css_class("boxed-list");
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let origin = gtk::Label::builder()
            .label(format!(
                "Backend: {} · Session: {}",
                context.backend,
                context
                    .key
                    .rsplit_once(':')
                    .map(|(_, thread)| thread)
                    .unwrap_or("")
            ))
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::Middle)
            .margin_start(18)
            .margin_end(18)
            .margin_top(8)
            .build();
        body.append(&origin);
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
            inventory,
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
        *self.inventory.borrow_mut() = mcp::Inventory::default();
        self.model.splice(0, self.model.n_items(), &[]);
        self.feedback.set_text("Loading MCP status…");
        self.reload.set_sensitive(false);
        if let Some(load) = self.on_load.borrow().as_ref() {
            load(None);
        }
    }
    pub fn error(&self, error: &str) {
        self.pending.set(0);
        self.feedback.set_text(error);
        self.reload.set_sensitive(true);
    }
    pub fn page(&self, page: mcp::Page) -> Result<Option<String>, String> {
        let before = self.inventory.borrow().servers.len();
        let cursor = self.inventory.borrow_mut().append(page)?;
        let after = self.inventory.borrow().servers.len();
        let rows = (before..after)
            .map(|index| index.to_string())
            .collect::<Vec<_>>();
        let rows = rows.iter().map(String::as_str).collect::<Vec<_>>();
        self.model.splice(before as u32, 0, &rows);
        self.pending.set(0);
        self.feedback.set_text(&if cursor.is_some() {
            format!("{after} servers · Loading more…")
        } else if after == 0 {
            "No MCP servers are configured.".into()
        } else {
            format!("{after} servers")
        });
        self.reload.set_sensitive(cursor.is_none());
        Ok(cursor)
    }
    #[cfg(debug_assertions)]
    pub fn smoke_names(&self) -> Vec<String> {
        self.inventory
            .borrow()
            .servers
            .iter()
            .map(|server| server.name.clone())
            .collect()
    }
    #[cfg(debug_assertions)]
    pub fn smoke_status(&self) -> String {
        self.feedback.text().into()
    }
}
