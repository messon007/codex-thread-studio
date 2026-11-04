//! Native search controls; text scanning belongs to the shared Rust worker.
use crate::{
    i18n_ui::{self, Part, UiText},
    session_search::{Filter, Occurrence, Scope},
};
use adw::prelude::*;
use gtk::{gio, glib};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
pub struct Search {
    pub widget: gtk::Box,
    entry: gtk::SearchEntry,
    case_sensitive: gtk::CheckButton,
    filter: gtk::DropDown,
    summary: gtk::Label,
    results: gtk::ScrolledWindow,
    model: gio::ListStore,
    selection: gtk::SingleSelection,
    previous: gtk::Button,
    next: gtk::Button,
    serial: Cell<u64>,
    current: Cell<Option<usize>>,
    matches: RefCell<Vec<Occurrence>>,
    scope: Cell<Scope>,
    pub on_request: RefCell<Option<Box<dyn Fn(u64, String, bool, Filter)>>>,
    pub on_select: RefCell<Option<Box<dyn Fn(Occurrence)>>>,
    pub on_cancel: RefCell<Option<Box<dyn Fn()>>>,
}
impl Search {
    pub fn new() -> Rc<Self> {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 6);
        widget.set_visible(false);
        let bar = crate::adaptive::toolbar(6);
        bar.set_margin_start(12);
        bar.set_margin_end(12);
        bar.set_margin_top(6);
        let entry = gtk::SearchEntry::builder()
            .placeholder_text("Find in conversation")
            .hexpand(true)
            .build()
            .ui_text("placeholder-text", "Find in conversation");
        i18n_ui::accessible_label(&entry, "Find in conversation");
        entry.set_width_chars(1);
        entry.set_max_width_chars(24);
        bar.append(&entry);
        let case_sensitive = gtk::CheckButton::with_label("Aa");
        i18n_ui::bind(&case_sensitive, "tooltip-text", "Case sensitive");
        bar.append(&case_sensitive);
        let filter = gtk::DropDown::from_strings(&["All", "User", "Assistant", "Activity"]);
        crate::adaptive::ui_string_selector(&filter, 12, None);
        bar.append(&filter);
        let previous = gtk::Button::builder()
            .icon_name("go-up-symbolic")
            .tooltip_text("Previous match")
            .build()
            .ui_text("tooltip-text", "Previous match");
        let next = gtk::Button::builder()
            .icon_name("go-down-symbolic")
            .tooltip_text("Next match")
            .build()
            .ui_text("tooltip-text", "Next match");
        bar.append(&previous);
        bar.append(&next);
        let close = gtk::Button::builder()
            .icon_name("window-close-symbolic")
            .tooltip_text("Close conversation search")
            .build()
            .ui_text("tooltip-text", "Close conversation search");
        bar.append(&close);
        widget.append(&bar);
        let summary = gtk::Label::builder()
            .label("Search the current conversation")
            .xalign(0.0)
            .margin_start(12)
            .margin_end(12)
            .build();
        summary.add_css_class("caption");
        i18n_ui::bind(&summary, "label", "Search the current conversation");
        widget.append(&summary);
        let model = gio::ListStore::new::<glib::BoxedAnyObject>();
        let selection = gtk::SingleSelection::new(Some(model.clone()));
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, item| {
            let label = gtk::Label::builder()
                .xalign(0.0)
                .wrap(true)
                .wrap_mode(gtk::pango::WrapMode::WordChar)
                .width_chars(1)
                .margin_start(12)
                .margin_end(12)
                .margin_top(8)
                .margin_bottom(8)
                .build();
            item.downcast_ref::<gtk::ListItem>()
                .unwrap()
                .set_child(Some(&label));
        });
        factory.connect_bind(|_, item| {
            let item = item.downcast_ref::<gtk::ListItem>().unwrap();
            let data = item.item().and_downcast::<glib::BoxedAnyObject>().unwrap();
            let hit = data.borrow::<Occurrence>();
            let label = item.child().and_downcast::<gtk::Label>().unwrap();
            let kind = match hit.kind {
                Filter::User => "User",
                Filter::Assistant => "Assistant",
                _ => "Activity",
            };
            let start = hit.highlight.start.min(hit.snippet.len());
            let end = hit.highlight.end.min(hit.snippet.len()).max(start);
            label.set_use_markup(true);
            i18n_ui::bind_parts(
                &label,
                "label",
                vec![
                    Part::literal("<b>"),
                    Part::message(kind, &[]),
                    Part::literal(format!(
                        "</b>\n{}<b>{}</b>{}",
                        escaped(&hit.snippet[..start]),
                        escaped(&hit.snippet[start..end]),
                        escaped(&hit.snippet[end..])
                    )),
                ],
                "",
            );
        });
        let list = gtk::ListView::new(Some(selection.clone()), Some(factory));
        list.set_single_click_activate(true);
        let results = gtk::ScrolledWindow::builder()
            .child(&list)
            .max_content_height(220)
            .propagate_natural_height(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .visible(false)
            .build();
        widget.append(&results);
        let search = Rc::new(Self {
            widget,
            entry,
            case_sensitive,
            filter,
            summary,
            results,
            model,
            selection,
            previous,
            next,
            serial: Cell::new(0),
            current: Cell::new(None),
            matches: RefCell::new(Vec::new()),
            scope: Cell::new(Scope::Loaded),
            on_request: RefCell::new(None),
            on_select: RefCell::new(None),
            on_cancel: RefCell::new(None),
        });
        let weak = Rc::downgrade(&search);
        search.entry.connect_search_changed(move |_| {
            if let Some(search) = weak.upgrade() {
                search.perform();
            }
        });
        let weak = Rc::downgrade(&search);
        search.case_sensitive.connect_toggled(move |_| {
            if let Some(search) = weak.upgrade() {
                search.perform();
            }
        });
        let weak = Rc::downgrade(&search);
        search.filter.connect_selected_notify(move |_| {
            if let Some(search) = weak.upgrade() {
                search.perform();
            }
        });
        let weak = Rc::downgrade(&search);
        search.entry.connect_activate(move |_| {
            if let Some(search) = weak.upgrade() {
                search.navigate(true);
            }
        });
        let weak = Rc::downgrade(&search);
        search.entry.connect_stop_search(move |_| {
            if let Some(search) = weak.upgrade() {
                search.close();
            }
        });
        let weak = Rc::downgrade(&search);
        close.connect_clicked(move |_| {
            if let Some(search) = weak.upgrade() {
                search.close();
            }
        });
        let weak = Rc::downgrade(&search);
        search.next.connect_clicked(move |_| {
            if let Some(search) = weak.upgrade() {
                search.navigate(true);
            }
        });
        let weak = Rc::downgrade(&search);
        search.previous.connect_clicked(move |_| {
            if let Some(search) = weak.upgrade() {
                search.navigate(false);
            }
        });
        let weak = Rc::downgrade(&search);
        list.connect_activate(move |_, index| {
            if let Some(search) = weak.upgrade() {
                search.select(index as usize);
            }
        });
        search.controls(false);
        search
    }
    pub fn open(&self) {
        self.widget.set_visible(true);
        self.entry.grab_focus();
        self.entry.select_region(0, -1);
        self.perform();
    }
    pub fn close(&self) {
        if let Some(cancel) = self.on_cancel.borrow().as_ref() {
            cancel();
        }
        self.widget.set_visible(false);
        self.serial.set(self.serial.get().wrapping_add(1));
    }
    pub fn clear(&self) {
        if let Some(cancel) = self.on_cancel.borrow().as_ref() {
            cancel();
        }
        self.serial.set(self.serial.get().wrapping_add(1));
        self.entry.set_text("");
        self.matches.borrow_mut().clear();
        self.model.remove_all();
        self.current.set(None);
        self.results.set_visible(false);
        self.controls(false);
        self.summary.set_tooltip_text(None);
        i18n_ui::bind(&self.summary, "label", "Search the current conversation");
    }
    pub fn serial(&self) -> u64 {
        self.serial.get()
    }
    fn controls(&self, enabled: bool) {
        self.next.set_sensitive(enabled);
        self.previous.set_sensitive(enabled);
    }
    pub fn perform(&self) {
        let serial = self.serial.get().wrapping_add(1);
        self.serial.set(serial);
        self.current.set(None);
        self.controls(false);
        self.model.remove_all();
        self.matches.borrow_mut().clear();
        self.results.set_visible(false);
        let query = self.entry.text().to_string();
        if query.is_empty() {
            if let Some(cancel) = self.on_cancel.borrow().as_ref() {
                cancel();
            }
            self.model.remove_all();
            self.matches.borrow_mut().clear();
            self.results.set_visible(false);
            self.summary.set_tooltip_text(None);
            i18n_ui::bind(&self.summary, "label", "Search the current conversation");
            return;
        }
        self.summary.set_tooltip_text(None);
        i18n_ui::bind(&self.summary, "label", "Searching conversation…");
        let filter = match self.filter.selected() {
            1 => Filter::User,
            2 => Filter::Assistant,
            3 => Filter::Activity,
            _ => Filter::All,
        };
        if let Some(callback) = self.on_request.borrow().as_ref() {
            callback(serial, query, self.case_sensitive.is_active(), filter);
        }
    }
    pub fn apply(&self, serial: u64, result: Result<Vec<Occurrence>, String>) {
        self.apply_scope(serial, result, Scope::Loaded);
    }
    pub fn apply_scope(&self, serial: u64, result: Result<Vec<Occurrence>, String>, scope: Scope) {
        if serial != self.serial.get() || !self.widget.is_visible() {
            return;
        }
        match result {
            Ok(matches) => {
                self.scope.set(scope);
                self.summary.set_tooltip_text(None);
                if scope == Scope::PartialHistory {
                    i18n_ui::bind(
                        &self.summary,
                        "tooltip-text",
                        "Search covers the retrieved history. Older messages may still exist.",
                    );
                }
                self.describe(matches.len(), None);
                self.controls(!matches.is_empty());
                self.results.set_visible(!matches.is_empty());
                self.model.splice(
                    0,
                    self.model.n_items(),
                    &matches
                        .iter()
                        .cloned()
                        .map(glib::BoxedAnyObject::new)
                        .collect::<Vec<_>>(),
                );
                self.matches.replace(matches);
            }
            Err(error) => {
                self.summary.set_tooltip_text(None);
                i18n_ui::literal(&self.summary, "label", &error);
                self.matches.borrow_mut().clear();
                self.model.remove_all();
                self.results.set_visible(false);
                self.controls(false);
            }
        }
    }
    fn navigate(&self, next: bool) {
        let count = self.matches.borrow().len();
        if count == 0 {
            return;
        }
        let index = match (self.current.get(), next) {
            (None, true) => 0,
            (None, false) => count - 1,
            (Some(index), true) => (index + 1) % count,
            (Some(index), false) => (index + count - 1) % count,
        };
        self.select(index);
    }
    fn select(&self, index: usize) {
        let hit = self.matches.borrow().get(index).cloned();
        if let Some(hit) = hit {
            self.current.set(Some(index));
            self.selection.set_selected(index as u32);
            let count = self.matches.borrow().len();
            self.describe(count, Some(index + 1));
            self.results.set_visible(false);
            if let Some(callback) = self.on_select.borrow().as_ref() {
                callback(hit);
            }
        }
    }
    fn describe(&self, count: usize, index: Option<usize>) {
        let key = match (self.scope.get(), index.is_some(), count == 100) {
            (Scope::History, true, _) => "{index} of {count} matches",
            (Scope::Loaded, true, _) => "{index} of {count} loaded matches",
            (Scope::PartialHistory, true, _) => "{index} of {count} retrieved matches",
            (Scope::History, false, true) => "First {count} matching messages",
            (Scope::Loaded, false, true) => "First {count} matching loaded messages",
            (Scope::PartialHistory, false, true) => "First {count} matching retrieved messages",
            (Scope::History, false, false) => "{count} matching messages",
            (Scope::Loaded, false, false) => "{count} matching loaded messages",
            (Scope::PartialHistory, false, false) => "{count} matching retrieved messages",
        };
        i18n_ui::bind_format(
            &self.summary,
            "label",
            key,
            &[
                ("count", &count.to_string()),
                ("index", &index.unwrap_or(0).to_string()),
            ],
        );
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_test(
        &self,
        capture: impl Fn(),
        visible: impl Fn(&Occurrence) -> bool,
    ) -> Result<serde_json::Value, String> {
        async fn until(test: impl Fn() -> bool) -> Result<(), String> {
            let started = std::time::Instant::now();
            while !test() {
                if started.elapsed() > std::time::Duration::from_secs(5) {
                    return Err("Native conversation search timed out".into());
                }
                glib::timeout_future(std::time::Duration::from_millis(25)).await;
            }
            Ok(())
        }
        self.open();
        self.entry.set_text("REQ-0067");
        self.perform();
        until(|| {
            self.matches
                .borrow()
                .iter()
                .any(|hit| hit.table_row == Some(67))
        })
        .await?;
        let hit = self.matches.borrow()[0].clone();
        if hit.turn_id.is_empty() || hit.block.is_none() {
            return Err("Search result lost its source anchor".into());
        }
        self.next.emit_clicked();
        capture();
        until(|| visible(&hit)).await?;
        let stale = self.serial();
        self.entry.set_text("REQ-0067 ");
        self.perform();
        self.apply(stale, Ok(vec![hit.clone()]));
        if !self.matches.borrow().is_empty() {
            return Err("An obsolete query replaced current results".into());
        }
        until(|| self.summary.text() == "0 matching messages").await?;
        self.entry.set_text("req-0067");
        self.perform();
        until(|| !self.matches.borrow().is_empty()).await?;
        self.case_sensitive.set_active(true);
        until(|| self.summary.text() == "0 matching messages").await?;
        self.case_sensitive.set_active(false);
        self.filter.set_selected(1);
        self.entry.set_text("native-smoke-send");
        self.perform();
        until(|| {
            self.matches
                .borrow()
                .iter()
                .any(|hit| hit.kind == Filter::User)
        })
        .await?;
        let serial = self.serial();
        self.close();
        self.apply(serial, Ok(vec![hit]));
        if self.widget.is_visible() {
            return Err("A late result reopened the search panel".into());
        }
        self.clear();
        Ok(
            serde_json::json!({"ok":true,"workerSearch":true,"unicodeAnchors":true,"tableRow":67,"literalTrailingSpace":true,"caseSensitive":true,"roleFilter":true,"staleResultIsolation":true}),
        )
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_old_target(&self) -> Result<(), String> {
        self.open();
        self.filter.set_selected(0);
        self.entry.set_text("native-old-marker");
        self.perform();
        let started = std::time::Instant::now();
        while !self
            .matches
            .borrow()
            .iter()
            .any(|hit| hit.item_id == "native-old-response")
        {
            if started.elapsed() > std::time::Duration::from_secs(5) {
                return Err(format!("Remote search failed: {}", self.summary.text()));
            }
            glib::timeout_future(std::time::Duration::from_millis(25)).await;
        }
        if self.scope.get() != Scope::History {
            return Err("Remote search was reduced to the loaded window".into());
        }
        self.next.emit_clicked();
        Ok(())
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_legacy_search(&self) -> Result<(), String> {
        self.open();
        self.filter.set_selected(0);
        self.entry.set_text("REQ-0067");
        self.perform();
        let started = std::time::Instant::now();
        while self.matches.borrow().is_empty() {
            if started.elapsed() > std::time::Duration::from_secs(5) {
                return Err(format!(
                    "Compatibility search failed: {}",
                    self.summary.text()
                ));
            }
            glib::timeout_future(std::time::Duration::from_millis(25)).await;
        }
        if self.scope.get() == Scope::History || !self.summary.text().contains("loaded") {
            return Err("Unsupported backend search did not report loaded-window scope".into());
        }
        self.close();
        self.clear();
        Ok(())
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_history_page(&self, query: &str, item: &str) -> Result<(), String> {
        self.open();
        self.entry.set_text(query);
        self.perform();
        let start = std::time::Instant::now();
        while !self.matches.borrow().iter().any(|hit| hit.item_id == item) {
            if start.elapsed() > std::time::Duration::from_secs(6) {
                return Err(format!(
                    "Historical window search failed: {}",
                    self.summary.text()
                ));
            }
            glib::timeout_future(std::time::Duration::from_millis(25)).await;
        }
        if self.scope.get() != Scope::Loaded {
            return Err("Unsupported search did not disclose its loaded-page scope".into());
        }
        self.next.emit_clicked();
        Ok(())
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_opencode_history(&self) -> Result<(), String> {
        self.open();
        self.filter.set_selected(0);
        self.case_sensitive.set_active(true);
        self.entry.set_text("native-oc-old-marker 中文😀 ");
        self.perform();
        let start = std::time::Instant::now();
        loop {
            if self
                .matches
                .borrow()
                .iter()
                .any(|hit| hit.item_id == "oc-old-answer-part" && hit.turn_id == "oc-old-user")
            {
                break;
            }
            if start.elapsed() > std::time::Duration::from_secs(5) {
                return Err(format!(
                    "OpenCode history search failed: {}",
                    self.summary.text()
                ));
            }
            glib::timeout_future(std::time::Duration::from_millis(25)).await;
        }
        if self.scope.get() != Scope::History {
            return Err("Cursor-backed OpenCode search was marked incomplete".into());
        }
        self.next.emit_clicked();
        Ok(())
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_opencode_legacy(&self) -> Result<(), String> {
        self.open();
        self.entry.set_text("native-oc-old-marker 中文😀 ");
        self.perform();
        let start = std::time::Instant::now();
        while self.scope.get() != Scope::PartialHistory
            || !self
                .summary
                .text()
                .starts_with("0 matching retrieved messages")
        {
            if start.elapsed() > std::time::Duration::from_secs(5) {
                return Err(format!(
                    "Legacy OpenCode history scope was not disclosed: {}",
                    self.summary.text()
                ));
            }
            glib::timeout_future(std::time::Duration::from_millis(25)).await;
        }
        if self.model.n_items() != 0 {
            return Err("Legacy history kept a stale older hit".into());
        }
        Ok(())
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_opencode_cycle(&self) -> Result<(), String> {
        self.perform();
        let start = std::time::Instant::now();
        while !self.summary.text().contains("repeated") {
            if start.elapsed() > std::time::Duration::from_secs(5) {
                return Err(format!(
                    "Repeated OpenCode cursor was not rejected: {}",
                    self.summary.text()
                ));
            }
            glib::timeout_future(std::time::Duration::from_millis(25)).await;
        }
        if self.model.n_items() != 0 {
            return Err("Failed history read retained search results".into());
        }
        Ok(())
    }
}
fn escaped(text: &str) -> String {
    glib::markup_escape_text(&text.replace('\0', "�")).into()
}
