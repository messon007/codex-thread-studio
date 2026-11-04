//! Native book search, with bounded pages of matching sections and native text.
use crate::epub_search::{Page, Section};
use crate::i18n_ui::{self, Part, UiText};
use adw::prelude::*;
use gtk::glib;
const RESULT_PAGE_ROWS: u32 = 100;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
pub struct Panel {
    pub dialog: adw::Dialog,
    query: gtk::SearchEntry,
    case: gtk::ToggleButton,
    search: gtk::Button,
    stop: gtk::Button,
    reconnect: gtk::Button,
    available: Cell<bool>,
    alive: Cell<bool>,
    previous: gtk::Button,
    next: gtk::Button,
    status: gtk::Label,
    results: gio::ListStore,
    view: gtk::ListView,
    hash: String,
    total: usize,
    serial: Cell<Option<u64>>,
    active_query: RefCell<String>,
    active_case: Cell<bool>,
    cursors: RefCell<Vec<usize>>,
    page: Cell<usize>,
    cursor: Cell<Option<usize>>,
    searched: Cell<usize>,
    errors: Cell<usize>,
    first_error: RefCell<String>,
    request: Box<dyn Fn(usize, &str, bool) -> Option<u64>>,
    selected: Box<dyn Fn(&Section, &str, bool)>,
}
use gtk::gio;
#[cfg(debug_assertions)]
#[path = "epub_search_recovery_acceptance.rs"]
mod recovery_acceptance;
impl Panel {
    pub fn new(
        hash: String,
        total: usize,
        title: &str,
        query: &str,
        case: bool,
        request: impl Fn(usize, &str, bool) -> Option<u64> + 'static,
        selected: impl Fn(&Section, &str, bool) + 'static,
    ) -> Rc<Self> {
        let dialog = adw::Dialog::builder()
            .title("Search book")
            .content_width(720)
            .content_height(620)
            .build()
            .ui_text("title", "Search book");
        let layout = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(
            &adw::WindowTitle::new("Search book", title).ui_text("title", "Search book"),
        ));
        layout.add_top_bar(&header);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        body.set_margin_start(12);
        body.set_margin_end(12);
        body.set_margin_bottom(12);
        let row = crate::adaptive::toolbar(6);
        let entry = gtk::SearchEntry::builder()
            .placeholder_text("Literal text across all sections")
            .text(query)
            .hexpand(true)
            .width_chars(20)
            .max_width_chars(32)
            .build()
            .ui_text("placeholder-text", "Literal text across all sections");
        i18n_ui::accessible_label(&entry, "Literal text across all sections");
        let sensitive = gtk::ToggleButton::with_label("Aa");
        sensitive.set_active(case);
        i18n_ui::bind(&sensitive, "tooltip-text", "Match case");
        let search = gtk::Button::with_label("Search").ui_text("label", "Search");
        search.add_css_class("suggested-action");
        let stop = gtk::Button::with_label("Stop").ui_text("label", "Stop");
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        actions.set_valign(gtk::Align::Center);
        actions.append(&search);
        actions.append(&stop);
        for w in [
            entry.upcast_ref::<gtk::Widget>(),
            sensitive.upcast_ref(),
            actions.upcast_ref(),
        ] {
            row.append(w);
        }
        body.append(&row);
        let status = gtk::Label::builder()
            .label(
                "Results use extracted book text. Open a section to navigate its rendered matches.",
            )
            .xalign(0.0)
            .wrap(true)
            .build()
            .ui_text(
                "label",
                "Results use extracted book text. Open a section to navigate its rendered matches.",
            );
        status.add_css_class("dim-label");
        body.append(&status);
        let results = gio::ListStore::new::<glib::BoxedAnyObject>();
        let selection = gtk::SingleSelection::new(Some(results.clone()));
        selection.set_autoselect(false);
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, object| {
            let item = object.downcast_ref::<gtk::ListItem>().unwrap();
            let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
            row.set_margin_top(10);
            row.set_margin_bottom(10);
            row.set_margin_start(10);
            row.set_margin_end(10);
            let title = gtk::Label::builder()
                .xalign(0.0)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .build();
            title.add_css_class("heading");
            row.append(&title);
            row.append(
                &gtk::Label::builder()
                    .xalign(0.0)
                    .wrap(true)
                    .lines(3)
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .build(),
            );
            item.set_child(Some(&row));
        });
        factory.connect_bind(|_, object| {
            let item = object.downcast_ref::<gtk::ListItem>().unwrap();
            let object = item.item().and_downcast::<glib::BoxedAnyObject>().unwrap();
            let section = object.borrow::<Section>();
            let row = item.child().unwrap();
            let title = row.first_child().and_downcast::<gtk::Label>().unwrap();
            let key = if section.count == 1 {
                "{count}{plus} match"
            } else {
                "{count}{plus} matches"
            };
            i18n_ui::bind_parts(
                &title,
                "label",
                vec![
                    Part::literal(section.label.clone()),
                    Part::message(
                        key,
                        &[
                            ("count", &section.count.to_string()),
                            ("plus", if section.limited { "+" } else { "" }),
                        ],
                    ),
                ],
                " · ",
            );
            let snippet = title.next_sibling().and_downcast::<gtk::Label>().unwrap();
            snippet.set_text(&section.snippet);
        });
        let view = gtk::ListView::new(Some(selection), Some(factory));
        view.add_css_class("boxed-list");
        view.set_single_click_activate(true);
        let scroll = gtk::ScrolledWindow::builder()
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&view)
            .build();
        body.append(&scroll);
        let footer = crate::adaptive::toolbar(6);
        footer.set_halign(gtk::Align::End);
        let previous =
            gtk::Button::with_label("Previous results").ui_text("label", "Previous results");
        let next = gtk::Button::with_label("Continue search").ui_text("label", "Continue search");
        let reconnect = gtk::Button::builder()
            .label("Reconnect")
            .action_name("win.reconnect")
            .visible(false)
            .build()
            .ui_text("label", "Reconnect");
        footer.append(&reconnect);
        footer.append(&previous);
        footer.append(&next);
        body.append(&footer);
        layout.set_content(Some(&body));
        dialog.set_child(Some(&layout));
        let panel = Rc::new(Self {
            dialog,
            query: entry,
            case: sensitive,
            search,
            stop,
            reconnect,
            available: Cell::new(true),
            alive: Cell::new(true),
            previous,
            next,
            status,
            results,
            view,
            hash,
            total,
            serial: Cell::new(None),
            active_query: RefCell::new(String::new()),
            active_case: Cell::new(case),
            cursors: RefCell::new(vec![0]),
            page: Cell::new(0),
            cursor: Cell::new(None),
            searched: Cell::new(0),
            errors: Cell::new(0),
            first_error: RefCell::new(String::new()),
            request: Box::new(request),
            selected: Box::new(selected),
        });
        let weak = Rc::downgrade(&panel);
        panel.search.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.begin();
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.query.connect_activate(move |_| {
            if let Some(p) = weak.upgrade() {
                p.begin();
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.stop.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade().filter(|p| p.online()) {
                p.serial.set(None);
                p.describe("Stopped");
                p.controls();
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.previous.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                if p.online() && p.serial.get().is_none() && p.page.get() > 0 {
                    p.page.set(p.page.get() - 1);
                    let start = p.cursors.borrow()[p.page.get()];
                    p.load(start);
                }
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.next.connect_clicked(move |_| {
            if let Some(p) = weak
                .upgrade()
                .filter(|p| p.online() && p.serial.get().is_none())
            {
                if let Some(start) = p.cursor.get() {
                    if p.results.n_items() < RESULT_PAGE_ROWS {
                        // Stop, queue rejection or gateway loss can interrupt a
                        // result page between batches. Finish that page before
                        // recording another page boundary.
                        p.send(start);
                    } else {
                        let page = p.page.get() + 1;
                        p.cursors.borrow_mut().truncate(page);
                        p.cursors.borrow_mut().push(start);
                        p.page.set(page);
                        p.load(start);
                    }
                }
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.view.connect_activate(move |_, index| {
            if let Some(p) = weak.upgrade().filter(|p| p.online()) {
                let section = p
                    .results
                    .item(index)
                    .and_downcast::<glib::BoxedAnyObject>()
                    .map(|s| s.borrow::<Section>().clone());
                if let Some(section) = section {
                    (p.selected)(&section, &p.active_query.borrow(), p.active_case.get());
                    p.retire();
                    p.dialog.close();
                }
            }
        });
        let weak = Rc::downgrade(&panel);
        panel.dialog.connect_closed(move |_| {
            if let Some(p) = weak.upgrade() {
                p.retire();
            }
        });
        panel.controls();
        panel
    }
    fn retire(&self) {
        self.alive.set(false);
        self.available.set(false);
        self.serial.set(None);
        self.reconnect.set_action_name(None);
        self.controls();
    }
    pub fn is_open(&self) -> bool {
        self.alive.get()
    }
    fn online(&self) -> bool {
        self.alive.get() && self.available.get()
    }
    pub fn gateway_failed(&self, error: &str) {
        if !self.alive.get() {
            return;
        }
        self.available.set(false);
        self.serial.set(None);
        self.reconnect.set_visible(true);
        self.describe_parts(Part::literal(error));
        self.controls();
    }
    pub fn revalidating(&self) {
        if !self.alive.get() {
            return;
        }
        self.available.set(false);
        self.serial.set(None);
        self.reconnect.set_visible(false);
        self.describe("Revalidating book…");
        self.controls();
    }
    pub fn revalidation_failed(&self, error: &str) {
        if !self.alive.get() {
            return;
        }
        self.available.set(false);
        self.serial.set(None);
        self.reconnect.set_visible(false);
        self.describe_parts(Part::literal(error));
        self.controls();
    }
    pub fn gateway_restored(&self, hash: &str, total: usize) {
        if !self.alive.get() {
            return;
        }
        if self.hash != hash || self.total != total {
            self.revalidation_failed("Book changed; reopen it to search");
            return;
        }
        self.available.set(true);
        self.reconnect.set_visible(false);
        self.describe("Book reconnected");
        self.controls();
    }
    fn begin(&self) {
        if !self.online() {
            return;
        }

        let query = self.query.text().to_string();
        if query.is_empty() || query.len() > 512 || query.chars().all(char::is_whitespace) {
            i18n_ui::bind(&self.status, "label", "Enter 1–512 bytes of literal text");
            return;
        }
        self.active_query.replace(query);
        self.active_case.set(self.case.is_active());
        self.cursors.replace(vec![0]);
        self.page.set(0);
        self.load(0);
    }
    fn load(&self, start: usize) {
        if !self.online() {
            return;
        }
        self.results.remove_all();
        self.errors.set(0);
        self.first_error.borrow_mut().clear();
        self.searched.set(start);
        self.send(start);
    }
    fn send(&self, start: usize) {
        if !self.online() {
            return;
        }
        self.cursor.set(Some(start));
        self.serial.set((self.request)(
            start,
            &self.active_query.borrow(),
            self.active_case.get(),
        ));
        if self.serial.get().is_some() {
            self.describe("Searching…");
        } else {
            self.describe("Search queue is busy; try Continue search");
        }
        self.controls();
    }
    fn describe(&self, prefix: &'static str) {
        self.describe_parts(Part::message(prefix, &[]));
    }
    fn describe_parts(&self, prefix: Part) {
        let mut parts = vec![
            prefix,
            Part::message(
                "{count} matching sections · searched {searched} / {total} sections",
                &[
                    ("count", &self.results.n_items().to_string()),
                    ("searched", &self.searched.get().to_string()),
                    ("total", &self.total.to_string()),
                ],
            ),
        ];
        if self.errors.get() > 0 {
            parts.push(Part::message(
                if self.errors.get() == 1 {
                    "{count} section could not be searched"
                } else {
                    "{count} sections could not be searched"
                },
                &[("count", &self.errors.get().to_string())],
            ));
        }
        i18n_ui::bind_parts(&self.status, "label", parts, " · ");
        self.status.set_tooltip_text(
            (self.errors.get() > 0)
                .then(|| self.first_error.borrow().clone())
                .as_deref(),
        );
    }
    fn controls(&self) {
        let online = self.online();
        let busy = self.serial.get().is_some();
        self.search.set_sensitive(online);
        self.view.set_sensitive(online);
        self.stop.set_sensitive(online && busy);
        self.next
            .set_sensitive(online && !busy && self.cursor.get().is_some());
        self.previous
            .set_sensitive(online && !busy && self.page.get() > 0);
    }
    pub fn event(&self, serial: u64, result: Result<Page, String>) {
        if !self.online() || self.serial.get() != Some(serial) {
            return;
        }
        self.serial.set(None);
        match result {
            Err(e) => {
                i18n_ui::literal(&self.status, "label", &e);
                self.controls();
            }
            Ok(page) => {
                if page.book_hash != self.hash
                    || page.query != *self.active_query.borrow()
                    || page.case_sensitive != self.active_case.get()
                    || Some(page.start) != self.cursor.get()
                    || page.total != self.total
                {
                    i18n_ui::bind(
                        &self.status,
                        "label",
                        "Search response does not match this book/query",
                    );
                    self.controls();
                    return;
                }
                self.searched.set(page.start + page.scanned);
                self.cursor.set(page.next);
                for issue in &page.issues {
                    if self.errors.get() == 0 {
                        self.first_error
                            .replace(format!("{}: {}", issue.label, issue.error));
                    }
                    self.errors.set(self.errors.get() + 1);
                }
                for section in page.sections {
                    self.results.append(&glib::BoxedAnyObject::new(section));
                }
                if let Some(next) = page
                    .next
                    .filter(|_| self.results.n_items() < RESULT_PAGE_ROWS)
                {
                    self.send(next);
                } else {
                    self.describe(if page.next.is_some() {
                        "More sections available"
                    } else {
                        "Search complete"
                    });
                    self.controls();
                }
            }
        }
    }
}
#[cfg(debug_assertions)]
impl Panel {
    async fn smoke_wait(&self) -> Result<(), String> {
        let start = std::time::Instant::now();
        while self.serial.get().is_some() || !self.query.is_mapped() || self.query.height() == 0 {
            if start.elapsed() > std::time::Duration::from_secs(30) {
                return Err(format!(
                    "Book search did not settle: {}",
                    self.status.text()
                ));
            }
            glib::timeout_future(std::time::Duration::from_millis(20)).await;
        }
        Ok(())
    }
    pub async fn smoke_test(
        &self,
        capture: impl Fn(&str) -> Result<(), String>,
    ) -> Result<(usize, u128), String> {
        self.smoke_wait().await?;
        self.query.set_text("needle");
        self.case.set_active(false);
        let started = std::time::Instant::now();
        self.search.emit_clicked();
        self.smoke_wait().await?;
        let first_page_ms = started.elapsed().as_millis();
        let count = self.results.n_items();
        if !(100..=115).contains(&count)
            || self.cursor.get().is_none()
            || self.errors.get() != 1
            || !self.status.text().contains("could not be searched")
        {
            return Err(format!(
                "Book search did not collect bounded results across unopened sections: {}",
                self.status.text()
            ));
        }
        capture("epub-book-search.png")?;
        self.dialog.set_content_width(340);
        glib::timeout_future(std::time::Duration::from_millis(350)).await;
        let row = self.query.parent().ok_or("Search toolbar has no parent")?;
        for control in [
            self.query.upcast_ref::<gtk::Widget>(),
            self.case.upcast_ref(),
            self.search.upcast_ref(),
            self.stop.upcast_ref(),
        ] {
            let bounds = control
                .compute_bounds(&row)
                .ok_or("Search toolbar control is not allocated")?;
            if bounds.width() <= 0.0
                || bounds.height() <= 0.0
                || bounds.x() < -1.0
                || bounds.x() + bounds.width() > row.width() as f32 + 1.0
            {
                return Err(format!(
                    "Narrow book search toolbar clips a control: {:?} in {}",
                    bounds,
                    row.width()
                ));
            }
        }
        if row.width() > 360 {
            return Err("Book search did not adapt to a narrow dialog".into());
        }
        capture("epub-book-search-narrow.png")?;
        self.dialog.set_content_width(720);
        glib::timeout_future(std::time::Duration::from_millis(200)).await;
        self.next.emit_clicked();
        self.smoke_wait().await?;
        if self.results.n_items() != 122 - count
            || self.cursor.get().is_some()
            || !self.status.text().contains("Search complete")
        {
            return Err("Book search continuation lost sections".into());
        }
        self.previous.emit_clicked();
        self.smoke_wait().await?;
        if self.results.n_items() != count {
            return Err("Previous book search page changed its sections".into());
        }
        self.case.set_active(true);
        self.search.emit_clicked();
        self.smoke_wait().await?;
        if self.results.n_items() != 0 || self.cursor.get().is_some() {
            return Err("Case-sensitive book search returned lowercase text that is absent".into());
        }
        self.query.set_text("needle ");
        self.case.set_active(false);
        self.search.emit_clicked();
        self.stop.emit_clicked();
        let before = self.results.n_items();
        glib::timeout_future(std::time::Duration::from_millis(300)).await;
        if self.serial.get().is_some()
            || self.results.n_items() != before
            || !self.status.text().starts_with("Stopped")
        {
            return Err("Stopped book search installed a late response".into());
        }
        self.query.set_text("needle");
        self.search.emit_clicked();
        self.smoke_wait().await?;
        let first = self
            .results
            .item(0)
            .and_downcast::<glib::BoxedAnyObject>()
            .ok_or("No book search row")?;
        let chapter = first.borrow::<Section>().chapter;
        self.view.emit_by_name::<()>("activate", &[&0u32]);
        Ok((chapter, first_page_ms))
    }
    pub fn smoke_pending(&self) -> Option<u64> {
        self.query.set_text("needle");
        self.search.emit_clicked();
        self.serial.get()
    }
    pub async fn smoke_empty(&self) -> Result<(), String> {
        self.query.set_text("needle");
        self.search.emit_clicked();
        self.smoke_wait().await?;
        if self.results.n_items() != 0 || self.total != 3 {
            return Err("Old book search results leaked into a replacement book".into());
        }
        Ok(())
    }
}
