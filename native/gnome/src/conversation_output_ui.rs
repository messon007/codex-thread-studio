//! One native read-only output viewer. The full snapshot stays on Rust workers.
use crate::{
    conversation_output::{Action, Page, Reference, Request, Response},
    i18n_ui::{self, UiText},
    protocol::Command,
};
use adw::prelude::*;
use gtk::{gio, glib};
use sourceview::prelude::*;
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::{Rc, Weak},
};
use tokio::sync::mpsc;
#[cfg(debug_assertions)]
#[path = "output_failure_acceptance.rs"]
mod failure_acceptance;
#[cfg(debug_assertions)]
#[path = "output_save_acceptance.rs"]
mod save_acceptance;

thread_local! {static ACTIVE:RefCell<Weak<Host>>=RefCell::new(Weak::new());}
pub struct Host {
    parent: glib::WeakRef<adw::ApplicationWindow>,
    sender: mpsc::Sender<Command>,
    sequence: Cell<u64>,
    panel: RefCell<Option<Rc<Panel>>>,
    pending_exports: RefCell<HashMap<(u64, u64), ExportContext>>,
    feedback: Rc<crate::write_feedback::Queue>,
}
struct ExportContext {
    title: String,
    file: gio::File,
}
struct Panel {
    id: u64,
    title: String,
    source: Reference,
    dialog: adw::Dialog,
    serial: Cell<u64>,
    busy: Cell<bool>,
    exporting: Cell<bool>,
    export_picker: RefCell<Option<gio::Cancellable>>,
    export_serial: Cell<u64>,
    page: RefCell<Option<Page>>,
    buffer: sourceview::Buffer,
    view: sourceview::View,
    scroll: gtk::ScrolledWindow,
    line: Rc<Cell<usize>>,
    previous: gtk::Button,
    next: gtk::Button,
    first: gtk::Button,
    last: gtk::Button,
    number: gtk::Entry,
    summary: gtk::Label,
    notice: gtk::Label,
    search: gtk::SearchEntry,
    find: gtk::Button,
    from: Cell<usize>,
    copy: gtk::Button,
    export: gtk::Button,
    _styles: Rc<crate::content_widgets::Styles>,
}
#[cfg(debug_assertions)]
pub(crate) struct ActivityProbe {
    pub dialog: adw::Dialog,
    pub buffer: sourceview::Buffer,
    pub copy: gtk::Button,
    pub first: gtk::Button,
    pub next: gtk::Button,
    pub last: gtk::Button,
    pub search: gtk::SearchEntry,
    pub find: gtk::Button,
    pub source: Reference,
    pub ready: bool,
    pub page: Option<usize>,
    pub pages: Option<usize>,
    pub selected: bool,
}
/// Factories in live, historical, Router and favorite views share this host.
pub fn open(source: Reference, title: &str) {
    ACTIVE.with(|active| {
        if let Some(host) = active.borrow().upgrade() {
            host.present(source, title);
        }
    });
}
pub fn open_details(source: Reference, title: &str) {
    ACTIVE.with(|active| {
        if let Some(host) = active.borrow().upgrade() {
            host.present_kind(source, title, "Activity details");
        }
    });
}
impl Host {
    #[cfg(debug_assertions)]
    pub(crate) fn activity_probe(&self) -> Option<ActivityProbe> {
        let p = self.panel.borrow().clone()?;
        let page = p.page.borrow();
        Some(ActivityProbe {
            dialog: p.dialog.clone(),
            buffer: p.buffer.clone(),
            copy: p.copy.clone(),
            first: p.first.clone(),
            next: p.next.clone(),
            last: p.last.clone(),
            search: p.search.clone(),
            find: p.find.clone(),
            source: p.source.clone(),
            ready: !p.busy.get() && page.is_some(),
            page: page.as_ref().map(|p| p.index),
            pages: page.as_ref().map(|p| p.pages),
            selected: page.as_ref().is_some_and(|p| p.selection.is_some()),
        })
    }
    #[cfg(debug_assertions)]
    pub(crate) fn activity_export(&self, path: std::path::PathBuf) -> Result<(), String> {
        let p = self
            .panel
            .borrow()
            .clone()
            .ok_or("Missing activity viewer")?;
        self.export_to(&p, path);
        Ok(())
    }
    pub fn install(
        parent: &adw::ApplicationWindow,
        sender: mpsc::Sender<Command>,
        feedback: Rc<crate::write_feedback::Queue>,
    ) -> Rc<Self> {
        let host = Rc::new(Self {
            parent: parent.downgrade(),
            sender,
            sequence: Cell::new(0),
            panel: RefCell::new(None),
            pending_exports: RefCell::new(HashMap::new()),
            feedback,
        });
        ACTIVE.with(|active| active.replace(Rc::downgrade(&host)));
        host
    }
    fn send(&self, command: Command) -> bool {
        self.sender.try_send(command).is_ok()
    }
    fn present(self: &Rc<Self>, source: Reference, title: &str) {
        self.present_kind(source, title, "Full output");
    }
    fn present_kind(self: &Rc<Self>, source: Reference, title: &str, heading: &'static str) {
        let Some(parent) = self.parent.upgrade() else {
            return;
        };
        let old = self.panel.borrow_mut().take();
        if let Some(old) = old {
            old.dialog.force_close();
        }
        self.sequence.set(self.sequence.get() + 1);
        let id = self.sequence.get();
        let dialog = adw::Dialog::builder()
            .content_width(1000)
            .content_height(700)
            .build();
        crate::i18n_ui::bind(&dialog, "title", heading);
        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        let caption = adw::WindowTitle::new("", title);
        crate::i18n_ui::bind(&caption, "title", heading);
        if matches!(
            title,
            "You"
                | "Assistant"
                | "Reasoning"
                | "Command"
                | "Files"
                | "Tool"
                | "Activity"
                | "Plan"
                | "Favorite"
        ) {
            crate::i18n_ui::bind(&caption, "subtitle", title);
        }
        header.set_title_widget(Some(&caption));
        let export = gtk::Button::builder()
            .icon_name("document-save-as-symbolic")
            .build()
            .ui_text("tooltip-text", "Export complete snapshot…");
        header.pack_end(&export);
        toolbar.add_top_bar(&header);
        let controls = crate::adaptive::toolbar(6);
        controls.set_margin_start(12);
        controls.set_margin_end(12);
        controls.set_margin_top(6);
        let button = |icon: &str, tip: &str| {
            gtk::Button::builder()
                .icon_name(icon)
                .build()
                .ui_text("tooltip-text", tip)
        };
        let first = button("go-first-symbolic", "First page");
        let previous = button("go-previous-symbolic", "Previous page");
        let next = button("go-next-symbolic", "Next page");
        let last = button("go-last-symbolic", "Last page");
        let number = gtk::Entry::builder()
            .width_chars(5)
            .max_width_chars(7)
            .max_length(10)
            .build()
            .ui_text("placeholder-text", "Page")
            .ui_text("tooltip-text", "Go to page (Enter)");
        for widget in [
            first.upcast_ref::<gtk::Widget>(),
            previous.upcast_ref(),
            number.upcast_ref(),
            next.upcast_ref(),
            last.upcast_ref(),
        ] {
            controls.append(widget);
        }
        let search = gtk::SearchEntry::builder()
            .hexpand(true)
            .build()
            .ui_text("placeholder-text", "Find exact text in full output");
        search.set_width_chars(1);
        search.set_max_width_chars(26);
        controls.append(&search);
        let find = button("go-down-symbolic", "Find next (case sensitive, literal)");
        controls.append(&find);
        let copy = button("edit-copy-symbolic", "Copy displayed page");
        controls.append(&copy);
        let wrap = gtk::ToggleButton::with_label("Wrap");
        i18n_ui::bind(&wrap, "label", "Wrap");
        wrap.set_active(true);
        controls.append(&wrap);
        let summary = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .margin_start(12)
            .margin_end(12)
            .margin_top(6)
            .build();
        summary.add_css_class("caption");
        let notice = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .margin_start(12)
            .margin_end(12)
            .margin_top(4)
            .margin_bottom(6)
            .build()
            .ui_text("label", "Loading immutable output snapshot…");
        notice.add_css_class("caption");
        let buffer = sourceview::Buffer::new(None::<&gtk::TextTagTable>);
        buffer.set_enable_undo(false);
        if heading != "Activity details" && (title == "Assistant" || title == "Favorite") {
            buffer.set_language(
                sourceview::LanguageManager::default()
                    .language("markdown")
                    .as_ref(),
            );
        }
        let styles = crate::content_widgets::Styles::new();
        styles.watch_buffer(&buffer);
        let view = sourceview::View::with_buffer(&buffer);
        view.set_editable(false);
        view.set_monospace(true);
        view.set_cursor_visible(true);
        view.set_wrap_mode(gtk::WrapMode::WordChar);
        view.set_left_margin(12);
        view.set_right_margin(12);
        view.set_top_margin(8);
        view.set_bottom_margin(12);
        view.add_css_class("studio-code");
        let line = Rc::new(Cell::new(1));
        let base = line.clone();
        let gutter = sourceview::GutterRendererText::new();
        gutter.set_width_request(76);
        gutter.set_xalign(1.0);
        gutter.connect_query_data(move |gutter, _, offset| {
            gutter.set_text(&(base.get() + offset as usize).to_string())
        });
        sourceview::prelude::ViewExt::gutter(&view, gtk::TextWindowType::Left).insert(&gutter, 0);
        let scroll = gtk::ScrolledWindow::builder()
            .child(&view)
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();
        let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        body.append(&controls);
        body.append(&summary);
        body.append(&notice);
        body.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        body.append(&scroll);
        toolbar.set_content(Some(&body));
        dialog.set_child(Some(&toolbar));
        let panel = Rc::new(Panel {
            id,
            title: title.to_owned(),
            source,
            dialog,
            serial: Cell::new(0),
            busy: Cell::new(false),
            exporting: Cell::new(false),
            export_picker: RefCell::new(None),
            export_serial: Cell::new(0),
            page: RefCell::new(None),
            buffer,
            view,
            scroll: scroll.clone(),
            line,
            previous,
            next,
            first,
            last,
            number,
            summary,
            notice,
            search,
            find,
            from: Cell::new(0),
            copy,
            export,
            _styles: styles,
        });
        let v = panel.view.downgrade();
        let s = scroll.downgrade();
        wrap.connect_toggled(move |button| {
            if let (Some(view), Some(scroll)) = (v.upgrade(), s.upgrade()) {
                view.set_wrap_mode(if button.is_active() {
                    gtk::WrapMode::WordChar
                } else {
                    gtk::WrapMode::None
                });
                scroll.set_hscrollbar_policy(if button.is_active() {
                    gtk::PolicyType::Never
                } else {
                    gtk::PolicyType::Automatic
                });
            }
        });
        for (button, kind) in [
            (&panel.first, 0),
            (&panel.previous, 1),
            (&panel.next, 2),
            (&panel.last, 3),
        ] {
            let h = Rc::downgrade(self);
            let p = Rc::downgrade(&panel);
            button.connect_clicked(move |_| {
                if let (Some(h), Some(p)) = (h.upgrade(), p.upgrade()) {
                    let index = p.page.borrow().as_ref().map(|p| match kind {
                        0 => 0,
                        1 => p.index.saturating_sub(1),
                        2 => (p.index + 1).min(p.pages - 1),
                        _ => p.pages - 1,
                    });
                    if let Some(index) = index {
                        h.request(&p, Action::Page(index));
                    }
                }
            });
        }
        let h = Rc::downgrade(self);
        let p = Rc::downgrade(&panel);
        panel.number.connect_activate(move |entry| {
            if let (Some(h), Some(p)) = (h.upgrade(), p.upgrade()) {
                let index = entry
                    .text()
                    .trim()
                    .parse::<usize>()
                    .ok()
                    .and_then(|n| n.checked_sub(1));
                if let Some(index) = index {
                    h.request(&p, Action::Page(index));
                } else {
                    i18n_ui::bind(&p.notice, "label", "Enter a page number starting at 1");
                }
            }
        });
        let p = Rc::downgrade(&panel);
        panel.search.connect_changed(move |_| {
            if let Some(p) = p.upgrade() {
                p.from.set(0);
            }
        });
        for activate in [false, true] {
            let h = Rc::downgrade(self);
            let p = Rc::downgrade(&panel);
            let action = move || {
                if let (Some(h), Some(p)) = (h.upgrade(), p.upgrade()) {
                    h.request(
                        &p,
                        Action::Find {
                            query: p.search.text().to_string(),
                            from: p.from.get(),
                        },
                    );
                }
            };
            if activate {
                panel.search.connect_activate(move |_| action());
            } else {
                panel.find.connect_clicked(move |_| action());
            }
        }
        let p = Rc::downgrade(&panel);
        panel.copy.connect_clicked(move |button| {
            if let Some(p) = p.upgrade() {
                if let Some(page) = p.page.borrow().as_ref() {
                    button.clipboard().set_text(&page.display);
                }
            }
        });
        let h = Rc::downgrade(self);
        let p = Rc::downgrade(&panel);
        panel.export.connect_clicked(move |_| {
            if let (Some(h), Some(p)) = (h.upgrade(), p.upgrade()) {
                h.choose_export(p);
            }
        });
        let h = Rc::downgrade(self);
        let p = Rc::downgrade(&panel);
        panel.dialog.connect_closed(move |_| {
            if let Some(p) = p.upgrade() {
                p.cancel_export_picker();
            }
            if let Some(h) = h.upgrade() {
                if h.panel.borrow().as_ref().is_some_and(|p| p.id == id) {
                    h.panel.borrow_mut().take();
                    h.send(Command::ConversationOutput(None));
                }
            }
        });
        self.panel.replace(Some(panel.clone()));
        panel.dialog.present(Some(&parent));
        self.request(&panel, Action::Page(0));
    }
    fn request(&self, p: &Panel, action: Action) {
        p.serial.set(p.serial.get() + 1);
        p.busy.set(true);
        i18n_ui::bind(&p.notice, "label", "Loading snapshot page…");
        p.sensitivity();
        if !self.send(Command::ConversationOutput(Some(Request {
            dialog: p.id,
            serial: p.serial.get(),
            source: p.source.clone(),
            action,
        }))) {
            p.busy.set(false);
            i18n_ui::bind(
                &p.notice,
                "label",
                "Native workers are busy; retry page navigation",
            );
            p.sensitivity();
        }
    }
    pub fn apply(&self, response: Response) {
        let panel = self.panel.borrow().clone();
        let Some(p) =
            panel.filter(|p| p.id == response.dialog && p.serial.get() == response.serial)
        else {
            return;
        };
        p.busy.set(false);
        match response.result {
            Ok(page) => {
                p.line.set(page.first_line);
                p.buffer.set_text(&page.display);
                p.number.set_text(&(page.index + 1).to_string());
                i18n_ui::bind_format(&p.summary, "label",
                    "Page {page} / {pages} · Lines {first}–{last} of {lines} · Bytes {start}–{end} of {bytes}",
                    &[("page", &(page.index + 1).to_string()), ("pages", &page.pages.to_string()),
                      ("first", &page.first_line.to_string()), ("last", &page.last_line.to_string()),
                      ("lines", &page.total_lines.to_string()),
                      ("start", &(if page.text.is_empty() {0} else {page.offset + 1}).to_string()),
                      ("end", &(page.offset + page.text.len()).to_string()), ("bytes", &page.total_bytes.to_string())]);
                let mut details = Vec::new();
                if page.continued {
                    details.push("continued source line");
                }
                details.push(if page.controls {
                    "controls shown as symbols; export preserves original bytes"
                } else {
                    "provider updates do not change this copy"
                });
                i18n_ui::bind_summary(&p.notice, "label", "Read-only snapshot", &[], &details);
                if let Some(selection) = &page.selection {
                    let begin = p.buffer.iter_at_offset(selection.start);
                    let end = p.buffer.iter_at_offset(selection.end);
                    p.buffer.select_range(&begin, &end);
                    p.view.scroll_to_iter(
                        &mut p.buffer.iter_at_offset(selection.start),
                        0.15,
                        false,
                        0.0,
                        0.0,
                    );
                    p.from.set(page.next_match.unwrap_or(page.total_bytes));
                    if selection.continues {
                        i18n_ui::bind(&p.notice, "label", "Match continues on the next page");
                    }
                } else {
                    p.buffer.place_cursor(&p.buffer.start_iter());
                    p.scroll.vadjustment().set_value(0.0);
                }
                p.page.replace(Some(page));
            }
            Err(error) => i18n_ui::literal(&p.notice, "label", &error),
        }
        p.sensitivity();
    }
    fn choose_export(self: &Rc<Self>, p: Rc<Panel>) {
        if self
            .panel
            .borrow()
            .as_ref()
            .is_none_or(|current| current.id != p.id)
            || !p.dialog.is_mapped()
            || p.page.borrow().is_none()
            || p.exporting.replace(true)
        {
            return;
        }
        let Some(parent) = self.parent.upgrade() else {
            p.exporting.set(false);
            return;
        };
        let cancel = gio::Cancellable::new();
        p.export_picker.replace(Some(cancel.clone()));
        p.sensitivity();
        let chooser = gtk::FileDialog::builder()
            .initial_name("conversation-output.txt")
            .build()
            .ui_text("title", "Export complete output snapshot");
        let host = Rc::downgrade(self);
        let panel = Rc::downgrade(&p);
        let request = cancel.clone();
        chooser.save(Some(&parent), Some(&cancel), move |selected| {
            let (Some(h), Some(p)) = (host.upgrade(), panel.upgrade()) else {
                return;
            };
            if p.export_picker.borrow().as_ref() != Some(&request)
                || h.panel
                    .borrow()
                    .as_ref()
                    .is_none_or(|current| current.id != p.id)
            {
                return;
            }
            p.export_picker.borrow_mut().take();
            match selected {
                Ok(file) => {
                    if let Some(path) = file.path() {
                        h.export_to(&p, path);
                    } else {
                        p.exporting.set(false);
                        i18n_ui::bind(&p.notice, "label", "Choose a local path for export");
                        p.sensitivity();
                    }
                }
                Err(error) => {
                    p.exporting.set(false);
                    if !error.matches(gtk::DialogError::Dismissed)
                        && !error.matches(gtk::DialogError::Cancelled)
                        && !error.matches(gio::IOErrorEnum::Cancelled)
                    {
                        i18n_ui::bind_format(
                            &p.notice,
                            "label",
                            "Unable to open export chooser: {error}",
                            &[("error", &error.to_string())],
                        );
                    }
                    p.sensitivity();
                }
            }
        });
    }
    fn export_to(&self, p: &Panel, path: std::path::PathBuf) {
        p.exporting.set(true);
        p.export_serial.set(p.export_serial.get() + 1);
        i18n_ui::bind(&p.notice, "label", "Exporting complete snapshot…");
        p.sensitivity();
        let operation = (p.id, p.export_serial.get());
        self.pending_exports.borrow_mut().insert(
            operation,
            ExportContext {
                title: p.title.clone(),
                file: gio::File::for_path(&path),
            },
        );
        if !self.send(Command::OutputExport {
            dialog: p.id,
            serial: p.export_serial.get(),
            source: p.source.clone(),
            path,
        }) {
            self.pending_exports.borrow_mut().remove(&operation);
            p.exporting.set(false);
            i18n_ui::bind(&p.notice, "label", "Native workers are busy; retry export");
            p.sensitivity();
        }
    }
    pub fn exported(&self, dialog: u64, serial: u64, result: Result<(), String>) {
        let Some(context) = self.pending_exports.borrow_mut().remove(&(dialog, serial)) else {
            return; // Duplicate/stale completions cannot change a newer export.
        };
        if let Err(error) = &result {
            self.feedback
                .export_failed(&context.title, &context.file, error);
        }
        if let Some(p) = self
            .panel
            .borrow()
            .as_ref()
            .filter(|p| p.id == dialog && p.export_serial.get() == serial)
        {
            p.exporting.set(false);
            match result {
                Ok(()) => i18n_ui::bind(&p.notice, "label", "Exported complete snapshot"),
                Err(e) => i18n_ui::bind_format(
                    &p.notice,
                    "label",
                    "Export failed: {error}",
                    &[("error", &e)],
                ),
            }
            p.sensitivity();
        }
    }
    pub fn allow_window_close(&self) -> bool {
        let panel = self.panel.borrow().clone();
        if let Some(panel) = panel {
            panel.cancel_export_picker();
        }
        !self.writing()
    }
    pub fn writing(&self) -> bool {
        !self.pending_exports.borrow().is_empty()
    }
}
impl Panel {
    fn cancel_export_picker(&self) {
        let pending = self.export_picker.borrow_mut().take();
        if let Some(cancel) = pending {
            cancel.cancel();
            self.exporting.set(false);
            self.sensitivity();
        }
    }
    fn sensitivity(&self) {
        let page = self.page.borrow();
        let valid = !self.busy.get();
        self.first
            .set_sensitive(valid && page.as_ref().is_some_and(|p| p.index > 0));
        self.previous.set_sensitive(self.first.is_sensitive());
        self.last
            .set_sensitive(valid && page.as_ref().is_some_and(|p| p.index + 1 < p.pages));
        self.next.set_sensitive(self.last.is_sensitive());
        self.copy.set_sensitive(page.is_some());
        self.export
            .set_sensitive(page.is_some() && !self.exporting.get());
        self.find.set_sensitive(valid);
        self.number.set_sensitive(valid);
    }
}

impl Drop for Panel {
    fn drop(&mut self) {
        if let Some(cancel) = self.export_picker.get_mut().take() {
            cancel.cancel();
        }
    }
}

#[cfg(debug_assertions)]
impl Host {
    pub fn smoke_open(&self) -> bool {
        self.panel.borrow().is_some()
    }
    pub(crate) fn smoke_gateway_export(
        self: &Rc<Self>,
        source: Reference,
        path: std::path::PathBuf,
    ) {
        self.present(source, "Gateway-loss immutable export");
        if let Some(panel) = self.panel.borrow().clone() {
            self.export_to(&panel, path);
        }
    }
    pub(crate) fn smoke_gateway_export_close(&self) {
        let panel = self.panel.borrow().clone();
        if let Some(panel) = panel {
            panel.dialog.close();
        }
    }
    pub async fn smoke(
        self: &Rc<Self>,
        root: &std::path::Path,
        case: &str,
    ) -> Result<serde_json::Value, String> {
        async fn until(step: &str, test: impl Fn() -> bool) -> Result<(), String> {
            let start = std::time::Instant::now();
            while !test() {
                if start.elapsed() > std::time::Duration::from_secs(12) {
                    return Err(format!("Output viewer timed out: {step}"));
                }
                glib::timeout_future(std::time::Duration::from_millis(20)).await;
            }
            Ok(())
        }
        let panel = self
            .panel
            .borrow()
            .clone()
            .ok_or("Full-output header action did not open its viewer")?;
        let started = std::time::Instant::now();
        let mapped = until("mapped source page", || {
            !panel.busy.get()
                && panel.page.borrow().is_some()
                && panel.view.is_mapped()
                && panel.view.height() > 0
                && panel
                    .view
                    .iter_location(&panel.buffer.start_iter())
                    .height()
                    > 0
        })
        .await;
        if let Err(error) = mapped {
            let details=format!(
                "{error}; busy={} page={} notice={} mapped={} size={}x{} chars={} iterHeight={} parent={:?}",
                panel.busy.get(),
                panel.page.borrow().is_some(),
                panel.notice.text(),
                panel.view.is_mapped(),
                panel.view.width(),
                panel.view.height(),
                panel.buffer.char_count(),
                panel
                    .view
                    .iter_location(&panel.buffer.start_iter())
                    .height(),
                self.parent.upgrade().map(|w|(w.is_mapped(),w.width(),w.height(),w.is_active()))
            );
            if let Some(window) = self.parent.upgrade() {
                let _ = crate::window_capture::capture_dialog(
                    &window,
                    &panel.dialog,
                    &root.join(format!("output-{case}-failure.png")),
                )
                .await;
            }
            return Err(details);
        }
        let open_ms = started.elapsed().as_millis();
        let (pages, total_bytes) = {
            let p = panel.page.borrow();
            let p = p.as_ref().unwrap();
            if p.index != 0
                || p.text.len() > crate::conversation_output::PAGE_BYTES
                || panel.view.is_editable()
            {
                return Err("Output did not open as a bounded read-only page".into());
            }
            if panel.buffer.line_count() as usize != p.last_line - p.first_line + 1 {
                return Err("GTK source lines differ from snapshot source offsets".into());
            }
            (p.pages, p.total_bytes)
        };
        if pages < 2 {
            return Err("Large output was not paginated".into());
        }
        panel.next.emit_clicked();
        until("second page", || {
            !panel.busy.get() && panel.page.borrow().as_ref().is_some_and(|p| p.index == 1)
        })
        .await?;
        if panel.line.get() != panel.page.borrow().as_ref().unwrap().first_line {
            return Err("Source gutter lost its global line offset".into());
        }
        if case == "command" && !panel.page.borrow().as_ref().unwrap().continued {
            return Err("A long source line was not marked as continued".into());
        }
        if case == "assistant" {
            panel.last.emit_clicked();
            until("last page", || {
                !panel.busy.get()
                    && panel
                        .page
                        .borrow()
                        .as_ref()
                        .is_some_and(|p| p.index + 1 == pages)
            })
            .await?;
            panel.search.set_text("output-tail-marker 中文😀 ");
            panel.search.emit_activate();
            until("literal full-output search", || {
                !panel.busy.get()
                    && panel
                        .page
                        .borrow()
                        .as_ref()
                        .is_some_and(|p| p.selection.is_some())
            })
            .await?;
            let first = panel.from.get();
            panel.find.emit_clicked();
            until("next full-output match", || {
                !panel.busy.get() && panel.from.get() > first
            })
            .await?;
            let selected = panel
                .buffer
                .selection_bounds()
                .map(|(a, b)| panel.buffer.text(&a, &b, true).to_string());
            if selected.as_deref() != Some("output-tail-marker 中文😀 ") {
                return Err("Full-output search lost its Unicode or trailing space".into());
            }
            let old = panel
                .buffer
                .text(&panel.buffer.start_iter(), &panel.buffer.end_iter(), true)
                .to_string();
            panel.search.set_text("output-tail-marker 中文😀  ");
            panel.search.emit_activate();
            until("missing literal retained page", || {
                !panel.busy.get() && panel.notice.text().contains("No matching")
            })
            .await?;
            if panel
                .buffer
                .text(&panel.buffer.start_iter(), &panel.buffer.end_iter(), true)
                .as_str()
                != old
            {
                return Err("An unsuccessful find replaced the reading page".into());
            }
            panel.number.set_text("9999999");
            panel.number.emit_activate();
            until("invalid page retained content", || {
                !panel.busy.get() && panel.notice.text().contains("outside")
            })
            .await?;
            if panel
                .buffer
                .text(&panel.buffer.start_iter(), &panel.buffer.end_iter(), true)
                .as_str()
                != old
            {
                return Err("An invalid page replaced the reading page".into());
            }
            self.request(&panel, Action::Page(0));
            until("restored first page", || {
                !panel.busy.get() && panel.page.borrow().as_ref().is_some_and(|p| p.index == 0)
            })
            .await?;
        }
        let window = self
            .parent
            .upgrade()
            .ok_or("Output parent window disappeared")?;
        crate::window_capture::capture_dialog(
            &window,
            &panel.dialog,
            &root.join(format!("output-{case}.png")),
        )
        .await?;
        self.export_to(&panel, root.join(format!("snapshot-{case}.txt")));
        until("full snapshot export", || !panel.exporting.get()).await?;
        if panel.notice.text() != "Exported complete snapshot" {
            return Err(panel.notice.text().to_string());
        }
        let closing = if case == "assistant" {
            let source = panel.source.clone();
            let old_id = panel.id;
            let old_serial = panel.serial.get();
            let stale = gtk::gio::spawn_blocking(move || {
                crate::conversation_output::execute(Request {
                    dialog: old_id,
                    serial: old_serial,
                    source,
                    action: Action::Page(0),
                })
            })
            .await
            .map_err(|_| "Output fixture worker stopped")??;
            self.present(
                Reference {
                    token: "0".repeat(32),
                    bytes: 1,
                    unavailable: None,
                },
                "Expired snapshot",
            );
            let replacement = self.panel.borrow().clone().unwrap();
            until("expired snapshot", || {
                !replacement.busy.get() && replacement.notice.text().contains("expired")
            })
            .await?;
            self.apply(Response {
                dialog: old_id,
                serial: old_serial,
                result: Ok(stale),
            });
            if replacement.page.borrow().is_some() || replacement.buffer.char_count() != 0 {
                return Err("A stale page crossed output dialog identities".into());
            }
            replacement.dialog.close();
            replacement.dialog.clone()
        } else {
            panel.dialog.close();
            panel.dialog.clone()
        };
        until("output dialog closed", || {
            self.panel.borrow().is_none() && !closing.is_mapped()
        })
        .await?;
        Ok(
            serde_json::json!({"ok":true,"case":case,"sourceBytes":total_bytes,"pages":pages,"readOnly":true,"boundedPage":true,"globalSourceLines":true,"immutableSnapshot":true,"fullExport":true,"mappedPageMs":open_ms}),
        )
    }
}
