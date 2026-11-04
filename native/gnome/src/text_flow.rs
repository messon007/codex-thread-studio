//! Native text-buffer adapter for ordinary bounded Markdown Preview.
//! Projection, selection extraction and SVG work stay on Rust workers.
use crate::content_widgets::OpenLink;
use adw::prelude::*;
use gtk::{gdk, gio, glib};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};
use studio_markdown_native::{
    flow::{self, ObjectKind, Style},
    syntax,
};

#[derive(Clone)]
pub struct SelectionTables {
    pub current: Rc<dyn Fn() -> bool>,
    pub resolve: Rc<
        dyn Fn(
            Vec<usize>,
            usize,
        ) -> futures_util::future::LocalBoxFuture<
            'static,
            Result<Vec<flow::TableText>, String>,
        >,
    >,
    pub cancel: gio::Cancellable,
    pub prefix: Option<
        Rc<
            dyn Fn(
                usize,
                usize,
            ) -> futures_util::future::LocalBoxFuture<
                'static,
                Result<flow::TablePrefix, String>,
            >,
        >,
    >,
}
fn selection_permit() -> Result<Arc<tokio::sync::OwnedSemaphorePermit>, String> {
    static LIMIT: std::sync::OnceLock<Arc<tokio::sync::Semaphore>> = std::sync::OnceLock::new();
    LIMIT
        .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(2)))
        .clone()
        .try_acquire_owned()
        .map(Arc::new)
        .map_err(|_| "Selection workers are busy; try again".into())
}
async fn selected_text(
    source: Arc<flow::Projection>,
    range: std::ops::Range<usize>,
    limit: usize,
    tables: Option<SelectionTables>,
) -> Result<flow::Selection, String> {
    let permit = selection_permit()?;
    enum Step {
        Ready(flow::Selection),
        Tables(flow::SelectionRequest),
    }
    let (copy, selected, lease, resolve) = (
        source.clone(),
        range.clone(),
        permit.clone(),
        tables.is_some(),
    );
    let step = gio::spawn_blocking(move || {
        let _permit = lease;
        if validate_selection_scope(&copy, &selected).is_err() {
            if !resolve {
                return Err(INDEXED_SELECTION.into());
            }
            copy.selection_request(selected, limit).map(Step::Tables)
        } else {
            copy.select(selected, limit).map(Step::Ready)
        }
    })
    .await
    .map_err(|_| "Selection worker stopped")??;
    let Step::Tables(request) = step else {
        let Step::Ready(value) = step else {
            unreachable!()
        };
        return Ok(value);
    };
    let tables = tables.unwrap();
    if tables.cancel.is_cancelled() || !(tables.current)() {
        return Err("Selection document snapshot changed".into());
    }
    let resolved = gio::CancellableFuture::new(
        (tables.resolve)(request.tables, request.remaining),
        tables.cancel.clone(),
    )
    .await
    .map_err(|_| "Selection document snapshot changed")??;
    if tables.cancel.is_cancelled() || !(tables.current)() {
        return Err("Selection document snapshot changed".into());
    }
    let lease = permit.clone();
    let result = gio::spawn_blocking(move || {
        let _permit = lease;
        source.select_resolved(range, limit, &resolved)
    })
    .await
    .map_err(|_| "Selection worker stopped")?;
    if tables.cancel.is_cancelled() || !(tables.current)() {
        return Err("Selection document snapshot changed".into());
    }
    result
}
fn view_scope(view: &gtk::TextView) -> Option<gio::Cancellable> {
    view.buffer()
        .downcast_ref::<primary_selection::Buffer>()?
        .scope()
}
async fn scoped_text(
    source: Arc<flow::Projection>,
    range: std::ops::Range<usize>,
    limit: usize,
    tables: Option<SelectionTables>,
    scope: gio::Cancellable,
) -> Result<flow::Selection, String> {
    gio::CancellableFuture::new(selected_text(source, range, limit, tables), scope)
        .await
        .map_err(|_| "Selection view is no longer visible".to_owned())?
}
fn bind_read_busy(view: &gtk::TextView, copying: Rc<Cell<bool>>, commenting: Rc<Cell<bool>>) {
    let reset: Rc<dyn Fn()> = Rc::new(move || {
        copying.set(false);
        commenting.set(false);
    });
    let changed = reset.clone();
    view.buffer().connect_changed(move |_| changed());
    let hidden = reset.clone();
    view.connect_unmap(move |_| hidden());
    let buffer = view.buffer().downgrade();
    view.connect_buffer_notify(move |view| {
        if buffer.upgrade().is_none_or(|b| b != view.buffer()) {
            reset();
        }
    });
}
async fn selected_prefix(
    source: Arc<flow::Projection>,
    range: std::ops::Range<usize>,
    tables: Option<SelectionTables>,
) -> Result<flow::PrefixResult, String> {
    let permit = selection_permit()?;
    let lease = permit.clone();
    let mut prefix = gio::spawn_blocking(move || {
        let _permit = lease;
        flow::Prefix::new(source, range, crate::translation_text::MAX_SOURCE)
    })
    .await
    .map_err(|_| "Selection worker stopped")??;
    let mut response = None;
    loop {
        if tables
            .as_ref()
            .is_some_and(|t| t.cancel.is_cancelled() || !(t.current)())
        {
            return Err("Selection document snapshot changed".into());
        }
        let lease = permit.clone();
        let (next, step) = gio::spawn_blocking(move || {
            let _permit = lease;
            let step = prefix.step(response)?;
            Ok::<_, String>((prefix, step))
        })
        .await
        .map_err(|_| "Selection worker stopped")??;
        prefix = next;
        match step {
            flow::PrefixStep::Complete(result) => return Ok(result),
            flow::PrefixStep::Table { id, characters } => {
                let tables = tables.as_ref().ok_or(INDEXED_SELECTION)?;
                let resolve = tables.prefix.as_ref().ok_or(INDEXED_SELECTION)?;
                response = Some(
                    gio::CancellableFuture::new(resolve(id, characters), tables.cancel.clone())
                        .await
                        .map_err(|_| "Selection document snapshot changed")??,
                );
            }
        }
    }
}

pub(crate) const INDEXED_SELECTION: &str =
    "Select text inside a table cell; selection across indexed tables is not available yet";
fn validate_selection_scope(
    document: &flow::Projection,
    range: &std::ops::Range<usize>,
) -> Result<(), String> {
    if !range.is_empty()
        && document.objects.iter().any(|object| {
            matches!(object.kind, ObjectKind::Table { id: Some(_), .. })
                && range.start < object.characters.end
                && range.end > object.characters.start
        })
    {
        return Err(INDEXED_SELECTION.into());
    }
    Ok(())
}

#[path = "flow_object_selection.rs"]
mod object_selection;
#[path = "projected_selection.rs"]
mod primary_selection;
#[path = "selection_scope.rs"]
mod selection_scope;

#[cfg(debug_assertions)]
#[path = "column_picker_acceptance.rs"]
pub mod column_acceptance;
#[cfg(debug_assertions)]
#[path = "flow_images_acceptance.rs"]
pub mod image_acceptance;

pub(crate) use crate::flow_prepare::prepare_projection;
#[path = "selection_request.rs"]
mod selection_request;
pub use crate::flow_prepare::{
    prepare, prepare_cancellable, prepare_lazy_context, prepare_lazy_diagrams, Prepared,
};

pub struct View {
    pub widget: gtk::TextView,
    pub objects: Vec<gtk::Widget>,
    pub document: Arc<flow::Projection>,
    pub copied: Rc<RefCell<Option<Result<flow::Selection, String>>>>,
    pub copying: Rc<Cell<bool>>,
    pub commenting: Rc<Cell<bool>>,
    pub syntax: Arc<syntax::Page>,
    syntax_view: Rc<crate::syntax_ui::Presentation>,
    #[cfg(debug_assertions)]
    pub preparation: (u128, usize, usize),
    #[cfg(debug_assertions)]
    pub construction_us: Vec<(&'static str, u128)>,
    #[cfg(debug_assertions)]
    pub mount_us: Cell<u128>,
}
pub type ConfigureSelection = Rc<dyn Fn(&gtk::Widget, &studio_markdown_native::SourceRange)>;
pub type SelectionSink = Rc<dyn Fn(Result<flow::Selection, String>)>;
#[derive(Clone)]
pub struct Comments {
    pub revision: String,
    /// Frozen document generation/revision and dirty/busy state, owned by caller.
    pub current: Rc<dyn Fn() -> bool>,
    /// Freeze the destination conversation at dispatch, not when the view opens.
    pub begin: Rc<dyn Fn() -> Option<SelectionSink>>,
    pub configure: Option<ConfigureSelection>,
}
pub struct Options {
    pub open: OpenLink,
    pub images: Option<crate::table_cell::RenderImages>,
    pub comments: Option<Comments>,
    /// Standalone diagram source stays immutable in the projection for Copy.
    /// Factories must queue rendering instead of running an engine in GTK.
    pub diagrams: Option<RenderDiagram>,
    /// Reuse document-bound paging/columns without retaining full table rows.
    pub indexed_tables: Option<RenderIndexedTable>,
    pub selection_error: Option<Rc<dyn Fn(&str)>>,
    pub selection_tables: Option<SelectionTables>,
}
pub type RenderIndexedTable = Rc<dyn Fn(usize, usize) -> Result<gtk::Widget, String>>;
pub type RenderDiagram = Rc<dyn Fn(&str, &str) -> Result<gtk::Widget, String>>;
fn table(
    header: &[studio_markdown_native::Cell],
    rows: &[studio_markdown_native::Row],
    open: OpenLink,
    images: Option<crate::table_cell::RenderImages>,
    configure: Option<ConfigureSelection>,
) -> gtk::Widget {
    let model = gio::ListStore::new::<glib::BoxedAnyObject>();
    let selected = Rc::new(RefCell::new((0..header.len()).collect::<Vec<_>>()));
    let view = gtk::ColumnView::new(Some(gtk::NoSelection::new(Some(model.clone()))));
    for (index, title) in header.iter().enumerate() {
        let factory = gtk::SignalListItemFactory::new();
        let follow = open.clone();
        let cell_images = images.clone();
        let cell_configure = configure.clone();
        factory.connect_bind(move |_, item| {
            let item = item.downcast_ref::<gtk::ListItem>().unwrap();
            let value = item.item().and_downcast::<glib::BoxedAnyObject>().unwrap();
            let row = value.borrow::<studio_markdown_native::Row>();
            if let Some(cell) = row.cells.get(index) {
                let configure = cell_configure.as_ref().map(|configure| {
                    let (configure, source) = (configure.clone(), cell.source.clone());
                    Rc::new(move |widget: &gtk::Widget| configure(widget, &source))
                        as crate::table_cell::ConfigureText
                });
                let (widget, label) =
                    crate::table_cell::widget(cell, follow.clone(), configure, cell_images.clone());
                label.set_width_chars(8);
                label.set_max_width_chars(28);
                label.set_lines(3);
                label.set_ellipsize(gtk::pango::EllipsizeMode::End);
                item.set_child(Some(&widget));
            }
        });
        factory.connect_unbind(|_, item| {
            item.downcast_ref::<gtk::ListItem>()
                .unwrap()
                .set_child(None::<&gtk::Widget>)
        });
        let column = gtk::ColumnViewColumn::new(Some(title.title()), Some(factory));
        column.set_resizable(true);
        view.append_column(&column);
        let configure = configure.as_ref().map(|configure| {
            let (configure, source) = (configure.clone(), title.source.clone());
            Rc::new(move |widget: &gtk::Widget| configure(widget, &source))
                as crate::table_cell::ConfigureText
        });
        crate::table_cell::header(
            &column,
            &view,
            title,
            &format!("flow-header-{index}"),
            open.clone(),
            configure,
            images.clone(),
        );
    }
    let scroll = gtk::ScrolledWindow::builder()
        .child(&view)
        .min_content_height(100)
        .max_content_height(160)
        .propagate_natural_height(true)
        .propagate_natural_width(false)
        .build();
    let result = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let toolbar = crate::adaptive::toolbar(6);
    let status = gtk::Label::builder().hexpand(true).xalign(0.0).build();
    let previous = gtk::Button::from_icon_name("go-previous-symbolic");
    previous.set_tooltip_text(Some("Previous table page"));
    let next = gtk::Button::from_icon_name("go-next-symbolic");
    next.set_tooltip_text(Some("Next table page"));
    next.set_widget_name("probe-table-next");
    let data = Rc::new(rows.to_vec());
    let start = Rc::new(Cell::new(0usize));
    let update: Rc<dyn Fn(usize)> = Rc::new({
        let (data, model, start, previous, next) = (
            data.clone(),
            model.clone(),
            start.clone(),
            previous.downgrade(),
            next.downgrade(),
        );
        let status = status.downgrade();
        let selected = selected.clone();
        move |row| {
            let page_size = (512 / selected.borrow().len().max(1)).clamp(1, 16);
            if row >= data.len() && !data.is_empty() {
                return;
            }
            let values: Vec<_> = data
                .iter()
                .skip(row)
                .take(page_size)
                .map(|row| glib::BoxedAnyObject::new(row.clone()))
                .collect();
            model.splice(0, model.n_items(), &values);
            start.set(row);
            if let Some(status) = status.upgrade() {
                crate::table_columns_ui::row_status(
                    &status,
                    (row + 1).min(data.len()),
                    (row + page_size).min(data.len()),
                    data.len(),
                );
            }
            if let Some(button) = previous.upgrade() {
                button.set_sensitive(row > 0);
            }
            if let Some(button) = next.upgrade() {
                button.set_sensitive(row + page_size < data.len());
            }
        }
    });
    {
        let (update, start, selected) = (update.clone(), start.clone(), selected.clone());
        previous.connect_clicked(move |_| {
            update(
                start
                    .get()
                    .saturating_sub((512 / selected.borrow().len().max(1)).clamp(1, 16)),
            )
        });
    }
    {
        let (update, start, selected) = (update.clone(), start.clone(), selected.clone());
        next.connect_clicked(move |_| {
            update(start.get() + (512 / selected.borrow().len().max(1)).clamp(1, 16))
        });
    }
    let columns = gtk::MenuButton::new();
    let owner = view.downgrade();
    let column_widgets = (0..view.columns().n_items())
        .filter_map(|index| {
            view.columns()
                .item(index)
                .and_downcast::<gtk::ColumnViewColumn>()
        })
        .map(|column| column.downgrade())
        .collect::<Vec<_>>();
    let change = update.clone();
    crate::table_columns_ui::install(
        &columns,
        header.iter().map(|cell| cell.title().to_owned()).collect(),
        selected.borrow().clone(),
        64,
        Rc::new({
            let (selected, start) = (selected.clone(), start.clone());
            move |indices| {
                let Some(owner) = owner.upgrade().filter(|view| view.is_mapped()) else {
                    return false;
                };
                *selected.borrow_mut() = indices.to_vec();
                for (index, column) in column_widgets.iter().enumerate() {
                    if let Some(column) = column
                        .upgrade()
                        .filter(|c| c.column_view().as_ref() == Some(&owner))
                    {
                        column.set_visible(indices.contains(&index));
                    }
                }
                change(start.get());
                true
            }
        }),
    );
    crate::i18n_ui::bind(&previous, "tooltip-text", "Previous table page");
    crate::i18n_ui::bind(&next, "tooltip-text", "Next table page");
    toolbar.append(&status);
    toolbar.append(&previous);
    toolbar.append(&next);
    toolbar.append(&columns);
    result.append(&toolbar);
    result.append(&scroll);
    update(0);
    result.upcast()
}
fn picture(image: &crate::diagram::Image, inline: bool, alternative: &str) -> gtk::Widget {
    let texture = gdk::MemoryTexture::new(
        image.width as i32,
        image.height as i32,
        gdk::MemoryFormat::R8g8b8a8Premultiplied,
        &glib::Bytes::from_owned(image.pixels.clone()),
        image.width as usize * 4,
    );
    let height = if inline { 30 } else { 90 };
    let width = (image.width as f64 / image.height as f64 * height as f64)
        .ceil()
        .clamp(1.0, 720.0) as i32;
    gtk::Picture::builder()
        .paintable(&texture)
        .alternative_text(alternative)
        .can_shrink(true)
        .can_target(false)
        .content_fit(gtk::ContentFit::Contain)
        .width_request(width)
        .height_request(height)
        .build()
        .upcast()
}
pub fn view(prepared: Prepared, open: OpenLink) -> Result<View, String> {
    view_with_images(prepared, open, None)
}

/// The owning document supplies its existing renderer and source context.
/// This adapter never reads image files, fetches URLs or decodes images in GTK.
pub fn view_with_images(
    prepared: Prepared,
    open: OpenLink,
    images: Option<crate::table_cell::RenderImages>,
) -> Result<View, String> {
    view_with_options(
        prepared,
        Options {
            open,
            images,
            comments: None,
            diagrams: None,
            indexed_tables: None,
            selection_error: None,
            selection_tables: None,
        },
    )
}
pub fn view_with_options(prepared: Prepared, options: Options) -> Result<View, String> {
    #[cfg(debug_assertions)]
    let construction_started = std::time::Instant::now();
    #[cfg(debug_assertions)]
    let mut construction_us = Vec::new();
    let Options {
        open,
        images,
        comments,
        diagrams,
        indexed_tables,
        selection_error,
        selection_tables,
    } = options;
    if comments
        .as_ref()
        .is_some_and(|c| c.revision != prepared.revision)
    {
        return Err("Text-flow comment source revision changed".into());
    }
    let document = prepared.document;
    let mut table_ids = std::collections::HashSet::new();
    for object in &document.objects {
        if let ObjectKind::Table { id: Some(id), .. } = &object.kind {
            if indexed_tables.is_none() {
                return Err("Indexed tables require the document table renderer".into());
            }
            if !table_ids.insert(*id) {
                return Err("Repeated indexed table anchors require a combined fragment".into());
            }
        }
    }
    if diagrams.is_none()
        && document.objects.iter().enumerate().any(|(i, o)| {
            matches!(o.kind, ObjectKind::Diagram { .. })
                && prepared.images.get(i).is_none_or(Option::is_none)
        })
    {
        return Err("Deferred diagrams require the native diagram renderer".into());
    }
    if images.is_none()
        && document.objects.iter().any(|object| match &object.kind {
            ObjectKind::Image(_) => true,
            ObjectKind::Table { header, rows, .. } => header
                .iter()
                .chain(rows.iter().flat_map(|row| &row.cells))
                .any(|cell| !cell.content.images.is_empty()),
            _ => false,
        })
    {
        return Err("Text-flow images require the document image renderer".into());
    }
    let projected_buffer = primary_selection::Buffer::new();
    let buffer: gtk::TextBuffer = projected_buffer.clone().upcast();
    buffer.set_enable_undo(false);
    buffer.set_text(&document.buffer);
    let layout = crate::anchored_text_view::AnchoredTextView::new(&buffer);
    layout.set_editable(false);
    layout.set_cursor_visible(true);
    layout.set_wrap_mode(gtk::WrapMode::WordChar);
    layout.set_left_margin(24);
    layout.set_right_margin(24);
    layout.set_top_margin(12);
    layout.set_bottom_margin(12);
    let widget: gtk::TextView = layout.clone().upcast();
    // Full documents and rich paragraphs share content-role typography;
    // the interface font can be configured independently.
    widget.add_css_class("studio-content");
    if document.quick.is_none() {
        crate::i18n_ui::accessible_label(&widget, "Preview");
    }
    #[cfg(debug_assertions)]
    construction_us.push(("buffer", construction_started.elapsed().as_micros()));
    let mut objects = Vec::new();
    let mut math_sizes = Vec::new();
    let source_current = Rc::new(Cell::new(true));
    for (index, object) in document.objects.iter().enumerate() {
        let mut start = buffer.iter_at_offset(object.characters.start as i32);
        let mut end = buffer.iter_at_offset(object.characters.start as i32 + 1);
        buffer.delete(&mut start, &mut end);
        let anchor = buffer.create_child_anchor(&mut start);
        let child = match &object.kind {
            ObjectKind::CodeHeader { language, .. } => {
                let expected_buffer = buffer.downgrade();
                let (view, source, current, scope) = (
                    widget.downgrade(),
                    document.clone(),
                    source_current.clone(),
                    selection_tables.as_ref().map(|t| t.current.clone()),
                );
                let header = crate::content_widgets::code_header(language, move |button| {
                    if !current.get()
                        || view.upgrade().is_none_or(|v| {
                            !v.is_mapped()
                                || expected_buffer.upgrade().is_none_or(|b| v.buffer() != b)
                        })
                        || scope.as_ref().is_some_and(|valid| !valid())
                    {
                        return;
                    }
                    if let ObjectKind::CodeHeader { text, .. } = &source.objects[index].kind {
                        button.clipboard().set_text(text);
                    }
                });
                header.add_css_class("card");
                header.upcast()
            }
            ObjectKind::Table {
                id: Some(id),
                row_start,
                ..
            } => indexed_tables.as_ref().unwrap()(*id, *row_start)?,
            ObjectKind::Table { header, rows, .. } => table(
                header,
                rows,
                open.clone(),
                images.clone(),
                comments.as_ref().and_then(|c| c.configure.clone()),
            ),
            ObjectKind::Image(image) => {
                let container = gtk::Box::new(gtk::Orientation::Vertical, 4);
                container.set_widget_name("studio-flow-image");
                let content = studio_markdown_native::RichText {
                    source_only: false,
                    plain: String::new(),
                    markup: String::new(),
                    images: vec![image.clone()],
                    math: Vec::new(),
                    attributes: None,
                };
                images.as_ref().unwrap()(&container, &content, true);
                if container.first_child().is_none() {
                    // Remote/unsupported references and missing snapshot
                    // leases remain readable without introducing a new loader.
                    let label = gtk::Label::builder()
                        .label(if image.alternative.is_empty() {
                            "Image"
                        } else {
                            &image.alternative
                        })
                        .wrap(true)
                        .xalign(0.0)
                        .build();
                    container.append(&label);
                }
                container.upcast()
            }
            ObjectKind::Math { .. } if document.quick.is_some() => {
                let image = prepared.images[index]
                    .as_ref()
                    .ok_or("Missing math image")?;
                let math = crate::math_picture::MathPicture::new(image, &object.alternative)?;
                math_sizes.push((
                    math.downgrade(),
                    image.math.clone().ok_or("Missing math metrics")?,
                ));
                math.upcast()
            }
            ObjectKind::Math { .. } => picture(
                prepared.images[index]
                    .as_ref()
                    .ok_or("Missing formula raster")?,
                true,
                &object.alternative,
            ),
            ObjectKind::Diagram { kind, source } => match &prepared.images[index] {
                Some(image) => picture(image, false, &object.alternative),
                None => diagrams.as_ref().unwrap()(kind, source)?,
            },
        };
        widget.add_child_at_anchor(&child, &anchor);
        if matches!(
            object.kind,
            ObjectKind::Table { .. } | ObjectKind::CodeHeader { .. } | ObjectKind::Image(_)
        ) || (matches!(object.kind, ObjectKind::Diagram { .. })
            && prepared.images[index].is_none())
        {
            let indent = document
                .spans
                .iter()
                .find_map(|span| match span.style {
                    Style::Paragraph { indent, .. }
                        if span.characters.contains(&object.characters.start) =>
                    {
                        Some(indent)
                    }
                    _ => None,
                })
                .unwrap_or(0);
            let inset = i32::try_from(indent)
                .unwrap_or(i32::MAX)
                .saturating_mul(16)
                .saturating_add(48);
            layout.fill_width(&child, inset);
        }
        objects.push(child);
    }
    buffer.connect_changed(move |_| source_current.set(false));
    #[cfg(debug_assertions)]
    construction_us.push(("objects", construction_started.elapsed().as_micros()));
    if document.quick.is_some() {
        widget.add_css_class("studio-markdown");
        let line_height = crate::appearance_ui::markdown_line_tag(&buffer, false);
        buffer.apply_tag(&line_height, &buffer.start_iter(), &buffer.end_iter());
    }
    if !math_sizes.is_empty() {
        let weak = widget.downgrade();
        let last_font = Cell::new(f64::NAN);
        let fit: Rc<dyn Fn()> = Rc::new(move || {
            let Some(view) = weak.upgrade().filter(|v| v.is_mapped()) else {
                return;
            };
            let font = view.pango_context().font_description().map_or(16.0, |f| {
                f.size() as f64 / gtk::pango::SCALE as f64
                    * if f.is_size_absolute() {
                        1.0
                    } else {
                        96.0 / 72.0
                    }
            });
            let font = font.clamp(8.0, 96.0);
            if last_font.replace(font) == font {
                return;
            }
            for (picture, metrics) in &math_sizes {
                if let Some(picture) = picture.upgrade() {
                    picture.fit(metrics, font);
                }
            }
        });
        let paintable = gtk::WidgetPaintable::new(None::<&gtk::Widget>);
        let update = fit.clone();
        paintable.connect_invalidate_size(move |_| update());
        // A compact paragraph can keep the same allocation after a font change.
        // Its content invalidation still observes the new CSS font. Cache that
        // value so ordinary cursor/selection paints never revisit the objects.
        let update = fit.clone();
        paintable.connect_invalidate_contents(move |_| update());
        let mapped = paintable.clone();
        let update = fit.clone();
        widget.connect_map(move |v| {
            mapped.set_widget(Some(v));
            update();
        });
        widget.connect_unmap(move |_| paintable.set_widget(None::<&gtk::Widget>));
    }
    let mut inline_tags: [Option<gtk::TextTag>; 4] = std::array::from_fn(|_| None);
    let mut code_tag: Option<gtk::TextTag> = None;
    for span in &document.spans {
        if matches!(span.style, Style::Code(_)) {
            let tag = code_tag.get_or_insert_with(|| {
                let tag = crate::appearance_ui::markdown_line_tag(&buffer, true);
                tag.set_family(Some("monospace"));
                crate::appearance_ui::watch_inline_tag(&tag);
                tag
            });
            buffer.apply_tag(
                tag,
                &buffer.iter_at_offset(span.characters.start as i32),
                &buffer.iter_at_offset(span.characters.end as i32),
            );
            continue;
        }
        let key = match &span.style {
            Style::Inline(style) => Some(match style {
                studio_markdown_native::inline::Style::Bold => 0,
                studio_markdown_native::inline::Style::Italic => 1,
                studio_markdown_native::inline::Style::Strike => 2,
                studio_markdown_native::inline::Style::Code => 3,
            }),
            _ => None,
        };
        if let Some(tag) = key.and_then(|k| inline_tags[k].as_ref()) {
            buffer.apply_tag(
                tag,
                &buffer.iter_at_offset(span.characters.start as i32),
                &buffer.iter_at_offset(span.characters.end as i32),
            );
            continue;
        }
        let tag = match &span.style {
            Style::Inline(style) => match style {
                studio_markdown_native::inline::Style::Bold => {
                    gtk::TextTag::builder().weight(700).build()
                }
                studio_markdown_native::inline::Style::Italic => gtk::TextTag::builder()
                    .style(gtk::pango::Style::Italic)
                    .build(),
                studio_markdown_native::inline::Style::Strike => {
                    gtk::TextTag::builder().strikethrough(true).build()
                }
                studio_markdown_native::inline::Style::Code => {
                    gtk::TextTag::builder().family("monospace").build()
                }
            },
            Style::Heading(level) => gtk::TextTag::builder()
                .weight(700)
                .scale(match level {
                    1 => 1.7,
                    2 => 1.4,
                    _ => 1.15,
                })
                .build(),
            Style::Code(_) => gtk::TextTag::builder().family("monospace").build(),
            Style::Paragraph { indent, quote } => gtk::TextTag::builder()
                .left_margin(24 + *indent as i32 * 16)
                .pixels_above_lines(if *quote { 4 } else { 0 })
                .build(),
        };
        buffer.tag_table().add(&tag);
        if let Some(key) = key {
            inline_tags[key] = Some(tag.clone());
        }
        if matches!(
            &span.style,
            Style::Inline(studio_markdown_native::inline::Style::Code)
        ) {
            crate::appearance_ui::watch_inline_tag(&tag);
        }
        buffer.apply_tag(
            &tag,
            &buffer.iter_at_offset(span.characters.start as i32),
            &buffer.iter_at_offset(span.characters.end as i32),
        );
    }
    if let Some(bold) = &inline_tags[0] {
        bold.set_priority(buffer.tag_table().size() - 1);
    }
    for link in &document.links {
        let tag = gtk::TextTag::builder()
            .underline(gtk::pango::Underline::Single)
            .foreground("#007f79")
            .build();
        if document.quick.is_some() {
            tag.set_foreground_rgba(Some(&adw::StyleManager::default().accent_color_rgba()));
        }
        buffer.tag_table().add(&tag);
        buffer.apply_tag(
            &tag,
            &buffer.iter_at_offset(link.characters.start as i32),
            &buffer.iter_at_offset(link.characters.end as i32),
        );
    }
    #[cfg(debug_assertions)]
    construction_us.push(("styles", construction_started.elapsed().as_micros()));
    let syntax_view = crate::syntax_ui::Presentation::attach(&widget, prepared.syntax.clone());
    projected_buffer.attach(
        &widget,
        &syntax_view,
        document.clone(),
        selection_tables.clone(),
    );
    object_selection::attach(&widget, &document, &objects);
    #[cfg(debug_assertions)]
    construction_us.push(("syntax", construction_started.elapsed().as_micros()));
    if document.quick.is_none() {
        crate::text_edges::bind(&widget);
    }
    projected_drag(&widget);
    let commenting = Rc::new(Cell::new(false));
    let actions = gio::SimpleActionGroup::new();
    let links = document.clone();
    crate::text_links::bind(
        &widget,
        &actions,
        !document.links.is_empty(),
        Rc::new(move |_, offset| {
            links
                .links
                .iter()
                .find(|l| l.characters.contains(&offset))
                .map(|l| l.destination.clone())
        }),
        open,
    );
    widget.insert_action_group("rich", Some(&actions));
    let menu = gio::Menu::new();
    menu.append(Some("Open link at cursor"), Some("rich.open-link"));
    widget.set_extra_menu(Some(&menu));
    crate::i18n_ui::menu_label(&menu, 0, "Open link at cursor");
    if document.quick.is_some() {
        crate::translation_ui::bind(&widget);
    } else {
        let source = document.clone();
        let current = Rc::downgrade(&syntax_view);
        let tables = selection_tables.clone();
        let report = selection_error.clone();
        let request = selection_request::SelectionRequest::attach(&widget);
        crate::translation_ui::bind_with(
            &widget,
            Rc::new(move |widget| {
                let Some(view) = widget
                    .downcast_ref::<gtk::TextView>()
                    .filter(|v| v.is_mapped())
                else {
                    return;
                };
                if current.upgrade().is_none_or(|p| !p.is_current()) {
                    return;
                }
                let Some((a, b)) = view.buffer().selection_bounds() else {
                    return;
                };
                let range = a.offset() as usize..b.offset() as usize;
                let Some(action) = crate::translation_ui::capture() else {
                    if let Some(report) = &report {
                        report("Select a session before translating");
                    }
                    return;
                };
                let Some(cancel) = request.begin(range.clone()) else {
                    return;
                };
                let (source, current, tables, report, request, view) = (
                    source.clone(),
                    current.clone(),
                    tables.clone(),
                    report.clone(),
                    request.clone(),
                    view.downgrade(),
                );
                glib::spawn_future_local(async move {
                    let selected = gio::CancellableFuture::new(
                        selected_prefix(source, range.clone(), tables.clone()),
                        cancel.clone(),
                    )
                    .await;
                    let valid = view.upgrade().is_some_and(|v| {
                        v.is_mapped()
                            && v.buffer().selection_bounds().is_some_and(|(a, b)| {
                                a.offset() as usize == range.start
                                    && b.offset() as usize == range.end
                            })
                    }) && current.upgrade().is_some_and(|p| p.is_current())
                        && tables
                            .as_ref()
                            .is_none_or(|t| !t.cancel.is_cancelled() && (t.current)());
                    if valid && !cancel.is_cancelled() {
                        match selected {
                            Ok(Ok(selected)) => action(selected.text),
                            Ok(Err(error)) => {
                                if let Some(report) = report {
                                    report(&error);
                                }
                            }
                            Err(_) => (),
                        }
                    }
                    request.finish(&cancel);
                });
            }),
        );
    }
    if let Some(comments) = comments {
        let selection_tables = selection_tables.clone();
        let (source, current, busy) = (
            document.clone(),
            Rc::downgrade(&syntax_view),
            commenting.clone(),
        );
        crate::selection_menu::action(
            widget.upcast_ref(),
            "reader-comment",
            Rc::new(move |widget| {
                if !(comments.current)() || busy.get() {
                    return;
                }
                let Some(view) = widget.downcast_ref::<gtk::TextView>() else {
                    return;
                };
                let Some(scope) = view_scope(view) else {
                    return;
                };
                let Some(sink) = (comments.begin)() else {
                    return;
                };
                if current.upgrade().is_none_or(|c| !c.is_current()) {
                    sink(Err("Text-flow source buffer changed".into()));
                    return;
                }
                let Some((start, end)) = view.buffer().selection_bounds() else {
                    sink(Err("Select a passage of at most 16 KiB in Preview, Source or saved Edit before commenting".into()));
                    return;
                };
                let range = start.offset() as usize..end.offset() as usize;
                busy.set(true);
                let (source, current, busy, view, comments) = (
                    source.clone(),
                    current.clone(),
                    busy.clone(),
                    view.downgrade(),
                    comments.clone(),
                );
                let selection_tables = selection_tables.clone();
                glib::spawn_future_local(async move {
                    let selected=scoped_text(source,range,studio_markdown_native::quote::MAX_QUOTE,selection_tables,scope.clone()).await
                    .and_then(|s| if s.text.trim().is_empty() || s.source.is_none(){Err("Select a passage of at most 16 KiB in Preview, Source or saved Edit before commenting".into())}else{Ok(s)});
                    if !scope.is_cancelled()
                        && view.upgrade().is_some_and(|v| v.is_mapped())
                        && current.upgrade().is_some_and(|c| c.is_current())
                        && (comments.current)()
                    {
                        sink(selected);
                    }
                    if !scope.is_cancelled() {
                        busy.set(false);
                    }
                });
            }),
        );
    }
    let copied = Rc::new(RefCell::new(None));
    let copying = Rc::new(Cell::new(false));
    let result = copied.clone();
    let busy = copying.clone();
    let source = document.clone();
    let current = Rc::downgrade(&syntax_view);
    widget.connect_copy_clipboard(move |view| {
        view.stop_signal_emission_by_name("copy-clipboard");
        if current.upgrade().is_none_or(|s| !s.is_current()) {
            *result.borrow_mut() = Some(Err("Text-flow source buffer changed".into()));
            return;
        }
        let Some((start, end)) = view.buffer().selection_bounds() else {
            return;
        };
        let Some(scope) = view_scope(view) else {
            return;
        };
        if busy.replace(true) {
            return;
        }
        let range = start.offset() as usize..end.offset() as usize;
        let (source, result, busy, weak, current) = (
            source.clone(),
            result.clone(),
            busy.clone(),
            view.downgrade(),
            current.clone(),
        );
        let selection_error = selection_error.clone();
        let selection_tables = selection_tables.clone();
        glib::spawn_future_local(async move {
            let selected = scoped_text(
                source,
                range,
                flow::BUFFER_BYTES,
                selection_tables,
                scope.clone(),
            )
            .await;
            if let Some(view) = weak.upgrade().filter(|v| {
                !scope.is_cancelled()
                    && v.is_mapped()
                    && current.upgrade().is_some_and(|s| s.is_current())
            }) {
                if let Ok(value) = &selected {
                    view.clipboard().set_text(&value.text);
                } else if let (Err(error), Some(report)) = (&selected, &selection_error) {
                    report(error);
                }
                *result.borrow_mut() = Some(selected);
            }
            if !scope.is_cancelled() {
                busy.set(false);
            }
        });
    });
    bind_read_busy(&widget, copying.clone(), commenting.clone());
    #[cfg(debug_assertions)]
    construction_us.push(("interactions", construction_started.elapsed().as_micros()));
    Ok(View {
        widget,
        objects,
        document,
        copied,
        copying,
        commenting,
        syntax: prepared.syntax,
        syntax_view,
        #[cfg(debug_assertions)]
        preparation: (
            prepared.render_us,
            prepared.rendered_objects,
            prepared.reused_objects,
        ),
        #[cfg(debug_assertions)]
        construction_us,
        #[cfg(debug_assertions)]
        mount_us: Cell::new(0),
    })
}

impl View {
    #[cfg(debug_assertions)]
    pub fn layout_updates(&self) -> u64 {
        self.widget
            .downcast_ref::<crate::anchored_text_view::AnchoredTextView>()
            .map_or(0, |view| view.layout_updates())
    }
    /// Styling is tied to the immutable buffer, never reused for replacement text.
    pub fn syntax_current(&self) -> bool {
        self.syntax_view.is_current()
    }
}

/// None means a normal buffer; Some(None) must never fall through to raw anchors.
pub(crate) fn quick_selection(widget: &gtk::Widget, limit: usize) -> Option<Option<String>> {
    let view = widget.downcast_ref::<gtk::TextView>()?;
    let buffer = view.buffer();
    let projected = buffer.downcast_ref::<primary_selection::Buffer>()?;
    Some(projected.quick_selection(limit))
}
pub(crate) fn owns_primary(view: &gtk::TextView) -> Option<bool> {
    view.buffer()
        .downcast_ref::<primary_selection::Buffer>()
        .map(|b| b.owns_primary())
}
pub(crate) fn restore_without_primary(view: &gtk::TextView, a: &gtk::TextIter, b: &gtk::TextIter) {
    let buffer = view.buffer();
    if let Some(buffer) = buffer.downcast_ref::<primary_selection::Buffer>() {
        buffer.select_without_primary(a, b);
    } else {
        buffer.place_cursor(a);
    }
}
pub(crate) fn quick_projection(view: &gtk::TextView) -> Option<Arc<flow::Projection>> {
    view.buffer()
        .downcast_ref::<primary_selection::Buffer>()?
        .projection()
        .filter(|p| p.quick.is_some())
}
fn projected_drag(view: &gtk::TextView) {
    use glib::translate::IntoGlib;
    fn selected(view: &gtk::TextView, x: f64, y: f64) -> Option<gdk::ContentProvider> {
        let buffer = view.buffer();
        let projected = buffer.downcast_ref::<primary_selection::Buffer>()?;
        let source = projected.projection()?;
        // A selected document can contain interactive table cells, links and
        // buttons. Their own selection/drag controllers remain authoritative.
        if source.quick.is_none()
            && view
                .pick(x, y, gtk::PickFlags::DEFAULT)
                .is_some_and(|w| w != *view)
        {
            return None;
        }
        let (a, b) = buffer.selection_bounds()?;
        let (x, y) = view.window_to_buffer_coords(gtk::TextWindowType::Widget, x as i32, y as i32);
        let (iter, _) = view.iter_at_position(x, y)?;
        let rect = view.iter_location(&iter);
        if iter.offset() < a.offset()
            || iter.offset() >= b.offset()
            || x < rect.x()
            || x >= rect.x() + rect.width()
            || y < rect.y()
            || y >= rect.y() + rect.height()
        {
            return None;
        }
        if source.quick.is_some() {
            Some(gdk::ContentProvider::for_value(
                &quick_selection(view.upcast_ref(), flow::RICH_TEXT_BYTES)??.to_value(),
            ))
        } else {
            projected.drag_provider()
        }
    }
    fn block(native: &gtk::GestureDrag, id: u32, blocked: &Cell<bool>, value: bool) {
        if blocked.replace(value) == value {
            return;
        }
        // Public GObject signal blocking, scoped to this owned controller. GTK
        // still recognizes the gesture and owns word/line selection. Only its
        // single-click selected-text export update is replaced, never layout
        // internals or private GTK memory.
        unsafe {
            let object = native.as_ptr().cast();
            let mask = glib::gobject_ffi::G_SIGNAL_MATCH_ID;
            if value {
                glib::gobject_ffi::g_signal_handlers_block_matched(
                    object,
                    mask,
                    id,
                    0,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                );
            } else {
                glib::gobject_ffi::g_signal_handlers_unblock_matched(
                    object,
                    mask,
                    id,
                    0,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                );
            }
        }
    }
    let controllers = view.observe_controllers();
    let Some(native) = (0..controllers.n_items())
        .find_map(|i| controllers.item(i).and_downcast::<gtk::GestureDrag>())
    else {
        return;
    };
    let id = glib::subclass::SignalId::lookup("drag-update", gtk::GestureDrag::static_type())
        .unwrap()
        .into_glib();
    let blocked = Rc::new(Cell::new(false));
    let pressed = Rc::new(Cell::new(None::<(f64, f64)>));
    let drag = gtk::DragSource::new();
    drag.set_actions(gdk::DragAction::COPY);
    drag.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = view.downgrade();
    let other = native.clone();
    let flag = blocked.clone();
    let start = pressed.clone();
    drag.connect_begin(move |gesture, _| {
        start.set(None);
        let Some(view) = weak.upgrade().filter(|v| v.is_mapped()) else {
            return;
        };
        if gesture
            .current_event()
            .is_none_or(|e| e.event_type() != gdk::EventType::ButtonPress)
            || gesture
                .current_event_state()
                .contains(gdk::ModifierType::SHIFT_MASK)
        {
            return;
        }
        let Some((x, y)) = gesture.point(None) else {
            return;
        };
        if selected(&view, x, y).is_some() {
            start.set(Some((x, y)));
            gesture.group_with(&other);
            block(&other, id, &flag, true);
        }
    });
    let weak = view.downgrade();
    drag.connect_prepare(move |_, x, y| {
        let view = weak.upgrade().filter(|v| v.is_mapped())?;
        selected(&view, x, y)
    });
    // Native click counting and multi-click selection stay authoritative. Do
    // not convert a double/triple-click word/line drag into a text export.
    for i in 0..controllers.n_items() {
        if let Some(click) = controllers.item(i).and_downcast::<gtk::GestureClick>() {
            let source = drag.downgrade();
            let other = native.clone();
            let flag = blocked.clone();
            let start = pressed.clone();
            click.connect_pressed(move |_, count, _, _| {
                if count > 1 && flag.get() {
                    start.set(None);
                    if let Some(source) = source.upgrade() {
                        source.ungroup();
                        source.set_state(gtk::EventSequenceState::Denied);
                    }
                    block(&other, id, &flag, false);
                }
            });
        }
    }
    let other = native.clone();
    let flag = blocked.clone();
    let weak = view.downgrade();
    let start = pressed.clone();
    drag.connect_end(move |source, _| {
        if source.drag().is_none() {
            // A capture-phase export gesture also consumes a plain release in
            // an existing selection. Preserve GTK's click-to-place-cursor
            // behavior through public hit testing, only below its own drag
            // threshold. Multi-clicks, touch and selection drags stay native.
            if let Some((sx, sy)) = start.take().filter(|_| flag.get()) {
                if let Some(view) = weak.upgrade().filter(|v| v.is_mapped()) {
                    if source
                        .current_event()
                        .is_some_and(|e| e.event_type() == gdk::EventType::ButtonRelease)
                    {
                        if let Some((x, y)) = source.point(None).filter(|(x, y)| {
                            !view.drag_check_threshold(sx as i32, sy as i32, *x as i32, *y as i32)
                        }) {
                            let (x, y) = view.window_to_buffer_coords(
                                gtk::TextWindowType::Widget,
                                x as i32,
                                y as i32,
                            );
                            if let Some((iter, _)) = view.iter_at_position(x, y) {
                                view.buffer().place_cursor(&iter);
                            }
                        }
                    }
                }
            }
            source.ungroup();
            block(&other, id, &flag, false);
        }
    });
    let other = native.clone();
    let flag = blocked.clone();
    drag.connect_drag_end(move |source, _, _| {
        source.ungroup();
        block(&other, id, &flag, false);
    });
    let other = native.clone();
    let flag = blocked.clone();
    view.connect_unmap(move |_| block(&other, id, &flag, false));
    view.add_controller(drag);
}

#[cfg(test)]
mod preparation_tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use studio_markdown_native::Fragment;
    fn fragment(text: String) -> Fragment {
        Fragment {
            page: 0,
            document_id: "flow-lazy-test".into(),
            revision: "immutable".into(),
            metadata: studio_markdown_native::FragmentMetadata {
                start: 0,
                end: text.len(),
                line: 1,
                rows: 0,
                row_start: 0,
                table: None,
            },
            source: studio_markdown_native::SourceWindow {
                start: 0,
                end: text.len(),
                line: 1,
                end_line: text.lines().count(),
                text,
                projection: vec![],
                references: vec![],
            },
            header: None,
        }
    }
    #[test]
    fn indexed_selection_guard_keeps_neighboring_text_and_local_tables() {
        let input = fragment("Before.\n\n| A | B |\n|---|---|\n| one | two |\n\nAfter.\n".into());
        let mut projected = flow::project(&input).unwrap();
        assert!(validate_selection_scope(&projected, &(0..projected.characters)).is_ok());
        let object = projected
            .objects
            .iter_mut()
            .find(|o| matches!(o.kind, ObjectKind::Table { .. }))
            .unwrap();
        let ObjectKind::Table { id, row_start, .. } = &mut object.kind else {
            unreachable!()
        };
        *id = Some(9);
        *row_start = 100;
        let range = object.characters.clone();
        assert_eq!(
            validate_selection_scope(&projected, &range),
            Err(INDEXED_SELECTION.into())
        );
        assert_eq!(
            validate_selection_scope(&projected, &(0..projected.characters)),
            Err(INDEXED_SELECTION.into())
        );
        assert!(validate_selection_scope(&projected, &(0..range.start)).is_ok());
        assert!(validate_selection_scope(&projected, &(range.end..projected.characters)).is_ok());
        assert!(validate_selection_scope(&projected, &(range.start..range.start)).is_ok());
    }
    #[test]
    fn context_preparation_freezes_revision_and_honors_early_cancellation() {
        let first = fragment("Before.\n\n".into());
        let mut second = fragment("After.\n".into());
        second.page = 1;
        second.source.start = first.source.end;
        second.source.end = second.source.start + second.source.text.len();
        second.source.line = 3;
        second.source.end_line = 3;
        second.metadata.start = second.source.start;
        second.metadata.end = second.source.end;
        second.metadata.line = second.source.line;
        let mut input = vec![first, second];
        let prepared = prepare_lazy_context(&input, Arc::new(AtomicBool::new(false))).unwrap();
        assert_eq!(prepared.revision, "immutable");
        assert_eq!(
            prepared
                .document
                .select(0..prepared.document.characters, 1024)
                .unwrap()
                .text,
            "Before.\n\nAfter."
        );
        assert_eq!(prepared.raster_bytes, 0);
        input[1].revision = "replaced".into();
        assert!(prepare_lazy_context(&input, Arc::new(AtomicBool::new(false))).is_err());
        assert_eq!(
            prepare_lazy_context(&input, Arc::new(AtomicBool::new(true)))
                .err()
                .unwrap(),
            "Text-flow preparation cancelled"
        );
    }
    #[test]
    fn standalone_engine_errors_do_not_block_selectable_text_or_source() {
        let source="Before 中文😀.\n\n```mermaid\nflowchart LR\nA[unterminated\n```\n\n```plantuml\n@startuml\ninvalid\n@enduml\n```\n\n```d2\n{ invalid\n```\n\n```math\n\\unsupportedNativeMacro{x}\n```\n\nAfter text.\n";
        let prepared =
            prepare_lazy_diagrams(&fragment(source.into()), Arc::new(AtomicBool::new(false)))
                .unwrap();
        assert_eq!(prepared.document.objects.len(), 4);
        assert!(prepared.images.iter().all(Option::is_none));
        assert_eq!(prepared.raster_bytes, 0);
        let selected = prepared
            .document
            .select(0..prepared.document.characters, flow::BUFFER_BYTES)
            .unwrap();
        assert!(selected.text.starts_with("Before 中文😀."));
        assert!(selected.text.ends_with("After text."));
        for original in [
            "A[unterminated",
            "@startuml",
            "{ invalid",
            r"\unsupportedNativeMacro{x}",
        ] {
            assert!(selected.text.contains(original));
        }
        assert_eq!(selected.source.unwrap().line, 1);
    }
    #[test]
    fn deferred_mode_preserves_cancellation_and_object_budget() {
        let one = fragment("```mermaid\nflowchart LR\nA-->B\n```\n".into());
        assert!(prepare_lazy_diagrams(&one, Arc::new(AtomicBool::new(true)))
            .err()
            .unwrap()
            .contains("cancelled"));
        let many = fragment(one.source.text.repeat(257));
        assert!(
            prepare_lazy_diagrams(&many, Arc::new(AtomicBool::new(false)))
                .err()
                .unwrap()
                .contains("object budget")
        );
    }
}
