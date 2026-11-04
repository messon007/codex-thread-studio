use crate::i18n_ui::{self, UiText};
use crate::{composer_refs::Suggestion, composer_tools_ui::Context, protocol::Command};
use adw::prelude::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
use tokio::sync::mpsc;
static NEXT: AtomicU64 = AtomicU64::new(1);
pub struct Palette {
    pub dialog: adw::Dialog,
    pub context: Context,
    id: u64,
    serial: Cell<u64>,
    search: gtk::Entry,
    list: gtk::ListBox,
    scroll: gtk::ScrolledWindow,
    status: gtk::Label,
    choices: RefCell<Vec<Arc<Suggestion>>>,
    sender: mpsc::Sender<Command>,
    pub on_action: RefCell<Option<Box<dyn Fn(String)>>>,
}

#[cfg(debug_assertions)]
impl Palette {
    pub fn smoke_query(&self, query: &str) {
        self.search.set_text(query);
    }
    pub fn smoke_has(&self, name: &str) -> bool {
        self.choices
            .borrow()
            .iter()
            .any(|choice| choice.name == name)
    }
    pub fn smoke_identity(&self) -> (u64, u64) {
        (self.id, self.serial.get())
    }
    pub fn smoke_pick(&self, name: &str) -> Result<(), String> {
        let index = self
            .choices
            .borrow()
            .iter()
            .position(|choice| choice.name == name)
            .ok_or("Command choice missing")?;
        let row = self
            .list
            .row_at_index(index as i32)
            .ok_or("Command row missing")?;
        self.list.emit_by_name::<()>("row-activated", &[&row]);
        Ok(())
    }
}
impl Palette {
    pub fn new(context: Context, sender: mpsc::Sender<Command>) -> Rc<Self> {
        let dialog = adw::Dialog::builder()
            .title("Commands")
            .content_width(540)
            .content_height(520)
            .build()
            .ui_text("title", "Commands");
        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(
            &adw::WindowTitle::new("Commands", "Selected session")
                .ui_text("title", "Commands")
                .ui_text("subtitle", "Selected session"),
        ));
        toolbar.add_top_bar(&header);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        body.set_margin_start(16);
        body.set_margin_end(16);
        body.set_margin_bottom(16);
        let search = gtk::Entry::builder()
            .placeholder_text("Search commands…")
            .primary_icon_name("system-search-symbolic")
            .max_length(256)
            .build();
        i18n_ui::bind(&search, "placeholder-text", "Search commands…");
        i18n_ui::accessible_label(&search, "Search commands…");
        body.append(&search);
        let list = gtk::ListBox::new();
        list.add_css_class("boxed-list");
        list.set_selection_mode(gtk::SelectionMode::Single);
        let scroll = gtk::ScrolledWindow::builder()
            .child(&list)
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();
        body.append(&scroll);
        let status = gtk::Label::builder().xalign(0.0).wrap(true).build();
        status.add_css_class("caption");
        status.add_css_class("dim-label");
        body.append(&status);
        toolbar.set_content(Some(&body));
        dialog.set_child(Some(&toolbar));
        let ui = Rc::new(Self {
            dialog,
            context,
            id: NEXT.fetch_add(1, Ordering::Relaxed),
            serial: Cell::new(0),
            search,
            list,
            scroll,
            status,
            choices: RefCell::new(Vec::new()),
            sender,
            on_action: RefCell::new(None),
        });
        let weak = Rc::downgrade(&ui);
        ui.search.connect_changed(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.query();
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.search.connect_activate(move |_| {
            if let Some(ui) = weak.upgrade() {
                let index = ui.list.selected_row().map(|row| row.index()).unwrap_or(0);
                ui.pick(index);
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.list.connect_row_activated(move |_, row| {
            if let Some(ui) = weak.upgrade() {
                ui.pick(row.index());
            }
        });
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Bubble);
        let weak = Rc::downgrade(&ui);
        keys.connect_key_pressed(move |_, key, _, state| {
            let Some(ui) = weak.upgrade() else {
                return gtk::glib::Propagation::Proceed;
            };
            use gtk::gdk::{Key, ModifierType};
            if matches!(key, Key::Return | Key::KP_Enter)
                && state.contains(ModifierType::CONTROL_MASK)
            {
                return gtk::glib::Propagation::Stop;
            }
            if key == Key::Escape {
                ui.dialog.close();
                return gtk::glib::Propagation::Stop;
            }
            if matches!(key, Key::Up | Key::Down)
                && !state.intersects(
                    ModifierType::CONTROL_MASK | ModifierType::ALT_MASK | ModifierType::SUPER_MASK,
                )
            {
                let count = ui.choices.borrow().len() as i32;
                if count > 0 {
                    let current = ui.list.selected_row().map(|row| row.index()).unwrap_or(0);
                    let next = if key == Key::Down {
                        (current + 1) % count
                    } else {
                        (current + count - 1) % count
                    };
                    if let Some(row) = ui.list.row_at_index(next) {
                        ui.list.select_row(Some(&row));
                        if let Some(bounds) = row.compute_bounds(&ui.list) {
                            let adjustment = ui.scroll.vadjustment();
                            let top = bounds.y() as f64;
                            let bottom = top + bounds.height() as f64;
                            let value = adjustment.value();
                            let page = adjustment.page_size();
                            adjustment.set_value(if top < value {
                                top
                            } else if bottom > value + page {
                                bottom - page
                            } else {
                                value
                            });
                        }
                    }
                    return gtk::glib::Propagation::Stop;
                }
            }
            gtk::glib::Propagation::Proceed
        });
        ui.dialog.add_controller(keys);
        ui.query();
        ui
    }
    fn query(&self) {
        self.serial.set(self.serial.get() + 1);
        self.choices.borrow_mut().clear();
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        i18n_ui::bind(&self.status, "label", "Finding commands…");
        if self
            .sender
            .try_send(Command::CommandPalette {
                dialog: self.id,
                serial: self.serial.get(),
                query: self.search.text().to_string(),
                opencode: self.context.opencode,
            })
            .is_err()
        {
            i18n_ui::bind(
                &self.status,
                "label",
                "Native workers are busy; type again to retry",
            );
        }
    }
    pub fn apply(&self, dialog: u64, serial: u64, result: Result<Vec<Arc<Suggestion>>, String>) {
        if dialog != self.id || serial != self.serial.get() {
            return;
        }
        let choices = match result {
            Ok(choices) => choices,
            Err(error) => {
                i18n_ui::literal(&self.status, "label", &error);
                return;
            }
        };
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        for item in &choices {
            let row = gtk::Box::new(gtk::Orientation::Vertical, 3);
            row.set_margin_start(12);
            row.set_margin_end(12);
            row.set_margin_top(8);
            row.set_margin_bottom(8);
            let title = gtk::Label::builder().label(&item.title).xalign(0.0).build();
            row.append(&title);
            let detail = gtk::Label::builder()
                .label(&item.detail)
                .xalign(0.0)
                .wrap(true)
                .build();
            // Command descriptions come from the fixed native command catalog.
            i18n_ui::bind(&detail, "label", &item.detail);
            detail.add_css_class("caption");
            detail.add_css_class("dim-label");
            row.append(&detail);
            self.list.append(&row);
        }
        i18n_ui::bind(
            &self.status,
            "label",
            if choices.is_empty() {
                "No matching commands"
            } else {
                "Enter to run · Escape to close"
            },
        );
        self.choices.replace(choices);
        if let Some(row) = self.list.row_at_index(0) {
            self.list.select_row(Some(&row));
        }
    }
    fn pick(&self, index: i32) {
        let name = self
            .choices
            .borrow()
            .get(index.max(0) as usize)
            .map(|choice| choice.name.clone());
        if let Some(name) = name {
            self.dialog.close();
            if let Some(callback) = self.on_action.borrow().as_ref() {
                callback(name);
            }
        }
    }
    pub fn present(&self, parent: &adw::ApplicationWindow) {
        self.dialog.present(Some(parent));
        self.search.grab_focus();
    }
}
