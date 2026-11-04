//! Explicit page editing for identified ordinary files; GTK owns only its page.
#[cfg(debug_assertions)]
#[path = "source_edit_acceptance.rs"]
mod acceptance;
#[cfg(debug_assertions)]
#[path = "html_acceptance.rs"]
mod html_acceptance;
#[cfg(debug_assertions)]
#[path = "source_preview_acceptance.rs"]
mod preview_acceptance;
#[cfg(debug_assertions)]
#[path = "source_reload_acceptance.rs"]
mod reload_acceptance;
use super::*;
use gtk::glib;
use std::rc::Weak;
use studio_markdown_native::{
    edit_buffer::MAX_PAGE_BYTES,
    text_edit::{self, Mutation, Page},
};

pub(super) enum Action {
    Reload(crate::source_reload::Context),
    HtmlHeading {
        label: String,
        depth: usize,
    },
    Open {
        path: String,
        line: usize,
        request: Request,
    },
    Prepared(workspace::Text),
    Clear,
    Close,
    Quit,
    Page(bool),
    Find {
        logical: usize,
        original: Option<std::ops::Range<usize>>,
        query: String,
        case: bool,
    },
}
pub(super) struct Editor {
    pub(super) stack: gtk::Stack,
    pub(super) preview: super::source_preview_ui::Preview,
    pub(super) html: Rc<crate::html_ui::Preview>,
    pub(super) source_reveal: Rc<crate::text_reveal::Reveal>,
    reveal: Rc<crate::text_reveal::Reveal>,
    view: sourceview::View,
    line: Rc<Cell<usize>>,
    modes: gtk::Box,
    source: gtk::ToggleButton,
    edit: gtk::ToggleButton,
    save: gtk::Button,
    undo: gtk::Button,
    redo: gtk::Button,
    reload: gtk::Button,
    pub(super) reload_context: RefCell<Option<crate::source_reload::Context>>,
    notice: gtk::Label,
    owner: RefCell<Weak<Source>>,
    page: RefCell<Option<Page>>,
    operations: RefCell<Vec<Mutation>>,
    bytes: Cell<usize>,
    inserted: Cell<usize>,
    pub(super) editing: Cell<bool>,
    pub(super) dirty: Cell<bool>,
    busy: Cell<bool>,
    loading: Cell<bool>,
    pub(super) failed: Cell<bool>,
    baseline_lines: Cell<i32>,
    pending: RefCell<Option<Action>>,
    guard: RefCell<Option<adw::AlertDialog>>,
}
impl Editor {
    pub(super) fn find_view(&self) -> &sourceview::View {
        &self.view
    }
    pub(super) fn new(toolbar: &adw::WrapBox, source: &sourceview::View) -> Self {
        let modes = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        modes.add_css_class("linked");
        modes.set_valign(gtk::Align::Center);
        let source_mode = gtk::ToggleButton::new();
        crate::i18n_ui::bind(&source_mode, "label", "Source code");
        source_mode.set_active(true);
        let edit = gtk::ToggleButton::new();
        crate::i18n_ui::bind(&edit, "label", "Edit");
        edit.set_group(Some(&source_mode));
        let preview = super::source_preview_ui::Preview::new(&modes, &source_mode);
        modes.append(&source_mode);
        modes.append(&edit);
        for button in [&preview.button, &source_mode, &edit] {
            crate::adaptive::compact_button(button);
        }
        toolbar.append(&modes);
        let html = crate::html_ui::Preview::new();
        toolbar.append(&html.contents);
        let reload = button("view-refresh-symbolic", "Reload saved document");
        crate::i18n_ui::accessible_label(&reload, "Reload saved document");
        toolbar.append(&reload);
        let save = button("document-save-symbolic", "Save current page (Ctrl+S)");
        let undo = button("edit-undo-symbolic", "Undo");
        let redo = button("edit-redo-symbolic", "Redo");
        for (widget, label) in [
            (&save, "Save current page (Ctrl+S)"),
            (&undo, "Undo"),
            (&redo, "Redo"),
        ] {
            crate::i18n_ui::bind(widget, "tooltip-text", label);
            crate::i18n_ui::accessible_label(widget, label);
            toolbar.append(widget);
        }
        let (view, line) = crate::reader::text_view(true);
        let stack = gtk::Stack::builder().vexpand(true).hexpand(true).build();
        for (view, name) in [(source, "source"), (&view, "edit")] {
            stack.add_named(
                &gtk::ScrolledWindow::builder()
                    .child(view)
                    .vexpand(true)
                    .hexpand(true)
                    .build(),
                Some(name),
            );
        }
        stack.set_visible_child_name("source");
        stack.add_named(&preview.scroll, Some("preview"));
        stack.add_named(&html.widget, Some("html"));
        let source_reveal = crate::text_reveal::Reveal::new(source);
        let reveal = crate::text_reveal::Reveal::new(&view);
        let notice = gtk::Label::builder()
            .wrap(true)
            .xalign(0.0)
            .margin_start(12)
            .margin_end(12)
            .margin_top(6)
            .margin_bottom(6)
            .visible(false)
            .build();
        notice.add_css_class("warning");
        crate::i18n_ui::bind(
            &notice,
            "label",
            "Unsaved edits are kept in memory. Save to refresh reading views.",
        );
        modes.set_visible(false);
        save.set_visible(false);
        undo.set_visible(false);
        redo.set_visible(false);
        reload.set_visible(false);
        Self {
            stack,
            preview,
            html,
            source_reveal,
            reveal,
            view,
            line,
            modes,
            source: source_mode,
            edit,
            save,
            undo,
            redo,
            reload,
            reload_context: RefCell::new(None),
            notice,
            owner: RefCell::new(Weak::new()),
            page: RefCell::new(None),
            operations: RefCell::new(Vec::new()),
            bytes: Cell::new(0),
            inserted: Cell::new(0),
            editing: Cell::new(false),
            dirty: Cell::new(false),
            busy: Cell::new(false),
            loading: Cell::new(false),
            failed: Cell::new(false),
            baseline_lines: Cell::new(1),
            pending: RefCell::new(None),
            guard: RefCell::new(None),
        }
    }
    pub(super) fn notice(&self) -> &gtk::Label {
        &self.notice
    }
    pub(super) fn wire(source: &Rc<Source>) {
        source.editor.owner.replace(Rc::downgrade(source));
        let weak = Rc::downgrade(source);
        source.editor.reload.connect_clicked(move |_| {
            if let Some(source) = weak.upgrade() {
                source.reload_document();
            }
        });
        let weak = Rc::downgrade(source);
        source.editor.html.changed.replace(Some(Box::new(move || {
            if let Some(source) = weak.upgrade() {
                source.update_comment();
            }
        })));
        let weak = Rc::downgrade(source);
        source
            .editor
            .html
            .navigate
            .replace(Some(Box::new(move |id| {
                if let Some(source) = weak.upgrade() {
                    if source.html_active() {
                        source.editor.html.jump(id);
                    } else {
                        let target = source
                            .text
                            .borrow()
                            .as_ref()
                            .and_then(|t| t.html.as_ref())
                            .and_then(|h| h.headings.get(id))
                            .filter(|h| h.original.is_some())
                            .map(|h| (h.label.clone(), h.depth));
                        if let Some((label, depth)) = target {
                            source.source_action(Action::HtmlHeading { label, depth });
                        } else {
                            source.error(&crate::language::tr(
                                "This heading has no verified Source location.",
                            ));
                        }
                    }
                }
            })));
        source.wire_html_find();
        for (button, editing) in [
            (&source.editor.source, false),
            (&source.editor.edit, true),
            (&source.editor.preview.button, false),
        ] {
            let weak = Rc::downgrade(source);
            button.connect_toggled(move |button| {
                if button.is_active() {
                    if let Some(source) = weak.upgrade() {
                        source.set_edit_mode(editing);
                    }
                }
            });
        }
        let weak = Rc::downgrade(source);
        source.editor.save.connect_clicked(move |_| {
            if let Some(source) = weak.upgrade() {
                source.save_source(false);
            }
        });
        for (button, redo) in [(&source.editor.undo, false), (&source.editor.redo, true)] {
            let weak = Rc::downgrade(source);
            button.connect_clicked(move |_| {
                if let Some(source) = weak.upgrade() {
                    let buffer = source.editor.view.buffer();
                    if redo {
                        buffer.redo()
                    } else {
                        buffer.undo()
                    }
                }
            });
        }
        let buffer = source.editor.view.buffer();
        let weak = Rc::downgrade(source);
        buffer.connect_changed(move |_| {
            if let Some(source) = weak.upgrade() {
                source.update_editor();
                if !source.editor.loading.get() {
                    source.schedule_search();
                }
            }
        });
        for redo in [false, true] {
            let weak = Rc::downgrade(source);
            if redo {
                buffer.connect_can_redo_notify(move |_| {
                    if let Some(source) = weak.upgrade() {
                        source.update_editor();
                    }
                });
            } else {
                buffer.connect_can_undo_notify(move |_| {
                    if let Some(source) = weak.upgrade() {
                        source.update_editor();
                    }
                });
            }
        }
        let weak = Rc::downgrade(source);
        buffer.connect_modified_changed(move |buffer| {
            if let Some(source) = weak.upgrade() {
                if !source.editor.loading.get() {
                    let dirty = buffer.is_modified()
                        || source.editor.page.borrow().as_ref().is_some_and(|page| {
                            buffer
                                .text(&buffer.start_iter(), &buffer.end_iter(), true)
                                .as_str()
                                != page.normalized
                        });
                    source.editor.dirty.set(dirty);
                    if !dirty {
                        source.editor.operations.borrow_mut().clear();
                        source.editor.inserted.set(0);
                    }
                    source.update_editor();
                }
            }
        });
        let weak = Rc::downgrade(source);
        buffer.connect_insert_text(move |buffer, iter, text| {
            let Some(source) = weak.upgrade() else { return };
            if source.editor.loading.get() {
                return;
            }
            let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
            if normalized != text {
                buffer.stop_signal_emission_by_name("insert-text");
                let mut at = *iter;
                buffer.insert(&mut at, &normalized);
                return;
            }
            if source.editor.busy.get()
                || !source.editor.editing.get()
                || text.contains('\0')
                || source.editor.bytes.get().saturating_add(text.len()) > MAX_PAGE_BYTES
                || source.editor.operations.borrow().len() >= text_edit::MAX_OPERATIONS
                || source.editor.inserted.get().saturating_add(text.len())
                    > text_edit::MAX_OPERATION_BYTES
            {
                buffer.stop_signal_emission_by_name("insert-text");
                source.error("Save the current edit page before making more changes");
                return;
            }
            source
                .editor
                .operations
                .borrow_mut()
                .push(Mutation::Insert {
                    offset: iter.offset() as usize,
                    text: text.into(),
                });
            source
                .editor
                .bytes
                .set(source.editor.bytes.get() + text.len());
            source
                .editor
                .inserted
                .set(source.editor.inserted.get() + text.len());
        });
        let weak = Rc::downgrade(source);
        buffer.connect_delete_range(move |buffer, start, end| {
            let Some(source) = weak.upgrade() else { return };
            if source.editor.loading.get() {
                return;
            }
            if source.editor.busy.get()
                || !source.editor.editing.get()
                || source.editor.operations.borrow().len() >= text_edit::MAX_OPERATIONS
            {
                buffer.stop_signal_emission_by_name("delete-range");
                source.error("Save the current edit page before making more changes");
                return;
            }
            let bytes = buffer.text(start, end, true).len();
            source
                .editor
                .operations
                .borrow_mut()
                .push(Mutation::Delete {
                    from: start.offset() as usize,
                    to: end.offset() as usize,
                });
            source
                .editor
                .bytes
                .set(source.editor.bytes.get().saturating_sub(bytes));
        });
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(source);
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            if modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                && matches!(key, gtk::gdk::Key::s | gtk::gdk::Key::S)
            {
                if let Some(source) = weak.upgrade() {
                    source.save_source(false);
                }
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        source.widget.add_controller(keys);
    }
}
impl Source {
    pub(super) fn reload_document(&self) {
        use crate::source_reload::{Context, Mode};
        if self.reading.get() || self.reload_target.borrow().is_none() {
            return;
        }
        let context = if self.text.borrow().is_none() {
            self.editor
                .reload_context
                .borrow()
                .clone()
                .unwrap_or(Context {
                    mode: Mode::Source,
                    line: 1,
                    offset: None,
                    revision: String::new(),
                })
        } else {
            let mode = if self.editor.editing.get() {
                Mode::Edit
            } else if self.preview_active() {
                Mode::Preview
            } else {
                Mode::Source
            };
            let offset = if mode == Mode::Preview {
                self.preview_original_cursor()
            } else if mode == Mode::Edit {
                self.editor.page.borrow().as_ref().map(|p| p.window.start)
            } else {
                self.window.borrow().as_ref().map(|w| w.start)
            };
            let text = self.text.borrow();
            let text = text.as_ref().unwrap();
            Context {
                mode,
                line: offset.map_or(1, |at| text.document.line_at(at)),
                offset,
                revision: text
                    .file
                    .as_ref()
                    .map_or_else(String::new, |f| f.hash.clone()),
            }
        };
        self.source_action(Action::Reload(context));
    }
    fn reload_now(&self, context: crate::source_reload::Context) {
        let target = self.reload_target.borrow().clone();
        if let Some(target) = target {
            self.open_now(
                &target.path,
                context.line.max(1),
                Request::Read {
                    root: target.root,
                    path: target.path.clone(),
                },
            );
            self.editor.reload_context.replace(Some(context));
        }
    }
    pub(super) fn restore_reload_mode(&self, mode: crate::source_reload::Mode) {
        match mode {
            crate::source_reload::Mode::Edit if self.writable_source() => {
                self.editor.edit.set_active(true)
            }
            crate::source_reload::Mode::Preview
                if self
                    .text
                    .borrow()
                    .as_ref()
                    .is_some_and(|t| t.preview.is_some() || t.html.is_some()) =>
            {
                self.editor.preview.button.set_active(true)
            }
            _ => self.editor.source.set_active(true),
        }
    }
    fn writable_source(&self) -> bool {
        self.text.borrow().as_ref().is_some_and(|text| {
            text.file.as_ref().is_some_and(|f| !f.read_only)
                && !text.binary
                && !text.truncated
                && text.diff.is_none()
        })
    }
    pub(super) fn reset_editor(&self) {
        self.editor.loading.set(true);
        self.editor.reload_context.borrow_mut().take();
        self.editor.reload.set_visible(false);
        self.editor.reload.set_sensitive(false);
        self.editor.html.clear();
        self.editor.preview.initial.set(false);
        self.editor.preview.window.borrow_mut().take();
        self.editor.preview.view.buffer().set_text("");
        self.editor.failed.set(false);
        self.editor.dirty.set(false);
        self.editor.busy.set(false);
        self.editor.editing.set(false);
        self.editor.pending.borrow_mut().take();
        self.editor.page.borrow_mut().take();
        self.editor.operations.borrow_mut().clear();
        let guard = self.editor.guard.borrow_mut().take();
        if let Some(guard) = guard {
            guard.force_close();
        }
        self.editor.view.buffer().set_text("");
        self.editor.view.buffer().set_modified(false);
        self.editor.bytes.set(0);
        self.editor.inserted.set(0);
        self.editor.source.set_active(true);
        self.editor.stack.set_visible_child_name("source");
        self.editor.modes.set_visible(false);
        self.editor.save.set_visible(false);
        self.editor.undo.set_visible(false);
        self.editor.redo.set_visible(false);
        self.editor.notice.set_visible(false);
        self.editor.loading.set(false);
    }
    pub(super) fn update_editor(&self) {
        if self.editor.loading.get() {
            return;
        }
        let enabled = self.writable_source();
        let busy = self.editor.busy.get();
        self.editor
            .reload
            .set_visible(self.reload_target.borrow().is_some());
        self.editor
            .reload
            .set_sensitive(!busy && !self.reading.get());
        let editing = self.editor.editing.get();
        if let Some(text) = self.text.borrow().as_ref() {
            crate::i18n_ui::literal(
                &self.title,
                "label",
                &format!(
                    "{}{}",
                    text.path,
                    if self.editor.dirty.get() { " •" } else { "" }
                ),
            );
        }
        let preview = self
            .text
            .borrow()
            .as_ref()
            .is_some_and(|t| t.preview.is_some() || t.html.is_some());
        self.editor.modes.set_visible(enabled || preview);
        self.editor.preview.button.set_visible(preview);
        self.editor.edit.set_visible(enabled);
        self.editor.modes.set_sensitive(!busy);
        self.editor.save.set_visible(enabled);
        self.editor
            .save
            .set_sensitive(enabled && !busy && self.editor.dirty.get());
        self.editor.undo.set_visible(enabled && editing);
        self.editor.redo.set_visible(enabled && editing);
        let buffer = self.editor.view.buffer();
        if editing && !busy && !self.editor.failed.get() {
            if let Some(page) = self.editor.page.borrow().as_ref() {
                let lines = self
                    .text
                    .borrow()
                    .as_ref()
                    .map_or(1, |text| text.document.lines()) as i64
                    + i64::from(buffer.line_count() - self.editor.baseline_lines.get());
                let end = page.window.line + (buffer.line_count() as usize).saturating_sub(1)
                    - usize::from(buffer.char_count() > 0 && buffer.end_iter().starts_line());
                let label = if self.editor.dirty.get() {
                    "{lines} lines · Lines {start}–{end} · Unsaved"
                } else {
                    "{lines} lines · Lines {start}–{end}"
                };
                crate::i18n_ui::bind_format(
                    &self.status,
                    "label",
                    label,
                    &[
                        ("lines", &lines.to_string()),
                        ("start", &page.window.line.to_string()),
                        ("end", &end.to_string()),
                    ],
                );
            }
        }
        self.editor.undo.set_sensitive(!busy && buffer.can_undo());
        self.editor.redo.set_sensitive(!busy && buffer.can_redo());
        self.editor.view.set_editable(enabled && editing && !busy);
        self.editor
            .notice
            .set_visible(self.editor.dirty.get() && !editing);
        if busy {
            self.previous.set_sensitive(false);
            self.next.set_sensitive(false);
            self.close.set_sensitive(false);
        } else {
            self.close.set_sensitive(true);
            if editing {
                if let Some(page) = self.editor.page.borrow().as_ref() {
                    self.previous.set_sensitive(page.window.start > 0);
                    self.next.set_sensitive(
                        self.text
                            .borrow()
                            .as_ref()
                            .is_some_and(|t| page.window.end < t.document.bytes()),
                    );
                }
            }
        }
        self.update_comment();
    }
    fn set_edit_mode(&self, editing: bool) {
        if self.editor.loading.get() || self.editor.busy.get() {
            return;
        }
        if editing && !self.writable_source() {
            return;
        }
        let was_preview = self.editor.stack.visible_child_name().as_deref() == Some("preview");
        if was_preview && !self.editor.dirty.get() {
            if let Some(original) = self.preview_original_cursor() {
                let position = self.text.borrow().as_ref().map(|t| {
                    let line = t.document.line_at(original);
                    let context = t
                        .document
                        .window(1, None, false)
                        .ok()
                        .filter(|w| original < w.end)
                        .or_else(|| {
                            t.document
                                .window(line, None, false)
                                .ok()
                                .filter(|w| original < w.end)
                        });
                    (line, context.map_or(original, |w| w.start))
                });
                if let Some((line, offset)) = position {
                    self.show(line, Some(offset), false);
                }
            }
        }
        self.editor.editing.set(editing);
        self.edit_search_mode(editing);
        if !editing && self.editor.preview.button.is_active() {
            self.show_preview_from_source();
            self.update_editor();
            return;
        }
        self.editor
            .stack
            .set_visible_child_name(if editing { "edit" } else { "source" });
        if editing {
            let keep = self.editor.page.borrow().as_ref().is_some_and(|page| {
                self.editor.dirty.get()
                    || self
                        .window
                        .borrow()
                        .as_ref()
                        .is_some_and(|window| window.start == page.window.start)
            });
            if !keep {
                self.prepare_source_edit();
            } else {
                self.editor.view.grab_focus();
                self.editor.reveal.queue(false);
            }
        } else {
            let position = self
                .window
                .borrow()
                .as_ref()
                .map(|window| (window.line, window.start));
            if let Some((line, start)) = position {
                self.show(line, Some(start), false);
            }
        }
        self.update_editor();
    }
    fn prepare_source_edit(&self) {
        let text = self.text.borrow();
        let Some(text) = text.as_ref() else { return };
        let window = self.window.borrow();
        let Some(window) = window.as_ref() else {
            return;
        };
        self.editor.busy.set(true);
        self.editor.failed.set(false);
        self.serial.set(self.serial.get().wrapping_add(1));
        if self
            .sender
            .try_send(Command::Workspace {
                panel: 3,
                generation: self.generation.get(),
                serial: self.serial.get(),
                request: Request::SourceEditPage {
                    document: text.document.clone(),
                    line: window.line,
                    offset: Some(window.start),
                    previous: false,
                },
            })
            .is_err()
        {
            self.editor.busy.set(false);
            self.error("The workspace is busy; try again");
        }
    }
    fn save_source(&self, overwrite: bool) {
        if self.editor.busy.get() || !self.editor.dirty.get() || !self.writable_source() {
            return;
        }
        let Some(snapshot) = self.text.borrow().clone() else {
            return;
        };
        let Some(page) = self.editor.page.borrow().clone() else {
            return;
        };
        let buffer = self.editor.view.buffer();
        let edited = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), true)
            .to_string();
        self.editor.busy.set(true);
        self.editor.failed.set(false);
        self.serial.set(self.serial.get().wrapping_add(1));
        let request = Request::SourceSave {
            snapshot,
            page,
            operations: self.editor.operations.borrow().clone(),
            edited,
            overwrite,
        };
        crate::i18n_ui::bind(&self.status, "label", "Saving current page…");
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
            self.editor.busy.set(false);
            self.editor.pending.borrow_mut().take();
            self.error("The workspace is busy; try again");
        }
        self.update_editor();
    }
    pub(super) fn edit_event(&self, result: Result<Reply, String>) {
        self.editor.busy.set(false);
        match result {
            Ok(Reply::SourceEditPage(page)) => {
                self.editor.failed.set(false);
                self.editor.loading.set(true);
                let buffer = self.editor.view.buffer();
                buffer.begin_irreversible_action();
                crate::source_surface::set_text(&buffer, &page.normalized);
                buffer.end_irreversible_action();
                buffer.set_modified(false);
                buffer.place_cursor(&buffer.start_iter());
                self.editor.reveal.queue(true);
                self.editor.line.set(page.window.line);
                self.editor.bytes.set(page.normalized.len());
                self.editor.inserted.set(0);
                self.editor.operations.borrow_mut().clear();
                self.editor.dirty.set(false);
                self.editor.page.replace(Some(page));
                self.editor.baseline_lines.set(buffer.line_count());
                self.editor.loading.set(false);
                self.editor.view.grab_focus();
            }
            Ok(Reply::SourceSaved { text, page }) => {
                self.editor.failed.set(false);
                let start = page.window.start;
                let line = page.window.line;
                let preview_position = self.preview_original_cursor().map(|position| {
                    self.editor.page.borrow().as_ref().map_or(position, |old| {
                        if position >= old.window.end {
                            page.window.end + position - old.window.end
                        } else if position >= old.window.start {
                            page.window.start
                        } else {
                            position
                        }
                    })
                });
                self.preserve_edit_find();
                self.cancel_search();
                if let Some(html) = text.html.as_ref() {
                    self.editor.html.document(html.clone());
                }
                self.text.replace(Some(text));
                self.editor.page.replace(Some(page));
                self.editor.operations.borrow_mut().clear();
                self.editor.inserted.set(0);
                self.editor.dirty.set(false);
                self.editor.view.buffer().set_modified(false);
                self.editor
                    .baseline_lines
                    .set(self.editor.view.buffer().line_count());
                self.show(line, Some(start), false);
                if self.preview_active() {
                    self.show_preview_original(preview_position.unwrap_or(start));
                }
                let pending = self.editor.pending.borrow_mut().take();
                if let Some(action) = pending {
                    self.perform_source_action(action);
                }
            }
            Ok(Reply::SourceConflict) => {
                self.editor.failed.set(true);
                crate::i18n_ui::bind(&self.status, "label", "File changed on disk");
                self.source_guard(true);
            }
            Err(error) => {
                self.editor.pending.borrow_mut().take();
                self.error(&error);
            }
            _ => {}
        }
        self.update_editor();
        self.refresh_edit_find();
    }
    pub(super) fn source_action(&self, action: Action) {
        if self.editor.busy.get() {
            self.error("Wait for the current document operation before closing");
            return;
        }
        if self.editor.guard.borrow().is_some() {
            return;
        }
        if self.editor.dirty.get() {
            let page = matches!(action, Action::Page(_));
            self.editor.pending.replace(Some(action));
            if page {
                self.save_source(false);
            } else {
                self.source_guard(false);
            }
        } else {
            self.perform_source_action(action);
        }
    }
    fn perform_source_action(&self, action: Action) {
        match action {
            Action::Reload(context) => self.reload_now(context),
            Action::HtmlHeading { label, depth } => {
                // A save may insert/remove earlier headings. Resolve the frozen
                // unique label again, rather than reusing an obsolete ordinal.
                let offset = self
                    .text
                    .borrow()
                    .as_ref()
                    .and_then(|t| t.html.as_ref())
                    .and_then(|h| {
                        h.headings
                            .iter()
                            .find(|h| h.label == label && h.depth == depth)
                    })
                    .and_then(|h| h.original);
                if let Some(offset) = offset {
                    let line = self
                        .text
                        .borrow()
                        .as_ref()
                        .map_or(1, |t| t.document.line_at(offset));
                    self.show(line, Some(offset), false);
                    if self.editor.editing.get() {
                        self.prepare_source_edit();
                    }
                } else {
                    self.error(&crate::language::tr(
                        "This heading has no verified Source location.",
                    ));
                }
            }
            Action::Find {
                logical,
                original,
                query,
                case,
            } => self.resume_guarded_find(logical, original, query, case, true),
            Action::Open {
                path,
                line,
                request,
            } => self.open_now(&path, line, request),
            Action::Prepared(text) => self.show_prepared_now(text),
            Action::Clear => self.clear_now(),
            Action::Close => {
                self.clear_now();
                if let Some(close) = self.on_close.borrow().as_ref() {
                    close();
                }
            }
            Action::Quit => {
                if let Some(window) = self.widget.root().and_downcast::<gtk::Window>() {
                    window.close();
                }
            }
            Action::Page(forward) => {
                self.clear_edit_find_anchor();
                if self.preview_active() {
                    self.page_preview(forward);
                    return;
                }
                let editing = self.editor.editing.get();
                let offset = if editing {
                    self.editor.page.borrow().as_ref().map(|p| {
                        if forward {
                            p.window.end
                        } else {
                            p.window.start
                        }
                    })
                } else {
                    self.window
                        .borrow()
                        .as_ref()
                        .map(|p| if forward { p.end } else { p.start })
                };
                if let Some(offset) = offset {
                    self.show(1, Some(offset), !forward);
                    if editing {
                        self.prepare_source_edit();
                        self.update_editor();
                    }
                }
            }
        }
    }
    fn source_guard(&self, conflict: bool) {
        let dialog = adw::AlertDialog::new(None, None);
        for (property, label) in [
            (
                "heading",
                if conflict {
                    "File changed on disk"
                } else {
                    "Save source changes?"
                },
            ),
            (
                "body",
                if conflict {
                    "The file changed on disk. Overwrite it with the current editor content?"
                } else {
                    "The current edit page has unsaved changes."
                },
            ),
        ] {
            crate::i18n_ui::bind(&dialog, property, label);
        }
        let responses = if conflict {
            vec![("cancel", "Cancel"), ("overwrite", "Overwrite")]
        } else {
            vec![
                ("cancel", "Cancel"),
                ("discard", "Discard"),
                ("save", "Save"),
            ]
        };
        for (id, label) in responses {
            dialog.add_response(id, label);
            crate::i18n_ui::response(&dialog, id, label);
        }
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        dialog.set_response_appearance(
            if conflict { "overwrite" } else { "discard" },
            adw::ResponseAppearance::Destructive,
        );
        let weak = self.editor.owner.borrow().clone();
        let generation = self.generation.get();
        dialog.connect_response(None, move |_, response| {
            let Some(source) = weak.upgrade() else { return };
            if source.generation.get() != generation || source.editor.loading.get() {
                return;
            }
            source.editor.guard.borrow_mut().take();
            match response {
                "overwrite" | "save" => source.save_source(response == "overwrite"),
                "discard" => {
                    let action = source.editor.pending.borrow_mut().take();
                    source.reset_editor();
                    if let Some(action) = action {
                        match action {
                            Action::Find {
                                logical,
                                original,
                                query,
                                case,
                            } => source.resume_guarded_find(logical, original, query, case, false),
                            action => source.perform_source_action(action),
                        }
                    }
                }
                _ => {
                    source.editor.pending.borrow_mut().take();
                }
            }
            source.update_editor();
        });
        self.editor.guard.replace(Some(dialog.clone()));
        dialog.present(Some(&self.widget));
    }
    pub fn allow_window_close(&self) -> bool {
        if self.editor.busy.get() {
            self.error("Wait for the current document operation before closing");
            return false;
        }
        if !self.editor.dirty.get() {
            return true;
        }
        self.source_action(Action::Quit);
        false
    }
    pub(super) fn edit_find_input(&self) -> Option<(Page, String)> {
        if !self.editor.editing.get() || self.editor.busy.get() || self.editor.loading.get() {
            return None;
        }
        let page = self.editor.page.borrow().clone()?;
        let buffer = self.editor.view.buffer();
        Some((
            page,
            buffer
                .text(&buffer.start_iter(), &buffer.end_iter(), true)
                .to_string(),
        ))
    }
    pub(super) fn edit_find_busy(&self) -> bool {
        self.editor.busy.get() || self.editor.loading.get()
    }
    pub(super) fn edit_find_select(&self, range: std::ops::Range<usize>) {
        let buffer = self.editor.view.buffer();
        if range.start > range.end || range.end > buffer.char_count() as usize {
            return;
        }
        buffer.select_range(
            &buffer.iter_at_offset(range.start as i32),
            &buffer.iter_at_offset(range.end as i32),
        );
        self.editor.reveal.queue(false);
    }
    pub(super) fn edit_find_jump(&self, range: std::ops::Range<usize>) {
        if self.editor.dirty.get() || self.edit_find_busy() {
            return;
        }
        let position = self.text.borrow().as_ref().and_then(|text| {
            let line = text.document.line_at(range.start);
            let window = text.document.window(line, None, false).ok()?;
            Some((
                line,
                if range.end <= window.end {
                    window.start
                } else {
                    range.start
                },
            ))
        });
        if let Some((line, start)) = position {
            self.show(line, Some(start), false);
            self.prepare_source_edit();
            self.update_editor();
        }
    }
    pub(super) fn ensure_edit_find(&self) {
        if !self.editor.editing.get() {
            self.editor.edit.set_active(true);
        }
    }
    pub(super) fn editor_language(&self, language: Option<&sourceview::Language>) {
        if let Ok(buffer) = self.editor.view.buffer().downcast::<sourceview::Buffer>() {
            buffer.set_language(language);
        }
    }
    pub(super) fn editor_theme(&self) {
        self.preview_theme();
        if let Ok(buffer) = self.editor.view.buffer().downcast::<sourceview::Buffer>() {
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
