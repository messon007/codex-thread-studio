//! Shared native rich-text and code widgets. Parsing stays outside GTK.
use adw::prelude::*;
use gtk::{gio, glib};
use sourceview::prelude::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
pub type OpenLink = Rc<dyn Fn(&str)>;
#[derive(Default)]
pub struct ParagraphState {
    pub(crate) page: Cell<usize>,
    pub(crate) target: RefCell<Option<std::ops::Range<usize>>>,
    pub(crate) continued: Cell<bool>,
    pub(crate) source_only: Cell<bool>,
    pub(crate) view: glib::WeakRef<gtk::TextView>,
    pub(crate) scroll: Rc<crate::text_position::Memory>,
    pub(crate) renderer: RefCell<Option<std::rc::Weak<dyn Fn(usize)>>>,
}
impl ParagraphState {
    #[cfg(debug_assertions)]
    pub fn smoke_view(&self) -> Option<gtk::TextView> {
        self.view.upgrade()
    }
    pub fn select(&self, page: usize) {
        self.source_only.set(false);
        self.target.replace(None);
        self.show(page);
    }
    pub fn focus(&self, page: usize, range: std::ops::Range<usize>) {
        self.source_only.set(false);
        self.target.replace(Some(range));
        self.show(page);
    }
    pub fn source_match(&self) {
        self.target.replace(None);
        self.source_only.set(true);
        self.show(0);
    }
    fn show(&self, page: usize) {
        self.scroll.reset();
        self.page.set(page);
        let renderer = self
            .renderer
            .borrow()
            .as_ref()
            .and_then(std::rc::Weak::upgrade);
        if let Some(renderer) = renderer {
            renderer(page);
        }
    }
    #[cfg(debug_assertions)]
    pub fn target_visible(&self, expected: &str, range: &std::ops::Range<usize>) -> bool {
        if self.target.borrow().as_ref() != Some(range) {
            return false;
        }
        let Some(view) = self.view.upgrade().filter(|view| view.is_mapped()) else {
            return false;
        };
        let buffer = view.buffer();
        let Some((start, end)) = buffer.selection_bounds() else {
            return false;
        };
        let projected = crate::text_flow::quick_selection(view.upcast_ref(), 16 * 1024);
        let text = projected
            .clone()
            .flatten()
            .unwrap_or_else(|| buffer.text(&start, &end, false).to_string());
        let glyph = view.iter_location(&start);
        let visible = view.visible_rect();
        let tail =
            view.iter_location(&buffer.iter_at_offset((end.offset() - 1).max(start.offset())));
        let fits = tail.y() + tail.height() - glyph.y() <= visible.height();
        !text.is_empty()
            && (if self.continued.get() {
                expected.starts_with(text.as_str())
            } else {
                if projected.is_some() {
                    text.contains(expected)
                } else {
                    expected == text.as_str()
                }
            })
            && glyph.height() > 0
            && glyph.y() >= visible.y()
            && glyph.y() + glyph.height() <= visible.y() + visible.height()
            && (!fits || tail.y() + tail.height() <= visible.y() + visible.height())
    }
}

/// Large plain blocks use GTK's scrollable text layout instead of measuring a
/// single unbounded wrapping label inside the conversation list.
pub fn plain(text: &str, monospace: bool) -> gtk::Widget {
    if text.len() <= studio_markdown_native::inline::SCROLLABLE_TEXT_BYTES {
        let label = rich("", Rc::new(|_| {}));
        label.set_text(text);
        label.remove_css_class("studio-markdown");
        if monospace {
            label.add_css_class("monospace");
        }
        return label.upcast();
    }
    let buffer = gtk::TextBuffer::new(None::<&gtk::TextTagTable>);
    buffer.set_enable_undo(false);
    buffer.set_text(text);
    scrollable(&buffer, monospace).0.upcast()
}

fn scrollable(buffer: &gtk::TextBuffer, monospace: bool) -> (gtk::ScrolledWindow, gtk::TextView) {
    let view = gtk::TextView::with_buffer(buffer);
    view.set_editable(false);
    view.set_cursor_visible(false);
    view.set_wrap_mode(gtk::WrapMode::WordChar);
    view.set_monospace(monospace);
    view.set_left_margin(8);
    view.set_right_margin(8);
    view.set_top_margin(8);
    view.set_bottom_margin(8);
    view.add_css_class("studio-content");
    crate::translation_ui::bind(&view);
    if monospace {
        view.add_css_class("studio-code");
    }
    let scroll = gtk::ScrolledWindow::builder()
        .child(&view)
        .hexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .min_content_height(80)
        .max_content_height(320)
        .build();
    (scroll, view)
}

/// GTK consumes worker-prepared Unicode ranges; no Markdown or markup parsing
/// is needed to lay out a long paragraph. The source and anchors stay unchanged.
pub fn formatted_at(
    content: &studio_markdown_native::RichText,
    open: OpenLink,
    reading: Rc<ParagraphState>,
) -> gtk::Widget {
    use studio_markdown_native::inline::Style;
    if !content.source_only && !content.math.is_empty() && content.attributes.is_some() {
        return crate::rich_paragraph::Paragraph::new(content, open, reading).upcast();
    }
    let Some(attributes) = &content.attributes else {
        let label = rich(&content.markup, open);
        crate::inline_math::decorate(&label, content);
        return label.upcast();
    };
    let buffer = gtk::TextBuffer::new(None::<&gtk::TextTagTable>);
    buffer.set_enable_undo(false);
    let line_height = crate::appearance_ui::markdown_line_tag(&buffer, content.source_only);
    let tags = [
        gtk::TextTag::builder()
            .name("studio-bold")
            .weight(700)
            .build(),
        gtk::TextTag::builder()
            .name("studio-italic")
            .style(gtk::pango::Style::Italic)
            .build(),
        gtk::TextTag::builder()
            .name("studio-strike")
            .strikethrough(true)
            .build(),
        gtk::TextTag::builder()
            .name("studio-inline-code")
            .family("monospace")
            .build(),
    ];
    for tag in &tags {
        buffer.tag_table().add(tag);
    }
    crate::appearance_ui::watch_inline_tag(&tags[3]);
    let link = gtk::TextTag::builder()
        .name("studio-link")
        .underline(gtk::pango::Underline::Single)
        .build();
    buffer.tag_table().add(&link);
    // Strong emphasis stays stronger than the configured inline-code weight.
    tags[0].set_priority(buffer.tag_table().size() - 1);
    let (scroll, view) = scrollable(&buffer, content.source_only);
    reading.scroll.attach(&view, &scroll);
    if !content.source_only {
        view.add_css_class("studio-markdown");
    }
    reading.view.set(Some(&view));
    // TextView initially reports only its first laid-out lines. Keep a useful
    // reading viewport instead of briefly showing a three-line clipped strip.
    let compact_cr = attributes.carriage_returns
        && !content.source_only
        && attributes.text.len() <= studio_markdown_native::inline::SCROLLABLE_TEXT_BYTES
        && content.markup.len() <= studio_markdown_native::inline::SCROLLABLE_TEXT_BYTES;
    scroll.set_min_content_height(if compact_cr { 80 } else { 320 });
    scroll.set_overlay_scrolling(false);
    scroll.add_css_class("frame");
    view.set_widget_name("studio-rich-text");
    view.set_cursor_visible(true);
    view.set_tooltip_text(Some(if content.source_only {
        "Read only source text. Select text to copy."
    } else {
        "Read only. Select text to copy; Ctrl+click or Ctrl+Enter opens a link."
    }));
    let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
    container.add_css_class("card");
    let header = crate::adaptive::toolbar(4);
    header.set_margin_start(8);
    header.set_margin_end(4);
    header.set_margin_top(4);
    header.set_margin_bottom(4);
    let label = gtk::Label::builder()
        .xalign(0.0)
        .hexpand(true)
        .wrap(true)
        .width_chars(1)
        .build();
    label.add_css_class("caption");
    if content.source_only {
        header.set_tooltip_text(Some(
            "This section is shown as source because it exceeds the formatting limit.",
        ));
    }
    header.append(&label);
    container.append(&header);
    container.append(&scroll);
    let page = reading.clone();
    let current = page.clone();
    let source = attributes.clone();
    let text = buffer.clone();
    let offset = scroll.vadjustment();
    let weak = view.downgrade();
    let render: Rc<dyn Fn(usize)> = Rc::new(move |index| {
        let Some(view) = weak.upgrade().filter(|v| current.scroll.owns(v)) else {
            return;
        };
        let position = current.scroll.freeze();
        let index = index.min(source.pages.len().saturating_sub(1));
        let Some(part) = source.pages.get(index) else {
            return;
        };
        current.page.set(index);
        text.set_text(&source.text[part.start..part.end]);
        text.apply_tag(&line_height, &text.start_iter(), &text.end_iter());
        for id in &part.spans {
            let span = &source.spans[*id];
            let tag = match span.style {
                Style::Bold => 0,
                Style::Italic => 1,
                Style::Strike => 2,
                Style::Code => 3,
            };
            text.apply_tag(
                &tags[tag],
                &text.iter_at_offset(span.start.saturating_sub(part.character) as i32),
                &text.iter_at_offset(
                    span.end
                        .min(part.character + part.characters)
                        .saturating_sub(part.character) as i32,
                ),
            );
        }
        for id in &part.links {
            let span = &source.links[*id];
            text.apply_tag(
                &link,
                &text.iter_at_offset(span.start.saturating_sub(part.character) as i32),
                &text.iter_at_offset(
                    span.end
                        .min(part.character + part.characters)
                        .saturating_sub(part.character) as i32,
                ),
            );
        }
        text.place_cursor(&text.start_iter());
        offset.set_value(0.0);
        let mut continued = false;
        if let Some(target) = current.target.borrow().as_ref().filter(|range| {
            range.start < range.end
                && range.start < part.character + part.characters
                && range.end > part.character
        }) {
            let start = text.iter_at_offset(target.start.saturating_sub(part.character) as i32);
            let end = text.iter_at_offset(
                target
                    .end
                    .min(part.character + part.characters)
                    .saturating_sub(part.character) as i32,
            );
            text.select_range(&start, &end);
            continued = target.end > part.character + part.characters;
            if position.is_none() {
                view.scroll_to_mark(&text.get_insert(), 0.1, true, 0.0, 0.35);
            }
        }
        current.continued.set(continued);
        label.set_text(&format!(
            "{}Page {}/{}{}",
            part.lines.map_or_else(String::new, |(start, end)| format!(
                "Source · lines {start}–{end} · "
            )),
            index + 1,
            source.pages.len(),
            if current.source_only.get() {
                " · match is in Markdown source"
            } else if continued {
                " · match continues on next page"
            } else {
                ""
            }
        ));
        current.scroll.settle(&view, position);
    });
    let actions = gio::SimpleActionGroup::new();
    for (name, icon, tip, step) in [
        ("first-page", "go-first-symbolic", "First text page", 0),
        (
            "previous-page",
            "go-previous-symbolic",
            "Previous text page",
            1,
        ),
        ("next-page", "go-next-symbolic", "Next text page", 2),
        ("last-page", "go-last-symbolic", "Last text page", 3),
    ] {
        let action = gio::SimpleAction::new(name, None);
        let display = render.clone();
        let current = page.clone();
        let weak = view.downgrade();
        let count = attributes.pages.len();
        action.connect_activate(move |_, _| {
            if weak.upgrade().is_none_or(|v| !current.scroll.owns(&v)) {
                return;
            }
            current.source_only.set(false);
            current.target.replace(None);
            current.scroll.reset();
            display(match step {
                0 => 0,
                1 => current.page.get().saturating_sub(1),
                2 => (current.page.get() + 1).min(count - 1),
                _ => count - 1,
            })
        });
        actions.add_action(&action);
        let button = gtk::Button::builder()
            .icon_name(icon)
            .tooltip_text(tip)
            .action_name(format!("rich.{name}"))
            .build();
        button.add_css_class("flat");
        header.append(&button);
        let action = action.downgrade();
        let current = page.clone();
        buffer.connect_changed(move |_| {
            if let Some(action) = action.upgrade() {
                action.set_enabled(if step < 2 {
                    current.page.get() > 0
                } else {
                    current.page.get() + 1 < count
                });
            }
        });
    }
    reading.renderer.replace(Some(Rc::downgrade(&render)));
    render(reading.page.get().min(attributes.pages.len() - 1));
    let ranges = attributes.clone();
    let current = page.clone();
    crate::text_links::bind(
        &view,
        &actions,
        !ranges.links.is_empty(),
        Rc::new(move |view, offset| {
            if !current.scroll.owns(view) {
                return None;
            }
            let offset = ranges.pages.get(current.page.get())?.character + offset;
            ranges
                .links
                .iter()
                .find(|l| l.start <= offset && offset < l.end)
                .map(|l| l.destination.clone())
        }),
        open,
    );
    container.insert_action_group("rich", Some(&actions));
    let menu = gio::Menu::new();
    menu.append(Some("Open link at cursor"), Some("rich.open-link"));
    view.set_extra_menu(Some(&menu));
    crate::translation_ui::bind(&view);
    container.upcast()
}

pub fn heading_at(
    content: &studio_markdown_native::RichText,
    level: u8,
    open: OpenLink,
    reading: Rc<ParagraphState>,
) -> gtk::Widget {
    let widget = formatted_at(content, open, reading);
    let class = match level {
        1 => "title-1",
        2 => "title-2",
        _ => "title-3",
    };
    fn apply(widget: &gtk::Widget, class: &str) {
        if widget.is::<gtk::Label>() || widget.is::<gtk::TextView>() {
            if widget.has_css_class("studio-content") {
                widget.add_css_class(class);
            }
        }
        let mut child = widget.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            apply(&widget, class);
        }
    }
    if !crate::rich_paragraph::when_ready(&widget, Rc::new(move |child| apply(child, class))) {
        apply(&widget, class);
    }
    widget
}

/// Shared selection extraction for labels and scrollable rich paragraphs.
pub fn selection(widget: &gtk::Widget, limit: usize) -> Option<String> {
    if let Some(text) = crate::text_flow::quick_selection(widget, limit) {
        return text.filter(|t| !t.trim().is_empty());
    }
    let text = if let Some(label) = widget.downcast_ref::<gtk::Label>() {
        let (a, b) = label.selection_bounds()?;
        if (a - b).unsigned_abs() as usize > limit {
            return None;
        }
        label
            .text()
            .chars()
            .skip(a.min(b) as usize)
            .take((a - b).unsigned_abs() as usize)
            .collect::<String>()
    } else if let Some(view) = widget.downcast_ref::<gtk::TextView>() {
        let buffer = view.buffer();
        let (a, b) = buffer.selection_bounds()?;
        if (a.offset() - b.offset()).unsigned_abs() as usize > limit {
            return None;
        }
        buffer.text(&a, &b, false).to_string()
    } else {
        let mut child = widget.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            if let Some(text) = selection(&widget, limit) {
                return Some(text);
            }
        }
        return None;
    };
    (!text.trim().is_empty() && text.len() <= limit).then_some(text)
}

pub fn rich(markup: &str, open: OpenLink) -> gtk::Label {
    let label = gtk::Label::builder()
        .xalign(0.0)
        .selectable(true)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .width_chars(1)
        .build();
    label.add_css_class("studio-content");
    label.add_css_class("studio-markdown");
    label.set_markup(markup);
    crate::appearance_ui::watch_inline_label(&label);
    crate::translation_ui::bind(&label);
    label.connect_activate_link(move |_, uri| {
        open(uri);
        glib::Propagation::Stop
    });
    label
}
/// Shared code controls. Owners supply the immutable source and lifetime guard;
/// labels remain literal and long language identifiers cannot widen the pane.
pub fn code_header(language: &str, copy: impl Fn(&gtk::Button) + 'static) -> gtk::Box {
    let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
    widget.set_widget_name("studio-code-header");
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    header.set_margin_top(6);
    header.set_margin_bottom(6);
    header.set_margin_start(12);
    header.set_margin_end(6);
    let name = gtk::Label::builder()
        .xalign(0.0)
        .hexpand(true)
        .ellipsize(gtk::pango::EllipsizeMode::Middle)
        .single_line_mode(true)
        .build();
    if language.is_empty() {
        crate::i18n_ui::bind(&name, "label", "Code");
    } else {
        name.set_text(language);
    }
    name.add_css_class("caption");
    header.append(&name);
    let button = gtk::Button::builder()
        .icon_name("edit-copy-symbolic")
        .focus_on_click(false)
        .build();
    button.set_widget_name("studio-code-copy");
    crate::i18n_ui::bind(&button, "tooltip-text", "Copy code");
    button.add_css_class("flat");
    button.connect_clicked(copy);
    header.append(&button);
    widget.append(&header);
    widget.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    widget
}
pub struct Styles {
    buffers: RefCell<Vec<glib::WeakRef<sourceview::Buffer>>>,
    handler: RefCell<Option<glib::SignalHandlerId>>,
}
impl Styles {
    pub fn new() -> Rc<Self> {
        let styles = Rc::new(Self {
            buffers: RefCell::new(Vec::new()),
            handler: RefCell::new(None),
        });
        let weak = Rc::downgrade(&styles);
        let handler = adw::StyleManager::default().connect_dark_notify(move |_| {
            if let Some(styles) = weak.upgrade() {
                styles.update();
            }
        });
        styles.handler.replace(Some(handler));
        styles
    }
    fn update(&self) {
        let scheme = sourceview::StyleSchemeManager::default().scheme(
            if adw::StyleManager::default().is_dark() {
                "Adwaita-dark"
            } else {
                "Adwaita"
            },
        );
        self.buffers.borrow_mut().retain(|weak| {
            if let Some(buffer) = weak.upgrade() {
                buffer.set_style_scheme(scheme.as_ref());
                true
            } else {
                false
            }
        });
    }
    pub fn watch_buffer(&self, buffer: &sourceview::Buffer) {
        self.buffers.borrow_mut().push(buffer.downgrade());
        self.update();
    }
    pub fn code(&self, language: &str, text: &str) -> gtk::Box {
        self.build_code(language, text, true)
    }
    fn build_code(&self, language: &str, text: &str, markdown: bool) -> gtk::Box {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
        widget.add_css_class("card");
        let source = text.to_owned();
        widget.append(&code_header(language, move |button| {
            button.clipboard().set_text(&source);
        }));
        let buffer = sourceview::Buffer::new(None::<&gtk::TextTagTable>);
        crate::source_surface::bind(buffer.upcast_ref());
        crate::source_surface::set_text(buffer.upcast_ref(), text);
        if markdown {
            let tag = crate::appearance_ui::markdown_line_tag(buffer.upcast_ref(), true);
            buffer.apply_tag(&tag, &buffer.start_iter(), &buffer.end_iter());
        }
        buffer.set_highlight_syntax(studio_markdown_native::text::highlight_safe(text));
        let manager = sourceview::LanguageManager::default();
        let syntax_id = match language {
            "bash" | "shell" => "sh",
            "js" | "jsx" => "js",
            "ts" | "tsx" => "typescript",
            "py" => "python",
            "rs" => "rust",
            "yml" => "yaml",
            "c++" => "cpp",
            "c#" | "cs" => "c-sharp",
            value => value,
        };
        let syntax = manager
            .language(syntax_id)
            .or_else(|| manager.guess_language(Some(format!("code.{language}")), None));
        buffer.set_language(syntax.as_ref());
        self.watch_buffer(&buffer);
        let view = sourceview::View::with_buffer(&buffer);
        view.add_css_class("studio-code");
        view.set_editable(false);
        view.set_monospace(true);
        view.set_show_line_numbers(true);
        view.set_wrap_mode(gtk::WrapMode::WordChar);
        view.set_left_margin(12);
        view.set_right_margin(12);
        view.set_top_margin(8);
        view.set_bottom_margin(8);
        view.set_tab_width(4);
        widget.append(
            &gtk::ScrolledWindow::builder()
                .child(&view)
                .propagate_natural_height(true)
                .max_content_height(340)
                .hscrollbar_policy(gtk::PolicyType::Never)
                .build(),
        );
        widget
    }
    /// Source panes use the available height; inline code blocks stay bounded.
    pub fn full_code(&self, language: &str, text: &str) -> gtk::Box {
        let widget = self.build_code(language, text, false);
        widget.set_vexpand(true);
        if let Some(scroll) = widget.last_child().and_downcast::<gtk::ScrolledWindow>() {
            scroll.set_propagate_natural_height(false);
            scroll.set_max_content_height(-1);
            scroll.set_min_content_height(120);
            scroll.set_vexpand(true);
        }
        widget
    }
}
impl Drop for Styles {
    fn drop(&mut self) {
        if let Some(handler) = self.handler.get_mut().take() {
            adw::StyleManager::default().disconnect(handler);
        }
    }
}
