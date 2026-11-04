//! Native named action rows using public GTK widgets, not private model buttons.
use adw::prelude::*;
use gtk::{gdk, gio, glib};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// GDK positions belong to their event surface; popover anchors belong to the
/// attached widget. Account for native surface insets before crossing widgets.
pub fn event_point(widget: &impl IsA<gtk::Widget>, event: &gdk::Event) -> Option<(f64, f64)> {
    let (x, y) = event.position()?;
    let native = gtk::Native::for_surface(&event.surface()?)?;
    let (dx, dy) = native.surface_transform();
    let point = native.upcast_ref::<gtk::Widget>().compute_point(
        widget,
        &gtk::graphene::Point::new((x + dx) as f32, (y + dy) as f32),
    )?;
    Some((point.x() as f64, point.y() as f64))
}

fn text(model: &gio::MenuModel, index: i32, key: &str) -> Option<String> {
    model
        .item_attribute_value(index, key, None)?
        .str()
        .map(str::to_owned)
}

fn append(
    model: &gio::MenuModel,
    body: &gtk::Box,
    popover: &gtk::Popover,
    rows: &mut Vec<gtk::Button>,
    budget: &mut usize,
    depth: usize,
) {
    if depth > 4 {
        return;
    }
    for index in 0..model.n_items() {
        if *budget == 0 {
            return;
        }
        *budget -= 1;
        if let Some(section) = model.item_link(index, "section") {
            body.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
            if let Some(title) = text(model, index, "label") {
                let label = gtk::Label::new(Some(&title));
                label.set_xalign(0.0);
                label.add_css_class("dim-label");
                label.set_margin_start(12);
                label.set_margin_end(12);
                body.append(&label);
            }
            append(&section, body, popover, rows, budget, depth + 1);
            continue;
        }
        let (Some(label), Some(action)) =
            (text(model, index, "label"), text(model, index, "action"))
        else {
            continue;
        };
        let button = gtk::Button::builder()
            .accessible_role(gtk::AccessibleRole::MenuItem)
            .action_name(&action)
            .build();
        let caption = gtk::Label::new(Some(&label));
        caption.set_xalign(0.0);
        button.set_child(Some(&caption));
        button.add_css_class("flat");
        button.set_action_target_value(model.item_attribute_value(index, "target", None).as_ref());
        let weak = popover.downgrade();
        button.connect_clicked(move |_| {
            if let Some(popover) = weak.upgrade() {
                popover.popdown();
            }
        });
        body.append(&button);
        rows.push(button);
    }
}

fn rebuild(
    model: &gio::MenuModel,
    body: &gtk::Box,
    popover: &gtk::Popover,
    rows: &RefCell<Vec<gtk::Button>>,
) {
    let focused = rows
        .borrow()
        .iter()
        .find(|row| row.has_focus())
        .and_then(|row| row.action_name());
    while let Some(child) = body.first_child() {
        body.remove(&child);
    }
    let mut next = Vec::new();
    append(model, body, popover, &mut next, &mut 128, 0);
    if popover.is_mapped() {
        if let Some(row) = next
            .iter()
            .find(|row| row.action_name() == focused && row.is_sensitive())
        {
            row.grab_focus();
        }
    }
    *rows.borrow_mut() = next;
}

/// This shell menu contains actions and sections; its GMenu remains the source
/// of translated labels, targets and action identities. Actionable propagates
/// the window's enabled state without a second action-dispatch implementation.
pub fn popover(model: &gio::Menu) -> gtk::Popover {
    let body = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .accessible_role(gtk::AccessibleRole::Menu)
        .build();
    let scroll = gtk::ScrolledWindow::builder()
        .child(&body)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .max_content_height(600)
        .build();
    let popover = gtk::Popover::builder().child(&scroll).build();
    let rows = Rc::new(RefCell::new(Vec::new()));
    rebuild(model.upcast_ref(), &body, &popover, &rows);
    // Retain the model while the popup exists, including live translated labels
    // on context menus whose caller does not otherwise retain a GMenu.
    let current_model = model.clone();
    let current_body = body.downgrade();
    let current_rows = rows.clone();
    popover.connect_show(move |popup| {
        if let Some(body) = current_body.upgrade() {
            rebuild(current_model.upcast_ref(), &body, popup, &current_rows);
        }
    });
    let pending = Rc::new(Cell::new(false));
    let weak_body = body.downgrade();
    let weak_popover = popover.downgrade();
    let update_rows = rows.clone();
    model.connect_items_changed(move |model, _, _, _| {
        if pending.replace(true) {
            return;
        }
        let model = model.clone();
        let (body, popover, rows, pending) = (
            weak_body.clone(),
            weak_popover.clone(),
            update_rows.clone(),
            pending.clone(),
        );
        glib::idle_add_local_once(move || {
            pending.set(false);
            if let (Some(body), Some(popover)) = (body.upgrade(), popover.upgrade()) {
                rebuild(model.upcast_ref(), &body, &popover, &rows);
            }
        });
    });
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed(move |_, key, _, modifiers| {
        if modifiers.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::ALT_MASK) {
            return glib::Propagation::Proceed;
        }
        let rows = rows.borrow();
        let available: Vec<_> = rows
            .iter()
            .filter(|row| row.is_sensitive() && row.is_visible())
            .collect();
        if available.is_empty() {
            return glib::Propagation::Proceed;
        }
        let current = available.iter().position(|row| row.has_focus());
        let index = match key {
            gdk::Key::Home => 0,
            gdk::Key::End => available.len() - 1,
            gdk::Key::Down => current.map_or(0, |i| (i + 1) % available.len()),
            gdk::Key::Up => current.map_or(available.len() - 1, |i| {
                (i + available.len() - 1) % available.len()
            }),
            _ => return glib::Propagation::Proceed,
        };
        available[index].grab_focus();
        glib::Propagation::Stop
    });
    popover.add_controller(keys);
    popover
}
pub fn button(model: &gio::Menu) -> gtk::MenuButton {
    gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .popover(&popover(model))
        .build()
}
