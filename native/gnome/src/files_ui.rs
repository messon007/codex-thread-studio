//! Lazy native Files tree. Workers own enumeration, ignore rules, filtering and watches.
use crate::{
    i18n_ui::{self, UiText},
    protocol::Command,
    workspace::{self, Reply, Request, Response},
    workspace_ui::{button, footer, header},
};
use adw::prelude::*;
use gtk::{gio, glib};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
    rc::{Rc, Weak},
};
use tokio::sync::mpsc;
#[cfg(debug_assertions)]
#[path = "files_acceptance.rs"]
mod acceptance;
#[path = "files_actions.rs"]
mod actions;
#[cfg(debug_assertions)]
#[path = "files_language_acceptance.rs"]
mod language_acceptance;
#[path = "files_rows.rs"]
mod rows;
#[path = "files_roots.rs"]
mod scopes;
#[derive(Clone, Copy)]
enum FileOperation {
    Reveal,
    CopyPath,
}
const MAX_NODES: usize = 20_000;
const MAX_EXPANDED: usize = 64;
#[derive(Clone)]
enum Data {
    Entry(workspace::Entry),
    More {
        parent: String,
        cursor: String,
    },
    Notice {
        parent: String,
        message: &'static str,
        error: Option<String>,
        retry: bool,
    },
}
struct Node {
    data: Data,
    children: gio::ListStore,
    loaded: Cell<bool>,
    loading: Cell<bool>,
    again: Cell<bool>,
    target: Cell<usize>,
    staged: RefCell<Option<workspace::Directory>>,
}
impl Node {
    fn new(data: Data) -> Rc<Self> {
        Rc::new(Self {
            data,
            children: gio::ListStore::new::<glib::BoxedAnyObject>(),
            loaded: Cell::new(false),
            loading: Cell::new(false),
            again: Cell::new(false),
            target: Cell::new(200),
            staged: RefCell::new(None),
        })
    }
    fn object(data: Data) -> glib::BoxedAnyObject {
        glib::BoxedAnyObject::new(Self::new(data))
    }
    fn folder(&self) -> Option<&str> {
        match &self.data {
            Data::Entry(e) if e.kind == "directory" => Some(&e.path),
            _ => None,
        }
    }
}
fn node(object: &glib::Object) -> Rc<Node> {
    object
        .downcast_ref::<glib::BoxedAnyObject>()
        .unwrap()
        .borrow::<Rc<Node>>()
        .clone()
}
struct Pending {
    node: Weak<Node>,
    append: bool,
}
pub struct Files {
    pub widget: gtk::Box,
    window: adw::ApplicationWindow,
    sender: mpsc::Sender<Command>,
    toast: adw::ToastOverlay,
    title: gtk::Label,
    status: gtk::Label,
    refresh: gtk::Button,
    search: gtk::SearchEntry,
    hidden: gtk::CheckButton,
    ignored: gtk::CheckButton,
    roots: gio::ListStore,
    tree: gtk::TreeListModel,
    list: gtk::ListView,
    root: RefCell<String>,
    reveal_folder: RefCell<Option<String>>,
    nodes: RefCell<HashMap<String, Weak<Node>>>,
    pending: RefCell<HashMap<u64, Pending>>,
    queue: RefCell<VecDeque<(Weak<Node>, Option<String>)>>,
    generation: Cell<u64>,
    root_epoch: Cell<u64>,
    roots_serial: Cell<u64>,
    operations: RefCell<HashMap<u64, FileOperation>>,
    context: RefCell<Option<gtk::PopoverMenu>>,
    #[cfg(debug_assertions)]
    last_location: RefCell<Option<workspace::Location>>,
    #[cfg(debug_assertions)]
    last_handoff: RefCell<Option<(String, bool)>>,
    #[cfg(debug_assertions)]
    last_handoff_result: Rc<RefCell<Option<Result<(), String>>>>,
    serial: Cell<u64>,
    scheduled: Cell<bool>,
    query: RefCell<String>,
    filter_scheduled: Cell<bool>,
    watch_generation: Cell<u64>,
    watched: RefCell<Vec<String>>,
    watch_status: RefCell<String>,
    pending_watch: RefCell<Option<Option<crate::files_watch::Subscription>>>,
    weak: Weak<Self>,
    pub on_open: RefCell<Option<Box<dyn Fn(&str, &str)>>>,
    pub on_close: RefCell<Option<Box<dyn Fn()>>>,
}
impl Files {
    fn toast_message(&self, source: &str) {
        self.toast
            .add_toast(adw::Toast::new("").ui_text("title", source));
    }
    pub fn new(
        window: &adw::ApplicationWindow,
        sender: mpsc::Sender<Command>,
        toast: &adw::ToastOverlay,
    ) -> Rc<Self> {
        let files = Rc::new_cyclic(|weak: &Weak<Self>| {
            let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let (bar, title) = header("Files");
            let choose = button("document-open-symbolic", "Choose project folder");
            let refresh = button("view-refresh-symbolic", "Refresh expanded folders");
            let close = button("window-close-symbolic", "Close files");
            let hidden = gtk::CheckButton::new().ui_text("label", "Show hidden files");
            let ignored = gtk::CheckButton::new().ui_text("label", "Show ignored files");
            let options = gtk::Box::new(gtk::Orientation::Vertical, 6);
            for side in [12] {
                options.set_margin_top(side);
                options.set_margin_bottom(side);
                options.set_margin_start(side);
                options.set_margin_end(side);
            }
            options.append(&hidden);
            options.append(&ignored);
            let reset = gtk::Button::new().ui_text("label", "Reset file tree");
            options.append(&reset);
            let weak_reset = weak.clone();
            reset.connect_clicked(move |_| {
                if let Some(files) = weak_reset.upgrade() {
                    let root = files.root.borrow().clone();
                    if !root.is_empty() {
                        files.replace_root(&root);
                    }
                }
            });
            let popover = gtk::Popover::new();
            popover.set_child(Some(&options));
            let menu = gtk::MenuButton::builder()
                .icon_name("view-more-symbolic")
                .popover(&popover)
                .build()
                .ui_text("tooltip-text", "File visibility");
            menu.add_css_class("flat");
            for child in [&choose, &refresh] {
                bar.append(child);
            }
            bar.append(&menu);
            bar.append(&close);
            widget.append(&bar);
            widget.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
            let search = gtk::SearchEntry::builder()
                .margin_start(12)
                .margin_end(12)
                .margin_top(8)
                .margin_bottom(8)
                .build()
                .ui_text("placeholder-text", "Filter expanded folders");
            i18n_ui::bind(
                &search,
                "tooltip-text",
                "Match file paths in expanded folders; expand a folder to search its contents",
            );
            widget.append(&search);
            let roots = gio::ListStore::new::<glib::BoxedAnyObject>();
            let weak_tree = weak.clone();
            let tree = gtk::TreeListModel::new(roots.clone(), false, false, move |object| {
                let current = node(object);
                current.folder()?;
                if let Some(files) = weak_tree.upgrade() {
                    if !current.loaded.get() && !current.loading.get() {
                        files.enqueue(&current, None);
                    }
                }
                Some(current.children.clone().upcast())
            });
            let selection = gtk::SingleSelection::new(Some(tree.clone()));
            selection.set_autoselect(false);
            selection.set_can_unselect(true);
            let factory = gtk::SignalListItemFactory::new();
            factory.connect_setup(|_, object| {
                let item = object.downcast_ref::<gtk::ListItem>().unwrap();
                item.set_focusable(false);
                let expander = gtk::TreeExpander::new();
                let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                line.set_margin_top(8);
                line.set_margin_bottom(8);
                line.set_margin_end(12);
                line.append(&gtk::Image::new());
                line.append(
                    &gtk::Label::builder()
                        .xalign(0.0)
                        .hexpand(true)
                        .ellipsize(gtk::pango::EllipsizeMode::Middle)
                        .build(),
                );
                let size = gtk::Label::new(None);
                size.add_css_class("caption");
                size.add_css_class("dim-label");
                line.append(&size);
                expander.set_child(Some(&line));
                item.set_child(Some(&expander));
            });
            factory.connect_bind(|_, object| {
                let item = object.downcast_ref::<gtk::ListItem>().unwrap();
                let row = item.item().and_downcast::<gtk::TreeListRow>().unwrap();
                let current = node(&row.item().unwrap());
                let expander = item.child().and_downcast::<gtk::TreeExpander>().unwrap();
                expander.set_list_row(Some(&row));
                let line = expander.child().unwrap();
                let icon = line.first_child().and_downcast::<gtk::Image>().unwrap();
                let label = icon.next_sibling().and_downcast::<gtk::Label>().unwrap();
                let size = label.next_sibling().and_downcast::<gtk::Label>().unwrap();
                rows::present(&current.data, row.depth() == 0, &icon, &label, &size);
            });
            factory.connect_unbind(|_, object| {
                if let Some(expander) = object
                    .downcast_ref::<gtk::ListItem>()
                    .and_then(|i| i.child())
                    .and_downcast::<gtk::TreeExpander>()
                {
                    expander.set_list_row(None);
                }
            });
            let list = gtk::ListView::new(Some(selection), Some(factory));
            list.set_single_click_activate(true);
            list.add_css_class("navigation-sidebar");
            list.add_css_class("studio-workspace");
            widget.append(
                &gtk::ScrolledWindow::builder()
                    .child(&list)
                    .vexpand(true)
                    .hscrollbar_policy(gtk::PolicyType::Never)
                    .build(),
            );
            let status = footer();
            widget.append(&status);
            let weak_choose = weak.clone();
            choose.connect_clicked(move |_| {
                if let Some(files) = weak_choose.upgrade() {
                    files.choose();
                }
            });
            let weak_close = weak.clone();
            close.connect_clicked(move |_| {
                if let Some(files) = weak_close.upgrade() {
                    if let Some(callback) = files.on_close.borrow().as_ref() {
                        callback();
                    }
                }
            });
            Self {
                widget,
                window: window.clone(),
                sender,
                toast: toast.clone(),
                title,
                status,
                refresh,
                search,
                hidden,
                ignored,
                roots,
                tree,
                list,
                root: RefCell::new(String::new()),
                reveal_folder: RefCell::new(None),
                nodes: RefCell::new(HashMap::new()),
                pending: RefCell::new(HashMap::new()),
                queue: RefCell::new(VecDeque::new()),
                generation: Cell::new(0),
                root_epoch: Cell::new(0),
                roots_serial: Cell::new(0),
                operations: RefCell::new(HashMap::new()),
                context: RefCell::new(None),
                #[cfg(debug_assertions)]
                last_location: RefCell::new(None),
                #[cfg(debug_assertions)]
                last_handoff: RefCell::new(None),
                #[cfg(debug_assertions)]
                last_handoff_result: Rc::new(RefCell::new(None)),
                serial: Cell::new(0),
                scheduled: Cell::new(false),
                query: RefCell::new(String::new()),
                filter_scheduled: Cell::new(false),
                watch_generation: Cell::new(0),
                watched: RefCell::new(Vec::new()),
                watch_status: RefCell::new(String::new()),
                pending_watch: RefCell::new(None),
                weak: weak.clone(),
                on_open: RefCell::new(None),
                on_close: RefCell::new(None),
            }
        });
        files.install_actions();
        let weak = Rc::downgrade(&files);
        files.list.connect_activate(move |_, position| {
            if let Some(files) = weak.upgrade() {
                files.activate(position);
            }
        });
        let weak = Rc::downgrade(&files);
        files.tree.connect_items_changed(move |_, _, _, _| {
            if let Some(files) = weak.upgrade() {
                files.schedule_sync();
            }
        });
        let weak = Rc::downgrade(&files);
        files.widget.connect_map(move |_| {
            if let Some(files) = weak.upgrade() {
                files.request_roots();
                files.schedule_sync();
            }
        });
        let weak = Rc::downgrade(&files);
        files.widget.connect_unmap(move |_| {
            if let Some(files) = weak.upgrade() {
                files.stop_watch();
                files.invalidate_reads(false);
            }
        });
        let weak = Rc::downgrade(&files);
        files.refresh.connect_clicked(move |_| {
            if let Some(files) = weak.upgrade() {
                files.request_roots();
                files.refresh_all();
                files.stop_watch();
                files.schedule_sync();
            }
        });
        let weak = Rc::downgrade(&files);
        files.search.connect_search_changed(move |entry| {
            if let Some(files) = weak.upgrade() {
                *files.query.borrow_mut() = entry.text().into();
                files.change_filter();
            }
        });
        for toggle in [&files.hidden, &files.ignored] {
            let weak = Rc::downgrade(&files);
            toggle.connect_toggled(move |_| {
                if let Some(files) = weak.upgrade() {
                    files.change_filter();
                }
            });
        }
        files
    }
    pub fn has_root(&self) -> bool {
        !self.root.borrow().is_empty()
    }
    pub fn open(&self, root: &str) {
        if self.root.borrow().as_str() == root && self.roots.n_items() > 0 {
            self.schedule_sync();
            return;
        }
        self.replace_root(root);
    }
    fn replace_root(&self, root: &str) {
        self.reveal_folder.borrow_mut().take();
        self.stop_watch();
        self.generation.set(self.generation.get() + 1);
        self.root_epoch.set(self.root_epoch.get() + 1);
        self.operations.borrow_mut().clear();
        self.pending.borrow_mut().clear();
        self.queue.borrow_mut().clear();
        self.nodes.borrow_mut().clear();
        *self.root.borrow_mut() = root.into();
        if let Some(name) = std::path::Path::new(root)
            .file_name()
            .and_then(|s| s.to_str())
        {
            i18n_ui::literal(&self.title, "label", name);
        } else {
            i18n_ui::bind(&self.title, "label", "Files");
        }
        i18n_ui::literal(&self.title, "tooltip-text", root);
        let current = Node::new(Data::Entry(workspace::Entry {
            name: "Project".into(),
            path: String::new(),
            kind: "directory".into(),
            size: None,
            read_only: false,
        }));
        self.nodes
            .borrow_mut()
            .insert(String::new(), Rc::downgrade(&current));
        self.roots.splice(
            0,
            self.roots.n_items(),
            &[glib::BoxedAnyObject::new(current)],
        );
        if let Some(row) = self.tree.row(0) {
            row.set_expanded(true);
        }
        self.request_roots();
    }
    pub fn choose(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        gtk::FileDialog::builder()
            .build()
            .ui_text("title", "Choose project folder")
            .select_folder(Some(&self.window), gio::Cancellable::NONE, move |result| {
                if let (Some(files), Ok(folder)) = (weak.upgrade(), result) {
                    if let Some(path) = folder.path() {
                        files.open(&path.to_string_lossy());
                    }
                }
            });
    }
    fn activate(&self, position: u32) {
        let Some(row) = self.tree.row(position) else {
            return;
        };
        let current = node(&row.item().unwrap());
        match &current.data {
            Data::Entry(e) if e.kind == "directory" => row.set_expanded(!row.is_expanded()),
            Data::Entry(e) => {
                if let Some(callback) = self.on_open.borrow().as_ref() {
                    callback(&self.root.borrow(), &e.path);
                }
            }
            Data::More { parent, cursor } => {
                let parent = self.nodes.borrow().get(parent).and_then(Weak::upgrade);
                if let Some(parent) = parent {
                    let reserved: usize = self
                        .nodes
                        .borrow()
                        .values()
                        .filter_map(Weak::upgrade)
                        .filter(|n| n.folder().is_some() && (n.loaded.get() || n.loading.get()))
                        .map(|n| n.target.get())
                        .sum();
                    if reserved + 200 > MAX_NODES {
                        self.toast_message("Loaded folder budget is full; use Reset file tree");
                        return;
                    }
                    parent.target.set((parent.target.get() + 200).min(10_000));
                    if !parent.loading.get() {
                        self.enqueue(&parent, Some(cursor.clone()));
                    }
                }
            }
            Data::Notice {
                parent,
                retry: true,
                ..
            } => {
                let parent = self.nodes.borrow().get(parent).and_then(Weak::upgrade);
                if let Some(parent) = parent {
                    self.enqueue(&parent, None);
                }
            }
            _ => {}
        }
    }
    fn enqueue(&self, current: &Rc<Node>, cursor: Option<String>) {
        if current.loading.get() {
            if cursor.is_none() {
                current.again.set(true);
            }
            return;
        }
        if self.queue.borrow().len() >= 128 {
            self.toast_message("Too many folder requests; try Refresh");
            return;
        }
        let expanded = self.expanded();
        if !current.loaded.get()
            && expanded.len() >= MAX_EXPANDED
            && !expanded
                .iter()
                .any(|p| Some(p.as_str()) == current.folder())
        {
            current.children.splice(
                0,
                current.children.n_items(),
                &[Node::object(Data::Notice {
                    parent: current.folder().unwrap().into(),
                    message: "Close some folders before expanding more (maximum 64)",
                    error: None,
                    retry: true,
                })],
            );
            return;
        }
        current.loading.set(true);
        if cursor.is_none() {
            current.staged.borrow_mut().take();
        }
        self.queue
            .borrow_mut()
            .push_back((Rc::downgrade(current), cursor));
        self.pump();
    }
    fn pump(&self) {
        while self.pending.borrow().len() < 2 {
            let Some((weak, cursor)) = self.queue.borrow_mut().pop_front() else {
                break;
            };
            let Some(current) = weak.upgrade() else {
                continue;
            };
            let path = current.folder().unwrap().to_owned();
            self.serial.set(self.serial.get() + 1);
            let serial = self.serial.get();
            let request = Request::List {
                root: self.root.borrow().clone(),
                path,
                query: self.query.borrow().clone(),
                show_hidden: self.hidden.is_active(),
                show_ignored: self.ignored.is_active(),
                cursor: cursor.clone(),
            };
            if self
                .sender
                .try_send(Command::Workspace {
                    panel: 1,
                    generation: self.generation.get(),
                    serial,
                    request,
                })
                .is_err()
            {
                current.loading.set(false);
                i18n_ui::bind(&self.status, "label", "The workspace is busy; use Refresh");
                continue;
            }
            self.pending.borrow_mut().insert(
                serial,
                Pending {
                    node: weak,
                    append: cursor.is_some(),
                },
            );
        }
    }
    pub fn event(&self, response: Response) {
        if response.kind == "roots" {
            self.roots_event(response);
            return;
        }
        if response.kind == "location" {
            self.location_event(response);
            return;
        }
        if response.generation != self.generation.get() {
            return;
        }
        let Some(pending) = self.pending.borrow_mut().remove(&response.serial) else {
            return;
        };
        if let Some(current) = pending.node.upgrade() {
            current.loading.set(false);
            if !self.active_scope(current.folder().unwrap_or_default()) {
                self.pump();
                return;
            }
            match response.result {
                Ok(Reply::Directory(mut directory))
                    if current.folder() == Some(directory.path.as_str()) =>
                {
                    if current.again.replace(false) {
                        current.staged.borrow_mut().take();
                        self.enqueue(&current, None);
                    } else {
                        if pending.append {
                            let mut entries = current
                                .staged
                                .borrow_mut()
                                .take()
                                .map(|d| d.entries)
                                .unwrap_or_else(|| {
                                    (0..current.children.n_items())
                                        .filter_map(|i| {
                                            let item = current.children.item(i)?;
                                            let child = node(&item);
                                            match &child.data {
                                                Data::Entry(e) => Some(e.clone()),
                                                _ => None,
                                            }
                                        })
                                        .collect()
                                });
                            entries.append(&mut directory.entries);
                            directory.entries = entries;
                        }
                        if directory.entries.len() < current.target.get()
                            && directory.next_cursor.is_some()
                        {
                            let cursor = directory.next_cursor.clone();
                            *current.staged.borrow_mut() = Some(directory);
                            self.enqueue(&current, cursor);
                        } else {
                            current.staged.borrow_mut().take();
                            self.fill(&current, directory);
                            current.loaded.set(true);
                        }
                    }
                }
                Err(error) => {
                    current.staged.borrow_mut().take();
                    if pending.append {
                        self.toast.add_toast(adw::Toast::new(&error));
                        self.enqueue(&current, None);
                    } else if current.children.n_items() == 0 {
                        current.children.append(&Node::object(Data::Notice {
                            parent: current.folder().unwrap().into(),
                            message: "{error} · Retry",
                            error: Some(error.clone()),
                            retry: true,
                        }));
                    }
                    i18n_ui::literal(&self.status, "label", &error);
                }
                _ => {}
            }
        }
        self.pump();
        self.schedule_sync();
    }
    fn fill(&self, current: &Rc<Node>, directory: workspace::Directory) {
        let old = (0..current.children.n_items())
            .filter_map(|i| {
                current
                    .children
                    .item(i)
                    .and_downcast::<glib::BoxedAnyObject>()
            })
            .collect::<Vec<_>>();
        let reusable = old
            .iter()
            .filter_map(|object| match &node(object.upcast_ref()).data {
                Data::Entry(e) => Some((e.path.clone(), object.clone())),
                _ => None,
            })
            .collect::<HashMap<_, _>>();
        let mut objects = Vec::new();
        let mut nodes = self.nodes.borrow_mut();
        nodes.retain(|_, weak| weak.strong_count() > 0);
        let mut capped = false;
        for entry in directory.entries {
            let reused = reusable
                .get(&entry.path)
                .filter(
                    |object| matches!(&node(object.upcast_ref()).data,Data::Entry(e)if *e==entry),
                )
                .cloned();
            if let Some(object) = reused {
                objects.push(object);
                continue;
            }
            if nodes.len() >= MAX_NODES && !nodes.contains_key(&entry.path) {
                capped = true;
                break;
            }
            let path = entry.path.clone();
            let child = Node::new(Data::Entry(entry));
            nodes.insert(path, Rc::downgrade(&child));
            objects.push(glib::BoxedAnyObject::new(child));
        }
        drop(nodes);
        let parent = current.folder().unwrap().to_owned();
        if !capped {
            if let Some(cursor) = directory.next_cursor {
                objects.push(Node::object(Data::More {
                    parent: parent.clone(),
                    cursor,
                }));
            }
        }
        if directory.scan_truncated || capped {
            objects.push(Node::object(Data::Notice {
                parent,
                message: if capped {
                    "Tree cache is full; use Reset file tree in File visibility"
                } else {
                    "Directory scan reached its entry, memory or time limit"
                },
                error: None,
                retry: false,
            }));
        }
        if objects.is_empty() {
            objects.push(Node::object(Data::Notice {
                parent: current.folder().unwrap().into(),
                message: if self.query.borrow().is_empty() {
                    "Empty folder"
                } else {
                    "No matching files"
                },
                error: None,
                retry: false,
            }));
        }
        // Keep unchanged row objects (and their expanded descendants) across live refreshes.
        let prefix = old.iter().zip(&objects).take_while(|(a, b)| a == b).count();
        let suffix = old[prefix..]
            .iter()
            .rev()
            .zip(objects[prefix..].iter().rev())
            .take_while(|(a, b)| a == b)
            .count();
        let expanded = self
            .expanded()
            .into_iter()
            .collect::<std::collections::HashSet<_>>();
        current.children.splice(
            prefix as u32,
            (old.len() - prefix - suffix) as u32,
            &objects[prefix..objects.len() - suffix],
        );
        let mut index = 0;
        while index < self.tree.n_items() {
            if let Some(row) = self.tree.row(index) {
                if let Some(path) = node(&row.item().unwrap()).folder() {
                    if expanded.contains(path) && !row.is_expanded() {
                        row.set_expanded(true);
                    }
                }
            }
            index += 1;
        }
        i18n_ui::bind_format(
            &self.status,
            "label",
            "{count} entries in folder · {elapsed} ms",
            &[
                ("count", &directory.total.to_string()),
                ("elapsed", &directory.elapsed_ms.to_string()),
            ],
        );
    }
    fn expanded(&self) -> Vec<String> {
        let mut paths = Vec::new();
        for i in 0..self.tree.n_items() {
            if let Some(row) = self.tree.row(i) {
                if row.is_expanded() {
                    if let Some(path) = node(&row.item().unwrap()).folder() {
                        paths.push(path.to_owned());
                    }
                }
            }
        }
        paths.sort();
        paths.dedup();
        paths
    }
    fn refresh_all(&self) {
        if !self.has_root() {
            return;
        }
        for path in self.expanded() {
            let node = self.nodes.borrow().get(&path).and_then(Weak::upgrade);
            if let Some(node) = node {
                self.enqueue(&node, None);
            }
        }
    }
    fn change_filter(&self) {
        self.invalidate_reads(true);
        if !self.filter_scheduled.replace(true) {
            let weak = self.weak.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(150), move || {
                if let Some(files) = weak.upgrade() {
                    files.filter_scheduled.set(false);
                    if files.widget.is_mapped() {
                        files.refresh_all();
                    }
                }
            });
        }
    }
    fn invalidate_reads(&self, reset_pages: bool) {
        self.generation.set(self.generation.get() + 1);
        self.operations.borrow_mut().clear();
        self.pending.borrow_mut().clear();
        self.queue.borrow_mut().clear();
        for weak in self.nodes.borrow().values() {
            if let Some(node) = weak.upgrade() {
                node.loading.set(false);
                node.again.set(false);
                if reset_pages {
                    node.target.set(200);
                }
                node.staged.borrow_mut().take();
            }
        }
    }
    fn schedule_sync(&self) {
        if self.scheduled.replace(true) {
            return;
        }
        let weak = self.weak.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(100), move || {
            if let Some(files) = weak.upgrade() {
                files.scheduled.set(false);
                files.reveal_pending_folder();
                files.flush_watch();
                files.sync_watch();
            }
        });
    }
    fn stop_watch(&self) {
        self.close_context();
        self.watch_generation.set(self.watch_generation.get() + 1);
        self.watched.borrow_mut().clear();
        self.watch_status.borrow_mut().clear();
        *self.pending_watch.borrow_mut() = Some(None);
        self.flush_watch();
    }
    fn flush_watch(&self) {
        let desired = self.pending_watch.borrow_mut().take();
        if let Some(desired) = desired {
            match self.sender.try_send(Command::FilesWatch(desired)) {
                Ok(()) => {}
                Err(mpsc::error::TrySendError::Full(Command::FilesWatch(desired))) => {
                    *self.pending_watch.borrow_mut() = Some(desired);
                    self.schedule_sync();
                }
                Err(_) => i18n_ui::bind(&self.status, "label", "Files gateway is unavailable"),
            }
        }
    }
    fn sync_watch(&self) {
        if !self.widget.is_mapped() || !self.has_root() {
            return;
        }
        let mut paths = self.expanded();
        paths.truncate(MAX_EXPANDED);
        if paths.is_empty() {
            if !self.watched.borrow().is_empty() {
                self.stop_watch();
            }
            return;
        }
        if *self.watched.borrow() == paths {
            return;
        }
        self.watch_generation.set(self.watch_generation.get() + 1);
        let subscription = crate::files_watch::Subscription {
            generation: self.watch_generation.get(),
            root: self.root.borrow().clone(),
            paths: paths.clone(),
        };
        *self.watched.borrow_mut() = paths;
        *self.watch_status.borrow_mut() = "Connecting live updates…".into();
        *self.pending_watch.borrow_mut() = Some(Some(subscription));
        self.flush_watch();
    }
    pub fn watch_event(&self, generation: u64, update: crate::files_watch::Update) {
        if generation != self.watch_generation.get() || !self.widget.is_mapped() {
            return;
        }
        match update {
            crate::files_watch::Update::Ready { paths } => {
                if paths == *self.watched.borrow() {
                    *self.watch_status.borrow_mut() = "Live updates".into();
                    i18n_ui::bind(&self.status, "label", "Live updates");
                    self.refresh_all();
                }
            }
            crate::files_watch::Update::Changed { paths } => {
                if paths.iter().any(|p| self.watched.borrow().contains(p)) {
                    self.refresh_all();
                }
            }
            crate::files_watch::Update::Error { message } => {
                self.request_roots();
                *self.watch_status.borrow_mut() = message.clone();
                i18n_ui::literal(&self.status, "label", &message);
            }
        }
    }
}
