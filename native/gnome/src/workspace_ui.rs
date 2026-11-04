//! Native file navigation and paged source/diff presentation.
#[cfg(debug_assertions)]
#[path = "source_comments_acceptance.rs"]
mod source_comments_acceptance;
#[path = "source_edit_ui.rs"]
mod source_edit_ui;
#[path = "source_preview_ui.rs"]
mod source_preview_ui;
#[path = "source_search_ui.rs"]
mod source_search_ui;
use crate::{
    protocol::Command,
    workspace::{self, Reply, Request, Response},
};
use adw::prelude::*;
#[cfg(debug_assertions)]
use gtk::glib;
use sourceview::prelude::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use tokio::sync::mpsc;

pub use crate::files_ui::Files;

pub struct Source {
    query_button: gtk::Button,
    query: RefCell<Option<Rc<crate::reader_query_source::Launcher>>>,
    pub widget: gtk::Box,
    title: gtk::Label,
    status: gtk::Label,
    view: sourceview::View,
    close: gtk::Button,
    comment: gtk::Button,
    search: source_search_ui::Find,
    editor: source_edit_ui::Editor,
    notice: gtk::Label,
    line: Rc<Cell<usize>>,
    diff: Rc<RefCell<Option<studio_markdown_native::unified_diff::Index>>>,
    new_lines: sourceview::GutterRendererText,
    previous: gtk::Button,
    next: gtk::Button,
    text: RefCell<Option<workspace::Text>>,
    window: RefCell<Option<studio_markdown_native::SourceWindow>>,
    sender: mpsc::Sender<Command>,
    toast: adw::ToastOverlay,
    serial: Cell<u64>,
    generation: Cell<u64>,
    reading: Cell<bool>,
    reload_target: RefCell<Option<crate::source_reload::Target>>,
    initial_line: Cell<usize>,
    initial_offset: Cell<Option<usize>>,
    pub on_close: RefCell<Option<Box<dyn Fn()>>>,
    pub on_open: RefCell<Option<Box<dyn Fn()>>>,
    pub on_comment: RefCell<Option<Box<dyn Fn(crate::annotations::Capture)>>>,
}

impl Source {
    #[cfg(debug_assertions)]
    pub(crate) fn smoke_file_state(&self) -> Option<(String, Option<String>)> {
        let text = self.text.borrow();
        let text = text.as_ref()?;
        let path = text.file.as_ref()?.path.clone();
        let buffer = self.view.buffer().downcast::<sourceview::Buffer>().ok()?;
        Some((
            path,
            buffer.language().map(|language| language.id().to_string()),
        ))
    }
    pub fn has_document(&self) -> bool {
        !self.title.text().is_empty()
    }
    #[cfg(debug_assertions)]
    pub fn smoke_close(&self) {
        self.close.emit_clicked();
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_test(
        &self,
        root: &std::path::Path,
        capture: impl Fn(),
    ) -> Result<serde_json::Value, String> {
        self.open(&root.to_string_lossy(), "code.rs");
        let start = std::time::Instant::now();
        while self.text.borrow().is_none() {
            if start.elapsed() > std::time::Duration::from_secs(10) {
                return Err("Native source view timed out".into());
            }
            glib::timeout_future(std::time::Duration::from_millis(50)).await;
        }
        if !self.visible_text().starts_with("// line 1: 中文😀") {
            return Err("Native source first line changed".into());
        }
        self.page(true);
        if self.line.get() != 201 || !self.visible_text().starts_with("// line 201: 中文😀") {
            return Err("Native source paging lost its line mapping".into());
        }
        capture();
        self.page(false);
        if self.line.get() != 1 {
            return Err("Native source previous page failed".into());
        }
        self.smoke_status_language()?;
        Ok(
            serde_json::json!({"ok":true,"pagedSource":true,"sourceLineMapping":true,"unicode":true,"liveStatusLanguage":true,"literalTitle":true,"selectionRetained":true}),
        )
    }
    #[cfg(debug_assertions)]
    pub fn visible_text(&self) -> String {
        let buffer = self.view.buffer();
        buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), true)
            .to_string()
    }
    #[cfg(debug_assertions)]
    pub fn smoke_diff_origin(
        &self,
        needle: &str,
    ) -> Option<studio_markdown_native::unified_diff::Line> {
        if !self.new_lines.is_visible() {
            return None;
        }
        let window = self.window.borrow();
        let window = window.as_ref()?;
        let row = window.text.lines().position(|line| line == needle)?;
        self.diff
            .borrow()
            .as_ref()
            .map(|index| index.line(window.line + row))
    }

    pub fn new(sender: mpsc::Sender<Command>, toast: &adw::ToastOverlay) -> Rc<Self> {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let toolbar = crate::adaptive::toolbar(6);
        toolbar.set_margin_start(12);
        toolbar.set_margin_end(12);
        toolbar.set_margin_top(8);
        toolbar.set_margin_bottom(8);
        toolbar.add_css_class("toolbar");
        let title = gtk::Label::builder()
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::Middle)
            .css_classes(["heading"])
            .build();
        crate::i18n_ui::bind(&title, "label", "markdown-mode\u{4}Source");
        toolbar.append(&title);
        let query_button = button("system-search-symbolic", "Query JSON with jq");
        crate::i18n_ui::bind(&query_button, "tooltip-text", "Query JSON with jq");
        query_button.set_visible(false);
        let close = button("window-close-symbolic", "Close source");
        let comment = button("insert-text-symbolic", "Comment on selected text");
        comment.set_focus_on_click(false);
        comment.set_sensitive(false);
        crate::i18n_ui::bind(&comment, "tooltip-text", "Comment on selected text");
        widget.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        let notice = gtk::Label::builder()
            .wrap(true)
            .xalign(0.0)
            .visible(false)
            .margin_start(12)
            .margin_end(12)
            .margin_top(6)
            .margin_bottom(6)
            .build();
        notice.add_css_class("warning");
        widget.append(&notice);
        let diff = Rc::new(RefCell::new(
            None::<studio_markdown_native::unified_diff::Index>,
        ));
        let old = diff.clone();
        let (view, line) = crate::reader::text_view_with_labels(
            false,
            Some(Rc::new(move |physical| {
                old.borrow().as_ref().map_or_else(
                    || physical.to_string(),
                    |index| {
                        let old = index.line(physical).old;
                        if old == 0 {
                            String::new()
                        } else {
                            old.to_string()
                        }
                    },
                )
            })),
        );
        let new_lines = sourceview::GutterRendererText::new();
        let editor = source_edit_ui::Editor::new(&toolbar, &view);
        toolbar.append(&query_button);
        let search =
            source_search_ui::Find::new(&toolbar, &view, editor.find_view(), &editor.preview.view);
        toolbar.append(&comment);
        toolbar.append(&close);
        widget.prepend(&toolbar);
        widget.append(search.bar());
        widget.append(editor.notice());
        new_lines.set_width_request(60);
        new_lines.set_xalign(1.0);
        new_lines.set_visible(false);
        let current = diff.clone();
        let base = line.clone();
        new_lines.connect_query_data(move |gutter, _, line| {
            let number = current
                .borrow()
                .as_ref()
                .map_or(0, |index| index.line(base.get() + line as usize).new);
            gutter.set_text(&if number == 0 {
                String::new()
            } else {
                number.to_string()
            });
        });
        sourceview::prelude::ViewExt::gutter(&view, gtk::TextWindowType::Left)
            .insert(&new_lines, 1);
        widget.append(&editor.stack);
        let navigation = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let status = footer();
        status.set_hexpand(true);
        let previous = button("go-previous-symbolic", "Previous source page");
        let next = button("go-next-symbolic", "Next source page");
        navigation.append(&status);
        navigation.append(&previous);
        navigation.append(&next);
        navigation.set_margin_end(12);
        widget.append(&navigation);
        let source = Rc::new(Self {
            query_button,
            query: RefCell::default(),
            widget,
            title,
            status,
            view,
            close: close.clone(),
            comment,
            search,
            editor,
            notice,
            line,
            diff,
            new_lines,
            previous,
            next,
            text: RefCell::new(None),
            window: RefCell::new(None),
            sender,
            toast: toast.clone(),
            serial: Cell::new(0),
            generation: Cell::new(0),
            reading: Cell::new(false),
            reload_target: RefCell::new(None),
            initial_line: Cell::new(1),
            initial_offset: Cell::new(None),
            on_close: RefCell::new(None),
            on_open: RefCell::new(None),
            on_comment: RefCell::new(None),
        });
        source_search_ui::Find::wire(&source);
        source_edit_ui::Editor::wire(&source);
        let weak = Rc::downgrade(&source);
        source.comment.connect_clicked(move |_| {
            if let Some(source) = weak.upgrade() {
                if let Some(capture) = source.capture() {
                    if let Some(callback) = source.on_comment.borrow().as_ref() {
                        callback(capture);
                    }
                }
            }
        });
        let weak = Rc::downgrade(&source);
        source.view.buffer().connect_mark_set(move |_, _, mark| {
            if !matches!(mark.name().as_deref(), Some("insert" | "selection_bound")) {
                return;
            }
            if let Some(source) = weak.upgrade() {
                source.update_comment();
            }
        });
        let weak = Rc::downgrade(&source);
        source
            .editor
            .preview
            .view
            .buffer()
            .connect_mark_set(move |_, _, mark| {
                if matches!(mark.name().as_deref(), Some("insert" | "selection_bound")) {
                    if let Some(source) = weak.upgrade() {
                        source.update_comment();
                    }
                }
            });
        for mapped in [true, false] {
            let weak = Rc::downgrade(&source);
            if mapped {
                source.widget.connect_map(move |_| {
                    if let Some(source) = weak.upgrade() {
                        source.update_comment();
                        source.resume_search();
                    }
                });
            } else {
                source.widget.connect_unmap(move |_| {
                    if let Some(source) = weak.upgrade() {
                        source.update_comment();
                        source.pause_search();
                    }
                });
            }
        }
        let weak = Rc::downgrade(&source);
        source.previous.connect_clicked(move |_| {
            if let Some(source) = weak.upgrade() {
                source.page(false);
            }
        });
        let weak = Rc::downgrade(&source);
        source.next.connect_clicked(move |_| {
            if let Some(source) = weak.upgrade() {
                source.page(true);
            }
        });
        let weak = Rc::downgrade(&source);
        close.connect_clicked(move |_| {
            if let Some(source) = weak.upgrade() {
                source.source_action(source_edit_ui::Action::Close);
            }
        });
        source.theme();
        let weak = Rc::downgrade(&source);
        adw::StyleManager::default().connect_dark_notify(move |_| {
            if let Some(source) = weak.upgrade() {
                source.theme();
            }
        });
        source
    }
    pub fn open(&self, root: &str, path: &str) {
        // Zero is an internal "ordinary open" intent; explicit source locations
        // (including line 1) always keep the Source view.
        self.open_at(root, path, 0);
    }
    pub fn open_at(&self, root: &str, path: &str, line: usize) {
        self.open_request(
            path,
            line,
            Request::Read {
                root: root.into(),
                path: path.into(),
            },
        );
    }
    pub fn open_comment(
        &self,
        root: &str,
        path: &str,
        mut anchor: studio_markdown_native::quote::Anchor,
        excerpt: &str,
    ) -> Result<(), String> {
        anchor.text_kind = studio_markdown_native::quote::Kind::Source;
        let line = anchor.start_line.unwrap_or(1);
        let query = studio_markdown_native::quote::Query {
            quote: excerpt.into(),
            anchor,
        };
        query.validate()?;
        self.open_request(
            path,
            line,
            Request::ReadComment {
                root: root.into(),
                path: path.into(),
                query,
            },
        );
        Ok(())
    }
    fn open_request(&self, path: &str, line: usize, request: Request) {
        self.source_action(source_edit_ui::Action::Open {
            path: path.into(),
            line,
            request,
        });
    }
    fn open_now(&self, path: &str, line: usize, request: Request) {
        self.clear_now();
        self.reload_target.replace(match &request {
            Request::Read { root, path } | Request::ReadComment { root, path, .. } => {
                Some(crate::source_reload::Target {
                    root: root.clone(),
                    path: path.clone(),
                })
            }
            _ => None,
        });
        self.reading.set(true);
        self.editor
            .preview
            .initial
            .set(line == 0 && matches!(&request, Request::Read { .. }));
        if let Some(open) = self.on_open.borrow().as_ref() {
            open();
        }
        self.initial_line.set(line.max(1));
        self.generation.set(self.generation.get() + 1);
        self.serial.set(self.serial.get() + 1);
        crate::i18n_ui::literal(&self.title, "label", path);
        self.title.set_tooltip_text(Some(path));
        crate::i18n_ui::bind(&self.status, "label", "Opening source…");
        self.text.borrow_mut().take();
        self.view.buffer().set_text("");
        if self
            .sender
            .try_send(Command::Workspace {
                panel: 3,
                generation: self.generation.get(),
                serial: self.serial.get(),
                request,
            })
            .is_err()
        {
            self.reading.set(false);
            self.error("The workspace is busy; try again");
        }
        self.update_editor();
    }
    pub fn set_close_visible(&self, visible: bool) {
        self.close.set_visible(visible);
    }
    pub fn event(&self, response: Response) {
        if response.kind == "source-search" {
            self.search_event(response);
            return;
        }
        if response.generation != self.generation.get() || response.serial != self.serial.get() {
            return;
        }
        if matches!(response.kind, "source-edit-page" | "source-save") {
            self.edit_event(response.result);
            return;
        }
        self.reading.set(false);
        match response.result {
            Ok(Reply::Text(text)) => self.set_text(text),
            Ok(Reply::CommentText(text, located)) => {
                use studio_markdown_native::quote::Status;
                if let Some(position) = located.position {
                    self.initial_line.set(position.line);
                    self.initial_offset.set(Some(position.start));
                }
                self.set_text(text);
                let notice=match located.status {
                    Status::Unique=>None,
                    Status::Missing=>Some("Comment quote was not found. The saved location is unverified."),
                    Status::Ambiguous=>Some("Comment quote matches multiple passages. The saved location is unverified."),
                    Status::Limited=>Some("Comment lookup reached a rendering limit. The saved location is unverified."),
                };
                if let Some(message) = notice {
                    crate::i18n_ui::bind(&self.notice, "label", message);
                }
                self.notice.set_visible(notice.is_some());
            }
            Err(error) => self.error(&error),
            _ => {}
        }
        self.update_editor();
    }
    pub fn set_text(&self, text: workspace::Text) {
        self.reading.set(false);
        self.cancel_search();
        let restore = self.editor.reload_context.borrow_mut().take();
        self.reload_target
            .replace(text.file.as_ref().map(|f| crate::source_reload::Target {
                root: f.root.clone(),
                path: f.path.clone(),
            }));
        let searchable = !text.binary;
        self.comment.set_visible(
            text.file.is_some() && text.diff.is_none() && !text.binary && !text.truncated,
        );
        self.diff.replace(text.diff.clone());
        let manager = sourceview::LanguageManager::default();
        let syntax = manager
            .language(&text.language)
            .or_else(|| manager.guess_language(Some(&text.path), None));
        self.editor_language(syntax.as_ref());
        self.new_lines.set_visible(text.diff.is_some());
        crate::i18n_ui::literal(&self.title, "label", &text.path);
        self.title.set_tooltip_text(Some(&text.path));
        if let Ok(buffer) = self.view.buffer().downcast::<sourceview::Buffer>() {
            buffer.set_language(syntax.as_ref());
        }
        let (line, offset) = restore.as_ref().map_or_else(
            || {
                (
                    self.initial_line.get().min(text.document.lines()),
                    self.initial_offset.take(),
                )
            },
            |context| {
                context.position(
                    &text.document,
                    text.file.as_ref().map_or("", |f| f.hash.as_str()),
                )
            },
        );
        if let Some(html) = text.html.as_ref() {
            self.editor.html.document(html.clone());
        }
        *self.text.borrow_mut() = Some(text);
        self.show(line, offset, false);
        self.search_ready(searchable);
        self.update_editor();
        if self.editor.preview.initial.replace(false)
            && self
                .text
                .borrow()
                .as_ref()
                .is_some_and(|t| t.preview.is_some() || t.html.is_some())
        {
            self.editor.preview.button.set_active(true);
        }
        if let Some(context) = restore {
            self.restore_reload_mode(context.mode);
        }
    }
    /// Replace a previous asynchronous source request with worker-prepared data.
    /// Its late response must not overwrite the newly selected conversation diff.
    pub fn show_prepared(&self, text: workspace::Text) {
        self.source_action(source_edit_ui::Action::Prepared(text));
    }
    fn show_prepared_now(&self, text: workspace::Text) {
        self.reset_editor();
        self.notice.set_visible(false);
        self.initial_offset.set(None);
        if let Some(open) = self.on_open.borrow().as_ref() {
            open();
        }
        self.generation.set(self.generation.get() + 1);
        self.serial.set(self.serial.get() + 1);
        self.initial_line.set(1);
        self.set_text(text);
    }
    /// Invalidate asynchronous reads and remove stale text before a new selection.
    pub fn clear(&self) {
        self.source_action(source_edit_ui::Action::Clear);
    }
    fn clear_now(&self) {
        if let Some(query) = self.query.borrow().as_ref() {
            query.sync(None, false, false);
        }
        self.reading.set(false);
        self.reload_target.borrow_mut().take();
        self.reset_editor();
        self.reset_search();
        self.generation.set(self.generation.get().wrapping_add(1));
        self.serial.set(self.serial.get().wrapping_add(1));
        self.text.borrow_mut().take();
        self.window.borrow_mut().take();
        self.initial_offset.set(None);
        self.notice.set_visible(false);
        self.comment.set_sensitive(false);
        self.comment.set_visible(false);
        self.diff.borrow_mut().take();
        self.line.set(1);
        self.new_lines.set_visible(false);
        self.view.buffer().set_text("");
        crate::i18n_ui::literal(&self.title, "label", "");
        self.title.set_tooltip_text(None);
        crate::i18n_ui::literal(&self.status, "label", "");
        self.previous.set_sensitive(false);
        self.next.set_sensitive(false);
    }
    fn page(&self, forward: bool) {
        self.source_action(source_edit_ui::Action::Page(forward));
    }
    fn show(&self, line: usize, offset: Option<usize>, previous: bool) {
        let text = self.text.borrow();
        let Some(text) = text.as_ref() else { return };
        match text.document.window(line, offset, previous) {
            Ok(window) => {
                self.line.set(window.line);
                crate::source_surface::set_text(&self.view.buffer(), &window.text);
                self.view
                    .buffer()
                    .place_cursor(&self.view.buffer().start_iter());
                self.editor.source_reveal.queue(true);
                self.previous.set_sensitive(window.start > 0);
                self.next.set_sensitive(window.end < text.document.bytes());
                let mut details = Vec::with_capacity(3);
                if text.binary {
                    details.push("Binary content");
                }
                if text.truncated {
                    details.push("Truncated by Git");
                }
                if text.diff.as_ref().is_some_and(|index| index.limited) {
                    details.push("Diff line mapping limited");
                }
                crate::i18n_ui::bind_summary(
                    &self.status,
                    "label",
                    "{lines} lines · Lines {start}–{end}",
                    &[
                        ("lines", &text.document.lines().to_string()),
                        ("start", &window.line.to_string()),
                        ("end", &window.end_line.to_string()),
                    ],
                    &details,
                );
                *self.window.borrow_mut() = Some(window);
                self.update_comment();
            }
            Err(error) => self.error(&error),
        }
    }
    fn error(&self, error: &str) {
        self.editor.failed.set(true);
        crate::i18n_ui::literal(&self.status, "label", error);
        self.toast.add_toast(adw::Toast::new(error));
    }
    fn selected(&self) -> Option<(String, usize, usize)> {
        if !self.widget.is_mapped() {
            return None;
        }
        if self.editor.editing.get() || self.editor.dirty.get() {
            return None;
        }
        if self.preview_active() {
            return self.preview_selected();
        }
        let text = self.text.borrow();
        let text = text.as_ref()?;
        if text.file.is_none() || text.diff.is_some() || text.binary || text.truncated {
            return None;
        }
        let window = self.window.borrow();
        let window = window.as_ref()?;
        let buffer = self.view.buffer();
        let (start, end) = buffer.selection_bounds()?;
        if end.offset() - start.offset() > 16384 {
            return None;
        }
        let excerpt = buffer.text(&start, &end, false).to_string();
        if excerpt.trim().is_empty() || excerpt.len() > 16384 {
            return None;
        }
        Some((
            excerpt,
            window.line + start.line() as usize,
            window.line + end.line() as usize
                - usize::from(end.starts_line() && end.line() > start.line()),
        ))
    }
    pub fn enable_queries(
        &self,
        parent: &adw::ApplicationWindow,
        documents: mpsc::Sender<crate::document::Message>,
    ) {
        self.query
            .replace(Some(crate::reader_query_source::Launcher::new(
                parent,
                &self.toast,
                documents,
                &self.query_button,
            )));
        self.update_comment();
    }

    fn update_comment(&self) {
        if let Some(query) = self.query.borrow().as_ref() {
            let target = self
                .text
                .borrow()
                .as_ref()
                .and_then(|text| text.file.as_ref())
                .filter(|file| crate::reader_query::jq_supported(&file.path))
                .map(|file| crate::reader_query_source::Target {
                    root: file.root.clone(),
                    path: file.path.clone(),
                    hash: file.hash.clone(),
                });
            query.sync(target, self.editor.dirty.get(), self.reading.get());
        }
        self.comment.set_sensitive(if self.html_active() {
            !self.editor.dirty.get() && self.editor.html.comment_selection().is_some()
        } else {
            self.selected().is_some()
        });
    }
    fn capture(&self) -> Option<crate::annotations::Capture> {
        if !self.widget.is_mapped() {
            return None;
        }
        if self.html_active() && !self.editor.dirty.get() {
            let excerpt = self.editor.html.comment_selection()?;
            let text = self.text.borrow();
            let file = text.as_ref()?.file.as_ref()?;
            return Some(crate::annotations::Capture {
                excerpt,
                source: serde_json::json!({"provider":"document","version":1,
                "anchor":{"root":file.root,"filePath":file.path,"baseHash":file.hash,"textKind":"preview"}}),
            });
        }
        let (excerpt, start, end) = self.selected()?;
        let text = self.text.borrow();
        let file = text.as_ref()?.file.as_ref()?;
        Some(crate::annotations::Capture {
            excerpt,
            source: serde_json::json!({"provider":"document","version":1,
            "anchor":{"root":file.root,"filePath":file.path,"baseHash":file.hash,"startLine":start,"endLine":end,"textKind":"source"}}),
        })
    }
    fn theme(&self) {
        self.editor_theme();
        if let Ok(buffer) = self.view.buffer().downcast::<sourceview::Buffer>() {
            buffer.set_style_scheme(
                sourceview::StyleSchemeManager::default()
                    .scheme(if adw::StyleManager::default().is_dark() {
                        "Adwaita-dark"
                    } else {
                        "Adwaita"
                    })
                    .as_ref(),
            );
        }
    }
}

pub fn header(name: &str) -> (gtk::Box, gtk::Label) {
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    header.set_margin_start(12);
    header.set_margin_end(12);
    header.set_margin_top(8);
    header.set_margin_bottom(8);
    header.add_css_class("toolbar");
    let title = gtk::Label::builder()
        .label(name)
        .xalign(0.0)
        .hexpand(true)
        .ellipsize(gtk::pango::EllipsizeMode::Middle)
        .build();
    title.add_css_class("heading");
    crate::i18n_ui::bind(&title, "label", name);
    header.append(&title);
    (header, title)
}
pub fn button(name: &str, tooltip: &str) -> gtk::Button {
    let button = gtk::Button::builder()
        .icon_name(name)
        .tooltip_text(tooltip)
        .valign(gtk::Align::Center)
        .build();
    crate::i18n_ui::bind(&button, "tooltip-text", tooltip);
    button
}
pub fn footer() -> gtk::Label {
    let label = gtk::Label::builder()
        .xalign(0.0)
        .margin_start(12)
        .margin_end(12)
        .margin_top(6)
        .margin_bottom(6)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .build();
    label.add_css_class("caption");
    label
}
