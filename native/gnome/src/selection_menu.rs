//! Shared context actions for bounded, mounted, read-only native selections.
use adw::prelude::*;
use gtk::gio;
use std::rc::Rc;

pub type Capture = Rc<dyn Fn(Option<String>)>;

fn contains(model: &gio::MenuModel, action: &str) -> bool {
    (0..model.n_items()).any(|i| {
        model
            .item_attribute_value(i, "action", None)
            .is_some_and(|v| v.str() == Some(action))
            || ["section", "submenu"].iter().any(|link| {
                model
                    .item_link(i, link)
                    .is_some_and(|child| contains(&child, action))
            })
    })
}

pub fn bind(widget: &gtk::Widget, namespace: &str, capture: Capture) {
    let dynamic_capture = capture.clone();
    let dynamic_namespace = namespace.to_owned();
    if crate::rich_paragraph::when_ready(
        widget,
        Rc::new(move |child| {
            bind(child, &dynamic_namespace, dynamic_capture.clone());
        }),
    ) {
        return;
    }
    let captured = capture.clone();
    action(
        widget,
        namespace,
        Rc::new(move |widget| {
            let selection = crate::content_widgets::selection(widget, 16 * 1024);
            if selection.is_some() {
                if let Some(window) = widget.root().and_downcast::<gtk::Window>() {
                    if window.widget_name() == "studio-selection-window" {
                        window.close();
                    }
                }
            }
            captured(selection);
        }),
    );
    let mut child = widget.first_child();
    while let Some(widget) = child {
        child = widget.next_sibling();
        bind(&widget, namespace, capture.clone());
    }
}

/// Install on this leaf only. Projected TextViews extract model text on a worker;
/// recursively installing their action on child anchors would mislabel cell text.
pub fn action(widget: &gtk::Widget, namespace: &str, invoke: Rc<dyn Fn(&gtk::Widget)>) {
    let label = widget
        .downcast_ref::<gtk::Label>()
        .filter(|l| l.is_selectable());
    let view = widget
        .downcast_ref::<gtk::TextView>()
        .filter(|v| !v.is_editable());
    if label.is_some() || view.is_some() {
        let action = gio::SimpleAction::new("capture", None);
        let weak = widget.downgrade();
        action.connect_activate(move |_, _| {
            if let Some(widget) = weak.upgrade().filter(|w| w.is_mapped()) {
                invoke(&widget);
            }
        });
        let group = gio::SimpleActionGroup::new();
        group.add_action(&action);
        widget.insert_action_group(namespace, Some(&group));
        let existing = label
            .and_then(|l| l.extra_menu())
            .or_else(|| view.and_then(|v| v.extra_menu()));
        let action_name = format!("{namespace}.capture");
        if !existing.as_ref().is_some_and(|m| contains(m, &action_name)) {
            let menu = gio::Menu::new();
            if let Some(existing) = existing {
                menu.append_section(None, &existing);
            }
            menu.append(
                Some(&crate::language::tr("Comment on selected text")),
                Some(&action_name),
            );
            crate::i18n_ui::menu_label(&menu, menu.n_items() - 1, "Comment on selected text");
            if let Some(label) = label {
                label.set_extra_menu(Some(&menu));
            } else if let Some(view) = view {
                view.set_extra_menu(Some(&menu));
            }
        }
    }
}
