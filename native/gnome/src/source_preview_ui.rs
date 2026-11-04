//! GTK presentation of immutable, worker-formatted ordinary files. Source and
//! editor buffers retain their own contents, positions and undo history.
use super::*;
use studio_markdown_native::{structured::Language, SourceWindow};

pub(super) struct Preview {
    pub button: gtk::ToggleButton,
    pub view: sourceview::View,
    line: Rc<Cell<usize>>,
    reveal: Rc<crate::text_reveal::Reveal>,
    pub scroll: gtk::ScrolledWindow,
    pub initial: Cell<bool>,
    pub window: RefCell<Option<SourceWindow>>,
}
impl Preview {
    pub fn new(modes: &gtk::Box, source: &gtk::ToggleButton) -> Self {
        let button = gtk::ToggleButton::new();
        crate::i18n_ui::bind(&button, "label", "Preview");
        button.set_group(Some(source));
        button.set_visible(false);
        modes.append(&button);
        let (view, line) = crate::reader::text_view(false);
        view.set_show_line_numbers(false);
        view.buffer()
            .downcast::<sourceview::Buffer>()
            .unwrap()
            .set_highlight_matching_brackets(false);
        crate::i18n_ui::accessible_label(&view, "Preview");
        let scroll = gtk::ScrolledWindow::builder()
            .child(&view)
            .hexpand(true)
            .vexpand(true)
            .build();
        let reveal = crate::text_reveal::Reveal::new(&view);
        Self {
            button,
            view,
            line,
            reveal,
            scroll,
            initial: Cell::new(false),
            window: RefCell::new(None),
        }
    }
}
fn byte_at(text: &str, scalar: i32) -> usize {
    text.char_indices()
        .nth(scalar.max(0) as usize)
        .map_or(text.len(), |(i, _)| i)
}
impl Source {
    pub(super) fn preview_active(&self) -> bool {
        self.editor.preview.button.is_active()
            && self
                .text
                .borrow()
                .as_ref()
                .is_some_and(|t| t.preview.is_some() || t.html.is_some())
    }
    pub(super) fn html_active(&self) -> bool {
        self.editor.preview.button.is_active()
            && self
                .text
                .borrow()
                .as_ref()
                .is_some_and(|t| t.html.is_some())
    }
    pub(super) fn preview_original_cursor(&self) -> Option<usize> {
        if self.html_active() {
            return self.editor.html.original();
        }
        let text = self.text.borrow();
        let preview = text.as_ref()?.preview.as_ref()?;
        let window = self.editor.preview.window.borrow();
        let window = window.as_ref()?;
        let buffer = self.editor.preview.view.buffer();
        let byte = byte_at(
            &window.text,
            buffer.iter_at_mark(&buffer.get_insert()).offset(),
        );
        Some(preview.original_at(window.start + byte))
    }
    pub(super) fn show_preview_from_source(&self) {
        let original = self.window.borrow().as_ref().map_or(0, |w| {
            let buffer = self.view.buffer();
            w.start + byte_at(&w.text, buffer.iter_at_mark(&buffer.get_insert()).offset())
        });
        self.show_preview_original(original);
    }
    pub(super) fn show_preview_original(&self, original: usize) {
        let html = self.text.borrow().as_ref().and_then(|t| t.html.clone());
        if let Some(html) = html {
            self.editor.stack.set_visible_child_name("html");
            self.editor.html.open(html, original);
            self.previous.set_sensitive(false);
            self.next.set_sensitive(false);
            crate::i18n_ui::bind(&self.status, "label", "HTML · Preview");
            self.update_comment();
            return;
        }
        let display = self
            .text
            .borrow()
            .as_ref()
            .and_then(|t| t.preview.as_ref())
            .map(|p| p.display_at(original));
        if let Some(display) = display {
            self.show_preview(display, false, None);
        }
    }
    fn show_preview(&self, at: usize, previous: bool, selected: Option<std::ops::Range<usize>>) {
        let preview = self.text.borrow().as_ref().and_then(|t| t.preview.clone());
        let Some(preview) = preview else {
            return;
        };
        let at = at.min(preview.document.bytes());
        let result = preview
            .document
            .window(preview.document.line_at(at), Some(at), previous);
        let window = match result {
            Ok(window) => window,
            Err(error) => {
                self.error(&error);
                return;
            }
        };
        let view = &self.editor.preview.view;
        let buffer = view.buffer();
        crate::source_surface::set_text(&buffer, &window.text);
        if let Ok(buffer) = buffer.clone().downcast::<sourceview::Buffer>() {
            let language = match preview.format.language {
                Language::Json => {
                    if preview.format.label == "JSON" {
                        "json"
                    } else {
                        "js"
                    }
                }
                Language::Xml => "xml",
                Language::Yaml => "yaml",
                Language::Toml => "toml",
                Language::Config => {
                    if preview.format.label == "ENV" {
                        "sh"
                    } else {
                        "ini"
                    }
                }
            };
            buffer.set_language(
                sourceview::LanguageManager::default()
                    .language(language)
                    .as_ref(),
            );
        }
        let start = selected.is_none();
        if let Some(range) = selected.filter(|r| r.start >= window.start && r.end <= window.end) {
            let start = window.text[..range.start - window.start].chars().count() as i32;
            let end = window.text[..range.end - window.start].chars().count() as i32;
            buffer.select_range(&buffer.iter_at_offset(start), &buffer.iter_at_offset(end));
        } else {
            buffer.place_cursor(&buffer.start_iter());
        }
        self.editor.stack.set_visible_child_name("preview");
        self.editor.preview.line.set(window.line);
        self.editor.preview.reveal.queue(start);
        self.previous.set_sensitive(window.start > 0);
        self.next
            .set_sensitive(window.end < preview.document.bytes());
        crate::i18n_ui::bind_format(
            &self.status,
            "label",
            if preview.formatted {
                "{format} · Formatted · Preview lines {start}–{end}"
            } else {
                "{format} · Preview lines {start}–{end}"
            },
            &[
                ("format", preview.format.label),
                ("start", &window.line.to_string()),
                ("end", &window.end_line.to_string()),
            ],
        );
        self.editor.preview.window.replace(Some(window));
        self.update_comment();
    }
    pub(super) fn page_preview(&self, forward: bool) {
        let at =
            self.editor
                .preview
                .window
                .borrow()
                .as_ref()
                .map(|w| if forward { w.end } else { w.start });
        if let Some(at) = at {
            self.show_preview(at, !forward, None);
        }
    }
    pub(super) fn preview_find_select(&self, range: std::ops::Range<usize>) {
        let at = self
            .text
            .borrow()
            .as_ref()
            .and_then(|t| t.preview.as_ref())
            .map(|p| {
                p.document
                    .window(
                        p.document.line_at(range.start).saturating_sub(100).max(1),
                        None,
                        false,
                    )
                    .ok()
                    .filter(|w| range.end <= w.end)
                    .map_or(range.start, |w| w.start)
            });
        if let Some(at) = at {
            self.show_preview(at, false, Some(range));
        }
    }
    pub(super) fn preview_selected(&self) -> Option<(String, usize, usize)> {
        let text = self.text.borrow();
        let text = text.as_ref()?;
        if text.file.is_none() || text.binary || text.truncated || text.diff.is_some() {
            return None;
        }
        let preview = text.preview.as_ref()?;
        let window = self.editor.preview.window.borrow();
        let window = window.as_ref()?;
        let buffer = self.editor.preview.view.buffer();
        let (a, b) = buffer.selection_bounds()?;
        let selected = window.start + byte_at(&window.text, a.offset())
            ..window.start + byte_at(&window.text, b.offset());
        let original = preview.original_range(selected)?;
        let quote = preview.original().as_str().get(original.clone())?;
        if quote.trim().is_empty() || quote.len() > 16384 {
            return None;
        }
        Some((
            quote.into(),
            preview.original().line_at(original.start),
            preview.original().line_at(original.end - 1),
        ))
    }
    pub(super) fn preview_theme(&self) {
        self.editor.html.theme();
        if let Ok(buffer) = self
            .editor
            .preview
            .view
            .buffer()
            .downcast::<sourceview::Buffer>()
        {
            buffer.set_style_scheme(
                crate::preview_scheme::get(adw::StyleManager::default().is_dark()).as_ref(),
            );
        }
    }
}
