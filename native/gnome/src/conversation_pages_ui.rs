//! Native controls for read-only history; the live model remains authoritative.
use crate::{
    conversation_pages::{Mode, Page, Position, Request, Response, Source, Trail},
    i18n_ui::{self, UiText},
    protocol::Command,
};
use adw::prelude::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};
use tokio::sync::mpsc;

#[derive(Clone, Copy)]
enum Navigation {
    Open,
    Older,
    Newer,
}
pub struct Pager {
    pub widget: adw::WrapBox,
    older: gtk::Button,
    newer: gtk::Button,
    live: gtk::Button,
    summary: gtk::Label,
    spinner: gtk::Spinner,
    sender: mpsc::Sender<Command>,
    source: RefCell<Option<Source>>,
    enabled: Cell<bool>,
    serial: Cell<u64>,
    page: RefCell<Option<Page>>,
    position: RefCell<Position>,
    mode: Cell<Mode>,
    trail: RefCell<Trail>,
    pending: RefCell<Option<(Navigation, Position)>>,
    error: RefCell<String>,
    scroll: Cell<Option<bool>>,
    pub on_present: RefCell<Option<Box<dyn Fn()>>>,
}
impl Pager {
    pub fn new(sender: mpsc::Sender<Command>) -> Rc<Self> {
        let widget = crate::adaptive::toolbar(6);
        widget.set_margin_start(12);
        widget.set_margin_end(12);
        widget.set_margin_top(6);
        widget.set_margin_bottom(6);
        widget.set_visible(false);
        let older = gtk::Button::new()
            .ui_text("label", "Older")
            .ui_text("tooltip-text", "Read the previous history page");
        let newer = gtk::Button::new()
            .ui_text("label", "Newer")
            .ui_text("tooltip-text", "Return to the next history page");
        let live = gtk::Button::new().ui_text("label", "Live").ui_text(
            "tooltip-text",
            "Return to the live conversation; your draft is retained",
        );
        let spinner = gtk::Spinner::new();
        let summary = gtk::Label::builder()
            .xalign(0.0)
            .hexpand(true)
            .width_chars(1)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        summary.add_css_class("caption");
        for w in [
            older.upcast_ref::<gtk::Widget>(),
            newer.upcast_ref(),
            live.upcast_ref(),
            spinner.upcast_ref(),
            summary.upcast_ref(),
        ] {
            widget.append(w);
        }
        let ui = Rc::new(Self {
            widget,
            older,
            newer,
            live,
            summary,
            spinner,
            sender,
            source: RefCell::new(None),
            enabled: Cell::new(false),
            serial: Cell::new(0),
            page: RefCell::new(None),
            position: RefCell::new(Default::default()),
            mode: Cell::new(Mode::Items),
            trail: RefCell::new(Default::default()),
            pending: RefCell::new(None),
            error: RefCell::new(String::new()),
            scroll: Cell::new(None),
            on_present: RefCell::new(None),
        });
        let weak = Rc::downgrade(&ui);
        ui.older.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.navigate(Navigation::Older);
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.newer.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.navigate(Navigation::Newer);
            }
        });
        let weak = Rc::downgrade(&ui);
        ui.live.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.close();
            }
        });
        ui
    }
    pub fn sync(&self, source: Option<Source>, enabled: bool) -> bool {
        let changed = *self.source.borrow() != source;
        if changed {
            let had_page = self.active();
            self.cancel();
            self.source.replace(source);
            self.page.replace(None);
            self.trail.replace(Default::default());
            self.position.replace(Default::default());
            self.mode.set(Mode::Items);
            self.widget.set_visible(false);
            self.error.borrow_mut().clear();
            self.scroll.set(had_page.then_some(true));
        }
        self.enabled.set(enabled);
        self.repaint();
        changed
    }
    pub fn open(&self) {
        if self.page.borrow().is_none() {
            self.navigate(Navigation::Open);
        }
    }
    pub fn active(&self) -> bool {
        self.page.borrow().is_some()
    }
    pub fn items(&self) -> Option<Vec<Arc<crate::model::Item>>> {
        self.page.borrow().as_ref().map(|p| p.items.clone())
    }
    pub fn take_scroll(&self) -> Option<bool> {
        self.scroll.take()
    }
    pub fn needs_render(&self) -> bool {
        self.scroll.get().is_some()
    }
    fn cancel(&self) {
        self.serial.set(self.serial.get().wrapping_add(1));
        self.pending.replace(None);
        let _ = self.sender.try_send(Command::ConversationPage(None));
    }
    pub fn close(&self) {
        self.cancel();
        self.page.replace(None);
        self.trail.replace(Default::default());
        self.position.replace(Default::default());
        self.mode.set(Mode::Items);
        self.widget.set_visible(false);
        self.error.borrow_mut().clear();
        self.scroll.set(Some(true));
        self.present();
    }
    fn present(&self) {
        if let Some(callback) = self.on_present.borrow().as_ref() {
            callback();
        }
    }
    fn navigate(&self, navigation: Navigation) {
        if !self.enabled.get() || self.pending.borrow().is_some() {
            return;
        }
        let Some(source) = self.source.borrow().clone() else {
            return;
        };
        let position = match navigation {
            Navigation::Open => Position::default(),
            Navigation::Older => {
                let Some(position) = self.page.borrow().as_ref().and_then(|p| p.older.clone())
                else {
                    return;
                };
                let error = self
                    .trail
                    .borrow()
                    .can_push(&self.position.borrow())
                    .err()
                    .or_else(|| {
                        if self.trail.borrow().contains(&position) {
                            Some("Backend repeated its history navigation position".into())
                        } else {
                            None
                        }
                    });
                if let Some(error) = error {
                    self.error.replace(error);
                    self.repaint();
                    return;
                }
                position
            }
            Navigation::Newer => {
                let Some(p) = self.trail.borrow().last().cloned() else {
                    return;
                };
                p
            }
        };
        self.serial.set(self.serial.get().wrapping_add(1));
        self.pending.replace(Some((navigation, position.clone())));
        self.error.borrow_mut().clear();
        self.widget.set_visible(true);
        self.repaint();
        if self
            .sender
            .try_send(Command::ConversationPage(Some(Request {
                source,
                serial: self.serial.get(),
                position,
                mode: self.mode.get(),
            })))
            .is_err()
        {
            self.pending.replace(None);
            self.error
                .replace("History queue is unavailable; try again".into());
            self.repaint();
        }
    }
    pub fn apply(&self, response: Response) {
        if response.serial != self.serial.get()
            || self.source.borrow().as_ref() != Some(&response.source)
        {
            return;
        }
        let Some((navigation, position)) = self.pending.borrow_mut().take() else {
            return;
        };
        match response.result {
            Ok(page) => {
                match navigation {
                    Navigation::Open => {
                        self.trail.replace(Default::default());
                    }
                    Navigation::Older => {
                        let pushed = self.trail.borrow_mut().push(self.position.borrow().clone());
                        if let Err(error) = pushed {
                            self.error.replace(error);
                            self.repaint();
                            return;
                        }
                    }
                    Navigation::Newer => self.trail.borrow_mut().pop(),
                }
                self.mode.set(page.mode);
                self.position.replace(position);
                self.page.replace(Some(page));
                self.scroll
                    .set(Some(!matches!(navigation, Navigation::Newer)));
                self.repaint();
                self.present();
            }
            Err(error) => {
                self.error.replace(error);
                self.repaint();
            }
        }
    }
    fn repaint(&self) {
        if !self.widget.is_visible() {
            return;
        }
        let busy = self.pending.borrow().is_some();
        let page = self.page.borrow();
        self.older.set_sensitive(
            self.enabled.get() && !busy && page.as_ref().is_some_and(|p| p.older.is_some()),
        );
        self.newer
            .set_sensitive(self.enabled.get() && !busy && self.trail.borrow().len() > 0);
        self.live.set_sensitive(true);
        self.spinner.set_spinning(busy);
        self.spinner.set_visible(busy);
        let error = self.error.borrow();
        for property in ["label", "tooltip-text"] {
            if !error.is_empty() {
                // Provider diagnostics are opaque data, even when they happen
                // to equal a UI translation key.
                let error: String = error.chars().take(2000).collect();
                i18n_ui::bind_format(
                    &self.summary,
                    property,
                    "History read failed: {error}",
                    &[("error", &error)],
                );
            } else if busy {
                i18n_ui::bind(
                    &self.summary,
                    property,
                    "Loading history… Live delivery continues.",
                );
            } else if let Some(page) = page.as_ref() {
                let mut details = Vec::new();
                if page.mode == Mode::Legacy {
                    details.push("legacy read");
                }
                if page.partial {
                    details.push("server may omit older messages");
                }
                if page.excerpts {
                    details.push("some long outputs are excerpts");
                }
                i18n_ui::bind_summary(
                    &self.summary,
                    property,
                    if page.items.len() == 1 {
                        "History · page {page} · {count} item"
                    } else {
                        "History · page {page} · {count} items"
                    },
                    &[
                        ("page", &(self.trail.borrow().len() + 1).to_string()),
                        ("count", &page.items.len().to_string()),
                    ],
                    &details,
                );
            } else {
                i18n_ui::bind(
                    &self.summary,
                    property,
                    "History is unavailable; Live is retained.",
                );
            }
        }
    }
}

#[cfg(debug_assertions)]
impl Pager {
    pub fn smoke_older(&self) {
        self.older.emit_clicked();
    }
    pub fn smoke_newer(&self) {
        self.newer.emit_clicked();
    }
    pub fn smoke_live(&self) {
        self.live.emit_clicked();
    }
    pub fn smoke_busy(&self) -> bool {
        self.pending.borrow().is_some()
    }
    pub fn smoke_summary(&self) -> String {
        self.summary.text().into()
    }
    pub fn smoke_mode(&self) -> Option<Mode> {
        self.page.borrow().as_ref().map(|p| p.mode)
    }
}
