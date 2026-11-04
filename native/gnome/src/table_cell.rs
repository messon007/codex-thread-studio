//! Bounded table labels and an explicit, native full-cell reading window.
//! Excerpts, styles and Unicode page offsets are prepared on Rust workers.
use crate::content_widgets::{self, OpenLink, ParagraphState};
use adw::prelude::*;
use gtk::{gio, glib};
use std::{cell::RefCell, rc::Rc};
use studio_markdown_native::Cell;

pub type ConfigureText = Rc<dyn Fn(&gtk::Widget)>;
pub type RenderImages = Rc<dyn Fn(&gtk::Box, &studio_markdown_native::RichText, bool)>;

#[path = "table_cell_find.rs"]
mod find;
#[path = "table_math_pages.rs"]
mod math_pages;

pub fn widget(
    cell: &Cell,
    open: OpenLink,
    configure: Option<ConfigureText>,
    images: Option<RenderImages>,
) -> (gtk::Box, gtk::Label) {
    let container = gtk::Box::new(gtk::Orientation::Vertical, 4);
    container.set_margin_start(10);
    container.set_margin_end(10);
    container.set_margin_top(7);
    container.set_margin_bottom(7);
    let label = content_widgets::rich("", open.clone());
    label.set_width_chars(1);
    label.set_hexpand(true);
    label.set_tooltip_text(Some(&format!("Source line {}", cell.source.line)));
    crate::i18n_ui::bind_format(
        &label,
        "tooltip-text",
        "Source line {line}",
        &[("line", &cell.source.line.to_string())],
    );
    if let Some(preview) = &cell.preview {
        // The model's trailing ellipsis is a display hint, not source text.
        // Keep it out of selections; Pango paints truncation and the full-cell
        // action makes the bounded excerpt explicit.
        label.set_text(preview.strip_suffix('…').unwrap_or(preview));
        crate::inline_math::decorate_excerpt(&label, cell);
        label.set_widget_name("studio-cell-excerpt");
        label.set_lines(5);
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    } else {
        label.set_markup(&cell.content.markup);
        crate::inline_math::decorate(&label, &cell.content);
    }
    if let Some(configure) = &configure {
        configure(label.upcast_ref());
    }
    container.append(&label);
    if !cell.content.images.is_empty() {
        if let Some(images) = &images {
            let previews = gtk::Box::new(gtk::Orientation::Vertical, 6);
            previews.set_widget_name("studio-cell-images");
            images(&previews, &cell.content, true);
            container.append(&previews);
        }
    }
    if cell.preview.is_some() || !cell.content.images.is_empty() {
        let button = gtk::Button::builder()
            .label("Read full cell")
            .halign(gtk::Align::Start)
            .tooltip_text("Read the complete cell with formatting and text selection")
            .build();
        button.add_css_class("flat");
        button.add_css_class("caption");
        button.set_widget_name("studio-cell-open");
        crate::i18n_ui::bind(&button, "label", "Read full cell");
        crate::i18n_ui::bind(
            &button,
            "tooltip-text",
            "Read the complete cell with formatting and text selection",
        );
        let reader = ReadCell::new(cell, container.upcast_ref(), open, configure, images);
        button.connect_clicked(move |button| {
            if button.is_mapped() {
                reader.present();
            }
        });
        container.prepend(&button);
    }
    (container, label)
}

/// One native reading window per source cell. Its owner controls its lifetime.
#[derive(Clone)]
pub struct ReadCell {
    present: Rc<dyn Fn()>,
    close: Rc<dyn Fn()>,
}
impl ReadCell {
    pub fn new(
        cell: &Cell,
        owner: &gtk::Widget,
        open: OpenLink,
        configure: Option<ConfigureText>,
        images: Option<RenderImages>,
    ) -> Self {
        let content = cell.content.clone();
        let line = cell.source.line;
        let window = Rc::new(RefCell::new(glib::WeakRef::<adw::Window>::new()));
        let closed = window.clone();
        let close: Rc<dyn Fn()> = Rc::new(move || {
            if let Some(window) = closed.borrow().upgrade() {
                window.close();
            }
        });
        let cleanup = close.clone();
        owner.connect_unmap(move |_| cleanup());
        let parent = owner.downgrade();
        let present = Rc::new(move || {
            let Some(parent) = parent.upgrade().filter(|w| w.is_mapped()) else {
                return;
            };
            if let Some(existing) = window.borrow().upgrade().filter(|w| w.is_visible()) {
                existing.present();
                return;
            }
            let root = adw::ToolbarView::new();
            let header = adw::HeaderBar::new();
            let title = adw::WindowTitle::new("Table cell", &format!("Source line {line}"));
            crate::i18n_ui::bind(&title, "title", "Table cell");
            crate::i18n_ui::bind_format(
                &title,
                "subtitle",
                "Source line {line}",
                &[("line", &line.to_string())],
            );
            header.set_title_widget(Some(&title));
            let copy = gtk::Button::builder()
                .icon_name("edit-copy-symbolic")
                .tooltip_text("Copy complete cell text")
                .build();
            crate::i18n_ui::bind(&copy, "tooltip-text", "Copy complete cell text");
            let text = content.clone();
            copy.connect_clicked(move |button| button.clipboard().set_text(&text.plain));
            header.pack_end(&copy);
            root.add_top_bar(&header);
            let reading = Rc::new(ParagraphState::default());
            let paged_math = content
                .attributes
                .as_ref()
                .is_some_and(|a| !a.carriage_returns)
                && !content.source_only
                && !content.math.is_empty();
            let formatted = if paged_math {
                math_pages::widget(
                    content.clone(),
                    open.clone(),
                    configure.clone(),
                    reading.clone(),
                )
            } else {
                let formatted =
                    content_widgets::formatted_at(&content, open.clone(), reading.clone());
                if let Some(configure) = &configure {
                    configure(&formatted);
                }
                formatted
            };
            let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
            body.append(&formatted);
            if !content.images.is_empty() {
                if let Some(images) = &images {
                    let previews = gtk::Box::new(gtk::Orientation::Vertical, 6);
                    previews.set_widget_name("studio-cell-images");
                    images(&previews, &content, false);
                    body.append(&previews);
                }
            }
            // Math cells use bounded formatted label pages. Other large cells
            // retain the existing 8-KiB styled TextView pages.
            body.set_margin_start(16);
            body.set_margin_end(16);
            body.set_margin_top(12);
            body.set_margin_bottom(12);
            if content.attributes.is_some() && content.images.is_empty() {
                body.set_vexpand(true);
                formatted.set_vexpand(true);
                let mut child = formatted.first_child();
                while let Some(widget) = child {
                    child = widget.next_sibling();
                    if let Some(scroll) = widget.downcast_ref::<gtk::ScrolledWindow>() {
                        scroll.set_min_content_height(80);
                        scroll.set_max_content_height(-1);
                        scroll.set_propagate_natural_height(false);
                        scroll.set_vexpand(true);
                    }
                }
                root.set_content(Some(&body));
            } else {
                let scroll = gtk::ScrolledWindow::builder()
                    .child(&body)
                    .hscrollbar_policy(gtk::PolicyType::Never)
                    .build();
                scroll.set_widget_name("studio-cell-content-scroll");
                root.set_content(Some(&scroll));
            }
            let dialog = adw::Window::builder()
                .title("Table cell")
                .content(&root)
                .default_width(740)
                .default_height(540)
                .modal(true)
                .build();
            if let Some(parent) = parent.root().and_downcast::<gtk::Window>() {
                dialog.set_transient_for(Some(&parent));
                dialog.set_destroy_with_parent(true);
            }
            crate::i18n_ui::bind(&dialog, "title", "Table cell");
            dialog.set_widget_name("studio-selection-window");
            find::Find::install(
                content.clone(),
                reading,
                &formatted,
                &dialog,
                &root,
                &header,
            );
            window.borrow().set(Some(&dialog));
            dialog.present();
        });
        Self { present, close }
    }
    pub fn present(&self) {
        (self.present)();
    }
    pub fn close(&self) {
        (self.close)();
    }
}

/// Native column-header menus expose formatting/images without duplicating the
/// plain column titles or replacing GTK's resizing/navigation behavior.
pub fn header(
    column: &gtk::ColumnViewColumn,
    view: &gtk::ColumnView,
    cell: &Cell,
    namespace: &str,
    open: OpenLink,
    configure: Option<ConfigureText>,
    images: Option<RenderImages>,
) {
    let reader = ReadCell::new(cell, view.upcast_ref(), open, configure, images);
    let group = gio::SimpleActionGroup::new();
    let action = gio::SimpleAction::new("read", None);
    let owner = column.downgrade();
    let current = reader.clone();
    action.connect_activate(move |_, _| {
        if owner
            .upgrade()
            .is_some_and(|c| c.is_visible() && c.column_view().is_some())
        {
            current.present();
        }
    });
    group.add_action(&action);
    view.insert_action_group(namespace, Some(&group));
    let menu = gio::Menu::new();
    menu.append(
        Some("Read column header"),
        Some(&format!("{namespace}.read")),
    );
    crate::i18n_ui::menu_label(&menu, 0, "Read column header");
    column.set_header_menu(Some(&menu));
    let cleanup = reader.clone();
    let owner = view.downgrade();
    let namespace = namespace.to_owned();
    column.connect_column_view_notify(move |c| {
        if c.column_view().is_none() {
            cleanup.close();
            if let Some(view) = owner.upgrade() {
                view.insert_action_group(&namespace, None::<&gio::SimpleActionGroup>);
            }
        }
    });
    column.connect_visible_notify(move |c| {
        if !c.is_visible() {
            reader.close();
        }
    });
}

#[cfg(debug_assertions)]
pub(crate) async fn smoke_find(
    window: &gtk::Window,
    query: &str,
    expected: &str,
) -> Result<(), String> {
    find::smoke(window, query, expected).await
}
