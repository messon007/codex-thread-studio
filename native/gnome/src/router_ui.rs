use crate::i18n_ui::UiText;
use adw::prelude::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
pub struct Bar {
    pub widget: gtk::Box,
    pub label: gtk::Label,
    pub older: gtk::Button,
    pub newer: gtk::Button,
    pub latest: gtk::Button,
    pub seen: gtk::Button,
    pub source: gtk::DropDown,
    pub search: gtk::SearchEntry,
    pub search_toggle: gtk::ToggleButton,
    pub search_bar: gtk::SearchBar,
    pub case: gtk::ToggleButton,
    pub updating: Cell<bool>,
    controller: RefCell<Option<String>>,
}
impl Bar {
    pub fn new() -> Rc<Self> {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
        widget.set_margin_start(24);
        widget.set_margin_end(24);
        widget.set_margin_top(6);
        widget.set_margin_bottom(6);
        widget.set_visible(false);
        let label = gtk::Label::builder()
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        label.add_css_class("caption");
        let tools = crate::adaptive::toolbar(8);
        let source = gtk::DropDown::from_strings(&["Current requests", "Earlier browser requests"]);
        crate::adaptive::string_selector(&source, 24);
        crate::i18n_ui::choices(&source);
        crate::i18n_ui::accessible_label(&source, "Router history source");
        tools.append(&source);
        tools.append(&label);
        let navigation = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        navigation.add_css_class("linked");
        let older = gtk::Button::builder()
            .icon_name("go-previous-symbolic")
            .tooltip_text("Older Router requests")
            .build()
            .ui_text("tooltip-text", "Older Router requests");
        let newer = gtk::Button::builder()
            .icon_name("go-next-symbolic")
            .tooltip_text("Newer Router requests")
            .build()
            .ui_text("tooltip-text", "Newer Router requests");
        let latest = gtk::Button::builder()
            .icon_name("go-last-symbolic")
            .tooltip_text("Latest Router requests")
            .build()
            .ui_text("tooltip-text", "Latest Router requests");
        navigation.append(&older);
        navigation.append(&newer);
        navigation.append(&latest);
        tools.append(&navigation);
        let seen = gtk::Button::builder()
            .icon_name("mail-mark-read-symbolic")
            .tooltip_text("Mark visible Router replies read")
            .build()
            .ui_text("tooltip-text", "Mark visible Router replies read");
        tools.append(&seen);
        let search_toggle = gtk::ToggleButton::builder()
            .icon_name("system-search-symbolic")
            .tooltip_text("Search Router requests and replies")
            .build()
            .ui_text("tooltip-text", "Search Router requests and replies");
        tools.append(&search_toggle);
        widget.append(&tools);
        let search = gtk::SearchEntry::builder()
            .placeholder_text("Search saved requests and replies…")
            .tooltip_text("Literal text in saved requests, replies and target details. Spaces are significant.")
            .hexpand(true)
            .build().ui_text("placeholder-text","Search saved requests and replies…")
            .ui_text("tooltip-text","Literal text in saved requests, replies and target details. Spaces are significant.");
        let case = gtk::ToggleButton::builder()
            .label("Aa")
            .tooltip_text("Match case")
            .build()
            .ui_text("tooltip-text", "Match case");
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        line.append(&search);
        line.append(&case);
        let search_bar = gtk::SearchBar::new();
        search_bar.set_child(Some(&line));
        search_bar.connect_entry(&search);
        widget.append(&search_bar);
        Rc::new(Self {
            widget,
            label,
            older,
            newer,
            latest,
            seen,
            source,
            search,
            search_toggle,
            search_bar,
            case,
            updating: Cell::new(false),
            controller: RefCell::new(None),
        })
    }
    pub fn bind(&self, controller: &str) {
        if self.controller.borrow().as_deref() != Some(controller) {
            self.controller.replace(Some(controller.into()));
            self.current();
            self.reset_filter();
        }
    }
    pub fn filter(&self) -> (String, bool) {
        let query = self.search.text().to_string();
        let case = !query.is_empty() && self.case.is_active();
        (query, case)
    }
    pub fn matches(&self, spec: &crate::router::PageSpec) -> bool {
        let (query, case) = self.filter();
        spec.browser == self.browser() && spec.query == query && spec.case_sensitive == case
    }
    pub fn browser(&self) -> bool {
        self.source.selected() == 1
    }
    pub fn current(&self) {
        self.updating.set(true);
        self.source.set_selected(0);
        self.updating.set(false);
    }
    pub fn reset_filter(&self) {
        self.updating.set(true);
        self.search.set_text("");
        self.case.set_active(false);
        self.search_toggle.set_active(false);
        self.search_bar.set_search_mode(false);
        self.updating.set(false);
    }
}
