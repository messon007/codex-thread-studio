//! Native prompt navigator over the displayed page, with geometry-based current turn.
use crate::{
    chat_visibility::Visibility,
    i18n_ui::{self, Part},
    model::Item,
    session_search::Context,
    turn_navigation::{Marker, Request, Response},
};
use adw::prelude::*;
use gtk::{gio, glib};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
};
type Jump = Box<dyn Fn(Context, Marker)>;
#[derive(Clone)]
struct Entry {
    serial: u64,
    ordinal: u32,
    marker: Marker,
}
#[derive(Clone)]
struct Bound {
    button: glib::WeakRef<gtk::Button>,
    label: glib::WeakRef<gtk::Label>,
    icon: glib::WeakRef<gtk::Image>,
    entry: Entry,
}
pub struct Panel {
    pub widget: gtk::MenuButton,
    pub on_jump: RefCell<Option<Jump>>,
    label: gtk::Label,
    popover: gtk::Popover,
    rows: gio::ListStore,
    turns: RefCell<HashMap<String, u32>>,
    unread: RefCell<HashSet<String>>,
    bound: RefCell<HashMap<usize, Bound>>,
    selection: gtk::SingleSelection,
    list: gtk::ListView,
    context: RefCell<Option<Context>>,
    users: RefCell<Vec<Arc<Item>>>,
    serial: Cell<u64>,
    sender: tokio::sync::watch::Sender<Option<Request>>,
    visibility: Rc<Visibility>,
    viewport: glib::WeakRef<gtk::Widget>,
    adjustment: glib::WeakRef<gtk::Adjustment>,
    pending: RefCell<Option<glib::SourceId>>,
}
impl Panel {
    #[cfg(debug_assertions)]
    pub(crate) fn capture_widget(&self) -> gtk::Widget {
        self.popover.child().expect("Turn popup content")
    }
    pub fn new(
        sender: tokio::sync::watch::Sender<Option<Request>>,
        visibility: Rc<Visibility>,
        scroll: &gtk::ScrolledWindow,
    ) -> Rc<Self> {
        let rows = gio::ListStore::new::<glib::BoxedAnyObject>();
        let selection = gtk::SingleSelection::new(Some(rows.clone()));
        selection.set_autoselect(false);
        selection.set_can_unselect(true);
        let factory = gtk::SignalListItemFactory::new();
        let owner = Rc::new(RefCell::new(std::rc::Weak::<Self>::new()));
        let actions = owner.clone();
        factory.connect_setup(move |_, object| {
            let row = object.downcast_ref::<gtk::ListItem>().unwrap();
            let label = gtk::Label::builder()
                .xalign(0.0)
                .width_chars(1)
                .max_width_chars(32)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .margin_top(8)
                .margin_bottom(8)
                .margin_start(12)
                .margin_end(12)
                .build();
            label.set_hexpand(true);
            let icon = gtk::Image::builder()
                .icon_name("mail-unread-symbolic")
                .accessible_role(gtk::AccessibleRole::Presentation)
                .build();
            icon.add_css_class("accent");
            icon.set_margin_end(12);
            // Reserve the indicator cell, as the shared selector does for its
            // checkmark, so acknowledgement cannot move an open popup's rows.
            icon.set_opacity(0.0);
            let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            content.append(&label);
            content.append(&icon);
            let button = gtk::Button::builder().child(&content).build();
            button.add_css_class("flat");
            let weak_row = row.downgrade();
            let actions = actions.clone();
            button.connect_clicked(move |_| {
                let Some(panel) = actions.borrow().upgrade() else {
                    return;
                };
                let Some(object) = weak_row
                    .upgrade()
                    .and_then(|row| row.item())
                    .and_downcast::<glib::BoxedAnyObject>()
                else {
                    return;
                };
                let entry = object.borrow::<Entry>().clone();
                panel.activate_entry(entry);
            });
            row.set_child(Some(&button));
        });
        let binding = owner.clone();
        factory.connect_bind(move |_, object| {
            let row = object.downcast_ref::<gtk::ListItem>().unwrap();
            let button = row.child().and_downcast::<gtk::Button>().unwrap();
            let content = button.child().and_downcast::<gtk::Box>().unwrap();
            let label = content.first_child().and_downcast::<gtk::Label>().unwrap();
            let icon = content.last_child().and_downcast::<gtk::Image>().unwrap();
            let object = row.item().and_downcast::<glib::BoxedAnyObject>().unwrap();
            let entry = object.borrow::<Entry>().clone();
            if let Some(panel) = binding.borrow().upgrade() {
                let bound = Bound {
                    button: button.downgrade(),
                    label: label.downgrade(),
                    icon: icon.downgrade(),
                    entry,
                };
                panel
                    .bound
                    .borrow_mut()
                    .insert(row.as_ptr() as usize, bound.clone());
                panel.present(&bound);
            }
        });
        let unbinding = owner.clone();
        factory.connect_unbind(move |_, object| {
            if let Some(panel) = unbinding.borrow().upgrade() {
                panel.bound.borrow_mut().remove(&(object.as_ptr() as usize));
            }
        });
        let list = gtk::ListView::new(Some(selection.clone()), Some(factory));
        list.set_single_click_activate(false);
        list.add_css_class("navigation-sidebar");
        let scroll_list = gtk::ScrolledWindow::builder()
            .child(&list)
            .min_content_width(220)
            .max_content_width(280)
            .propagate_natural_width(true)
            .min_content_height(48)
            .max_content_height(320)
            .propagate_natural_height(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();
        let heading = gtk::Label::new(None);
        i18n_ui::bind(&heading, "label", "Turns in this page");
        heading.add_css_class("heading");
        heading.set_margin_top(6);
        heading.set_margin_bottom(6);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.append(&heading);
        content.append(&scroll_list);
        let popover = gtk::Popover::builder().child(&content).build();
        let label = gtk::Label::new(None);
        let button_content = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        button_content.append(&gtk::Image::from_icon_name("view-list-symbolic"));
        button_content.append(&label);
        let widget = gtk::MenuButton::builder()
            .child(&button_content)
            .popover(&popover)
            .visible(false)
            .build();
        widget.add_css_class("flat");
        i18n_ui::bind(
            &widget,
            "tooltip-text",
            "Navigate turns in the displayed conversation page",
        );
        i18n_ui::accessible_label(&widget, "Turn navigation");
        let adjustment = scroll.vadjustment();
        let panel = Rc::new(Self {
            widget,
            on_jump: RefCell::new(None),
            label,
            popover,
            rows,
            turns: RefCell::new(HashMap::new()),
            unread: RefCell::new(HashSet::new()),
            bound: RefCell::new(HashMap::new()),
            selection,
            list,
            context: RefCell::new(None),
            users: RefCell::new(Vec::new()),
            serial: Cell::new(0),
            sender,
            visibility: visibility.clone(),
            viewport: scroll.clone().upcast::<gtk::Widget>().downgrade(),
            adjustment: adjustment.downgrade(),
            pending: RefCell::new(None),
        });
        *owner.borrow_mut() = Rc::downgrade(&panel);
        let weak = Rc::downgrade(&panel);
        panel.list.connect_activate(move |_, position| {
            let Some(panel) = weak.upgrade() else { return };
            let Some(object) = panel
                .rows
                .item(position)
                .and_downcast::<glib::BoxedAnyObject>()
            else {
                return;
            };
            let entry = object.borrow::<Entry>().clone();
            panel.activate_entry(entry);
        });
        for changed in [false, true] {
            let weak = Rc::downgrade(&panel);
            let observe = move |_: &gtk::Adjustment| {
                if let Some(panel) = weak.upgrade() {
                    panel.schedule();
                }
            };
            if changed {
                adjustment.connect_changed(observe);
            } else {
                adjustment.connect_value_changed(observe);
            }
        }
        let weak = Rc::downgrade(&panel);
        *visibility.on_change.borrow_mut() = Some(Box::new(move || {
            if let Some(panel) = weak.upgrade() {
                panel.schedule();
            }
        }));
        let weak = Rc::downgrade(&panel);
        panel.popover.connect_map(move |_| {
            if let Some(panel) = weak.upgrade() {
                panel.schedule();
            }
        });
        panel
    }
    fn present(&self, bound: &Bound) {
        if bound.entry.serial != self.serial.get() {
            return;
        }
        let (Some(button), Some(label), Some(icon)) = (
            bound.button.upgrade(),
            bound.label.upgrade(),
            bound.icon.upgrade(),
        ) else {
            return;
        };
        let mut parts = vec![Part::message(
            "User input {index}",
            &[("index", &bound.entry.ordinal.to_string())],
        )];
        if !bound.entry.marker.preview.is_empty() {
            parts.push(Part::literal(": "));
            parts.push(Part::literal(bound.entry.marker.preview.clone()));
        }
        i18n_ui::bind_parts(&label, "label", parts.clone(), "");
        let unread = self.unread.borrow().contains(&bound.entry.marker.turn);
        icon.set_opacity(if unread { 1.0 } else { 0.0 });
        // Retain the composed prompt's literal fragments, rather than feeding
        // the resulting user text back through a translation key.
        if unread {
            parts.push(Part::literal(" · "));
            parts.push(Part::message("Unread response", &[]));
        }
        i18n_ui::bind_parts(&button, "tooltip-text", parts.clone(), "");
        i18n_ui::accessible_parts(&button, parts, "");
    }
    pub fn set_unread(&self, context: &Context, unread: HashSet<String>) {
        if self.context.borrow().as_ref() != Some(context) || *self.unread.borrow() == unread {
            return;
        }
        self.unread.replace(unread);
        let bound = {
            let mut rows = self.bound.borrow_mut();
            rows.retain(|_, row| row.button.upgrade().is_some());
            rows.values().cloned().collect::<Vec<_>>()
        };
        for row in bound {
            self.present(&row);
        }
    }
    fn activate_entry(&self, entry: Entry) {
        let context = self.context.borrow().clone();
        if let Some(context) = context.filter(|_| entry.serial == self.serial.get()) {
            self.popover.popdown();
            if let Some(jump) = self.on_jump.borrow().as_ref() {
                jump(context, entry.marker);
            }
        }
    }
    pub fn reset(&self) {
        self.serial.set(self.serial.get().wrapping_add(1));
        self.context.replace(None);
        self.users.borrow_mut().clear();
        self.unread.borrow_mut().clear();
        self.bound.borrow_mut().clear();
        self.sender.send_replace(None);
        self.rows.remove_all();
        self.turns.borrow_mut().clear();
        self.popover.popdown();
        self.widget.set_visible(false);
    }
    pub fn refresh(&self, context: Option<Context>, items: &[Arc<Item>]) {
        let Some(context) = context else {
            self.reset();
            return;
        };
        let users = items
            .iter()
            .filter(|i| i.role == "You")
            .cloned()
            .collect::<Vec<_>>();
        if self.context.borrow().as_ref() == Some(&context)
            && self
                .users
                .borrow()
                .iter()
                .zip(&users)
                .all(|(a, b)| Arc::ptr_eq(a, b))
            && self.users.borrow().len() == users.len()
        {
            return;
        }
        self.reset();
        self.context.replace(Some(context.clone()));
        self.users.replace(users.clone());
        self.sender.send_replace(Some(Request {
            serial: self.serial.get(),
            context,
            users,
        }));
    }
    pub fn apply(self: &Rc<Self>, response: Response) {
        if response.serial != self.serial.get()
            || self.context.borrow().as_ref() != Some(&response.context)
        {
            return;
        }
        let Ok(markers) = response.result else { return };
        self.turns.replace(
            markers
                .iter()
                .enumerate()
                .map(|(i, m)| (m.turn.clone(), i as u32))
                .collect(),
        );
        let rows = markers
            .into_iter()
            .enumerate()
            .map(|(i, marker)| {
                glib::BoxedAnyObject::new(Entry {
                    serial: response.serial,
                    ordinal: i as u32 + 1,
                    marker,
                })
            })
            .collect::<Vec<_>>();
        self.rows.splice(0, self.rows.n_items(), &rows);
        self.widget.set_visible(rows.len() > 1);
        self.schedule();
    }
    fn schedule(self: &Rc<Self>) {
        if self.pending.borrow().is_some() {
            return;
        }
        let weak = Rc::downgrade(self);
        self.pending
            .replace(Some(glib::idle_add_local_once(move || {
                if let Some(panel) = weak.upgrade() {
                    panel.pending.borrow_mut().take();
                    panel.observe();
                }
            })));
    }
    fn observe(&self) {
        let (Some(viewport), Some(adjustment)) =
            (self.viewport.upgrade(), self.adjustment.upgrade())
        else {
            return;
        };
        if !viewport.is_mapped() {
            return;
        }
        let turns = self.turns.borrow();
        let active = if adjustment.value() + adjustment.page_size() >= adjustment.upper() - 8.0 {
            (turns.len() as u32).checked_sub(1)
        } else {
            self.visibility.turn_at_marker(&viewport, &turns)
        };
        let selected = active.unwrap_or(gtk::INVALID_LIST_POSITION);
        if self.selection.selected() != selected {
            self.selection.set_selected(selected);
        }
        if selected != gtk::INVALID_LIST_POSITION && self.popover.is_mapped() {
            self.list
                .scroll_to(selected, gtk::ListScrollFlags::NONE, None);
        }
        i18n_ui::literal(
            &self.label,
            "label",
            &format!(
                "{} / {}",
                active
                    .map(|i| (i + 1).to_string())
                    .unwrap_or_else(|| "–".into()),
                turns.len()
            ),
        );
    }
}
impl Drop for Panel {
    fn drop(&mut self) {
        if let Some(pending) = self.pending.get_mut().take() {
            pending.remove();
        }
    }
}
