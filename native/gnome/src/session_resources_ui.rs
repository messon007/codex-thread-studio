//! Virtualized native latest-turn resource panel. Immutable source handles and
//! serials keep delayed results/actions tied to the originating session.
use crate::{
    model::{Item, Thread},
    protocol::Command,
    session_resources::{self as data, Filter, Occurrence, Resource},
    session_search::Context,
};
use adw::prelude::*;
use gtk::{gio, glib};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct Input {
    pub context: Context,
    pub thread: Thread,
    pub items: Vec<Arc<Item>>,
}
pub type Action = Box<dyn Fn(Input, Arc<Resource>, Occurrence)>;
pub struct Panel {
    pub widget: gtk::Box,
    pub on_open: RefCell<Option<Action>>,
    pub on_source: RefCell<Option<Action>>,
    pub on_favorite: RefCell<Option<Action>>,
    pub on_close: RefCell<Option<Box<dyn Fn()>>>,
    input: RefCell<Option<Input>>,
    serial: Cell<u64>,
    rows: gio::ListStore,
    search: gtk::SearchEntry,
    filter: gtk::DropDown,
    status: gtk::Label,
    subtitle: gtk::Label,
    sender: mpsc::Sender<Command>,
    applying: Cell<bool>,
    rebuild: Cell<bool>,
    preferences: RefCell<std::collections::VecDeque<(String, String, u32)>>,
}
fn bound(label: &gtk::Label, text: &str) {
    crate::i18n_ui::literal(label, "label", &text.replace('\0', "�"));
}
fn occurrence_factory(popup: bool) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(move |_, object| {
        let label = gtk::Label::builder()
            .xalign(0.0)
            .width_chars(1)
            .max_width_chars(if popup { 64 } else { 32 })
            .wrap(popup)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .ellipsize(if popup {
                gtk::pango::EllipsizeMode::None
            } else {
                gtk::pango::EllipsizeMode::End
            })
            .build();
        object
            .downcast_ref::<gtk::ListItem>()
            .unwrap()
            .set_child(Some(&label));
    });
    factory.connect_bind(|_, object| {
        let item = object.downcast_ref::<gtk::ListItem>().unwrap();
        let label = item.child().and_downcast::<gtk::Label>().unwrap();
        let value = item
            .item()
            .and_downcast::<gtk::StringObject>()
            .unwrap()
            .string();
        let (role, excerpt) = value.split_once(" · ").unwrap_or(("", &value));
        let role = match role {
            "You" => "You",
            "Plan" => "Plan",
            _ => "Assistant",
        };
        crate::i18n_ui::bind_parts(
            &label,
            "label",
            vec![
                crate::i18n_ui::Part::message(role, &[]),
                crate::i18n_ui::Part::literal(excerpt),
            ],
            " · ",
        );
    });
    factory
}
impl Panel {
    pub fn refresh(&self) {
        self.rebuild.set(true);
        self.request(false);
    }
    pub fn focus_search(&self) {
        self.search.grab_focus();
    }
    #[cfg(debug_assertions)]
    pub(crate) fn smoke_state(&self) -> serde_json::Value {
        serde_json::json!({"serial":self.serial.get(),"query":self.search.text().to_string(),"filter":self.filter.selected(),"mapped":self.widget.is_mapped(),"status":self.status.text().to_string(),"rows":(0..self.rows.n_items()).filter_map(|i|self.rows.item(i).and_downcast::<glib::BoxedAnyObject>()).map(|o|o.borrow::<(u64,Arc<Resource>)>().1.address.clone()).collect::<Vec<_>>()})
    }
    pub fn new(sender: mpsc::Sender<Command>) -> Rc<Self> {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let (header, _) = crate::workspace_ui::header("Resources");
        let reload = crate::workspace_ui::button("view-refresh-symbolic", "Refresh resources");
        let close = crate::workspace_ui::button("window-close-symbolic", "Close resources");
        header.append(&reload);
        header.append(&close);
        widget.append(&header);
        let subtitle = crate::workspace_ui::footer();
        widget.append(&subtitle);
        let controls = crate::adaptive::toolbar(6);
        controls.set_margin_start(12);
        controls.set_margin_end(12);
        let search = gtk::SearchEntry::builder().hexpand(true).build();
        crate::i18n_ui::bind(&search, "placeholder-text", "Search resources…");
        let filter = gtk::DropDown::from_strings(&["All", "Web", "Files", "Code"]);
        crate::i18n_ui::choices(&filter);
        controls.append(&search);
        controls.append(&filter);
        widget.append(&controls);
        let rows = gio::ListStore::new::<glib::BoxedAnyObject>();
        let factory = gtk::SignalListItemFactory::new();
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
        status.set_wrap(true);
        widget.append(&status);
        let panel = Rc::new(Self {
            widget,
            on_open: Default::default(),
            on_source: Default::default(),
            on_favorite: Default::default(),
            on_close: Default::default(),
            input: Default::default(),
            serial: Cell::new(0),
            rows,
            search,
            filter,
            status,
            subtitle,
            sender,
            applying: Cell::new(false),
            rebuild: Cell::new(false),
            preferences: Default::default(),
        });
        let owner = Rc::downgrade(&panel);
        factory.connect_setup(move |_, object| {
            let item = object.downcast_ref::<gtk::ListItem>().unwrap();
            let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
            for p in ["margin-start", "margin-end", "margin-top", "margin-bottom"] {
                card.set_property(p, 12i32);
            }
            let title = gtk::Label::builder()
                .xalign(0.0)
                .ellipsize(gtk::pango::EllipsizeMode::Middle)
                .build();
            title.add_css_class("heading");
            let address = gtk::Label::builder()
                .xalign(0.0)
                .wrap(true)
                .wrap_mode(gtk::pango::WrapMode::WordChar)
                .width_chars(1)
                .selectable(true)
                .build();
            address.add_css_class("dim-label");
            let occurrences = gtk::DropDown::from_strings(&[]);
            occurrences.set_hexpand(true);
            crate::adaptive::string_selector(&occurrences, 32);
            occurrences.set_factory(Some(&occurrence_factory(false)));
            occurrences.set_list_factory(Some(&occurrence_factory(true)));
            crate::i18n_ui::accessible_label(&occurrences, "Resource occurrences");
            let actions = crate::adaptive::toolbar(6);
            let open = crate::workspace_ui::button("document-open-symbolic", "Open resource");
            let source = crate::workspace_ui::button("go-jump-symbolic", "Go to message");
            let copy = crate::workspace_ui::button("edit-copy-symbolic", "Copy address");
            let favorite = crate::workspace_ui::button("star-new-symbolic", "Save resource");
            actions.append(&occurrences);
            for b in [&open, &source, &copy, &favorite] {
                actions.append(b);
            }
            card.append(&title);
            card.append(&address);
            card.append(&actions);
            let reason = gtk::Label::builder()
                .xalign(0.0)
                .wrap(true)
                .width_chars(1)
                .build();
            reason.add_css_class("caption");
            reason.add_css_class("dim-label");
            card.append(&reason);
            item.set_child(Some(&card));
            for (button, kind) in [(open, 0), (source, 1), (copy, 2), (favorite, 3)] {
                let owner = owner.clone();
                let item = item.downgrade();
                let occurrences = occurrences.clone();
                button.connect_clicked(move |_| {
                    let Some(panel) = owner.upgrade() else {
                        return;
                    };
                    let Some(row) = item
                        .upgrade()
                        .and_then(|i| i.item())
                        .and_downcast::<glib::BoxedAnyObject>()
                    else {
                        return;
                    };
                    let (serial, resource) = row.borrow::<(u64, Arc<Resource>)>().clone();
                    if serial != panel.serial.get() {
                        return;
                    }
                    let Some(input) = panel.input.borrow().clone() else {
                        return;
                    };
                    let Some(occurrence) = resource
                        .occurrences
                        .get(occurrences.selected() as usize)
                        .cloned()
                    else {
                        return;
                    };
                    if kind == 2 {
                        panel.widget.clipboard().set_text(&resource.address);
                        return;
                    }
                    let slot = match kind {
                        0 => &panel.on_open,
                        1 => &panel.on_source,
                        _ => &panel.on_favorite,
                    };
                    if kind == 0 && resource.blocked.is_some() {
                        return;
                    }
                    if let Some(action) = slot.borrow().as_ref() {
                        action(input, resource, occurrence);
                    };
                });
            }
        });
        factory.connect_bind(|_, object| {
            let item = object.downcast_ref::<gtk::ListItem>().unwrap();
            let row = item.item().and_downcast::<glib::BoxedAnyObject>().unwrap();
            let data = row.borrow::<(u64, Arc<Resource>)>();
            let resource = &data.1;
            let card = item.child().and_downcast::<gtk::Box>().unwrap();
            let title = card.first_child().and_downcast::<gtk::Label>().unwrap();
            bound(&title, &resource.title);
            crate::i18n_ui::bind(&title, "tooltip-text", resource.kind.label());
            let address = title.next_sibling().and_downcast::<gtk::Label>().unwrap();
            bound(&address, &resource.address);
            let actions = address
                .next_sibling()
                .and_downcast::<adw::WrapBox>()
                .unwrap();
            let occurrences = actions
                .first_child()
                .and_downcast::<gtk::DropDown>()
                .unwrap();
            let labels = resource
                .occurrences
                .iter()
                .map(|o| format!("{} · {}", o.item.role, o.excerpt))
                .collect::<Vec<_>>();
            occurrences.set_model(Some(&gtk::StringList::new(
                &labels.iter().map(String::as_str).collect::<Vec<_>>(),
            )));
            occurrences.set_selected(labels.len().saturating_sub(1) as u32);
            let open = occurrences
                .next_sibling()
                .and_downcast::<gtk::Button>()
                .unwrap();
            open.set_sensitive(resource.blocked.is_none());
            let reason = actions.next_sibling().and_downcast::<gtk::Label>().unwrap();
            reason.set_visible(resource.blocked.is_some());
            if let Some(reason) = resource.blocked {
                crate::i18n_ui::bind(&open, "tooltip-text", reason);
                let label = actions.next_sibling().and_downcast::<gtk::Label>().unwrap();
                crate::i18n_ui::bind(&label, "label", reason);
            } else {
                crate::i18n_ui::bind(&open, "tooltip-text", "Open resource");
            }
        });
        let weak = Rc::downgrade(&panel);
        reload.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.request(true);
            }
        });
        let weak = Rc::downgrade(&panel);
        close.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                if let Some(action) = p.on_close.borrow().as_ref() {
                    action();
                }
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.search.connect_search_changed(move |_| {
            if let Some(p) = weak.upgrade() {
                p.request(false);
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.filter.connect_selected_notify(move |_| {
            if let Some(p) = weak.upgrade() {
                p.request(false);
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.widget.connect_map(move |_| {
            if let Some(p) = weak.upgrade() {
                p.request(false);
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.widget.connect_unmap(move |_| {
            if let Some(p) = weak.upgrade() {
                p.serial.set(p.serial.get() + 1);
                let _ = p.sender.try_send(Command::SessionResources(None));
            }
        });
        panel
    }
    fn remember(&self, input: &Input) {
        let key = format!("{}:{}", input.context.backend, input.context.thread);
        let mut prefs = self.preferences.borrow_mut();
        prefs.retain(|(k, _, _)| k != &key);
        prefs.push_back((key, self.search.text().to_string(), self.filter.selected()));
        while prefs.len() > 16 {
            prefs.pop_front();
        }
    }
    pub fn sync(&self, input: Option<Input>) {
        let input = input.map(|mut input| {
            input.items = data::latest(&input.items);
            input
        });
        let unchanged = {
            let old = self.input.borrow();
            match (old.as_ref(), input.as_ref()) {
                (Some(a), Some(b)) => {
                    a.context == b.context
                        && a.thread == b.thread
                        && data::same_items(&a.items, &b.items)
                }
                (None, None) => true,
                _ => false,
            }
        };
        if unchanged {
            return;
        }
        let changed_session =
            self.input.borrow().as_ref().map(|i| &i.context) != input.as_ref().map(|i| &i.context);
        if changed_session {
            if let Some(old) = self.input.borrow().as_ref() {
                self.remember(old);
            }
            self.applying.set(true);
            let prefs = input.as_ref().and_then(|i| {
                let key = format!("{}:{}", i.context.backend, i.context.thread);
                self.preferences
                    .borrow()
                    .iter()
                    .find(|(k, _, _)| k == &key)
                    .cloned()
            });
            self.search
                .set_text(prefs.as_ref().map_or("", |(_, q, _)| q));
            self.filter.set_selected(prefs.map_or(0, |(_, _, f)| f));
            self.applying.set(false);
        }
        self.serial.set(self.serial.get() + 1);
        self.rows.remove_all();
        bound(
            &self.subtitle,
            input.as_ref().map_or("", |i| i.thread.cwd.as_str()),
        );
        self.input.replace(input);
        self.request(false);
    }
    fn request(&self, force: bool) {
        if self.applying.get() || !self.widget.is_mapped() {
            return;
        }
        let Some(input) = self.input.borrow().clone() else {
            crate::i18n_ui::bind(&self.status, "label", "Select a session to view resources");
            return;
        };
        self.serial.set(self.serial.get() + 1);
        self.rows.remove_all();
        crate::i18n_ui::bind(&self.status, "label", "Loading resources…");
        let request = data::Request {
            context: input.context,
            root: input.thread.cwd,
            items: input.items,
            serial: self.serial.get(),
            force: force | self.rebuild.replace(false),
            query: self.search.text().chars().take(4096).collect(),
            filter: match self.filter.selected() {
                1 => Filter::Web,
                2 => Filter::File,
                3 => Filter::Code,
                _ => Filter::All,
            },
        };
        if self
            .sender
            .try_send(Command::SessionResources(Some(request)))
            .is_err()
        {
            self.rebuild.set(true);
            crate::i18n_ui::bind(&self.status, "label", "Unable to load resources; retry");
        }
    }
    pub fn event(&self, response: data::Response) {
        if response.serial != self.serial.get()
            || !self.widget.is_mapped()
            || self
                .input
                .borrow()
                .as_ref()
                .is_none_or(|i| i.context != response.context)
        {
            return;
        }
        match response.result {
            Ok(view) => {
                let rows = view
                    .rows
                    .into_iter()
                    .map(|r| glib::BoxedAnyObject::new((response.serial, r)))
                    .collect::<Vec<_>>();
                self.rows.splice(0, self.rows.n_items(), &rows);
                crate::i18n_ui::bind_parts(
                    &self.status,
                    "label",
                    vec![
                        crate::i18n_ui::Part::message(
                            "{resources} resources · {occurrences} references",
                            &[
                                ("resources", &view.index.resources.len().to_string()),
                                ("occurrences", &view.index.occurrences.to_string()),
                            ],
                        ),
                        crate::i18n_ui::Part::literal(" · "),
                        crate::i18n_ui::Part::message("Latest turn only", &[]),
                        crate::i18n_ui::Part::message(
                            if view.index.truncated {
                                " · Resource index reached its limit"
                            } else {
                                ""
                            },
                            &[],
                        ),
                    ],
                    "",
                );
            }
            Err(error) => bound(&self.status, &error),
        }
    }
}
