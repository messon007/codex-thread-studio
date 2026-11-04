//! Native favorites library and explicit metadata editing, backed by Studio's API.
use crate::{
    favorites::{Favorite, Reply, Request, Summary},
    i18n_ui::{self, UiText},
    protocol::Command,
};
use adw::prelude::*;
use gtk::{gio, glib};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};
#[cfg(debug_assertions)]
#[path = "favorites_failure_acceptance.rs"]
mod failure_acceptance;
#[cfg(debug_assertions)]
#[path = "favorites_language_acceptance.rs"]
mod language_acceptance;
#[cfg(debug_assertions)]
#[path = "favorites_save_acceptance.rs"]
mod save_acceptance;
pub const DIAGRAM_KEY: u64 = 6;
pub struct Favorites {
    pub dialog: adw::Dialog,
    pub id: u64,
    pub diagrams: Rc<crate::chat_diagrams::Renderer>,
    pub on_source: RefCell<Option<Box<dyn Fn(Favorite)>>>,
    sender: tokio::sync::mpsc::Sender<Command>,
    styles: Rc<crate::content_widgets::Styles>,
    toast: adw::ToastOverlay,
    search: gtk::SearchEntry,
    rows: gio::ListStore,
    list: gtk::ListView,
    summary: gtk::Label,
    split: adw::OverlaySplitView,
    editor: gtk::Box,
    empty: adw::StatusPage,
    title: gtk::Entry,
    tags: gtk::Entry,
    note: gtk::TextView,
    location: gtk::Label,
    question: gtk::Label,
    save: gtk::Button,
    remove: gtk::Button,
    source: gtk::Button,
    copy: gtk::Button,
    export: gtk::Button,
    preview: gio::ListStore,
    views: gtk::Stack,
    source_box: gtk::Box,
    current: RefCell<Option<Favorite>>,
    new: Cell<bool>,
    dirty: Cell<bool>,
    updating: Cell<bool>,
    sequence: Cell<u64>,
    list_serial: Cell<u64>,
    detail_serial: Cell<u64>,
    write_serial: Cell<u64>,
    export_serial: Cell<u64>,
    export_writing: Cell<bool>,
    export_target: RefCell<Option<gio::File>>,
    feedback: Rc<crate::write_feedback::Queue>,
    export_picker: RefCell<Option<gio::Cancellable>>,
    prompt: Cell<bool>,
    close_after_save: Cell<bool>,
}
impl Favorites {
    #[cfg(debug_assertions)]
    pub(crate) async fn smoke_resource_save(&self) -> Result<Favorite, String> {
        Self::until(|| self.current.borrow().is_some() && self.new.get()).await?;
        let original = self.current.borrow().clone().unwrap();
        if original.scope != "selection" {
            return Err("Resource favorite lost its selection scope".into());
        }
        self.save.emit_clicked();
        Self::until(|| !self.dirty.get() && self.write_serial.get() == 0 && !self.new.get())
            .await?;
        let saved = self
            .current
            .borrow()
            .clone()
            .ok_or("Saved resource disappeared")?;
        if saved.content != original.content
            || saved.turn_id != original.turn_id
            || saved.item_id != original.item_id
        {
            return Err("Resource favorite changed its source or content after saving".into());
        }
        self.dialog.close();
        Ok(saved)
    }
    pub fn new(
        parent: &adw::ApplicationWindow,
        feedback: Rc<crate::write_feedback::Queue>,
        id: u64,
        sender: tokio::sync::mpsc::Sender<Command>,
        documents: tokio::sync::mpsc::Sender<crate::document::Message>,
        open: crate::content_widgets::OpenLink,
        images: Option<crate::chat_ui::ImageAction>,
    ) -> Rc<Self> {
        let dialog = adw::Dialog::builder()
            .title("Favorites")
            .content_width(1100)
            .content_height(760)
            .width_request(360)
            .height_request(420)
            .build()
            .ui_text("title", "Favorites");
        let toast = adw::ToastOverlay::new();
        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        let heading = adw::WindowTitle::new("", "")
            .ui_text("title", "Favorites")
            .ui_text("subtitle", "Shared with Codex Thread Studio");
        header.set_title_widget(Some(&heading));
        let sidebar = gtk::Button::builder()
            .icon_name("sidebar-show-symbolic")
            .tooltip_text("Show favorites list")
            .build()
            .ui_text("tooltip-text", "Show favorites list");
        header.pack_start(&sidebar);
        let export = gtk::Button::builder()
            .icon_name("document-save-as-symbolic")
            .tooltip_text("Export all favorites as Markdown")
            .build()
            .ui_text("tooltip-text", "Export all favorites as Markdown");
        header.pack_end(&export);
        toolbar.add_top_bar(&header);
        let search = gtk::SearchEntry::builder()
            .placeholder_text("Search favorites")
            .margin_start(12)
            .margin_end(12)
            .margin_top(12)
            .margin_bottom(6)
            .build()
            .ui_text("placeholder-text", "Search favorites");
        i18n_ui::accessible_label(&search, "Search favorites");
        let rows = gio::ListStore::new::<glib::BoxedAnyObject>();
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, item| {
            let label = gtk::Label::builder()
                .xalign(0.0)
                .wrap(true)
                .wrap_mode(gtk::pango::WrapMode::WordChar)
                .width_chars(1)
                .lines(5)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .margin_top(12)
                .margin_bottom(12)
                .margin_start(12)
                .margin_end(12)
                .build();
            item.downcast_ref::<gtk::ListItem>()
                .unwrap()
                .set_child(Some(&label));
        });
        factory.connect_bind(|_, item| {
            let item = item.downcast_ref::<gtk::ListItem>().unwrap();
            let object = item.item().and_downcast::<glib::BoxedAnyObject>().unwrap();
            let value = object.borrow::<Summary>();
            i18n_ui::literal(item, "accessible-label", &value.title);
            i18n_ui::literal(
                item,
                "accessible-description",
                &format!("{} · {}", value.thread_title, value.snippet),
            );
            item.child()
                .and_downcast::<gtk::Label>()
                .unwrap()
                .set_markup(&format!(
                    "<b>{}</b>\n{}\n<small>{}</small>",
                    glib::markup_escape_text(&value.title),
                    glib::markup_escape_text(&value.thread_title),
                    glib::markup_escape_text(&value.snippet)
                ));
        });
        let list = gtk::ListView::new(
            Some(gtk::SingleSelection::new(Some(rows.clone()))),
            Some(factory),
        );
        list.set_single_click_activate(true);
        let summary = gtk::Label::builder()
            .label("Loading favorites…")
            .xalign(0.0)
            .margin_start(12)
            .margin_end(12)
            .margin_bottom(6)
            .build()
            .ui_text("label", "Loading favorites…");
        summary.add_css_class("caption");
        let catalog = gtk::Box::new(gtk::Orientation::Vertical, 0);
        catalog.append(&search);
        catalog.append(&summary);
        catalog.append(
            &gtk::ScrolledWindow::builder()
                .child(&list)
                .vexpand(true)
                .hscrollbar_policy(gtk::PolicyType::Never)
                .build(),
        );
        let editor = gtk::Box::new(gtk::Orientation::Vertical, 8);
        editor.set_margin_start(16);
        editor.set_margin_end(16);
        editor.set_margin_top(12);
        editor.set_margin_bottom(12);
        let title = gtk::Entry::builder()
            .placeholder_text("Title")
            .max_length(512)
            .build()
            .ui_text("placeholder-text", "Title");
        i18n_ui::accessible_label(&title, "Favorite title");
        let tags = gtk::Entry::builder()
            .placeholder_text("Tags, separated by commas")
            .build()
            .ui_text("placeholder-text", "Tags, separated by commas");
        i18n_ui::accessible_label(&tags, "Favorite tags");
        let note = gtk::TextView::builder()
            .wrap_mode(gtk::WrapMode::WordChar)
            .top_margin(8)
            .bottom_margin(8)
            .left_margin(8)
            .right_margin(8)
            .build();
        i18n_ui::accessible_label(&note, "Favorite note");
        let note_scroll = gtk::ScrolledWindow::builder()
            .child(&note)
            .min_content_height(60)
            .max_content_height(100)
            .propagate_natural_height(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();
        note_scroll.add_css_class("card");
        let note_title = gtk::Label::builder()
            .xalign(0.0)
            .build()
            .ui_text("label", "Note");
        note_title.add_css_class("caption");
        let location = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .ellipsize(gtk::pango::EllipsizeMode::Middle)
            .build();
        location.add_css_class("caption");
        location.add_css_class("dim-label");
        let question = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .selectable(true)
            .lines(3)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        let question_expander = gtk::Expander::builder()
            .label("Question")
            .child(&question)
            .build()
            .ui_text("label", "Question");
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let save = gtk::Button::new().ui_text("label", "Save");
        save.add_css_class("suggested-action");
        let remove = gtk::Button::builder()
            .icon_name("user-trash-symbolic")
            .tooltip_text("Remove favorite")
            .build()
            .ui_text("tooltip-text", "Remove favorite");
        let source = gtk::Button::new().ui_text("label", "Open source");
        let copy = gtk::Button::builder()
            .icon_name("edit-copy-symbolic")
            .tooltip_text("Copy favorite")
            .build()
            .ui_text("tooltip-text", "Copy favorite");
        actions.append(&save);
        actions.append(&source);
        actions.append(&copy);
        actions.append(&gtk::Box::builder().hexpand(true).build());
        actions.append(&remove);
        let styles = crate::content_widgets::Styles::new();
        let diagrams = crate::chat_diagrams::Renderer::new_with_key(documents, DIAGRAM_KEY);
        let preview = gio::ListStore::new::<glib::BoxedAnyObject>();
        let preview_list = gtk::ListView::new(
            Some(gtk::NoSelection::new(Some(preview.clone()))),
            Some(crate::chat_ui::factory(
                open,
                styles.clone(),
                diagrams.clone(),
                None,
                images,
            )),
        );
        let views = gtk::Stack::builder().vexpand(true).build();
        let preview_page = views.add_titled(
            &gtk::ScrolledWindow::builder()
                .child(&preview_list)
                .vexpand(true)
                .hscrollbar_policy(gtk::PolicyType::Never)
                .build(),
            Some("preview"),
            "Preview",
        );
        i18n_ui::bind(&preview_page, "title", "Preview");
        let source_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let source_page = views.add_titled(&source_box, Some("source"), "Source");
        i18n_ui::bind(&source_page, "title", "markdown-mode\u{4}Source");
        let switcher = gtk::StackSwitcher::builder()
            .stack(&views)
            .halign(gtk::Align::Start)
            .build();
        for widget in [
            title.upcast_ref::<gtk::Widget>(),
            tags.upcast_ref(),
            note_title.upcast_ref(),
            note_scroll.upcast_ref(),
            actions.upcast_ref(),
            location.upcast_ref(),
            question_expander.upcast_ref(),
            switcher.upcast_ref(),
            views.upcast_ref(),
        ] {
            editor.append(widget);
        }
        let empty = adw::StatusPage::builder()
            .icon_name("starred-symbolic")
            .title("Your saved responses")
            .description("Save a message or select response text to add a favorite.")
            .build()
            .ui_text("title", "Your saved responses")
            .ui_text(
                "description",
                "Save a message or select response text to add a favorite.",
            );
        let detail = gtk::Box::new(gtk::Orientation::Vertical, 0);
        detail.append(&empty);
        detail.append(&editor);
        editor.set_visible(false);
        let split = adw::OverlaySplitView::builder()
            .sidebar(&catalog)
            .content(&detail)
            .min_sidebar_width(240.0)
            .max_sidebar_width(320.0)
            .sidebar_width_fraction(0.3)
            .build();
        let breakpoint =
            adw::Breakpoint::new(adw::BreakpointCondition::parse("max-width: 700sp").unwrap());
        breakpoint.add_setter(&split, "collapsed", Some(&true.to_value()));
        dialog.add_breakpoint(breakpoint);
        toolbar.set_content(Some(&split));
        toast.set_child(Some(&toolbar));
        dialog.set_child(Some(&toast));
        let ui = Rc::new(Self {
            dialog,
            id,
            sender,
            styles,
            diagrams,
            on_source: RefCell::new(None),
            toast,
            search,
            rows,
            list,
            summary,
            split,
            editor,
            empty,
            title,
            tags,
            note,
            location,
            question,
            save,
            remove,
            source,
            copy,
            export,
            preview,
            views,
            source_box,
            current: RefCell::new(None),
            new: Cell::new(false),
            dirty: Cell::new(false),
            updating: Cell::new(false),
            sequence: Cell::new(0),
            list_serial: Cell::new(0),
            detail_serial: Cell::new(0),
            write_serial: Cell::new(0),
            export_serial: Cell::new(0),
            export_writing: Cell::new(false),
            export_target: RefCell::new(None),
            feedback,
            export_picker: RefCell::new(None),
            prompt: Cell::new(false),
            close_after_save: Cell::new(false),
        });
        let weak = Rc::downgrade(&ui);
        sidebar.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.split.set_show_sidebar(!ui.split.shows_sidebar());
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.search.connect_search_changed(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.refresh();
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.list.connect_activate(move |_, index| {
            if let Some(ui) = weak.upgrade() {
                let object = ui.rows.item(index).and_downcast::<glib::BoxedAnyObject>();
                if let Some(object) = object {
                    let id = object.borrow::<Summary>().id.clone();
                    ui.select(id);
                }
            }
        });
        for entry in [&ui.title, &ui.tags] {
            let weak = Rc::downgrade(&ui);
            entry.connect_changed(move |_| {
                if let Some(ui) = weak.upgrade() {
                    ui.changed();
                }
            });
        }
        let weak = Rc::downgrade(&ui);
        ui.note.buffer().connect_changed(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.changed();
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.save.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.save();
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.remove.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.remove();
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.copy.connect_clicked(move |button| {
            if let Some(ui) = weak.upgrade() {
                if let Some(value) = ui.current.borrow().as_ref() {
                    button.clipboard().set_text(&format!(
                        "{}{}{}",
                        if value.question.is_empty() {
                            String::new()
                        } else {
                            format!("Question:\n{}\n\n", value.question)
                        },
                        value.content,
                        if value.note.is_empty() {
                            String::new()
                        } else {
                            format!("\n\nNote:\n{}", value.note)
                        }
                    ));
                }
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.source.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.open_source();
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.export.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.export();
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.views.connect_visible_child_name_notify(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.ensure_source();
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.dialog.connect_close_attempt(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.close_attempt();
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.dialog.connect_closed(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.cancel_export_picker();
            }
        });
        ui.refresh();
        ui.dialog.present(Some(parent));
        ui
    }
    fn next(&self) -> u64 {
        let next = self.sequence.get() + 1;
        self.sequence.set(next);
        next
    }
    fn request(&self, serial: u64, request: Request) -> bool {
        if self
            .sender
            .try_send(Command::Favorites {
                dialog: self.id,
                serial,
                request,
            })
            .is_err()
        {
            self.notice("Favorites request queue is full or disconnected");
            false
        } else {
            true
        }
    }
    fn error(&self, message: &str) {
        let toast = adw::Toast::new(message);
        toast.set_use_markup(false);
        self.toast.add_toast(toast);
    }
    fn notice(&self, source: &'static str) {
        let toast = adw::Toast::new("").ui_text("title", source);
        toast.set_use_markup(false);
        self.toast.add_toast(toast);
    }
    fn sync(&self) {
        let busy = self.write_serial.get() != 0 || self.export_serial.get() != 0;
        self.dialog
            .set_can_close(!busy && !self.dirty.get() && self.export_serial.get() == 0);
        self.title.set_sensitive(!busy);
        self.tags.set_sensitive(!busy);
        self.note.set_sensitive(!busy);
        self.save.set_sensitive(!busy && self.dirty.get());
        self.remove
            .set_sensitive(!busy && !self.new.get() && self.current.borrow().is_some());
        self.source
            .set_sensitive(!busy && self.current.borrow().is_some());
        self.export
            .set_sensitive(!busy && self.export_picker.borrow().is_none());
    }
    fn changed(&self) {
        if !self.updating.get() && self.current.borrow().is_some() {
            self.dirty.set(true);
            self.sync();
        }
    }
    fn refresh(&self) {
        let serial = self.next();
        self.list_serial.set(serial);
        self.request(serial, Request::List(self.search.text().to_string()));
    }
    pub fn busy(&self) -> bool {
        self.write_serial.get() != 0 || self.dirty.get() || self.export_serial.get() != 0
    }
    pub fn allow_window_close(&self) -> bool {
        self.cancel_export_picker();
        !self.busy()
    }
    pub fn project_path(&self) -> String {
        self.current
            .borrow()
            .as_ref()
            .map(|favorite| favorite.project_path.clone())
            .unwrap_or_default()
    }
    pub fn draft(&self, favorite: Favorite) {
        let serial = self.next();
        self.detail_serial.set(serial);
        self.editor.set_sensitive(false);
        if !self.request(serial, Request::Preview(favorite)) {
            self.editor.set_sensitive(true);
        }
    }
    fn select(self: &Rc<Self>, id: String) {
        if self.write_serial.get() != 0 || self.export_serial.get() != 0 {
            return;
        }
        if self.dirty.get() {
            let ui = self.clone();
            glib::MainContext::default().spawn_local(async move {
                if ui.discard("Open another favorite?").await {
                    ui.load(id);
                }
            });
        } else {
            self.load(id);
        }
    }
    fn load(&self, id: String) {
        let serial = self.next();
        self.detail_serial.set(serial);
        self.editor.set_sensitive(false);
        if !self.request(serial, Request::Get(id)) {
            self.editor.set_sensitive(true);
        }
    }
    async fn discard(&self, heading: &'static str) -> bool {
        if self.prompt.replace(true) {
            return false;
        }
        let alert = adw::AlertDialog::builder()
            .heading(heading)
            .body("Discard the unsaved changes to this favorite?")
            .build()
            .ui_text("heading", heading)
            .ui_text("body", "Discard the unsaved changes to this favorite?");
        alert.add_responses(&[("cancel", "Cancel"), ("discard", "Discard")]);
        i18n_ui::response(&alert, "cancel", "Cancel");
        i18n_ui::response(&alert, "discard", "Discard");
        alert.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
        alert.set_default_response(Some("cancel"));
        alert.set_close_response("cancel");
        let response = alert.choose_future(Some(&self.dialog)).await;
        self.prompt.set(false);
        if response == "discard" {
            self.dirty.set(false);
            self.sync();
            true
        } else {
            false
        }
    }
    fn show(&self, favorite: Favorite, preview: Arc<crate::model::Item>, new: bool) {
        self.updating.set(true);
        self.title.set_text(&favorite.title);
        self.tags.set_text(&favorite.tags.join(", "));
        self.note.buffer().set_text(&favorite.note);
        self.question.set_text(&favorite.question);
        self.location.set_text(&format!(
            "{} · {} · {}",
            favorite.backend, favorite.thread_title, favorite.created_at
        ));
        self.current.replace(Some(favorite));
        self.new.set(new);
        self.dirty.set(new);
        self.updating.set(false);
        self.diagrams.reset();
        crate::chat_ui::reconcile(&self.preview, &[preview]);
        while let Some(child) = self.source_box.first_child() {
            self.source_box.remove(&child);
        }
        self.ensure_source();
        self.empty.set_visible(false);
        self.editor.set_visible(true);
        self.editor.set_sensitive(true);
        if self.split.is_collapsed() {
            self.split.set_show_sidebar(false);
        }
        self.sync();
    }
    fn ensure_source(&self) {
        if self.views.visible_child_name().as_deref() == Some("source")
            && self.source_box.first_child().is_none()
        {
            if let Some(value) = self.current.borrow().as_ref() {
                let code = self.styles.full_code("markdown", &value.content);
                code.set_vexpand(true);
                self.source_box.append(&code);
            }
        }
    }
    fn save(&self) {
        if self.write_serial.get() != 0 {
            return;
        }
        let Some(mut favorite) = self.current.borrow().clone() else {
            return;
        };
        favorite.title = self.title.text().trim().into();
        favorite.tags = crate::favorites::tags(&self.tags.text());
        let buffer = self.note.buffer();
        favorite.note = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .to_string();
        if favorite.title.is_empty()
            || favorite.title.len() > 512
            || favorite.note.len() > 16 * 1024
            || favorite.tags.len() > 20
            || favorite.tags.iter().any(|tag| tag.len() > 128)
        {
            self.notice("Use a title up to 512 bytes, a note up to 16 KiB and at most 20 tags of 128 bytes each");
            return;
        }
        let serial = self.next();
        self.write_serial.set(serial);
        self.sync();
        if !self.request(
            serial,
            Request::Save {
                favorite,
                new: self.new.get(),
            },
        ) {
            self.write_serial.set(0);
            self.sync();
        }
    }
    fn remove(self: &Rc<Self>) {
        if self.write_serial.get() != 0 {
            return;
        }
        let Some(value) = self.current.borrow().clone() else {
            return;
        };
        let ui = self.clone();
        glib::MainContext::default().spawn_local(async move {
            if ui.prompt.replace(true) {
                return;
            }
            let alert = adw::AlertDialog::builder()
                .heading("Remove favorite?")
                .body("The conversation will remain unchanged.")
                .build()
                .ui_text("heading", "Remove favorite?")
                .ui_text("body", "The conversation will remain unchanged.");
            alert.add_responses(&[("cancel", "Cancel"), ("remove", "Remove")]);
            i18n_ui::response(&alert, "cancel", "Cancel");
            i18n_ui::response(&alert, "remove", "Remove");
            alert.set_response_appearance("remove", adw::ResponseAppearance::Destructive);
            alert.set_default_response(Some("cancel"));
            alert.set_close_response("cancel");
            let response = alert.choose_future(Some(&ui.dialog)).await;
            ui.prompt.set(false);
            if response == "remove" {
                let serial = ui.next();
                ui.write_serial.set(serial);
                ui.sync();
                if !ui.request(serial, Request::Delete(value.id)) {
                    ui.write_serial.set(0);
                    ui.sync();
                }
            }
        });
    }
    fn open_source(self: &Rc<Self>) {
        if self.write_serial.get() != 0 {
            return;
        }
        let ui = self.clone();
        glib::MainContext::default().spawn_local(async move {
            if ui.dirty.get() && !ui.discard("Open the source conversation?").await {
                return;
            }
            let value = ui.current.borrow().clone();
            if let Some(value) = value {
                ui.dialog.close();
                if let Some(callback) = ui.on_source.borrow().as_ref() {
                    callback(value);
                }
            }
        });
    }
    fn close_attempt(self: &Rc<Self>) {
        self.cancel_export_picker();
        if self.write_serial.get() != 0 || self.export_serial.get() != 0 {
            self.notice("Wait for the favorite change to finish");
            return;
        }
        // Reserve the confirmation before scheduling it. A second close can
        // otherwise start after the first response has closed this library.
        if self.prompt.replace(true) {
            return;
        }
        let ui = self.clone();
        glib::MainContext::default().spawn_local(async move {
            let alert = adw::AlertDialog::builder()
                .heading("Save favorite changes?")
                .body("Changes have not been saved.")
                .build()
                .ui_text("heading", "Save favorite changes?")
                .ui_text("body", "Changes have not been saved.");
            alert.add_responses(&[
                ("cancel", "Cancel"),
                ("discard", "Discard"),
                ("save", "Save"),
            ]);
            for (id, label) in [
                ("cancel", "Cancel"),
                ("discard", "Discard"),
                ("save", "Save"),
            ] {
                i18n_ui::response(&alert, id, label);
            }
            alert.set_response_appearance("save", adw::ResponseAppearance::Suggested);
            alert.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
            alert.set_default_response(Some("save"));
            alert.set_close_response("cancel");
            let response = alert.choose_future(Some(&ui.dialog)).await;
            ui.prompt.set(false);
            match response.as_str() {
                "discard" => {
                    ui.dirty.set(false);
                    ui.dialog.force_close();
                }
                "save" => {
                    ui.close_after_save.set(true);
                    ui.save();
                }
                _ => {}
            }
        });
    }
    fn export(self: &Rc<Self>) {
        if !self.dialog.is_mapped()
            || self.write_serial.get() != 0
            || self.export_serial.get() != 0
            || self.export_picker.borrow().is_some()
        {
            return;
        }
        let Some(parent) = self.dialog.root().and_downcast::<gtk::Window>() else {
            return;
        };
        let cancel = gio::Cancellable::new();
        self.export_picker.replace(Some(cancel.clone()));
        self.sync();
        let picker = gtk::FileDialog::builder()
            .title("Export favorites")
            .initial_name("codex-thread-studio-favorites.md")
            .build()
            .ui_text("title", "Export favorites");
        let weak = Rc::downgrade(self);
        let request = cancel.clone();
        picker.save(Some(&parent), Some(&cancel), move |selected| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            if ui.export_picker.borrow().as_ref() != Some(&request) {
                return;
            }
            ui.export_picker.borrow_mut().take();
            ui.sync();
            match selected {
                Ok(file) => ui.export_to(file),
                Err(error) => {
                    if !error.matches(gtk::DialogError::Dismissed)
                        && !error.matches(gtk::DialogError::Cancelled)
                        && !error.matches(gio::IOErrorEnum::Cancelled)
                    {
                        let toast = adw::Toast::new("");
                        toast.set_use_markup(false);
                        i18n_ui::bind_format(
                            &toast,
                            "title",
                            "Unable to open export chooser: {error}",
                            &[("error", &error.to_string())],
                        );
                        ui.toast.add_toast(toast);
                    }
                }
            }
        });
    }
    fn cancel_export_picker(&self) {
        let pending = self.export_picker.borrow_mut().take();
        if let Some(cancel) = pending {
            cancel.cancel();
            self.sync();
        }
    }
    fn export_to(&self, file: gio::File) {
        let serial = self.next();
        self.export_serial.set(serial);
        self.export_target.replace(Some(file));
        self.export.set_sensitive(false);
        self.sync();
        if !self.request(serial, Request::Export) {
            self.export_serial.set(0);
            self.export_target.borrow_mut().take();
            self.export.set_sensitive(true);
            self.sync();
        }
    }
    pub fn event(self: &Rc<Self>, serial: u64, result: Result<Reply, String>) {
        if serial == 0 {
            return;
        }
        match result {
            Ok(Reply::List(list)) if serial == self.list_serial.get() => {
                self.rows.splice(
                    0,
                    self.rows.n_items(),
                    &list
                        .items
                        .into_iter()
                        .map(glib::BoxedAnyObject::new)
                        .collect::<Vec<_>>(),
                );
                i18n_ui::bind_format(
                    &self.summary,
                    "label",
                    "{count} of {total} favorites",
                    &[
                        ("count", &list.total.to_string()),
                        ("total", &list.all_total.to_string()),
                    ],
                );
            }
            Ok(Reply::Detail { favorite, preview }) if serial == self.detail_serial.get() => {
                self.show(favorite, preview, false)
            }
            Ok(Reply::Draft { favorite, preview }) if serial == self.detail_serial.get() => {
                self.show(favorite, preview, true)
            }
            Ok(Reply::Saved { favorite, preview }) if serial == self.write_serial.get() => {
                self.write_serial.set(0);
                self.show(favorite, preview, false);
                self.refresh();
                self.notice("Favorite saved");
                if self.close_after_save.replace(false) {
                    self.dialog.close();
                }
            }
            Ok(Reply::Deleted) if serial == self.write_serial.get() => {
                self.write_serial.set(0);
                self.dirty.set(false);
                self.current.borrow_mut().take();
                self.preview.remove_all();
                self.diagrams.reset();
                self.editor.set_visible(false);
                self.empty.set_visible(true);
                self.sync();
                self.refresh();
                self.notice("Favorite removed");
            }
            Ok(Reply::Export(markdown)) if serial == self.export_serial.get() => {
                if self.export_writing.get() {
                    return; // A duplicate reply cannot start a second file write.
                }
                let Some(file) = self.export_target.borrow_mut().take() else {
                    self.export_serial.set(0);
                    self.sync();
                    self.notice("The export destination is no longer available");
                    return;
                };
                self.export_writing.set(true);
                let ui = self.clone();
                glib::MainContext::default().spawn_local(async move {
                    let result = async {
                        #[cfg(debug_assertions)]
                        ui.smoke_hold_write(&file).await?;
                        file.replace_contents_future(
                            markdown.into_bytes(),
                            None,
                            false,
                            gio::FileCreateFlags::REPLACE_DESTINATION,
                        )
                        .await
                        .map(|_| ())
                        .map_err(|(_, error)| error.to_string())
                    }
                    .await;
                    if ui.export_serial.get() != serial || !ui.export_writing.get() {
                        return;
                    }
                    ui.export_writing.set(false);
                    ui.export_serial.set(0);
                    ui.sync();
                    match result {
                        Ok(_) => ui.notice("Favorites exported"),
                        Err(error) => {
                            ui.notice("Favorites export failed");
                            ui.feedback.export_failed("Favorites", &file, &error);
                        }
                    }
                });
            }
            Err(error) => {
                if serial == self.export_serial.get() && self.export_writing.get() {
                    return; // Local I/O owns completion after the gateway reply.
                }
                let current = [
                    self.list_serial.get(),
                    self.detail_serial.get(),
                    self.write_serial.get(),
                    self.export_serial.get(),
                ]
                .contains(&serial);
                if serial == self.detail_serial.get() {
                    self.editor.set_sensitive(true);
                }
                if serial == self.list_serial.get() {
                    i18n_ui::literal(&self.summary, "label", &error);
                }
                if serial == self.write_serial.get() {
                    self.write_serial.set(0);
                    self.close_after_save.set(false);
                    self.sync();
                }
                if serial == self.export_serial.get() {
                    self.export_serial.set(0);
                    if let Some(file) = self.export_target.borrow_mut().take() {
                        self.feedback.export_failed("Favorites", &file, &error);
                    }
                    self.export.set_sensitive(true);
                }
                self.sync();
                if current {
                    self.error(&error);
                }
            }
            _ => {}
        }
    }
    pub fn gateway_failed(&self, error: &str) {
        self.write_serial.set(0);
        // An admitted GFile write no longer depends on gateway availability.
        // Keep its close/concurrency guard until its own completion arrives.
        if !self.export_writing.get() {
            self.export_serial.set(0);
            if let Some(file) = self.export_target.borrow_mut().take() {
                self.feedback.export_failed("Favorites", &file, error);
            }
        }
        self.close_after_save.set(false);
        self.editor.set_sensitive(true);
        self.export.set_sensitive(true);
        i18n_ui::bind(&self.summary, "label", "Favorites unavailable");
        self.sync();
        self.error(error);
    }
}
impl Drop for Favorites {
    fn drop(&mut self) {
        if let Some(cancel) = self.export_picker.get_mut().take() {
            cancel.cancel();
        }
        self.diagrams.reset();
    }
}

#[cfg(debug_assertions)]
impl Favorites {
    async fn until(test: impl Fn() -> bool) -> Result<(), String> {
        let start = std::time::Instant::now();
        while !test() {
            if start.elapsed() > std::time::Duration::from_secs(5) {
                return Err("Native favorites did not become ready".into());
            }
            glib::timeout_future(std::time::Duration::from_millis(25)).await;
        }
        Ok(())
    }
    async fn alert(parent: &adw::ApplicationWindow) -> Result<adw::AlertDialog, String> {
        Self::until(|| {
            parent
                .visible_dialog()
                .is_some_and(|dialog| dialog.is::<adw::AlertDialog>())
        })
        .await?;
        parent
            .visible_dialog()
            .and_downcast()
            .ok_or("Favorite confirmation was not shown".into())
    }
    fn respond(dialog: &adw::AlertDialog, label: &str) -> Result<(), String> {
        fn find(widget: &gtk::Widget, label: &str) -> Option<gtk::Button> {
            if let Some(button) = widget.downcast_ref::<gtk::Button>() {
                if button.label().as_deref() == Some(label) {
                    return Some(button.clone());
                }
            }
            let mut child = widget.first_child();
            while let Some(widget) = child {
                child = widget.next_sibling();
                if let Some(button) = find(&widget, label) {
                    return Some(button);
                }
            }
            None
        }
        let button = find(dialog.upcast_ref(), label)
            .ok_or_else(|| format!("Favorite confirmation button {label} was not found"))?;
        button.emit_clicked();
        Ok(())
    }
    pub async fn smoke_router_save(
        self: &Rc<Self>,
        origin: &crate::model::Origin,
    ) -> Result<(), String> {
        Self::until(|| self.current.borrow().is_some() && self.new.get()).await?;
        let favorite = self.current.borrow().clone().unwrap();
        if favorite.backend != origin.backend
            || favorite.thread_id != origin.thread
            || favorite.project_path != origin.root
            || favorite.item_id != origin.item
            || favorite.turn_id != origin.turn
            || favorite.question != *origin.question
        {
            return Err("Routed favorite lost its target source".into());
        }
        self.save.emit_clicked();
        Self::until(|| !self.dirty.get() && self.write_serial.get() == 0 && !self.new.get())
            .await?;
        self.dialog.close();
        Ok(())
    }
    pub async fn smoke_save(
        self: &Rc<Self>,
        parent: &adw::ApplicationWindow,
        capture: impl Fn(&adw::Dialog, &str),
    ) -> Result<Favorite, String> {
        Self::until(|| self.current.borrow().is_some() && self.new.get()).await?;
        let original = self.current.borrow().clone().unwrap();
        if original.scope != "message"
            || original.item_id != "history-message"
            || original.turn_id != "history-turn"
        {
            return Err("Message favorite lost its source anchor".into());
        }
        self.title.set_text("Native favorite 中文😀");
        self.tags.set_text("native，review, native");
        self.note.buffer().set_text("Shared native note 中文😀");
        if self.dialog.can_close() {
            return Err("Unsaved favorite allowed closing without a guard".into());
        }
        self.dialog.close();
        let alert = Self::alert(parent).await?;
        Self::respond(&alert, "Cancel")?;
        Self::until(|| !self.prompt.get()).await?;
        if !self.dirty.get() || self.title.text() != "Native favorite 中文😀" {
            return Err("Cancelling close discarded favorite changes".into());
        }
        self.save.emit_clicked();
        Self::until(|| !self.dirty.get() && self.write_serial.get() == 0 && !self.new.get())
            .await?;
        let saved = self.current.borrow().clone().unwrap();
        if saved.tags != ["native", "review"] || saved.note != "Shared native note 中文😀" {
            return Err("Favorite metadata did not round trip".into());
        }
        self.search.set_text("Shared native note");
        self.refresh();
        Self::until(|| self.rows.n_items() == 1).await?;
        self.copy.emit_clicked();
        let text = self
            .copy
            .clipboard()
            .read_text_future()
            .await
            .map_err(|error| error.to_string())?
            .ok_or("Favorite copy was empty")?;
        if !text.contains("Shared native note 中文😀") || !text.contains("# Native GNOME") {
            return Err("Copy favorite omitted its content or note".into());
        }
        self.views.set_visible_child_name("source");
        Self::until(|| self.source_box.first_child().is_some()).await?;
        crate::window_capture::next_frame(
            &self
                .dialog
                .child()
                .ok_or("Favorite dialog has no content")?,
        )
        .await?;
        capture(&self.dialog, "favorites-source.png");
        self.views.set_visible_child_name("preview");
        glib::timeout_future(std::time::Duration::from_millis(100)).await;
        capture(&self.dialog, "favorites-preview.png");
        self.search.set_text("");
        self.refresh();
        Ok(saved)
    }
    pub async fn smoke_selection(
        self: &Rc<Self>,
        parent: &adw::ApplicationWindow,
    ) -> Result<(), String> {
        Self::until(|| {
            self.new.get()
                && self
                    .current
                    .borrow()
                    .as_ref()
                    .is_some_and(|favorite| favorite.scope == "selection")
        })
        .await?;
        if self.current.borrow().as_ref().unwrap().content != "GTK4" {
            return Err("Selection favorite saved the entire response".into());
        }
        self.save.emit_clicked();
        Self::until(|| !self.dirty.get() && self.write_serial.get() == 0).await?;
        self.remove.emit_clicked();
        let alert = Self::alert(parent).await?;
        Self::respond(&alert, "Cancel")?;
        Self::until(|| !self.prompt.get()).await?;
        if self.current.borrow().is_none() {
            return Err("Cancel removed the favorite".into());
        }
        self.remove.emit_clicked();
        let alert = Self::alert(parent).await?;
        Self::respond(&alert, "Remove")?;
        Self::until(|| {
            self.current.borrow().is_none()
                && self.write_serial.get() == 0
                && self.rows.n_items() == 1
        })
        .await?;
        Ok(())
    }
    pub async fn smoke_export(
        self: &Rc<Self>,
        saved: &Favorite,
        path: &std::path::Path,
    ) -> Result<(), String> {
        self.load(saved.id.clone());
        Self::until(|| {
            self.current
                .borrow()
                .as_ref()
                .is_some_and(|favorite| favorite.id == saved.id)
        })
        .await?;
        let before = self.current.borrow().as_ref().unwrap().note.clone();
        let mut duplicate = saved.clone();
        duplicate.id = glib::uuid_string_random().to_string();
        self.draft(duplicate);
        Self::until(|| self.new.get()).await?;
        self.note
            .buffer()
            .set_text("This edit must not overwrite the saved note");
        self.save.emit_clicked();
        Self::until(|| self.write_serial.get() == 0).await?;
        if !self.dirty.get()
            || !self.new.get()
            || self.note.buffer().text(
                &self.note.buffer().start_iter(),
                &self.note.buffer().end_iter(),
                false,
            ) != "This edit must not overwrite the saved note"
        {
            return Err("A rejected duplicate save discarded the edited favorite".into());
        }
        self.select(saved.id.clone());
        let parent = self
            .dialog
            .root()
            .and_downcast::<adw::ApplicationWindow>()
            .ok_or("Missing native favorite parent")?;
        let alert = Self::alert(&parent).await?;
        Self::respond(&alert, "Discard")?;
        Self::until(|| {
            !self.dirty.get()
                && self
                    .note
                    .buffer()
                    .text(
                        &self.note.buffer().start_iter(),
                        &self.note.buffer().end_iter(),
                        false,
                    )
                    .as_str()
                    == before
        })
        .await?;
        self.export_to(gio::File::for_path(path));
        Self::until(|| self.export_serial.get() == 0).await?;
        self.source.emit_clicked();
        Ok(())
    }
}
