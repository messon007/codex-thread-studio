//! Native Git status/history/staging. Mutations occur only on explicit clicks.
use crate::{
    git_review::Mode,
    i18n_ui::UiText,
    protocol::Command,
    workspace::{self, Reply, Request, Response},
    workspace_ui::{self, Source},
};
use adw::prelude::*;
use gtk::{gio, glib};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::Arc,
};
use tokio::sync::mpsc;

#[derive(Default)]
struct State {
    root: String,
    generation: u64,
    serial: u64,
    latest: HashMap<&'static str, u64>,
    files: Arc<[workspace::ChangedFile]>,
    commits: Arc<[workspace::Commit]>,
    commit: Option<String>,
    selected: Option<String>,
    has_more: bool,
    mutation: bool,
    mode: Mode,
    truncated: bool,
    diff_ready: bool,
    history_busy: bool,
}

pub struct Git {
    pub widget: gtk::Box,
    title: gtk::Label,
    status: gtk::Label,
    scope: gtk::DropDown,
    filter: gtk::SearchEntry,
    files: gio::ListStore,
    selection: gtk::SingleSelection,
    history: gio::ListStore,
    history_selection: gtk::SingleSelection,
    history_panel: gtk::Box,
    history_status: gtk::Label,
    more: gtk::Button,
    stage: gtk::Button,
    open_file: gtk::Button,
    source: Rc<Source>,
    sender: mpsc::Sender<Command>,
    toast: adw::ToastOverlay,
    state: RefCell<State>,
    updating: Cell<bool>,
    pub on_open: RefCell<Option<Box<dyn Fn(&str, &str)>>>,
    pub on_close: RefCell<Option<Box<dyn Fn()>>>,
}

impl Git {
    #[cfg(debug_assertions)]
    pub fn smoke_close(&self) {
        self.close();
    }
    #[cfg(debug_assertions)]
    pub async fn smoke_test(
        &self,
        root: &std::path::Path,
        capture: impl Fn(),
    ) -> Result<serde_json::Value, String> {
        async fn until(phase: &str, test: impl Fn() -> bool) -> Result<(), String> {
            let start = std::time::Instant::now();
            while !test() {
                if start.elapsed() > std::time::Duration::from_secs(10) {
                    return Err(format!("Native Git acceptance timed out: {phase}"));
                }
                glib::timeout_future(std::time::Duration::from_millis(50)).await;
            }
            Ok(())
        }
        self.open(&root.to_string_lossy());
        until("initial changed files", || self.files.n_items() == 2).await?;
        let position = (0..self.files.n_items())
            .find(|index| {
                self.files
                    .item(*index)
                    .and_downcast::<glib::BoxedAnyObject>()
                    .unwrap()
                    .borrow::<workspace::ChangedFile>()
                    .path
                    == "README.md"
            })
            .ok_or("Git fixture file missing")?;
        self.selection.set_selected(position);
        if self.stage.is_sensitive()
            || self.open_file.is_sensitive()
            || !self.source.visible_text().is_empty()
        {
            return Err(
                "Selecting a file left old text or actions active while loading its diff".into(),
            );
        }
        // Programmatic activation must also obey the busy guard.
        self.stage.emit_clicked();
        until("initial diff", || {
            self.source.visible_text().contains("+Modified.")
        })
        .await?;
        if self.source.smoke_diff_origin("-Original.")
            != Some(studio_markdown_native::unified_diff::Line { old: 3, new: 0 })
            || self.source.smoke_diff_origin("+Modified.")
                != Some(studio_markdown_native::unified_diff::Line { old: 0, new: 3 })
        {
            return Err("Native Git diff old/new gutter origins changed".into());
        }
        self.filter.set_text("no file should match this");
        until("hide selected file", || self.files.n_items() == 0).await?;
        if self.stage.is_sensitive()
            || self.open_file.is_sensitive()
            || !self.source.visible_text().is_empty()
        {
            return Err("A hidden Git selection retained stale diff/actions".into());
        }
        self.filter.set_text(" readme.MD ");
        until("matching file filter", || self.files.n_items() == 1).await?;
        self.selection.set_selected(0);
        until("filtered file diff", || {
            self.stage.is_sensitive() && self.source.visible_text().contains("+Modified.")
        })
        .await?;
        self.filter.set_text("");
        until("clear file filter", || {
            self.files.n_items() == 2 && self.stage.is_sensitive()
        })
        .await?;
        if !self.source.visible_text().contains("+Modified.") {
            return Err("Filtering discarded a still-visible selection".into());
        }
        self.stage.emit_clicked();
        until("stage acknowledgement", || {
            !self.state.borrow().mutation && self.files.n_items() == 1
        })
        .await?;
        self.scope.set_selected(1);
        until("staged file scope", || {
            self.files.n_items() == 1
                && self
                    .files
                    .item(0)
                    .and_downcast::<glib::BoxedAnyObject>()
                    .unwrap()
                    .borrow::<workspace::ChangedFile>()
                    .path
                    == "README.md"
        })
        .await?;
        self.selection.set_selected(0);
        until("staged diff", || {
            self.stage.is_sensitive() && self.source.visible_text().contains("+Modified.")
        })
        .await?;
        self.stage.emit_clicked();
        until("unstage acknowledgement", || {
            !self.state.borrow().mutation && self.files.n_items() == 0
        })
        .await?;
        self.scope.set_selected(2);
        until("first history page", || self.history.n_items() == 100).await?;
        self.history_selection.set_selected(0);
        until("commit files", || self.files.n_items() == 1).await?;
        self.selection.set_selected(0);
        until("commit diff", || {
            self.source.visible_text().contains("+fn main()")
        })
        .await?;
        if self.stage.is_visible() || self.open_file.is_visible() {
            return Err("History exposed working-tree actions".into());
        }
        self.filter.set_text("ADD CODE FOR HISTORY");
        until("matching commit filter", || self.history.n_items() == 1).await?;
        if self.files.n_items() != 1 || !self.source.visible_text().contains("+fn main()") {
            return Err("Commit filtering discarded the visible selected commit".into());
        }
        self.filter.set_text("no matching commit");
        until("hide selected commit", || self.history.n_items() == 0).await?;
        if self.files.n_items() != 0 || !self.source.visible_text().is_empty() {
            return Err("A filtered-out commit retained its old files or diff".into());
        }
        self.filter.set_text("");
        until("clear commit filter", || self.history.n_items() == 100).await?;
        self.history_selection.set_selected(0);
        until("restored commit files", || self.files.n_items() == 1).await?;
        self.selection.set_selected(0);
        until("restored commit diff", || {
            self.source.visible_text().contains("+fn main()")
        })
        .await?;
        let first = self.history.item(0).ok_or("First history item missing")?;
        self.more.emit_clicked();
        if self.more.is_sensitive() {
            return Err("History paging remained enabled while a page was pending".into());
        }
        // A commit switch must not invalidate the unrelated history page request.
        self.history_selection.set_selected(1);
        self.history_selection.set_selected(0);
        until("concurrent history page and commit switch", || {
            self.history.n_items() == 103
                && !self.state.borrow().history_busy
                && self.files.n_items() == 1
        })
        .await
        .map_err(|error| {
            format!(
                "{error}; loaded={}, visible={}, files={}, status={}",
                self.state.borrow().commits.len(),
                self.history.n_items(),
                self.files.n_items(),
                self.status.text()
            )
        })?;
        // Changing commits deliberately clears its file selection. Choose the
        // file again, just as in the ordinary review workflow.
        self.selection.set_selected(0);
        until("diff after concurrent history paging", || {
            self.source.visible_text().contains("+fn main()")
        })
        .await?;
        if self.history.item(0).as_ref() != Some(&first) || self.more.is_visible() {
            return Err(
                "History paging replaced stable rows or retained an exhausted page action".into(),
            );
        }
        capture();
        let generation = self.state.borrow().generation;
        self.event(Response {
            panel: 2,
            generation: generation.saturating_sub(1),
            serial: 0,
            kind: "status",
            result: Err("Stale Git response".into()),
        });
        if !self.source.visible_text().contains("+fn main()") {
            return Err("A stale Git response changed the view".into());
        }
        self.smoke_open_working_file("README.md").await?;
        Ok(
            serde_json::json!({"ok":true,"status":true,"stage":true,"unstage":true,"history":true,"commitFiles":true,"commitDiff":true,"generationIsolation":true,"oldNewGutters":true,"fileFilter":true,"commitFilter":true,"hiddenSelectionGuard":true,"loadingActionGuard":true,"historyPaging":true,"stableHistoryRows":true,"concurrentCommitPage":true}),
        )
    }

    #[cfg(debug_assertions)]
    pub async fn smoke_open_working_file(&self, path: &str) -> Result<(), String> {
        self.scope.set_selected(0);
        let start = std::time::Instant::now();
        loop {
            let position = (0..self.files.n_items()).find(|position| {
                self.files
                    .item(*position)
                    .and_downcast::<glib::BoxedAnyObject>()
                    .is_some_and(|object| object.borrow::<workspace::ChangedFile>().path == path)
            });
            if let Some(position) = position {
                self.selection.set_selected(position);
                if self.open_file.is_sensitive() {
                    self.open_file.emit_clicked();
                    return Ok(());
                }
            }
            if start.elapsed() > std::time::Duration::from_secs(10) {
                return Err("Git Open file did not become ready".into());
            }
            glib::timeout_future(std::time::Duration::from_millis(25)).await;
        }
    }

    pub fn new(sender: mpsc::Sender<Command>, toast: &adw::ToastOverlay) -> Rc<Self> {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let toolbar = crate::adaptive::toolbar(6);
        toolbar.set_margin_start(12);
        toolbar.set_margin_end(12);
        toolbar.set_margin_top(8);
        toolbar.set_margin_bottom(8);
        toolbar.add_css_class("toolbar");
        let title = gtk::Label::builder()
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::Middle)
            .width_chars(1)
            .build()
            .ui_text("label", "Git Review");
        title.add_css_class("heading");
        toolbar.append(&title);
        let scope = gtk::DropDown::from_strings(&["Unstaged", "Staged", "History"]);
        scope.set_factory(Some(&crate::i18n_ui::choice_factory()));
        scope.set_list_factory(Some(&crate::i18n_ui::choice_factory()));
        toolbar.append(&scope);
        let stage = gtk::Button::new().ui_text("label", "Stage file");
        stage.set_sensitive(false);
        toolbar.append(&stage);
        let open_file = gtk::Button::builder()
            .icon_name("document-open-symbolic")
            .sensitive(false)
            .build()
            .ui_text("tooltip-text", "Open file");
        toolbar.append(&open_file);
        let refresh = workspace_ui::button("view-refresh-symbolic", "Refresh Git")
            .ui_text("tooltip-text", "Refresh Git");
        toolbar.append(&refresh);
        let close = workspace_ui::button("window-close-symbolic", "Close Git Review")
            .ui_text("tooltip-text", "Close Git Review");
        toolbar.append(&close);
        widget.append(&toolbar);
        widget.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        let filter = gtk::SearchEntry::builder()
            .search_delay(150)
            .hexpand(true)
            .width_chars(1)
            .margin_start(12)
            .margin_end(12)
            .margin_top(8)
            .margin_bottom(8)
            .build()
            .ui_text("placeholder-text", "Filter files");
        widget.append(&filter);
        crate::i18n_ui::accessible_label(&filter, "Filter files");
        let files = gio::ListStore::new::<glib::BoxedAnyObject>();
        let selection = gtk::SingleSelection::new(Some(files.clone()));
        selection.set_autoselect(false);
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, item| {
            let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
            row.set_margin_start(12);
            row.set_margin_end(12);
            row.set_margin_top(8);
            row.set_margin_bottom(8);
            row.append(
                &gtk::Label::builder()
                    .xalign(0.0)
                    .wrap(true)
                    .width_chars(1)
                    .hexpand(true)
                    .build(),
            );
            let detail = gtk::Label::builder().xalign(0.0).build();
            detail.add_css_class("caption");
            detail.add_css_class("dim-label");
            row.append(&detail);
            item.downcast_ref::<gtk::ListItem>()
                .unwrap()
                .set_child(Some(&row));
        });
        factory.connect_bind(|_, item| {
            let item = item.downcast_ref::<gtk::ListItem>().unwrap();
            let object = item.item().and_downcast::<glib::BoxedAnyObject>().unwrap();
            let file = object.borrow::<workspace::ChangedFile>();
            crate::i18n_ui::literal(item, "accessible-label", &file.path);
            let row = item.child().unwrap();
            let label = row.first_child().and_downcast::<gtk::Label>().unwrap();
            crate::i18n_ui::literal(&label, "label", &file.path);
            let detail = if file.conflicted {
                "Conflict"
            } else if file.untracked {
                "Untracked"
            } else if !file.status.is_empty() {
                &file.status
            } else if file.staged && file.unstaged {
                "Staged + unstaged"
            } else if file.staged {
                "Staged"
            } else {
                "Unstaged"
            };
            let detail_label = label.next_sibling().and_downcast::<gtk::Label>().unwrap();
            if !file.status.is_empty() && !file.conflicted && !file.untracked {
                crate::i18n_ui::literal(&detail_label, "label", detail);
                crate::i18n_ui::literal(item, "accessible-description", detail);
            } else {
                crate::i18n_ui::bind(&detail_label, "label", detail);
                crate::i18n_ui::bind(item, "accessible-description", detail);
            }
            label.set_tooltip_text(file.previous_path.as_deref());
        });
        let list = gtk::ListView::new(Some(selection.clone()), Some(factory));
        list.set_single_click_activate(true);
        list.add_css_class("navigation-sidebar");
        let history = gio::ListStore::new::<glib::BoxedAnyObject>();
        let history_selection = gtk::SingleSelection::new(Some(history.clone()));
        history_selection.set_autoselect(false);
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, item| {
            let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
            row.set_margin_start(12);
            row.set_margin_end(12);
            row.set_margin_top(8);
            row.set_margin_bottom(8);
            row.append(
                &gtk::Label::builder()
                    .xalign(0.0)
                    .wrap(true)
                    .width_chars(1)
                    .build(),
            );
            let detail = gtk::Label::builder()
                .xalign(0.0)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .build();
            detail.add_css_class("caption");
            detail.add_css_class("dim-label");
            row.append(&detail);
            item.downcast_ref::<gtk::ListItem>()
                .unwrap()
                .set_child(Some(&row));
        });
        factory.connect_bind(|_, item| {
            let item = item.downcast_ref::<gtk::ListItem>().unwrap();
            let object = item.item().and_downcast::<glib::BoxedAnyObject>().unwrap();
            let commit = object.borrow::<workspace::Commit>();
            crate::i18n_ui::literal(
                item,
                "accessible-label",
                &format!("{} · {}", commit.short_hash, commit.subject),
            );
            crate::i18n_ui::literal(
                item,
                "accessible-description",
                &format!("{} · {}", commit.author_name, commit.authored_at),
            );
            let row = item.child().unwrap();
            let title = row.first_child().and_downcast::<gtk::Label>().unwrap();
            title.set_text(&commit.subject);
            title
                .next_sibling()
                .and_downcast::<gtk::Label>()
                .unwrap()
                .set_text(&format!(
                    "{} · {} · {}",
                    commit.short_hash,
                    commit.author_name,
                    commit.authored_at.get(..10).unwrap_or(&commit.authored_at)
                ));
        });
        let history_list = gtk::ListView::new(Some(history_selection.clone()), Some(factory));
        history_list.set_single_click_activate(true);
        history_list.add_css_class("navigation-sidebar");
        let history_panel = gtk::Box::new(gtk::Orientation::Vertical, 0);
        history_panel.set_width_request(220);
        history_panel.append(
            &gtk::ScrolledWindow::builder()
                .child(&history_list)
                .vexpand(true)
                .hscrollbar_policy(gtk::PolicyType::Never)
                .build(),
        );
        let more = gtk::Button::new().ui_text("label", "Load more commits");
        let history_status = workspace_ui::footer();
        history_panel.append(&history_status);
        more.set_visible(false);
        history_panel.append(&more);
        history_panel.set_visible(false);
        let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 0);
        sidebar.set_width_request(200);
        sidebar.append(
            &gtk::ScrolledWindow::builder()
                .child(&list)
                .vexpand(true)
                .hscrollbar_policy(gtk::PolicyType::Never)
                .build(),
        );
        let source = Source::new(sender.clone(), toast);
        source.set_close_visible(false);
        // The diff's title/paging are part of the native reader; its close control
        // closes this complete review panel through the same callback.
        let detail = gtk::Paned::new(gtk::Orientation::Horizontal);
        detail.set_start_child(Some(&sidebar));
        detail.set_end_child(Some(&source.widget));
        detail.set_position(210);
        detail.set_resize_start_child(false);
        detail.set_shrink_start_child(true);
        detail.set_shrink_end_child(true);
        detail.set_vexpand(true);
        let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        body.append(&history_panel);
        body.append(&detail);
        detail.set_hexpand(true);
        widget.append(&body);
        let status = workspace_ui::footer();
        sidebar.append(&status);
        let git = Rc::new(Self {
            widget,
            title,
            status,
            scope,
            filter,
            files,
            selection,
            history,
            history_selection,
            history_panel,
            history_status,
            more,
            stage,
            open_file,
            source,
            sender,
            toast: toast.clone(),
            state: RefCell::new(State::default()),
            updating: Cell::new(false),
            on_open: RefCell::new(None),
            on_close: RefCell::new(None),
        });
        let weak = Rc::downgrade(&git);
        git.filter.connect_search_changed(move |_| {
            if let Some(git) = weak.upgrade().filter(|git| !git.updating.get()) {
                git.project();
            }
        });
        let weak = Rc::downgrade(&git);
        git.open_file.connect_clicked(move |_| {
            if let Some(git) = weak.upgrade() {
                let state = git.state.borrow();
                if state.mode == Mode::History
                    || !state.diff_ready
                    || state.mutation
                    || !git.open_file.is_sensitive()
                {
                    return;
                }
                let Some(path) = state.selected.clone() else {
                    return;
                };
                let root = state.root.clone();
                drop(state);
                if let Some(open) = git.on_open.borrow().as_ref() {
                    open(&root, &path);
                }
            }
        });
        let weak = Rc::downgrade(&git);
        git.scope.connect_selected_notify(move |_| {
            if let Some(git) = weak.upgrade() {
                git.refresh();
            }
        });
        let weak = Rc::downgrade(&git);
        refresh.connect_clicked(move |_| {
            if let Some(git) = weak.upgrade() {
                git.refresh();
            }
        });
        let weak = Rc::downgrade(&git);
        git.more.connect_clicked(move |_| {
            if let Some(git) = weak.upgrade() {
                let mut state = git.state.borrow_mut();
                if state.history_busy || !state.has_more {
                    return;
                }
                state.history_busy = true;
                let request = Request::History {
                    root: state.root.clone(),
                    offset: state.commits.len(),
                    previous: state.commits.clone(),
                };
                drop(state);
                git.more.set_sensitive(false);
                git.request(request);
            }
        });
        let weak = Rc::downgrade(&git);
        list.connect_activate(move |_, position| {
            if let Some(git) = weak.upgrade() {
                git.selection.set_selected(position);
            }
        });
        let weak = Rc::downgrade(&git);
        history_list.connect_activate(move |_, position| {
            if let Some(git) = weak.upgrade() {
                git.history_selection.set_selected(position);
            }
        });
        let weak = Rc::downgrade(&git);
        git.selection
            .connect_selected_item_notify(move |selection| {
                if let Some(git) = weak.upgrade() {
                    if git.updating.get() {
                        return;
                    }
                    if let Some(object) = selection
                        .selected_item()
                        .and_downcast::<glib::BoxedAnyObject>()
                    {
                        let file = object.borrow::<workspace::ChangedFile>().clone();
                        git.select(&file.path);
                    } else {
                        git.clear_selection();
                    }
                }
            });
        let weak = Rc::downgrade(&git);
        git.history_selection
            .connect_selected_item_notify(move |selection| {
                if let Some(git) = weak.upgrade() {
                    if git.updating.get() {
                        return;
                    }
                    if let Some(object) = selection
                        .selected_item()
                        .and_downcast::<glib::BoxedAnyObject>()
                    {
                        let commit = object.borrow::<workspace::Commit>().hash.clone();
                        let root = {
                            let mut state = git.state.borrow_mut();
                            state.latest.remove("commit");
                            state.latest.remove("diff");
                            state.latest.remove("review-filter");
                            state.selected = None;
                            state.files = Arc::from([]);
                            state.diff_ready = false;
                            state.commit = Some(commit.clone());
                            state.root.clone()
                        };
                        git.updating.set(true);
                        git.files.remove_all();
                        git.updating.set(false);
                        git.source.clear();
                        crate::i18n_ui::bind(&git.status, "label", "Reading commit files…");
                        git.controls();
                        git.request(Request::Commit { root, commit });
                    } else {
                        git.clear_commit();
                    }
                }
            });
        let weak = Rc::downgrade(&git);
        git.stage.connect_clicked(move |_| {
            if let Some(git) = weak.upgrade() {
                let (root, path) = {
                    let state = git.state.borrow();
                    if state.mutation {
                        return;
                    }
                    if !state.diff_ready || state.mode == Mode::History || !git.stage.is_sensitive()
                    {
                        return;
                    }
                    let Some(path) = &state.selected else { return };
                    (state.root.clone(), path.clone())
                };
                git.state.borrow_mut().mutation = true;
                git.stage.set_sensitive(false);
                git.open_file.set_sensitive(false);
                git.request(Request::Stage {
                    root,
                    path,
                    stage: git.scope.selected() == 0,
                });
            }
        });
        let weak = Rc::downgrade(&git);
        close.connect_clicked(move |_| {
            if let Some(git) = weak.upgrade() {
                git.close();
            }
        });
        let weak = Rc::downgrade(&git);
        *git.source.on_close.borrow_mut() = Some(Box::new(move || {
            if let Some(git) = weak.upgrade() {
                git.close();
            }
        }));
        git
    }
    pub fn open(&self, root: &str) {
        if self.state.borrow().mutation {
            self.error("Wait for the Git operation before switching projects");
            return;
        }
        let generation = self.state.borrow().generation.wrapping_add(1);
        *self.state.borrow_mut() = State {
            root: root.into(),
            generation,
            ..State::default()
        };
        self.updating.set(true);
        self.filter.set_text("");
        self.files.remove_all();
        self.history.remove_all();
        self.updating.set(false);
        self.refresh();
    }
    fn controls(&self) {
        let state = self.state.borrow();
        let ready = state.diff_ready
            && state.selected.is_some()
            && !state.mutation
            && !state.latest.contains_key("review-filter")
            && state.mode != Mode::History;
        self.stage.set_visible(state.mode != Mode::History);
        self.scope.set_sensitive(!state.mutation);
        self.open_file.set_visible(state.mode != Mode::History);
        self.stage.set_sensitive(ready);
        self.open_file.set_sensitive(ready);
        self.more
            .set_visible(state.mode == Mode::History && state.has_more);
        self.more.set_sensitive(!state.history_busy);
    }
    fn request(&self, request: Request) {
        let kind = request.kind();
        let (serial, generation) = {
            let mut state = self.state.borrow_mut();
            state.serial = state.serial.wrapping_add(1);
            let serial = state.serial;
            state.latest.insert(kind, serial);
            (serial, state.generation)
        };
        if self
            .sender
            .try_send(Command::Workspace {
                panel: 2,
                generation,
                serial,
                request,
            })
            .is_err()
        {
            let mut state = self.state.borrow_mut();
            state.latest.remove(kind);
            if kind == "stage" {
                state.mutation = false;
            }
            if kind == "history" {
                state.history_busy = false;
            }
            drop(state);
            self.error("The workspace is busy; try again");
        }
        self.controls();
    }
    fn refresh(&self) {
        if self.state.borrow().mutation {
            return;
        }
        let mode = Mode::from_selected(self.scope.selected());
        let clear_filter = {
            let state = self.state.borrow();
            state.mode != mode && (state.mode == Mode::History || mode == Mode::History)
        };
        self.updating.set(true);
        if clear_filter {
            self.filter.set_text("");
        }
        self.files.remove_all();
        self.history.remove_all();
        self.updating.set(false);
        let root = {
            let mut state = self.state.borrow_mut();
            state.generation = state.generation.wrapping_add(1);
            state.latest.clear();
            state.mode = mode;
            state.selected = None;
            state.commit = None;
            state.diff_ready = false;
            state.files = Arc::from([]);
            state.commits = Arc::from([]);
            state.truncated = false;
            state.has_more = false;
            state.history_busy = mode == Mode::History;
            state.root.clone()
        };
        self.source.clear();
        self.controls();
        self.history_panel.set_visible(mode == Mode::History);
        crate::i18n_ui::bind(
            &self.filter,
            "placeholder-text",
            if mode == Mode::History {
                "Filter loaded commits"
            } else {
                "Filter files"
            },
        );
        crate::i18n_ui::accessible_label(
            &self.filter,
            if mode == Mode::History {
                "Filter loaded commits"
            } else {
                "Filter files"
            },
        );
        crate::i18n_ui::bind(
            &self.stage,
            "label",
            if mode == Mode::Unstaged {
                "Stage file"
            } else {
                "Unstage file"
            },
        );
        if root.is_empty() {
            return;
        }
        crate::i18n_ui::bind(&self.status, "label", "Loading Git…");
        self.request(if mode == Mode::History {
            Request::History {
                root,
                offset: 0,
                previous: Arc::from([]),
            }
        } else {
            Request::Status { root }
        });
    }
    fn clear_selection(&self) {
        let mut state = self.state.borrow_mut();
        state.selected = None;
        state.diff_ready = false;
        state.latest.remove("diff");
        drop(state);
        self.source.clear();
        self.controls();
    }
    fn clear_commit(&self) {
        let mut state = self.state.borrow_mut();
        state.commit = None;
        state.files = Arc::from([]);
        state.truncated = false;
        state.latest.remove("commit");
        drop(state);
        self.updating.set(true);
        self.files.remove_all();
        self.updating.set(false);
        self.clear_selection();
    }
    fn project(&self) {
        let state = self.state.borrow();
        if state.root.is_empty() {
            return;
        }
        let request = Request::ReviewFilter {
            files: state.files.clone(),
            commits: state.commits.clone(),
            mode: state.mode,
            query: self.filter.text().to_string(),
        };
        drop(state);
        self.request(request);
    }
    fn apply_rows(&self, rows: crate::git_review::Rows) {
        let lost_commit = {
            let state = self.state.borrow();
            state.mode == Mode::History
                && state.commit.as_ref().is_some_and(|hash| {
                    !rows
                        .commits
                        .iter()
                        .any(|index| state.commits.get(*index).is_some_and(|c| &c.hash == hash))
                })
        };
        if lost_commit {
            self.clear_commit();
        }
        let state = self.state.borrow();
        let files = rows
            .files
            .iter()
            .filter_map(|index| state.files.get(*index))
            .collect::<Vec<_>>();
        let commits = rows
            .commits
            .iter()
            .filter_map(|index| state.commits.get(*index))
            .collect::<Vec<_>>();
        let selected = state
            .selected
            .as_ref()
            .and_then(|path| files.iter().position(|file| &file.path == path));
        let commit = state
            .commit
            .as_ref()
            .and_then(|hash| commits.iter().position(|commit| &commit.hash == hash));
        self.updating.set(true);
        crate::list_store::reconcile(&self.files, &files);
        crate::list_store::reconcile(&self.history, &commits);
        self.selection
            .set_selected(selected.map_or(gtk::INVALID_LIST_POSITION, |position| position as u32));
        self.history_selection
            .set_selected(commit.map_or(gtk::INVALID_LIST_POSITION, |position| position as u32));
        self.updating.set(false);
        if state.mode == Mode::History {
            crate::i18n_ui::bind_format(
                &self.history_status,
                "label",
                "{shown} of {total} loaded commits",
                &[
                    ("shown", &commits.len().to_string()),
                    ("total", &state.commits.len().to_string()),
                ],
            );
            if state.commit.is_some() {
                crate::i18n_ui::bind_format(
                    &self.status,
                    "label",
                    if state.truncated {
                        "{count} commit files · Truncated"
                    } else {
                        "{count} commit files"
                    },
                    &[("count", &files.len().to_string())],
                );
            } else {
                crate::i18n_ui::bind(
                    &self.status,
                    "label",
                    "Select a commit to view changed files",
                );
            }
        } else {
            crate::i18n_ui::bind_format(
                &self.status,
                "label",
                if state.truncated {
                    "{shown} of {total} changed files · Truncated"
                } else {
                    "{shown} of {total} changed files"
                },
                &[
                    ("shown", &files.len().to_string()),
                    ("total", &rows.total_files.to_string()),
                ],
            );
        }
        drop(state);
        if selected.is_none() {
            self.clear_selection();
        }
        self.controls();
    }
    fn select(&self, path: &str) {
        let (root, commit, mode) = {
            let mut state = self.state.borrow_mut();
            if !state.files.iter().any(|file| file.path == path) {
                return;
            }
            state.selected = Some(path.into());
            state.diff_ready = false;
            (state.root.clone(), state.commit.clone(), state.mode)
        };
        self.source.clear();
        self.controls();
        self.request(Request::Diff {
            root,
            path: path.into(),
            scope: if mode == Mode::Unstaged {
                "unstaged"
            } else {
                "staged"
            }
            .into(),
            commit,
        });
    }
    pub fn event(&self, response: Response) {
        let state = self.state.borrow();
        if response.generation != state.generation
            || state.latest.get(response.kind) != Some(&response.serial)
        {
            return;
        }
        drop(state);
        self.state.borrow_mut().latest.remove(response.kind);
        match response.result {
            Ok(Reply::Status(status)) => {
                crate::i18n_ui::bind_format(
                    &self.title,
                    "label",
                    "Git · {branch}",
                    &[("branch", &status.branch)],
                );
                let mut state = self.state.borrow_mut();
                state.files = status.files.into();
                state.truncated = status.truncated;
                drop(state);
                self.project();
            }
            Ok(Reply::History(history)) => {
                let mut state = self.state.borrow_mut();
                state.commits = history.commits;
                state.has_more = history.has_more;
                state.history_busy = false;
                drop(state);
                self.project();
            }
            Ok(Reply::Commit(files, truncated)) => {
                let mut state = self.state.borrow_mut();
                state.files = files.into();
                state.truncated = truncated;
                drop(state);
                self.project();
            }
            Ok(Reply::ReviewRows(rows)) => self.apply_rows(rows),
            Ok(Reply::Text(text)) => {
                if self.state.borrow().selected.as_deref() == Some(text.path.as_str()) {
                    self.source.set_text(text);
                    self.state.borrow_mut().diff_ready = true;
                }
            }
            Ok(Reply::Staged) => {
                self.state.borrow_mut().mutation = false;
                self.refresh();
            }
            Err(error) => {
                let mut state = self.state.borrow_mut();
                if response.kind == "stage" {
                    state.mutation = false;
                }
                if response.kind == "history" {
                    state.history_busy = false;
                }
                drop(state);
                self.error(&error);
            }
            _ => {}
        }
        self.controls();
    }
    fn error(&self, error: &str) {
        crate::i18n_ui::literal(&self.status, "label", error);
        self.toast.add_toast(adw::Toast::new(error));
    }
    pub fn allow_window_close(&self) -> bool {
        if self.state.borrow().mutation {
            self.error("Wait for the Git operation before closing");
            false
        } else {
            true
        }
    }
    fn close(&self) {
        if let Some(callback) = self.on_close.borrow().as_ref() {
            callback();
        }
    }
}
