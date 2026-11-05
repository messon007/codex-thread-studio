//! Two independent, bounded read-only readers. Diff work stays in the gateway.
use crate::{
    document,
    i18n_ui::{self, UiText},
    protocol::Command,
    reader::Reader,
    workspace::{Comparison, Reply, Request, Response},
};
use adw::prelude::*;
use gtk::glib;
#[cfg(debug_assertions)]
use std::path::Path;
use std::{cell::RefCell, rc::Rc};
use tokio::sync::mpsc;

#[derive(Default)]
struct State {
    generation: u64,
    serial: u64,
    pair: Option<((String, String), (String, String))>,
    comparison: Option<Comparison>,
    change: Option<usize>,
    navigating: bool,
}

pub struct Compare {
    pub widget: gtk::Box,
    pub left: Rc<Reader>,
    pub right: Rc<Reader>,
    previous: gtk::Button,
    next: gtk::Button,
    back: gtk::Button,
    status: gtk::Label,
    sender: mpsc::Sender<Command>,
    toast: adw::ToastOverlay,
    state: RefCell<State>,
    pub on_reader: RefCell<Option<Box<dyn Fn()>>>,
}

impl Compare {
    #[cfg(debug_assertions)]
    pub async fn smoke_test(
        self: &Rc<Self>,
        directory: &Path,
        capture: impl Fn(&Path),
    ) -> Result<serde_json::Value, String> {
        async fn until(test: impl Fn() -> bool) -> Result<(), String> {
            let start = std::time::Instant::now();
            while !test() {
                if start.elapsed() > std::time::Duration::from_secs(10) {
                    return Err("Native comparison timed out".into());
                }
                glib::timeout_future(std::time::Duration::from_millis(40)).await;
            }
            Ok(())
        }
        let root = directory.join("project");
        let left = root.join("comparison-left.md");
        let right_directory = root.join("comparison-other");
        std::fs::create_dir_all(&right_directory).map_err(|error| error.to_string())?;
        let right = right_directory.join("comparison-right.md");
        std::fs::write(&left, "# Comparison\n\nSame text.\n\nBefore 中文😀\n")
            .map_err(|error| error.to_string())?;
        std::fs::write(
            &right,
            "# Comparison\n\nSame text.\n\nAfter 中文😀\n\nAdded\n",
        )
        .map_err(|error| error.to_string())?;
        self.open(Some((root.clone(), left.clone())));
        until(|| self.left.is_ready()).await?;
        if self.right.snapshot().is_some() {
            return Err("Right side opened unexpectedly".into());
        }
        crate::language::set("zh-CN", "en-US")?;
        i18n_ui::apply();
        if self.status.text() != "打开两个 Markdown 文件进行比较"
            || self.previous.label().as_deref() != Some("上一处更改")
            || self.next.label().as_deref() != Some("下一处更改")
            || self.back.label().as_deref() != Some("阅读器")
            || self.next.tooltip_text().as_deref() != Some("两侧均使用源码模式时才能跳转更改")
            || self.left.location().as_ref() != Some(&(root.clone(), left.clone()))
        {
            return Err("Live comparison translation missed controls or changed the file".into());
        }
        capture(&directory.join("compare-chinese-left.png"));
        crate::language::set("en-US", "en-US")?;
        i18n_ui::apply();
        capture(&directory.join("compare-left-only.png"));
        self.right.open_project_path(&root, &right, None, None);
        until(|| self.right.is_ready() && self.state.borrow().comparison.is_some()).await?;
        let summary = self.status.text().to_string();
        crate::language::set("zh-CN", "en-US")?;
        i18n_ui::apply();
        if !self.status.text().contains("处更改") || !self.status.text().contains("毫秒") {
            return Err("Loaded comparison summary did not follow the interface language".into());
        }
        if self.left.smoke_status_text() != "6 行 · 0.0 MB · 第 1 行"
            || self.right.smoke_status_text() != "8 行 · 0.0 MB · 第 1 行"
        {
            return Err(format!(
                "Comparison reader footers did not translate: {} / {}",
                self.left.smoke_status_text(),
                self.right.smoke_status_text()
            ));
        }
        capture(&directory.join("compare-chinese-preview.png"));
        crate::language::set("en-US", "en-US")?;
        i18n_ui::apply();
        if self.status.text() != summary {
            return Err("Locale switching changed comparison counts/timing".into());
        }
        if self.next.is_sensitive() {
            return Err("Preview change navigation must be disabled".into());
        }
        capture(&directory.join("compare-preview.png"));
        self.left.show_source();
        until(|| self.left.is_ready()).await?;
        if self.next.is_sensitive() {
            return Err("Mixed-mode navigation must be disabled".into());
        }
        self.right.show_source();
        until(|| self.right.is_ready() && self.next.is_sensitive()).await?;
        self.next.emit_clicked();
        until(|| {
            self.left.is_ready() && self.right.is_ready() && self.state.borrow().change == Some(0)
        })
        .await?;
        crate::language::set("zh-CN", "en-US")?;
        i18n_ui::apply();
        if self.next.tooltip_text().as_deref() != Some("跳转到有更改的源码段落")
            || self.state.borrow().change != Some(0)
            || !self.left.source_mode()
            || !self.right.source_mode()
        {
            return Err("Source comparison translation changed navigation state".into());
        }
        crate::language::set("en-US", "en-US")?;
        i18n_ui::apply();
        capture(&directory.join("compare-source.png"));
        self.left.show_preview();
        until(|| self.left.is_ready()).await?;
        if self.next.is_sensitive() {
            return Err("Independent Preview switch failed".into());
        }
        if std::fs::read_to_string(&left).unwrap()
            != "# Comparison\n\nSame text.\n\nBefore 中文😀\n"
        {
            return Err("Read-only comparison changed a file".into());
        }
        Ok(
            serde_json::json!({"ok":true,"leftFirst":true,"differentDirectories":true,"independentModes":true,"sourceNavigation":true,"previewNavigationDisabled":true,"readOnly":true,"liveLanguage":true,"literalFileIdentity":true}),
        )
    }
    pub fn new(
        window: &adw::ApplicationWindow,
        documents: mpsc::Sender<document::Message>,
        sender: mpsc::Sender<Command>,
        toast: &adw::ToastOverlay,
    ) -> Rc<Self> {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        toolbar.add_css_class("toolbar");
        toolbar.set_margin_start(8);
        toolbar.set_margin_end(8);
        toolbar.set_margin_top(8);
        toolbar.set_margin_bottom(8);
        let status = gtk::Label::builder()
            .label("Open two Markdown files to compare")
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build()
            .ui_text("label", "Open two Markdown files to compare");
        status.add_css_class("caption");
        let previous = gtk::Button::new().ui_text("label", "Previous change");
        let next = gtk::Button::new().ui_text("label", "Next change");
        let reader = gtk::Button::new().ui_text("label", "Reader");
        toolbar.append(&status);
        for button in [&previous, &next, &reader] {
            toolbar.append(button);
        }
        widget.append(&toolbar);
        widget.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        let left = Reader::new(2, window, documents.clone(), toast);
        let right = Reader::new(3, window, documents, toast);
        for side in [&left, &right] {
            side.make_read_only();
            side.widget.set_visible(true);
        }
        let panes = gtk::Paned::new(gtk::Orientation::Horizontal);
        panes.set_start_child(Some(&left.widget));
        panes.set_end_child(Some(&right.widget));
        panes.set_resize_start_child(true);
        panes.set_resize_end_child(true);
        // GTK chooses the initial split from the readers' requisitions, then
        // preserves the user's position across allocation and reopen. Neither
        // reader may be dragged or resized below its measured minimum.
        panes.set_shrink_start_child(false);
        panes.set_shrink_end_child(false);
        panes.set_vexpand(true);
        widget.append(&panes);
        let ui = Rc::new(Self {
            widget,
            left,
            right,
            previous,
            next,
            back: reader,
            status,
            sender,
            toast: toast.clone(),
            state: RefCell::new(State::default()),
            on_reader: RefCell::new(None),
        });
        for side in [&ui.left, &ui.right] {
            let weak = Rc::downgrade(&ui);
            *side.on_changed.borrow_mut() = Some(Box::new(move || {
                if let Some(ui) = weak.upgrade() {
                    ui.refresh();
                }
            }));
            let weak = Rc::downgrade(&ui);
            *side.on_open.borrow_mut() = Some(Box::new(move || {
                if let Some(ui) = weak.upgrade() {
                    ui.refresh();
                }
            }));
            let weak = Rc::downgrade(&ui);
            let side_weak = Rc::downgrade(side);
            *side.on_close.borrow_mut() = Some(Box::new(move || {
                if let (Some(ui), Some(side)) = (weak.upgrade(), side_weak.upgrade()) {
                    side.widget.set_visible(true);
                    ui.refresh();
                }
            }));
        }
        for (button, delta) in [(&ui.previous, -1), (&ui.next, 1)] {
            let weak = Rc::downgrade(&ui);
            button.connect_clicked(move |_| {
                if let Some(ui) = weak.upgrade() {
                    ui.navigate(delta);
                }
            });
        }
        let weak = Rc::downgrade(&ui);
        ui.back.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                if let Some(callback) = ui.on_reader.borrow().as_ref() {
                    callback();
                }
            }
        });
        ui.refresh();
        ui
    }

    pub fn open(&self, current: Option<(std::path::PathBuf, std::path::PathBuf)>) {
        if let Some((root, path)) = current {
            if self.left.location().as_ref() != Some(&(root.clone(), path.clone())) {
                self.left.open_project_path(&root, &path, None, None);
            }
        }
    }

    fn request(&self, request: Request) {
        let (generation, serial) = {
            let mut state = self.state.borrow_mut();
            state.serial += 1;
            (state.generation, state.serial)
        };
        if self
            .sender
            .try_send(Command::Workspace {
                panel: 4,
                generation,
                serial,
                request,
            })
            .is_err()
        {
            self.state.borrow_mut().navigating = false;
            i18n_ui::bind(
                &self.status,
                "label",
                "The workspace is busy; reopen comparison to retry",
            );
            let toast = adw::Toast::new("");
            i18n_ui::bind(&toast, "title", "The workspace is busy; try again");
            self.toast.add_toast(toast);
        }
    }

    fn refresh(&self) {
        let pair = self.left.snapshot().zip(self.right.snapshot());
        let mut state = self.state.borrow_mut();
        if state.pair != pair {
            state.generation += 1;
            state.pair = pair.clone();
            state.comparison = None;
            state.change = None;
            state.navigating = false;
            drop(state);
            if let Some((left, right)) = pair {
                i18n_ui::bind(&self.status, "label", "Comparing…");
                self.request(Request::Compare {
                    left: left.0,
                    right: right.0,
                });
            } else {
                i18n_ui::bind(&self.status, "label", "Open two Markdown files to compare");
            }
        } else {
            drop(state);
        }
        self.update();
    }

    fn update(&self) {
        let state = self.state.borrow();
        let source = self.left.source_mode() && self.right.source_mode();
        let ready = source && !state.navigating && self.left.is_ready() && self.right.is_ready();
        self.previous
            .set_sensitive(ready && state.change.is_some_and(|change| change > 0));
        self.next.set_sensitive(
            ready
                && state.comparison.as_ref().is_some_and(|comparison| {
                    state
                        .change
                        .is_none_or(|change| change + 1 < comparison.changes)
                        && comparison.changes > 0
                }),
        );
        for button in [&self.previous, &self.next] {
            i18n_ui::bind(
                button,
                "tooltip-text",
                if source {
                    "Jump to a changed source section"
                } else {
                    "Change navigation requires Source mode on both sides"
                },
            );
        }
    }

    fn navigate(&self, delta: i32) {
        if !(self.left.source_mode()
            && self.right.source_mode()
            && self.left.is_ready()
            && self.right.is_ready())
        {
            return;
        }
        let request = {
            let mut state = self.state.borrow_mut();
            if state.navigating {
                return;
            }
            let Some(comparison) = &state.comparison else {
                return;
            };
            let next = match state.change {
                Some(index) => (index as i64 + i64::from(delta)).max(0) as usize,
                None => 0,
            };
            if next >= comparison.changes {
                return;
            }
            let request = Request::CompareChange {
                id: comparison.id.clone(),
                index: next,
            };
            state.navigating = true;
            request
        };
        self.request(request);
        self.update();
    }

    pub fn event(self: &Rc<Self>, response: Response) {
        {
            let state = self.state.borrow();
            if response.generation != state.generation || response.serial != state.serial {
                return;
            }
        }
        self.state.borrow_mut().navigating = false;
        match response.result {
            Ok(Reply::Comparison(comparison)) => {
                let matches = self
                    .state
                    .borrow()
                    .pair
                    .as_ref()
                    .is_some_and(|(left, right)| {
                        left.1 == comparison.left_hash && right.1 == comparison.right_hash
                    });
                if !matches {
                    i18n_ui::bind(
                        &self.status,
                        "label",
                        "Document versions changed; reopen comparison",
                    );
                    return;
                }
                i18n_ui::bind_summary(
                    &self.status,
                    "label",
                    "{changes} changes · +{added} / −{removed} · {elapsed} ms",
                    &[
                        ("changes", &comparison.changes.to_string()),
                        ("added", &comparison.added.to_string()),
                        ("removed", &comparison.removed.to_string()),
                        ("elapsed", &format!("{:.0}", comparison.elapsed_ms)),
                    ],
                    if comparison.bounded_alignment {
                        &["Bounded alignment"]
                    } else {
                        &[]
                    },
                );
                self.state.borrow_mut().comparison = Some(comparison);
            }
            Ok(Reply::Change(change)) => {
                self.state.borrow_mut().change = Some(change.index);
                self.left
                    .navigate_change(change.left_line, change.left_changed);
                self.right
                    .navigate_change(change.right_line, change.right_changed);
            }
            Err(error) => {
                if response.kind == "compare" && error.contains("A comparison is still running") {
                    let generation = response.generation;
                    let weak = Rc::downgrade(self);
                    glib::timeout_add_local_once(
                        std::time::Duration::from_millis(250),
                        move || {
                            if let Some(ui) = weak.upgrade() {
                                if ui.state.borrow().generation == generation {
                                    let pair = ui.state.borrow().pair.clone();
                                    if let Some((left, right)) = pair {
                                        ui.request(Request::Compare {
                                            left: left.0,
                                            right: right.0,
                                        });
                                    }
                                }
                            }
                        },
                    );
                } else {
                    i18n_ui::literal(&self.status, "label", &error);
                    self.toast.add_toast(adw::Toast::new(&error));
                }
            }
            _ => {}
        }
        self.update();
    }

    pub fn document_event(&self, response: document::Response) {
        if self.left.accepts_key(response.key) {
            self.left.event(response);
        } else if self.right.accepts_key(response.key) {
            self.right.event(response);
        }
    }

    pub fn gateway_failed(&self, error: &str) {
        self.left.gateway_failed(error);
        self.right.gateway_failed(error);
    }

    pub fn gateway_restored(&self) {
        self.left.gateway_restored();
        self.right.gateway_restored();
    }
}
