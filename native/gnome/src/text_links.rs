//! Shared read-only TextView link input. Callers retain routing/source policy.
use adw::prelude::*;
use gtk::{gdk, gio, glib};
use std::rc::Rc;

pub type Resolve = Rc<dyn Fn(&gtk::TextView, usize) -> Option<String>>;
pub type Open = Rc<dyn Fn(&str)>;

pub fn bind(
    view: &gtk::TextView,
    actions: &gio::SimpleActionGroup,
    enabled: bool,
    resolve: Resolve,
    open: Open,
) {
    let action = gio::SimpleAction::new("open-link", None);
    action.set_enabled(enabled);
    let weak = view.downgrade();
    let (find, follow) = (resolve.clone(), open.clone());
    action.connect_activate(move |_, _| {
        if let Some(view) = weak.upgrade().filter(|v| v.is_mapped()) {
            if let Some(destination) = find(&view, view.buffer().cursor_position() as usize) {
                follow(&destination);
            }
        }
    });
    actions.add_action(&action);
    let keys = gtk::EventControllerKey::new();
    let weak = view.downgrade();
    keys.connect_key_pressed(move |_, key, _, modifiers| {
        if modifiers.contains(gdk::ModifierType::CONTROL_MASK)
            && matches!(key, gdk::Key::Return | gdk::Key::KP_Enter)
        {
            if let Some(view) = weak.upgrade().filter(|v| v.is_mapped()) {
                let _ = view.activate_action("rich.open-link", None);
            }
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    view.add_controller(keys);
    let click = gtk::GestureClick::new();
    click.set_button(1);
    let weak = view.downgrade();
    click.connect_released(move |gesture, presses, x, y| {
        if presses != 1
            || !gesture
                .current_event_state()
                .contains(gdk::ModifierType::CONTROL_MASK)
        {
            return;
        }
        let Some(view) = weak.upgrade().filter(|v| v.is_mapped()) else {
            return;
        };
        if view.buffer().has_selection() {
            return;
        }
        let (x, y) = view.window_to_buffer_coords(gtk::TextWindowType::Widget, x as i32, y as i32);
        // Hit-test the character, not its nearest insertion cursor. The latter
        // skips narrow glyphs and the trailing half of a link's last glyph.
        let Some((iter, _)) = view.iter_at_position(x, y) else {
            return;
        };
        let rect = view.iter_location(&iter);
        if x < rect.x()
            || x >= rect.x() + rect.width()
            || y < rect.y()
            || y >= rect.y() + rect.height()
        {
            return;
        }
        if let Some(destination) = resolve(&view, iter.offset() as usize) {
            open(&destination);
        }
    });
    view.add_controller(click);
}
