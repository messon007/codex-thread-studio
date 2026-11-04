//! Native bounded reader/editor. GTK handles widgets and interaction; document
//! I/O, Markdown parsing, column projection and edit composition run in document.rs.
#[cfg(debug_assertions)]
#[path = "reader_code_surface_acceptance.rs"]
mod code_surface_acceptance;
#[cfg(debug_assertions)]
#[path = "reader_comment_acceptance.rs"]
mod comment_acceptance;
#[cfg(debug_assertions)]
#[path = "reader_dense_acceptance.rs"]
mod dense_acceptance;
#[cfg(debug_assertions)]
#[path = "reader_diagram_style_acceptance.rs"]
mod diagram_style_acceptance;
#[cfg(debug_assertions)]
#[path = "reader_file_portal_acceptance.rs"]
mod file_portal_acceptance;
#[cfg(debug_assertions)]
#[path = "reader_flow_diagrams_acceptance.rs"]
mod flow_diagrams_acceptance;
#[cfg(debug_assertions)]
#[path = "reader_flow_input_acceptance.rs"]
mod flow_input_acceptance;
#[cfg(debug_assertions)]
#[path = "reader_flow_pipeline_acceptance.rs"]
mod flow_pipeline_acceptance;
#[cfg(debug_assertions)]
#[path = "reader_flow_tables_acceptance.rs"]
mod flow_tables_acceptance;
#[cfg(debug_assertions)]
#[path = "reader_gateway_loss_acceptance.rs"]
mod gateway_loss_acceptance;
#[cfg(debug_assertions)]
#[path = "reader_math_acceptance.rs"]
mod math_acceptance;
#[cfg(debug_assertions)]
#[path = "reader_translation_acceptance.rs"]
mod translation_acceptance;
#[cfg(debug_assertions)]
#[path = "reader_workspace_selection_acceptance.rs"]
mod workspace_selection_acceptance;
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use adw::prelude::*;
use gtk::{gio, glib};
use sourceview::prelude::*;
use studio_markdown_native::{Block, BlockKind, DisplayBlock, SourceWindow};
use tokio::sync::mpsc;

use crate::document::{EditPage, Index, Message, Reply, Request, Response, TablePage};

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Preview,
    Source,
    Edit,
}

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::Source => "source",
            Self::Edit => "edit",
        }
    }
}

enum Navigation {
    Line(usize),
    Page(usize),
    Offset(usize, bool),
}

struct OpenTarget {
    root: PathBuf,
    path: PathBuf,
    line: Option<usize>,
    heading: Option<String>,
    comment: Option<studio_markdown_native::quote::Query>,
}

struct State {
    reader_id: Option<String>,
    generation: u64,
    serial: u64,
    latest: HashMap<&'static str, u64>,
    index: Option<Arc<Index>>,
    path: Option<PathBuf>,
    root: Option<PathBuf>,
    initial_heading: Option<String>,
    initial_comment: Option<studio_markdown_native::quote::Query>,
    comment_notice: Option<&'static str>,
    service_notice: Option<&'static str>,
    preview_error: Option<String>,
    locating_quote: bool,
    comment_kind: studio_markdown_native::quote::Kind,
    line_reference: bool,
    mode: Mode,
    page: usize,
    line: usize,
    anchor_line: usize,
    exact_preview: bool,
    source: Option<SourceWindow>,
    source_caret: Option<usize>,
    editor: Option<EditPage>,
    read_only: bool,
    dirty: bool,
    busy: bool,
    saving: bool,
    loading_editor: bool,
    edit_bytes: usize,
    pending_navigation: Option<Navigation>,
    pending_close: bool,
    pending_quit: bool,
    pending_open: Option<OpenTarget>,
    tables: HashMap<usize, Rc<TableUi>>,
    paragraphs: lru::LruCache<usize, Rc<crate::content_widgets::ParagraphState>>,
    reset_paragraphs: bool,
    edit_marks: Vec<(usize, gtk::TextMark)>,
    diagrams: HashMap<usize, Rc<DiagramUi>>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            reader_id: None,
            generation: 0,
            serial: 0,
            latest: HashMap::new(),
            index: None,
            path: None,
            root: None,
            initial_heading: None,
            initial_comment: None,
            comment_notice: None,
            service_notice: None,
            preview_error: None,
            locating_quote: false,
            comment_kind: studio_markdown_native::quote::Kind::Auto,
            line_reference: false,
            mode: Mode::Preview,
            page: 0,
            line: 1,
            anchor_line: 1,
            exact_preview: false,
            source: None,
            source_caret: None,
            editor: None,
            read_only: true,
            dirty: false,
            busy: false,
            saving: false,
            loading_editor: false,
            edit_bytes: 0,
            pending_navigation: None,
            pending_close: false,
            pending_quit: false,
            pending_open: None,
            tables: HashMap::new(),
            paragraphs: lru::LruCache::new(std::num::NonZeroUsize::new(512).unwrap()),
            reset_paragraphs: false,
            edit_marks: Vec::new(),
            diagrams: HashMap::new(),
        }
    }
}

#[derive(Clone)]
struct TrackedSelection {
    generation: u64,
    revision: String,
    widget: glib::WeakRef<gtk::Widget>,
    // Projected views dispatch their source-aware action instead of reading
    // synthetic GTK characters as if they were original document text.
    source: Option<studio_markdown_native::SourceRange>,
    changed: Rc<Cell<u64>>,
}

pub struct Reader {
    query_button: gtk::Button,
    query: RefCell<Option<Rc<crate::reader_query_ui::Jq>>>,
    sql_query: RefCell<Option<Rc<crate::reader_sql_ui::Sql>>>,
    gateway_available: Cell<bool>,
    session_valid: Cell<bool>,
    pub image_renderer: RefCell<Option<Rc<crate::images_ui::Renderer>>>,
    diagram_preferences: RefCell<Option<Rc<dyn Fn()>>>,
    flow_diagrams: Rc<crate::chat_diagrams::Renderer>,
    flow_tables: RefCell<HashMap<usize, Rc<TableUi>>>,
    flow_epoch: Cell<u64>,
    continuous_preview: Cell<bool>,
    flow_preview: RefCell<Option<crate::text_flow::View>>,
    flow_scroll: RefCell<Option<Rc<crate::viewport::FrameUpdate>>>,
    flow_cancel: RefCell<Option<std::sync::Arc<std::sync::atomic::AtomicBool>>>,
    content_styles: Rc<crate::content_widgets::Styles>,
    pub widget: gtk::Box,
    key: u64,
    window: adw::ApplicationWindow,
    sender: mpsc::Sender<Message>,
    toast: adw::ToastOverlay,
    title: gtk::Label,
    status: gtk::Label,
    notice: gtk::Label,
    modes: [gtk::ToggleButton; 3],
    stack: gtk::Stack,
    blocks: gio::ListStore,
    preview_scroll: gtk::ScrolledWindow,
    preview: gtk::ListView,
    outline: gtk::Revealer,
    headings: gio::ListStore,
    heading_selection: gtk::SingleSelection,
    search: gtk::SearchEntry,
    search_rows: gtk::ListBox,
    source_view: sourceview::View,
    edit_view: sourceview::View,
    source_line: Rc<Cell<usize>>,
    edit_line: Rc<Cell<usize>>,
    open: gtk::Button,
    picker: crate::file_picker::Pending,
    save: gtk::Button,
    previous: gtk::Button,
    next: gtk::Button,
    state: RefCell<State>,
    force_read_only: Cell<bool>,
    changed_lines: RefCell<Vec<usize>>,
    pub on_changed: RefCell<Option<Box<dyn Fn()>>>,
    pub on_close: RefCell<Option<Box<dyn Fn()>>>,
    /// Parent navigation runs before GtkStack can choose a fallback for a hidden child.
    pub on_closing: RefCell<Option<Box<dyn Fn()>>>,
    pub on_open: RefCell<Option<Box<dyn Fn()>>>,
    pub on_file: RefCell<Option<Box<dyn Fn(&str, &str, Option<usize>)>>>,
    pub on_comment: RefCell<Option<Box<dyn Fn(crate::annotations::Capture)>>>,
    pub comments: RefCell<Option<Rc<crate::annotations_ui::CaptureTarget>>>,
    selection: RefCell<Vec<TrackedSelection>>,
    selection_clock: Rc<Cell<u64>>,
}

impl Reader {
    // Reader keys 1..=3 occupy independent document sessions. Keep continuous
    // diagram generations separate from those sessions and conversation lanes.
    fn flow_key(key: u64) -> u64 {
        100 + key
    }
    pub fn accepts_key(&self, key: u64) -> bool {
        key == self.key || key == Self::flow_key(self.key)
    }
    fn clear_flow_preview(&self) {
        if let Some(update) = self.flow_scroll.borrow_mut().take() {
            update.clear();
        }
        if let Some(cancel) = self.flow_cancel.borrow_mut().take() {
            cancel.store(true, std::sync::atomic::Ordering::Release);
        }
        self.flow_epoch.set(self.flow_epoch.get().wrapping_add(1));
        self.flow_diagrams.reset();
        for table in self
            .flow_tables
            .borrow_mut()
            .drain()
            .map(|(_, table)| table)
        {
            table.active.set(false);
            // TextView can validate its buffer after unrooting. Detach an
            // expired table anchor before GTK makes its ColumnView inert;
            // otherwise it can measure recycled cells with no factory child.
            if let Some(parent) = table.widget.parent().and_downcast::<gtk::TextView>() {
                parent.remove(&table.widget);
            }
            table.rows.remove_all();
        }
        if self.preview_scroll.child().as_ref() != Some(self.preview.upcast_ref()) {
            self.preview_scroll.set_child(Some(&self.preview));
        }
        self.flow_preview.borrow_mut().take();
    }
    fn scroll_flow_after_layout(self: &Rc<Self>, view: &gtk::TextView) {
        let (owner, widget, epoch) = (Rc::downgrade(self), view.downgrade(), self.flow_epoch.get());
        let update = crate::viewport::FrameUpdate::new(Rc::new(move |_| {
            if let Some(reader) = owner.upgrade() {
                if reader.flow_epoch.get() == epoch && reader.state.borrow().mode == Mode::Preview {
                    if let Some(view) = widget.upgrade().filter(|v| {
                        v.is_mapped()
                            && reader.preview_scroll.child().as_ref() == Some(v.upcast_ref())
                    }) {
                        let buffer = view.buffer();
                        view.scroll_to_mark(&buffer.get_insert(), 0.0, true, 0.0, 0.0);
                    }
                }
                if reader.flow_epoch.get() == epoch {
                    reader.flow_scroll.borrow_mut().take();
                }
            }
        }));
        let weak = Rc::downgrade(&update);
        view.connect_map(move |view| {
            if let Some(update) = weak.upgrade() {
                update.queue(view);
            }
        });
        let weak = Rc::downgrade(&update);
        view.connect_unmap(move |_| {
            if let Some(update) = weak.upgrade() {
                update.clear();
            }
        });
        self.flow_scroll.replace(Some(update.clone()));
        update.queue(view);
    }
    fn preview_request(&self, page: usize) -> Request {
        self.state.borrow_mut().preview_error = None;
        if let Some(cancel) = self.flow_cancel.borrow_mut().take() {
            cancel.store(true, std::sync::atomic::Ordering::Release);
        }
        if !self.continuous_preview.get() {
            return Request::Fragment(page);
        }
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.flow_cancel.replace(Some(cancel.clone()));
        Request::SelectableFragment { page, cancel }
    }
    #[cfg(debug_assertions)]
    pub(crate) fn smoke_status_text(&self) -> String {
        self.status.text().to_string()
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_images(self: &Rc<Self>) -> Result<serde_json::Value, String> {
        let start = std::time::Instant::now();
        while crate::images_ui::loaded_count(self.widget.upcast_ref(), true) < 2 {
            if start.elapsed() > Duration::from_secs(15) {
                return Err("Inline image previews did not load".into());
            }
            glib::timeout_future(Duration::from_millis(20)).await;
        }
        let count = crate::images_ui::loaded_count(self.widget.upcast_ref(), true);
        self.modes[1].set_active(true);
        glib::timeout_future(Duration::from_millis(50)).await;
        if crate::images_ui::loaded_count(self.widget.upcast_ref(), false) != 0 {
            return Err("Source mode retained mapped image textures".into());
        }
        self.modes[0].set_active(true);
        let start = std::time::Instant::now();
        while crate::images_ui::loaded_count(self.widget.upcast_ref(), true) < 2 {
            if start.elapsed() > Duration::from_secs(15) {
                return Err("Inline image previews did not restore".into());
            }
            glib::timeout_future(Duration::from_millis(20)).await;
        }
        Ok(serde_json::json!({"ok":true,"images":count,"sourceSwitch":true,"previewRestore":true}))
    }
    pub fn make_read_only(&self) {
        self.force_read_only.set(true);
        self.modes[2].set_visible(false);
        self.save.set_visible(false);
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_shared_read_only(
        self: &Rc<Self>,
        root: &Path,
        path: &Path,
        capture: impl Fn(),
    ) -> Result<serde_json::Value, String> {
        let start = std::time::Instant::now();
        while self.state.borrow().busy || self.state.borrow().index.is_none() {
            if start.elapsed() > std::time::Duration::from_secs(12) {
                return Err(format!(
                    "Shared document did not open: {}",
                    self.status.text()
                ));
            }
            glib::timeout_future(std::time::Duration::from_millis(25)).await;
        }
        let state = self.state.borrow();
        if !state.read_only
            || self.modes[2].is_sensitive()
            || self.edit_view.is_editable()
            || state.root.as_deref() != Some(root)
            || state.path.as_deref() != Some(path)
        {
            return Err("Shared reader lost its project/read-only context".into());
        }
        drop(state);
        capture();
        Ok(
            serde_json::json!({"ok":true,"readOnly":true,"editDisabled":true,"projectContextPreserved":true}),
        )
    }

    pub fn location(&self) -> Option<(PathBuf, PathBuf)> {
        let state = self.state.borrow();
        Some((state.root.clone()?, state.path.clone()?))
    }

    pub fn snapshot(&self) -> Option<(String, String)> {
        let state = self.state.borrow();
        Some((
            state.reader_id.clone()?,
            state.index.as_ref()?.revision.clone(),
        ))
    }

    pub fn source_mode(&self) -> bool {
        self.state.borrow().mode == Mode::Source
    }

    pub fn is_ready(&self) -> bool {
        let state = self.state.borrow();
        state.index.is_some() && !state.busy
    }

    #[cfg(debug_assertions)]
    pub fn show_source(self: &Rc<Self>) {
        self.set_mode(Mode::Source);
    }

    #[cfg(debug_assertions)]
    pub fn show_preview(self: &Rc<Self>) {
        self.set_mode(Mode::Preview);
    }

    #[cfg(debug_assertions)]
    pub fn smoke_jump_to_first_line(self: &Rc<Self>) {
        self.navigate(Navigation::Line(1));
    }

    #[cfg(debug_assertions)]
    pub fn smoke_close(self: &Rc<Self>) {
        self.close();
    }
    #[cfg(debug_assertions)]
    pub fn smoke_comment_position(&self) -> (usize, bool, bool) {
        let state = self.state.borrow();
        (
            state.anchor_line,
            state.comment_notice.is_some(),
            state.mode == Mode::Source,
        )
    }
    #[cfg(debug_assertions)]
    pub fn smoke_comment_debug(&self) -> String {
        let state = self.state.borrow();
        format!(
            "ready={} busy={} locating={} line={} status={:?} notice={:?}",
            state.index.is_some(),
            state.busy,
            state.locating_quote,
            state.line,
            self.status.text(),
            state.comment_notice
        )
    }

    pub fn navigate_change(self: &Rc<Self>, line: usize, changed: Vec<usize>) {
        *self.changed_lines.borrow_mut() = changed;
        {
            let mut state = self.state.borrow_mut();
            state.mode = Mode::Source;
            state.reset_paragraphs = true;
        }
        self.load_line(line.saturating_sub(4).max(1));
    }

    fn highlight_source(&self) {
        let buffer = self.source_view.buffer();
        let tag = buffer
            .tag_table()
            .lookup("comparison-change")
            .unwrap_or_else(|| {
                let tag = gtk::TextTag::builder().name("comparison-change").build();
                buffer.tag_table().add(&tag);
                tag
            });
        let dark = adw::StyleManager::default().is_dark();
        tag.set_background(Some(match (self.key == 2, dark) {
            (true, true) => "#663438",
            (true, false) => "#fbe5e5",
            (false, true) => "#204f3d",
            (false, false) => "#dff2e5",
        }));
        buffer.remove_tag(&tag, &buffer.start_iter(), &buffer.end_iter());
        for line in self.changed_lines.borrow().iter() {
            let Some(local) = line.checked_sub(self.source_line.get()) else {
                continue;
            };
            if let Some(start) = buffer.iter_at_line(local as i32) {
                let mut end = start;
                end.forward_line();
                buffer.apply_tag(&tag, &start, &end);
            }
        }
    }

    #[cfg(debug_assertions)]
    pub async fn smoke_link(self: &Rc<Self>, path: &Path) -> Result<serde_json::Value, String> {
        let started = std::time::Instant::now();
        while self.state.borrow().busy || self.state.borrow().index.is_none() {
            if started.elapsed() > Duration::from_secs(10) {
                return Err("Native conversation link timed out".into());
            }
            glib::timeout_future(Duration::from_millis(50)).await;
        }
        if self.state.borrow().path.as_deref() != Some(path)
            || self.state.borrow().mode != Mode::Preview
        {
            return Err("A Markdown line link did not open the mapped native Preview".into());
        }
        Ok(serde_json::json!({"ok":true,"nativeDocumentLink":true,"lineHintPreview":true}))
    }

    #[cfg(debug_assertions)]
    pub(crate) fn smoke_block_adapter(self: &Rc<Self>) -> impl Drop {
        struct Restore(std::rc::Weak<Reader>, bool);
        impl Drop for Restore {
            fn drop(&mut self) {
                if let Some(reader) = self.0.upgrade() {
                    reader.continuous_preview.set(self.1);
                }
            }
        }
        Restore(Rc::downgrade(self), self.continuous_preview.replace(false))
    }

    #[cfg(debug_assertions)]
    pub async fn smoke_test(
        self: &Rc<Self>,
        directory: &Path,
        capture: impl Fn(&Path),
    ) -> Result<serde_json::Value, String> {
        // Retained A/B probe of the old block adapter, including its original
        // row/diagram models. Normal reader acceptance uses the installed
        // controls and continuous pipeline fixtures, without this override.
        let _diagnostic = self.smoke_block_adapter();
        use std::time::Instant;
        async fn until(test: impl Fn() -> bool) -> Result<(), String> {
            let start = Instant::now();
            while !test() {
                if start.elapsed() > Duration::from_secs(25) {
                    return Err("Native document acceptance step timed out".into());
                }
                glib::timeout_future(Duration::from_millis(50)).await;
            }
            Ok(())
        }
        let started = Instant::now();
        let ticks = Rc::new(Cell::new(0));
        let counted = ticks.clone();
        let timer = glib::timeout_add_local(Duration::from_millis(16), move || {
            counted.set(counted.get() + 1);
            glib::ControlFlow::Continue
        });
        self.open_path(&directory.join("project/large.md"));
        until(|| {
            !self.state.borrow().busy
                && self.state.borrow().index.is_some()
                && self.blocks.n_items() > 0
        })
        .await?;
        let open_ms = started.elapsed().as_millis();
        let opening_ticks = ticks.get();
        timer.remove();
        let (bytes, headings, table_page) = {
            let state = self.state.borrow();
            let index = state.index.as_ref().unwrap();
            (
                index.source_bytes,
                index.toc.len(),
                index.tables.first().map(|table| table.first_page),
            )
        };
        let mut expired_table = None;
        if let Some(page) = table_page {
            self.navigate(Navigation::Page(page));
            until(|| !self.state.borrow().busy).await?;
            crate::window_capture::next_frame(&self.widget).await?;
            capture(&directory.join("reader.png"));
            until(|| {
                self.state
                    .borrow()
                    .tables
                    .values()
                    .any(|table| table.rows.n_items() > 0)
            })
            .await?;
            let table = self
                .state
                .borrow()
                .tables
                .get(&0)
                .cloned()
                .ok_or("Native table missing")?;
            expired_table = Some(table.clone());
            if table.columns.borrow().len() > 1 {
                fn checks(widget: &gtk::Widget) -> Vec<gtk::CheckButton> {
                    let mut values = widget
                        .clone()
                        .downcast::<gtk::CheckButton>()
                        .into_iter()
                        .collect::<Vec<_>>();
                    let mut child = widget.first_child();
                    while let Some(widget) = child {
                        child = widget.next_sibling();
                        values.extend(checks(&widget));
                    }
                    values
                }
                until(|| table.widget.is_mapped() && table.widget.width() > 0).await?;
                let original = table
                    .view
                    .columns()
                    .item(0)
                    .and_downcast::<gtk::ColumnViewColumn>()
                    .ok_or("First reader column missing")?;
                original.set_fixed_width(310);
                let old_count = table.columns.borrow().len();
                let popover = table.menu.popover().ok_or("Reader column menu missing")?;
                popover.popup();
                until(|| {
                    checks(popover.upcast_ref())
                        .iter()
                        .filter(|c| c.is_mapped())
                        .count()
                        >= 2
                })
                .await?;
                let options = checks(popover.upcast_ref());
                options[0].set_active(false);
                until(|| {
                    table
                        .rows
                        .item(0)
                        .and_downcast::<glib::BoxedAnyObject>()
                        .is_some_and(|row| {
                            row.borrow::<Vec<studio_markdown_native::Cell>>().len() == old_count - 1
                        })
                })
                .await?;
                let moved = table
                    .view
                    .columns()
                    .item(0)
                    .and_downcast::<gtk::ColumnViewColumn>()
                    .unwrap();
                if moved.fixed_width() != 210 {
                    return Err("Resized width moved to the wrong original column".into());
                }
                options[0].set_active(true);
                until(|| {
                    table
                        .view
                        .columns()
                        .item(0)
                        .and_downcast::<gtk::ColumnViewColumn>()
                        .is_some_and(|column| {
                            column.title() == original.title() && column.fixed_width() == 310
                        })
                })
                .await?;
                options[1].set_active(false);
                until(|| {
                    table.columns.borrow().len() == old_count - 1
                        && table
                            .rows
                            .item(0)
                            .and_downcast::<glib::BoxedAnyObject>()
                            .is_some_and(|r| {
                                r.borrow::<Vec<studio_markdown_native::Cell>>().len()
                                    == old_count - 1
                            })
                })
                .await?;
                popover.popdown();
            }
            let size = table.size.get();
            if table.total.get() > size {
                table.load(size);
                until(|| table.start.get() == size && table.rows.n_items() > 0).await?;
            }
            crate::window_capture::next_frame(&self.widget).await?;
            capture(&directory.join("reader.png"));
        } else {
            crate::window_capture::next_frame(&self.widget).await?;
            capture(&directory.join("reader.png"));
        }
        self.set_mode(Mode::Source);
        until(|| !self.state.borrow().busy && self.state.borrow().source.is_some()).await?;
        self.navigate(Navigation::Line(100));
        until(|| !self.state.borrow().busy && self.source_line.get() == 100).await?;
        if self.state.borrow().source.as_ref().unwrap().line != 100 {
            return Err("Source line mapping changed".into());
        }
        self.finish_close();
        self.open_path(&directory.join("project/diagrams.md"));
        until(|| !self.state.borrow().busy && self.blocks.n_items() > 0).await?;
        if let Some(table) = expired_table {
            let serial = table.serial.get();
            if table.current_reader().is_some() {
                return Err("Old table retained a live document snapshot".into());
            }
            table.load(0);
            if table.serial.get() != serial {
                return Err("Expired table requested rows from a replacement document".into());
            }
        }
        crate::window_capture::next_frame(&self.widget).await?;
        capture(&directory.join("diagrams.png"));
        until(|| {
            self.state.borrow().diagrams.len() == 3
                && self
                    .state
                    .borrow()
                    .diagrams
                    .values()
                    .all(|diagram| diagram.picture.paintable().is_some())
        })
        .await?;
        self.preview.queue_resize();
        self.preview_scroll.vadjustment().set_value(0.0);
        glib::timeout_future(Duration::from_millis(100)).await;
        crate::window_capture::next_frame(&self.widget).await?;
        capture(&directory.join("diagrams.png"));
        let geometry=self.state.borrow().diagrams.values().map(|diagram|{
            let bounds=diagram.picture.compute_bounds(&self.preview);
            serde_json::json!({"kind":diagram.kind,"width":diagram.picture.width(),"height":diagram.picture.height(),"visible":diagram.picture.is_visible(),"x":bounds.map(|value|value.x()),"y":bounds.map(|value|value.y())})
        }).collect::<Vec<_>>();
        let _ = std::fs::write(
            directory.join("diagram-geometry.json"),
            serde_json::to_vec_pretty(&geometry).unwrap(),
        );
        if self
            .state
            .borrow()
            .diagrams
            .values()
            .any(|diagram| diagram.picture.height() < 20)
        {
            return Err("Native diagram row collapsed to zero height".into());
        }
        self.finish_close();
        self.smoke_math(directory, &capture).await?;
        self.open_path(&directory.join("project/edit.md"));
        until(|| !self.state.borrow().busy && self.state.borrow().index.is_some()).await?;
        self.set_mode(Mode::Edit);
        until(|| !self.state.borrow().busy && self.state.borrow().editor.is_some()).await?;
        let buffer = self.edit_view.buffer();
        let original = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), true)
            .to_string();
        buffer.begin_user_action();
        buffer.insert(&mut buffer.start_iter(), "Native 😀 edit\n");
        buffer.end_user_action();
        if !self.state.borrow().dirty {
            return Err("Native editor failed to track input".into());
        }
        self.navigate(Navigation::Line(1));
        if !self.state.borrow().dirty
            || self.state.borrow().busy
            || std::fs::read(directory.join("project/edit.md"))
                .map_err(|error| error.to_string())?
                != "# Native edit\r\n\r\n中文😀 before\r\n".as_bytes()
        {
            return Err("Navigating within the edited page unexpectedly saved it".into());
        }
        self.set_mode(Mode::Preview);
        until(|| !self.state.borrow().busy).await?;
        if !self.state.borrow().dirty || !self.notice.is_visible() {
            return Err("Switching to Preview discarded dirty edits".into());
        }
        self.set_mode(Mode::Edit);
        self.save_page();
        until(|| !self.state.borrow().busy && !self.state.borrow().dirty).await?;
        until(|| {
            self.state
                .borrow()
                .index
                .as_ref()
                .is_some_and(|index| index.source_lines >= 5)
        })
        .await?;
        let file = directory.join("project/edit.md");
        let saved = std::fs::read(&file).map_err(|e| e.to_string())?;
        if !saved.starts_with("Native 😀 edit\r\n".as_bytes()) {
            return Err("Native save lost Unicode or CRLF line endings".into());
        }
        buffer.undo();
        self.save_page();
        until(|| !self.state.borrow().busy && !self.state.borrow().dirty).await?;
        if buffer.text(&buffer.start_iter(), &buffer.end_iter(), true) != original {
            return Err("Undo across save failed".into());
        }
        buffer.redo();
        self.save_page();
        until(|| !self.state.borrow().busy && !self.state.borrow().dirty).await?;
        buffer.insert(&mut buffer.end_iter(), "Unsaved local edit\n");
        let replacement = directory.join("project/diagrams.md");
        self.open_path(&replacement);
        let find_guard = || {
            gtk::Window::list_toplevels()
                .into_iter()
                .filter_map(|window| window.downcast::<adw::MessageDialog>().ok())
                .find(|dialog| {
                    dialog.heading().as_deref()
                        == Some("Save changes before opening another document?")
                })
        };
        let guard = find_guard().ok_or("Document replacement did not offer a dirty-edit guard")?;
        guard.response("cancel");
        if !self.state.borrow().dirty || self.state.borrow().path.as_ref() != Some(&file) {
            return Err("Cancelling replacement lost the current edit page".into());
        }
        std::fs::write(&file, b"# External change\n").map_err(|e| e.to_string())?;
        self.save_page();
        until(|| !self.state.borrow().busy).await?;
        if !self.state.borrow().dirty
            || std::fs::read(&file).map_err(|e| e.to_string())? != b"# External change\n"
        {
            return Err("Native save conflict overwrote the file or lost edits".into());
        }
        crate::window_capture::next_frame(&self.widget).await?;
        capture(&directory.join("editor.png"));
        self.open_path(&replacement);
        let guard = find_guard().ok_or("Replacement discard guard was not shown")?;
        guard.response("discard");
        until(|| self.is_ready() && self.state.borrow().path.as_ref() == Some(&replacement))
            .await?;
        if std::fs::read(&file).map_err(|error| error.to_string())? != b"# External change\n" {
            return Err("Discarding a page changed the original file".into());
        }
        self.finish_close();
        let flow_comments = self.smoke_flow_comments(directory).await?;
        let flow_diagrams = self.smoke_flow_diagrams(directory).await?;
        let flow_tables = self.smoke_flow_tables(directory).await?;
        Ok(
            serde_json::json!({"ok":true,"sourceBytes":bytes,"openMs":open_ms,"openingMainLoopTicks":opening_ticks,"headings":headings,"tables":table_page.is_some(),"columnSelection":true,"columnWidths":true,"staleTableRejected":true,"tablePaging":true,"sourceLineMapping":true,"pagedEdit":true,"dirtyViewSwitch":true,"unicodeCrlfSave":true,"undoRedoAcrossSave":true,"saveConflict":true,"replacementGuard":true,"flowComments":flow_comments,"flowDiagrams":flow_diagrams,"flowTables":flow_tables}),
        )
    }

    pub fn new(
        key: u64,
        window: &adw::ApplicationWindow,
        sender: mpsc::Sender<Message>,
        toast: &adw::ToastOverlay,
    ) -> Rc<Self> {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let header = crate::adaptive::toolbar(6);
        header.set_margin_start(12);
        header.set_margin_end(12);
        header.set_margin_top(8);
        header.set_margin_bottom(8);
        header.add_css_class("toolbar");
        let title = gtk::Label::builder()
            .label("Document")
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::Middle)
            .hexpand(true)
            .build();
        title.add_css_class("heading");
        let open = icon("document-open-symbolic", "Open Markdown");
        header.append(&open);
        header.append(&title);
        let modes = [
            gtk::ToggleButton::with_label("Preview"),
            gtk::ToggleButton::with_label("Source"),
            gtk::ToggleButton::with_label("Edit"),
        ];
        for (button, key) in modes
            .iter()
            .zip(["Preview", "markdown-mode\u{4}Source", "Edit"])
        {
            crate::i18n_ui::bind(button, "label", key);
        }
        let mode_group = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        mode_group.add_css_class("linked");
        modes[1].set_group(Some(&modes[0]));
        modes[2].set_group(Some(&modes[0]));
        modes[0].set_active(true);
        for mode in &modes {
            crate::adaptive::compact_button(mode);
            mode_group.append(mode);
        }
        header.append(&mode_group);
        let query_button = icon("system-search-symbolic", "Query JSON with jq");
        query_button.set_visible(false);
        header.append(&query_button);
        let comment = icon("insert-text-symbolic", "Comment on selected text");
        comment.set_widget_name("studio-reader-comment");
        comment.set_focus_on_click(false);
        header.append(&comment);
        let contents = icon("view-list-symbolic", "Contents and search");
        let reload = icon("view-refresh-symbolic", "Reload saved document");
        let save = icon("document-save-symbolic", "Save current page (Ctrl+S)");
        let close = icon("window-close-symbolic", "Close document");
        for button in [&contents, &reload, &save, &close] {
            header.append(button);
        }
        widget.append(&header);
        widget.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        let notice = gtk::Label::builder()
            .label("Unsaved edits are kept in memory. Save to refresh Preview and Source.")
            .wrap(true)
            .xalign(0.0)
            .visible(false)
            .build();
        notice.set_margin_start(20);
        notice.set_margin_end(20);
        notice.set_margin_top(8);
        notice.set_margin_bottom(8);
        notice.add_css_class("warning");
        widget.append(&notice);

        let blocks = gio::ListStore::new::<glib::BoxedAnyObject>();
        let factory = gtk::SignalListItemFactory::new();
        let preview = gtk::ListView::new(
            Some(gtk::NoSelection::new(Some(blocks.clone()))),
            Some(factory.clone()),
        );
        preview.add_css_class("background");
        let preview_scroll = gtk::ScrolledWindow::builder()
            .child(&preview)
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();
        let (source_view, source_line) = text_view(false);
        let (edit_view, edit_line) = text_view(true);
        let stack = gtk::Stack::builder().vexpand(true).hexpand(true).build();
        stack.add_named(&preview_scroll, Some("preview"));
        stack.add_named(
            &gtk::ScrolledWindow::builder().child(&source_view).build(),
            Some("source"),
        );
        stack.add_named(
            &gtk::ScrolledWindow::builder().child(&edit_view).build(),
            Some("edit"),
        );
        let search = gtk::SearchEntry::builder()
            .placeholder_text("Search exact source text")
            .build();
        search.set_margin_start(8);
        search.set_margin_end(8);
        search.set_margin_top(8);
        search.set_margin_bottom(8);
        let headings = gio::ListStore::new::<glib::BoxedAnyObject>();
        let heading_selection = gtk::SingleSelection::new(Some(headings.clone()));
        heading_selection.set_autoselect(false);
        heading_selection.set_can_unselect(true);
        let heading_factory = gtk::SignalListItemFactory::new();
        heading_factory.connect_setup(|_, item| {
            let item = item.downcast_ref::<gtk::ListItem>().unwrap();
            item.set_child(Some(
                &gtk::Label::builder()
                    .xalign(0.0)
                    .wrap(true)
                    .margin_top(7)
                    .margin_bottom(7)
                    .margin_end(8)
                    .build(),
            ));
        });
        heading_factory.connect_bind(|_, item| {
            let item = item.downcast_ref::<gtk::ListItem>().unwrap();
            let object = item.item().and_downcast::<glib::BoxedAnyObject>().unwrap();
            let heading = object.borrow::<crate::document::Heading>();
            let label = item.child().and_downcast::<gtk::Label>().unwrap();
            label.set_text(&heading.label);
            label.set_margin_start(12 + heading.level.saturating_sub(1) as i32 * 12);
            crate::i18n_ui::bind_format(
                &label,
                "tooltip-text",
                "Line {line}",
                &[("line", &heading.line.to_string())],
            );
        });
        let heading_view =
            gtk::ListView::new(Some(heading_selection.clone()), Some(heading_factory));
        heading_view.set_single_click_activate(true);
        heading_view.add_css_class("navigation-sidebar");
        let search_rows = gtk::ListBox::new();
        search_rows.add_css_class("navigation-sidebar");
        let outline_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        outline_box.set_width_request(220);
        outline_box.append(&search);
        outline_box.append(
            &gtk::ScrolledWindow::builder()
                .child(&heading_view)
                .vexpand(true)
                .hscrollbar_policy(gtk::PolicyType::Never)
                .build(),
        );
        outline_box.append(
            &gtk::ScrolledWindow::builder()
                .child(&search_rows)
                .max_content_height(240)
                .propagate_natural_height(true)
                .hscrollbar_policy(gtk::PolicyType::Never)
                .build(),
        );
        let outline = gtk::Revealer::builder()
            .child(&outline_box)
            .transition_type(gtk::RevealerTransitionType::SlideRight)
            .build();
        let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        body.set_vexpand(true);
        body.append(&outline);
        body.append(&stack);
        widget.append(&body);
        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        footer.set_margin_top(6);
        footer.set_margin_bottom(6);
        footer.set_margin_start(12);
        footer.set_margin_end(12);
        let status = gtk::Label::builder()
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        status.add_css_class("caption");
        let previous = icon("go-previous-symbolic", "Previous page");
        let next = icon("go-next-symbolic", "Next page");
        footer.append(&status);
        footer.append(&previous);
        footer.append(&next);
        widget.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        widget.append(&footer);
        let reader = Rc::new(Self {
            query_button: query_button.clone(),
            query: RefCell::default(),
            sql_query: RefCell::default(),
            gateway_available: Cell::new(true),
            session_valid: Cell::new(true),
            flow_diagrams: crate::chat_diagrams::Renderer::new_viewport_bounded(
                sender.clone(),
                Self::flow_key(key),
                32 * 1024 * 1024,
            ),
            flow_tables: RefCell::default(),
            flow_epoch: Cell::new(0),
            continuous_preview: Cell::new(
                std::env::var_os("STUDIO_GNOME_CONTINUOUS_PREVIEW").is_none_or(|v| v != "0"),
            ),
            flow_preview: RefCell::default(),
            flow_scroll: RefCell::default(),
            flow_cancel: RefCell::default(),
            content_styles: crate::content_widgets::Styles::new(),
            widget,
            key,
            window: window.clone(),
            sender,
            toast: toast.clone(),
            title,
            status,
            notice,
            modes,
            stack,
            blocks,
            preview_scroll,
            preview,
            outline,
            headings,
            heading_selection,
            search,
            search_rows,
            source_view,
            edit_view,
            source_line,
            edit_line,
            open: open.clone(),
            picker: Default::default(),
            save: save.clone(),
            previous,
            next,
            state: RefCell::new(State::default()),
            force_read_only: Cell::new(false),
            changed_lines: RefCell::new(Vec::new()),
            image_renderer: RefCell::new(None),
            diagram_preferences: RefCell::new(None),
            on_changed: RefCell::new(None),
            on_close: RefCell::new(None),
            on_closing: RefCell::new(None),
            on_open: RefCell::new(None),
            on_file: RefCell::new(None),
            on_comment: RefCell::new(None),
            comments: RefCell::new(None),
            selection: RefCell::new(Vec::new()),
            selection_clock: Rc::new(Cell::new(0)),
        });
        let weak = Rc::downgrade(&reader);
        query_button.connect_clicked(move |_| {
            if let Some(reader) = weak.upgrade() {
                reader.open_query();
            }
        });
        let weak = Rc::downgrade(&reader);
        comment.connect_clicked(move |_| {
            if let Some(reader) = weak.upgrade() {
                reader.comment_selection();
            }
        });
        let weak = Rc::downgrade(&reader);
        factory.connect_bind(move |_, item| {
            let Some(reader) = weak.upgrade() else { return };
            let item = item.downcast_ref::<gtk::ListItem>().unwrap();
            let object = item.item().and_downcast::<glib::BoxedAnyObject>().unwrap();
            let row = object.borrow::<DisplayBlock>();
            let content = reader.block(&row.block);
            // Clamp individual realized rows; the ListView must remain the scrollable child.
            let clamp = adw::Clamp::builder()
                .maximum_size(1000)
                .tightening_threshold(800)
                .build();
            crate::appearance_ui::watch_clamp(&clamp);
            content.set_margin_start(20 + row.indent as i32 * 16);
            if row.quote {
                content.add_css_class("dim-label");
            }
            if let Some(marker) = &row.marker {
                let outer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                let label = gtk::Label::builder()
                    .label(marker)
                    .valign(gtk::Align::Start)
                    .margin_top(8)
                    .margin_start(20 + (row.indent.saturating_sub(1)) as i32 * 16)
                    .build();
                content.set_margin_start(8);
                outer.append(&label);
                outer.append(&content);
                clamp.set_child(Some(&outer));
            } else {
                clamp.set_child(Some(&content));
            }
            item.set_child(Some(&clamp));
        });
        factory.connect_unbind(|_, item| {
            item.downcast_ref::<gtk::ListItem>()
                .unwrap()
                .set_child(gtk::Widget::NONE);
        });
        for (index, mode) in [Mode::Preview, Mode::Source, Mode::Edit]
            .into_iter()
            .enumerate()
        {
            let weak = Rc::downgrade(&reader);
            reader.modes[index].connect_toggled(move |button| {
                if button.is_active() {
                    if let Some(reader) = weak.upgrade() {
                        reader.set_mode(mode);
                    }
                }
            });
        }
        for (button, action) in [
            (&open, Self::choose as fn(&Rc<Self>)),
            (&contents, Self::toggle_outline),
            (&reload, Self::reload),
            (&save, Self::save_page),
            (&close, Self::close),
        ] {
            let weak = Rc::downgrade(&reader);
            button.connect_clicked(move |_| {
                if let Some(reader) = weak.upgrade() {
                    action(&reader);
                }
            });
        }
        let weak = Rc::downgrade(&reader);
        reader.previous.connect_clicked(move |_| {
            if let Some(reader) = weak.upgrade() {
                reader.page(false);
            }
        });
        let weak = Rc::downgrade(&reader);
        reader.next.connect_clicked(move |_| {
            if let Some(reader) = weak.upgrade() {
                reader.page(true);
            }
        });
        let weak = Rc::downgrade(&reader);
        reader
            .heading_selection
            .connect_selected_item_notify(move |selection| {
                if let Some(reader) = weak.upgrade() {
                    if let Some(item) = selection
                        .selected_item()
                        .and_downcast::<glib::BoxedAnyObject>()
                    {
                        let line = item.borrow::<crate::document::Heading>().line;
                        reader.navigate(Navigation::Line(line));
                    }
                }
            });
        let weak = Rc::downgrade(&reader);
        reader.search.connect_activate(move |entry| {
            if let Some(reader) = weak.upgrade() {
                let query = entry.text().to_string();
                if !query.is_empty() {
                    reader.request("search", Request::Search(query));
                }
            }
        });
        connect_edits(&reader);
        let keys = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(&reader);
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            if modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK) && key == gtk::gdk::Key::s {
                if let Some(reader) = weak.upgrade() {
                    reader.save_page();
                }
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        reader.widget.add_controller(keys);
        let weak = Rc::downgrade(&reader);
        reader.widget.connect_unmap(move |_| {
            if let Some(reader) = weak.upgrade() {
                reader.cancel_picker();
            }
        });
        let weak = Rc::downgrade(&reader);
        glib::timeout_add_local(Duration::from_secs(30), move || {
            let Some(reader) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            if reader.widget.is_visible() && reader.state.borrow().generation > 0 {
                reader.request("heartbeat", Request::Heartbeat);
            }
            glib::ControlFlow::Continue
        });
        reader.update_style();
        let weak = Rc::downgrade(&reader);
        let callback: Rc<dyn Fn()> = Rc::new(move || {
            if let Some(reader) = weak.upgrade() {
                reader.diagram_preferences_changed();
            }
        });
        crate::appearance_ui::watch_diagrams(&callback);
        reader.diagram_preferences.replace(Some(callback));
        let weak = Rc::downgrade(&reader);
        adw::StyleManager::default().connect_dark_notify(move |_| {
            if let Some(reader) = weak.upgrade() {
                reader.update_style();
            }
        });
        reader.update();
        reader
    }

    fn request(&self, kind: &'static str, request: Request) -> Option<u64> {
        let (serial, generation) = {
            let mut state = self.state.borrow_mut();
            state.serial += 1;
            (state.serial, state.generation)
        };
        let message = Message {
            key: self.key,
            generation,
            serial,
            request,
        };
        let local = matches!(
            &message.request,
            Request::Insert { .. }
                | Request::Delete { .. }
                | Request::Close
                | Request::CancelDiagram { .. }
                | Request::CancelDiagrams
        );
        let opening = matches!(
            &message.request,
            Request::Open { .. } | Request::Resume { .. }
        );
        let sent =
            if local || (self.gateway_available.get() && (opening || self.session_valid.get())) {
                self.sender.try_send(message)
            } else {
                Err(mpsc::error::TrySendError::Closed(message))
            };
        match sent {
            Ok(()) => {
                self.state.borrow_mut().latest.insert(kind, serial);
                Some(serial)
            }
            Err(_) => {
                let mut state = self.state.borrow_mut();
                if kind == "source" {
                    state.source_caret = None;
                }
                if matches!(
                    kind,
                    "open" | "resume" | "index" | "fragment" | "source" | "edit" | "save"
                ) {
                    state.busy = false;
                    if kind == "save" {
                        state.saving = false;
                    }
                    state.pending_navigation = None;
                    state.pending_close = false;
                    state.pending_quit = false;
                }
                drop(state);
                self.error("The document worker is busy; try again");
                None
            }
        }
    }

    pub fn choose(self: &Rc<Self>) {
        if self.state.borrow().busy || self.picker.is_pending() {
            return;
        }
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("Markdown"));
        for pattern in ["*.md", "*.markdown", "*.MD", "*.mdown"] {
            filter.add_pattern(pattern);
        }
        let dialog = gtk::FileDialog::builder().title("Open Markdown").build();
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        dialog.set_filters(Some(&filters));
        let Some(cancel) = self.picker.begin() else {
            return;
        };
        self.update();
        let weak = Rc::downgrade(self);
        let request = cancel.clone();
        dialog.open(Some(&self.window), Some(&cancel), move |result| {
            let Some(reader) = weak.upgrade() else { return };
            if !reader.picker.complete(&request) {
                return;
            }
            reader.update();
            match result {
                Ok(file) => match file.path().filter(|p| p.to_str().is_some()) {
                    Some(path) => reader.open_path(&path),
                    None => reader.error("Choose a local Markdown file with a UTF-8 path"),
                },
                Err(error)
                    if error.matches(gtk::DialogError::Dismissed)
                        || error.matches(gio::IOErrorEnum::Cancelled) => {}
                Err(error) => reader.error(&format!("Unable to open file chooser: {error}")),
            }
        });
    }

    fn cancel_picker(&self) {
        self.picker.cancel();
        self.open.set_sensitive(!self.state.borrow().busy);
    }

    pub fn gateway_failed(&self, error: &str) {
        self.gateway_available.set(false);
        self.session_valid.set(false);
        let mut state = self.state.borrow_mut();
        // In particular, an old Save acknowledgement must not clear text typed
        // after failure releases the editor's saving lock.
        state.latest.clear();
        state.service_notice = Some(
            "Gateway disconnected. Edits are kept in memory. Use Reconnect to restore services.",
        );
        if state.locating_quote {
            state.locating_quote = false;
            state.comment_notice = Some("Comment lookup failed. The saved location is unverified.");
        }
        state.busy = false;
        state.saving = false;
        state.pending_navigation = None;
        state.pending_close = false;
        state.pending_quit = false;
        state.pending_open = None;
        drop(state);
        if self.widget.is_visible() {
            self.error(error);
            self.update();
        }
    }

    pub fn gateway_restored(self: &Rc<Self>) {
        self.gateway_available.set(true);
        if self.state.borrow().path.is_none() {
            self.session_valid.set(true);
            self.update();
            return;
        }
        if self.session_valid.get() || self.state.borrow().busy {
            self.update();
            return;
        }
        let request = {
            let mut state = self.state.borrow_mut();
            let revision = state
                .editor
                .as_ref()
                .map(|page| page.revision.clone())
                .or_else(|| state.index.as_ref().map(|index| index.revision.clone()));
            let request = if let Some(revision) = revision {
                let text = state.editor.as_ref().map(|_| {
                    let buffer = self.edit_view.buffer();
                    buffer
                        .text(&buffer.start_iter(), &buffer.end_iter(), true)
                        .to_string()
                });
                Request::Resume { revision, text }
            } else {
                Request::Open {
                    root: state.root.as_ref().unwrap().to_string_lossy().into_owned(),
                    path: state.path.as_ref().unwrap().to_string_lossy().into_owned(),
                    read_only: self.force_read_only.get(),
                }
            };
            state.generation += 1;
            state.latest.clear();
            state.busy = true;
            state.service_notice = Some("Revalidating the document snapshot…");
            request
        };
        let kind = if matches!(request, Request::Resume { .. }) {
            "resume"
        } else {
            "open"
        };
        self.request(kind, request);
        self.update();
    }

    pub fn open_path(self: &Rc<Self>, path: &Path) {
        let Some(parent) = path.parent() else { return };
        self.open_project_path(parent, path, None, None);
    }

    pub fn open_project_path(
        self: &Rc<Self>,
        root: &Path,
        path: &Path,
        line: Option<usize>,
        heading: Option<String>,
    ) {
        self.open_target(OpenTarget {
            root: root.to_owned(),
            path: path.to_owned(),
            line,
            heading,
            comment: None,
        });
    }
    pub fn open_comment(
        self: &Rc<Self>,
        root: &Path,
        path: &Path,
        anchor: studio_markdown_native::quote::Anchor,
        excerpt: &str,
    ) -> Result<(), String> {
        let line = anchor.start_line;
        let query = studio_markdown_native::quote::Query {
            anchor,
            quote: excerpt.into(),
        };
        query.validate()?;
        self.open_target(OpenTarget {
            root: root.to_owned(),
            path: path.to_owned(),
            line,
            heading: None,
            comment: Some(query),
        });
        Ok(())
    }
    fn open_target(self: &Rc<Self>, target: OpenTarget) {
        self.cancel_picker();
        if self.state.borrow().dirty {
            self.confirm_open(target);
            return;
        }
        if self.state.borrow().busy {
            return;
        }
        self.clear_flow_preview();
        let OpenTarget {
            root,
            path,
            line,
            heading,
            comment,
        } = target;
        let generation = self.state.borrow().generation + 1;
        self.changed_lines.borrow_mut().clear();
        *self.state.borrow_mut() = State {
            generation,
            busy: true,
            path: Some(path.to_owned()),
            root: Some(root.to_owned()),
            line: line.unwrap_or(1),
            line_reference: line.is_some() || heading.is_some(),
            initial_heading: heading,
            comment_kind: comment
                .as_ref()
                .map_or(studio_markdown_native::quote::Kind::Auto, |q| {
                    q.anchor.text_kind
                }),
            initial_comment: comment,
            ..State::default()
        };
        self.widget.set_visible(true);
        if let Some(callback) = self.on_open.borrow().as_ref() {
            callback();
        }
        crate::i18n_ui::literal(
            &self.title,
            "label",
            &path.file_name().unwrap_or_default().to_string_lossy(),
        );
        crate::i18n_ui::bind(&self.status, "label", "Opening and indexing…");
        self.blocks.remove_all();
        self.headings.remove_all();
        self.source_view.buffer().set_text("");
        self.request(
            "open",
            Request::Open {
                root: root.to_string_lossy().into_owned(),
                path: path.to_string_lossy().into_owned(),
                read_only: self.force_read_only.get(),
            },
        );
        self.update();
    }

    pub fn event(self: &Rc<Self>, response: Response) {
        if response.key == Self::flow_key(self.key) {
            self.flow_diagrams.event(response);
            return;
        }
        if response.key != self.key || response.generation != self.state.borrow().generation {
            return;
        }
        if response.kind == "diagram"
            && !self
                .state
                .borrow()
                .diagrams
                .values()
                .any(|d| d.serial.get() == response.serial)
        {
            return;
        }
        if response.kind == "table"
            && !self
                .state
                .borrow()
                .tables
                .values()
                .chain(self.flow_tables.borrow().values())
                .any(|t| t.active.get() && t.serial.get() == response.serial)
        {
            return;
        }
        if response.kind != "table"
            && response.kind != "diagram"
            && response.kind != "mutation"
            && self.state.borrow().latest.get(response.kind) != Some(&response.serial)
        {
            return;
        }
        let result = match response.result {
            Ok(result) => result,
            Err(error) => {
                if response.kind == "fragment" {
                    self.state.borrow_mut().preview_error = Some(error.clone());
                }
                if response.kind == "resume" {
                    self.state.borrow_mut().service_notice = Some("Document recovery failed. Save is disabled; your edits are kept in memory.");
                }
                if response.kind == "locate-quote" {
                    self.state.borrow_mut().locating_quote = false;
                    self.state.borrow_mut().comment_notice =
                        Some("Comment lookup failed. The saved location is unverified.");
                    let line = self.state.borrow().line;
                    self.state.borrow_mut().busy = false;
                    self.state.borrow_mut().mode = Mode::Source;
                    self.load_line(line);
                    self.error(&error);
                    return;
                }
                let mut state = self.state.borrow_mut();
                if response.kind == "diagram" {
                    for diagram in state.diagrams.values() {
                        if diagram.serial.get() == response.serial {
                            diagram.label.set_text(&error);
                        }
                    }
                } else if matches!(
                    response.kind,
                    "open" | "resume" | "index" | "fragment" | "source" | "edit" | "save"
                ) {
                    state.busy = false;
                    if response.kind == "source" {
                        state.source_caret = None;
                    }
                    if response.kind == "save" {
                        state.saving = false;
                    }
                    state.pending_navigation = None;
                    state.pending_close = false;
                    state.pending_quit = false;
                    state.pending_open = None;
                }
                drop(state);
                self.error(&error);
                self.update();
                return;
            }
        };
        match result {
            Reply::Resumed { reader_id, editor } => {
                self.session_valid.set(true);
                let (line, mode) = {
                    let mut state = self.state.borrow_mut();
                    state.reader_id = Some(reader_id);
                    state.editor = editor;
                    state.busy = false;
                    state.service_notice = None;
                    (state.line, state.mode)
                };
                if mode != Mode::Edit {
                    self.load_line(line);
                }
            }
            Reply::Open {
                read_only,
                reader_id,
            } => {
                self.session_valid.set(true);
                let mut state = self.state.borrow_mut();
                state.service_notice = None;
                state.read_only = read_only;
                state.reader_id = Some(reader_id);
                drop(state);
                self.request("index", Request::Index);
            }
            Reply::Index(None) => {
                let weak = Rc::downgrade(self);
                let generation = response.generation;
                glib::timeout_add_local_once(Duration::from_millis(120), move || {
                    if let Some(reader) = weak.upgrade() {
                        if reader.state.borrow().generation == generation {
                            reader.request("index", Request::Index);
                        }
                    }
                });
            }
            Reply::Index(Some(index)) => {
                let initial = self.state.borrow_mut().initial_heading.take();
                if let Some(initial) = initial {
                    if let Some(heading) = index.toc.iter().find(|heading| {
                        format!("heading-{}", heading.id) == initial
                            || crate::links::heading_slug(&heading.label) == initial
                    }) {
                        self.state.borrow_mut().line = heading.line;
                    }
                }
                {
                    let mut state = self.state.borrow_mut();
                    if state.line_reference {
                        if !index.toc.iter().any(|heading| heading.line <= state.line) {
                            state.mode = Mode::Source;
                        }
                        state.line_reference = false;
                    }
                }
                crate::i18n_ui::literal(&self.title, "label", &index.name);
                let headings = index
                    .toc
                    .iter()
                    .cloned()
                    .map(glib::BoxedAnyObject::new)
                    .collect::<Vec<_>>();
                self.headings.splice(0, self.headings.n_items(), &headings);
                let line = self.state.borrow().line;
                {
                    let mut state = self.state.borrow_mut();
                    if state
                        .index
                        .as_ref()
                        .is_none_or(|previous| previous.revision != index.revision)
                    {
                        state.paragraphs.clear();
                    }
                    state.index = Some(index);
                }
                self.state.borrow_mut().busy = false;
                self.rebuild_edit_marks();
                let comment = self.state.borrow_mut().initial_comment.take();
                if let Some(query) = comment {
                    let mut state = self.state.borrow_mut();
                    state.line = state
                        .line
                        .min(state.index.as_ref().unwrap().source_lines)
                        .max(1);
                    state.busy = true;
                    state.locating_quote = true;
                    drop(state);
                    crate::i18n_ui::bind(&self.status, "label", "Locating comment quote…");
                    if self
                        .request("locate-quote", Request::LocateQuote(query))
                        .is_none()
                    {
                        let line = self.state.borrow().line;
                        self.state.borrow_mut().busy = false;
                        self.state.borrow_mut().locating_quote = false;
                        self.state.borrow_mut().mode = Mode::Source;
                        self.state.borrow_mut().comment_notice =
                            Some("Comment lookup failed. The saved location is unverified.");
                        self.load_line(line);
                    }
                    self.update();
                    return;
                }
                let navigation = self.state.borrow_mut().pending_navigation.take();
                if let Some(navigation) = navigation {
                    self.navigate(navigation);
                } else {
                    self.load_line(line);
                }
            }
            Reply::LocatedQuote(located) => {
                use studio_markdown_native::quote::{Kind, Status};
                let mut state = self.state.borrow_mut();
                state.busy = false;
                state.locating_quote = false;
                state.comment_notice = match located.status {
                    Status::Unique => None,
                    Status::Missing => Some("Comment quote was not found. The saved location is unverified."),
                    Status::Ambiguous => Some("Comment quote matches multiple passages. The saved location is unverified."),
                    Status::Limited => Some("Comment lookup reached a rendering limit. The saved location is unverified."),
                };
                if let Some(position) = located.position {
                    state.line = position.line;
                }
                state.mode = if located.status != Status::Unique
                    || state.comment_kind == Kind::Source
                    || (state.comment_kind == Kind::Auto && located.text_kind == Some(Kind::Source))
                    || !state
                        .index
                        .as_ref()
                        .unwrap()
                        .toc
                        .iter()
                        .any(|h| h.line <= state.line)
                {
                    Mode::Source
                } else {
                    Mode::Preview
                };
                let line = state.line;
                drop(state);
                self.load_line_at(line, true);
            }
            Reply::SelectableFragment {
                page,
                line,
                prepared,
            } => {
                #[cfg(debug_assertions)]
                let mount_started = std::time::Instant::now();
                let anchor = {
                    let state = self.state.borrow();
                    if state.mode != Mode::Preview
                        || state
                            .index
                            .as_ref()
                            .is_none_or(|i| i.revision != prepared.revision)
                    {
                        return;
                    }
                    state.anchor_line
                };
                self.clear_flow_preview();
                {
                    let mut state = self.state.borrow_mut();
                    state.page = page;
                    state.line = if state.exact_preview { anchor } else { line };
                    state.busy = false;
                    state.tables.clear();
                    state.diagrams.clear();
                }
                self.blocks.remove_all();
                match self.flow_view(*prepared) {
                    Ok(view) => {
                        let offset = view
                            .document
                            .segments
                            .iter()
                            .find(|s| s.source.end_line >= anchor)
                            .map_or(0, |s| s.characters.start);
                        let buffer = view.widget.buffer();
                        buffer.place_cursor(&buffer.iter_at_offset(offset as i32));
                        self.preview_scroll.set_child(Some(&view.widget));
                        #[cfg(debug_assertions)]
                        view.mount_us.set(mount_started.elapsed().as_micros());
                        let widget = view.widget.clone();
                        self.flow_preview.replace(Some(view));
                        self.scroll_flow_after_layout(&widget);
                    }
                    Err(error) => self.error(&error),
                }
            }
            Reply::Fragment { page, line, blocks } => {
                self.clear_flow_preview();
                let anchor = self.state.borrow().anchor_line;
                let position = blocks
                    .iter()
                    .position(|row| row.block.source.end_line >= anchor)
                    .unwrap_or(0);
                let mut state = self.state.borrow_mut();
                state.page = page;
                state.line = if state.exact_preview { anchor } else { line };
                state.busy = false;
                state.tables.clear();
                if state.reset_paragraphs {
                    for row in &blocks {
                        state.paragraphs.pop(&row.block.source.start);
                    }
                    state.reset_paragraphs = false;
                }
                state.diagrams.clear();
                drop(state);
                let values = blocks
                    .into_iter()
                    .map(glib::BoxedAnyObject::new)
                    .collect::<Vec<_>>();
                self.blocks.splice(0, self.blocks.n_items(), &values);
                if self.blocks.n_items() > 0 {
                    let scroll = gtk::ScrollInfo::new();
                    scroll.set_enable_vertical(true);
                    self.preview.scroll_to(
                        position as u32,
                        gtk::ListScrollFlags::empty(),
                        Some(scroll),
                    );
                } else {
                    self.preview_scroll.vadjustment().set_value(0.0);
                }
            }
            Reply::Table(page) => {
                let table = self
                    .flow_tables
                    .borrow()
                    .get(&page.id)
                    .filter(|t| t.serial.get() == response.serial)
                    .cloned()
                    .or_else(|| self.state.borrow().tables.get(&page.id).cloned());
                if let Some(table) = table {
                    if table.active.get() && table.serial.get() == response.serial {
                        table.apply(page);
                    }
                }
            }
            Reply::Diagram { id, image } => {
                if let Some(diagram) = self.state.borrow().diagrams.get(&id) {
                    if diagram.serial.get() == response.serial {
                        diagram.picture.set_diagram(image);
                        diagram.picture.set_visible(true);
                        diagram.label.set_visible(false);
                    }
                }
            }
            Reply::Source(window) => {
                let target = self.state.borrow_mut().source_caret.take();
                let source_bytes = self
                    .state
                    .borrow()
                    .index
                    .as_ref()
                    .map_or(0, |i| i.source_bytes);
                let line = if let Some(line) = target {
                    if !window.contains_caret_line(line, source_bytes) {
                        // A saved edit may have grown beyond a source window's
                        // byte budget. Read the exact line once in that case.
                        self.load_line(line);
                        self.update();
                        return;
                    }
                    line
                } else {
                    window.line
                };
                self.apply_source_window(window, line);
            }
            Reply::Edit(page) => {
                self.edit_line.set(page.window.line);
                {
                    let mut state = self.state.borrow_mut();
                    state.loading_editor = true;
                    state.edit_bytes = page.window.text.len();
                }
                let buffer = self.edit_view.buffer();
                buffer.set_enable_undo(false);
                crate::source_surface::set_text(&buffer, &page.window.text);
                buffer.place_cursor(&buffer.start_iter());
                self.edit_view
                    .scroll_to_mark(&buffer.get_insert(), 0.0, true, 0.0, 0.0);
                buffer.set_enable_undo(true);
                buffer.set_modified(false);
                let mut state = self.state.borrow_mut();
                state.line = page.window.line;
                state.editor = Some(page);
                state.loading_editor = false;
                state.dirty = false;
                state.busy = false;
                drop(state);
                self.rebuild_edit_marks();
            }
            Reply::Saved(page) => {
                let warning = page.warning.clone();
                let (close, quit) = {
                    let mut state = self.state.borrow_mut();
                    state.busy = false;
                    state.saving = false;
                    state.dirty = false;
                    state.source = None;
                    // Save statistics belong to the new snapshot; the old
                    // shared index remains immutable until refresh completes.
                    crate::i18n_ui::bind_format(
                        &self.status,
                        "label",
                        "Saved {lines} lines · {bytes} bytes",
                        &[
                            ("lines", &page.source_lines.to_string()),
                            ("bytes", &page.source_bytes.to_string()),
                        ],
                    );
                    let old_end = state.editor.as_ref().map(|old| old.window.end);
                    if let Some(Navigation::Offset(offset, previous)) =
                        state.pending_navigation.as_mut()
                    {
                        if !*previous && Some(*offset) == old_end {
                            *offset = page.window.end;
                        }
                    }
                    state.editor = Some(page);
                    (
                        std::mem::take(&mut state.pending_close),
                        std::mem::take(&mut state.pending_quit),
                    )
                };
                self.edit_view.buffer().set_modified(false);
                if let Some(warning) = warning {
                    self.error(&warning);
                }
                if quit {
                    self.window.close();
                    return;
                }
                if close {
                    self.finish_close();
                    return;
                }
                let pending = self.state.borrow_mut().pending_open.take();
                if let Some(target) = pending {
                    self.open_target(target);
                    return;
                }
                // Read the new source index before cross-page navigation. Keep
                // the current GTK buffer and its undo history while it indexes.
                self.state.borrow_mut().busy = true;
                self.request("index", Request::Index);
            }
            Reply::Search(hits) => {
                while let Some(row) = self.search_rows.first_child() {
                    self.search_rows.remove(&row);
                }
                for hit in hits {
                    let button = gtk::Button::builder()
                        .child(
                            &gtk::Label::builder()
                                .label(format!("{} · {}", hit.line, hit.preview))
                                .wrap(true)
                                .xalign(0.0)
                                .build(),
                        )
                        .build();
                    button.add_css_class("flat");
                    let weak = Rc::downgrade(self);
                    button.connect_clicked(move |_| {
                        if let Some(reader) = weak.upgrade() {
                            reader.navigate(Navigation::Line(hit.line));
                        }
                    });
                    self.search_rows.append(&button);
                }
            }
            Reply::Ack => {}
        }
        self.update();
    }

    fn apply_source_window(&self, window: SourceWindow, line: usize) {
        self.source_line.set(window.line);
        let buffer = self.source_view.buffer();
        crate::source_surface::set_text(&buffer, &window.text);
        // Explicitly place the mark: set_text otherwise leaves it at EOF. A
        // retained window keeps context above the caret, including at EOF.
        let cursor = buffer
            .iter_at_line(line.saturating_sub(window.line).min(i32::MAX as usize) as i32)
            .unwrap_or_else(|| buffer.start_iter());
        buffer.place_cursor(&cursor);
        self.source_view
            .scroll_to_mark(&buffer.get_insert(), 0.0, true, 0.0, 0.0);
        let mut state = self.state.borrow_mut();
        state.line = window.line + cursor.line() as usize;
        state.source_caret = None;
        state.source = Some(window);
        state.busy = false;
        drop(state);
        self.highlight_source();
    }

    fn set_mode(self: &Rc<Self>, mode: Mode) {
        if self.state.borrow().mode == mode {
            return;
        }
        if self.state.borrow().busy {
            self.sync_modes();
            return;
        }
        let line = self.current_line();
        let (source, source_context) = if mode == Mode::Source {
            let state = self.state.borrow();
            state.index.as_ref().map_or((None, None), |index| {
                let line = line.clamp(1, index.source_lines.max(1));
                // A long physical line may span several byte windows. Its line
                // number alone cannot identify the current editor's context.
                let editing = (state.mode == Mode::Edit)
                    .then_some(state.editor.as_ref())
                    .flatten();
                let cached = state
                    .source
                    .as_ref()
                    .filter(|window| {
                        window.contains_caret_line(line, index.source_bytes)
                            && (!window.text.is_empty() || index.source_bytes == 0)
                            && editing.is_none_or(|page| {
                                window.start < page.window.end && page.window.start < window.end
                            })
                    })
                    .map(|window| (window.clone(), line));
                // Edit text normalizes CRLF. Reuse its saved bounds, never its
                // text, when raw Source is absent. Require the same snapshot.
                let edited = editing
                    .filter(|page| page.revision == index.revision)
                    .map(|page| &page.window);
                let context = edited
                    .filter(|window| window.contains_caret_line(line, index.source_bytes))
                    .map(|window| {
                        let previous =
                            line == index.source_lines && window.end == index.source_bytes;
                        (
                            if previous { window.end } else { window.start },
                            previous,
                            line,
                        )
                    });
                (cached, context)
            })
        } else {
            (None, None)
        };
        if mode != Mode::Preview {
            self.clear_flow_preview();
        }
        self.state.borrow_mut().mode = mode;
        if let Some(scroll) = self
            .stack
            .visible_child()
            .and_downcast::<gtk::ScrolledWindow>()
        {
            // End any in-flight scroll before unmapping the old mode. GTK
            // 4.22's animation teardown can reenter a value-changed handler
            // while detaching its frame clock and disconnect handler zero.
            // Setting the existing value uses GTK's public, nonanimated path;
            // normal scrolling and animations in the new mode remain enabled.
            for adjustment in [scroll.hadjustment(), scroll.vadjustment()] {
                adjustment.set_value(adjustment.value());
            }
        }
        self.stack.set_visible_child_name(mode.name());
        let keep_editor = {
            let state = self.state.borrow();
            state.editor.as_ref().is_some_and(|page| {
                state.dirty
                    || state.index.as_ref().is_some_and(|index| {
                        page.revision == index.revision
                            && page.window.contains_caret_line(line, index.source_bytes)
                    })
            })
        };
        if let Some((window, line)) = source {
            self.apply_source_window(window, line);
        } else if let Some((offset, previous, line)) = source_context {
            let mut state = self.state.borrow_mut();
            state.busy = true;
            state.source_caret = Some(line);
            drop(state);
            crate::i18n_ui::bind(&self.status, "label", "Loading page…");
            self.request(
                "source",
                Request::Source {
                    line: 1,
                    offset: Some(offset),
                    previous,
                },
            );
        } else if mode != Mode::Edit || !keep_editor {
            self.load_line(line);
        }
        self.update();
    }

    fn confirm_open(self: &Rc<Self>, target: OpenTarget) {
        let dialog = adw::MessageDialog::builder()
            .transient_for(&self.window)
            .heading("Save changes before opening another document?")
            .body("The current edit page has unsaved changes.")
            .build();
        dialog.add_responses(&[
            ("cancel", "Cancel"),
            ("discard", "Discard"),
            ("save", "Save"),
        ]);
        dialog.set_close_response("cancel");
        dialog.set_default_response(Some("save"));
        dialog.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
        dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
        dialog.set_response_enabled("save", !self.state.borrow().busy);
        let generation = self.state.borrow().generation;
        let target = Rc::new(RefCell::new(Some(target)));
        let weak = Rc::downgrade(self);
        dialog.connect_response(None, move |_, response| {
            let Some(reader) = weak.upgrade() else { return };
            if reader.state.borrow().generation != generation || reader.state.borrow().saving {
                return;
            }
            let Some(target) = target.borrow_mut().take() else {
                return;
            };
            match response {
                "discard" => {
                    reader.state.borrow_mut().dirty = false;
                    reader.open_target(target);
                }
                "save" => {
                    reader.state.borrow_mut().pending_open = Some(target);
                    reader.save_page();
                }
                _ => {}
            }
        });
        dialog.present();
    }

    fn current_line(&self) -> usize {
        let state = self.state.borrow();
        if state.mode == Mode::Preview {
            if let Some(view) = self.flow_preview.borrow().as_ref() {
                let buffer = view.widget.buffer();
                let cursor = buffer.iter_at_mark(&buffer.get_insert());
                let at = cursor.offset() as usize;
                let index = view
                    .document
                    .segments
                    .partition_point(|s| s.characters.start <= at)
                    .saturating_sub(1);
                if let Some(segment) = view.document.segments.get(index) {
                    let start = buffer.iter_at_offset(segment.characters.start as i32);
                    return (segment.source.line
                        + cursor.line().saturating_sub(start.line()) as usize)
                        .min(segment.source.end_line.max(segment.source.line));
                }
            }
        }
        let view = match state.mode {
            Mode::Source => Some((&self.source_view, self.source_line.get())),
            Mode::Edit => Some((&self.edit_view, self.edit_line.get())),
            _ => None,
        };
        if let Some((view, base)) = view {
            let buffer = view.buffer();
            let iter = buffer.iter_at_mark(&buffer.get_insert());
            base + iter.line() as usize
        } else {
            state.line
        }
    }

    fn rebuild_edit_marks(&self) {
        let buffer = self.edit_view.buffer();
        let mut state = self.state.borrow_mut();
        if state.dirty {
            return;
        }
        for (_, mark) in state.edit_marks.drain(..) {
            if !mark.is_deleted() {
                buffer.delete_mark(&mark);
            }
        }
        let Some(page) = &state.editor else { return };
        let Some(index) = &state.index else { return };
        let marks = index
            .toc
            .iter()
            .filter(|heading| {
                heading.line >= page.window.line && heading.line <= page.window.end_line
            })
            .filter_map(|heading| {
                buffer
                    .iter_at_line((heading.line - page.window.line) as i32)
                    .map(|iter| (heading.line, buffer.create_mark(None, &iter, false)))
            })
            .collect();
        state.edit_marks = marks;
    }

    fn navigate(self: &Rc<Self>, navigation: Navigation) {
        if self.state.borrow().busy {
            return;
        }
        if let Navigation::Line(line) = &navigation {
            let state = self.state.borrow();
            if state.mode == Mode::Edit
                && state
                    .editor
                    .as_ref()
                    .is_some_and(|page| *line >= page.window.line && *line <= page.window.end_line)
            {
                let buffer = self.edit_view.buffer();
                let mut iter = state
                    .edit_marks
                    .iter()
                    .find(|(original, _)| original == line)
                    .map(|(_, mark)| buffer.iter_at_mark(mark))
                    .or_else(|| {
                        buffer.iter_at_line(line.saturating_sub(self.edit_line.get()) as i32)
                    });
                drop(state);
                self.state.borrow_mut().reset_paragraphs = true;
                if let Some(iter) = iter.as_mut() {
                    buffer.place_cursor(iter);
                    self.edit_view.scroll_to_iter(iter, 0.15, false, 0.0, 0.0);
                }
                return;
            }
        }
        if self.state.borrow().dirty {
            self.state.borrow_mut().pending_navigation = Some(navigation);
            self.save_page();
            return;
        }
        // An explicit outline/search/page jump starts its destination context;
        // an ordinary Source/Preview switch calls load_line directly and keeps
        // the same revision's per-paragraph reading positions.
        self.state.borrow_mut().reset_paragraphs = true;
        match navigation {
            Navigation::Page(page) => {
                let mut state = self.state.borrow_mut();
                state.busy = true;
                state.exact_preview = false;
                state.anchor_line = state
                    .index
                    .as_ref()
                    .and_then(|index| index.pages.get(page))
                    .map_or(1, |page| page.line);
                drop(state);
                self.request("fragment", self.preview_request(page));
                self.update();
            }
            Navigation::Line(line) => {
                // An existing edit page can use live GTK marks/line positions;
                // crossing its range goes through the save boundary above.
                let in_page = self.state.borrow().mode == Mode::Edit
                    && self.state.borrow().editor.as_ref().is_some_and(|page| {
                        line >= page.window.line && line <= page.window.end_line
                    });
                if in_page {
                    let offset = line - self.edit_line.get();
                    let buffer = self.edit_view.buffer();
                    if let Some(mut iter) = buffer.iter_at_line(offset as i32) {
                        buffer.place_cursor(&iter);
                        self.edit_view
                            .scroll_to_iter(&mut iter, 0.15, false, 0.0, 0.0);
                    }
                } else {
                    self.load_line(line);
                }
            }
            Navigation::Offset(offset, previous) => {
                let mode = self.state.borrow().mode;
                let request = if mode == Mode::Edit {
                    Request::Edit {
                        line: 1,
                        offset: Some(offset),
                        previous,
                    }
                } else {
                    Request::Source {
                        line: 1,
                        offset: Some(offset),
                        previous,
                    }
                };
                let kind = if mode == Mode::Edit { "edit" } else { "source" };
                self.state.borrow_mut().busy = true;
                self.request(kind, request);
                self.update();
            }
        }
    }

    fn load_line(self: &Rc<Self>, line: usize) {
        self.load_line_at(line, false);
    }
    fn load_line_at(self: &Rc<Self>, line: usize, exact_preview: bool) {
        let (mode, line, page, anchor) = {
            let state = self.state.borrow();
            let Some(index) = &state.index else { return };
            let line = line.clamp(1, index.source_lines.max(1));
            let anchor = if state.mode == Mode::Preview && !exact_preview {
                index
                    .toc
                    .iter()
                    .rev()
                    .find(|heading| heading.line <= line)
                    .map_or(line, |heading| heading.line)
            } else {
                line
            };
            let page = index
                .pages
                .partition_point(|page| page.line <= anchor)
                .saturating_sub(1);
            (state.mode, line, page, anchor)
        };
        let keep_editor = mode == Mode::Edit && {
            let state = self.state.borrow();
            state
                .index
                .as_ref()
                .zip(state.editor.as_ref())
                .is_some_and(|(index, page)| {
                    page.revision == index.revision
                        && page.window.contains_caret_line(line, index.source_bytes)
                })
        };
        if keep_editor {
            self.stack.set_visible_child_name("edit");
            return;
        }
        self.state.borrow_mut().busy = true;
        self.state.borrow_mut().exact_preview = exact_preview && mode == Mode::Preview;
        self.state.borrow_mut().anchor_line = anchor;
        crate::i18n_ui::bind(&self.status, "label", "Loading page…");
        match mode {
            Mode::Preview => {
                self.request("fragment", self.preview_request(page));
            }
            Mode::Source => {
                self.state.borrow_mut().source_caret = None;
                self.request(
                    "source",
                    Request::Source {
                        line,
                        offset: None,
                        previous: false,
                    },
                );
            }
            Mode::Edit => {
                self.request(
                    "edit",
                    Request::Edit {
                        line,
                        offset: None,
                        previous: false,
                    },
                );
            }
        }
        self.stack.set_visible_child_name(mode.name());
        self.update();
    }

    fn page(self: &Rc<Self>, forward: bool) {
        let navigation = {
            let state = self.state.borrow();
            if state.busy {
                return;
            }
            if state.mode == Mode::Preview {
                let Some(index) = &state.index else { return };
                let Some(current) = index.pages.get(state.page) else {
                    return;
                };
                let page = if let Some(table) = current.table {
                    let table = &index.tables[table];
                    if forward {
                        table.end_page
                    } else {
                        table.first_page.saturating_sub(1)
                    }
                } else if forward {
                    state.page + 1
                } else {
                    state.page.saturating_sub(1)
                };
                let Some(target) = index.pages.get(page) else {
                    return;
                };
                let _ = target;
                Navigation::Page(page)
            } else {
                let window = if state.mode == Mode::Edit {
                    state.editor.as_ref().map(|page| &page.window)
                } else {
                    state.source.as_ref()
                };
                let Some(window) = window else { return };
                Navigation::Offset(if forward { window.end } else { window.start }, !forward)
            }
        };
        self.navigate(navigation);
    }

    fn save_page(self: &Rc<Self>) {
        if self.state.borrow().busy || self.state.borrow().editor.is_none() {
            return;
        }
        let buffer = self.edit_view.buffer();
        let text = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), true)
            .to_string();
        if self.request("save", Request::Save { text }).is_some() {
            self.state.borrow_mut().busy = true;
            self.state.borrow_mut().saving = true;
            crate::i18n_ui::bind(&self.status, "label", "Saving current page…");
            self.update();
        }
    }

    fn close(self: &Rc<Self>) {
        if self.state.borrow().saving {
            return;
        }
        if !self.state.borrow().dirty {
            self.finish_close();
            return;
        }
        let dialog = adw::MessageDialog::builder()
            .transient_for(&self.window)
            .heading("Save changes?")
            .body("The current edit page has unsaved changes.")
            .build();
        dialog.add_responses(&[
            ("cancel", "Cancel"),
            ("discard", "Discard"),
            ("save", "Save"),
        ]);
        dialog.set_close_response("cancel");
        dialog.set_default_response(Some("save"));
        dialog.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
        dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
        dialog.set_response_enabled("save", !self.state.borrow().busy);
        let weak = Rc::downgrade(self);
        dialog.connect_response(None, move |_, response| {
            if let Some(reader) = weak.upgrade() {
                match response {
                    "discard" => reader.finish_close(),
                    "save" => {
                        reader.state.borrow_mut().pending_close = true;
                        reader.save_page();
                    }
                    _ => {}
                }
            }
        });
        dialog.present();
    }

    pub fn allow_window_close(self: &Rc<Self>) -> bool {
        self.cancel_picker();
        if self.state.borrow().saving {
            self.error("Wait for the current document operation before closing");
            return false;
        }
        if !self.state.borrow().dirty {
            return true;
        }
        let dialog = adw::MessageDialog::builder()
            .transient_for(&self.window)
            .heading("Save changes before closing?")
            .body("The current edit page has unsaved changes.")
            .build();
        dialog.add_responses(&[
            ("cancel", "Cancel"),
            ("discard", "Discard"),
            ("save", "Save"),
        ]);
        dialog.set_close_response("cancel");
        dialog.set_default_response(Some("save"));
        dialog.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
        dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
        dialog.set_response_enabled("save", !self.state.borrow().busy);
        let weak = Rc::downgrade(self);
        dialog.connect_response(None, move |_, response| {
            if let Some(reader) = weak.upgrade() {
                match response {
                    "discard" => {
                        reader.state.borrow_mut().dirty = false;
                        reader.window.close();
                    }
                    "save" => {
                        reader.state.borrow_mut().pending_quit = true;
                        reader.save_page();
                    }
                    _ => {}
                }
            }
        });
        dialog.present();
        false
    }

    fn finish_close(&self) {
        if let Some(query) = self.sql_query.borrow_mut().take() {
            query.close();
        }
        if let Some(query) = self.query.borrow_mut().take() {
            query.close();
        }
        self.clear_flow_preview();
        self.cancel_picker();
        self.request("close", Request::Close);
        if let Some(callback) = self.on_closing.borrow().as_ref() {
            callback();
        }
        self.widget.set_visible(false);
        self.blocks.remove_all();
        self.headings.remove_all();
        let generation = self.state.borrow().generation + 1;
        *self.state.borrow_mut() = State {
            generation,
            ..State::default()
        };
        if let Some(callback) = self.on_close.borrow().as_ref() {
            callback();
        }
    }

    fn reload(self: &Rc<Self>) {
        if self.state.borrow().dirty {
            self.error("Save or discard the edit page before reloading");
            return;
        }
        if self.state.borrow().busy {
            return;
        }
        let path = self.state.borrow().path.clone();
        let root = self.state.borrow().root.clone();
        let line = self.current_line();
        if let (Some(path), Some(root)) = (path, root) {
            self.open_project_path(&root, &path, None, None);
            self.state.borrow_mut().line = line;
        }
    }
    fn toggle_outline(self: &Rc<Self>) {
        self.outline.set_reveal_child(!self.outline.reveals_child());
    }

    pub fn activate_link(self: &Rc<Self>, reference: &str) {
        let (root, path) = {
            let state = self.state.borrow();
            (state.root.clone(), state.path.clone())
        };
        let Some(root) = root else { return };
        match crate::links::resolve(reference, &root, path.as_deref()) {
            Ok(crate::links::Target::Heading(fragment)) => {
                let line = self.state.borrow().index.as_ref().and_then(|index| {
                    index
                        .toc
                        .iter()
                        .find(|heading| {
                            format!("heading-{}", heading.id) == fragment
                                || crate::links::heading_slug(&heading.label) == fragment
                        })
                        .map(|heading| heading.line)
                });
                if let Some(line) = line {
                    self.navigate(Navigation::Line(line));
                }
            }
            Ok(crate::links::Target::Document {
                path,
                line,
                heading,
            }) => {
                if path
                    .extension()
                    .and_then(|value| value.to_str())
                    .is_some_and(|extension| {
                        matches!(
                            extension.to_ascii_lowercase().as_str(),
                            "md" | "markdown" | "mdown"
                        )
                    })
                {
                    self.open_project_path(&root, &path, line, heading);
                } else if let Some(callback) = self.on_file.borrow().as_ref() {
                    let line = if crate::pdf::supports(&path) {
                        heading
                            .as_deref()
                            .and_then(|value| value.strip_prefix("page="))
                            .and_then(|value| value.parse::<usize>().ok())
                            .or(line)
                    } else {
                        line
                    };
                    callback(&root.to_string_lossy(), &path.to_string_lossy(), line);
                }
            }
            Ok(crate::links::Target::External(uri)) => {
                if let Err(error) =
                    gio::AppInfo::launch_default_for_uri(&uri, gio::AppLaunchContext::NONE)
                {
                    self.error(&error.to_string());
                }
            }
            Err(error) => self.error(&error),
        }
    }

    fn label_link(self: &Rc<Self>, label: &gtk::Label) {
        let weak = Rc::downgrade(self);
        label.connect_activate_link(move |_, uri| {
            if let Some(reader) = weak.upgrade() {
                reader.activate_link(uri);
            }
            glib::Propagation::Stop
        });
    }
    fn error(&self, error: &str) {
        self.toast.add_toast(adw::Toast::new(error));
        crate::i18n_ui::literal(&self.status, "label", error);
    }
    fn sync_modes(&self) {
        let mode = self.state.borrow().mode;
        for (button, value) in self
            .modes
            .iter()
            .zip([Mode::Preview, Mode::Source, Mode::Edit])
        {
            button.set_active(mode == value);
        }
    }
    fn open_table_query(&self, table: usize) {
        let state = self.state.borrow();
        let Some(index) = state
            .index
            .as_ref()
            .filter(|index| table < index.tables.len())
        else {
            return;
        };
        if !self.gateway_available.get() || !self.session_valid.get() || state.busy || state.dirty {
            return;
        }
        let context = crate::reader_query::Context {
            key: self.key,
            generation: state.generation,
            revision: index.revision.clone(),
        };
        if let Some(query) =
            self.sql_query.borrow().as_ref().filter(|query| {
                query.context == context && query.table == table && !query.is_closed()
            })
        {
            query.present();
            return;
        }
        let name = index.name.clone();
        drop(state);
        if let Some(query) = self.sql_query.borrow_mut().take() {
            query.close();
        }
        self.sql_query.replace(Some(crate::reader_sql_ui::Sql::new(
            &self.window,
            self.sender.clone(),
            context,
            table,
            &name,
        )));
    }

    fn open_query(&self) {
        let state = self.state.borrow();
        let Some(index) = state
            .index
            .as_ref()
            .filter(|index| crate::reader_query::jq_supported(&index.name))
        else {
            return;
        };
        if !self.gateway_available.get() || !self.session_valid.get() || state.busy || state.dirty {
            return;
        }
        let context = crate::reader_query::Context {
            key: self.key,
            generation: state.generation,
            revision: index.revision.clone(),
        };
        if let Some(query) = self
            .query
            .borrow()
            .as_ref()
            .filter(|query| query.context == context && !query.is_closed())
        {
            query.present();
            return;
        }
        let name = index.name.clone();
        drop(state);
        if let Some(old) = self.query.borrow_mut().take() {
            old.close();
        }
        let query =
            crate::reader_query_ui::Jq::new(&self.window, self.sender.clone(), context, &name);
        self.query.replace(Some(query));
    }

    fn update(&self) {
        let state = self.state.borrow();
        let available = self.gateway_available.get() && self.session_valid.get();
        let close_sql = self.sql_query.borrow().as_ref().is_some_and(|query| {
            !available
                || state.dirty
                || query.context.generation != state.generation
                || state
                    .index
                    .as_ref()
                    .is_none_or(|index| query.context.revision != index.revision)
        });
        if close_sql {
            if let Some(query) = self.sql_query.borrow_mut().take() {
                query.close();
            }
        }
        let query_supported = state
            .index
            .as_ref()
            .is_some_and(|index| crate::reader_query::jq_supported(&index.name));
        self.query_button.set_visible(query_supported);
        self.query_button
            .set_sensitive(available && query_supported && !state.busy && !state.dirty);
        let close_query = self.query.borrow().as_ref().is_some_and(|query| {
            !available
                || state.dirty
                || query.context.generation != state.generation
                || state
                    .index
                    .as_ref()
                    .is_none_or(|index| query.context.revision != index.revision)
        });
        if close_query {
            if let Some(query) = self.query.borrow_mut().take() {
                query.close();
            }
        }
        self.open.set_sensitive(
            self.gateway_available.get() && !state.busy && !self.picker.is_pending(),
        );
        let loaded = state.index.is_some();
        self.save
            .set_sensitive(available && !state.busy && state.dirty);
        self.edit_view
            .set_editable(!state.busy && loaded && !state.read_only);
        let dirty_notice = state.dirty && state.mode != Mode::Edit;
        let preview_error = (state.mode == Mode::Preview)
            .then_some(state.preview_error.as_deref())
            .flatten();
        let notice = state.service_notice.or(preview_error).or(if dirty_notice {
            Some("Unsaved edits are kept in memory. Save to refresh Preview and Source.")
        } else {
            state.comment_notice
        });
        if let Some(text) = notice {
            if state.service_notice.is_none() && preview_error.is_some() {
                crate::i18n_ui::literal(&self.notice, "label", text);
            } else {
                crate::i18n_ui::bind(&self.notice, "label", text);
            }
        }
        self.notice.set_visible(notice.is_some());
        for button in &self.modes {
            button.set_sensitive(available && loaded && !state.busy);
        }
        self.modes[2].set_sensitive(available && loaded && !state.busy && !state.read_only);
        let (before, after) = if let Some(index) = &state.index {
            if !state.busy {
                let line = state.line;
                crate::i18n_ui::bind_summary(
                    &self.status,
                    "label",
                    "{lines} lines · {size} MB · Line {line}",
                    &[
                        ("lines", &index.source_lines.to_string()),
                        (
                            "size",
                            &format!("{:.1}", index.source_bytes as f64 / 1_000_000.0),
                        ),
                        ("line", &line.to_string()),
                    ],
                    if state.dirty { &["Unsaved"] } else { &[] },
                );
            }
            if state.mode == Mode::Preview {
                if let Some(table) = index
                    .pages
                    .get(state.page)
                    .and_then(|page| page.table)
                    .and_then(|id| index.tables.get(id))
                {
                    (table.first_page > 0, table.end_page < index.pages.len())
                } else {
                    (state.page > 0, state.page + 1 < index.pages.len())
                }
            } else {
                let window = if state.mode == Mode::Edit {
                    state.editor.as_ref().map(|page| &page.window)
                } else {
                    state.source.as_ref()
                };
                window.map_or((false, false), |window| {
                    (window.start > 0, window.end < index.source_bytes)
                })
            }
        } else {
            (false, false)
        };
        self.previous
            .set_sensitive(available && before && !state.busy);
        self.next.set_sensitive(available && after && !state.busy);
        drop(state);
        self.sync_modes();
        if let Some(callback) = self.on_changed.borrow().as_ref() {
            callback();
        }
    }

    fn update_style(self: &Rc<Self>) {
        let id = if adw::StyleManager::default().is_dark() {
            "Adwaita-dark"
        } else {
            "Adwaita"
        };
        let scheme = sourceview::StyleSchemeManager::default().scheme(id);
        for view in [&self.source_view, &self.edit_view] {
            if let Ok(buffer) = view.buffer().downcast::<sourceview::Buffer>() {
                buffer.set_style_scheme(scheme.as_ref());
            }
        }
        let diagrams = self
            .state
            .borrow()
            .diagrams
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for diagram in diagrams {
            diagram.load(self);
        }
    }
    fn diagram_preferences_changed(&self) {
        let diagrams = self
            .state
            .borrow()
            .diagrams
            .values()
            .filter(|d| d.kind == "mermaid")
            .cloned()
            .collect::<Vec<_>>();
        for diagram in diagrams {
            self.request("diagram-cancel", Request::CancelDiagram { id: diagram.id });
            diagram.serial.set(0);
            diagram.picture.clear_texture();
            if diagram.widget.is_mapped() {
                diagram.load(self);
            }
        }
    }

    #[cfg(debug_assertions)]
    pub async fn smoke_comment(self: &Rc<Self>, source: bool) -> Result<(), String> {
        let start = std::time::Instant::now();
        while !self.is_ready() {
            if start.elapsed() > Duration::from_secs(15) {
                return Err("Comment document did not open".into());
            }
            glib::timeout_future(Duration::from_millis(25)).await;
        }
        if source {
            self.set_mode(Mode::Source);
            let start = std::time::Instant::now();
            while !self
                .source_view
                .buffer()
                .text(
                    &self.source_view.buffer().start_iter(),
                    &self.source_view.buffer().end_iter(),
                    false,
                )
                .contains("Source **中文😀** selection.")
            {
                if start.elapsed() > Duration::from_secs(10) {
                    return Err("Source comment window did not load".into());
                }
                glib::timeout_future(Duration::from_millis(25)).await;
            }
            let buffer = self.source_view.buffer();
            let begin = buffer.iter_at_line(2).ok_or("Source line missing")?;
            let mut end = begin;
            end.forward_to_line_end();
            buffer.select_range(&begin, &end);
        } else {
            self.set_mode(Mode::Preview);
            let start = std::time::Instant::now();
            loop {
                fn selected(widget: &gtk::Widget) -> bool {
                    if let Some(label) = widget.downcast_ref::<gtk::Label>() {
                        if label.text() == "Source 中文😀 selection." {
                            label.select_region(0, -1);
                            return true;
                        }
                    }
                    let mut child = widget.first_child();
                    while let Some(c) = child {
                        if selected(&c) {
                            return true;
                        }
                        child = c.next_sibling();
                    }
                    false
                }
                if selected(self.preview.upcast_ref()) {
                    break;
                }
                if start.elapsed() > Duration::from_secs(10) {
                    return Err("Rendered comment paragraph missing".into());
                }
                glib::timeout_future(Duration::from_millis(25)).await;
            }
        }
        self.comment_selection();
        Ok(())
    }
    fn capture(
        &self,
        excerpt: String,
        line: usize,
        end: usize,
    ) -> Option<crate::annotations::Capture> {
        if excerpt.trim().is_empty() || excerpt.len() > 16384 {
            return None;
        }
        let state = self.state.borrow();
        let (root, path, index) = (
            state.root.as_ref()?,
            state.path.as_ref()?,
            state.index.as_ref()?,
        );
        Some(crate::annotations::Capture {
            excerpt,
            source: serde_json::json!({"provider":"document","version":1,"anchor":{"filePath":path,"root":root,"baseHash":index.revision,"startLine":line,"endLine":end,"textKind":if state.mode == Mode::Preview {"preview"} else {"source"}}}),
        })
    }
    /// Document-bound callbacks for the continuous-reader adapter. The current
    /// production renderer remains unchanged until the remaining parity gates.
    pub(super) fn flow_options(self: &Rc<Self>) -> Option<crate::text_flow::Options> {
        let epoch = self.flow_epoch.get();
        let (generation, revision) = {
            let state = self.state.borrow();
            (state.generation, state.index.as_ref()?.revision.clone())
        };
        let weak = Rc::downgrade(self);
        let expected = revision.clone();
        let current: Rc<dyn Fn() -> bool> = Rc::new(move || {
            weak.upgrade().is_some_and(|reader| {
                let state = reader.state.borrow();
                state.generation == generation
                    && reader.flow_epoch.get() == epoch
                    && state.mode == Mode::Preview
                    && !state.busy
                    && !state.dirty
                    && !state.saving
                    && state.index.as_ref().is_some_and(|i| i.revision == expected)
            })
        });
        let configure = {
            let (weak, current) = (Rc::downgrade(self), current.clone());
            Rc::new(
                move |widget: &gtk::Widget, source: &studio_markdown_native::SourceRange| {
                    if current() {
                        if let Some(reader) = weak.upgrade() {
                            reader.track_selection(widget, source);
                        }
                    }
                },
            ) as crate::text_flow::ConfigureSelection
        };
        let begin = {
            let (weak, current) = (Rc::downgrade(self), current.clone());
            Rc::new(move || {
                let reader = weak.upgrade()?;
                if !current() {
                    return None;
                }
                let target = reader.comments.borrow().clone()?;
                let Some(owner) = (target.owner)() else {
                    reader.error("Select a session before adding comments");
                    return None;
                };
                let (weak, current) = (weak.clone(), current.clone());
                Some(Rc::new(
                    move |selected: Result<studio_markdown_native::flow::Selection, String>| {
                        if !current() {
                            return;
                        }
                        let Some(reader) = weak.upgrade() else {
                            return;
                        };
                        if selected.as_ref().is_err_and(|s| {
                            s == "Selection workers are busy; try again"
                                || s == crate::text_flow::INDEXED_SELECTION
                        }) {
                            reader.error(&crate::language::tr(selected.as_ref().unwrap_err()));
                            return;
                        }
                        let capture = selected.ok().and_then(|s| {
                            let source = s.source?;
                            let mut capture =
                                reader.capture(s.text, source.line, source.end_line)?;
                            capture.source["anchor"]["textKind"] = "flow".into();
                            Some(capture)
                        });
                        if let Some(capture) = capture {
                            match (target.capture)(owner.clone(), capture) {
                                Ok(()) => reader.update(),
                                Err(error) => reader.error(&error),
                            }
                        } else {
                            reader.error("Select a passage of at most 16 KiB in Preview, Source or saved Edit before commenting");
                        }
                    },
                ) as crate::text_flow::SelectionSink)
            }) as Rc<dyn Fn() -> Option<crate::text_flow::SelectionSink>>
        };
        Some(crate::text_flow::Options {
            open: self.cell_link(),
            images: self.table_images(),
            indexed_tables: Some({
                let epoch = self.flow_epoch.get();
                let owner = Rc::downgrade(self);
                let snapshot = self.snapshot();
                Rc::new(move |id, row| {
                    let reader = owner
                        .upgrade()
                        .filter(|r| {
                            let state = r.state.borrow();
                            state.generation == generation
                                && r.flow_epoch.get() == epoch
                                && state.mode == Mode::Preview
                                && !state.busy
                                && r.snapshot() == snapshot
                                && state
                                    .index
                                    .as_ref()
                                    .and_then(|i| i.tables.get(id))
                                    .is_some_and(|t| row < t.rows || (row == 0 && t.rows == 0))
                        })
                        .ok_or("Document changed before table mounting")?;
                    if reader.flow_tables.borrow().contains_key(&id) {
                        return Err("Indexed table already belongs to another flow view".into());
                    }
                    let table = TableUi::new(&reader, id);
                    reader.flow_tables.borrow_mut().insert(id, table.clone());
                    table.defer_rows();
                    table.load(row);
                    Ok(table.widget.clone().upcast())
                })
            }),
            selection_tables: Some({
                let epoch = self.flow_epoch.get();
                let snapshot = self.snapshot();
                let owner = Rc::downgrade(self);
                let current = Rc::new(move || {
                    owner.upgrade().is_some_and(|r| {
                        let state = r.state.borrow();
                        state.generation == generation
                            && r.flow_epoch.get() == epoch
                            && state.mode == Mode::Preview
                            && !state.busy
                            && r.snapshot() == snapshot
                    })
                });
                let owner = Rc::downgrade(self);
                let snapshot = self.snapshot();
                let resolve = Rc::new(move |ids, limit| {
                    let (reply, receive) = tokio::sync::oneshot::channel();
                    let sent = owner.upgrade().zip(snapshot.clone()).is_some_and(
                        |(r, (reader, revision))| {
                            r.request(
                                "selection-tables",
                                Request::SelectionTables {
                                    reader,
                                    revision,
                                    ids,
                                    limit,
                                    reply,
                                },
                            )
                            .is_some()
                        },
                    );
                    Box::pin(async move {
                        if !sent {
                            return Err("Selection request queue is unavailable".into());
                        }
                        receive
                            .await
                            .map_err(|_| "Selection read stopped".to_owned())?
                    })
                        as futures_util::future::LocalBoxFuture<
                            'static,
                            Result<Vec<studio_markdown_native::flow::TableText>, String>,
                        >
                });
                crate::text_flow::SelectionTables {
                    prefix: Some({
                        let owner = Rc::downgrade(self);
                        let snapshot = self.snapshot();
                        Rc::new(move |id, characters| {
                            let (reply, receive) = tokio::sync::oneshot::channel();
                            let sent = owner.upgrade().zip(snapshot.clone()).is_some_and(
                                |(r, (reader, revision))| {
                                    r.request(
                                        "selection-prefix",
                                        Request::SelectionPrefix {
                                            reader,
                                            revision,
                                            id,
                                            characters,
                                            reply,
                                        },
                                    )
                                    .is_some()
                                },
                            );
                            Box::pin(async move {
                                if !sent {
                                    return Err("Selection request queue is unavailable".into());
                                }
                                receive
                                    .await
                                    .map_err(|_| "Selection read stopped".to_owned())?
                            })
                                as futures_util::future::LocalBoxFuture<
                                    'static,
                                    Result<studio_markdown_native::flow::TablePrefix, String>,
                                >
                        })
                    }),
                    current,
                    resolve,
                    cancel: gio::Cancellable::new(),
                }
            }),
            selection_error: Some({
                let owner = Rc::downgrade(self);
                Rc::new(move |error| {
                    if let Some(reader) = owner
                        .upgrade()
                        .filter(|r| r.state.borrow().generation == generation)
                    {
                        reader.error(&crate::language::tr(error));
                    }
                })
            }),
            diagrams: Some({
                let renderer = self.flow_diagrams.clone();
                let styles = self.content_styles.clone();
                let owner = Rc::downgrade(self);
                let snapshot = self.snapshot();
                Rc::new(move |kind, source| {
                    // Saved Preview remains readable while an Edit page is
                    // dirty. Diagram mounting needs snapshot identity, not the
                    // stricter comment-submission eligibility above.
                    if owner.upgrade().is_some_and(|r| {
                        r.state.borrow().generation == generation && r.snapshot() == snapshot
                    }) {
                        Ok(renderer.widget(kind, source, styles.clone()).upcast())
                    } else {
                        Err("Document changed before diagram mounting".into())
                    }
                })
            }),
            comments: Some(crate::text_flow::Comments {
                revision,
                current,
                begin,
                configure: Some(configure),
            }),
        })
    }
    /// Construct the native text adapter with the same toolbar/context targets.
    /// Preparation remains worker-owned; this only installs native widgets.
    pub(super) fn flow_view(
        self: &Rc<Self>,
        prepared: crate::text_flow::Prepared,
    ) -> Result<crate::text_flow::View, String> {
        let options = self.flow_options().ok_or("No flow document context")?;
        let view = match crate::text_flow::view_with_options(prepared, options) {
            Ok(view) => view,
            Err(error) => {
                // A later factory can fail after earlier anchors queued work.
                // Do not retain those partial rows/diagrams without a view.
                self.clear_flow_preview();
                return Err(error);
            }
        };
        self.remember_selection(view.widget.upcast_ref(), None);
        Ok(view)
    }
    fn remember_selection(
        &self,
        widget: &gtk::Widget,
        source: Option<studio_markdown_native::SourceRange>,
    ) {
        let label = widget
            .downcast_ref::<gtk::Label>()
            .filter(|l| l.is_selectable());
        let view = widget
            .downcast_ref::<gtk::TextView>()
            .filter(|v| !v.is_editable());
        if label.is_none() && view.is_none() {
            let mut child = widget.first_child();
            while let Some(widget) = child {
                child = widget.next_sibling();
                self.remember_selection(&widget, source.clone());
            }
            return;
        }
        let state = self.state.borrow();
        let Some(index) = &state.index else { return };
        let (generation, revision) = (state.generation, index.revision.clone());
        drop(state);
        let mut selections = self.selection.borrow_mut();
        selections.retain(|entry| {
            entry.generation == generation
                && entry.revision == revision
                && entry.widget.upgrade().is_some()
        });
        if let Some(entry) = selections
            .iter_mut()
            .find(|entry| entry.widget.upgrade().as_ref() == Some(widget))
        {
            entry.source = source;
            return;
        }
        if selections.len() >= 512 {
            selections.remove(0);
        }
        let changed = Rc::new(Cell::new(0));
        let update: Rc<dyn Fn()> = Rc::new({
            let (clock, changed) = (self.selection_clock.clone(), changed.clone());
            move || {
                let next = clock.get().saturating_add(1);
                clock.set(next);
                changed.set(next);
            }
        });
        if let Some(label) = label {
            // GTK4 Label has no selection-bound/cursor-position properties.
            // Observe native focus and input without claiming its gestures or
            // polling the label, while TextView exposes actual buffer marks.
            let focus = gtk::EventControllerFocus::new();
            let change = update.clone();
            focus.connect_enter(move |_| change());
            label.add_controller(focus);
            let input = gtk::EventControllerLegacy::new();
            input.set_propagation_phase(gtk::PropagationPhase::Capture);
            input.connect_event(move |_, event| {
                if matches!(
                    event.event_type(),
                    gtk::gdk::EventType::ButtonPress
                        | gtk::gdk::EventType::ButtonRelease
                        | gtk::gdk::EventType::KeyPress
                        | gtk::gdk::EventType::KeyRelease
                        | gtk::gdk::EventType::TouchBegin
                        | gtk::gdk::EventType::TouchEnd
                ) {
                    update();
                }
                glib::Propagation::Proceed
            });
            label.add_controller(input);
        } else if let Some(view) = view {
            view.buffer().connect_mark_set(move |buffer, _, mark| {
                if *mark == buffer.get_insert() || *mark == buffer.selection_bound() {
                    update();
                }
            });
        }
        selections.push(TrackedSelection {
            generation,
            revision,
            widget: widget.downgrade(),
            source,
            changed,
        });
    }
    fn track_selection(
        self: &Rc<Self>,
        label: &impl IsA<gtk::Widget>,
        source: &studio_markdown_native::SourceRange,
    ) {
        let (generation, revision) = {
            let state = self.state.borrow();
            let Some(index) = &state.index else { return };
            (state.generation, index.revision.clone())
        };
        let owner = Rc::downgrade(self);
        let range = source.clone();
        let expected = revision.clone();
        if crate::rich_paragraph::when_ready(
            label.as_ref(),
            Rc::new(move |child| {
                if let Some(reader) = owner.upgrade().filter(|r| {
                    r.state.borrow().generation == generation
                        && r.state
                            .borrow()
                            .index
                            .as_ref()
                            .is_some_and(|i| i.revision == expected)
                }) {
                    reader.track_selection(child, &range);
                }
            }),
        ) {
            return;
        }
        self.remember_selection(label.as_ref(), Some(source.clone()));
        let weak = Rc::downgrade(self);
        let source = source.clone();
        crate::selection_menu::bind(
            label.as_ref(),
            "reader-comment",
            Rc::new(move |text| {
                let Some(reader) = weak.upgrade() else { return };
                let valid = {
                    let state = reader.state.borrow();
                    state.generation == generation
                        && state.mode == Mode::Preview
                        && !state.dirty
                        && !state.busy
                        && !state.saving
                        && state
                            .index
                            .as_ref()
                            .is_some_and(|index| index.revision == revision)
                };
                if !valid {
                    return;
                }
                if let Some(capture) =
                    text.and_then(|text| reader.capture(text, source.line, source.end_line))
                {
                    if let Some(callback) = reader.on_comment.borrow().as_ref() {
                        callback(capture);
                    }
                } else {
                    reader.error("Select a passage of at most 16 KiB in Preview, Source or saved Edit before commenting");
                }
            }),
        );
    }
    fn comment_selection(&self) {
        let capture = {
            let state = self.state.borrow();
            if state.dirty {
                drop(state);
                self.error("Save document edits before adding a source-linked comment");
                return;
            }
            if state.busy || state.saving {
                return;
            }
            let mode = state.mode;
            let generation = state.generation;
            let revision = state.index.as_ref().map(|index| index.revision.clone());
            drop(state);
            match mode {
                Mode::Preview => {
                    let selected = self
                        .selection
                        .borrow()
                        .iter()
                        .filter(|entry| {
                            entry.generation == generation
                                && revision.as_ref() == Some(&entry.revision)
                        })
                        .filter_map(|entry| {
                            let widget = entry.widget.upgrade().filter(|w| w.is_mapped())?;
                            let selected = if let Some(label) = widget.downcast_ref::<gtk::Label>()
                            {
                                label.selection_bounds().is_some()
                            } else if let Some(view) = widget.downcast_ref::<gtk::TextView>() {
                                view.buffer().has_selection()
                            } else {
                                false
                            };
                            selected.then(|| (entry.clone(), widget))
                        })
                        .max_by_key(|(entry, widget)| (widget.has_focus(), entry.changed.get()));
                    if let Some((entry, widget)) = selected {
                        if let Some(source) = entry.source {
                            let text = crate::content_widgets::selection(&widget, 16384);
                            text.and_then(|text| self.capture(text, source.line, source.end_line))
                        } else {
                            // Dispatch outside the registry borrow: the action may
                            // synchronously open a dialog or update the document.
                            let _ = widget.activate_action("reader-comment.capture", None);
                            return;
                        }
                    } else {
                        None
                    }
                }
                Mode::Source | Mode::Edit => {
                    let (view, base) = if mode == Mode::Source {
                        (&self.source_view, self.source_line.get())
                    } else {
                        (&self.edit_view, self.edit_line.get())
                    };
                    let buffer = view.buffer();
                    buffer.selection_bounds().and_then(|(start, end)| {
                        self.capture(
                            buffer.text(&start, &end, false).to_string(),
                            base + start.line() as usize,
                            base + end.line() as usize
                                - usize::from(end.starts_line() && end.line() > start.line()),
                        )
                    })
                }
            }
        };
        if let Some(capture) = capture {
            if let Some(callback) = self.on_comment.borrow().as_ref() {
                callback(capture);
            }
        } else {
            self.error("Select a passage of at most 16 KiB in Preview, Source or saved Edit before commenting");
        }
    }
    #[cfg(debug_assertions)]
    pub fn smoke_inline_comment(
        &self,
        expected: &str,
        line: usize,
    ) -> Result<serde_json::Value, String> {
        self.smoke_inline_comment_range(expected, line, line)
    }
    #[cfg(debug_assertions)]
    pub fn smoke_inline_comment_range(
        &self,
        expected: &str,
        first_line: usize,
        last_line: usize,
    ) -> Result<serde_json::Value, String> {
        let captured = Rc::new(RefCell::new(None));
        let result = captured.clone();
        let previous = self.on_comment.replace(Some(Box::new(move |capture| {
            result.replace(Some(capture));
        })));
        self.comment_selection();
        self.on_comment.replace(previous);
        let capture = captured
            .borrow_mut()
            .take()
            .ok_or("Inline formula was not captured by the comment action")?;
        if capture.excerpt != expected
            || capture
                .source
                .pointer("/anchor/startLine")
                .and_then(serde_json::Value::as_u64)
                != Some(first_line as u64)
            || capture
                .source
                .pointer("/anchor/endLine")
                .and_then(serde_json::Value::as_u64)
                != Some(last_line as u64)
        {
            return Err(format!(
                "Inline formula comment lost source anchors: {capture:?}"
            ));
        }
        Ok(serde_json::json!({"quote":capture.excerpt,"sourceLines":true}))
    }
    #[cfg(debug_assertions)]
    pub fn smoke_rich_comment(&self) -> Result<serde_json::Value, String> {
        let widget = self
            .selection
            .borrow()
            .iter()
            .filter_map(|entry| entry.widget.upgrade())
            .find_map(|widget| {
                fn find(widget: &gtk::Widget) -> Option<gtk::TextView> {
                    if let Some(view) = widget
                        .downcast_ref::<gtk::TextView>()
                        .filter(|v| v.is_mapped())
                    {
                        return Some(view.clone());
                    }
                    let mut child = widget.first_child();
                    while let Some(widget) = child {
                        child = widget.next_sibling();
                        if let Some(view) = find(&widget) {
                            return Some(view);
                        }
                    }
                    None
                }
                find(&widget)
            });
        let Some(view) = widget else {
            return Ok(serde_json::json!({"legacyLabel":true}));
        };
        let buffer = view.buffer();
        let (start, end) = buffer
            .start_iter()
            .forward_search("bold 中文😀", gtk::TextSearchFlags::TEXT_ONLY, None)
            .ok_or("Rich comment quote missing")?;
        buffer.select_range(&start, &end);
        let captured = Rc::new(RefCell::new(None));
        let result = captured.clone();
        let previous = self.on_comment.replace(Some(Box::new(move |capture| {
            result.replace(Some(capture));
        })));
        self.comment_selection();
        self.on_comment.replace(previous);
        buffer.place_cursor(&buffer.start_iter());
        let capture = captured
            .borrow_mut()
            .take()
            .ok_or("Rich comment action did not capture selection")?;
        if capture.excerpt != "bold 中文😀"
            || capture
                .source
                .pointer("/anchor/startLine")
                .and_then(serde_json::Value::as_u64)
                != Some(1)
            || capture
                .source
                .pointer("/anchor/endLine")
                .and_then(serde_json::Value::as_u64)
                != Some(1)
        {
            return Err("Rich comment lost exact quote or original block lines".into());
        }
        Ok(serde_json::json!({"unicodeQuote":true,"sourceLines":true}))
    }
    fn paragraph_reading(
        &self,
        offset: usize,
        large: bool,
    ) -> Rc<crate::content_widgets::ParagraphState> {
        if !large {
            return Rc::new(crate::content_widgets::ParagraphState::default());
        }
        self.state
            .borrow_mut()
            .paragraphs
            .get_or_insert(offset, || {
                Rc::new(crate::content_widgets::ParagraphState::default())
            })
            .clone()
    }
    fn cell_link(self: &Rc<Self>) -> crate::content_widgets::OpenLink {
        let snapshot = self.snapshot();
        let generation = self.state.borrow().generation;
        let owner = Rc::downgrade(self);
        Rc::new(move |uri| {
            if let Some(reader) = owner
                .upgrade()
                .filter(|r| r.state.borrow().generation == generation && r.snapshot() == snapshot)
            {
                reader.activate_link(uri);
            }
        })
    }
    fn cell_selection(
        self: &Rc<Self>,
        cell: &studio_markdown_native::Cell,
    ) -> crate::table_cell::ConfigureText {
        let snapshot = self.snapshot();
        let generation = self.state.borrow().generation;
        let owner = Rc::downgrade(self);
        let source = cell.source.clone();
        Rc::new(move |widget| {
            if let Some(reader) = owner
                .upgrade()
                .filter(|r| r.state.borrow().generation == generation && r.snapshot() == snapshot)
            {
                reader.track_selection(widget, &source);
            }
        })
    }
    fn table_images(self: &Rc<Self>) -> Option<crate::table_cell::RenderImages> {
        let renderer = self.image_renderer.borrow().clone()?;
        let (root, path) = self.location()?;
        let (reader_id, revision) = self.snapshot()?;
        let generation = self.state.borrow().generation;
        let owner = Rc::downgrade(self);
        Some(Rc::new(move |container, content, compact| {
            let Some(reader) = owner.upgrade().filter(|r| {
                r.state.borrow().generation == generation
                    && r.snapshot()
                        .as_ref()
                        .is_some_and(|(id, rev)| id == &reader_id && rev == &revision)
            }) else {
                return;
            };
            let open = reader.cell_link();
            renderer.append_table_images(
                container,
                content,
                &root,
                Some(&path),
                open,
                Some((reader_id.clone(), revision.clone())),
                compact,
            );
        }))
    }
    fn block(self: &Rc<Self>, block: &Block) -> gtk::Widget {
        let container = gtk::Box::new(gtk::Orientation::Vertical, 8);
        container.set_margin_start(20);
        container.set_margin_end(20);
        container.set_margin_top(8);
        container.set_margin_bottom(8);
        crate::appearance_ui::watch_markdown_block(&container);
        container.set_tooltip_text(Some(&format!(
            "Source lines {}–{}",
            block.source.line, block.source.end_line
        )));
        match &block.kind {
            BlockKind::Paragraph(content) => {
                let weak = Rc::downgrade(self);
                let reading =
                    self.paragraph_reading(block.source.start, content.attributes.is_some());
                let label = crate::content_widgets::formatted_at(
                    content,
                    Rc::new(move |uri| {
                        if let Some(reader) = weak.upgrade() {
                            reader.activate_link(uri);
                        }
                    }),
                    reading,
                );
                self.track_selection(&label, &block.source);
                container.append(&label);
                if let (Some(renderer), Some((root, path))) =
                    (self.image_renderer.borrow().clone(), self.location())
                {
                    let weak = Rc::downgrade(self);
                    let context = {
                        let state = self.state.borrow();
                        state
                            .reader_id
                            .clone()
                            .zip(state.index.as_ref().map(|i| i.revision.clone()))
                    };
                    renderer.append_document_images(
                        &container,
                        content,
                        &root,
                        Some(&path),
                        Rc::new(move |reference| {
                            if let Some(reader) = weak.upgrade() {
                                reader.activate_link(reference);
                            }
                        }),
                        context,
                    );
                }
            }
            BlockKind::Heading { level, content } => {
                let weak = Rc::downgrade(self);
                let label = crate::content_widgets::heading_at(
                    content,
                    *level,
                    Rc::new(move |uri| {
                        if let Some(reader) = weak.upgrade() {
                            reader.activate_link(uri);
                        }
                    }),
                    self.paragraph_reading(block.source.start, content.attributes.is_some()),
                );
                self.track_selection(&label, &block.source);
                container.append(&label);
            }
            BlockKind::Code { language, text } => {
                let widget = self.content_styles.code(language, text);
                self.track_selection(&widget, &block.source);
                container.append(&widget);
            }
            BlockKind::Diagram { kind, source } => {
                let id = block.source.start;
                let existing = self.state.borrow().diagrams.get(&id).cloned();
                let diagram = existing.unwrap_or_else(|| {
                    let widget = gtk::Box::new(gtk::Orientation::Vertical, 8);
                    let alternative = if kind == "math" {
                        format!("Math formula: {source}")
                    } else {
                        format!("{kind} diagram")
                    };
                    let picture = crate::scaled_picture::ScaledPicture::new(&alternative);
                    picture.set_hexpand(true);
                    picture.set_visible(false);
                    let label = gtk::Label::builder()
                        .label(format!("Rendering {kind}…"))
                        .wrap(true)
                        .xalign(0.0)
                        .build();
                    let controls = crate::adaptive::toolbar(6);
                    label.set_hexpand(true);
                    controls.append(&label);
                    controls.append(&picture.inspect_button(kind));
                    widget.append(&controls);
                    widget.append(&picture);
                    let diagram = Rc::new(DiagramUi {
                        widget,
                        picture,
                        label,
                        serial: Cell::new(0),
                        id,
                        kind: kind.clone(),
                        source: source.clone(),
                    });
                    let owner = Rc::downgrade(self);
                    let pending = Rc::downgrade(&diagram);
                    diagram.widget.connect_map(move |_| {
                        if let (Some(reader), Some(diagram)) = (owner.upgrade(), pending.upgrade())
                        {
                            if diagram.serial.get() == 0 {
                                diagram.load(&reader);
                            }
                        }
                    });
                    self.state.borrow_mut().diagrams.insert(id, diagram.clone());
                    diagram.load(self);
                    diagram
                });
                if let Some(parent) = diagram.widget.parent().and_downcast::<gtk::Box>() {
                    parent.remove(&diagram.widget);
                }
                container.append(&diagram.widget);
                if diagram.serial.get() == 0 {
                    diagram.load(self);
                }
            }
            BlockKind::Table { id: Some(id), .. } => {
                let existing = self.state.borrow().tables.get(id).cloned();
                let table = existing.unwrap_or_else(|| {
                    let table = TableUi::new(self, *id);
                    self.state.borrow_mut().tables.insert(*id, table.clone());
                    table.load(0);
                    table
                });
                // Virtualized rows may be rebound; reparent the stable table to
                // retain the current page, selected columns and focus.
                if let Some(parent) = table.widget.parent().and_downcast::<gtk::Box>() {
                    parent.remove(&table.widget);
                }
                container.append(&table.widget);
            }
            BlockKind::Table { header, rows, .. } => {
                for row in std::iter::once(header).chain(rows.iter().map(|row| &row.cells)) {
                    let label = rich(
                        &row.iter()
                            .map(|cell| cell.content.markup.as_str())
                            .collect::<Vec<_>>()
                            .join("   │   "),
                    );
                    self.label_link(&label);
                    container.append(&label);
                }
            }
            BlockKind::List { children, .. }
            | BlockKind::Item(children)
            | BlockKind::Quote(children)
            | BlockKind::Group(children) => {
                for child in children {
                    container.append(&self.block(child));
                }
            }
            BlockKind::Rule => container.append(&gtk::Separator::new(gtk::Orientation::Horizontal)),
        }
        container.upcast()
    }
}

fn icon(name: &str, tooltip: &str) -> gtk::Button {
    let button = gtk::Button::builder()
        .icon_name(name)
        .tooltip_text(tooltip)
        .valign(gtk::Align::Center)
        .build();
    crate::i18n_ui::bind(&button, "tooltip-text", tooltip);
    button
}
fn rich(markup: &str) -> gtk::Label {
    let label = gtk::Label::builder()
        .xalign(0.0)
        .yalign(0.0)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .selectable(true)
        .width_chars(1)
        .hexpand(true)
        .build();
    label.add_css_class("studio-content");
    label.add_css_class("studio-markdown");
    label.set_markup(markup);
    crate::appearance_ui::watch_inline_label(&label);
    label
}

pub(crate) fn text_view(editable: bool) -> (sourceview::View, Rc<Cell<usize>>) {
    text_view_with_labels(editable, None)
}

pub(crate) fn text_view_with_labels(
    editable: bool,
    labels: Option<Rc<dyn Fn(usize) -> String>>,
) -> (sourceview::View, Rc<Cell<usize>>) {
    let buffer = sourceview::Buffer::new(None::<&gtk::TextTagTable>);
    buffer.set_language(
        sourceview::LanguageManager::default()
            .language("markdown")
            .as_ref(),
    );
    buffer.set_highlight_syntax(true);
    crate::source_surface::bind(buffer.upcast_ref());
    buffer.set_max_undo_levels(200);
    let view = sourceview::View::with_buffer(&buffer);
    view.add_css_class("studio-code");
    view.set_editable(editable);
    view.set_monospace(true);
    view.set_wrap_mode(gtk::WrapMode::WordChar);
    view.set_left_margin(8);
    view.set_right_margin(16);
    view.set_top_margin(12);
    view.set_bottom_margin(12);
    view.set_tab_width(4);
    view.set_auto_indent(editable);
    let base = Rc::new(Cell::new(1));
    let line_base = base.clone();
    let gutter = sourceview::GutterRendererText::new();
    gutter.set_width_request(60);
    gutter.set_xalign(1.0);
    gutter.connect_query_data(move |gutter, _, line| {
        let physical = line_base.get() + line as usize;
        gutter.set_text(
            &labels
                .as_ref()
                .map_or_else(|| physical.to_string(), |labels| labels(physical)),
        );
    });
    sourceview::prelude::ViewExt::gutter(&view, gtk::TextWindowType::Left).insert(&gutter, 0);
    crate::translation_ui::bind(&view);
    (view, base)
}

fn connect_edits(reader: &Rc<Reader>) {
    let weak = Rc::downgrade(reader);
    reader
        .edit_view
        .buffer()
        .connect_modified_changed(move |buffer| {
            if let Some(reader) = weak.upgrade() {
                if !reader.state.borrow().loading_editor {
                    reader.state.borrow_mut().dirty = buffer.is_modified();
                    reader.update();
                }
            }
        });
    let weak = Rc::downgrade(reader);
    reader
        .edit_view
        .buffer()
        .connect_insert_text(move |buffer, iter, text| {
            let Some(reader) = weak.upgrade() else { return };
            if reader.state.borrow().loading_editor {
                return;
            }
            let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
            if normalized != text {
                buffer.stop_signal_emission_by_name("insert-text");
                let mut iter = *iter;
                buffer.insert(&mut iter, &normalized);
                return;
            }
            if text.contains('\0')
                || reader.state.borrow().busy
                || reader.state.borrow().edit_bytes.saturating_add(text.len())
                    > studio_markdown_native::edit_buffer::MAX_PAGE_BYTES
            {
                buffer.stop_signal_emission_by_name("insert-text");
                reader.error("The edit page is busy or would exceed 512 KiB");
                return;
            }
            if reader
                .request(
                    "mutation",
                    Request::Insert {
                        offset: iter.offset() as usize,
                        text: text.into(),
                    },
                )
                .is_none()
            {
                buffer.stop_signal_emission_by_name("insert-text");
                return;
            }
            let mut state = reader.state.borrow_mut();
            state.edit_bytes += text.len();
            state.dirty = true;
        });
    let weak = Rc::downgrade(reader);
    reader
        .edit_view
        .buffer()
        .connect_delete_range(move |buffer, start, end| {
            let Some(reader) = weak.upgrade() else { return };
            if reader.state.borrow().loading_editor {
                return;
            }
            if reader.state.borrow().busy {
                buffer.stop_signal_emission_by_name("delete-range");
                return;
            }
            if reader
                .request(
                    "mutation",
                    Request::Delete {
                        from: start.offset() as usize,
                        to: end.offset() as usize,
                    },
                )
                .is_none()
            {
                buffer.stop_signal_emission_by_name("delete-range");
                return;
            }
            let bytes = buffer.text(start, end, true).len();
            let mut state = reader.state.borrow_mut();
            state.edit_bytes = state.edit_bytes.saturating_sub(bytes);
            state.dirty = true;
        });
    let weak = Rc::downgrade(reader);
    reader.edit_view.buffer().connect_changed(move |_| {
        if let Some(reader) = weak.upgrade() {
            if !reader.state.borrow().loading_editor {
                reader.update();
            }
        }
    });
}

struct DiagramUi {
    widget: gtk::Box,
    picture: crate::scaled_picture::ScaledPicture,
    label: gtk::Label,
    serial: Cell<u64>,
    id: usize,
    kind: String,
    source: String,
}

impl DiagramUi {
    fn load(&self, reader: &Reader) {
        self.serial.set(0);
        self.picture.clear_texture();
        self.label.set_text(&format!("Rendering {}…", self.kind));
        self.label.set_visible(true);
        if let Some(serial) = reader.request(
            "diagram",
            Request::Diagram {
                id: self.id,
                kind: self.kind.clone(),
                source: self.source.clone(),
                dark: adw::StyleManager::default().is_dark(),
                style: crate::appearance_ui::diagram_style(),
            },
        ) {
            self.serial.set(serial);
        }
    }
}

struct TableUi {
    widget: gtk::Box,
    view: gtk::ColumnView,
    rows: gio::ListStore,
    model: gtk::NoSelection,
    #[cfg(debug_assertions)]
    bindings: Rc<Cell<usize>>,
    scroll: gtk::ScrolledWindow,
    visibility: RefCell<Option<Rc<crate::viewport::Visibility>>>,
    restore: RefCell<Option<Rc<crate::viewport::FrameUpdate>>>,
    scroll_memory: Cell<Option<(f64, f64)>>,
    label: gtk::Label,
    previous: gtk::Button,
    next: gtk::Button,
    #[cfg(debug_assertions)]
    menu: gtk::MenuButton,
    active: Cell<bool>,
    serial: Cell<u64>,
    start: Cell<usize>,
    size: Cell<usize>,
    total: Cell<usize>,
    columns: RefCell<Vec<usize>>,
    widths: Rc<RefCell<HashMap<usize, i32>>>,
    reader: std::rc::Weak<Reader>,
    generation: u64,
    revision: String,
    id: usize,
}

impl TableUi {
    fn defer_rows(self: &Rc<Self>) {
        // A stable bounded viewport preserves outer text/anchor geometry while
        // GTK row factories are detached. The immutable row page stays in Rust.
        self.scroll.set_propagate_natural_height(false);
        self.view.set_model(None::<&gtk::SelectionModel>);
        let weak = Rc::downgrade(self);
        let visibility = crate::viewport::Visibility::new(
            &self.scroll,
            Rc::new(move |visible| {
                if let Some(table) = weak.upgrade().filter(|t| t.current_reader().is_some()) {
                    table.admit_rows(visible);
                }
            }),
        );
        self.visibility.replace(Some(visibility));
    }
    fn admit_rows(self: &Rc<Self>, visible: bool) {
        if visible == self.view.model().is_some() {
            return;
        }
        if !visible {
            let restoring = self.restore.borrow_mut().take();
            if restoring.is_none() {
                self.scroll_memory.set(Some((
                    self.scroll.hadjustment().value(),
                    self.scroll.vadjustment().value(),
                )));
            }
            if let Some(update) = restoring {
                update.clear();
            }
            self.view.set_model(None::<&gtk::SelectionModel>);
            return;
        }
        self.view.set_model(Some(&self.model));
        let Some((horizontal, vertical)) = self.scroll_memory.get() else {
            return;
        };
        let weak = Rc::downgrade(self);
        let attempts = Cell::new(0);
        let update = crate::viewport::FrameUpdate::new(Rc::new(move |_| {
            let Some(table) = weak.upgrade() else { return };
            if table.current_reader().is_none() || table.view.model().is_none() {
                table.restore.borrow_mut().take();
                return;
            }
            let h = table.scroll.hadjustment();
            let v = table.scroll.vadjustment();
            if (h.upper() - h.page_size() + 1. >= horizontal
                && v.upper() - v.page_size() + 1. >= vertical)
                || attempts.get() >= 3
            {
                h.set_value(
                    horizontal
                        .min((h.upper() - h.page_size()).max(h.lower()))
                        .max(h.lower()),
                );
                v.set_value(
                    vertical
                        .min((v.upper() - v.page_size()).max(v.lower()))
                        .max(v.lower()),
                );
                table.scroll_memory.set(None);
                table.restore.borrow_mut().take();
            } else {
                attempts.set(attempts.get() + 1);
                if let Some(update) = table.restore.borrow().as_ref() {
                    update.queue(&table.scroll);
                }
            }
        }));
        self.restore.replace(Some(update.clone()));
        update.queue(&self.scroll);
    }
    fn new(reader: &Rc<Reader>, id: usize) -> Rc<Self> {
        let (columns, generation, revision) = {
            let state = reader.state.borrow();
            let columns = state
                .index
                .as_ref()
                .and_then(|index| index.tables.get(id))
                .map(|table| (0..table.columns.len().min(512)).collect::<Vec<_>>())
                .unwrap_or_default();
            (
                columns,
                state.generation,
                state
                    .index
                    .as_ref()
                    .map(|index| index.revision.clone())
                    .unwrap_or_default(),
            )
        };
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
        widget.add_css_class("card");
        let toolbar = crate::adaptive::toolbar(6);
        toolbar.set_margin_start(10);
        toolbar.set_margin_end(10);
        toolbar.set_margin_top(6);
        toolbar.set_margin_bottom(6);
        let label = gtk::Label::builder()
            .label("Loading table…")
            .xalign(0.0)
            .hexpand(true)
            .build();
        label.add_css_class("caption");
        let previous = icon("go-previous-symbolic", "Previous table page");
        let next = icon("go-next-symbolic", "Next table page");
        let menu = gtk::MenuButton::builder().label("Columns").build();
        let sql = icon("system-search-symbolic", "Query table with SQL");
        toolbar.append(&label);
        toolbar.append(&previous);
        toolbar.append(&next);
        toolbar.append(&menu);
        toolbar.append(&sql);
        widget.append(&toolbar);
        let rows = gio::ListStore::new::<glib::BoxedAnyObject>();
        let model = gtk::NoSelection::new(Some(rows.clone()));
        let view = gtk::ColumnView::new(Some(model.clone()));
        view.add_css_class("studio-workspace");
        view.set_show_column_separators(true);
        view.set_show_row_separators(true);
        view.set_reorderable(false);
        let scroll = gtk::ScrolledWindow::builder()
            .child(&view)
            .min_content_height(320)
            .max_content_height(440)
            .propagate_natural_height(true)
            .build();
        widget.append(&scroll);
        let table = Rc::new(Self {
            widget,
            view,
            rows,
            model,
            #[cfg(debug_assertions)]
            bindings: Rc::new(Cell::new(0)),
            scroll,
            visibility: RefCell::new(None),
            restore: RefCell::new(None),
            scroll_memory: Cell::new(None),
            label,
            previous,
            next,
            #[cfg(debug_assertions)]
            menu: menu.clone(),
            active: Cell::new(true),
            serial: Cell::new(0),
            start: Cell::new(0),
            size: Cell::new(50),
            total: Cell::new(0),
            columns: RefCell::new(columns),
            widths: Rc::default(),
            reader: Rc::downgrade(reader),
            generation,
            revision,
            id,
        });
        let weak = Rc::downgrade(&table);
        table.previous.connect_clicked(move |_| {
            if let Some(table) = weak.upgrade() {
                table.load(table.start.get().saturating_sub(table.size.get()));
            }
        });
        let weak = Rc::downgrade(&table);
        table.next.connect_clicked(move |_| {
            if let Some(table) = weak.upgrade() {
                table.load(table.start.get() + table.size.get());
            }
        });
        let weak = Rc::downgrade(&table);
        sql.connect_clicked(move |_| {
            if let Some(table) = weak.upgrade() {
                if let Some(reader) = table.current_reader() {
                    reader.open_table_query(table.id);
                }
            }
        });
        let names = reader
            .state
            .borrow()
            .index
            .as_ref()
            .and_then(|index| index.tables.get(id))
            .map(|table| table.columns.clone())
            .unwrap_or_default();
        let weak = Rc::downgrade(&table);
        crate::table_columns_ui::install(
            &menu,
            names,
            table.columns.borrow().clone(),
            512,
            Rc::new(move |columns| {
                let Some(table) = weak.upgrade() else {
                    return false;
                };
                if table.current_reader().is_none() {
                    return false;
                }
                *table.columns.borrow_mut() = columns.to_vec();
                table.load(table.start.get());
                true
            }),
        );
        table
    }

    fn current_reader(&self) -> Option<Rc<Reader>> {
        self.reader.upgrade().filter(|reader| {
            let state = reader.state.borrow();
            self.active.get()
                && state.generation == self.generation
                && state
                    .index
                    .as_ref()
                    .is_some_and(|index| index.revision == self.revision)
        })
    }
    fn load(&self, row: usize) {
        if let Some(reader) = self.current_reader() {
            let request = Request::Table {
                id: self.id,
                row,
                columns: self.columns.borrow().clone(),
            };
            if let Some(serial) = reader.request("table", request) {
                self.serial.set(serial);
                self.previous.set_sensitive(false);
                self.next.set_sensitive(false);
            }
        }
    }
    fn apply(&self, page: TablePage) {
        self.start.set(page.start);
        self.size.set(page.page_size);
        self.total.set(page.total);
        crate::table_columns_ui::row_status(
            &self.label,
            if page.total == 0 { 0 } else { page.start + 1 },
            page.end,
            page.total,
        );
        // Keep the table widget and scroller stable while replacing row data.
        // Rebuild columns only when selection changes, not on every page.
        let current = self.view.columns();
        let same = current.n_items() as usize == page.columns.len()
            && page.columns == *self.columns.borrow()
            && current.n_items() > 0
            && current
                .item(0)
                .and_downcast::<gtk::ColumnViewColumn>()
                .is_some_and(|column| {
                    column.title().as_deref() == page.header.first().map(|cell| cell.title())
                });
        let model = self.view.model();
        if !same {
            // Appending columns may synchronously bind to existing rows. Do
            // not expose the previous column schema to the new factories or
            // retain GTK's recycled row cells while changing that schema.
            self.view.set_model(None::<&gtk::SelectionModel>);
            self.rows.remove_all();
            while let Some(column) = self
                .view
                .columns()
                .item(0)
                .and_downcast::<gtk::ColumnViewColumn>()
            {
                self.view.remove_column(&column);
            }
            for (position, cell) in page.header.iter().enumerate() {
                let factory = gtk::SignalListItemFactory::new();
                factory.connect_setup(|_, item| {
                    let item = item.downcast_ref::<gtk::ListItem>().unwrap();
                    // GTK may measure a fixed-width cell between bindings.
                    // Keep its child alive even while its contents change.
                    item.set_child(Some(&gtk::Box::new(gtk::Orientation::Vertical, 0)));
                });
                factory.connect_unbind(|_, item| {
                    let item = item.downcast_ref::<gtk::ListItem>().unwrap();
                    let container = item.child().and_downcast::<gtk::Box>().unwrap();
                    while let Some(child) = container.first_child() {
                        container.remove(&child);
                    }
                });
                let reader = self.reader.clone();
                let snapshot = reader.upgrade().and_then(|reader| {
                    let state = reader.state.borrow();
                    Some((state.generation, state.index.as_ref()?.revision.clone()))
                });
                #[cfg(debug_assertions)]
                let bindings = self.bindings.clone();
                factory.connect_bind(move |_, item| {
                    #[cfg(debug_assertions)]
                    bindings.set(bindings.get() + 1);
                    let item = item.downcast_ref::<gtk::ListItem>().unwrap();
                    let object = item.item().and_downcast::<glib::BoxedAnyObject>().unwrap();
                    let row = object.borrow::<Vec<studio_markdown_native::Cell>>();
                    let container = item.child().and_downcast::<gtk::Box>().unwrap();
                    while let Some(child) = container.first_child() {
                        container.remove(&child);
                    }
                    if let Some(cell) = row.get(position) {
                        let images = if cell.content.images.is_empty() {
                            None
                        } else {
                            reader
                                .upgrade()
                                .filter(|r| {
                                    snapshot.as_ref().is_some_and(|(generation, revision)| {
                                        let state = r.state.borrow();
                                        state.generation == *generation
                                            && state
                                                .index
                                                .as_ref()
                                                .is_some_and(|i| i.revision == *revision)
                                    })
                                })
                                .and_then(|r| r.table_images())
                        };
                        let owner = reader.clone();
                        let source = cell.source.clone();
                        let link_snapshot = snapshot.clone();
                        let snapshot = snapshot.clone();
                        let configure = Rc::new(move |widget: &gtk::Widget| {
                            if let Some(reader) = owner.upgrade() {
                                let valid = {
                                    let state = reader.state.borrow();
                                    snapshot.as_ref().is_some_and(|(generation, revision)| {
                                        state.generation == *generation
                                            && state
                                                .index
                                                .as_ref()
                                                .is_some_and(|index| index.revision == *revision)
                                    })
                                };
                                if valid {
                                    reader.track_selection(widget, &source);
                                }
                            }
                        });
                        let reader = reader.clone();
                        let open = Rc::new(move |uri: &str| {
                            if let Some(reader) = reader.upgrade().filter(|r| {
                                link_snapshot
                                    .as_ref()
                                    .is_some_and(|(generation, revision)| {
                                        let state = r.state.borrow();
                                        state.generation == *generation
                                            && state
                                                .index
                                                .as_ref()
                                                .is_some_and(|i| i.revision == *revision)
                                    })
                            }) {
                                reader.activate_link(uri);
                            }
                        });
                        let (widget, _) =
                            crate::table_cell::widget(cell, open, Some(configure), images);
                        container.append(&widget);
                    }
                });
                let column = gtk::ColumnViewColumn::new(Some(cell.title()), Some(factory));
                if let Some(reader) = self.reader.upgrade() {
                    let configure = reader.cell_selection(cell);
                    let open = reader.cell_link();
                    crate::table_cell::header(
                        &column,
                        &self.view,
                        cell,
                        &format!("cell-header-{position}"),
                        open,
                        Some(configure),
                        if cell.content.images.is_empty() {
                            None
                        } else {
                            reader.table_images()
                        },
                    );
                }
                column.set_resizable(true);
                let original = page.columns[position];
                column.set_fixed_width(self.widths.borrow().get(&original).copied().unwrap_or(210));
                let widths = self.widths.clone();
                column.connect_fixed_width_notify(move |column| {
                    widths.borrow_mut().insert(original, column.fixed_width());
                });
                self.view.append_column(&column);
            }
        }
        let rows = page
            .rows
            .into_iter()
            .map(glib::BoxedAnyObject::new)
            .collect::<Vec<_>>();
        self.rows.splice(0, self.rows.n_items(), &rows);
        if !same {
            self.view.set_model(model.as_ref());
        }
        self.previous.set_sensitive(page.start > 0);
        self.next.set_sensitive(page.end < page.total);
    }
}
