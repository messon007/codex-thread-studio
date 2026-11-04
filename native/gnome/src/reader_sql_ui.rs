//! Read-only Markdown table SQL using the shared query shell and table widgets.
use crate::{
    document, i18n_ui,
    reader_query::{Context, Operation, Reply, SqlPage, SqlSchema},
    reader_query_ui::{self, Controls},
};
use adw::prelude::*;
use gtk::{gio, glib};
use sourceview::prelude::*;
use std::collections::HashMap;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};
use tokio::sync::mpsc;

struct Row {
    page: Arc<SqlPage>,
    index: usize,
}

pub struct Sql {
    pub context: Context,
    pub table: usize,
    pub controls: Controls,
    sender: mpsc::Sender<document::Message>,
    schema: RefCell<Option<SqlSchema>>,
    result: RefCell<Option<Arc<SqlPage>>>,
    rows: gio::ListStore,
    grid: gtk::ColumnView,
    viewport: gtk::ScrolledWindow,
    columns: gtk::MenuButton,
    wrap: gtk::ToggleButton,
    compact: gtk::ToggleButton,
    reset: gtk::Button,
    help: sourceview::View,
    labels: RefCell<Vec<glib::WeakRef<gtk::Label>>>,
    layout: RefCell<Vec<String>>,
    visible: RefCell<Vec<usize>>,
    rendered: RefCell<Vec<(usize, gtk::ColumnViewColumn)>>,
    widths: RefCell<HashMap<usize, i32>>,
    sequence: Cell<u64>,
    busy: Cell<bool>,
    closed: Cell<bool>,
}

impl Sql {
    pub fn new(
        parent: &adw::ApplicationWindow,
        sender: mpsc::Sender<document::Message>,
        context: Context,
        table: usize,
        name: &str,
    ) -> Rc<Self> {
        let controls = Controls::new(parent, "Query table with SQL", name, "sql", "SQL query");
        let reset = gtk::Button::with_label("Reset query");
        i18n_ui::bind(&reset, "label", "Reset query");
        let compact = gtk::ToggleButton::with_label("Compact");
        i18n_ui::bind(&compact, "label", "Compact");
        let help_button = gtk::MenuButton::builder()
            .icon_name("dialog-information-symbolic")
            .build();
        i18n_ui::bind(&help_button, "tooltip-text", "Columns and query help");
        let (help, _) = crate::reader::text_view(false);
        i18n_ui::accessible_label(&help, "Columns and query help");
        let popover = gtk::Popover::new();
        popover.set_child(Some(
            &gtk::ScrolledWindow::builder()
                .child(&help)
                .min_content_width(380)
                .min_content_height(180)
                .max_content_height(300)
                .propagate_natural_height(true)
                .build(),
        ));
        help_button.set_popover(Some(&popover));
        controls.options.prepend(&help_button);
        controls.options.prepend(&compact);
        controls.options.prepend(&reset);
        let columns = gtk::MenuButton::builder().label("Columns").build();
        i18n_ui::bind(&columns, "label", "Columns");
        columns.set_sensitive(false);
        let wrap = gtk::ToggleButton::with_label("Wrap");
        i18n_ui::bind(&wrap, "label", "Wrap");
        wrap.set_active(true);
        controls.tools.remove(&controls.copy);
        controls.tools.append(&columns);
        controls.tools.append(&controls.copy);
        controls.tools.append(&wrap);
        let rows = gio::ListStore::new::<glib::BoxedAnyObject>();
        let grid = gtk::ColumnView::new(Some(gtk::NoSelection::new(Some(rows.clone()))));
        grid.add_css_class("studio-workspace");
        grid.set_show_row_separators(true);
        grid.set_show_column_separators(true);
        let viewport = gtk::ScrolledWindow::builder()
            .child(&grid)
            .vexpand(true)
            .build();
        controls.output.append(&viewport);
        let ui = Rc::new(Self {
            context,
            table,
            controls,
            sender,
            schema: RefCell::default(),
            result: RefCell::default(),
            rows,
            grid,
            viewport,
            columns,
            wrap,
            compact,
            reset,
            help,
            labels: RefCell::default(),
            layout: RefCell::default(),
            visible: RefCell::default(),
            rendered: RefCell::default(),
            widths: RefCell::default(),
            sequence: Cell::new(0),
            busy: Cell::new(false),
            closed: Cell::new(false),
        });
        let weak = Rc::downgrade(&ui);
        ui.controls.query.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.run();
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.reset.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                if let Some(schema) = ui.schema.borrow().as_ref() {
                    let sql = schema.default_sql.clone();
                    ui.controls.input.buffer().set_text(&sql);
                }
                ui.run();
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.compact.connect_toggled(move |button| {
            if let Some(ui) = weak.upgrade() {
                if !ui.busy.get() {
                    let sql = ui.sql_text();
                    ui.request(Operation::SqlFormat {
                        sql,
                        compact: button.is_active(),
                    });
                }
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.wrap.connect_toggled(move |button| {
            if let Some(ui) = weak.upgrade() {
                ui.labels.borrow_mut().retain(|label| {
                    if let Some(label) = label.upgrade() {
                        crate::tables_ui::style_cell(&label, button.is_active());
                        true
                    } else {
                        false
                    }
                });
            }
        });
        for (button, forward) in [(&ui.controls.previous, false), (&ui.controls.next, true)] {
            let weak = Rc::downgrade(&ui);
            button.connect_clicked(move |_| {
                if let Some(ui) = weak.upgrade() {
                    ui.page(forward);
                }
            });
        }
        let weak = Rc::downgrade(&ui);
        ui.controls.copy.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                if let Some(page) = ui.result.borrow().as_ref() {
                    ui.controls.window.clipboard().set_text(&page.copy_text);
                }
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.controls.window.connect_close_request(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.release();
            }
            glib::Propagation::Proceed
        });
        let weak = Rc::downgrade(&ui);
        adw::StyleManager::default().connect_dark_notify(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.style();
            }
        });
        ui.style();
        ui.controls.window.present();
        ui.request(Operation::SqlSchema { table });
        ui
    }
    pub fn is_closed(&self) -> bool {
        self.closed.get()
    }
    pub fn present(&self) {
        self.controls.window.present();
    }
    pub fn close(&self) {
        self.release();
        self.controls.window.close();
    }
    fn release(&self) {
        if self.closed.replace(true) {
            return;
        }
        if let Some(page) = self.result.borrow_mut().take() {
            reader_query_ui::release_result(
                &self.sender,
                &self.context,
                Operation::SqlClose {
                    id: page.id.clone(),
                },
            );
        }
    }
    fn style(&self) {
        let name = if adw::StyleManager::default().is_dark() {
            "Adwaita-dark"
        } else {
            "Adwaita"
        };
        let scheme = sourceview::StyleSchemeManager::default().scheme(name);
        for view in [&self.controls.input, &self.help] {
            view.buffer()
                .downcast::<sourceview::Buffer>()
                .unwrap()
                .set_style_scheme(scheme.as_ref());
        }
    }
    fn sql_text(&self) -> String {
        let buffer = self.controls.input.buffer();
        if buffer.char_count() > 65536 {
            return String::new();
        }
        buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .to_string()
    }
    fn run(self: &Rc<Self>) {
        if self.schema.borrow().is_none() {
            return;
        }
        let sql = self.sql_text();
        if sql.trim().is_empty() || sql.len() > 16384 {
            self.error("Enter one SELECT query of at most 16 KiB");
            return;
        }
        self.request(Operation::Sql {
            table: self.table,
            sql,
        });
    }
    fn page(self: &Rc<Self>, forward: bool) {
        let operation = self.result.borrow().as_ref().and_then(|page| {
            let row = if forward {
                page.row_start.checked_add(page.page_size)?
            } else {
                page.row_start.checked_sub(page.page_size)?
            };
            (row < page.total_rows).then(|| Operation::SqlPage {
                table: self.table,
                id: page.id.clone(),
                row,
            })
        });
        if let Some(operation) = operation {
            self.request(operation);
        }
    }
    fn request(self: &Rc<Self>, operation: Operation) {
        if self.closed.get() || self.busy.get() {
            return;
        }
        let sequence = self.sequence.get().wrapping_add(1);
        self.sequence.set(sequence);
        let receive = match reader_query_ui::send(&self.sender, &self.context, sequence, operation)
        {
            Ok(receive) => receive,
            Err(error) => {
                self.error(&error);
                return;
            }
        };
        self.busy.set(true);
        self.controls.busy(true);
        self.reset.set_sensitive(false);
        self.compact.set_sensitive(false);
        let weak = Rc::downgrade(self);
        let sender = self.sender.clone();
        let context = self.context.clone();
        glib::MainContext::default().spawn_local(async move {
            let result = receive
                .await
                .unwrap_or_else(|_| Err("Reader query was disconnected".into()));
            let Some(ui) = weak
                .upgrade()
                .filter(|ui| !ui.closed.get() && ui.sequence.get() == sequence)
            else {
                if let Ok(Reply::Sql(page)) = result {
                    reader_query_ui::release_result(
                        &sender,
                        &context,
                        Operation::SqlClose { id: page.id },
                    );
                }
                return;
            };
            ui.busy.set(false);
            ui.controls.busy(false);
            let ready = ui.schema.borrow().is_some();
            ui.reset.set_sensitive(ready);
            ui.compact.set_sensitive(ready);
            ui.controls.query.set_sensitive(ready);
            match result {
                Ok(Reply::Schema(schema)) => {
                    let help = format!(
                        "-- {} source rows\n-- data: _row, _line, {}\n-- t: {}\n\n{}",
                        schema.source_rows,
                        schema.columns.join(", "),
                        schema.positional_columns.join(", "),
                        schema.default_sql
                    );
                    crate::source_surface::set_text(&ui.help.buffer(), &help);
                    ui.controls.input.buffer().set_text(&schema.default_sql);
                    ui.schema.replace(Some(schema));
                    ui.reset.set_sensitive(true);
                    ui.compact.set_sensitive(true);
                    ui.run();
                }
                Ok(Reply::Sql(page)) => ui.display(Arc::new(page)),
                Ok(Reply::Formatted(sql)) => {
                    ui.controls.input.buffer().set_text(&sql);
                    ui.update_pager();
                }
                Ok(_) => ui.error("Unexpected reader query response"),
                Err(error) => ui.error(&error),
            }
        });
    }
    fn display(self: &Rc<Self>, page: Arc<SqlPage>) {
        if let Some(old) = self
            .result
            .borrow_mut()
            .take()
            .filter(|old| old.id != page.id)
        {
            reader_query_ui::release_result(
                &self.sender,
                &self.context,
                Operation::SqlClose { id: old.id.clone() },
            );
        }
        let changed = *self.layout.borrow() != page.columns;
        self.result.replace(Some(page.clone()));
        if changed {
            // Appending a GTK column binds it to existing model rows immediately.
            // Retire the previous projection before installing the new schema;
            // its rows can have fewer cells than the incoming columns.
            self.rows.remove_all();
            self.rendered.borrow_mut().clear();
            self.widths.borrow_mut().clear();
            self.layout.replace(page.columns.clone());
            self.visible.replace((0..page.columns.len()).collect());
            let weak = Rc::downgrade(self);
            let names = page.columns.clone();
            crate::table_columns_ui::install(
                &self.columns,
                page.columns.clone(),
                self.visible.borrow().clone(),
                512,
                Rc::new(move |columns| {
                    let Some(ui) = weak.upgrade().filter(|ui| {
                        !ui.closed.get() && !ui.busy.get() && *ui.layout.borrow() == names
                    }) else {
                        return false;
                    };
                    ui.visible.replace(columns.to_vec());
                    ui.rebuild_columns();
                    true
                }),
            );
            self.rebuild_columns();
        }
        let rows = (0..page.rows.len())
            .map(|index| {
                glib::BoxedAnyObject::new(Row {
                    page: page.clone(),
                    index,
                })
            })
            .collect::<Vec<_>>();
        self.rows.splice(0, self.rows.n_items(), &rows);
        self.viewport.vadjustment().set_value(0.0);
        self.columns.set_sensitive(true);
        self.controls.copy.set_sensitive(true);
        self.update_pager();
    }
    fn rebuild_columns(self: &Rc<Self>) {
        for (index, column) in self.rendered.borrow_mut().drain(..) {
            self.widths.borrow_mut().insert(index, column.fixed_width());
        }
        while let Some(column) = self
            .grid
            .columns()
            .item(0)
            .and_downcast::<gtk::ColumnViewColumn>()
        {
            self.grid.remove_column(&column);
        }
        self.labels.borrow_mut().clear();
        for index in self.visible.borrow().iter().copied() {
            let title = self.layout.borrow()[index].clone();
            let factory = gtk::SignalListItemFactory::new();
            let weak = Rc::downgrade(self);
            factory.connect_setup(move |_, object| {
                let item = object.downcast_ref::<gtk::ListItem>().unwrap();
                let label = crate::table_cell::plain_label();
                if let Some(ui) = weak.upgrade() {
                    crate::tables_ui::style_cell(&label, ui.wrap.is_active());
                    ui.labels.borrow_mut().push(label.downgrade());
                }
                item.set_child(Some(&label));
            });
            factory.connect_bind(move |_, object| {
                let item = object.downcast_ref::<gtk::ListItem>().unwrap();
                let object = item.item().and_downcast::<glib::BoxedAnyObject>().unwrap();
                let row = object.borrow::<Row>();
                let value = &row.page.rows[row.index][index];
                let label = item.child().and_downcast::<gtk::Label>().unwrap();
                label.set_text(value.as_deref().unwrap_or("NULL"));
                if value.is_none() {
                    label.add_css_class("dim-label");
                } else {
                    label.remove_css_class("dim-label");
                }
            });
            let column = gtk::ColumnViewColumn::new(Some(&title), Some(factory));
            column.set_resizable(true);
            column.set_fixed_width(self.widths.borrow().get(&index).copied().unwrap_or(220));
            self.grid.append_column(&column);
            self.rendered.borrow_mut().push((index, column));
        }
    }
    fn update_pager(&self) {
        if let Some(page) = self.result.borrow().as_ref() {
            self.controls
                .previous
                .set_sensitive(!self.busy.get() && page.row_start > 0);
            self.controls
                .next
                .set_sensitive(!self.busy.get() && page.row_end < page.total_rows);
            crate::table_columns_ui::row_status(
                &self.controls.status,
                page.row_start,
                page.row_end,
                page.total_rows,
            );
        }
    }
    fn error(&self, message: &str) {
        self.update_pager();
        let message: String = message.chars().take(2048).collect();
        i18n_ui::literal(&self.controls.status, "label", &message);
    }
}
