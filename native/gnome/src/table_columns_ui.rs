//! Shared virtual column picker. Source labels stay literal; UI labels bind live.
use crate::table_columns::Selection;
use adw::prelude::*;
use gtk::{gio, glib};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
type Changed = Rc<dyn Fn(&[usize]) -> bool>;
pub fn row_status(label: &gtk::Label, start: usize, end: usize, total: usize) {
    crate::i18n_ui::bind_format(
        label,
        "label",
        "{start}–{end} of {total} rows",
        &[
            ("start", &start.to_string()),
            ("end", &end.to_string()),
            ("total", &total.to_string()),
        ],
    );
}
struct Choice {
    index: usize,
    title: String,
}
struct Picker {
    selection: RefCell<Selection>,
    binding: Cell<bool>,
    button: glib::WeakRef<gtk::MenuButton>,
    total: usize,
    changed: Changed,
}
impl Picker {
    fn summary(&self) {
        if let Some(button) = self.button.upgrade() {
            crate::i18n_ui::bind_format(
                &button,
                "label",
                "Columns {visible}/{total}",
                &[
                    (
                        "visible",
                        &self.selection.borrow().selected().len().to_string(),
                    ),
                    ("total", &self.total.to_string()),
                ],
            );
        }
    }
    fn bind(&self, check: &gtk::CheckButton, index: usize) {
        self.binding.set(true);
        check.set_active(self.selection.borrow().selected().contains(&index));
        self.binding.set(false);
    }
    fn toggle(&self, check: &gtk::CheckButton, index: usize) {
        if self.binding.get() {
            return;
        }
        if !check.is_mapped() {
            self.bind(check, index);
            return;
        }
        let old = self.selection.borrow().clone();
        if let Some(next) = old.propose(index, check.is_active()) {
            let proposal = next.selected().to_vec();
            *self.selection.borrow_mut() = next;
            if !(self.changed)(&proposal) {
                *self.selection.borrow_mut() = old;
            }
        }
        self.bind(check, index);
        self.summary();
    }
}
/// Caller retains its own table state and returns false if its source expired.
/// Recycled list items resolve their current choice; setup never captures an old
/// column index. This component owns no document/table widgets or source leases.
pub fn install(
    button: &gtk::MenuButton,
    names: Vec<String>,
    selected: Vec<usize>,
    limit: usize,
    changed: Changed,
) {
    let total = names.len();
    let picker = Rc::new(Picker {
        selection: RefCell::new(Selection::new(total, selected, limit)),
        binding: Cell::new(false),
        button: button.downgrade(),
        total,
        changed,
    });
    button.set_widget_name("studio-table-columns");
    button.set_sensitive(total > 0);
    crate::i18n_ui::bind(button, "tooltip-text", "Choose visible table columns");
    picker.summary();
    let model = gio::ListStore::new::<glib::BoxedAnyObject>();
    model.extend_from_slice(
        &names
            .into_iter()
            .enumerate()
            .map(|(index, title)| glib::BoxedAnyObject::new(Choice { index, title }))
            .collect::<Vec<_>>(),
    );
    let factory = gtk::SignalListItemFactory::new();
    let current = picker.clone();
    factory.connect_setup(move |_, item| {
        let item = item.downcast_ref::<gtk::ListItem>().unwrap();
        let check = gtk::CheckButton::with_label("");
        check.set_margin_start(12);
        check.set_margin_end(12);
        check.set_margin_top(4);
        check.set_margin_bottom(4);
        if let Some(label) = check.child().and_downcast::<gtk::Label>() {
            label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            label.set_max_width_chars(42);
            label.set_xalign(0.0);
        }
        let weak_item = item.downgrade();
        let current = current.clone();
        check.connect_toggled(move |check| {
            let Some(item) = weak_item.upgrade() else {
                return;
            };
            let Some(value) = item.item().and_downcast::<glib::BoxedAnyObject>() else {
                return;
            };
            let index = value.borrow::<Choice>().index;
            current.toggle(check, index);
        });
        item.set_child(Some(&check));
    });
    let current = picker.clone();
    factory.connect_bind(move |_, item| {
        let item = item.downcast_ref::<gtk::ListItem>().unwrap();
        let value = item.item().and_downcast::<glib::BoxedAnyObject>().unwrap();
        let choice = value.borrow::<Choice>();
        let check = item.child().and_downcast::<gtk::CheckButton>().unwrap();
        check.set_label(Some(&choice.title));
        check.set_tooltip_text(Some(&choice.title));
        current.bind(&check, choice.index);
    });
    let list = gtk::ListView::new(Some(gtk::NoSelection::new(Some(model))), Some(factory));
    list.set_widget_name("studio-column-choices");
    let scroll = gtk::ScrolledWindow::builder()
        .child(&list)
        .min_content_height((total.min(8) * 40).max(40) as i32)
        .max_content_height(360)
        .propagate_natural_height(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .width_request(320)
        .build();
    let popover = gtk::Popover::new();
    popover.set_child(Some(&scroll));
    button.set_popover(Some(&popover));
}
