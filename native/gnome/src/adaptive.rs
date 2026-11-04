//! Shared native layout primitives. Leave measuring/wrapping to libadwaita.
use adw::prelude::*;

/// A toolbar retains its order and wraps before squeezing labels to ellipses.
/// Flexible labels/selectors consume spare space; short action-only lines align right.
pub fn toolbar(spacing: i32) -> adw::WrapBox {
    adw::WrapBox::builder()
        .child_spacing(spacing)
        .line_spacing(6)
        .wrap_policy(adw::WrapPolicy::Natural)
        .justify(adw::JustifyMode::None)
        .align(1.0)
        .build()
}

/// Keep the selected value compact without truncating model IDs or popup text.
pub fn string_selector(dropdown: &gtk::DropDown, characters: i32) {
    string_selector_impl(dropdown, characters, false, None);
}
/// UI-owned choices keep the same bounded/wrapped popup and selection marker.
/// Model names use `string_selector` and remain literal, even if they match a key.
pub fn ui_string_selector(
    dropdown: &gtk::DropDown,
    characters: i32,
    context: Option<&'static str>,
) {
    string_selector_impl(dropdown, characters, true, context);
}
fn string_selector_impl(
    dropdown: &gtk::DropDown,
    characters: i32,
    ui: bool,
    context: Option<&'static str>,
) {
    fn factory(
        characters: i32,
        popup: bool,
        ui: bool,
        context: Option<&'static str>,
    ) -> gtk::SignalListItemFactory {
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(move |_, object| {
            let item = object.downcast_ref::<gtk::ListItem>().unwrap();
            let label = gtk::Label::builder()
                .xalign(0.0)
                .width_chars(if popup {
                    1
                } else if characters >= 20 {
                    8
                } else {
                    4
                })
                .max_width_chars(characters)
                .wrap(popup)
                .wrap_mode(gtk::pango::WrapMode::WordChar)
                .ellipsize(if popup {
                    gtk::pango::EllipsizeMode::None
                } else {
                    gtk::pango::EllipsizeMode::End
                })
                .build();
            if popup {
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                label.set_hexpand(true);
                row.append(&label);
                let selected = gtk::Image::from_icon_name("object-select-symbolic");
                selected.set_valign(gtk::Align::Start);
                selected.set_margin_top(3);
                selected.set_opacity(if item.is_selected() { 1.0 } else { 0.0 });
                row.append(&selected);
                let weak = selected.downgrade();
                item.connect_selected_notify(move |item| {
                    if let Some(selected) = weak.upgrade() {
                        selected.set_opacity(if item.is_selected() { 1.0 } else { 0.0 });
                    }
                });
                item.set_child(Some(&row));
            } else {
                item.set_child(Some(&label));
            }
        });
        factory.connect_bind(move |_, object| {
            let item = object.downcast_ref::<gtk::ListItem>().unwrap();
            let child = item.child().unwrap();
            let label = child
                .clone()
                .downcast::<gtk::Label>()
                .ok()
                .or_else(|| child.first_child().and_downcast::<gtk::Label>())
                .unwrap();
            let text = item
                .item()
                .and_downcast::<gtk::StringObject>()
                .map(|s| s.string())
                .unwrap_or_default();
            for property in ["label", "tooltip-text"] {
                if ui {
                    let key = context.map_or_else(
                        || text.to_string(),
                        |context| format!("{context}\u{4}{text}"),
                    );
                    crate::i18n_ui::bind(&label, property, &key);
                } else {
                    crate::i18n_ui::literal(&label, property, &text);
                }
            }
        });
        factory
    }
    dropdown.set_factory(Some(&factory(characters, false, ui, context)));
    dropdown.set_list_factory(Some(&factory(64, true, ui, context)));
    dropdown.set_valign(gtk::Align::Center);
}

/// Keep linked mode buttons usable with large fonts in a narrow document pane.
pub fn compact_button(button: &impl IsA<gtk::Button>) {
    let button = button.as_ref();
    if let Some(label) = button.child().and_downcast::<gtk::Label>() {
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        label.set_width_chars(1);
        button.set_tooltip_text(button.label().as_deref());
    }
}
