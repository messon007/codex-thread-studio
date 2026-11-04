//! Explicit UI-property bindings. Never walk or translate document/user widgets.
use adw::prelude::*;
use gtk::glib;
use std::{cell::RefCell, collections::HashMap};
#[derive(Clone, PartialEq, Eq, Hash)]
enum Target {
    Property(&'static str),
    Response(String),
    AccessibleLabel,
    MenuLabel(i32),
}
impl Target {
    fn current(&self, object: &glib::Object, last: &str) -> bool {
        match self {
            Self::Property(property) => {
                object.property::<Option<String>>(property).as_deref() == Some(last)
            }
            // libadwaita has no response-label getter. These bindings own only
            // explicitly registered UI action labels, never provider text.
            Self::Response(id) => {
                if let Some(dialog) = object.downcast_ref::<adw::AlertDialog>() {
                    dialog.has_response(id)
                } else if let Some(dialog) = object.downcast_ref::<adw::MessageDialog>() {
                    dialog.has_response(id)
                } else {
                    false
                }
            }
            // GTK provides no accessible-label getter for widgets. This target
            // exclusively owns explicitly registered fixed interface labels.
            Self::AccessibleLabel => object.is::<gtk::Widget>(),
            Self::MenuLabel(index) => object.downcast_ref::<gtk::gio::Menu>().is_some_and(|menu| {
                menu.item_attribute_value(*index, "label", None)
                    .is_some_and(|v| v.str() == Some(last))
            }),
        }
    }
    fn write(&self, object: &glib::Object, value: &str) {
        match self {
            Self::Property(property) => object.set_property(property, value),
            Self::Response(id) => {
                if let Some(dialog) = object.downcast_ref::<adw::AlertDialog>() {
                    if dialog.has_response(id) {
                        dialog.set_response_label(id, value);
                    }
                } else if let Some(dialog) = object.downcast_ref::<adw::MessageDialog>() {
                    if dialog.has_response(id) {
                        dialog.set_response_label(id, value);
                    }
                }
            }
            Self::AccessibleLabel => {
                if let Some(widget) = object.downcast_ref::<gtk::Widget>() {
                    widget.update_property(&[gtk::accessible::Property::Label(value)]);
                }
            }
            Self::MenuLabel(index) => {
                if let Some(menu) = object
                    .downcast_ref::<gtk::gio::Menu>()
                    .filter(|m| *index < m.n_items())
                {
                    let item = gtk::gio::MenuItem::from_model(menu, *index);
                    item.set_label(Some(value));
                    menu.remove(*index);
                    menu.insert_item(*index, &item);
                }
            }
        }
    }
}
#[derive(Clone)]
pub struct Part {
    source: Option<&'static str>,
    text: String,
    variables: Vec<(String, String)>,
}
impl Part {
    pub fn literal(text: impl Into<String>) -> Self {
        Self {
            source: None,
            text: text.into(),
            variables: Vec::new(),
        }
    }
    pub fn message(source: &'static str, variables: &[(&str, &str)]) -> Self {
        Self {
            source: Some(source),
            text: String::new(),
            variables: variables
                .iter()
                .map(|(k, v)| ((*k).into(), (*v).into()))
                .collect(),
        }
    }
    fn render(&self) -> String {
        match self.source {
            None => self.text.clone(),
            Some(source) => crate::language::format(
                source,
                &self
                    .variables
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.as_str()))
                    .collect::<Vec<_>>(),
            ),
        }
    }
}
#[derive(Clone)]
struct Binding {
    object: glib::WeakRef<glib::Object>,
    target: Target,
    source: String,
    variables: Vec<(String, String)>,
    details: Vec<String>,
    parts: Option<(Vec<Part>, String)>,
    last: String,
    id: u64,
}
#[derive(Default)]
struct Registry {
    next: u64,
    bindings: HashMap<(usize, Target), Binding>,
}
thread_local! { static REGISTRY:RefCell<Registry>=RefCell::default(); }
/// Mark an interface-owned property when constructing a widget. Unlike a
/// translated constructor argument, this also follows subsequent locale changes.
pub trait UiText: IsA<glib::Object> + Sized {
    fn ui_text(self, property: &'static str, source: &str) -> Self {
        bind(&self, property, source);
        self
    }
}
impl<T: IsA<glib::Object>> UiText for T {}
pub fn bind(object: &impl IsA<glib::Object>, property: &'static str, source: &str) {
    bind_format(object, property, source, &[]);
}
pub fn bind_format(
    object: &impl IsA<glib::Object>,
    property: &'static str,
    source: &str,
    variables: &[(&str, &str)],
) {
    register(
        object.as_ref(),
        Target::Property(property),
        source,
        variables,
    );
}
/// Compose optional interface fragments and opaque source identifiers without
/// translating the latter or registering every possible fragment combination.
pub fn bind_parts(
    object: &impl IsA<glib::Object>,
    property: &'static str,
    parts: Vec<Part>,
    separator: &str,
) {
    install(
        object.as_ref(),
        Target::Property(property),
        "",
        &[],
        &[],
        Some((parts, separator.into())),
    );
}
/// A formatted status followed by optional, UI-owned translated details.
/// Variable values remain literal; only the explicit message keys are translated.
pub fn bind_summary(
    object: &impl IsA<glib::Object>,
    property: &'static str,
    source: &str,
    variables: &[(&str, &str)],
    details: &[&str],
) {
    register_details(
        object.as_ref(),
        Target::Property(property),
        source,
        variables,
        details,
    );
}
/// Register a fixed UI label after adding its response. IDs, default/close
/// decisions, response availability and provider payloads are never translated.
pub fn response(object: &impl IsA<glib::Object>, id: &str, source: &str) {
    register(object.as_ref(), Target::Response(id.into()), source, &[]);
}
/// Fixed UI-owned accessible names, independent of editable text/placeholder.
/// Do not register provider questions, file names or other model content here.
pub fn accessible_label(widget: &impl IsA<gtk::Widget>, source: &str) {
    let widget: &gtk::Widget = widget.as_ref();
    register(widget.upcast_ref(), Target::AccessibleLabel, source, &[]);
}
/// Compose translated interface cues with literal model text in an accessible
/// name, using the same live-language lifetime rules as visible label parts.
pub fn accessible_parts(widget: &impl IsA<gtk::Widget>, parts: Vec<Part>, separator: &str) {
    let widget: &gtk::Widget = widget.as_ref();
    install(
        widget.upcast_ref(),
        Target::AccessibleLabel,
        "",
        &[],
        &[],
        Some((parts, separator.into())),
    );
}
/// GTK widgets expose accessible labels through GtkAccessible, rather than a
/// GObject string property. Explicit model assignments also retire UI bindings.
pub fn literal_accessible_label(widget: &impl IsA<gtk::Widget>, value: &str) {
    let widget: &gtk::Widget = widget.as_ref();
    REGISTRY.with_borrow_mut(|registry| {
        registry
            .bindings
            .remove(&(widget.as_ptr() as usize, Target::AccessibleLabel));
    });
    widget.update_property(&[gtk::accessible::Property::Label(value)]);
}
/// Bind a fixed interface menu entry after its sections/items have been built.
/// Retains its action/links and never walks or translates model-generated menus.
pub fn menu_label(menu: &gtk::gio::Menu, index: i32, source: &str) {
    register(menu.upcast_ref(), Target::MenuLabel(index), source, &[]);
}
/// Replace a previously bound UI property with literal model/provider content,
/// including when that content happens to equal the previous visible UI text.
pub fn literal(object: &impl IsA<glib::Object>, property: &'static str, value: &str) {
    let object: &glib::Object = object.as_ref();
    REGISTRY.with_borrow_mut(|registry| {
        registry
            .bindings
            .remove(&(object.as_ptr() as usize, Target::Property(property)));
    });
    object.set_property(property, value);
}
fn register(object: &glib::Object, target: Target, source: &str, variables: &[(&str, &str)]) {
    register_details(object, target, source, variables, &[]);
}
fn summary(source: &str, variables: &[(&str, &str)], details: &[impl AsRef<str>]) -> String {
    let mut text = crate::language::format(source, variables);
    for detail in details {
        text.push_str(" · ");
        text.push_str(&crate::language::tr(detail.as_ref()));
    }
    text
}
fn register_details(
    object: &glib::Object,
    target: Target,
    source: &str,
    variables: &[(&str, &str)],
    details: &[&str],
) {
    install(object, target, source, variables, details, None);
}
fn install(
    object: &glib::Object,
    target: Target,
    source: &str,
    variables: &[(&str, &str)],
    details: &[&str],
    parts: Option<(Vec<Part>, String)>,
) {
    let translated = match &parts {
        Some((parts, separator)) => parts
            .iter()
            .map(Part::render)
            .collect::<Vec<_>>()
            .join(separator),
        None => summary(source, variables, details),
    };
    let key = (object.as_ptr() as usize, target.clone());
    REGISTRY.with_borrow_mut(|registry| {
        if registry.next % 128 == 0 {
            registry
                .bindings
                .retain(|_, binding| binding.object.upgrade().is_some());
        }
        registry.next = registry.next.wrapping_add(1);
        registry.bindings.insert(
            key,
            Binding {
                object: object.downgrade(),
                target: target.clone(),
                source: source.into(),
                variables: variables
                    .iter()
                    .map(|(key, value)| ((*key).into(), (*value).into()))
                    .collect(),
                details: details.iter().map(|text| (*text).into()).collect(),
                parts,
                last: translated.clone(),
                id: registry.next,
            },
        );
    });
    target.write(object, &translated);
}
pub fn apply() {
    let bindings = REGISTRY.with_borrow_mut(|registry| {
        registry
            .bindings
            .retain(|_, b| b.object.upgrade().is_some());
        registry
            .bindings
            .iter()
            .map(|(key, b)| (key.clone(), b.clone()))
            .collect::<Vec<_>>()
    });
    for (key, binding) in bindings {
        let Some(object) = binding.object.upgrade() else {
            continue;
        };
        // A later model/user assignment supersedes the old UI binding. It must
        // never be overwritten with an obsolete status or translated content.
        if !binding.target.current(&object, &binding.last) {
            REGISTRY.with_borrow_mut(|r| {
                if r.bindings.get(&key).is_some_and(|b| b.id == binding.id) {
                    r.bindings.remove(&key);
                }
            });
            continue;
        }
        let variables = binding
            .variables
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect::<Vec<_>>();
        let translated = match &binding.parts {
            Some((parts, separator)) => parts
                .iter()
                .map(Part::render)
                .collect::<Vec<_>>()
                .join(separator),
            None => summary(&binding.source, &variables, &binding.details),
        };
        binding.target.write(&object, &translated);
        REGISTRY.with_borrow_mut(|r| {
            if let Some(b) = r.bindings.get_mut(&key).filter(|b| b.id == binding.id) {
                b.last = translated;
            }
        });
    }
}
/// Use only with UI-owned choice keys; the underlying values and selection stay
/// unchanged. Backend names, file paths and editable values are not registered.
pub fn choices(dropdown: &gtk::DropDown) {
    dropdown.set_factory(Some(&choice_factory()));
    dropdown.set_list_factory(Some(&choice_factory()));
}
pub fn choice_factory() -> gtk::SignalListItemFactory {
    choice_factory_with_context(None)
}
pub fn contextual_choice_factory(context: &'static str) -> gtk::SignalListItemFactory {
    choice_factory_with_context(Some(context))
}
fn choice_factory_with_context(context: Option<&'static str>) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, object| {
        let item = object.downcast_ref::<gtk::ListItem>().unwrap();
        item.set_child(Some(
            &gtk::Label::builder()
                .xalign(0.0)
                .max_width_chars(30)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .build(),
        ));
    });
    factory.connect_bind(move |_, object| {
        let item = object.downcast_ref::<gtk::ListItem>().unwrap();
        if let (Some(label), Some(value)) = (
            item.child().and_downcast::<gtk::Label>(),
            item.item().and_downcast::<gtk::StringObject>(),
        ) {
            let source = match context {
                Some(context) => format!("{context}\u{4}{}", value.string()),
                None => value.string().to_string(),
            };
            bind(&label, "label", &source);
        }
    });
    factory
}
