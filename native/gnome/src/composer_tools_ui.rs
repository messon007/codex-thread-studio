use crate::composer_refs::{self, Kind, Suggestion, Trigger};
use adw::prelude::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Context {
    pub key: String,
    pub backend: String,
    pub generation: u64,
    pub root: String,
    pub opencode: bool,
    pub target: Option<ReferenceTarget>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceTarget {
    pub session: crate::router::Target,
    pub generation: u64,
    pub opencode: bool,
}
impl Context {
    pub fn reference_backend(&self) -> &str {
        self.target
            .as_ref()
            .map_or(&self.backend, |target| &target.session.backend)
    }
    pub fn reference_generation(&self) -> u64 {
        self.target
            .as_ref()
            .map_or(self.generation, |target| target.generation)
    }
    pub fn reference_root(&self) -> &str {
        self.target
            .as_ref()
            .map_or(&self.root, |target| &target.session.cwd)
    }
    pub fn reference_opencode(&self) -> bool {
        self.target
            .as_ref()
            .map_or(self.opencode, |target| target.opencode)
    }
}
#[derive(Clone, Debug)]
pub struct Query {
    pub context: Context,
    pub serial: u64,
    pub trigger: Trigger,
}
pub struct Tools {
    pub widget: gtk::ScrolledWindow,
    chips: gtk::FlowBox,
    pub popover: gtk::Popover,
    list: gtk::ListBox,
    scroll: gtk::ScrolledWindow,
    status: gtk::Label,
    pub query: RefCell<Option<Query>>,
    serial: Cell<u64>,
    pub sources: RefCell<Vec<Arc<Vec<Arc<Suggestion>>>>>,
    choices: RefCell<Vec<Arc<Suggestion>>>,
    selected: Cell<usize>,
    pub preedit: Cell<bool>,
    pub scheduled: Cell<bool>,
    pub filter_ticket: Cell<u64>,
    stamp: RefCell<(String, u64)>,
    pub on_pick: RefCell<Option<Box<dyn Fn(Query, Arc<Suggestion>)>>>,
    pub on_remove: RefCell<Option<Box<dyn Fn(String, String)>>>,
}
impl Tools {
    #[cfg(debug_assertions)]
    pub fn smoke_keyboard_insert(&self) -> Result<(), String> {
        use gtk::gdk::{Key, ModifierType};
        if self.choices.borrow().is_empty() {
            return Err("Keyboard completion has no choices".into());
        }
        let path = self.choices.borrow()[self.selected.get()].path.clone();
        self.preedit.set(true);
        let ignored =
            self.key(Key::Return, ModifierType::empty()) == gtk::glib::Propagation::Proceed;
        self.preedit.set(false);
        if !ignored {
            return Err("Completion consumed an IME preedit key".into());
        }
        self.key(Key::Down, ModifierType::empty());
        self.key(Key::Up, ModifierType::empty());
        if self.choices.borrow()[self.selected.get()].path != path {
            return Err("Completion keyboard navigation lost its selection".into());
        }
        let query = self
            .query
            .borrow()
            .clone()
            .ok_or("Keyboard query disappeared")?;
        let mut reordered = self.choices.borrow().clone();
        reordered.reverse();
        self.apply(query.serial, Ok(reordered));
        if self.choices.borrow()[self.selected.get()].path != path {
            return Err("A menu update changed the selected reference".into());
        }
        if self.key(Key::Tab, ModifierType::empty()) != gtk::glib::Propagation::Stop {
            return Err("Tab did not insert the selected reference".into());
        }
        Ok(())
    }
    pub fn new(anchor: &gtk::Box) -> Rc<Self> {
        let chips = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .min_children_per_line(1)
            .max_children_per_line(8)
            .column_spacing(4)
            .row_spacing(4)
            .margin_start(8)
            .margin_end(8)
            .build();
        let widget = gtk::ScrolledWindow::builder()
            .child(&chips)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .max_content_height(90)
            .propagate_natural_height(true)
            .visible(false)
            .build();
        let popover = gtk::Popover::builder()
            .autohide(false)
            .has_arrow(false)
            .position(gtk::PositionType::Top)
            .build();
        // GtkBox's layout manager allocates native popovers. GtkTextView uses
        // custom allocation and does not allocate an arbitrary child popover.
        popover.set_parent(anchor);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 4);
        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::Single);
        list.add_css_class("boxed-list");
        list.set_focusable(false);
        let scroll = gtk::ScrolledWindow::builder()
            .child(&list)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .max_content_height(300)
            .propagate_natural_height(true)
            .min_content_width(360)
            .build();
        content.append(&scroll);
        let status = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .margin_top(4)
            .margin_bottom(4)
            .margin_start(8)
            .margin_end(8)
            .max_width_chars(60)
            .build();
        status.add_css_class("caption");
        status.add_css_class("dim-label");
        content.append(&status);
        popover.set_child(Some(&content));
        let ui = Rc::new(Self {
            widget,
            chips,
            popover,
            list,
            scroll,
            status,
            query: RefCell::new(None),
            serial: Cell::new(0),
            sources: RefCell::new(Vec::new()),
            choices: RefCell::new(Vec::new()),
            selected: Cell::new(0),
            preedit: Cell::new(false),
            scheduled: Cell::new(false),
            filter_ticket: Cell::new(0),
            stamp: RefCell::new((String::new(), 0)),
            on_pick: RefCell::new(None),
            on_remove: RefCell::new(None),
        });
        let weak = Rc::downgrade(&ui);
        ui.list.connect_row_activated(move |_, row| {
            if let Some(ui) = weak.upgrade() {
                ui.select(row.index() as usize);
            }
        });
        ui
    }
    pub fn begin(&self, context: Option<Context>, buffer: &gtk::TextBuffer) -> Option<Query> {
        let trigger = if !buffer.has_selection() && !self.preedit.get() {
            let end = buffer.iter_at_offset(buffer.cursor_position());
            let start = buffer.iter_at_offset((end.offset() - 258).max(0));
            composer_refs::trigger(&buffer.text(&start, &end, false), end.offset())
        } else {
            None
        };
        let (Some(context), Some(trigger)) = (context, trigger) else {
            self.close();
            return None;
        };
        if self
            .query
            .borrow()
            .as_ref()
            .is_some_and(|query| query.context == context && query.trigger == trigger)
        {
            return None;
        }
        self.serial.set(self.serial.get() + 1);
        self.filter_ticket.set(self.filter_ticket.get() + 1);
        let query = Query {
            context,
            serial: self.serial.get(),
            trigger,
        };
        self.query.replace(Some(query.clone()));
        self.sources.borrow_mut().clear();
        self.choices.borrow_mut().clear();
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        self.status.set_text(if query.trigger.kind == Kind::File {
            "Searching project files…"
        } else if query.trigger.kind == Kind::Command {
            "Finding commands…"
        } else {
            "Discovering skills and plugins…"
        });
        self.popover.popup();
        self.popover.present();
        Some(query)
    }
    pub fn current(&self, serial: u64) -> bool {
        self.serial.get() == serial
            && self
                .query
                .borrow()
                .as_ref()
                .is_some_and(|query| query.serial == serial)
    }
    pub fn close(&self) {
        if self.query.borrow_mut().take().is_some() {
            self.serial.set(self.serial.get() + 1);
            self.filter_ticket.set(self.filter_ticket.get() + 1);
        }
        self.sources.borrow_mut().clear();
        self.choices.borrow_mut().clear();
        self.popover.popdown();
    }
    pub fn error(&self, serial: u64, error: &str) {
        if self.current(serial) {
            self.status.set_text(error);
        }
    }
    pub fn apply(&self, serial: u64, result: Result<Vec<Arc<Suggestion>>, String>) {
        if !self.current(serial) {
            return;
        }
        let choices = match result {
            Ok(choices) => choices,
            Err(error) => {
                self.error(serial, &error);
                return;
            }
        };
        let selected_path = self
            .choices
            .borrow()
            .get(self.selected.get())
            .map(|choice| choice.path.clone());
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        for choice in &choices {
            let content = gtk::Box::new(gtk::Orientation::Vertical, 2);
            content.set_margin_start(10);
            content.set_margin_end(10);
            content.set_margin_top(6);
            content.set_margin_bottom(6);
            let title = gtk::Label::builder()
                .label(&choice.title)
                .xalign(0.0)
                .ellipsize(gtk::pango::EllipsizeMode::Middle)
                .max_width_chars(65)
                .build();
            content.append(&title);
            if !choice.detail.is_empty() {
                let detail = gtk::Label::builder()
                    .label(&choice.detail)
                    .xalign(0.0)
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .max_width_chars(65)
                    .build();
                detail.add_css_class("caption");
                detail.add_css_class("dim-label");
                content.append(&detail);
            }
            let row = gtk::ListBoxRow::new();
            row.set_focusable(false);
            row.set_child(Some(&content));
            self.list.append(&row);
        }
        let command = self
            .query
            .borrow()
            .as_ref()
            .is_some_and(|query| query.trigger.kind == Kind::Command);
        self.status.set_text(if command && choices.is_empty() {
            "No matching commands"
        } else if command {
            "↑ / ↓ to select · Tab or Enter to run · Esc to close"
        } else if choices.is_empty() {
            "No matching references"
        } else {
            "↑ / ↓ to select · Tab or Enter to insert · Esc to close"
        });
        let selected = selected_path
            .and_then(|path| choices.iter().position(|choice| choice.path == path))
            .unwrap_or(0);
        self.choices.replace(choices);
        self.selected.set(selected);
        if let Some(row) = self.list.row_at_index(selected as i32) {
            self.list.select_row(Some(&row));
        }
        self.popover.present();
    }
    pub fn filtered(&self, ticket: u64, result: Result<Vec<Arc<Suggestion>>, String>) {
        if ticket != self.filter_ticket.get() {
            return;
        }
        let query = self.query.borrow().clone();
        if let Some(query) = query {
            self.apply(query.serial, result);
        }
    }
    fn select(&self, index: usize) {
        let query = self.query.borrow().clone();
        let choice = self.choices.borrow().get(index).cloned();
        if let (Some(query), Some(choice)) = (query, choice) {
            if let Some(callback) = self.on_pick.borrow().as_ref() {
                callback(query, choice);
            }
        }
    }
    pub fn key(&self, key: gtk::gdk::Key, state: gtk::gdk::ModifierType) -> gtk::glib::Propagation {
        use gtk::gdk::{Key, ModifierType};
        if !self.popover.is_visible()
            || self.preedit.get()
            || state.intersects(
                ModifierType::CONTROL_MASK | ModifierType::ALT_MASK | ModifierType::SUPER_MASK,
            )
        {
            return gtk::glib::Propagation::Proceed;
        }
        let count = self.choices.borrow().len();
        match key {
            Key::Escape => self.close(),
            Key::Return | Key::KP_Enter | Key::Tab if count > 0 => self.select(self.selected.get()),
            Key::Down | Key::Up if count > 0 => {
                let selected = if key == Key::Down {
                    (self.selected.get() + 1) % count
                } else {
                    (self.selected.get() + count - 1) % count
                };
                self.selected.set(selected);
                if let Some(row) = self.list.row_at_index(selected as i32) {
                    self.list.select_row(Some(&row));
                    if let Some(bounds) = row.compute_bounds(&self.list) {
                        let adjustment = self.scroll.vadjustment();
                        let top = bounds.y() as f64;
                        let bottom = top + bounds.height() as f64;
                        let current = adjustment.value();
                        let page = adjustment.page_size();
                        let value = if top < current {
                            top
                        } else if bottom > current + page {
                            bottom - page
                        } else {
                            current
                        };
                        adjustment.set_value(value.clamp(
                            adjustment.lower(),
                            (adjustment.upper() - page).max(adjustment.lower()),
                        ));
                    }
                }
            }
            _ => return gtk::glib::Propagation::Proceed,
        }
        gtk::glib::Propagation::Stop
    }
    pub fn render_refs(self: &Rc<Self>, key: &str, book: &composer_refs::Book) {
        let stamp = (key.to_owned(), book.revision);
        if *self.stamp.borrow() == stamp {
            return;
        }
        self.stamp.replace(stamp);
        while let Some(child) = self.chips.first_child() {
            self.chips.remove(&child);
        }
        let items = book.get(key);
        self.widget.set_visible(!items.is_empty());
        for item in items {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            row.add_css_class("card");
            row.set_margin_start(4);
            row.set_margin_end(4);
            let label = gtk::Label::builder()
                .label(&item.label)
                .tooltip_text(item.input["path"].as_str().unwrap_or(""))
                .ellipsize(gtk::pango::EllipsizeMode::Middle)
                .max_width_chars(28)
                .build();
            row.append(&label);
            let remove = gtk::Button::builder()
                .icon_name("window-close-symbolic")
                .tooltip_text("Remove reference")
                .build();
            remove.add_css_class("flat");
            row.append(&remove);
            let weak = Rc::downgrade(self);
            let key = key.to_owned();
            let id = item.id.clone();
            remove.connect_clicked(move |_| {
                if let Some(ui) = weak.upgrade() {
                    if let Some(callback) = ui.on_remove.borrow().as_ref() {
                        callback(key.clone(), id.clone());
                    }
                }
            });
            self.chips.insert(&row, -1);
        }
    }
}
impl Drop for Tools {
    fn drop(&mut self) {
        self.popover.unparent();
    }
}

#[cfg(debug_assertions)]
impl Tools {
    pub fn smoke_has(&self, name: &str) -> bool {
        self.choices
            .borrow()
            .iter()
            .any(|choice| choice.name == name)
    }
    pub fn smoke_pick(&self, name: &str) -> Result<(), String> {
        let index = self
            .choices
            .borrow()
            .iter()
            .position(|choice| choice.name == name)
            .ok_or_else(|| format!("Reference choice missing: {name}"))?;
        let row = self
            .list
            .row_at_index(index as i32)
            .ok_or("Reference row missing")?;
        self.list.emit_by_name::<()>("row-activated", &[&row]);
        Ok(())
    }
    pub fn smoke_remove_first(&self) -> Result<(), String> {
        let button = self
            .chips
            .first_child()
            .and_then(|child| child.first_child())
            .and_then(|row| row.last_child())
            .and_downcast::<gtk::Button>()
            .ok_or("Reference remove button missing")?;
        button.emit_clicked();
        Ok(())
    }
    pub fn smoke_capture(&self, path: &std::path::Path) -> Result<(), String> {
        if let Some(clock) = self.popover.frame_clock() {
            clock.emit_by_name::<()>("update", &[]);
            clock.emit_by_name::<()>("update", &[]);
        }
        self.popover.present();
        let snapshot = gtk::Snapshot::new();
        // GtkPopover wraps the public content in an internal styled child.
        let child = self
            .popover
            .first_child()
            .ok_or("Reference popup is empty")?;
        if self.popover.width() == 0 || self.popover.height() == 0 {
            return Err("Reference popup was not allocated".into());
        }
        self.popover.snapshot_child(&child, &snapshot);
        let node = snapshot
            .to_node()
            .ok_or("Reference popup has no snapshot")?;
        let texture = self
            .popover
            .renderer()
            .ok_or("Reference popup has no renderer")?
            .render_texture(&node, None);
        texture.save_to_png(path).map_err(|error| error.to_string())
    }
}
