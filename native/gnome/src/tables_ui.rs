//! Native paged table view. Parsing, SQL, projection and chart selection stay in Rust workers.
#[cfg(debug_assertions)]
#[path = "tables_recovery_acceptance.rs"]
mod recovery_acceptance;
use crate::i18n_ui::{self, Part, UiText};
use crate::{
    protocol::Command,
    tables::{self, Opened, Page, Reply, Response, Sheet},
};
use adw::prelude::*;
use gtk::{gio, glib};
use serde_json::{json, Value};
use sourceview::prelude::*;
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::Arc,
};
use tokio::sync::mpsc;

#[derive(Clone)]
struct Selected {
    page: Arc<Page>,
    row: usize,
    column: usize,
}
struct PendingComment {
    target: Rc<crate::annotations_ui::CaptureTarget>,
    owner: String,
    root: String,
}
struct Restore {
    anchor: crate::annotations::TableAnchor,
    quote: String,
    ticket: u64,
    phase: u8,
    trusted: bool,
}
#[derive(Default)]
struct State {
    generation: u64,
    sequence: u64,
    latest: HashMap<&'static str, u64>,
    view_ticket: u64,
    id: Option<String>,
    hash: String,
    pending_comment: Option<PendingComment>,
    restore: Option<Restore>,
    restore_verified: Option<bool>,
    location: Option<(String, String)>,
    sheets: Vec<Sheet>,
    sheet: usize,
    sql_mode: bool,
    query_id: Option<String>,
    sql: HashMap<usize, String>,
    compact_sheets: HashMap<usize, bool>,
    page: Option<Arc<Page>>,
    selected: Option<Selected>,
    cell_context: HashMap<&'static str, (u64, Selected)>,
    chart_context: u64,
    format_original: Option<(String, usize)>,
    layout_columns: Vec<String>,
    layout_key: (usize, bool),
    layout_stamp: u64,
    widths: HashMap<(usize, bool), (Vec<String>, Vec<i32>)>,
    executed_sql: HashMap<usize, String>,
    pending_sql: Option<String>,
    recovering: bool,
    resume_pending: bool,
}
struct Row {
    page: Arc<Page>,
    index: usize,
}
pub struct Tables {
    pub widget: adw::ToolbarView,
    parent: glib::WeakRef<adw::ApplicationWindow>,
    toast: adw::ToastOverlay,
    sender: mpsc::Sender<Command>,
    title: adw::WindowTitle,
    sheet: gtk::DropDown,
    delimiter: gtk::DropDown,
    data: gtk::ToggleButton,
    sql_mode: gtk::ToggleButton,
    editor: sourceview::View,
    editor_box: gtk::Box,
    query: gtk::Button,
    compact: gtk::ToggleButton,
    chart: gtk::Button,
    copy: gtk::Button,
    comment: gtk::Button,
    wrap: gtk::ToggleButton,
    rows: gio::ListStore,
    grid: gtk::ColumnView,
    status: gtk::Label,
    previous: gtk::Button,
    next: gtk::Button,
    labels: RefCell<Vec<glib::WeakRef<gtk::Label>>>,
    selected_label: RefCell<Option<glib::WeakRef<gtk::Label>>>,
    bound_cells: RefCell<HashMap<(usize, usize), glib::WeakRef<gtk::Label>>>,
    error_toast: RefCell<Option<adw::Toast>>,
    updating: Cell<bool>,
    state: RefCell<State>,
    _styles: Rc<crate::content_widgets::Styles>,
    pub on_close: RefCell<Option<Box<dyn Fn()>>>,
    pub on_open: RefCell<Option<Box<dyn Fn()>>>,
    pub comments: RefCell<Option<Rc<crate::annotations_ui::CaptureTarget>>>,
}
impl Tables {
    pub fn new(
        parent: &adw::ApplicationWindow,
        sender: mpsc::Sender<Command>,
        toast: &adw::ToastOverlay,
    ) -> Rc<Self> {
        let widget = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        header.set_show_start_title_buttons(false);
        header.set_show_end_title_buttons(false);
        let title = adw::WindowTitle::new("Table", "").ui_text("title", "Table");
        header.set_title_widget(Some(&title));
        let open = gtk::Button::builder()
            .icon_name("document-open-symbolic")
            .tooltip_text("Open table")
            .build()
            .ui_text("tooltip-text", "Open table");
        header.pack_start(&open);
        let close = gtk::Button::builder()
            .icon_name("window-close-symbolic")
            .tooltip_text("Close table")
            .build()
            .ui_text("tooltip-text", "Close table");
        header.pack_end(&close);
        let reload = gtk::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text("Reload table from file")
            .build()
            .ui_text("tooltip-text", "Reload table from file");
        header.pack_end(&reload);
        widget.add_top_bar(&header);
        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        toolbar.set_margin_start(12);
        toolbar.set_margin_end(12);
        toolbar.set_margin_bottom(8);
        let modes = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        modes.add_css_class("linked");
        let data = gtk::ToggleButton::with_label("Data").ui_text("label", "Data");
        let sql_mode = gtk::ToggleButton::with_label("SQL").ui_text("label", "SQL");
        sql_mode.set_group(Some(&data));
        data.set_active(true);
        modes.append(&data);
        modes.append(&sql_mode);
        toolbar.append(&modes);
        let sheet = gtk::DropDown::from_strings(&[]);
        i18n_ui::bind(&sheet, "tooltip-text", "Worksheet");
        sheet.set_visible(false);
        toolbar.append(&sheet);
        let delimiter =
            gtk::DropDown::from_strings(&["Auto delimiter", "Comma", "Tab", "Semicolon"]);
        i18n_ui::bind(&delimiter, "tooltip-text", "Delimited file separator");
        crate::adaptive::ui_string_selector(&delimiter, 16, Some("Delimiter"));
        toolbar.append(&delimiter);
        toolbar.append(&gtk::Box::builder().hexpand(true).build());
        let chart = gtk::Button::with_label("Chart").ui_text("label", "Chart");
        let copy = gtk::Button::builder()
            .icon_name("edit-copy-symbolic")
            .tooltip_text("Copy full selected cell")
            .build()
            .ui_text("tooltip-text", "Copy full selected cell");
        copy.set_sensitive(false);
        let wrap = gtk::ToggleButton::with_label("Wrap").ui_text("label", "Wrap");
        wrap.set_active(true);
        toolbar.append(&chart);
        toolbar.append(&copy);
        let comment = gtk::Button::builder()
            .icon_name("document-edit-symbolic")
            .tooltip_text("Comment on selected cell")
            .focus_on_click(false)
            .build()
            .ui_text("tooltip-text", "Comment on selected cell");
        toolbar.append(&comment);
        toolbar.append(&wrap);
        widget.add_top_bar(&toolbar);
        let editor = sourceview::View::new();
        i18n_ui::accessible_label(&editor, "SQL query");
        editor.add_css_class("studio-code");
        editor.set_monospace(true);
        editor.set_wrap_mode(gtk::WrapMode::WordChar);
        editor.set_auto_indent(true);
        editor.set_left_margin(12);
        editor.set_right_margin(12);
        editor.set_top_margin(10);
        editor.set_bottom_margin(10);
        let buffer = editor.buffer().downcast::<sourceview::Buffer>().unwrap();
        buffer.set_language(
            sourceview::LanguageManager::default()
                .language("sql")
                .as_ref(),
        );
        let styles = crate::content_widgets::Styles::new();
        styles.watch_buffer(&buffer);
        let editor_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        editor_box.add_css_class("card");
        editor_box.set_margin_start(12);
        editor_box.set_margin_end(12);
        editor_box.set_visible(false);
        editor_box.append(
            &gtk::ScrolledWindow::builder()
                .child(&editor)
                .min_content_height(110)
                .max_content_height(420)
                .propagate_natural_height(true)
                .hscrollbar_policy(gtk::PolicyType::Never)
                .build(),
        );
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        actions.set_halign(gtk::Align::End);
        actions.set_margin_end(8);
        actions.set_margin_bottom(8);
        let compact = gtk::ToggleButton::with_label("Compact").ui_text("label", "Compact");
        let query = gtk::Button::with_label("Query").ui_text("label", "Query");
        query.add_css_class("suggested-action");
        actions.append(&compact);
        actions.append(&query);
        editor_box.append(&actions);
        let rows = gio::ListStore::new::<glib::BoxedAnyObject>();
        let grid = gtk::ColumnView::new(Some(gtk::NoSelection::new(Some(rows.clone()))));
        grid.add_css_class("studio-workspace");
        grid.set_show_row_separators(true);
        grid.set_show_column_separators(true);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        body.append(&editor_box);
        body.append(
            &gtk::ScrolledWindow::builder()
                .child(&grid)
                .vexpand(true)
                .hexpand(true)
                .build(),
        );
        widget.set_content(Some(&body));
        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        footer.set_margin_start(12);
        footer.set_margin_end(12);
        footer.set_margin_top(6);
        footer.set_margin_bottom(6);
        let status = gtk::Label::builder()
            .label("Open a CSV, TSV or XLSX file")
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build()
            .ui_text("label", "Open a CSV, TSV or XLSX file");
        status.add_css_class("caption");
        footer.append(&status);
        let previous = gtk::Button::builder()
            .icon_name("go-previous-symbolic")
            .tooltip_text("Previous table page")
            .build()
            .ui_text("tooltip-text", "Previous table page");
        let next = gtk::Button::builder()
            .icon_name("go-next-symbolic")
            .tooltip_text("Next table page")
            .build()
            .ui_text("tooltip-text", "Next table page");
        footer.append(&previous);
        footer.append(&next);
        widget.add_bottom_bar(&footer);
        let ui = Rc::new(Self {
            widget,
            parent: parent.downgrade(),
            toast: toast.clone(),
            sender,
            title,
            sheet,
            delimiter,
            data,
            sql_mode,
            editor,
            editor_box,
            query,
            compact,
            chart,
            copy,
            comment,
            wrap,
            rows,
            grid,
            status,
            previous,
            next,
            labels: RefCell::new(Vec::new()),
            selected_label: RefCell::new(None),
            bound_cells: RefCell::default(),
            error_toast: RefCell::new(None),
            updating: Cell::new(false),
            state: RefCell::new(State::default()),
            _styles: styles,
            on_close: RefCell::new(None),
            on_open: RefCell::new(None),
            comments: RefCell::default(),
        });
        let weak = Rc::downgrade(&ui);
        open.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.choose();
            }
        });
        let weak = Rc::downgrade(&ui);
        close.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.close();
                if let Some(callback) = ui.on_close.borrow().as_ref() {
                    callback();
                }
            }
        });
        let weak = Rc::downgrade(&ui);
        reload.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.reload();
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.data.connect_toggled(move |button| {
            if button.is_active() {
                if let Some(ui) = weak.upgrade() {
                    ui.mode(false);
                }
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.sql_mode.connect_toggled(move |button| {
            if button.is_active() {
                if let Some(ui) = weak.upgrade() {
                    ui.mode(true);
                }
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.sheet.connect_selected_notify(move |button| {
            if let Some(ui) = weak.upgrade() {
                if !ui.updating.get() {
                    ui.change_sheet(button.selected() as usize);
                }
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.delimiter.connect_selected_notify(move |_| {
            if let Some(ui) = weak.upgrade() {
                if !ui.updating.get() {
                    ui.reload();
                }
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.query.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.run_query();
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.compact.connect_toggled(move |button| {
            if let Some(ui) = weak.upgrade() {
                if !ui.updating.get() {
                    let text = ui.sql_text();
                    let sheet = ui.state.borrow().sheet;
                    ui.state
                        .borrow_mut()
                        .compact_sheets
                        .insert(sheet, button.is_active());
                    ui.state.borrow_mut().format_original = Some((text.clone(), sheet));
                    ui.request(
                        "format",
                        json!({"operation":"format","sql":text,"compact":button.is_active()}),
                    );
                }
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.wrap.connect_toggled(move |button| {
            if let Some(ui) = weak.upgrade() {
                ui.labels.borrow_mut().retain(|label| {
                    if let Some(label) = label.upgrade() {
                        style_cell(&label, button.is_active());
                        true
                    } else {
                        false
                    }
                });
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.previous.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                let offset = ui
                    .state
                    .borrow()
                    .page
                    .as_ref()
                    .map(|page| page.offset.saturating_sub(page.page_size));
                if let Some(offset) = offset {
                    ui.page(offset);
                }
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.next.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                let offset = ui
                    .state
                    .borrow()
                    .page
                    .as_ref()
                    .map(|page| page.offset + page.page_size);
                if let Some(offset) = offset {
                    ui.page(offset);
                }
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.copy.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.cell_action(false);
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.comment.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.comment_cell();
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.chart.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                let body = ui.scope_body("chart");
                if let Some(body) = body {
                    let view = ui.state.borrow().view_ticket;
                    ui.state.borrow_mut().chart_context = view;
                    ui.request("chart", body);
                }
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.editor.buffer().connect_changed(move |_| {
            if let Some(ui) = weak.upgrade() {
                if !ui.updating.get() {
                    let text = ui.sql_text();
                    let mut state = ui.state.borrow_mut();
                    let sheet = state.sheet;
                    state.sql.insert(sheet, text);
                }
            }
        });
        let keys = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(&ui);
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            if key == gtk::gdk::Key::Return
                && modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK)
            {
                if let Some(ui) = weak.upgrade() {
                    ui.run_query();
                }
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        ui.editor.add_controller(keys);
        ui.controls();
        ui
    }
    fn request(&self, kind: &'static str, body: Value) {
        let (generation, serial) = {
            let mut state = self.state.borrow_mut();
            if kind == "query" {
                state.pending_sql = body["sql"].as_str().map(str::to_owned);
            }
            state.sequence += 1;
            let serial = state.sequence;
            let group = if matches!(kind, "open" | "resume" | "page" | "query") {
                "view"
            } else {
                kind
            };
            state.latest.insert(group, serial);
            if group == "view" {
                state.view_ticket = serial;
            }
            (state.generation, serial)
        };
        if matches!(kind, "open" | "page" | "query") {
            self.previous.set_sensitive(false);
            self.next.set_sensitive(false);
            self.query.set_sensitive(false);
        }
        if self
            .sender
            .try_send(Command::Table {
                generation,
                serial,
                kind,
                body,
            })
            .is_err()
        {
            if kind == "resume" {
                self.state.borrow_mut().resume_pending = false;
            }
            if kind == "comment" {
                self.state.borrow_mut().pending_comment.take();
            }
            if matches!(kind, "open" | "page" | "query" | "source") {
                let had_restore = self.state.borrow_mut().restore.take().is_some();
                if had_restore {
                    self.state.borrow_mut().restore_verified = Some(false);
                }
            }
            self.ui_error("Table request queue is busy; try again");
            self.controls();
        }
    }
    fn close_id(&self, id: String) {
        self.request("close", json!({"operation":"close","id":id}));
    }
    pub fn close(&self) {
        let old_toast = self.error_toast.borrow_mut().take();
        if let Some(toast) = old_toast {
            toast.dismiss();
        }
        let old = {
            let mut state = self.state.borrow_mut();
            let generation = state.generation + 1;
            let old = state.id.take();
            *state = State {
                generation,
                ..Default::default()
            };
            old
        };
        if let Some(id) = old {
            self.close_id(id);
        }
        self.rows.remove_all();
        self.labels.borrow_mut().clear();
        self.bound_cells.borrow_mut().clear();
        self.controls();
    }
    pub fn open(&self, root: &str, path: &str) {
        self.updating.set(true);
        self.delimiter.set_selected(0);
        self.updating.set(false);
        self.begin_open(root, path);
        if let Some(open) = self.on_open.borrow().as_ref() {
            open();
        }
    }
    pub fn has_document(&self) -> bool {
        self.state.borrow().location.is_some()
    }
    pub fn gateway_failed(&self, error: &str) {
        let mut state = self.state.borrow_mut();
        state.recovering = state.location.is_some();
        state.resume_pending = false;
        state.pending_sql = None;
        state.generation += 1;
        state.latest.clear();
        state.id = None;
        state.query_id = None;
        state.cell_context.clear();
        state.selected = None;
        state.pending_comment = None;
        state.restore = None;
        state.restore_verified = Some(false);
        let open = state.location.is_some();
        drop(state);
        self.controls();
        if open {
            self.error(error);
        }
    }
    pub fn gateway_restored(&self) {
        let body = {
            let mut state = self.state.borrow_mut();
            if !state.recovering || state.resume_pending {
                return;
            }
            let Some((root, path)) = state.location.clone() else {
                return;
            };
            state.resume_pending = true;
            let query = state
                .page
                .as_ref()
                .filter(|p| state.sql_mode && p.query_id.is_some())
                .and_then(|_| state.executed_sql.get(&state.sheet))
                .cloned();
            json!({"root":root,"path":path,"hash":state.hash,"sheet":state.sheet,
                "sql":query,"offset":state.page.as_ref().map_or(0,|p|p.offset),
                "delimiter":match self.delimiter.selected(){1=>Some(","),2=>Some("\t"),3=>Some(";"),_=>None}})
        };
        self.message("Revalidating table…");
        self.request("resume", body);
    }
    #[cfg(debug_assertions)]
    pub fn smoke_ready(&self) -> bool {
        self.state.borrow().page.is_some()
    }
    fn begin_open(&self, root: &str, path: &str) {
        self.close();
        self.state.borrow_mut().location = Some((root.into(), path.into()));
        i18n_ui::literal(
            &self.title,
            "title",
            std::path::Path::new(path)
                .file_name()
                .unwrap_or_default()
                .to_str()
                .unwrap_or("Table"),
        );
        i18n_ui::literal(&self.title, "subtitle", path);
        self.message("Opening table…");
        let is_xlsx = std::path::Path::new(path)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("xlsx"));
        self.delimiter.set_visible(!is_xlsx);
        let delimiter = if is_xlsx {
            None
        } else {
            Some(match self.delimiter.selected() {
                1 => ",",
                2 => "\t",
                3 => ";",
                _ => "auto",
            })
        };
        self.request(
            "open",
            json!({"operation":"open","root":root,"path":path,"delimiter":delimiter}),
        );
    }
    fn reload(&self) {
        let location = self.state.borrow().location.clone();
        if let Some((root, path)) = location {
            self.begin_open(&root, &path);
        }
    }
    pub fn choose(self: &Rc<Self>) {
        let dialog = gtk::FileDialog::builder()
            .title("Open table")
            .accept_label("Open")
            .build()
            .ui_text("accept-label", "Open")
            .ui_text("title", "Open table");
        let filter = gtk::FileFilter::new();
        i18n_ui::bind(&filter, "name", "Tables");
        for suffix in ["csv", "tsv", "xlsx"] {
            filter.add_suffix(suffix);
        }
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        dialog.set_filters(Some(&filters));
        let weak = Rc::downgrade(self);
        let parent = self.parent.upgrade();
        dialog.open(parent.as_ref(), gio::Cancellable::NONE, move |result| {
            if let (Some(ui), Ok(file)) = (weak.upgrade(), result) {
                if let Some(path) = file.path() {
                    if let Some(root) = path.parent() {
                        ui.open(&root.to_string_lossy(), &path.to_string_lossy());
                    }
                }
            }
        });
    }
    fn sql_text(&self) -> String {
        let buffer = self.editor.buffer();
        buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .to_string()
    }
    fn set_sql(&self, text: &str) {
        self.updating.set(true);
        self.editor.buffer().set_text(text);
        self.updating.set(false);
    }
    fn mode(&self, sql: bool) {
        if self.updating.get() {
            return;
        }
        self.state.borrow_mut().sql_mode = sql;
        self.editor_box.set_visible(sql);
        self.page(0);
    }
    fn change_sheet(&self, sheet: usize) {
        let (current, available) = {
            let state = self.state.borrow();
            (
                state.sheet,
                state.id.is_some() && sheet < state.sheets.len(),
            )
        };
        if !available || current == sheet {
            // A chooser notification can outlive its document connection. Keep
            // the retained page/draft and the visible selector on one sheet.
            if self.sheet.selected() as usize != current {
                let updating = self.updating.replace(true);
                self.sheet.set_selected(current as u32);
                self.updating.set(updating);
            }
            return;
        }
        let text = {
            let mut state = self.state.borrow_mut();
            if sheet >= state.sheets.len() {
                return;
            }
            state.sheet = sheet;
            state.query_id = None;
            state
                .sql
                .get(&sheet)
                .cloned()
                .unwrap_or_else(|| state.sheets[sheet].sql.clone())
        };
        self.set_sql(&text);
        let compact = self
            .state
            .borrow()
            .compact_sheets
            .get(&sheet)
            .copied()
            .unwrap_or(false);
        self.updating.set(true);
        self.compact.set_active(compact);
        self.updating.set(false);
        self.page(0);
    }
    fn scope_body(&self, operation: &str) -> Option<Value> {
        let state = self.state.borrow();
        Some(
            json!({"operation":operation,"id":state.id.as_ref()?,"sheet":state.sheet,"query":if state.sql_mode{state.query_id.as_ref()}else{None}}),
        )
    }
    fn page(&self, offset: usize) {
        let blank = {
            let mut state = self.state.borrow_mut();
            state.selected = None;
            state.page = None;
            state.sql_mode && state.query_id.is_none()
        };
        self.copy.set_sensitive(false);
        self.comment.set_sensitive(false);
        if blank {
            self.rows.remove_all();
            self.message("Run a read-only SELECT or WITH query");
            let mut state = self.state.borrow_mut();
            state.sequence += 1;
            let ticket = state.sequence;
            state.latest.insert("view", ticket);
            state.view_ticket = ticket;
            drop(state);
            self.controls();
            return;
        }
        if let Some(mut body) = self.scope_body("page") {
            body["offset"] = offset.into();
            self.request("page", body);
        }
    }
    fn run_query(&self) {
        let state = self.state.borrow();
        let Some(id) = state.id.clone() else {
            return;
        };
        let sheet = state.sheet;
        drop(state);
        if self.editor.buffer().char_count() > 65536 {
            self.ui_error("SQL is limited to 64 KiB");
            return;
        }
        self.message("Querying…");
        self.request(
            "query",
            json!({"operation":"query","id":id,"sheet":sheet,"sql":self.sql_text()}),
        );
    }
    fn error(&self, error: &str) {
        for property in ["label", "tooltip-text"] {
            i18n_ui::literal(&self.status, property, error);
        }
        let toast = adw::Toast::new(error);
        toast.set_use_markup(false);
        let previous = self.error_toast.replace(Some(toast.clone()));
        if let Some(previous) = previous {
            previous.dismiss();
        }
        self.toast.add_toast(toast);
    }
    fn controls(&self) {
        let state = self.state.borrow();
        for button in [&self.data, &self.sql_mode] {
            button.set_sensitive(state.id.is_some());
        }
        self.sheet.set_sensitive(state.id.is_some());
        self.delimiter.set_sensitive(state.id.is_some());
        self.query.set_sensitive(state.id.is_some());
        self.chart
            .set_sensitive(state.id.is_some() && state.page.is_some());
        self.copy
            .set_sensitive(state.id.is_some() && state.selected.is_some());
        self.comment.set_sensitive(
            state.id.is_some() && state.selected.is_some() && state.pending_comment.is_none(),
        );
        self.previous.set_sensitive(
            state.id.is_some() && state.page.as_ref().is_some_and(|page| page.offset > 0),
        );
        self.next.set_sensitive(
            state.id.is_some()
                && state
                    .page
                    .as_ref()
                    .is_some_and(|page| page.offset + page.rows.len() < page.total),
        );
    }
    fn cell_action(&self, dialog: bool) {
        let kind = if dialog { "cell" } else { "copy" };
        let mut state = self.state.borrow_mut();
        let Some(selected) = state.selected.clone() else {
            return;
        };
        let view = state.view_ticket;
        state.cell_context.insert(kind, (view, selected.clone()));
        drop(state);
        if !dialog {
            self.copy.set_sensitive(false);
        }
        self.request(kind,json!({"operation":"cell","id":selected.page.document_id,"sheet":selected.page.sheet,"query":selected.page.query_id,"row":selected.row,"column":selected.column}));
    }
    fn comment_cell(&self) {
        let Some(target) = self.comments.borrow().clone() else {
            return;
        };
        let Some(owner) = (target.owner)() else {
            self.ui_error("Select a session before commenting");
            return;
        };
        let mut state = self.state.borrow_mut();
        if state.pending_comment.is_some() {
            return;
        }
        let Some(selected) = state.selected.clone() else {
            return;
        };
        let root = state
            .location
            .as_ref()
            .map(|l| l.0.clone())
            .unwrap_or_default();
        let view = state.view_ticket;
        state
            .cell_context
            .insert("comment", (view, selected.clone()));
        state.pending_comment = Some(PendingComment {
            target,
            owner,
            root,
        });
        drop(state);
        self.request("comment",json!({"operation":"comment","id":selected.page.document_id,"sheet":selected.page.sheet,"query":selected.page.query_id,"row":selected.row,"column":selected.column}));
        self.controls();
    }
    pub fn open_comment(&self, anchor: crate::annotations::TableAnchor, quote: &str) {
        self.updating.set(true);
        self.delimiter
            .set_selected(match anchor.delimiter.as_deref() {
                Some(",") => 1,
                Some("\t") => 2,
                Some(";") => 3,
                _ => 0,
            });
        self.updating.set(false);
        self.begin_open(&anchor.root, &anchor.file_path);
        let ticket = self.state.borrow().view_ticket;
        self.state.borrow_mut().restore = Some(Restore {
            anchor,
            quote: quote.into(),
            ticket,
            phase: 0,
            trusted: false,
        });
        if let Some(open) = self.on_open.borrow().as_ref() {
            open();
        }
    }
    fn restore_failed(&self, message: &str) {
        let mut state = self.state.borrow_mut();
        state.restore.take();
        state.restore_verified = Some(false);
        drop(state);
        self.message(message);
    }
    fn restore_source(&self) {
        let (anchor, phase, ticket) = match self.state.borrow().restore.as_ref() {
            Some(r) => (r.anchor.clone(), r.phase, r.ticket),
            None => return,
        };
        if ticket != self.state.borrow().view_ticket {
            self.restore_failed("Source navigation cancelled");
            return;
        }
        let Ok((row, column)) = crate::annotations::table_cell(&anchor.range) else {
            self.restore_failed("Invalid comment cell address");
            return;
        };
        if phase == 0 {
            if anchor.sheet == "Query"
                && anchor.sql.is_none()
                && anchor.header.is_empty()
                && anchor.delimiter.is_none()
            {
                self.restore_failed("This legacy query comment has no saved SQL; showing the workbook without a cell highlight");
                return;
            }
            let state = self.state.borrow();
            let sheet = if anchor.sheet.is_empty() {
                Some(0)
            } else {
                state.sheets.iter().position(|s| s.name == anchor.sheet)
            };
            let Some(sheet) = sheet else {
                drop(state);
                self.restore_failed("The comment worksheet is no longer available");
                return;
            };
            let trusted = !anchor.document_hash.is_empty() && anchor.document_hash == state.hash;
            drop(state);
            self.updating.set(true);
            self.sheet.set_selected(sheet as u32);
            let sql = anchor.sql.as_ref().filter(|_| trusted);
            self.sql_mode.set_active(sql.is_some());
            self.data.set_active(sql.is_none());
            self.editor_box.set_visible(sql.is_some());
            let editor = sql
                .cloned()
                .unwrap_or_else(|| self.state.borrow().sheets[sheet].sql.clone());
            self.editor.buffer().set_text(&editor);
            self.updating.set(false);
            {
                let mut state = self.state.borrow_mut();
                state.sheet = sheet;
                state.sql_mode = sql.is_some();
                state.query_id = None;
                state.sql.insert(sheet, editor);
                let restore = state.restore.as_mut().unwrap();
                restore.trusted = trusted;
                restore.phase = if sql.is_some() { 1 } else { 2 };
            }
            if sql.is_some() {
                self.run_query();
            } else {
                self.page(if anchor.sql.is_some() { 0 } else { row });
            }
            let ticket = self.state.borrow().view_ticket;
            if let Some(restore) = self.state.borrow_mut().restore.as_mut() {
                restore.ticket = ticket;
            }
            return;
        }
        if phase == 1 || phase == 2 {
            let state = self.state.borrow();
            let Some(page) = state.page.clone() else {
                return;
            };
            if !state.restore.as_ref().unwrap().trusted {
                drop(state);
                self.restore_failed("Table changed or its edition is unverified; showing the sheet without a cell highlight or rerunning its query");
                return;
            }
            if row >= page.total || column >= page.columns.len() {
                drop(state);
                self.restore_failed("The comment cell is outside the current preview");
                return;
            }
            drop(state);
            if phase == 1 && !(page.offset..page.offset + page.rows.len()).contains(&row) {
                self.state.borrow_mut().restore.as_mut().unwrap().phase = 2;
                self.page(row);
                let ticket = self.state.borrow().view_ticket;
                if let Some(restore) = self.state.borrow_mut().restore.as_mut() {
                    restore.ticket = ticket;
                }
                return;
            }
            if !(page.offset..page.offset + page.rows.len()).contains(&row) {
                self.restore_failed("Source navigation cancelled");
                return;
            }
            self.state.borrow_mut().restore.as_mut().unwrap().phase = 3;
            self.request("source",json!({"operation":"comment","id":page.document_id,"sheet":page.sheet,"query":page.query_id,"row":row,"column":column}));
        }
    }
    fn mark_source(&self, row: usize, column: usize) {
        let Some(page) = self.state.borrow().page.clone() else {
            return;
        };
        self.state.borrow_mut().selected = Some(Selected {
            page: page.clone(),
            row,
            column,
        });
        let label = self
            .bound_cells
            .borrow()
            .get(&(row, column))
            .and_then(|w| w.upgrade());
        if let Some(label) = label {
            self.select(page.clone(), row, column, &label, false);
        }
        let col = self
            .grid
            .columns()
            .item(column as u32)
            .and_downcast::<gtk::ColumnViewColumn>();
        self.grid.scroll_to(
            (row - page.offset) as u32,
            col.as_ref(),
            gtk::ListScrollFlags::FOCUS,
            None,
        );
        self.controls();
    }
    fn select(&self, page: Arc<Page>, row: usize, column: usize, label: &gtk::Label, dialog: bool) {
        let mut state = self.state.borrow_mut();
        if !state
            .page
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, &page))
        {
            return;
        }
        state.selected = Some(Selected { page, row, column });
        drop(state);
        if let Some(old) = self
            .selected_label
            .borrow_mut()
            .take()
            .and_then(|old| old.upgrade())
        {
            old.remove_css_class("accent");
        }
        label.add_css_class("accent");
        self.selected_label.replace(Some(label.downgrade()));
        self.copy.set_sensitive(true);
        self.controls();
        if dialog {
            self.cell_action(true);
        }
    }
    pub fn event(self: &Rc<Self>, response: Response) {
        if response.kind == "close" {
            return;
        }
        let group = if matches!(response.kind, "open" | "resume" | "page" | "query") {
            "view"
        } else {
            response.kind
        };
        let editor_text = if response.kind == "format" {
            Some(self.sql_text())
        } else {
            None
        };
        let current = {
            let state = self.state.borrow();
            state.generation == response.generation
                && state.latest.get(group) == Some(&response.serial)
                && match response.kind {
                    "cell" | "copy" | "comment" => state
                        .cell_context
                        .get(response.kind)
                        .is_some_and(|(view, selected)| {
                            *view == state.view_ticket
                                && state.selected.as_ref().is_some_and(|current| {
                                    Arc::ptr_eq(&current.page, &selected.page)
                                        && current.row == selected.row
                                        && current.column == selected.column
                                })
                        }),
                    "chart" => state.chart_context == state.view_ticket,
                    "source" => state
                        .restore
                        .as_ref()
                        .is_some_and(|r| r.ticket == state.view_ticket),
                    "format" => state.format_original.as_ref().is_some_and(|(text, sheet)| {
                        Some(text) == editor_text.as_ref() && *sheet == state.sheet
                    }),
                    _ => true,
                }
        };
        if !current {
            if response.kind == "comment"
                && self.state.borrow().latest.get("comment") == Some(&response.serial)
            {
                self.state.borrow_mut().pending_comment.take();
                self.controls();
            }
            if let Ok(Reply::Opened(opened)) = response.result {
                self.close_id(opened.id);
            }
            return;
        }
        match response.result {
            Err(error) => {
                if response.kind == "resume" {
                    self.state.borrow_mut().resume_pending = false;
                }
                if response.kind == "query" {
                    self.state.borrow_mut().pending_sql = None;
                }
                if response.kind == "comment" {
                    self.state.borrow_mut().pending_comment.take();
                }
                let had_restore = self.state.borrow_mut().restore.take().is_some();
                if had_restore {
                    self.state.borrow_mut().restore_verified = Some(false);
                }
                self.error(&error);
            }
            Ok(Reply::Opened(opened)) if response.kind == "resume" => {
                let blank = {
                    let mut state = self.state.borrow_mut();
                    state.id = Some(opened.id);
                    state.query_id = opened.page.query_id.clone();
                    state.sheets = opened.sheets;
                    state.resume_pending = false;
                    state.recovering = false;
                    state.sql_mode && state.query_id.is_none()
                };
                if blank {
                    self.page(0);
                } else {
                    self.render(Arc::new(opened.page));
                }
            }
            Ok(Reply::Opened(opened)) => self.opened(opened),
            Ok(Reply::Page(page)) => {
                let mut state = self.state.borrow_mut();
                if state.id.as_deref() != Some(page.document_id.as_str())
                    || state.sheet != page.sheet
                {
                    return;
                }
                if response.kind == "query" {
                    state.query_id = page.query_id.clone();
                    if let Some(sql) = state.pending_sql.take() {
                        state.executed_sql.insert(page.sheet, sql);
                    }
                }
                drop(state);
                self.render(page);
                self.restore_source();
            }
            Ok(Reply::Comment(comment)) => {
                if response.kind == "comment" {
                    let pending = self.state.borrow_mut().pending_comment.take();
                    if let Some(pending) = pending {
                        let mut anchor = comment.anchor;
                        anchor.root = pending.root;
                        let excerpt = if comment.text.trim().is_empty() {
                            format!("[{}]", anchor.range)
                        } else {
                            comment.text
                        };
                        let result = crate::annotations::Capture {
                            excerpt,
                            source: json!({"provider":"table","version":1,"anchor":anchor}),
                        }
                        .validate()
                        .and_then(|capture| (pending.target.capture)(pending.owner, capture));
                        match result {
                            Ok(()) => self.message("Comment added"),
                            Err(e) => self.error(&e),
                        }
                    }
                } else {
                    let restore = self.state.borrow_mut().restore.take();
                    if let Some(restore) = restore {
                        let expected = if comment.text.trim().is_empty() {
                            format!("[{}]", comment.anchor.range)
                        } else {
                            comment.text
                        };
                        let matched = expected == restore.quote
                            && comment.anchor.document_hash == restore.anchor.document_hash
                            && comment.anchor.sql == restore.anchor.sql
                            && (restore.anchor.sheet.is_empty()
                                || comment.anchor.sheet == restore.anchor.sheet)
                            && crate::annotations::table_cell(&comment.anchor.range).ok()
                                == crate::annotations::table_cell(&restore.anchor.range).ok()
                            && self
                                .state
                                .borrow()
                                .location
                                .as_ref()
                                .is_some_and(|l| l.1 == comment.anchor.file_path);
                        self.state.borrow_mut().restore_verified = Some(matched);
                        if matched {
                            if let Ok((row, column)) =
                                crate::annotations::table_cell(&restore.anchor.range)
                            {
                                self.mark_source(row, column);
                            }
                            self.message("Comment cell verified");
                        } else {
                            self.message("Comment value or query result did not match; showing its page without a cell highlight");
                        }
                    }
                }
            }
            Ok(Reply::Cell(text)) => {
                let valid = {
                    let state = self.state.borrow();
                    state
                        .cell_context
                        .get(response.kind)
                        .is_some_and(|(view, selected)| {
                            *view == state.view_ticket
                                && state.selected.as_ref().is_some_and(|current| {
                                    Arc::ptr_eq(&current.page, &selected.page)
                                        && current.row == selected.row
                                        && current.column == selected.column
                                })
                        })
                };
                if valid {
                    if response.kind == "copy" {
                        self.widget.clipboard().set_text(&text);
                    } else {
                        self.show_cell(&text);
                    }
                }
            }
            Ok(Reply::Chart(chart)) => {
                if self.state.borrow().chart_context == self.state.borrow().view_ticket {
                    self.show_chart(chart);
                }
            }
            Ok(Reply::Format(text)) => {
                self.set_sql(&text);
                let mut state = self.state.borrow_mut();
                let sheet = state.sheet;
                state.sql.insert(sheet, text);
            }
            Ok(Reply::Closed) => {}
        }
        self.controls();
    }
    fn opened(self: &Rc<Self>, opened: Opened) {
        let Opened {
            id,
            name,
            path,
            hash,
            delimiter,
            sheets,
            page,
        } = opened;
        i18n_ui::literal(&self.title, "title", &name);
        i18n_ui::literal(&self.title, "subtitle", &path);
        i18n_ui::bind(
            &self.delimiter,
            "tooltip-text",
            match delimiter.as_deref() {
                Some(",") => "Separator: comma",
                Some(";") => "Separator: semicolon",
                Some("\t") => "Separator: tab",
                _ => "Delimited file separator",
            },
        );
        self.updating.set(true);
        self.sheet.set_model(Some(&gtk::StringList::new(
            &sheets
                .iter()
                .map(|sheet| sheet.name.as_str())
                .collect::<Vec<_>>(),
        )));
        self.sheet.set_selected(0);
        self.sheet.set_visible(sheets.len() > 1);
        self.data.set_active(true);
        self.compact.set_active(false);
        self.editor_box.set_visible(false);
        let sql = sheets[0].sql.clone();
        {
            let mut state = self.state.borrow_mut();
            state.id = Some(id);
            state.hash = hash;
            if let Some(location) = state.location.as_mut() {
                location.1 = path;
            }
            state.sheets = sheets;
            state.sql_mode = false;
            state.sheet = 0;
        }
        self.editor.buffer().set_text(&sql);
        self.updating.set(false);
        self.render(Arc::new(page));
        self.restore_source();
    }
    fn render(self: &Rc<Self>, page: Arc<Page>) {
        self.bound_cells.borrow_mut().clear();
        self.selected_label.borrow_mut().take();
        let (rebuild, stamp, widths) = {
            let mut state = self.state.borrow_mut();
            state.selected = None;
            state.page = Some(page.clone());
            let key = (state.sheet, state.sql_mode);
            let changed = state.layout_columns != page.columns || state.layout_key != key;
            if changed {
                state.layout_stamp += 1;
                state.layout_columns = page.columns.clone();
                state.layout_key = key;
                let slot = state
                    .widths
                    .entry(key)
                    .or_insert_with(|| (page.columns.clone(), vec![220; page.columns.len()]));
                if slot.0 != page.columns {
                    *slot = (page.columns.clone(), vec![220; page.columns.len()]);
                }
            }
            (
                changed,
                state.layout_stamp,
                state
                    .widths
                    .get(&key)
                    .map(|slot| slot.1.clone())
                    .unwrap_or_else(|| vec![220; page.columns.len()]),
            )
        };
        if rebuild {
            // Appending a column can synchronously bind it to current rows.
            // Detach the old schema's model before installing new factories.
            self.rows.remove_all();
            while let Some(column) = self
                .grid
                .columns()
                .item(0)
                .and_downcast::<gtk::ColumnViewColumn>()
            {
                self.grid.remove_column(&column);
            }
            for (column, name) in page.columns.iter().enumerate() {
                let factory = gtk::SignalListItemFactory::new();
                let weak = Rc::downgrade(self);
                factory.connect_setup(move |_, object| {
                    let item = object.downcast_ref::<gtk::ListItem>().unwrap();
                    let label = gtk::Label::builder()
                        .xalign(0.0)
                        .yalign(0.0)
                        .hexpand(true)
                        .width_chars(1)
                        .selectable(true)
                        .margin_start(10)
                        .margin_end(10)
                        .margin_top(7)
                        .margin_bottom(7)
                        .build();
                    if let Some(ui) = weak.upgrade() {
                        ui.labels.borrow_mut().push(label.downgrade());
                    }
                    let target = item.downgrade();
                    let label_ref = label.downgrade();
                    let owner = weak.clone();
                    let pick: Rc<dyn Fn(bool)> = Rc::new(move |dialog| {
                        if let (Some(ui), Some(item), Some(label)) =
                            (owner.upgrade(), target.upgrade(), label_ref.upgrade())
                        {
                            if let Some(object) = item.item().and_downcast::<glib::BoxedAnyObject>()
                            {
                                let row = object.borrow::<Row>();
                                ui.select(row.page.clone(), row.index, column, &label, dialog);
                            }
                        }
                    });
                    let click = gtk::GestureClick::new();
                    click.set_button(1);
                    click.set_propagation_phase(gtk::PropagationPhase::Capture);
                    let picked = pick.clone();
                    click.connect_released(move |_, count, _, _| picked(count == 2));
                    label.add_controller(click);
                    let keys = gtk::EventControllerKey::new();
                    keys.connect_key_pressed(move |_, key, _, _| {
                        if matches!(key, gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter) {
                            pick(true);
                            glib::Propagation::Stop
                        } else {
                            glib::Propagation::Proceed
                        }
                    });
                    label.add_controller(keys);
                    item.set_child(Some(&label));
                });
                let weak = Rc::downgrade(self);
                factory.connect_bind(move |_, object| {
                    let item = object.downcast_ref::<gtk::ListItem>().unwrap();
                    if let (Some(ui), Some(row), Some(label)) = (
                        weak.upgrade(),
                        item.item().and_downcast::<glib::BoxedAnyObject>(),
                        item.child().and_downcast::<gtk::Label>(),
                    ) {
                        let row = row.borrow::<Row>();
                        let Some(cell) = row
                            .index
                            .checked_sub(row.page.offset)
                            .and_then(|index| row.page.rows.get(index))
                            .and_then(|row| row.get(column))
                        else {
                            label.set_text("");
                            return;
                        };
                        label.set_text(&format!(
                            "{}{}",
                            cell.text,
                            if cell.clipped { " …" } else { "" }
                        ));
                        let selected = ui.state.borrow().selected.as_ref().is_some_and(|s| {
                            Arc::ptr_eq(&s.page, &row.page)
                                && s.row == row.index
                                && s.column == column
                        });
                        if selected {
                            label.add_css_class("accent");
                        } else {
                            label.remove_css_class("accent");
                        }
                        if ui
                            .state
                            .borrow()
                            .page
                            .as_ref()
                            .is_some_and(|p| Arc::ptr_eq(p, &row.page))
                        {
                            ui.bound_cells
                                .borrow_mut()
                                .insert((row.index, column), label.downgrade());
                        }
                        i18n_ui::bind(
                            &label,
                            "tooltip-text",
                            if cell.clipped {
                                "Preview clipped · Enter or double-click for full cell"
                            } else {
                                "Enter or double-click for full cell content"
                            },
                        );
                        style_cell(&label, ui.wrap.is_active());
                        ui.labels
                            .borrow_mut()
                            .retain(|label| label.upgrade().is_some());
                    }
                });
                let title = name.chars().take(128).collect::<String>();
                let view_column = gtk::ColumnViewColumn::new(Some(&title), Some(factory));
                view_column.set_resizable(true);
                view_column.set_fixed_width(widths[column]);
                let weak = Rc::downgrade(self);
                view_column.connect_fixed_width_notify(move |view_column| {
                    if let Some(ui) = weak.upgrade() {
                        let mut state = ui.state.borrow_mut();
                        if state.layout_stamp == stamp {
                            let key = state.layout_key;
                            if let Some((_, widths)) = state.widths.get_mut(&key) {
                                widths[column] = view_column.fixed_width();
                            }
                        }
                    }
                });
                self.grid.append_column(&view_column);
            }
        }
        self.rows.splice(
            0,
            self.rows.n_items(),
            &(0..page.rows.len())
                .map(|index| {
                    glib::BoxedAnyObject::new(Row {
                        page: page.clone(),
                        index: page.offset + index,
                    })
                })
                .collect::<Vec<_>>(),
        );
        let state = self.state.borrow();
        let limited = state
            .sheets
            .get(state.sheet)
            .is_some_and(|sheet| sheet.truncated);
        let key = if page.total == 1 {
            "{first}–{last} of {total} row"
        } else {
            "{first}–{last} of {total} rows"
        };
        let mut parts = vec![Part::message(
            key,
            &[
                (
                    "first",
                    &if page.total == 0 { 0 } else { page.offset + 1 }.to_string(),
                ),
                ("last", &(page.offset + page.rows.len()).to_string()),
                ("total", &page.total.to_string()),
            ],
        )];
        if limited {
            parts.push(Part::message(
                "Source preview limited to 10,000 rows / 200 columns",
                &[],
            ));
        } else if page.truncated {
            parts.push(Part::message("Result limited", &[]));
        }
        for property in ["label", "tooltip-text"] {
            i18n_ui::bind_parts(&self.status, property, parts.clone(), " · ");
        }
    }
    fn show_cell(&self, text: &str) {
        let dialog = adw::Dialog::builder()
            .title("Cell content")
            .content_width(720)
            .content_height(480)
            .build()
            .ui_text("title", "Cell content");
        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&adw::HeaderBar::new());
        let view = gtk::TextView::builder()
            .editable(false)
            .cursor_visible(false)
            .wrap_mode(gtk::WrapMode::WordChar)
            .left_margin(16)
            .right_margin(16)
            .top_margin(12)
            .bottom_margin(12)
            .build();
        view.buffer().set_text(text);
        toolbar.set_content(Some(
            &gtk::ScrolledWindow::builder()
                .child(&view)
                .vexpand(true)
                .build(),
        ));
        dialog.set_child(Some(&toolbar));
        dialog.present(self.parent.upgrade().as_ref());
    }
    fn message(&self, source: &str) {
        for property in ["label", "tooltip-text"] {
            i18n_ui::bind(&self.status, property, source);
        }
    }
    fn ui_error(&self, source: &str) {
        self.error(&crate::language::tr(source));
        self.message(source);
    }
    fn show_chart(&self, chart: tables::Chart) {
        let dialog = adw::Dialog::builder()
            .title("Chart")
            .content_width(780)
            .content_height(560)
            .build()
            .ui_text("title", "Chart");
        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(
            &adw::WindowTitle::new("Chart", &chart.column.chars().take(160).collect::<String>())
                .ui_text("title", "Chart"),
        ));
        toolbar.add_top_bar(&header);
        let rows = gtk::Box::new(gtk::Orientation::Vertical, 10);
        rows.set_margin_start(16);
        rows.set_margin_end(16);
        rows.set_margin_top(12);
        rows.set_margin_bottom(12);
        for point in chart.points {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let label = gtk::Label::builder()
                .label(&point.label)
                .xalign(0.0)
                .width_chars(18)
                .max_width_chars(18)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .build();
            label.set_tooltip_text(Some(&point.label));
            row.append(&label);
            let negative = gtk::ProgressBar::builder()
                .hexpand(true)
                .inverted(true)
                .build();
            let positive = gtk::ProgressBar::builder().hexpand(true).build();
            let fraction = (point.value.abs() / chart.max_abs).clamp(0.0, 1.0);
            if point.value < 0.0 {
                negative.set_fraction(fraction);
            } else {
                positive.set_fraction(fraction);
            }
            row.append(&negative);
            row.append(&positive);
            row.append(
                &gtk::Label::builder()
                    .label(point.value.to_string())
                    .width_chars(12)
                    .xalign(1.0)
                    .build(),
            );
            rows.append(&row);
        }
        toolbar.set_content(Some(
            &gtk::ScrolledWindow::builder()
                .child(&rows)
                .vexpand(true)
                .build(),
        ));
        dialog.set_child(Some(&toolbar));
        dialog.present(self.parent.upgrade().as_ref());
    }
}
fn style_cell(label: &gtk::Label, wrap: bool) {
    label.set_wrap(wrap);
    label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    label.set_lines(if wrap { -1 } else { 1 });
    label.set_ellipsize(if wrap {
        gtk::pango::EllipsizeMode::None
    } else {
        gtk::pango::EllipsizeMode::End
    });
}

#[cfg(debug_assertions)]
impl Tables {
    pub fn smoke_comment_pending(&self) -> bool {
        self.state.borrow().pending_comment.is_some()
    }
    pub async fn smoke_comment(&self, kind: &str, capture: impl Fn()) -> Result<(), String> {
        async fn wait(test: impl Fn() -> bool) -> Result<(), String> {
            let now = std::time::Instant::now();
            while !test() {
                if now.elapsed() > std::time::Duration::from_secs(20) {
                    return Err("Table comment preparation timed out".into());
                }
                glib::timeout_future(std::time::Duration::from_millis(25)).await;
            }
            Ok(())
        }
        wait(|| {
            self.state.borrow().page.is_some() && self.state.borrow().pending_comment.is_none()
        })
        .await?;
        let (row, column) = match kind {
            "sql" => {
                let old = self.state.borrow().query_id.clone();
                self.sql_mode.set_active(true);
                self.editor.buffer().set_text("SELECT note AS quoted FROM data WHERE name = 'needle' ORDER BY CAST(score AS INTEGER) DESC");
                self.run_query();
                wait(|| {
                    self.state
                        .borrow()
                        .query_id
                        .as_ref()
                        .is_some_and(|id| Some(id) != old.as_ref())
                })
                .await?;
                (0, 0)
            }
            "xlsx" => {
                self.sheet.set_selected(1);
                self.page(0);
                wait(|| {
                    self.state
                        .borrow()
                        .page
                        .as_ref()
                        .is_some_and(|p| p.sheet == 1)
                })
                .await?;
                (0, 0)
            }
            _ => {
                self.data.set_active(true);
                self.page(56);
                wait(|| {
                    self.state.borrow().page.as_ref().is_some_and(|p| {
                        (p.offset..p.offset + p.rows.len()).contains(&56) && p.query_id.is_none()
                    })
                })
                .await?;
                (56, 2)
            }
        };
        self.mark_source(row, column);
        capture();
        if !self.comment.is_sensitive() {
            return Err("Native table comment stayed disabled".into());
        }
        if kind == "sql" {
            self.editor
                .buffer()
                .set_text("SELECT 'not the selected result' AS changed");
        }
        self.comment.emit_clicked();
        if !self.smoke_comment_pending() {
            return Err("Table comment did not freeze its pending target".into());
        }
        Ok(())
    }
    pub async fn smoke_comment_reopened(
        &self,
        expected: bool,
        sql: bool,
        capture: impl Fn(),
    ) -> Result<(), String> {
        let now = std::time::Instant::now();
        while self.state.borrow().restore_verified.is_none() {
            if now.elapsed() > std::time::Duration::from_secs(20) {
                return Err(format!(
                    "Table source reopening timed out: {}",
                    self.status.text()
                ));
            }
            glib::timeout_future(std::time::Duration::from_millis(25)).await;
        }
        if self.state.borrow().restore_verified != Some(expected)
            || self.state.borrow().selected.is_some() != expected
        {
            return Err(format!(
                "Table source verification failed: {}",
                self.status.text()
            ));
        }
        if expected && self.state.borrow().sql_mode != sql {
            return Err("SQL source reopened as a raw source cell".into());
        }
        if std::env::var("STUDIO_GNOME_SMOKE_SOURCE_VARIANT").as_deref() == Ok("changed")
            && (self.state.borrow().sql_mode || self.state.borrow().query_id.is_some())
        {
            return Err("Changed table silently reran a stored query".into());
        }
        capture();
        if expected {
            let state = self.state.borrow();
            let selected = state.selected.as_ref().unwrap();
            let label = self
                .bound_cells
                .borrow()
                .get(&(selected.row, selected.column))
                .and_then(|l| l.upgrade())
                .ok_or("Verified source cell was not brought into view")?;
            if !label.has_css_class("accent") {
                return Err("Verified table source cell lost its native highlight".into());
            }
        }
        Ok(())
    }
    pub async fn smoke_test(
        self: &Rc<Self>,
        directory: &std::path::Path,
        capture: impl Fn(&str),
        capture_dialog: impl Fn(&adw::Dialog, &str),
    ) -> Result<Value, String> {
        async fn until(test: impl Fn() -> bool) -> Result<(), String> {
            let start = std::time::Instant::now();
            while !test() {
                if start.elapsed() > std::time::Duration::from_secs(10) {
                    return Err("Native table widgets did not become ready".into());
                }
                glib::timeout_future(std::time::Duration::from_millis(25)).await;
            }
            Ok(())
        }
        fn find_label(widget: &gtk::Widget, prefix: &str) -> Option<gtk::Label> {
            if let Some(label) = widget.downcast_ref::<gtk::Label>() {
                if label.is_mapped() && label.text().starts_with(prefix) {
                    return Some(label.clone());
                }
            }
            let mut child = widget.first_child();
            while let Some(widget) = child {
                child = widget.next_sibling();
                if let Some(label) = find_label(&widget, prefix) {
                    return Some(label);
                }
            }
            None
        }
        fn click_cell(label: &gtk::Label, count: i32) -> Result<(), String> {
            let controllers = label.observe_controllers();
            let gesture = (0..controllers.n_items())
                .find_map(|index| controllers.item(index).and_downcast::<gtk::GestureClick>())
                .ok_or("Cell gesture was not installed")?;
            gesture.emit_by_name::<()>("released", &[&count, &0.0f64, &0.0f64]);
            Ok(())
        }
        let root = directory.join("project");
        until(|| {
            self.state
                .borrow()
                .page
                .as_ref()
                .is_some_and(|page| page.total == 131 && page.columns.len() == 3)
                && self.next.is_sensitive()
        })
        .await?;
        capture("table-data.png");
        let label = find_label(self.grid.upcast_ref(), "long value 中文")
            .ok_or("Long native table cell was not mounted")?;
        click_cell(&label, 1)?;
        self.copy.emit_clicked();
        until(|| self.copy.is_sensitive()).await?;
        if !self.status.text().contains("of 131 rows") {
            return Err(format!("Cell copy failed: {}", self.status.text()));
        }
        let copied = self
            .widget
            .clipboard()
            .read_text_future()
            .await
            .map_err(|error| error.to_string())?
            .ok_or("Full cell copy was empty")?;
        if copied.len() <= 2048 || !copied.ends_with("完整结束") {
            return Err("Copy used the clipped table preview rather than the complete cell".into());
        }
        click_cell(&label, 2)?;
        let parent = self.parent.upgrade().ok_or("No table window")?;
        until(|| parent.visible_dialog().is_some()).await?;
        let dialog = parent.visible_dialog().unwrap();
        crate::window_capture::ready(&dialog.child().ok_or("Cell dialog has no content")?).await?;
        capture_dialog(&parent.visible_dialog().unwrap(), "table-cell.png");
        parent.visible_dialog().unwrap().force_close();
        until(|| parent.visible_dialog().is_none()).await?;
        let height = label.height();
        let column = self
            .grid
            .columns()
            .item(2)
            .and_downcast::<gtk::ColumnViewColumn>()
            .unwrap();
        column.set_fixed_width(110);
        capture("table-wrap.png");
        if label.height() < height {
            return Err("Narrowing a wrapped table column reduced its content height".into());
        }
        self.wrap.set_active(false);
        capture("table-nowrap.png");
        if label.wraps() {
            return Err("Table cell wrapping did not turn off".into());
        }
        self.wrap.set_active(true);
        self.next.emit_clicked();
        until(|| {
            self.state
                .borrow()
                .page
                .as_ref()
                .is_some_and(|page| page.offset == 50)
        })
        .await?;
        if self
            .grid
            .columns()
            .item(2)
            .and_downcast::<gtk::ColumnViewColumn>()
            .unwrap()
            .fixed_width()
            != 110
        {
            return Err("Paging reset table column width".into());
        }
        self.previous.emit_clicked();
        until(|| {
            self.state
                .borrow()
                .page
                .as_ref()
                .is_some_and(|page| page.offset == 0)
        })
        .await?;
        self.sql_mode.set_active(true);
        until(|| self.editor_box.is_visible()).await?;
        if !self.sql_text().contains("\"name\",\n  \"score\"") {
            return Err("SQL did not enumerate all source columns".into());
        }
        if self.data.is_active() || !self.sql_mode.is_active() {
            return Err("SQL mode buttons disagree with the displayed view".into());
        }
        self.compact.set_active(true);
        until(|| {
            self.sql_text()
                .starts_with("SELECT \"name\", \"score\", \"note\"\n")
        })
        .await?;
        self.compact.set_active(false);
        until(|| self.sql_text().contains("\n  \"score\",\n")).await?;
        self.editor.buffer().set_text("SELECT name, CAST(score AS INTEGER)-127 AS score\nFROM data WHERE CAST(score AS INTEGER)>=125 ORDER BY score DESC");
        self.query.emit_clicked();
        until(|| {
            self.state
                .borrow()
                .page
                .as_ref()
                .is_some_and(|page| page.query_id.is_some() && page.rows.len() == 5)
        })
        .await?;
        if self.state.borrow().page.as_ref().unwrap().rows[0][1].text != "2" {
            return Err("SQL filtering, casting or ordering was lost".into());
        }
        capture("table-sql.png");
        let buffer = self
            .editor
            .buffer()
            .downcast::<sourceview::Buffer>()
            .map_err(|_| "SQL buffer is not GtkSourceView")?;
        if buffer
            .language()
            .is_none_or(|language| language.id() != "sql")
        {
            return Err("SQL syntax highlighting was not configured".into());
        }
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);
        if buffer
            .style_scheme()
            .is_none_or(|scheme| scheme.id() != "Adwaita-dark")
        {
            return Err("SQL editor did not follow dark appearance".into());
        }
        capture("table-sql-dark.png");
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceLight);
        self.chart.emit_clicked();
        until(|| parent.visible_dialog().is_some()).await?;
        let dialog = parent.visible_dialog().unwrap();
        crate::window_capture::ready(&dialog.child().ok_or("Chart dialog has no content")?).await?;
        capture_dialog(&parent.visible_dialog().unwrap(), "table-chart.png");
        parent.visible_dialog().unwrap().force_close();
        until(|| parent.visible_dialog().is_none()).await?;
        let query_id = self.state.borrow().query_id.clone();
        self.editor.buffer().set_text("DELETE FROM data");
        self.query.emit_clicked();
        until(|| self.status.text().contains("not authorized")).await?;
        if self.state.borrow().query_id != query_id
            || self.state.borrow().page.as_ref().unwrap().rows.len() != 5
        {
            return Err("A failed SQL query replaced the previous result".into());
        }
        self.data.set_active(true);
        until(|| {
            self.state
                .borrow()
                .page
                .as_ref()
                .is_some_and(|page| page.query_id.is_none() && page.total == 131)
        })
        .await?;
        if self
            .grid
            .columns()
            .item(2)
            .and_downcast::<gtk::ColumnViewColumn>()
            .unwrap()
            .fixed_width()
            != 110
        {
            return Err("Switching SQL/Data reset the data column width".into());
        }
        self.open(
            &root.to_string_lossy(),
            &root.join("book.xlsx").to_string_lossy(),
        );
        until(|| {
            self.state.borrow().sheets.len() == 2
                && self
                    .state
                    .borrow()
                    .page
                    .as_ref()
                    .is_some_and(|page| page.rows.len() == 2)
        })
        .await?;
        self.sheet.set_selected(1);
        until(|| {
            self.state
                .borrow()
                .page
                .as_ref()
                .is_some_and(|page| page.sheet == 1 && page.total == 1)
        })
        .await?;
        capture("table-xlsx.png");
        self.sheet.set_selected(0);
        until(|| {
            self.state
                .borrow()
                .page
                .as_ref()
                .is_some_and(|page| page.sheet == 0)
        })
        .await?;
        self.sql_mode.set_active(true);
        self.compact.set_active(true);
        until(|| self.sql_text().starts_with("SELECT \"name\", \"value\"\n")).await?;
        let first_sql = self.sql_text();
        self.sheet.set_selected(1);
        if self.compact.is_active() || self.sql_text() == first_sql {
            return Err("Worksheet SQL formatting leaked into another sheet".into());
        }
        let second_sql = "SELECT \"Second worksheet\" AS label FROM data";
        self.editor.buffer().set_text(second_sql);
        self.sheet.set_selected(0);
        if !self.compact.is_active() || self.sql_text() != first_sql {
            return Err("Worksheet SQL text or Compact state was not restored".into());
        }
        self.sheet.set_selected(1);
        if self.compact.is_active() || self.sql_text() != second_sql {
            return Err("Edited SQL was lost when switching worksheets".into());
        }
        self.open(
            &root.to_string_lossy(),
            &root.join("data.csv").to_string_lossy(),
        );
        self.open(
            &root.to_string_lossy(),
            &root.join("other.tsv").to_string_lossy(),
        );
        until(|| {
            self.state
                .borrow()
                .page
                .as_ref()
                .is_some_and(|page| page.total == 2 && page.rows[1][0].text == "other")
        })
        .await?;
        glib::timeout_future(std::time::Duration::from_millis(180)).await;
        if self.title.title() != "other.tsv" {
            return Err("A late table open replaced the newer file".into());
        }
        capture("table-tsv.png");
        let ticks = Rc::new(Cell::new(0usize));
        let gap = Rc::new(Cell::new(0u128));
        let previous = Rc::new(RefCell::new(std::time::Instant::now()));
        let (tick_count, max_gap, last) = (ticks.clone(), gap.clone(), previous.clone());
        let timer = glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
            let now = std::time::Instant::now();
            max_gap.set(
                max_gap
                    .get()
                    .max(now.duration_since(*last.borrow()).as_millis()),
            );
            last.replace(now);
            tick_count.set(tick_count.get() + 1);
            glib::ControlFlow::Continue
        });
        let started = std::time::Instant::now();
        self.open(
            &root.to_string_lossy(),
            &root.join("large.csv").to_string_lossy(),
        );
        let loaded = until(|| {
            self.state
                .borrow()
                .page
                .as_ref()
                .is_some_and(|page| page.total == 10000 && page.columns.len() == 40)
        })
        .await;
        let elapsed = started.elapsed().as_millis();
        timer.remove();
        loaded?;
        if elapsed > 100 && ticks.get() == 0 {
            return Err("Large table loading blocked the GTK timer".into());
        }
        if self.state.borrow().page.as_ref().unwrap().rows.len() * 40 > 512 {
            return Err("A large table exceeded the viewport cell budget".into());
        }
        capture("table-large.png");
        self.sql_mode.set_active(true);
        self.editor.buffer().set_text("SELECT CAST(c0 AS INTEGER) AS id, c1 FROM data WHERE CAST(c0 AS INTEGER)>=400 ORDER BY id DESC LIMIT 3");
        let query_ticks = Rc::new(Cell::new(0usize));
        let query_count = query_ticks.clone();
        let query_timer =
            glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
                query_count.set(query_count.get() + 1);
                glib::ControlFlow::Continue
            });
        let query_started = std::time::Instant::now();
        self.query.emit_clicked();
        let queried = until(|| {
            self.state
                .borrow()
                .page
                .as_ref()
                .is_some_and(|page| page.query_id.is_some() && page.rows.len() == 3)
        })
        .await;
        let query_elapsed = query_started.elapsed().as_millis();
        query_timer.remove();
        queried?;
        if self.state.borrow().page.as_ref().unwrap().rows[0][0].text != "399920" {
            return Err("Large-table SQL did not use the immutable imported rows".into());
        }
        capture("table-large-sql.png");
        let wide_started = std::time::Instant::now();
        self.open(
            &root.to_string_lossy(),
            &root.join("wide.csv").to_string_lossy(),
        );
        until(|| {
            self.state
                .borrow()
                .page
                .as_ref()
                .is_some_and(|page| page.total == 10000 && page.columns.len() == 200)
        })
        .await?;
        let wide_elapsed = wide_started.elapsed().as_millis();
        if self.state.borrow().page.as_ref().unwrap().rows.len() > 2 {
            return Err("A 200-column source exceeded the bounded viewport".into());
        }
        capture("table-wide.png");
        self.close();
        Ok(
            json!({"ok":true,"dataPaging":true,"fullCellCopy":true,"cellDialog":true,"wrapResize":true,"stableColumns":true,"sqlProjectionFilterSort":true,"compact":true,"readOnlySql":true,"previousResultRetained":true,"chart":true,"xlsxSheets":true,"lateOpenGuard":true,"largeOpenMs":elapsed,"largePreviewRows":10000,"largeColumns":40,"mainLoopTicks":ticks.get(),"maxMainLoopGapMs":gap.get(),"largeQueryMs":query_elapsed,"queryMainLoopTicks":query_ticks.get(),"wideOpenMs":wide_elapsed,"wideColumns":200}),
        )
    }
}
