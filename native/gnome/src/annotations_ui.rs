//! Native session comments, preserving opaque shared records and stale-save protection.
use crate::{
    annotations::{self, Capture, Reply, Request, Snapshot},
    i18n_ui::{self, UiText},
    protocol::Command,
};
use adw::prelude::*;
use gtk::glib;
use serde_json::{json, Value};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};
use tokio::sync::mpsc;
#[cfg(debug_assertions)]
#[path = "annotations_language_acceptance.rs"]
mod language_acceptance;
/// Freeze the conversation owner before asynchronous content selection work.
pub struct CaptureTarget {
    pub owner: Box<dyn Fn() -> Option<String>>,
    pub capture: Box<dyn Fn(String, Capture) -> Result<(), String>>,
}
pub struct Panel {
    pub dialog: adw::Dialog,
    pub id: u64,
    pub key: String,
    parent: glib::WeakRef<adw::ApplicationWindow>,
    sender: mpsc::Sender<Command>,
    rows: gtk::ListBox,
    note: gtk::TextView,
    additional: gtk::TextView,
    excerpt: gtk::Label,
    location: gtk::Label,
    status: gtk::Label,
    save: gtk::Button,
    remove: gtk::Button,
    source: gtk::Button,
    insert: gtk::Button,
    send: gtk::Button,
    want_send: Cell<bool>,
    ticket: RefCell<Option<Arc<annotations::Submission>>>,
    cleanup: RefCell<Option<Arc<annotations::Submission>>>,
    conflict: Cell<bool>,
    reload: gtk::Button,
    snapshot: RefCell<Option<Snapshot>>,
    pending_capture: RefCell<Vec<Capture>>,
    selected: Cell<Option<usize>>,
    dirty: Cell<bool>,
    updating: Cell<bool>,
    busy: Cell<bool>,
    serial: Cell<u64>,
    guard: Cell<bool>,
    close_after: Cell<bool>,
    quit_after: Cell<bool>,
    pub on_send:
        RefCell<Option<Box<dyn Fn(Arc<annotations::Submission>, String) -> Result<(), String>>>>,
    pub on_source: RefCell<Option<Box<dyn Fn(Value, String) -> Result<(), String>>>>,
    pub on_prompt: RefCell<Option<Box<dyn Fn(String) -> Result<(), String>>>>,
}
impl Panel {
    fn notice(&self, source: &'static str) {
        i18n_ui::bind(&self.status, "label", source);
    }
    #[cfg(debug_assertions)]
    pub fn smoke_chat_records(&self) -> Option<Vec<Value>> {
        (!self.busy.get())
            .then(|| self.snapshot.borrow().as_ref().map(|s| s.drafts.clone()))
            .flatten()
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_chat_insert(&self) -> Result<(), String> {
        self.insert.emit_clicked();
        let started = std::time::Instant::now();
        while self.busy.get() {
            if started.elapsed() > std::time::Duration::from_secs(12) {
                return Err("Chat comment insertion timed out".into());
            }
            glib::timeout_future(std::time::Duration::from_millis(20)).await;
        }
        Ok(())
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_save_source_capture(&self, capture: &Capture) -> Result<Value, String> {
        async fn ready(p: &Panel) -> Result<(), String> {
            let start = std::time::Instant::now();
            while p.busy.get() || p.snapshot.borrow().is_none() {
                if start.elapsed() > std::time::Duration::from_secs(15) {
                    return Err("Source comment library timed out".into());
                }
                glib::timeout_future(std::time::Duration::from_millis(20)).await;
            }
            Ok(())
        }
        ready(self).await?;
        let record = self
            .snapshot
            .borrow()
            .as_ref()
            .unwrap()
            .drafts
            .last()
            .cloned()
            .ok_or("Source capture missing")?;
        if annotations::excerpt(&record) != capture.excerpt
            || annotations::source(&record) != capture.source
        {
            return Err(format!("Source selection changed while opening Comments: expected {:?}, actual {:?}; expected quote={:?}, actual quote={:?}", capture.source, annotations::source(&record), capture.excerpt.chars().take(80).collect::<String>(), annotations::excerpt(&record).chars().take(80).collect::<String>()));
        }
        self.note
            .buffer()
            .set_text("Please clarify source code 中文😀");
        self.save.emit_clicked();
        ready(self).await?;
        if self.dirty.get() {
            return Err(format!(
                "Source comment did not save: {}",
                self.status.text()
            ));
        }
        Ok(self
            .snapshot
            .borrow()
            .as_ref()
            .unwrap()
            .drafts
            .last()
            .cloned()
            .unwrap())
    }
    #[cfg(debug_assertions)]
    pub fn smoke_open_record(&self, id: &str) -> Result<bool, String> {
        if self.busy.get() || self.snapshot.borrow().is_none() {
            return Ok(false);
        }
        let index = self
            .snapshot
            .borrow()
            .as_ref()
            .unwrap()
            .drafts
            .iter()
            .position(|d| d["id"] == id)
            .ok_or("Comment fixture record was lost")?;
        self.rows
            .select_row(self.rows.row_at_index(index as i32).as_ref());
        if !self.source.is_sensitive() {
            return Err("Comment source action disabled".into());
        }
        self.source.emit_clicked();
        Ok(true)
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_test(
        self: &Rc<Self>,
        restore: bool,
        directory: &std::path::Path,
        capture: impl Fn(&adw::Dialog),
    ) -> Result<Value, String> {
        async fn ready(p: &Panel) -> Result<(), String> {
            let start = std::time::Instant::now();
            while p.busy.get() || p.snapshot.borrow().is_none() {
                if start.elapsed() > std::time::Duration::from_secs(15) {
                    return Err(format!("Comments timed out: {}", p.status.text()));
                }
                glib::timeout_future(std::time::Duration::from_millis(25)).await;
            }
            Ok(())
        }
        ready(self).await?;
        let drafts = self.snapshot.borrow().as_ref().unwrap().drafts.clone();
        let latest = drafts.last().ok_or("Captured comment missing")?;
        if annotations::excerpt(latest)
            != if restore {
                "Source **中文😀** selection."
            } else {
                "Source 中文😀 selection."
            }
            || latest["source"]["anchor"]["startLine"] != 3
            || latest["source"]["anchor"]["endLine"] != 3
        {
            return Err(format!("Selection/source line mapping failed: {latest}"));
        }
        if drafts[0]["source"]["provider"] != "future" || drafts[0]["source"]["version"] != 7 {
            return Err("Opaque existing comment source was lost".into());
        }
        if restore {
            if drafts.len() != 3 || annotations::note(&drafts[1]) != "Please clarify 中文😀" {
                return Err("Comments did not survive application restart".into());
            }
            self.remove.emit_clicked();
            self.save.emit_clicked();
            ready(self).await?;
            if self.dirty.get() || self.snapshot.borrow().as_ref().unwrap().drafts.len() != 2 {
                return Err("Comment removal did not save".into());
            }
            self.rows.select_row(self.rows.row_at_index(1).as_ref());
            capture(&self.dialog);
            self.source.emit_clicked();
            return Ok(
                json!({"ok":true,"restored":true,"sourceSelection":true,"sourceLines":true,"remove":true,"reopenSource":true}),
            );
        }
        self.note.buffer().set_text("Please clarify 中文😀");
        self.additional.buffer().set_text("Keep all rows.");
        self.save.emit_clicked();
        ready(self).await?;
        if self.dirty.get() {
            return Err(format!("Comment save failed: {}", self.status.text()));
        }
        capture(&self.dialog);
        self.insert.emit_clicked();
        ready(self).await?;
        self.insert.emit_clicked();
        ready(self).await?;
        std::fs::write(directory.join("comment-conflict-ready"), b"ready")
            .map_err(|e| e.to_string())?;
        let start = std::time::Instant::now();
        while !directory.join("comment-conflict-written").exists() {
            if start.elapsed() > std::time::Duration::from_secs(8) {
                return Err("External comment writer did not finish".into());
            }
            glib::timeout_future(std::time::Duration::from_millis(25)).await;
        }
        self.note
            .buffer()
            .set_text("Retain this conflicting local note");
        self.save.emit_clicked();
        ready(self).await?;
        if !self.dirty.get()
            || text(&self.note.buffer()) != "Retain this conflicting local note"
            || !self.status.text().contains("changed elsewhere")
        {
            return Err(format!(
                "Comment conflict lost local edits or its error: {}",
                self.status.text()
            ));
        }
        self.reload.emit_clicked();
        let prompt = gtk::Window::list_toplevels()
            .into_iter()
            .filter_map(|w| w.downcast::<adw::MessageDialog>().ok())
            .find(|w| {
                w.heading().as_deref() == Some(&crate::language::tr("Reload saved comments?"))
            })
            .ok_or("Comment reload did not offer cancellation")?;
        prompt.response("cancel");
        if !self.dirty.get() {
            return Err("Cancelling comment reload discarded local edits".into());
        }
        self.reload.emit_clicked();
        let prompt = gtk::Window::list_toplevels()
            .into_iter()
            .filter_map(|w| w.downcast::<adw::MessageDialog>().ok())
            .find(|w| {
                w.heading().as_deref() == Some(&crate::language::tr("Reload saved comments?"))
            })
            .ok_or("Comment reload guard missing")?;
        prompt.response("reload");
        ready(self).await?;
        if self.dirty.get()
            || self.snapshot.borrow().as_ref().unwrap().drafts[0]["note"] != "newer external writer"
        {
            return Err("Comment reload did not recover the newer shared data".into());
        }
        self.note.buffer().set_text("Unsaved comment edit");
        self.dialog.close();
        let prompt = gtk::Window::list_toplevels()
            .into_iter()
            .filter_map(|w| w.downcast::<adw::MessageDialog>().ok())
            .find(|w| w.heading().as_deref() == Some(&crate::language::tr("Save comment changes?")))
            .ok_or("Comment close did not protect unsaved input")?;
        prompt.response("cancel");
        if !self.dirty.get() || text(&self.note.buffer()) != "Unsaved comment edit" {
            return Err("Cancelling comment close lost edits".into());
        }
        self.dialog.close();
        let prompt = gtk::Window::list_toplevels()
            .into_iter()
            .filter_map(|w| w.downcast::<adw::MessageDialog>().ok())
            .find(|w| w.heading().as_deref() == Some(&crate::language::tr("Save comment changes?")))
            .ok_or("Comment discard guard missing")?;
        prompt.response("discard");
        Ok(
            json!({"ok":true,"previewSelection":true,"sourceLines":true,"opaqueSource":true,"saved":true,"prompt":true,"cancelClose":true,"discard":true,"conflictRetention":true,"reloadCancellation":true,"reloadRecovery":true}),
        )
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_provider(
        self: &Rc<Self>,
        provider: &str,
        restore: bool,
        capture: impl Fn(&adw::Dialog),
    ) -> Result<(Value, String), String> {
        async fn ready(panel: &Panel) -> Result<(), String> {
            let start = std::time::Instant::now();
            while panel.busy.get() || panel.snapshot.borrow().is_none() {
                if start.elapsed() > std::time::Duration::from_secs(20) {
                    return Err(format!(
                        "Provider comments did not load: {}",
                        panel.status.text()
                    ));
                }
                glib::timeout_future(std::time::Duration::from_millis(25)).await;
            }
            Ok(())
        }
        ready(self).await?;
        let records = self.snapshot.borrow().as_ref().unwrap().drafts.clone();
        if records.len() != 2 {
            return Err(format!("Provider comment count changed: {}", records.len()));
        }
        let record = records.last().unwrap();
        let source = annotations::source(record);
        let quote = annotations::excerpt(record).to_owned();
        if source["provider"] != provider || !self.source.is_sensitive() {
            return Err("Provider comment lost its source action".into());
        }
        if provider == "epub"
            && (!quote.starts_with("Paragraph 15.")
                || crate::epub::cfi_chapter(source["anchor"]["cfiRange"].as_str().unwrap_or(""))
                    != Some(1))
        {
            return Err("EPUB comment lost its range/section".into());
        }
        if provider == "browser" && quote != "Browser excerpt 中文😀" {
            return Err("Browser comment lost its Unicode selection".into());
        }
        if provider == "table"
            && !quote.starts_with("Cell passage 中文😀")
            && quote != "Second worksheet 中文😀"
        {
            return Err("Table comment lost the complete Unicode value".into());
        }
        if provider == "pdf"
            && (quote != "Page"
                || source["anchor"]["page"] != 2
                || source["anchor"]["rects"]
                    .as_array()
                    .is_none_or(|r| r.is_empty()))
        {
            return Err("PDF comment lost its page or selection geometry".into());
        }
        if !restore {
            self.note.buffer().set_text("Explain this passage 中文😀");
            self.save.emit_clicked();
            ready(self).await?;
            if self.dirty.get() {
                return Err("Provider comment did not save".into());
            }
            self.insert.emit_clicked();
            ready(self).await?;
        } else if annotations::note(record) != "Explain this passage 中文😀" {
            return Err("Provider comment did not survive restart".into());
        }
        capture(&self.dialog);
        self.source.emit_clicked();
        Ok((source, quote))
    }
    pub fn new(
        parent: &adw::ApplicationWindow,
        id: u64,
        key: String,
        title: &str,
        sender: mpsc::Sender<Command>,
        capture: Option<Capture>,
    ) -> Rc<Self> {
        let dialog = adw::Dialog::builder()
            .title("Comments")
            .content_width(880)
            .content_height(650)
            .build()
            .ui_text("title", "Comments");
        dialog.set_size_request(320, 240);
        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        let heading = adw::WindowTitle::new("", title).ui_text("title", "Comments");
        header.set_title_widget(Some(&heading));
        let save = gtk::Button::new().ui_text("label", "Save");
        save.add_css_class("suggested-action");
        header.pack_end(&save);
        let reload = gtk::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text("Reload saved comments")
            .build()
            .ui_text("tooltip-text", "Reload saved comments");
        header.pack_start(&reload);
        toolbar.add_top_bar(&header);
        let rows = gtk::ListBox::new();
        rows.set_selection_mode(gtk::SelectionMode::Single);
        rows.add_css_class("boxed-list");
        let note = editor();
        let additional = editor();
        i18n_ui::accessible_label(&note, "Comment text");
        i18n_ui::accessible_label(&additional, "Additional instructions");
        let excerpt = gtk::Label::builder()
            .wrap(true)
            .selectable(true)
            .xalign(0.0)
            .width_chars(1)
            .build();
        let location = gtk::Label::builder()
            .wrap(true)
            .lines(3)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .selectable(true)
            .xalign(0.0)
            .width_chars(1)
            .build();
        location.add_css_class("caption");
        location.add_css_class("dim-label");
        let details = gtk::Box::new(gtk::Orientation::Vertical, 12);
        details.set_margin_start(16);
        details.set_margin_end(16);
        details.set_margin_top(16);
        details.set_margin_bottom(16);
        details.append(&location);
        details.append(
            &gtk::ScrolledWindow::builder()
                .child(&excerpt)
                .max_content_height(180)
                .propagate_natural_height(true)
                .build(),
        );
        details.append(
            &gtk::Label::builder()
                .xalign(0.0)
                .build()
                .ui_text("label", "comment-field\u{4}Comment"),
        );
        details.append(
            &gtk::ScrolledWindow::builder()
                .child(&note)
                .min_content_height(110)
                .vexpand(true)
                .build(),
        );
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let source = gtk::Button::new().ui_text("label", "Open source");
        let remove = gtk::Button::new().ui_text("label", "Remove comment");
        actions.append(&source);
        actions.append(&remove);
        details.append(&actions);
        details.append(
            &gtk::Label::builder()
                .label("Additional instructions")
                .xalign(0.0)
                .build()
                .ui_text("label", "Additional instructions"),
        );
        details.append(
            &gtk::ScrolledWindow::builder()
                .child(&additional)
                .min_content_height(80)
                .build(),
        );
        let sidebar = gtk::ScrolledWindow::builder()
            .child(&rows)
            .min_content_width(220)
            .max_content_width(280)
            .build();
        let split = adw::OverlaySplitView::builder()
            .sidebar(&sidebar)
            .content(&gtk::ScrolledWindow::builder().child(&details).build())
            .sidebar_width_fraction(0.32)
            .build();
        let breakpoint = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
            adw::BreakpointConditionLengthType::MaxWidth,
            620.0,
            adw::LengthUnit::Sp,
        ));
        breakpoint.add_setter(&split, "collapsed", Some(&true.to_value()));
        dialog.add_breakpoint(breakpoint);
        let toggle = gtk::Button::builder()
            .icon_name("sidebar-show-symbolic")
            .tooltip_text("Show comment list")
            .build()
            .ui_text("tooltip-text", "Show comment list");
        header.pack_start(&toggle);
        let sidebar_toggle = split.clone();
        toggle.connect_clicked(move |_| {
            sidebar_toggle.set_show_sidebar(!sidebar_toggle.shows_sidebar())
        });
        toolbar.set_content(Some(&split));
        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        footer.set_margin_start(12);
        footer.set_margin_end(12);
        footer.set_margin_top(8);
        footer.set_margin_bottom(8);
        let status = gtk::Label::builder()
            .label("Loading comments…")
            .hexpand(true)
            .xalign(0.0)
            .wrap(true)
            .width_chars(1)
            .build()
            .ui_text("label", "Loading comments…");
        status.add_css_class("caption");
        let insert = gtk::Button::new().ui_text("label", "Insert into message");
        insert.set_valign(gtk::Align::Center);
        let send = gtk::Button::new().ui_text("label", "Send comments");
        send.set_valign(gtk::Align::Center);
        send.add_css_class("suggested-action");
        footer.append(&status);
        footer.append(&insert);
        footer.append(&send);
        toolbar.add_bottom_bar(&footer);
        dialog.set_child(Some(&toolbar));
        let p = Rc::new(Self {
            dialog,
            id,
            key,
            parent: parent.downgrade(),
            sender,
            rows,
            note,
            additional,
            excerpt,
            location,
            status,
            save,
            remove,
            source,
            insert,
            send,
            want_send: Cell::new(false),
            ticket: RefCell::new(None),
            cleanup: RefCell::new(None),
            conflict: Cell::new(false),
            reload,
            snapshot: RefCell::new(None),
            pending_capture: RefCell::new(capture.into_iter().collect()),
            selected: Cell::new(None),
            dirty: Cell::new(false),
            updating: Cell::new(false),
            busy: Cell::new(false),
            serial: Cell::new(0),
            guard: Cell::new(false),
            close_after: Cell::new(false),
            quit_after: Cell::new(false),
            on_source: RefCell::new(None),
            on_send: RefCell::new(None),
            on_prompt: RefCell::new(None),
        });
        let weak = Rc::downgrade(&p);
        p.rows.connect_row_selected(move |_, row| {
            if let Some(p) = weak.upgrade() {
                p.choose(row.map(|r| r.index() as usize));
            }
        });
        let weak = Rc::downgrade(&p);
        p.note.buffer().connect_changed(move |buffer| {
            if let Some(p) = weak.upgrade() {
                if p.updating.get() {
                    return;
                }
                if let Some(index) = p.selected.get() {
                    if let Some(s) = p.snapshot.borrow_mut().as_mut() {
                        if let Some(draft) = s.drafts.get_mut(index) {
                            annotations::set_note(draft, text(buffer));
                        }
                    }
                    p.changed();
                }
            }
        });
        let weak = Rc::downgrade(&p);
        p.additional.buffer().connect_changed(move |buffer| {
            if let Some(p) = weak.upgrade() {
                if !p.updating.get() {
                    if let Some(s) = p.snapshot.borrow_mut().as_mut() {
                        s.additional = text(buffer);
                    }
                    p.changed();
                }
            }
        });
        let weak = Rc::downgrade(&p);
        p.save.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.save();
            }
        });
        let weak = Rc::downgrade(&p);
        p.reload.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                p.reload_saved();
            }
        });
        let weak = Rc::downgrade(&p);
        p.remove.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                if let Some(index) = p.selected.get() {
                    let removed = {
                        let mut s = p.snapshot.borrow_mut();
                        s.as_mut()
                            .filter(|s| index < s.drafts.len())
                            .map(|s| s.drafts.remove(index))
                            .is_some()
                    };
                    if removed {
                        p.changed();
                        p.render();
                    }
                }
            }
        });
        let weak = Rc::downgrade(&p);
        p.source.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                if p.dirty.get() {
                    p.notice("Save comments before opening their source");
                    return;
                }
                if let Some(source) = p.selected.get().and_then(|i| {
                    p.snapshot
                        .borrow()
                        .as_ref()
                        .and_then(|s| s.drafts.get(i))
                        .map(annotations::source)
                }) {
                    if let Some(f) = p.on_source.borrow().as_ref() {
                        let excerpt = p
                            .selected
                            .get()
                            .and_then(|i| {
                                p.snapshot
                                    .borrow()
                                    .as_ref()
                                    .and_then(|s| s.drafts.get(i))
                                    .map(|d| annotations::excerpt(d).to_owned())
                            })
                            .unwrap_or_default();
                        match f(source, excerpt) {
                            Ok(()) => {
                                p.dialog.close();
                            }
                            Err(error) => i18n_ui::literal(&p.status, "label", &error),
                        }
                    }
                }
            }
        });
        let weak = Rc::downgrade(&p);
        p.insert.connect_clicked(move |_| {
            if let Some(p) = weak.upgrade() {
                let request = p.snapshot.borrow().as_ref().map(|s| Request::Prompt {
                    drafts: s.drafts.clone(),
                    additional: s.additional.clone(),
                });
                if let Some(request) = request {
                    p.request(request);
                }
            }
        });
        let weak = Rc::downgrade(&p);
        p.send.connect_clicked({
            let weak = Rc::downgrade(&p);
            move |_| {
                if let Some(p) = weak.upgrade() {
                    p.send_comments();
                }
            }
        });
        p.dialog.connect_close_attempt(move |_| {
            if let Some(p) = weak.upgrade() {
                p.close_guard();
            }
        });
        p.update();
        p.read();
        p
    }
    pub fn gateway_failed(&self) {
        self.serial.set(self.serial.get() + 1);
        self.busy.set(false);
        self.want_send.set(false);
        self.ticket.borrow_mut().take();
        self.close_after.set(false);
        self.quit_after.set(false);
        self.notice(if self.cleanup.borrow().is_some(){"Submission was accepted, but cleanup is unconfirmed. Comments are retained; retry cleanup when the gateway is available."}else{"Gateway disconnected. Comments are retained; check the conversation before sending again."});
        self.update();
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_submit(
        self: &Rc<Self>,
        mode: &str,
        directory: &std::path::Path,
        on_pending: impl Fn(),
        capture: impl Fn(&adw::Dialog),
    ) -> Result<Value, String> {
        async fn wait(test: impl Fn() -> bool) -> Result<(), String> {
            let start = std::time::Instant::now();
            while !test() {
                if start.elapsed() > std::time::Duration::from_secs(20) {
                    return Err("Comment submission deadline exceeded".into());
                }
                glib::timeout_future(std::time::Duration::from_millis(25)).await;
            }
            Ok(())
        }
        wait(|| !self.busy.get() && self.snapshot.borrow().is_some()).await?;
        self.note.buffer().set_text("Sent comment 中文😀");
        self.additional.buffer().set_text("Sent instructions");
        if mode == "cleanup" || mode == "cleanup-close" {
            std::fs::write(directory.join("lose-comment-cleanup-ack"), b"once")
                .map_err(|e| e.to_string())?;
        }
        if mode != "reject" {
            std::fs::write(directory.join("hold-comment-ack"), b"hold")
                .map_err(|e| e.to_string())?;
        }
        self.send.emit_clicked();
        wait(|| directory.join("comment-model-request").exists()).await?;
        if mode == "reject" {
            wait(|| self.ticket.borrow().is_none() && !self.busy.get()).await?;
            if self.status.text() != crate::language::tr("Submission was not confirmed. Comments are retained. Check the conversation before retrying.")
                || self.cleanup.borrow().is_some()
                || self.snapshot.borrow().as_ref().unwrap().drafts.len() != 2
            {
                return Err(format!(
                    "Rejected send consumed comments: {}",
                    self.status.text()
                ));
            }
            capture(&self.dialog);
            self.dialog.close();
            return Ok(json!({"ok":true,"rejectionRetainsComments":true}));
        }
        if self.ticket.borrow().is_none() {
            return Err(format!(
                "Comments did not await backend acceptance: {}",
                self.status.text()
            ));
        }
        self.send.emit_clicked(); // A second activation cannot enqueue the pending batch again.
        if mode != "cleanup-close" {
            self.note.buffer().set_text("New local note after send");
            self.additional.buffer().set_text("New local instructions");
        }
        on_pending();
        std::fs::write(directory.join("comment-submit-edit-ready"), b"ready")
            .map_err(|e| e.to_string())?;
        wait(|| directory.join("comment-submit-edit-written").exists()).await?;
        std::fs::remove_file(directory.join("hold-comment-ack")).map_err(|e| e.to_string())?;
        wait(|| self.ticket.borrow().is_none() && !self.busy.get()).await?;
        if mode == "cleanup" || mode == "cleanup-close" {
            if self.cleanup.borrow().is_none()
                || self.send.label().as_deref() != Some(&crate::language::tr("Retry cleanup"))
            {
                return Err("An accepted cleanup failure did not offer cleanup-only retry".into());
            }
            if mode == "cleanup-close" {
                self.parent
                    .upgrade()
                    .ok_or("Comment parent missing")?
                    .close();
                let guard = gtk::Window::list_toplevels()
                    .into_iter()
                    .filter_map(|w| w.downcast::<adw::MessageDialog>().ok())
                    .find(|w| {
                        w.heading().as_deref()
                            == Some(&crate::language::tr("Save comment changes?"))
                    })
                    .ok_or("Accepted cleanup failure did not protect application close")?;
                if guard.is_response_enabled("save") {
                    return Err("Accepted cleanup close offered a stale save".into());
                }
                guard.response("cancel");
            }
            self.send.emit_clicked();
            wait(|| !self.busy.get() && self.cleanup.borrow().is_none()).await?;
        }
        if mode == "cleanup-close" {
            if self.cleanup.borrow().is_some()
                || self.dirty.get()
                || self.snapshot.borrow().as_ref().unwrap().drafts.len() != 2
            {
                return Err("Cleanup retry after cancelled close lost shared comments".into());
            }
            capture(&self.dialog);
            self.dialog.close();
            return Ok(
                json!({"ok":true,"acceptedCleanup":true,"cleanupOnlyRetry":true,"cleanupCloseGuard":true}),
            );
        }
        if mode == "conflict" {
            if !self.conflict.get() || !self.dirty.get() || self.save.is_sensitive() {
                return Err("Concurrent edits did not require conflict resolution".into());
            }
            self.dialog.close();
            let guard = gtk::Window::list_toplevels()
                .into_iter()
                .filter_map(|w| w.downcast::<adw::MessageDialog>().ok())
                .find(|w| {
                    w.heading().as_deref() == Some(&crate::language::tr("Save comment changes?"))
                })
                .ok_or("Conflicting edits did not protect dialog close")?;
            if guard.is_response_enabled("save") {
                return Err("Conflict close guard allowed overwriting shared edits".into());
            }
            guard.response("cancel");
            capture(&self.dialog);
            self.reload.emit_clicked();
            let guard = gtk::Window::list_toplevels()
                .into_iter()
                .filter_map(|w| w.downcast::<adw::MessageDialog>().ok())
                .find(|w| {
                    w.heading().as_deref() == Some(&crate::language::tr("Reload saved comments?"))
                })
                .ok_or("Conflict reload guard missing")?;
            guard.response("reload");
            wait(|| !self.busy.get()).await?;
            if self.conflict.get()
                || self.dirty.get()
                || !self
                    .snapshot
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .drafts
                    .iter()
                    .any(|r| annotations::note(r) == "server changed the same comment")
            {
                return Err("Conflict recovery overwrote the shared comment".into());
            }
            self.dialog.close();
            return Ok(
                json!({"ok":true,"acceptedCleanup":true,"concurrentEditConflict":true,"saveBlocked":true,"reloadResolvesConflict":true}),
            );
        }
        if self.cleanup.borrow().is_some() || !self.dirty.get() || self.conflict.get() {
            return Err(format!(
                "Accepted comments were not merged with newer edits: {}",
                self.status.text()
            ));
        }
        let snapshot = self.snapshot.borrow().clone().unwrap();
        if snapshot.drafts.len() != 3
            || !snapshot
                .drafts
                .iter()
                .any(|d| annotations::note(d) == "New local note after send")
            || !snapshot.drafts.iter().any(|d| d["id"] == "server-new")
            || snapshot.additional != "New local instructions"
        {
            return Err("Accepted cleanup lost local or shared edits".into());
        }
        self.save.emit_clicked();
        wait(|| !self.busy.get()).await?;
        if self.dirty.get() {
            return Err(format!(
                "Newer comments could not be saved: {}",
                self.status.text()
            ));
        }
        capture(&self.dialog);
        self.dialog.close();
        Ok(
            json!({"ok":true,"acceptedCleanup":true,"newerLocalEdits":true,"newerSharedEdits":true,"duplicateSendGuard":true,"cleanupOnlyRetry":mode=="cleanup"}),
        )
    }
    fn send_comments(&self) {
        if self.busy.get() || self.ticket.borrow().is_some() || self.conflict.get() {
            return;
        }
        if self.cleanup.borrow().is_some() {
            self.finish_cleanup();
            return;
        }
        self.want_send.set(true);
        self.save();
    }
    pub fn submission_result(&self, ticket: &Arc<annotations::Submission>, accepted: bool) {
        let same = self.ticket.borrow().as_ref().is_some_and(|t| {
            t.dialog == ticket.dialog && t.serial == ticket.serial && t.key == ticket.key
        });
        if !same {
            return;
        }
        self.ticket.borrow_mut().take();
        if accepted {
            self.cleanup.replace(Some(ticket.clone()));
            self.finish_cleanup();
        } else {
            self.notice("Submission was not confirmed. Comments are retained. Check the conversation before retrying.");
            self.update();
        }
    }
    fn finish_cleanup(&self) {
        if self.busy.get() {
            return;
        }
        let ticket = self.cleanup.borrow().clone();
        let local = self.snapshot.borrow().clone();
        if let (Some(ticket), Some(local)) = (ticket, local) {
            self.request(Request::Consume {
                key: self.key.clone(),
                submitted: ticket.snapshot.clone(),
                local,
            });
        }
    }
    fn reload_saved(self: &Rc<Self>) {
        if self.busy.get() || self.ticket.borrow().is_some() || self.cleanup.borrow().is_some() {
            return;
        }
        if !self.dirty.get() {
            self.read();
            return;
        }
        let Some(parent) = self.parent.upgrade() else {
            return;
        };
        let dialog = adw::MessageDialog::builder()
            .transient_for(&parent)
            .heading("Reload saved comments?")
            .body("Reloading discards your unsaved comment edits.")
            .build()
            .ui_text("heading", "Reload saved comments?")
            .ui_text("body", "Reloading discards your unsaved comment edits.");
        dialog.add_responses(&[("cancel", "Cancel"), ("reload", "Discard and reload")]);
        i18n_ui::response(&dialog, "cancel", "Cancel");
        i18n_ui::response(&dialog, "reload", "Discard and reload");
        dialog.set_close_response("cancel");
        dialog.set_default_response(Some("cancel"));
        dialog.set_response_appearance("reload", adw::ResponseAppearance::Destructive);
        let weak = Rc::downgrade(self);
        dialog.connect_response(None, move |_, response| {
            if response == "reload" {
                if let Some(p) = weak.upgrade() {
                    p.dirty.set(false);
                    p.read();
                }
            }
        });
        dialog.present();
    }
    fn read(&self) {
        self.request(Request::Read(self.key.clone()));
    }
    fn request(&self, request: Request) {
        if self.busy.replace(true) {
            return;
        }
        let serial = self.serial.get() + 1;
        self.serial.set(serial);
        self.update();
        if self
            .sender
            .try_send(Command::Annotations {
                dialog: self.id,
                serial,
                request,
            })
            .is_err()
        {
            self.busy.set(false);
            self.want_send.set(false);
            self.notice("Native workers are busy; retry the operation");
            self.update();
        }
    }
    pub fn event(self: &Rc<Self>, serial: u64, result: Result<Reply, String>) {
        if serial != self.serial.get() {
            return;
        }
        self.busy.set(false);
        match result {
            Ok(Reply::Snapshot(s)) => {
                self.snapshot.replace(Some(s));
                self.dirty.set(false);
                self.conflict.set(false);
                self.render();
                let pending = std::mem::take(&mut *self.pending_capture.borrow_mut());
                for capture in pending {
                    self.add(capture);
                }
                self.notice(if self.dirty.get() {
                    "New comment · save to keep it"
                } else {
                    "Saved comments"
                });
                if self.want_send.get() {
                    let request = self.snapshot.borrow().as_ref().map(|s| Request::Prompt {
                        drafts: s.drafts.clone(),
                        additional: s.additional.clone(),
                    });
                    if let Some(request) = request {
                        self.request(request);
                        return;
                    }
                }
                if self.close_after.replace(false) {
                    if self.quit_after.replace(false) {
                        if let Some(parent) = self.parent.upgrade() {
                            parent.close();
                        }
                    } else {
                        self.dialog.force_close();
                    }
                }
            }
            Ok(Reply::Prompt(prompt)) => {
                if self.want_send.replace(false) {
                    let snapshot = Arc::new(self.snapshot.borrow().clone().unwrap());
                    let ticket = Arc::new(annotations::Submission {
                        dialog: self.id,
                        serial: self.serial.get(),
                        key: self.key.clone(),
                        snapshot,
                    });
                    self.ticket.replace(Some(ticket.clone()));
                    let result = self
                        .on_send
                        .borrow()
                        .as_ref()
                        .map(|f| f(ticket, prompt))
                        .unwrap_or_else(|| Err("No conversation is available".into()));
                    match result {
                        Ok(()) => {
                            if self.ticket.borrow().is_some() {
                                self.notice("Waiting for conversation acceptance…");
                            }
                        }
                        Err(error) => {
                            self.ticket.borrow_mut().take();
                            i18n_ui::literal(&self.status, "label", &error);
                        }
                    }
                    self.update();
                    return;
                }
                let result = self
                    .on_prompt
                    .borrow()
                    .as_ref()
                    .map(|f| f(prompt))
                    .unwrap_or_else(|| Err("No composer is available".into()));
                match result {
                    Ok(()) => self.notice(
                        "Inserted into message. Comments are retained until you remove them.",
                    ),
                    Err(error) => i18n_ui::literal(&self.status, "label", &error),
                }
            }
            Ok(Reply::Consumed(result)) => {
                self.cleanup.borrow_mut().take();
                self.snapshot.replace(Some(result.snapshot));
                self.dirty.set(result.dirty);
                self.conflict.set(result.conflict);
                self.render();
                self.notice(if result.conflict{"Submitted. Local and shared edits conflict; reload to resolve them. Your local edits are retained."}else if result.dirty{"Submitted. Newer edits are retained; save them to keep them."}else{"Submitted comments removed. Newer shared comments are retained."});
            }
            Err(error) => {
                self.want_send.set(false);
                self.close_after.set(false);
                self.quit_after.set(false);
                if self.cleanup.borrow().is_some() {
                    i18n_ui::bind_format(&self.status, "label", "Submission accepted; comment cleanup failed: {error}. Retry cleanup without sending again.", &[("error", &error)]);
                } else {
                    i18n_ui::literal(&self.status, "label", &error);
                }
            }
        }
        self.update();
    }
    pub fn add(&self, capture: Capture) {
        if capture.excerpt.trim().is_empty() || capture.excerpt.len() > 16384 {
            self.notice("Select a shorter passage for this comment");
            return;
        }
        if self.snapshot.borrow().is_none() || self.busy.get() {
            let mut pending = self.pending_capture.borrow_mut();
            if pending.len() < 32 {
                pending.push(capture);
            } else {
                self.notice("Finish the current comment operation first");
            }
            return;
        }
        let mut snapshot = self.snapshot.borrow_mut();
        let s = snapshot.as_mut().unwrap();
        if s.drafts.len() >= 32 {
            self.notice("A session can keep at most 32 comments");
            return;
        }
        s.drafts.push(json!({"id":glib::uuid_string_random().to_string(),"excerpt":capture.excerpt,"note":"","source":capture.source,"createdAt":glib::DateTime::now_utc().and_then(|d|d.format_iso8601()).map(|s|s.to_string()).unwrap_or_default()}));
        drop(snapshot);
        self.changed();
        self.render();
    }
    fn changed(&self) {
        self.dirty.set(true);
        self.notice("Unsaved comments");
        self.update();
    }
    fn render(&self) {
        self.updating.set(true);
        while let Some(row) = self.rows.first_child() {
            self.rows.remove(&row);
        }
        let values = self
            .snapshot
            .borrow()
            .as_ref()
            .map(|s| (s.drafts.clone(), s.additional.clone()));
        if let Some((drafts, additional)) = values {
            for (i, draft) in drafts.iter().enumerate() {
                let label = gtk::Label::builder()
                    .label(format!(
                        "{}. {}",
                        i + 1,
                        annotations::excerpt(draft)
                            .chars()
                            .take(100)
                            .collect::<String>()
                    ))
                    .xalign(0.0)
                    .wrap(true)
                    .width_chars(1)
                    .margin_top(12)
                    .margin_bottom(12)
                    .margin_start(12)
                    .margin_end(12)
                    .build();
                self.rows.append(&label);
                if let Some(row) = self.rows.row_at_index(i as i32) {
                    i18n_ui::literal_accessible_label(&row, &label.text());
                }
            }
            self.additional.buffer().set_text(&additional);
            if let Some(row) = self
                .rows
                .row_at_index(drafts.len().saturating_sub(1) as i32)
            {
                self.rows.select_row(Some(&row));
            } else {
                self.choose(None);
            }
        }
        self.updating.set(false);
        self.update();
    }
    fn show_location(&self, source: &Value) {
        Self::bind_location(&self.location, source);
    }
    fn bind_location(label: &gtk::Label, source: &Value) {
        use i18n_ui::Part;
        let anchor = &source["anchor"];
        let raw = |key: &str| anchor[key].as_str().unwrap_or("");
        let mut parts = Vec::new();
        let separator = match source["provider"].as_str() {
            Some("table") => {
                parts.push(Part::literal(format!(
                    "{} · {}!{}",
                    anchor["filePath"].as_str().unwrap_or("Table"),
                    anchor["sheet"].as_str().unwrap_or("Sheet"),
                    anchor["range"].as_str().unwrap_or("A1")
                )));
                if anchor["sql"].is_string() {
                    parts.push(Part::message("SQL result", &[]));
                }
                " · "
            }
            Some("pdf") => {
                parts.push(Part::literal(anchor["filePath"].as_str().unwrap_or("PDF")));
                parts.push(Part::message(
                    "Page {value}",
                    &[("value", &anchor["page"].as_u64().unwrap_or(1).to_string())],
                ));
                let count = anchor["rects"].as_array().map_or(0, Vec::len);
                parts.push(Part::message(
                    if count == 1 {
                        "{count} text region"
                    } else {
                        "{count} text regions"
                    },
                    &[("count", &count.to_string())],
                ));
                " · "
            }
            Some("chat") => {
                for (key, message) in [
                    ("sessionKey", "session {value}"),
                    ("turnId", "turn {value}"),
                    ("itemId", "item {value}"),
                ] {
                    if !raw(key).is_empty() {
                        parts.push(Part::message(message, &[("value", raw(key))]));
                    }
                }
                if parts.is_empty() {
                    parts.push(Part::message("Chat comment", &[]));
                }
                " / "
            }
            Some("epub") => {
                if !raw("filePath").is_empty() {
                    parts.push(Part::literal(raw("filePath")));
                }
                for (key, message) in [
                    ("chapterLabel", "chapter {value}"),
                    ("cfiRange", "EPUB CFI {value}"),
                    ("bookHash", "base {value}"),
                ] {
                    if !raw(key).is_empty() {
                        parts.push(Part::message(message, &[("value", raw(key))]));
                    }
                }
                " / "
            }
            Some("document") if !raw("filePath").is_empty() => {
                parts.push(Part::literal(raw("filePath")));
                let start = anchor["startLine"].as_u64().unwrap_or(0);
                let end = anchor["endLine"].as_u64().unwrap_or(start);
                if start > 0 {
                    let range = if end > start {
                        format!("{start}–{end}")
                    } else {
                        start.to_string()
                    };
                    parts.push(Part::message("line {value}", &[("value", &range)]));
                }
                if !raw("baseHash").is_empty() {
                    parts.push(Part::message("base {value}", &[("value", raw("baseHash"))]));
                }
                " / "
            }
            _ => {
                i18n_ui::literal(label, "label", &annotations::location(source));
                return;
            }
        };
        i18n_ui::bind_parts(label, "label", parts, separator);
    }
    fn choose(&self, index: Option<usize>) {
        self.selected.set(index);
        let old = self.updating.replace(true);
        let draft = index.and_then(|i| {
            self.snapshot
                .borrow()
                .as_ref()
                .and_then(|s| s.drafts.get(i))
                .cloned()
        });
        if let Some(draft) = &draft {
            i18n_ui::literal(&self.excerpt, "label", annotations::excerpt(draft));
            self.show_location(&annotations::source(draft));
        } else {
            i18n_ui::bind(
                &self.excerpt,
                "label",
                "Select text in a document and choose Comment to add it here.",
            );
            i18n_ui::literal(&self.location, "label", "");
        }
        self.note
            .buffer()
            .set_text(draft.as_ref().map(annotations::note).unwrap_or(""));
        self.updating.set(old);
        self.update();
    }
    fn update(&self) {
        let ready = self.snapshot.borrow().is_some() && !self.busy.get();
        self.note
            .set_sensitive(ready && self.selected.get().is_some());
        self.additional.set_sensitive(ready);
        let awaiting = self.ticket.borrow().is_some();
        let cleanup = self.cleanup.borrow().is_some();
        i18n_ui::bind(
            &self.send,
            "label",
            if cleanup {
                "Retry cleanup"
            } else {
                "Send comments"
            },
        );
        self.send.set_sensitive(
            ready
                && !awaiting
                && !self.conflict.get()
                && (cleanup
                    || self
                        .snapshot
                        .borrow()
                        .as_ref()
                        .is_some_and(|s| !s.drafts.is_empty())),
        );
        self.save.set_sensitive(
            ready && self.dirty.get() && !awaiting && !cleanup && !self.conflict.get(),
        );
        self.remove
            .set_sensitive(ready && self.selected.get().is_some());
        self.source.set_sensitive(
            ready
                && !awaiting
                && !cleanup
                && self
                    .selected
                    .get()
                    .and_then(|i| {
                        self.snapshot
                            .borrow()
                            .as_ref()
                            .and_then(|s| s.drafts.get(i))
                            .map(annotations::source)
                    })
                    .is_some_and(|s| annotations::reopen(&s).is_ok()),
        );
        self.insert.set_sensitive(
            ready
                && !awaiting
                && !cleanup
                && self
                    .snapshot
                    .borrow()
                    .as_ref()
                    .is_some_and(|s| !s.drafts.is_empty()),
        );
        self.reload
            .set_sensitive(!self.busy.get() && !awaiting && !cleanup);
        self.dialog
            .set_can_close(!self.busy.get() && !self.dirty.get() && !awaiting && !cleanup);
    }
    fn save(&self) {
        if self.busy.get() || self.ticket.borrow().is_some() || self.conflict.get() {
            return;
        }
        if let Some(s) = self.snapshot.borrow().clone() {
            self.request(Request::Save {
                key: self.key.clone(),
                snapshot: s,
            });
        }
    }
    pub fn allow_window_close(self: &Rc<Self>) -> bool {
        if !self.busy.get()
            && !self.dirty.get()
            && self.ticket.borrow().is_none()
            && self.cleanup.borrow().is_none()
        {
            return true;
        }
        self.quit_after.set(true);
        self.close_guard();
        false
    }
    fn close_guard(self: &Rc<Self>) {
        if self.busy.get() || self.ticket.borrow().is_some() {
            self.notice("Wait for the current comment operation to finish");
            return;
        }
        let accepted = self.cleanup.borrow().is_some();
        if !self.dirty.get() && !accepted {
            return;
        }
        if self.guard.replace(true) {
            return;
        }
        let Some(parent) = self.parent.upgrade() else {
            return;
        };
        let dialog = adw::MessageDialog::builder()
            .transient_for(&parent)
            .heading("Save comment changes?")
            .body(if accepted{"The submission was accepted. Closing keeps any remaining comments on disk and discards unsaved local edits. Check the conversation before sending those comments again."}else{"Unsaved comment edits will be lost if you discard them."})
            .build().ui_text("heading", "Save comment changes?");
        i18n_ui::bind(
            &dialog,
            "body",
            if accepted {
                "The submission was accepted. Closing keeps any remaining comments on disk and discards unsaved local edits. Check the conversation before sending those comments again."
            } else {
                "Unsaved comment edits will be lost if you discard them."
            },
        );
        dialog.add_responses(&[
            ("cancel", "Cancel"),
            (
                "discard",
                if accepted {
                    "Keep comments and close"
                } else {
                    "Discard"
                },
            ),
            ("save", "Save"),
        ]);
        i18n_ui::response(&dialog, "cancel", "Cancel");
        i18n_ui::response(
            &dialog,
            "discard",
            if accepted {
                "Keep comments and close"
            } else {
                "Discard"
            },
        );
        i18n_ui::response(&dialog, "save", "Save");
        let can_save = !accepted && !self.conflict.get() && self.snapshot.borrow().is_some();
        dialog.set_response_enabled("save", can_save);
        dialog.set_default_response(Some(if can_save { "save" } else { "cancel" }));
        dialog.set_close_response("cancel");
        dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
        let weak = Rc::downgrade(self);
        dialog.connect_response(None, move |_, answer| {
            if let Some(p) = weak.upgrade() {
                p.guard.set(false);
                match answer {
                    "save" => {
                        p.close_after.set(true);
                        p.save();
                    }
                    "discard" => {
                        p.cleanup.borrow_mut().take();
                        p.dirty.set(false);
                        if p.quit_after.replace(false) {
                            if let Some(parent) = p.parent.upgrade() {
                                parent.close();
                            }
                        } else {
                            p.dialog.force_close();
                        }
                    }
                    _ => p.quit_after.set(false),
                }
            }
        });
        dialog.present();
    }
}
fn editor() -> gtk::TextView {
    gtk::TextView::builder()
        .wrap_mode(gtk::WrapMode::WordChar)
        .left_margin(8)
        .right_margin(8)
        .top_margin(8)
        .bottom_margin(8)
        .build()
}
fn text(buffer: &gtk::TextBuffer) -> String {
    buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), false)
        .to_string()
}
