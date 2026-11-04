//! GTK query controls shared by reader query adapters. Evaluation stays in Rust.
use crate::{
    document, i18n_ui,
    reader_query::{Context, JqPage, Operation, Reply},
};
use adw::prelude::*;
use gtk::glib;
use sourceview::prelude::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use tokio::sync::{mpsc, oneshot};

/// The editor, actions, output slot and pager are independent of the query
/// language, so the Markdown SQL adapter can use the same native shell.
pub(crate) struct Controls {
    pub window: adw::Window,
    pub input: sourceview::View,
    pub options: adw::WrapBox,
    pub output: gtk::Box,
    pub tools: adw::WrapBox,
    pub status: gtk::Label,
    pub query: gtk::Button,
    pub previous: gtk::Button,
    pub next: gtk::Button,
    pub copy: gtk::Button,
    spinner: gtk::Spinner,
}
impl Controls {
    pub fn new(
        parent: &adw::ApplicationWindow,
        title: &'static str,
        name: &str,
        language: &str,
        input_label: &'static str,
    ) -> Self {
        let window = adw::Window::builder()
            .transient_for(parent)
            .default_width(900)
            .default_height(680)
            .build();
        i18n_ui::bind(&window, "title", title);
        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        let heading = adw::WindowTitle::new("", name);
        i18n_ui::bind(&heading, "title", title);
        header.set_title_widget(Some(&heading));
        toolbar.add_top_bar(&header);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let (input, _) = crate::reader::text_view(true);
        i18n_ui::accessible_label(&input, input_label);
        let buffer = input.buffer().downcast::<sourceview::Buffer>().unwrap();
        buffer.set_language(
            sourceview::LanguageManager::default()
                .language(language)
                .as_ref(),
        );
        let editor = gtk::ScrolledWindow::builder()
            .child(&input)
            .min_content_height(90)
            .vexpand(true)
            .build();
        let input_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        input_box.append(&editor);
        let options = crate::adaptive::toolbar(6);
        options.set_margin_start(12);
        options.set_margin_end(12);
        options.set_margin_top(6);
        options.set_margin_bottom(6);
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        let spinner = gtk::Spinner::new();
        let query = gtk::Button::with_label("Query");
        query.add_css_class("suggested-action");
        i18n_ui::bind(&query, "label", "Query");
        options.append(&spacer);
        options.append(&spinner);
        options.append(&query);
        input_box.append(&options);
        let output = gtk::Box::new(gtk::Orientation::Vertical, 0);
        output.set_vexpand(true);
        let split = gtk::Paned::builder()
            .orientation(gtk::Orientation::Vertical)
            .start_child(&input_box)
            .end_child(&output)
            .position(180)
            .resize_start_child(false)
            .shrink_start_child(false)
            .shrink_end_child(false)
            .vexpand(true)
            .build();
        body.append(&split);
        let footer = crate::adaptive::toolbar(6);
        footer.set_margin_start(12);
        footer.set_margin_end(12);
        footer.set_margin_top(6);
        footer.set_margin_bottom(6);
        let status = gtk::Label::builder()
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        i18n_ui::bind(&status, "label", "Read-only query results");
        let copy = gtk::Button::with_label("Copy");
        i18n_ui::bind(&copy, "label", "Copy");
        i18n_ui::bind(&copy, "tooltip-text", "Copy result page");
        let previous = gtk::Button::builder()
            .icon_name("go-previous-symbolic")
            .build();
        let next = gtk::Button::builder().icon_name("go-next-symbolic").build();
        i18n_ui::bind(&previous, "tooltip-text", "Previous result page");
        i18n_ui::bind(&next, "tooltip-text", "Next result page");
        for button in [&query, &copy, &previous, &next] {
            crate::adaptive::compact_button(button);
        }
        let tools = crate::adaptive::toolbar(6);
        tools.set_margin_start(12);
        tools.set_margin_end(12);
        tools.set_margin_top(6);
        tools.set_margin_bottom(6);
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        tools.append(&spacer);
        tools.append(&copy);
        output.append(&tools);
        footer.append(&status);
        footer.append(&previous);
        footer.append(&next);
        body.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        body.append(&footer);
        toolbar.set_content(Some(&body));
        window.set_content(Some(&toolbar));
        copy.set_sensitive(false);
        previous.set_sensitive(false);
        next.set_sensitive(false);
        Self {
            window,
            input,
            options,
            output,
            tools,
            status,
            query,
            previous,
            next,
            copy,
            spinner,
        }
    }
    pub(crate) fn busy(&self, busy: bool) {
        self.query.set_sensitive(!busy);
        self.input.set_editable(!busy);
        self.spinner.set_spinning(busy);
        if busy {
            self.previous.set_sensitive(false);
            self.next.set_sensitive(false);
            i18n_ui::bind(&self.status, "label", "Running query…");
        }
    }
}

pub struct Jq {
    pub context: Context,
    pub controls: Controls,
    sender: mpsc::Sender<document::Message>,
    result: RefCell<Option<JqPage>>,
    output: sourceview::View,
    slurp: gtk::CheckButton,
    sequence: Cell<u64>,
    busy: Cell<bool>,
    closed: Cell<bool>,
}

pub(crate) fn send(
    sender: &mpsc::Sender<document::Message>,
    context: &Context,
    serial: u64,
    operation: Operation,
) -> Result<oneshot::Receiver<Result<Reply, String>>, String> {
    let (reply, receive) = oneshot::channel();
    sender
        .try_send(document::Message {
            key: context.key,
            generation: context.generation,
            serial,
            request: document::Request::Query {
                revision: context.revision.clone(),
                operation,
                reply,
            },
        })
        .map_err(|_| "Reader query could not be sent".to_string())?;
    Ok(receive)
}
pub(crate) fn release_result(
    sender: &mpsc::Sender<document::Message>,
    context: &Context,
    operation: Operation,
) {
    if let Ok(receive) = send(sender, context, 0, operation) {
        glib::MainContext::default().spawn_local(async move {
            let _ = receive.await;
        });
    }
}

impl Jq {
    pub fn new(
        parent: &adw::ApplicationWindow,
        sender: mpsc::Sender<document::Message>,
        context: Context,
        name: &str,
    ) -> Rc<Self> {
        let controls = Controls::new(parent, "JSON query", name, "jq", "jq filter");
        controls.input.buffer().set_text(".");
        let slurp = gtk::CheckButton::with_label("Slurp input into an array");
        i18n_ui::bind(&slurp, "label", "Slurp input into an array");
        controls.options.prepend(&slurp);
        let (output, _) = crate::reader::text_view(false);
        i18n_ui::accessible_label(&output, "Query output");
        output
            .buffer()
            .downcast::<sourceview::Buffer>()
            .unwrap()
            .set_language(
                sourceview::LanguageManager::default()
                    .language("json")
                    .as_ref(),
            );
        controls.output.append(
            &gtk::ScrolledWindow::builder()
                .child(&output)
                .vexpand(true)
                .build(),
        );
        let ui = Rc::new(Self {
            context,
            controls,
            sender,
            result: RefCell::default(),
            output,
            slurp,
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
                if let Some(result) = ui.result.borrow().as_ref() {
                    ui.controls.window.clipboard().set_text(&result.text);
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
        ui
    }
    pub fn is_closed(&self) -> bool {
        self.closed.get()
    }
    pub fn close(&self) {
        self.release();
        self.controls.window.close();
    }
    pub fn present(&self) {
        self.controls.window.present();
    }
    fn release(&self) {
        if self.closed.replace(true) {
            return;
        }
        if let Some(result) = self.result.borrow_mut().take() {
            release_result(
                &self.sender,
                &self.context,
                Operation::JqClose { id: result.id },
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
        for view in [&self.controls.input, &self.output] {
            view.buffer()
                .downcast::<sourceview::Buffer>()
                .unwrap()
                .set_style_scheme(scheme.as_ref());
        }
    }
    fn run(self: &Rc<Self>) {
        let buffer = self.controls.input.buffer();
        // Checking the live character count avoids copying an unbounded paste
        // merely to reject it. The shared service enforces the UTF-8 byte cap.
        if buffer.char_count() > 16384 {
            self.error("Enter a jq filter of at most 16 KiB");
            return;
        }
        let filter = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .to_string();
        if filter.trim().is_empty() || filter.len() > 16384 {
            self.error("Enter a jq filter of at most 16 KiB");
            return;
        }
        self.request(Operation::Jq {
            filter,
            slurp: self.slurp.is_active(),
        });
    }
    fn page(self: &Rc<Self>, forward: bool) {
        let operation = self.result.borrow().as_ref().and_then(|result| {
            let page = if forward {
                result.page.checked_add(1)?
            } else {
                result.page.checked_sub(1)?
            };
            (page < result.pages).then(|| Operation::JqPage {
                id: result.id.clone(),
                page,
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
        let receive = match send(&self.sender, &self.context, sequence, operation) {
            Ok(receive) => receive,
            Err(error) => {
                self.error(&error);
                return;
            }
        };
        self.busy.set(true);
        self.controls.busy(true);
        self.slurp.set_sensitive(false);
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
                if let Ok(Reply::Jq(result)) = result {
                    release_result(&sender, &context, Operation::JqClose { id: result.id });
                }
                return;
            };
            ui.busy.set(false);
            ui.controls.busy(false);
            ui.slurp.set_sensitive(true);
            match result {
                Ok(Reply::Jq(page)) => ui.display(page),
                Ok(Reply::Closed) => {}
                Ok(_) => ui.error("Unexpected reader query response"),
                Err(error) => ui.error(&error),
            }
        });
    }
    fn display(&self, page: JqPage) {
        if let Some(old) = self
            .result
            .borrow_mut()
            .take()
            .filter(|old| old.id != page.id)
        {
            release_result(
                &self.sender,
                &self.context,
                Operation::JqClose { id: old.id },
            );
        }
        crate::source_surface::set_text(&self.output.buffer(), &page.text);
        self.controls.previous.set_sensitive(page.page > 0);
        self.controls.next.set_sensitive(page.page + 1 < page.pages);
        self.controls.copy.set_sensitive(true);
        i18n_ui::bind_format(
            &self.controls.status,
            "label",
            "Page {page} of {pages} · {count} total",
            &[
                ("page", &(page.page + 1).to_string()),
                ("pages", &page.pages.to_string()),
                ("count", &page.total_values.to_string()),
            ],
        );
        self.result.replace(Some(page));
    }
    fn error(&self, message: &str) {
        let message: String = message.chars().take(2048).collect();
        i18n_ui::literal(&self.controls.status, "label", &message);
        // An error leaves the previous output readable, with its own identity.
        if let Some(page) = self.result.borrow().as_ref() {
            self.controls.previous.set_sensitive(page.page > 0);
            self.controls.next.set_sensitive(page.page + 1 < page.pages);
        }
    }
}
