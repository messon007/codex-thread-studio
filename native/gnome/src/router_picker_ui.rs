//! Bounded GTK target selection; discovery and Unicode search run on Rust workers.
use crate::{
    i18n_ui::{self, UiText},
    router::{ChoicePage, Choices, Reply, Request, Target},
};
use adw::prelude::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

pub struct Picker {
    pub dialog: adw::Dialog,
    pub search: gtk::SearchEntry,
    pub list: gtk::ListBox,
    pub automatic: gtk::Button,
    pub previous: gtk::Button,
    pub next: gtk::Button,
    reload: gtk::Button,
    status: gtk::Label,
    warnings: gtk::Label,
    catalog: RefCell<Option<Arc<Choices>>>,
    page: RefCell<Option<ChoicePage>>,
    busy: Cell<bool>,
    offset: Cell<usize>,
    selected: Option<Target>,
    controller: String,
    request: Box<dyn Fn(Request)>,
    pick: Box<dyn Fn(Option<Target>, Option<String>) -> bool>,
}
impl Picker {
    #[cfg(debug_assertions)]
    pub fn smoke_status(&self) -> String {
        self.status.text().into()
    }
    #[cfg(debug_assertions)]
    pub fn smoke_ready(&self, query: &str, total: usize, offset: usize) -> bool {
        !self.busy.get()
            && self.list.is_sensitive()
            && self.page.borrow().as_ref().is_some_and(|page| {
                page.query == query && page.total == total && page.offset == offset
            })
    }
    pub fn new(
        controller: String,
        selected: Option<Target>,
        request: impl Fn(Request) + 'static,
        pick: impl Fn(Option<Target>, Option<String>) -> bool + 'static,
    ) -> Rc<Self> {
        let dialog = adw::Dialog::builder()
            .content_width(650)
            .content_height(620)
            .build()
            .ui_text("title", "Router target");
        let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
        body.set_margin_start(16);
        body.set_margin_end(16);
        body.set_margin_top(12);
        body.set_margin_bottom(16);
        let automatic = gtk::Button::new()
            .ui_text("label", "Automatic routing")
            .ui_text(
                "tooltip-text",
                "Let Thread Router choose a suitable session",
            );
        if selected.is_none() {
            automatic.add_css_class("suggested-action");
        }
        body.append(&automatic);
        let search = gtk::SearchEntry::builder().build().ui_text(
            "placeholder-text",
            "Search sessions, backends or project paths",
        );
        i18n_ui::accessible_label(&search, "Search sessions, backends or project paths");
        if let Some(text) = search.delegate().and_downcast::<gtk::Text>() {
            text.set_max_length(512);
        }
        body.append(&search);
        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::None);
        list.add_css_class("boxed-list");
        let scroll = gtk::ScrolledWindow::builder()
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&list)
            .build();
        body.append(&scroll);
        let warnings = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .visible(false)
            .build();
        warnings.add_css_class("caption");
        warnings.add_css_class("warning");
        body.append(&warnings);
        let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let status = gtk::Label::builder()
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        status.add_css_class("caption");
        controls.append(&status);
        let previous = gtk::Button::builder()
            .icon_name("go-previous-symbolic")
            .sensitive(false)
            .build()
            .ui_text("tooltip-text", "Previous target results");
        let next = gtk::Button::builder()
            .icon_name("go-next-symbolic")
            .sensitive(false)
            .build()
            .ui_text("tooltip-text", "Next target results");
        controls.append(&previous);
        controls.append(&next);
        let reload = gtk::Button::builder()
            .icon_name("view-refresh-symbolic")
            .build()
            .ui_text("tooltip-text", "Refresh Router targets");
        controls.append(&reload);
        body.append(&controls);
        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&adw::HeaderBar::new());
        toolbar.set_content(Some(&body));
        dialog.set_child(Some(&toolbar));
        let panel = Rc::new(Self {
            dialog,
            search,
            list,
            automatic,
            previous,
            next,
            reload,
            status,
            warnings,
            catalog: RefCell::new(None),
            page: RefCell::new(None),
            busy: Cell::new(false),
            offset: Cell::new(0),
            selected,
            controller,
            request: Box::new(request),
            pick: Box::new(pick),
        });
        let weak = Rc::downgrade(&panel);
        panel.automatic.connect_clicked(move |_| {
            if let Some(panel) = weak.upgrade() {
                panel.choose(None);
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.reload.connect_clicked(move |_| {
            if let Some(panel) = weak.upgrade() {
                panel.load();
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.search.connect_search_changed(move |_| {
            if let Some(panel) = weak.upgrade() {
                panel.offset.set(0);
                panel.filter();
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.list.connect_row_activated(move |_, row| {
            if let Some(panel) = weak.upgrade() {
                let choice = panel.page.borrow().as_ref().and_then(|page| {
                    page.indices
                        .get(row.index() as usize)
                        .map(|index| page.catalog.targets[*index].clone())
                });
                if panel.list.is_sensitive() {
                    if let Some(choice) = choice {
                        panel.choose(Some(choice));
                    }
                }
            }
        });
        for (button, next_page) in [(&panel.previous, false), (&panel.next, true)] {
            let weak = Rc::downgrade(&panel);
            button.connect_clicked(move |_| {
                if let Some(panel) = weak.upgrade() {
                    if !panel.busy.get() {
                        panel.offset.set(if next_page {
                            panel.offset.get() + 50
                        } else {
                            panel.offset.get().saturating_sub(50)
                        });
                        panel.filter();
                    }
                }
            });
        }
        panel
    }
    pub fn load(&self) {
        if self.busy.replace(true) {
            return;
        }
        self.catalog.replace(None);
        self.page.replace(None);
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        self.controls(false, false, false);
        i18n_ui::bind(&self.status, "label", "Loading available sessions…");
        self.warnings.set_visible(false);
        (self.request)(Request::Choices(self.controller.clone()));
    }
    fn controls(&self, ready: bool, previous: bool, next: bool) {
        self.list.set_sensitive(ready);
        self.reload.set_sensitive(!self.busy.get());
        self.previous.set_sensitive(ready && previous);
        self.next.set_sensitive(ready && next);
    }
    fn filter(&self) {
        self.list.set_sensitive(false);
        let catalog = self.catalog.borrow().clone();
        let Some(catalog) = catalog else {
            return;
        };
        if self.busy.replace(true) {
            return;
        }
        self.controls(false, false, false);
        (self.request)(Request::FilterChoices {
            catalog,
            query: self.search.text().to_string(),
            offset: self.offset.get(),
        });
    }
    pub fn event(self: &Rc<Self>, result: Result<Reply, String>) {
        self.busy.set(false);
        match result {
            Ok(Reply::Choices(catalog)) => {
                i18n_ui::literal(&self.warnings, "label", &catalog.warnings.join("\n"));
                self.warnings.set_visible(!catalog.warnings.is_empty());
                self.catalog.replace(Some(catalog));
                self.offset.set(0);
                self.filter();
            }
            Ok(Reply::ChoicePage(page)) => {
                if page.query != self.search.text()
                    || self
                        .catalog
                        .borrow()
                        .as_ref()
                        .is_none_or(|catalog| !Arc::ptr_eq(catalog, &page.catalog))
                {
                    self.offset.set(0);
                    self.filter();
                    return;
                }
                while let Some(child) = self.list.first_child() {
                    self.list.remove(&child);
                }
                for index in &page.indices {
                    let target = &page.catalog.targets[*index];
                    let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
                    content.set_margin_top(10);
                    content.set_margin_bottom(10);
                    content.set_margin_start(12);
                    content.set_margin_end(12);
                    let text = gtk::Box::new(gtk::Orientation::Vertical, 4);
                    text.set_hexpand(true);
                    let title = gtk::Label::builder()
                        .label(&target.title)
                        .xalign(0.0)
                        .ellipsize(gtk::pango::EllipsizeMode::End)
                        .build();
                    let subtitle = gtk::Label::builder()
                        .label(format!("{} · {}", target.session_key, target.cwd))
                        .xalign(0.0)
                        .ellipsize(gtk::pango::EllipsizeMode::Middle)
                        .build();
                    subtitle.add_css_class("caption");
                    subtitle.add_css_class("dim-label");
                    text.append(&title);
                    text.append(&subtitle);
                    content.append(&text);
                    if self.selected.as_ref().is_some_and(|picked| {
                        picked.session_key == target.session_key && picked.cwd == target.cwd
                    }) {
                        let selected = gtk::Image::from_icon_name("object-select-symbolic");
                        i18n_ui::accessible_label(&selected, "Selected target");
                        content.append(&selected);
                    }
                    let row = gtk::ListBoxRow::builder()
                        .activatable(true)
                        .selectable(false)
                        .child(&content)
                        .tooltip_text(format!(
                            "{}\n{}\n{}",
                            target.title, target.session_key, target.cwd
                        ))
                        .build();
                    self.list.append(&row);
                }
                self.offset.set(page.offset);
                if page.total == 0 {
                    i18n_ui::bind(&self.status, "label", "No matching sessions");
                } else {
                    i18n_ui::bind_format(
                        &self.status,
                        "label",
                        "{start}–{end} of {total} sessions",
                        &[
                            ("start", &(page.offset + 1).to_string()),
                            ("end", &(page.offset + page.indices.len()).to_string()),
                            ("total", &page.total.to_string()),
                        ],
                    );
                }
                self.controls(
                    true,
                    page.offset > 0,
                    page.offset + page.indices.len() < page.total,
                );
                self.page.replace(Some(page));
            }
            Err(error) => {
                self.controls(false, false, false);
                i18n_ui::bind_format(
                    &self.status,
                    "label",
                    "Unable to list Router targets: {error}",
                    &[("error", &error)],
                );
            }
            _ => {
                self.controls(false, false, false);
                i18n_ui::bind(
                    &self.status,
                    "label",
                    "Unexpected target discovery response",
                );
            }
        }
    }
    fn choose(&self, target: Option<Target>) {
        let revision = self
            .catalog
            .borrow()
            .as_ref()
            .map(|catalog| catalog.revision.clone());
        if (self.pick)(target, revision) {
            self.dialog.close();
        } else {
            i18n_ui::bind(
                &self.status,
                "label",
                "Router selection changed. Refresh the list before choosing again.",
            );
        }
    }
}
