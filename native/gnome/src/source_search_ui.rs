//! Full-snapshot worker search with bounded native page highlighting.
#[cfg(debug_assertions)]
#[path = "source_search_acceptance.rs"]
mod acceptance;
#[cfg(debug_assertions)]
#[path = "edit_find_acceptance.rs"]
mod edit_acceptance;
use super::*;
use crate::source_search::{EditTarget, Hit, Matches};
use gtk::glib;
use std::{
    rc::Weak,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
#[derive(Clone)]
enum Anchor {
    Logical(usize),
    Original(std::ops::Range<usize>),
}

pub(super) struct Find {
    bar: gtk::Box,
    toggle: gtk::ToggleButton,
    entry: gtk::SearchEntry,
    case: gtk::ToggleButton,
    previous: gtk::Button,
    next: gtk::Button,
    count: gtk::Label,
    close: gtk::Button,
    settings: sourceview::SearchSettings,
    source_context: sourceview::SearchContext,
    edit_context: sourceview::SearchContext,
    preview_context: sourceview::SearchContext,
    owner: RefCell<Weak<Source>>,
    timer: RefCell<Option<glib::SourceId>>,
    cancelled: RefCell<Option<Arc<AtomicBool>>>,
    serial: Cell<u64>,
    matches: RefCell<Matches>,
    index: Cell<Option<usize>>,
    targets: RefCell<Option<Vec<EditTarget>>>,
    anchor: RefCell<Option<Anchor>>,
}
impl Find {
    pub(super) fn new(
        toolbar: &adw::WrapBox,
        view: &sourceview::View,
        editor: &sourceview::View,
        preview: &sourceview::View,
    ) -> Self {
        let toggle = gtk::ToggleButton::builder()
            .icon_name("edit-find-symbolic")
            .sensitive(false)
            .build();
        crate::i18n_ui::bind(&toggle, "tooltip-text", "Find in source");
        crate::i18n_ui::accessible_label(&toggle, "Find in source");
        toolbar.append(&toggle);
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        bar.set_visible(false);
        bar.set_margin_start(12);
        bar.set_margin_end(12);
        bar.set_margin_top(6);
        bar.set_margin_bottom(6);
        let entry = gtk::SearchEntry::builder()
            .hexpand(true)
            .width_chars(10)
            .build();
        // Width stays adaptive; the query byte budget is enforced before dispatch.
        entry.set_max_width_chars(28);
        if let Some(text) = entry.delegate().and_downcast::<gtk::Text>() {
            text.set_max_length(crate::source_search::MAX_QUERY as i32);
        }
        crate::i18n_ui::bind(&entry, "placeholder-text", "Find in source");
        crate::i18n_ui::accessible_label(&entry, "Find in source");
        let case = gtk::ToggleButton::with_label("Aa");
        crate::i18n_ui::bind(&case, "tooltip-text", "Match case");
        crate::i18n_ui::accessible_label(&case, "Match case");
        let previous = button("go-up-symbolic", "Previous match");
        let next = button("go-down-symbolic", "Next match");
        let close = button("window-close-symbolic", "Close search");
        for (widget, label) in [
            (&previous, "Previous match"),
            (&next, "Next match"),
            (&close, "Close search"),
        ] {
            crate::i18n_ui::bind(widget, "tooltip-text", label);
            crate::i18n_ui::accessible_label(widget, label);
        }
        previous.set_sensitive(false);
        next.set_sensitive(false);
        let count = gtk::Label::new(None);
        count.set_ellipsize(gtk::pango::EllipsizeMode::End);
        count.set_max_width_chars(18);
        count.add_css_class("caption");
        for widget in [
            entry.upcast_ref::<gtk::Widget>(),
            case.upcast_ref(),
            previous.upcast_ref(),
            next.upcast_ref(),
            count.upcast_ref(),
            close.upcast_ref(),
        ] {
            bar.append(widget);
        }
        let settings = sourceview::SearchSettings::new();
        settings.set_regex_enabled(false);
        let buffer = view
            .buffer()
            .downcast::<sourceview::Buffer>()
            .expect("Source uses GtkSourceBuffer");
        let context = sourceview::SearchContext::new(&buffer, Some(&settings));
        let buffer = editor
            .buffer()
            .downcast::<sourceview::Buffer>()
            .expect("Editor uses GtkSourceBuffer");
        let edit_context = sourceview::SearchContext::new(&buffer, Some(&settings));
        edit_context.set_highlight(false);
        let preview_context = sourceview::SearchContext::new(
            &preview
                .buffer()
                .downcast::<sourceview::Buffer>()
                .expect("Preview uses GtkSourceBuffer"),
            Some(&settings),
        );
        preview_context.set_highlight(false);
        Self {
            bar,
            toggle,
            entry,
            case,
            previous,
            next,
            count,
            close,
            settings,
            source_context: context,
            edit_context,
            preview_context,
            owner: RefCell::new(Weak::new()),
            timer: RefCell::new(None),
            cancelled: RefCell::new(None),
            serial: Cell::new(0),
            matches: RefCell::new(Matches::default()),
            index: Cell::new(None),
            targets: RefCell::new(None),
            anchor: RefCell::new(None),
        }
    }
    pub(super) fn bar(&self) -> &gtk::Box {
        &self.bar
    }
    pub(super) fn wire(source: &Rc<Source>) {
        source.search.owner.replace(Rc::downgrade(source));
        let weak = Rc::downgrade(source);
        source.search.toggle.connect_toggled(move |toggle| {
            if let Some(source) = weak.upgrade() {
                source.search.bar.set_visible(toggle.is_active());
                if toggle.is_active() {
                    source.search.entry.grab_focus();
                    source.schedule_search();
                } else {
                    source.cancel_search();
                }
            }
        });
        let weak = Rc::downgrade(source);
        source.search.entry.connect_changed(move |_| {
            if let Some(source) = weak.upgrade() {
                source.schedule_search();
            }
        });
        let weak = Rc::downgrade(source);
        source.search.entry.connect_activate(move |_| {
            if let Some(source) = weak.upgrade() {
                source.search_step(true);
            }
        });
        let entry_keys = gtk::EventControllerKey::new();
        entry_keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(source);
        entry_keys.connect_key_pressed(move |_, key, _, modifiers| {
            if key == gtk::gdk::Key::Return
                && modifiers.contains(gtk::gdk::ModifierType::SHIFT_MASK)
            {
                if let Some(source) = weak.upgrade() {
                    source.search_step(false);
                }
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        source.search.entry.add_controller(entry_keys);
        let weak = Rc::downgrade(source);
        source.search.case.connect_toggled(move |_| {
            if let Some(source) = weak.upgrade() {
                source.schedule_search();
            }
        });
        for (widget, forward) in [
            (&source.search.previous, false),
            (&source.search.next, true),
        ] {
            let weak = Rc::downgrade(source);
            widget.connect_clicked(move |_| {
                if let Some(source) = weak.upgrade() {
                    source.search_step(forward);
                }
            });
        }
        let weak = Rc::downgrade(source);
        source.search.close.connect_clicked(move |_| {
            if let Some(source) = weak.upgrade() {
                source.close_search();
            }
        });
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(source);
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            if let Some(source) = weak.upgrade() {
                let ctrl = modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK);
                let shift = modifiers.contains(gtk::gdk::ModifierType::SHIFT_MASK);
                if ctrl
                    && matches!(key, gtk::gdk::Key::f | gtk::gdk::Key::F)
                    && source.search.toggle.is_sensitive()
                {
                    source.search.toggle.set_active(true);
                    source.search.entry.grab_focus();
                    source.search.entry.select_region(0, -1);
                } else if source.search.bar.is_visible() && key == gtk::gdk::Key::Escape {
                    source.close_search();
                } else if source.search.bar.is_visible()
                    && (key == gtk::gdk::Key::F3
                        || ctrl && matches!(key, gtk::gdk::Key::g | gtk::gdk::Key::G))
                {
                    source.search_step(!shift);
                } else {
                    return glib::Propagation::Proceed;
                }
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        source.widget.add_controller(keys);
    }
}
impl Source {
    pub(super) fn wire_html_find(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.editor.html.found.replace(Some(Box::new(move |count| {
            if let Some(source) = weak
                .upgrade()
                .filter(|s| s.html_active() && s.search.bar.is_visible())
            {
                crate::i18n_ui::bind_format(
                    &source.search.count,
                    "label",
                    "{count} matches",
                    &[(
                        "count",
                        &if count == u32::MAX {
                            format!("{}+", crate::html_ui::MATCH_LIMIT)
                        } else {
                            count.to_string()
                        },
                    )],
                );
                source.search.previous.set_sensitive(count > 0);
                source.search.next.set_sensitive(count > 0);
            }
        })));
        let weak = Rc::downgrade(self);
        self.editor.html.loaded.replace(Some(Box::new(move || {
            if let Some(source) = weak.upgrade().filter(|s| s.html_active()) {
                source.schedule_search();
            }
        })));
    }
    #[cfg(debug_assertions)]
    pub(super) fn smoke_find_query(&self, query: &str) {
        self.search.toggle.set_active(true);
        self.search.entry.set_text(query);
        self.search.entry.emit_activate();
    }
    #[cfg(debug_assertions)]
    pub(super) fn smoke_find_has_match(&self) -> bool {
        self.search.index.get().is_some()
    }
    #[cfg(debug_assertions)]
    pub(super) fn smoke_find_close(&self) {
        self.search.close.emit_clicked();
    }
    #[cfg(debug_assertions)]
    pub(super) fn smoke_find_count(&self) -> String {
        self.search.count.text().to_string()
    }
    fn configure_find_mode(&self, editing: bool) {
        let preview = self.preview_active();
        self.search
            .source_context
            .set_highlight(!editing && !preview);
        self.search.preview_context.set_highlight(preview);
        self.search.edit_context.set_highlight(editing);
        let label = if editing {
            "Find in editor"
        } else if preview {
            "Find in preview"
        } else {
            "Find in source"
        };
        crate::i18n_ui::bind(&self.search.toggle, "tooltip-text", label);
        crate::i18n_ui::accessible_label(&self.search.toggle, label);
        crate::i18n_ui::bind(&self.search.entry, "placeholder-text", label);
        crate::i18n_ui::accessible_label(&self.search.entry, label);
    }
    pub(super) fn edit_search_mode(&self, editing: bool) {
        self.search.anchor.borrow_mut().take();
        self.cancel_search();
        self.configure_find_mode(editing);
        self.search
            .toggle
            .set_sensitive(self.text.borrow().as_ref().is_some_and(|text| !text.binary));
        self.search.bar.set_visible(self.search.toggle.is_active());
        self.schedule_search();
    }
    pub(super) fn reset_search(&self) {
        self.search.anchor.borrow_mut().take();
        self.cancel_search();
        self.search.toggle.set_active(false);
        self.search.entry.set_text("");
        self.configure_find_mode(false);
        self.search.toggle.set_sensitive(false);
    }
    pub(super) fn cancel_search(&self) {
        self.editor.html.stop_find();
        self.stop_search_work();
        self.search.matches.replace(Matches::default());
        self.search.index.set(None);
        self.search.targets.borrow_mut().take();
        self.search.settings.set_search_text(None);
        self.search.previous.set_sensitive(false);
        self.search.next.set_sensitive(false);
        self.search.count.set_text("");
        self.search.count.set_tooltip_text(None);
    }
    fn stop_search_work(&self) {
        if let Some(timer) = self.search.timer.borrow_mut().take() {
            timer.remove();
        }
        if let Some(cancelled) = self.search.cancelled.borrow_mut().take() {
            cancelled.store(true, Ordering::Relaxed);
        }
        self.search
            .serial
            .set(self.search.serial.get().wrapping_add(1));
    }
    pub(super) fn pause_search(&self) {
        self.stop_search_work();
        self.search.settings.set_search_text(None);
    }
    pub(super) fn resume_search(&self) {
        if self.search.bar.is_visible() && self.search.index.get().is_some() {
            self.search
                .settings
                .set_case_sensitive(self.search.case.is_active());
            self.search
                .settings
                .set_search_text(Some(&self.search.entry.text()));
        } else {
            self.schedule_search();
        }
    }
    fn close_search(&self) {
        self.search.toggle.set_active(false);
        if self.html_active() {
            self.editor.html.focus();
        } else {
            self.active_find_view().grab_focus();
        }
    }
    pub(super) fn search_ready(&self, enabled: bool) {
        self.configure_find_mode(self.editor.editing.get());
        self.search.toggle.set_sensitive(enabled);
        self.schedule_search();
    }
    pub(super) fn schedule_search(&self) {
        self.search.anchor.borrow_mut().take();
        self.schedule_search_job();
    }
    fn schedule_search_job(&self) {
        self.cancel_search();
        if !self.widget.is_mapped()
            || !self.search.bar.is_visible()
            || !self.search.toggle.is_sensitive()
            || self.search.entry.text().is_empty()
            || self.editor.editing.get() && self.edit_find_busy()
        {
            return;
        }
        // Queries are single-line GTK input, but preserve all literal spaces.
        if self.search.entry.text().len() > crate::source_search::MAX_QUERY {
            crate::i18n_ui::bind(
                &self.search.count,
                "label",
                "Source search query exceeds 4 KiB",
            );
            return;
        }
        crate::i18n_ui::bind(&self.search.count, "label", "Searching…");
        let weak = self.search.owner.borrow().clone();
        let timer = glib::timeout_add_local_once(Duration::from_millis(140), move || {
            if let Some(source) = weak.upgrade() {
                source.search.timer.borrow_mut().take();
                source.start_search();
            }
        });
        self.search.timer.replace(Some(timer));
    }
    fn start_search(&self) {
        if self.html_active() {
            self.editor
                .html
                .find(&self.search.entry.text(), self.search.case.is_active());
            return;
        }
        let text = self.text.borrow();
        let Some(text) = text.as_ref() else {
            return;
        };
        let cancelled = Arc::new(AtomicBool::new(false));
        self.search.cancelled.replace(Some(cancelled.clone()));
        let request = if self.editor.editing.get() {
            let Some((page, edited)) = self.edit_find_input() else {
                return;
            };
            Request::SourceEditSearch {
                document: text.document.clone(),
                page,
                edited,
                query: self.search.entry.text().to_string(),
                case_sensitive: self.search.case.is_active(),
                cancelled,
            }
        } else {
            Request::SourceSearch {
                document: if self.preview_active() {
                    text.preview.as_ref().unwrap().document.clone()
                } else {
                    text.document.clone()
                },
                query: self.search.entry.text().to_string(),
                case_sensitive: self.search.case.is_active(),
                cancelled,
            }
        };
        if self
            .sender
            .try_send(Command::Workspace {
                panel: 3,
                generation: self.generation.get(),
                serial: self.search.serial.get(),
                request,
            })
            .is_err()
        {
            self.search_failed("The workspace is busy; try again");
        }
    }
    pub(super) fn search_event(&self, response: Response) {
        if response.generation != self.generation.get()
            || response.serial != self.search.serial.get()
            || !self.widget.is_mapped()
            || !self.search.bar.is_visible()
        {
            return;
        }
        self.search.cancelled.borrow_mut().take();
        match response.result {
            Ok(Reply::SourceMatches(matches)) => {
                if self.editor.editing.get() {
                    return;
                }
                let has_matches = !matches.hits.is_empty();
                self.search.matches.replace(matches);
                self.search.previous.set_sensitive(has_matches);
                self.search.next.set_sensitive(has_matches);
                self.search
                    .settings
                    .set_case_sensitive(self.search.case.is_active());
                self.search
                    .settings
                    .set_search_text(Some(&self.search.entry.text()));
                if has_matches {
                    self.search_select(0);
                } else {
                    crate::i18n_ui::bind(&self.search.count, "label", "No matches found");
                }
            }
            Ok(Reply::SourceEditedMatches(result)) => {
                if !self.editor.editing.get() || self.edit_find_busy() {
                    return;
                }
                let has_matches = !result.matches.hits.is_empty();
                let anchor = self.search.anchor.borrow_mut().take();
                let requested = anchor.as_ref().and_then(|anchor| match anchor {
                    Anchor::Logical(start) => result
                        .matches
                        .hits
                        .iter()
                        .position(|hit| hit.start == *start),
                    Anchor::Original(range) => result
                        .targets
                        .iter()
                        .position(|target| target.original.as_ref() == Some(range)),
                });
                let local = result
                    .targets
                    .iter()
                    .position(|target| target.local.is_some());
                self.search.matches.replace(result.matches);
                self.search.targets.replace(Some(result.targets));
                self.search.previous.set_sensitive(has_matches);
                self.search.next.set_sensitive(has_matches);
                self.search
                    .settings
                    .set_case_sensitive(self.search.case.is_active());
                self.search
                    .settings
                    .set_search_text(Some(&self.search.entry.text()));
                if let Some(index) = requested.or(local) {
                    self.search_select(index);
                } else if has_matches {
                    let count = self.search.matches.borrow().hits.len().to_string();
                    let limited = self.search.matches.borrow().limited;
                    let label = if limited {
                        "{count} matches · Other pages · Limit reached"
                    } else {
                        "{count} matches · Other pages"
                    };
                    crate::i18n_ui::bind_format(
                        &self.search.count,
                        "label",
                        label,
                        &[("count", &count)],
                    );
                    if limited {
                        crate::i18n_ui::bind_format(&self.search.count,"tooltip-text","Showing the first {count} matches. Use Next or Previous to open a matching page. Unsaved changes require Save, Discard or Cancel.",&[("count",&count)]);
                    } else {
                        crate::i18n_ui::bind(&self.search.count,"tooltip-text","Use Next or Previous to open a matching page. Unsaved changes require Save, Discard or Cancel.");
                    }
                } else {
                    crate::i18n_ui::bind(&self.search.count, "label", "No matches found");
                }
            }
            Err(error) => self.search_failed(&error),
            _ => {}
        }
    }
    fn search_failed(&self, error: &str) {
        // Errors can contain literal model/runtime values, never bind them as UI keys.
        crate::i18n_ui::literal(&self.search.count, "label", error);
        self.search.previous.set_sensitive(false);
        self.search.next.set_sensitive(false);
    }
    fn search_step(&self, forward: bool) {
        if self.html_active() {
            self.editor.html.step(forward);
            return;
        }
        let len = self.search.matches.borrow().hits.len();
        if len == 0
            && self.widget.is_mapped()
            && self.search.bar.is_visible()
            && !self.edit_find_busy()
            && self.search.timer.borrow().is_none()
            && self.search.cancelled.borrow().is_none()
        {
            self.schedule_search_job();
            return;
        }
        if len == 0
            || !self.widget.is_mapped()
            || self.editor.editing.get() && self.edit_find_busy()
        {
            return;
        }
        let index = match self.search.index.get() {
            Some(i) if forward => (i + 1) % len,
            Some(i) => (i + len - 1) % len,
            None if !forward => len - 1,
            None => 0,
        };
        self.search_select(index);
    }
    fn search_select(&self, index: usize) {
        let Some(Hit { start, end, line }) = self.search.matches.borrow().hits.get(index).cloned()
        else {
            return;
        };
        if self.editor.editing.get() {
            let target = self
                .search
                .targets
                .borrow()
                .as_ref()
                .and_then(|targets| targets.get(index))
                .cloned();
            let Some(target) = target else {
                return;
            };
            if let Some(local) = target.local {
                self.edit_find_select(local);
            } else {
                if self.editor.dirty.get() {
                    self.source_action(super::source_edit_ui::Action::Find {
                        logical: start,
                        original: target.original,
                        query: self.search.entry.text().to_string(),
                        case: self.search.case.is_active(),
                    });
                } else if let Some(range) = target.original {
                    self.search.anchor.replace(Some(Anchor::Logical(start)));
                    self.edit_find_jump(range);
                }
                return;
            }
        } else if self.preview_active() {
            self.preview_find_select(start..end);
        } else {
            let offset = self
                .text
                .borrow()
                .as_ref()
                .and_then(|text| text.document.window(line, None, false).ok())
                .filter(|window| start >= window.start && end <= window.end)
                .map_or(Some(start), |window| Some(window.start));
            self.show(line, offset, false);
            if let Some(window) = self.window.borrow().as_ref() {
                let begin = start.saturating_sub(window.start).min(window.text.len());
                let end = end.saturating_sub(window.start).min(window.text.len());
                let buffer = self.view.buffer();
                let a = buffer.iter_at_offset(window.text[..begin].chars().count() as i32);
                let b = buffer.iter_at_offset(window.text[..end].chars().count() as i32);
                buffer.select_range(&a, &b);
                self.editor.source_reveal.queue(false);
            }
        }
        self.search.index.set(Some(index));
        let matches = self.search.matches.borrow();
        let label = if matches.limited {
            "{index} of {count} matches · Limit reached"
        } else {
            "{index} of {count} matches"
        };
        crate::i18n_ui::bind_format(
            &self.search.count,
            "label",
            label,
            &[
                ("index", &(index + 1).to_string()),
                ("count", &matches.hits.len().to_string()),
            ],
        );
        crate::i18n_ui::bind_format(
            &self.search.count,
            "tooltip-text",
            label,
            &[
                ("index", &(index + 1).to_string()),
                ("count", &matches.hits.len().to_string()),
            ],
        );
    }
    pub(super) fn active_find_view(&self) -> &sourceview::View {
        if self.editor.editing.get() {
            self.editor.find_view()
        } else if self.preview_active() {
            &self.editor.preview.view
        } else {
            &self.view
        }
    }
    pub(super) fn preserve_edit_find(&self) {
        if self.search.anchor.borrow().is_some() {
            return;
        }
        if self.search.targets.borrow().is_some() {
            if let Some(start) = self.search.index.get().and_then(|index| {
                self.search
                    .matches
                    .borrow()
                    .hits
                    .get(index)
                    .map(|hit| hit.start)
            }) {
                self.search.anchor.replace(Some(Anchor::Logical(start)));
            }
        }
    }
    pub(super) fn refresh_edit_find(&self) {
        if self.editor.editing.get() && !self.edit_find_busy() {
            self.schedule_search_job();
        }
    }
    pub(super) fn clear_edit_find_anchor(&self) {
        self.search.anchor.borrow_mut().take();
        self.cancel_search();
    }
    pub(super) fn resume_guarded_find(
        &self,
        logical: usize,
        original: Option<std::ops::Range<usize>>,
        query: String,
        case: bool,
        saved: bool,
    ) {
        self.ensure_edit_find();
        self.search.toggle.set_active(true);
        if self.search.entry.text().as_str() == query && self.search.case.is_active() == case {
            self.search.anchor.replace(if saved {
                Some(Anchor::Logical(logical))
            } else {
                original.map(Anchor::Original)
            });
        }
        self.refresh_edit_find();
    }
}
