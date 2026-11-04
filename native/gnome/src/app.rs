use crate::i18n_ui::UiText;
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use adw::prelude::*;
use gtk::{gio, glib};
use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::{
    model::{self, Backend, Item, Thread},
    protocol::{self, Command, Event},
};
#[cfg(debug_assertions)]
#[path = "activity_details_acceptance.rs"]
mod activity_details_acceptance;
#[path = "agents_controller.rs"]
mod agent_commands;
#[cfg(debug_assertions)]
#[path = "annotation_location_acceptance.rs"]
mod annotation_location_acceptance;
#[path = "approval_controller.rs"]
mod approval_controller;
#[path = "attachment_controller.rs"]
mod attachment_controller;
#[path = "backend_recovery_controller.rs"]
mod backend_recovery;
#[cfg(debug_assertions)]
#[path = "backend_recovery_acceptance.rs"]
mod backend_recovery_acceptance;
#[cfg(debug_assertions)]
#[path = "backend_settings_acceptance.rs"]
mod backend_settings_acceptance;
#[cfg(debug_assertions)]
#[path = "browser_settings_acceptance.rs"]
mod browser_settings_acceptance;
#[cfg(debug_assertions)]
#[path = "chat_comment_acceptance.rs"]
mod chat_comment_acceptance;
#[path = "command_controller.rs"]
mod commands;
#[cfg(debug_assertions)]
#[path = "connections_acceptance.rs"]
mod connections_acceptance;
#[path = "connections_controller.rs"]
mod connections_controller;
#[path = "continuation_controller.rs"]
mod continuation_controller;
#[cfg(debug_assertions)]
#[path = "conversation_pages_acceptance.rs"]
mod conversation_pages_acceptance;
#[path = "conversation_paging.rs"]
mod conversation_paging;
#[cfg(debug_assertions)]
#[path = "embedded_image_acceptance.rs"]
mod embedded_image_acceptance;
#[cfg(debug_assertions)]
#[path = "fallback_acceptance.rs"]
mod fallback_acceptance;
#[cfg(debug_assertions)]
#[path = "file_portal_acceptance.rs"]
mod file_portal_acceptance;
#[cfg(debug_assertions)]
#[path = "gateway_stop_acceptance.rs"]
mod gateway_stop_acceptance;
#[cfg(debug_assertions)]
#[path = "history_images_acceptance.rs"]
mod history_images_acceptance;
#[cfg(debug_assertions)]
#[path = "html_action_workflow_acceptance.rs"]
mod html_action_acceptance;
#[cfg(debug_assertions)]
#[path = "hydration_acceptance.rs"]
mod hydration_acceptance;
#[cfg(debug_assertions)]
#[path = "inline_math_acceptance.rs"]
mod inline_math_acceptance;
#[cfg(debug_assertions)]
#[path = "interaction_acceptance.rs"]
mod interaction_acceptance;
#[path = "interaction_controller.rs"]
mod interactions;
#[cfg(debug_assertions)]
#[path = "language_acceptance.rs"]
mod language_acceptance;
#[cfg(debug_assertions)]
#[path = "layout_acceptance.rs"]
mod layout_acceptance;
#[path = "mcp_controller.rs"]
mod mcp_commands;
#[cfg(debug_assertions)]
#[path = "workspace_navigation_acceptance.rs"]
mod navigation_acceptance;
#[cfg(debug_assertions)]
#[path = "notification_acceptance.rs"]
mod notification_acceptance;
#[cfg(debug_assertions)]
#[path = "opencode_input_acceptance.rs"]
mod opencode_input_acceptance;
#[cfg(debug_assertions)]
#[path = "output_acceptance.rs"]
mod output_acceptance;
#[cfg(debug_assertions)]
#[path = "position_acceptance.rs"]
mod position_acceptance;
#[path = "queue_controller.rs"]
mod queues;
#[path = "composer_refs_controller.rs"]
mod refs;
#[cfg(debug_assertions)]
#[path = "remote_image_acceptance.rs"]
mod remote_image_acceptance;
#[cfg(debug_assertions)]
#[path = "session_resources_acceptance.rs"]
mod resource_acceptance;
#[cfg(debug_assertions)]
#[path = "rich_text_acceptance.rs"]
mod rich_text_acceptance;
#[path = "router_controller.rs"]
mod router_chat;
#[cfg(debug_assertions)]
#[path = "router_map_acceptance.rs"]
mod router_map_acceptance;
#[path = "router_config_controller.rs"]
mod router_settings;
#[cfg(debug_assertions)]
#[path = "session_info_acceptance.rs"]
mod session_info_acceptance;
#[path = "session_info_controller.rs"]
mod session_info_controller;
#[cfg(debug_assertions)]
#[path = "session_map_workflow_acceptance.rs"]
mod session_map_acceptance;
#[path = "session_map_controller.rs"]
mod session_map_controller;
#[cfg(debug_assertions)]
#[path = "session_map_sync_acceptance.rs"]
mod session_map_sync_acceptance;
#[path = "session_resources_controller.rs"]
mod session_resources_controller;
#[cfg(debug_assertions)]
#[path = "source_comment_workflow_acceptance.rs"]
mod source_comment_workflow_acceptance;
#[cfg(debug_assertions)]
#[path = "startup_acceptance.rs"]
mod startup_acceptance;
#[cfg(debug_assertions)]
#[path = "streaming_acceptance.rs"]
mod streaming_acceptance;
#[path = "streaming_controller.rs"]
mod streaming_controller;
#[cfg(debug_assertions)]
#[path = "supervision_acceptance.rs"]
mod supervision_acceptance;
#[path = "supervision_controller.rs"]
mod supervision_controller;
#[cfg(debug_assertions)]
#[path = "table_cell_acceptance.rs"]
mod table_cell_acceptance;
#[cfg(debug_assertions)]
#[path = "table_image_acceptance.rs"]
mod table_image_acceptance;
#[cfg(debug_assertions)]
#[path = "translation_workflow_acceptance.rs"]
mod translation_acceptance;

#[derive(Clone)]
enum Pending {
    Agents {
        dialog: u64,
        revision: u64,
        step: crate::agents::Step,
    },
    Mcp {
        dialog: u64,
        revision: u64,
    },
    Operation {
        thread: String,
        name: &'static str,
        original: Option<String>,
    },
    Tools(u64),
    Queue {
        key: String,
        thread: String,
        id: String,
    },
    Catalog {
        append: bool,
        generation: u64,
    },
    SessionMutation {
        thread: String,
        kind: &'static str,
        name: String,
    },
    Models,
    Select(String, u64),
    Create,
    Send {
        information: Option<crate::session_info_ui::Ticket>,
        annotation: Option<Arc<crate::annotations::Submission>>,
        thread: String,
        text: String,
        images: Vec<String>,
        references: Vec<String>,
    },
    Stop(String),
}

struct PendingRequest {
    backend: String,
    generation: u64,
    action: Pending,
}
impl Pending {
    fn thread(&self) -> Option<&str> {
        match self {
            Self::Select(thread, _) | Self::Stop(thread) => Some(thread),
            Self::Operation { thread, .. } => Some(thread),
            Self::Queue { thread, .. } => Some(thread),
            Self::Send { thread, .. } | Self::SessionMutation { thread, .. } => Some(thread),
            _ => None,
        }
    }
}

#[derive(Default)]
struct State {
    startup_generation: u64,
    startup_ready: bool,
    command_data: crate::command_data::Book,
    policies: crate::command_tools::Policies,
    shells: crate::command_tools::Shells,
    references: crate::composer_refs::Book,
    queue_references: HashMap<(String, u64), Vec<String>>,
    images: crate::attachments::Book,
    queues: crate::message_queue::Book,
    cache: crate::conversation_cache::Cache,
    last_selected: HashMap<String, Thread>,
    options: crate::turn_options::Options,
    options_request: u64,
    options_loaded: bool,
    updating_options: bool,
    fast_models: HashSet<String>,
    default_model: String,
    session_model: String,
    session_tier: String,
    model_efforts: HashMap<String, Vec<String>>,
    efforts: Vec<String>,
    activity: HashMap<String, crate::activity::Activity>,
    backend_generations: HashMap<String, u64>,
    backend_ready: HashSet<String>,
    connection_sequence: u64,
    history_generation: u64,
    history_changes: crate::history_state::Changes,
    history_verified: bool,
    visibility: Option<Arc<crate::session_directory_policy::Filter>>,
    visibility_revision: String,
    session_rows_serial: u64,
    catalog_generation: u64,
    catalog_loading: bool,
    catalog_truncated: bool,
    archived: bool,
    pinned_only: bool,
    session_mutation: bool,
    pins: Vec<String>,
    pins_loaded: bool,
    pin_pending: bool,
    saved_backend: String,
    saved_session: String,
    backends: Vec<Backend>,
    backend: String,
    socket_generation: u64,
    restore_selection: Option<Thread>,
    reconnecting: Option<String>,
    reference_selection: Option<String>,
    ready: bool,
    threads: Vec<Thread>,
    selected: Option<Thread>,
    cursor: Option<String>,
    items: Vec<Arc<Item>>,
    active_turn: Option<String>,
    models: Vec<String>,
    gateway_version: String,
    pending: HashMap<u64, PendingRequest>,
    sequence: u64,
    sending: bool,
    loading: bool,
    drafts: HashMap<String, String>,
    render_dirty: bool,
}

struct Ui {
    recovery: RefCell<backend_recovery::Recovery>,
    gateway_stopped: Cell<bool>,
    streaming: RefCell<streaming_controller::Book>,
    #[cfg(debug_assertions)]
    event_max_ms: std::cell::Cell<u128>,
    #[cfg(debug_assertions)]
    render_max_ms: std::cell::Cell<u128>,
    appearance: Rc<crate::appearance_ui::Runtime>,
    translation: Rc<crate::translation_ui::Runtime>,
    notifications: Rc<crate::notifications_ui::Runtime>,
    language: Rc<crate::language_ui::Runtime>,
    message_flow: Rc<crate::message_flow_ui::Runtime>,
    continuation: RefCell<Option<continuation_controller::Flight>>,
    composer_revision: Cell<u64>,
    continue_button: gtk::Button,
    menu: gio::Menu,
    file_picker: crate::file_picker::Pending,
    router: RefCell<router_chat::Book>,
    chat_visibility: Rc<crate::chat_visibility::Visibility>,
    router_bar: Rc<crate::router_ui::Bar>,
    router_back: gtk::Button,
    router_target: gtk::Button,
    router_target_label: gtk::Label,
    router_file_button: gtk::Button,
    router_picker: RefCell<Option<Rc<crate::router_picker_ui::Picker>>>,
    router_settings: RefCell<Option<Rc<crate::router_config_ui::Panel>>>,
    agents: RefCell<Option<Rc<crate::agents_ui::Panel>>>,
    session_map: Rc<crate::session_map_ui::Panel>,
    resources: Rc<crate::session_resources_ui::Panel>,
    session_info: RefCell<Option<Rc<crate::session_info_ui::Panel>>>,
    connections: RefCell<Option<Rc<crate::connections_ui::Panel>>>,
    mcp: RefCell<Option<Rc<crate::mcp_ui::Panel>>>,
    palette: RefCell<Option<Rc<crate::command_palette::Palette>>>,
    interactions: RefCell<crate::interaction_ui::Book>,
    approvals: RefCell<crate::interaction_ui::Approvals>,
    permission_dialog: RefCell<Option<adw::Dialog>>,
    status_dialog: RefCell<Option<adw::Dialog>>,
    shell_hint: gtk::Label,
    composer_tools: Rc<crate::composer_tools_ui::Tools>,
    image_button: gtk::Button,
    image_chips: gtk::FlowBox,
    image_stamp: RefCell<(String, u64)>,
    queue: Rc<crate::queue_ui::Panel>,
    queue_button: gtk::Button,
    supervision: RefCell<crate::supervision_state::Book>,
    supervision_menu: Rc<crate::supervision_ui::Menu>,
    supervision_status: gtk::Label,
    supervision_options: RefCell<HashMap<u64, supervision_controller::OptionsRead>>,
    favorites: RefCell<Option<Rc<crate::favorites_ui::Favorites>>>,
    annotations: RefCell<Option<Rc<crate::annotations_ui::Panel>>>,
    annotation_prepared: RefCell<Option<Arc<crate::annotations::Submission>>>,
    history_context: RefCell<Option<Rc<crate::conversation_context_ui::View>>>,
    output: Rc<crate::conversation_output_ui::Host>,
    write_feedback: Rc<crate::write_feedback::Queue>,
    document_sender: mpsc::Sender<crate::document::Message>,
    finder: Rc<crate::session_search_ui::Search>,
    chat_diagrams: Rc<crate::chat_diagrams::Renderer>,
    inline_math: Rc<crate::inline_math::Renderer>,
    settings: RefCell<Option<Rc<crate::settings_ui::Settings>>>,
    fast: gtk::ToggleButton,
    session_list: gtk::ListView,
    pin: gtk::ToggleButton,
    scope: gtk::DropDown,
    window: adw::ApplicationWindow,
    split: adw::OverlaySplitView,
    title: adw::WindowTitle,
    status: gtk::Label,
    toast: adw::ToastOverlay,
    backend: gtk::DropDown,
    sessions: gio::ListStore,
    selection: gtk::SingleSelection,
    search: gtk::SearchEntry,
    more: gtk::Button,
    transcript: gio::ListStore,
    transcript_scroll: gtk::ScrolledWindow,
    transcript_view: gtk::ListView,
    transcript_navigation: Rc<crate::list_navigation::Navigation>,
    turn_navigation: Rc<crate::turn_navigation_ui::Panel>,
    history_pages: Rc<crate::conversation_pages_ui::Pager>,
    composer: gtk::TextView,
    send: gtk::Button,
    stop: gtk::Button,
    model: gtk::DropDown,
    effort: gtk::DropDown,
    sender: mpsc::Sender<Command>,
    launch_control: tokio::sync::watch::Sender<crate::gateway_launch::Control>,
    session_rows_sender: tokio::sync::watch::Sender<Option<crate::session_list::Request>>,
    reader: Rc<crate::reader::Reader>,
    tables: Rc<crate::tables_ui::Tables>,
    images: Rc<crate::images_ui::Renderer>,
    image_viewer: Rc<crate::images_ui::Viewer>,
    pdf: Rc<crate::pdf_ui::Pdf>,
    epub: Rc<crate::epub_ui::Epub>,
    browser: Rc<crate::browser_ui::Browser>,
    workspace: gtk::Paned,
    conversation: adw::ToolbarView,
    tools: gtk::Stack,
    tool_navigation: RefCell<crate::workspace_navigation::Navigation>,
    terminal: Rc<crate::terminal::Terminal>,
    files: Rc<crate::workspace_ui::Files>,
    source: Rc<crate::workspace_ui::Source>,
    git: Rc<crate::git_ui::Git>,
    compare: Rc<crate::compare_ui::Compare>,
    state: RefCell<State>,
}

pub fn run() -> glib::ExitCode {
    let runtime = Rc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .max_blocking_threads(2)
            .enable_all()
            .build()
            .expect("native asynchronous runtime"),
    );
    let id = if cfg!(debug_assertions) && std::env::var_os("STUDIO_GNOME_SMOKE_DIR").is_some() {
        let private = std::env::var_os("STUDIO_GNOME_SMOKE_DIR")
            .map(std::path::PathBuf::from)
            .is_some_and(|root| {
                std::env::var_os("CODEX_THREAD_STUDIO_CODEX_BIN").map(std::path::PathBuf::from)
                    == Some(root.join("fake-codex.py"))
                    && std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from)
                        == Some(root.join("config"))
            });
        std::env::var("STUDIO_GNOME_SMOKE_APPLICATION_ID")
            .ok()
            .filter(|id| {
                private
                    && id.starts_with("io.github.messon007.codexthreadstudio.Gnome.Smoke")
                    && id.len() < 180
                    && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.')
            })
            .unwrap_or_else(|| {
                format!(
                    "io.github.messon007.codexthreadstudio.Gnome.Smoke{}",
                    std::process::id()
                )
            })
    } else {
        "io.github.messon007.codexthreadstudio.Gnome".into()
    };
    let app = adw::Application::builder()
        .application_id(&id)
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();
    let controller = Rc::new(RefCell::new(std::rc::Weak::<Ui>::new()));
    let activated_controller = controller.clone();
    let action = gio::SimpleAction::new("show-notification", Some(&String::static_variant_type()));
    let weak_app = app.downgrade();
    let target = controller.clone();
    action.connect_activate(move |_, parameter| {
        let Some(id) = parameter
            .and_then(|v| v.get::<String>())
            .filter(|s| crate::notification_ids::valid(s))
        else {
            return;
        };
        if let Some(app) = weak_app.upgrade() {
            app.activate();
            if let Some(ui) = target.borrow().upgrade() {
                ui.notifications.activated(&id);
            }
        }
    });
    app.add_action(&action);
    let runtime_for_app = runtime.clone();
    app.connect_activate(move |app| {
        if let Some(window) = app.active_window() {
            window.present();
            return;
        }
        let (sender, documents, events, session_rows_sender, launch_control, turn_rows) =
            protocol::start(&runtime_for_app);
        let ui = build(
            app,
            sender,
            documents,
            session_rows_sender,
            launch_control,
            turn_rows,
        );
        *activated_controller.borrow_mut() = Rc::downgrade(&ui);
        connect(&ui);
        let weak = Rc::downgrade(&ui);
        glib::MainContext::default().spawn_local(async move {
            while let Ok(event) = events.recv().await {
                let Some(ui) = weak.upgrade() else {
                    break;
                };
                #[cfg(debug_assertions)]
                let started = std::time::Instant::now();
                ui.event(event);
                #[cfg(debug_assertions)]
                ui.event_max_ms
                    .set(ui.event_max_ms.get().max(started.elapsed().as_millis()));
            }
        });
        let weak = Rc::downgrade(&ui);
        glib::timeout_add_local(Duration::from_millis(33), move || {
            let Some(ui) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            ui.router_tick();
            ui.streaming_tick();
            if ui.state.borrow().render_dirty || ui.history_pages.needs_render() {
                #[cfg(debug_assertions)]
                let started = std::time::Instant::now();
                ui.render_items();
                #[cfg(debug_assertions)]
                ui.render_max_ms
                    .set(ui.render_max_ms.get().max(started.elapsed().as_millis()));
            }
            if ui.state.borrow().queues.dirty {
                ui.pump_queues();
            }
            glib::ControlFlow::Continue
        });
        // Own the controller for exactly the window's lifetime.
        let owner = Rc::new(RefCell::new(Some(ui.clone())));
        let owner_close = owner.clone();
        ui.window.connect_destroy(move |_| {
            owner_close.borrow_mut().take();
        });
        ui.window.present();
        #[cfg(debug_assertions)]
        smoke::start(&ui);
    });
    app.connect_open(move |app, files, _| {
        if controller.borrow().upgrade().is_none() {
            app.activate();
        }
        if let Some(ui) = controller.borrow().upgrade() {
            ui.window.present();
            if let Some(file) = files.first() {
                if let Some(path) = file.path() {
                    ui.open_local_file(&path);
                } else {
                    ui.error("Open a file on this server's local filesystem");
                }
            }
        }
    });
    let code = app.run();
    drop(app);
    // Give the actor time to close the parent pipe and reap the owned gateway.
    if let Ok(runtime) = Rc::try_unwrap(runtime) {
        runtime.shutdown_timeout(Duration::from_secs(6));
    }
    code
}

fn build(
    app: &adw::Application,
    sender: mpsc::Sender<Command>,
    documents: mpsc::Sender<crate::document::Message>,
    session_rows_sender: tokio::sync::watch::Sender<Option<crate::session_list::Request>>,
    launch_control: tokio::sync::watch::Sender<crate::gateway_launch::Control>,
    turn_rows: tokio::sync::watch::Sender<Option<crate::turn_navigation::Request>>,
) -> Rc<Ui> {
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Codex Thread Studio · GNOME")
        .default_width(1280)
        .default_height(850)
        .build();
    let toast = adw::ToastOverlay::new();
    let write_feedback = crate::write_feedback::Queue::new(&window, &toast);
    let appearance = crate::appearance_ui::Runtime::install();
    let inline_math = crate::inline_math::Renderer::install(documents.clone());
    let reader = crate::reader::Reader::new(1, &window, documents.clone(), &toast);
    let tables = crate::tables_ui::Tables::new(&window, sender.clone(), &toast);
    let images = crate::images_ui::Renderer::new(sender.clone());
    let image_viewer =
        crate::images_ui::Viewer::new(&window, images.clone(), write_feedback.clone());
    let pdf = crate::pdf_ui::Pdf::new(&window, sender.clone());
    let epub = crate::epub_ui::Epub::new(&window, sender.clone());
    let browser = crate::browser_ui::Browser::new(&window, sender.clone());
    reader.image_renderer.replace(Some(images.clone()));
    let terminal = crate::terminal::Terminal::new(sender.clone(), &toast);
    let files = crate::workspace_ui::Files::new(&window, sender.clone(), &toast);
    let source = crate::workspace_ui::Source::new(sender.clone(), &toast);
    let git = crate::git_ui::Git::new(sender.clone(), &toast);
    reader.widget.set_visible(false);
    let sessions = gio::ListStore::new::<glib::BoxedAnyObject>();
    let selection = gtk::SingleSelection::new(Some(sessions.clone()));
    selection.set_autoselect(false);
    selection.set_can_unselect(true);
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, object| {
        let item = object.downcast_ref::<gtk::ListItem>().unwrap();
        let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
        row.set_margin_top(10);
        row.set_margin_bottom(10);
        row.set_margin_start(12);
        row.set_margin_end(12);
        let title = gtk::Label::builder()
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        title.add_css_class("heading");
        let cwd = gtk::Label::builder()
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::Middle)
            .build();
        cwd.add_css_class("caption");
        cwd.add_css_class("dim-label");
        row.append(&title);
        row.append(&cwd);
        item.set_child(Some(&row));
    });
    factory.connect_bind(|_, object| {
        let item = object.downcast_ref::<gtk::ListItem>().unwrap();
        let data = item.item().and_downcast::<glib::BoxedAnyObject>().unwrap();
        let thread = data.borrow::<Thread>();
        let row = item.child().unwrap();
        let title = row.first_child().and_downcast::<gtk::Label>().unwrap();
        let mut parts = Vec::new();
        if let Some(activity) = match thread.activity.as_str() {
            "attention" => Some("Attention"),
            "active" => Some("Working"),
            "unknown" => Some("Unknown"),
            _ => None,
        } {
            parts.push(crate::i18n_ui::Part::message(activity, &[]));
        }
        if thread.pinned {
            parts.push(crate::i18n_ui::Part::message("Pinned", &[]));
        }
        parts.push(crate::i18n_ui::Part::literal(thread.title.clone()));
        crate::i18n_ui::bind_parts(&title, "label", parts, " · ");
        title
            .next_sibling()
            .and_downcast::<gtk::Label>()
            .unwrap()
            .set_text(&thread.cwd);
    });
    let list = gtk::ListView::new(Some(selection.clone()), Some(factory));
    list.add_css_class("navigation-sidebar");
    list.set_single_click_activate(true);
    let sidebar_header = adw::HeaderBar::new();
    sidebar_header.set_show_start_title_buttons(false);
    sidebar_header.set_show_end_title_buttons(false);
    sidebar_header.set_title_widget(Some(&gtk::Label::new(None).ui_text("label", "Sessions")));
    let new = icon_button("list-add-symbolic", "New session", "win.new-session");
    sidebar_header.pack_start(&new);
    sidebar_header.pack_end(&icon_button(
        "view-refresh-symbolic",
        "Refresh sessions",
        "win.refresh",
    ));
    let backend = gtk::DropDown::from_strings(&["Starting gateway…"]);
    backend.set_sensitive(false);
    let search = gtk::SearchEntry::builder()
        .placeholder_text("Search sessions or paths")
        .build();
    crate::i18n_ui::bind(&search, "placeholder-text", "Search sessions or paths");
    let controls = gtk::Box::new(gtk::Orientation::Vertical, 10);
    controls.set_margin_start(12);
    controls.set_margin_end(12);
    controls.set_margin_bottom(12);
    controls.append(&backend);
    let scope = gtk::DropDown::from_strings(&[
        "All sessions",
        "Pinned sessions",
        "Archived sessions",
        "Active sessions",
        "Attention",
    ]);
    crate::i18n_ui::choices(&scope);
    controls.append(&scope);
    controls.append(&search);
    let more = gtk::Button::with_label("Load more sessions");
    crate::i18n_ui::bind(&more, "label", "Load more sessions");
    more.set_visible(false);
    let sidebar_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    sidebar_box.append(&controls);
    sidebar_box.append(
        &gtk::ScrolledWindow::builder()
            .child(&list)
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build(),
    );
    sidebar_box.append(&more);
    let sidebar = adw::ToolbarView::new();
    sidebar.add_top_bar(&sidebar_header);
    sidebar.set_content(Some(&sidebar_box));

    let header = adw::HeaderBar::new();
    let router_back = gtk::Button::builder()
        .icon_name("go-previous-symbolic")
        .tooltip_text("Return to Router")
        .visible(false)
        .build();
    crate::i18n_ui::bind(&router_back, "tooltip-text", "Return to Router");
    header.pack_start(&router_back);
    let title = adw::WindowTitle::new("Codex Thread Studio", "Choose a session or open a document");
    crate::i18n_ui::bind(&title, "subtitle", "Choose a session or open a document");
    header.set_title_widget(Some(&title));
    let pin = gtk::ToggleButton::builder()
        .sensitive(false)
        .build()
        .ui_text("tooltip-text", "Pin selected session");
    pin.set_child(Some(&gtk::Image::from_gicon(&gio::ThemedIcon::from_names(
        &["view-pin-symbolic", "starred-symbolic"],
    ))));
    header.pack_end(&pin);
    header.pack_end(&icon_button(
        "system-search-symbolic",
        "Find in conversation (Ctrl+Shift+F)",
        "win.find-conversation",
    ));
    header.pack_start(&icon_button(
        "sidebar-show-symbolic",
        "Toggle session sidebar",
        "win.sidebar",
    ));
    header.pack_end(&icon_button(
        "document-open-symbolic",
        "Open document",
        "win.open-document",
    ));
    header.pack_end(&icon_button(
        "utilities-terminal-symbolic",
        "Terminal",
        "win.terminal",
    ));
    header.pack_end(&icon_button("folder-symbolic", "Files", "win.files"));
    header.pack_end(&icon_button("vcs-branch-symbolic", "Git Review", "win.git"));
    header.pack_end(&icon_button(
        "view-dual-symbolic",
        "Compare Markdown files",
        "win.compare",
    ));
    let menu = gio::Menu::new();
    update_menu(&menu);
    header.pack_end(&crate::action_menu::button(&menu).ui_text("tooltip-text", "Main menu"));

    let transcript = gio::ListStore::new::<glib::BoxedAnyObject>();
    let link_owner: Rc<RefCell<std::rc::Weak<Ui>>> = Rc::new(RefCell::new(std::rc::Weak::new()));
    let links = link_owner.clone();
    let open: crate::chat_ui::MessageLink = Rc::new(move |item, uri| {
        if let Some(ui) = links.borrow().upgrade() {
            if let Some(origin) = &item.origin {
                ui.activate_reference_from(uri, &origin.root);
            } else {
                ui.activate_reference(uri);
            }
        }
    });
    let chat_diagrams = crate::chat_diagrams::Renderer::new(documents.clone());
    let favorites_owner = link_owner.clone();
    let favorite: crate::chat_ui::FavoriteAction = Rc::new(move |item, selection| {
        if let Some(ui) = favorites_owner.borrow().upgrade() {
            ui.save_favorite(item, selection);
        }
    });
    let comments_owner = link_owner.clone();
    let comment: crate::chat_ui::CommentAction = Rc::new(move |item, selection| {
        if let Some(ui) = comments_owner.borrow().upgrade() {
            ui.comment_reply(item, selection);
        }
    });
    let target_owner = link_owner.clone();
    let open_target: crate::chat_ui::SourceAction = Rc::new(move |item| {
        if let Some(ui) = target_owner.borrow().upgrade() {
            ui.router_open_target(&item);
        }
    });
    let source_owner = link_owner.clone();
    let open_source: crate::chat_ui::SourceAction = Rc::new(move |item| {
        if let Some(ui) = source_owner.borrow().upgrade() {
            ui.router_source(&item);
        }
    });
    let chat_visibility = crate::chat_visibility::Visibility::new();
    let image_owner = link_owner.clone();
    let image_content: crate::chat_ui::ImageAction = Rc::new(move |item, content, open| {
        if let Some(ui) = image_owner.borrow().upgrade() {
            let root = item
                .origin
                .as_ref()
                .map(|origin| origin.root.clone())
                .or_else(|| {
                    ui.state
                        .borrow()
                        .selected
                        .as_ref()
                        .map(|thread| thread.cwd.clone())
                })
                .unwrap_or_default();
            crate::images_ui::chat(&ui.images, content, &root, open)
        } else {
            match content {
                crate::chat_ui::ImageContent::Cell(_, _) => {
                    gtk::Box::new(gtk::Orientation::Vertical, 0).upcast()
                }
                crate::chat_ui::ImageContent::Markdown(content, reading) => {
                    crate::content_widgets::formatted_at(content, open, reading)
                }
                crate::chat_ui::ImageContent::Attachment(image) => {
                    gtk::Label::new(Some(&image.name)).upcast()
                }
            }
        }
    });
    let items_factory = crate::chat_ui::factory_observed(
        open,
        crate::content_widgets::Styles::new(),
        chat_diagrams.clone(),
        Some(favorite),
        Some(comment),
        Some(open_source),
        Some(chat_visibility.clone()),
        Some(open_target),
        Some(image_content),
    );
    let transcript_view = gtk::ListView::new(
        Some(gtk::NoSelection::new(Some(transcript.clone()))),
        Some(items_factory),
    );
    transcript_view.add_css_class("background");
    let transcript_scroll = gtk::ScrolledWindow::builder()
        .child(&transcript_view)
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();
    let transcript_navigation = crate::list_navigation::Navigation::new(&transcript_scroll);
    let turn_navigation = crate::turn_navigation_ui::Panel::new(
        turn_rows,
        chat_visibility.clone(),
        &transcript_scroll,
    );

    let composer = gtk::TextView::builder()
        .wrap_mode(gtk::WrapMode::WordChar)
        .top_margin(12)
        .bottom_margin(12)
        .left_margin(12)
        .right_margin(12)
        .build();
    composer.set_accepts_tab(false);
    crate::i18n_ui::bind(
        &composer,
        "tooltip-text",
        "Write a message. Ctrl+Enter sends; Enter starts a new line.",
    );
    crate::i18n_ui::accessible_label(
        &composer,
        "Write a message. Ctrl+Enter sends; Enter starts a new line.",
    );
    let composer_scroll = gtk::ScrolledWindow::builder()
        .child(&composer)
        .min_content_height(75)
        .max_content_height(200)
        .propagate_natural_height(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();
    let model = gtk::DropDown::from_strings(&["Default model"]);
    crate::adaptive::string_selector(&model, 22);
    model.set_hexpand(true);
    let effort = gtk::DropDown::from_strings(&["Default effort", "low", "medium", "high", "xhigh"]);
    crate::adaptive::string_selector(&effort, 12);
    let fast = gtk::ToggleButton::builder()
        .label("Fast")
        .build()
        .ui_text("tooltip-text", "Fast service tier for the next turn");
    let send = gtk::Button::with_label("Send");
    crate::i18n_ui::bind(&send, "label", "Send");
    send.add_css_class("suggested-action");
    send.set_sensitive(false);
    let stop = gtk::Button::with_label("Stop");
    crate::i18n_ui::bind(&stop, "label", "Stop");
    let queue_button = gtk::Button::with_label("Queue");
    crate::i18n_ui::bind(&queue_button, "label", "Queue");
    crate::i18n_ui::bind(
        &queue_button,
        "tooltip-text",
        "Send after the current turn completes",
    );
    let queue = crate::queue_ui::Panel::new();
    let continue_button = gtk::Button::new().ui_text("label", "Continue").ui_text(
        "tooltip-text",
        "Continue with the configured behavior · Ctrl+Shift+Enter",
    );
    let supervision_menu = crate::supervision_ui::Menu::new();
    let image_button = gtk::Button::builder()
        .icon_name("mail-attachment-symbolic")
        .build()
        .ui_text("tooltip-text", "Attach images (PNG, JPEG, WebP, GIF)");
    let image_chips = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .min_children_per_line(1)
        .max_children_per_line(4)
        .row_spacing(4)
        .column_spacing(4)
        .margin_start(8)
        .margin_end(8)
        .visible(false)
        .build();
    stop.set_sensitive(false);
    let actions = crate::adaptive::toolbar(8);
    actions.set_margin_start(8);
    actions.set_margin_end(8);
    actions.set_margin_bottom(8);
    actions.append(&image_button);
    let router_file_button = gtk::Button::builder()
        .icon_name("text-x-generic-symbolic")
        .visible(false)
        .build()
        .ui_text("tooltip-text", "Attach reference files to Router");
    actions.append(&router_file_button);
    actions.append(&model);
    actions.append(&effort);
    actions.append(&fast);
    actions.append(&supervision_menu.button);
    actions.append(&stop);
    actions.append(&queue_button);
    actions.append(&continue_button);
    actions.append(&send);
    let composer_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
    composer_box.append(&composer_scroll);
    let shell_hint=gtk::Label::builder().label("Shell commands use your local user access and do not inherit model sandbox permissions.").xalign(0.0).wrap(true).margin_start(12).margin_end(12).visible(false).build();
    shell_hint.add_css_class("caption");
    shell_hint.add_css_class("dim-label");
    composer_box.append(&shell_hint);
    let composer_tools = crate::composer_tools_ui::Tools::new(&composer_box);
    composer_box.append(&composer_tools.widget);
    composer_box.append(&image_chips);
    let router_target_label = gtk::Label::builder()
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .max_width_chars(45)
        .build()
        .ui_text("label", "Automatic routing");
    let target_content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    target_content.append(&gtk::Image::from_icon_name("send-to-symbolic"));
    target_content.append(&router_target_label);
    target_content.append(&gtk::Image::from_icon_name("pan-down-symbolic"));
    let router_target = gtk::Button::builder()
        .child(&target_content)
        .halign(gtk::Align::Start)
        .margin_start(8)
        .margin_end(8)
        .visible(false)
        .build()
        .ui_text("tooltip-text", "Choose a Router target");
    router_target.add_css_class("flat");
    composer_box.append(&router_target);
    composer_box.append(&actions);
    composer_box.add_css_class("card");
    let composer_clamp = adw::Clamp::builder()
        .maximum_size(1000)
        .tightening_threshold(800)
        .child(&composer_box)
        .build();
    composer_clamp.set_margin_top(12);
    crate::appearance_ui::watch_clamp(&composer_clamp);
    composer.add_css_class("studio-content");
    composer_clamp.set_margin_bottom(16);
    composer_clamp.set_margin_start(24);
    composer_clamp.set_margin_end(24);
    let status = gtk::Label::builder()
        .label("Starting Studio…")
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .build();
    status.add_css_class("caption");
    status.add_css_class("dim-label");
    status.set_margin_start(24);
    status.set_margin_end(24);
    status.set_hexpand(true);
    let supervision_status = gtk::Label::builder()
        .xalign(1.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .max_width_chars(42)
        .margin_end(24)
        .visible(false)
        .css_classes(["caption", "accent"])
        .build();
    let status_line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    status_line.append(&status);
    status_line.append(&turn_navigation.widget);
    status_line.append(&supervision_status);
    let content_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let router_bar = crate::router_ui::Bar::new();
    let history_pages = crate::conversation_pages_ui::Pager::new(sender.clone());
    content_box.append(&router_bar.widget);
    content_box.append(&history_pages.widget);
    content_box.append(&transcript_scroll);
    content_box.append(&status_line);
    content_box.append(&queue.widget);
    content_box.append(&composer_clamp);
    let content = adw::ToolbarView::new();
    let finder = crate::session_search_ui::Search::new();
    content.add_top_bar(&finder.widget);
    content.set_content(Some(&content_box));
    let workspace = gtk::Paned::new(gtk::Orientation::Horizontal);
    let compare =
        crate::compare_ui::Compare::new(&window, documents.clone(), sender.clone(), &toast);
    workspace.set_start_child(Some(&content));
    let tools = gtk::Stack::new();
    tools.set_hhomogeneous(false);
    tools.set_vhomogeneous(false);
    tools.add_named(&reader.widget, Some("document"));
    tools.add_named(&tables.widget, Some("table"));
    tools.add_named(&image_viewer.widget, Some("image"));
    tools.add_named(&pdf.widget, Some("pdf"));
    tools.add_named(&epub.widget, Some("epub"));
    tools.add_named(&browser.widget, Some("browser"));
    compare.left.image_renderer.replace(Some(images.clone()));
    compare.right.image_renderer.replace(Some(images.clone()));
    tools.add_named(&terminal.widget, Some("terminal"));
    tools.add_named(&files.widget, Some("files"));
    tools.add_named(&source.widget, Some("source"));
    tools.add_named(&git.widget, Some("git"));
    tools.add_named(&compare.widget, Some("compare"));
    let session_map = crate::session_map_ui::Panel::new(&window, sender.clone());
    tools.add_named(&session_map.widget, Some("map"));
    let resources = crate::session_resources_ui::Panel::new(sender.clone());
    tools.add_named(&resources.widget, Some("resources"));
    tools.set_visible(false);
    workspace.set_end_child(Some(&tools));
    workspace.set_resize_start_child(true);
    workspace.set_resize_end_child(true);
    // Let GTK constrain divider travel to each pane's measured minimum. Allowing
    // smaller allocations clips controls when typography or queued rows grow.
    workspace.set_shrink_start_child(false);
    workspace.set_shrink_end_child(false);
    workspace.set_position(600);
    let split = adw::OverlaySplitView::builder()
        .sidebar(&sidebar)
        .content(&workspace)
        .min_sidebar_width(240.0)
        .max_sidebar_width(340.0)
        .sidebar_width_fraction(0.25)
        .build();
    let breakpoint =
        adw::Breakpoint::new(adw::BreakpointCondition::parse("max-width: 800sp").unwrap());
    breakpoint.add_setter(&split, "collapsed", Some(&true.to_value()));
    window.add_breakpoint(breakpoint);
    let shell = adw::ToolbarView::new();
    shell.add_top_bar(&header);
    shell.set_content(Some(&split));
    toast.set_child(Some(&shell));
    window.set_content(Some(&toast));
    let translation = crate::translation_ui::Runtime::new(&window, sender.clone());
    let ui = Rc::new(Ui {
        launch_control,
        recovery: RefCell::new(Default::default()),
        gateway_stopped: Cell::new(false),
        translation,
        menu: menu.clone(),
        streaming: RefCell::new(Default::default()),
        notifications: crate::notifications_ui::Runtime::new(app, sender.clone()),
        language: crate::language_ui::Runtime::new(sender.clone(), write_feedback.clone()),
        message_flow: crate::message_flow_ui::Runtime::new(sender.clone(), write_feedback.clone()),
        continuation: RefCell::default(),
        composer_revision: Cell::new(0),
        continue_button,
        #[cfg(debug_assertions)]
        event_max_ms: std::cell::Cell::new(0),
        #[cfg(debug_assertions)]
        render_max_ms: std::cell::Cell::new(0),
        appearance,
        queue,
        queue_button,
        supervision: RefCell::default(),
        supervision_menu,
        supervision_status,
        supervision_options: RefCell::default(),
        favorites: RefCell::new(None),
        annotations: RefCell::new(None),
        annotation_prepared: RefCell::new(None),
        history_context: RefCell::new(None),
        output: crate::conversation_output_ui::Host::install(
            &window,
            sender.clone(),
            write_feedback.clone(),
        ),
        write_feedback,
        document_sender: documents,
        finder,
        chat_diagrams,
        inline_math,
        settings: RefCell::new(None),
        fast,
        session_list: list,
        pin,
        scope,
        window,
        split,
        title,
        status,
        toast,
        backend,
        sessions,
        selection,
        search,
        more,
        transcript,
        transcript_scroll,
        transcript_view,
        transcript_navigation,
        turn_navigation,
        history_pages,
        composer,
        composer_tools,
        shell_hint,
        palette: RefCell::new(None),
        interactions: RefCell::new(Default::default()),
        approvals: RefCell::new(Default::default()),
        permission_dialog: RefCell::new(None),
        status_dialog: RefCell::new(None),
        mcp: RefCell::new(None),
        agents: RefCell::new(None),
        session_map,
        resources,
        session_info: RefCell::new(None),
        connections: RefCell::new(None),
        router_settings: RefCell::new(None),
        router: RefCell::new(router_chat::Book::default()),
        chat_visibility,
        router_bar,
        router_back,
        router_target,
        router_target_label,
        router_file_button,
        router_picker: RefCell::new(None),
        file_picker: Default::default(),
        image_button,
        image_chips,
        image_stamp: RefCell::new((String::new(), 0)),
        send,
        stop,
        model,
        effort,
        sender,
        session_rows_sender,
        reader,
        tables,
        images,
        image_viewer,
        pdf,
        epub,
        browser,
        workspace,
        conversation: content,
        tools,
        tool_navigation: RefCell::default(),
        terminal,
        files,
        source,
        git,
        compare,
        state: RefCell::new(State::default()),
    });
    let weak = Rc::downgrade(&ui);
    *ui.translation.context.borrow_mut() = Some(Box::new(move || {
        let ui = weak.upgrade()?;
        let state = ui.state.borrow();
        let thread = state.selected.as_ref()?;
        let backend = state
            .backends
            .iter()
            .find(|b| b.id == state.backend)?
            .clone();
        let model = if !state.options.model.is_empty() {
            state.options.model.clone()
        } else if !state.session_model.is_empty() {
            state.session_model.clone()
        } else {
            state.default_model.clone()
        };
        Some(crate::translation::Context {
            backend,
            socket: state.socket_generation,
            thread: thread.id.clone(),
            cwd: thread.cwd.clone(),
            model,
            ready: state.ready,
        })
    }));
    let weak = Rc::downgrade(&ui);
    *ui.translation.error.borrow_mut() = Some(Box::new(move |error| {
        if let Some(ui) = weak.upgrade() {
            ui.error(error);
        }
    }));
    *link_owner.borrow_mut() = Rc::downgrade(&ui);
    ui.connect_session_map();
    ui.connect_resources();
    ui.connect_supervision();
    let weak = Rc::downgrade(&ui);
    *ui.images.on_open_embedded.borrow_mut() = Some(Rc::new(move |source, title| {
        if let Some(ui) = weak.upgrade() {
            ui.image_viewer.open_embedded(source, title);
        }
    }));
    ui
}

fn update_menu(menu: &gio::Menu) {
    menu.remove_all();
    menu.append(
        Some(&crate::language::tr("Connections & accounts")),
        Some("win.connections"),
    );
    menu.append(
        Some(&crate::language::tr("Session information")),
        Some("win.session-info"),
    );
    menu.append(
        Some(&crate::language::tr("Resources")),
        Some("win.resources"),
    );
    menu.append(
        Some(&crate::language::tr("Open file…")),
        Some("win.open-file"),
    );
    menu.append(Some(&crate::language::tr("Browser")), Some("win.browser"));
    menu.append(
        Some(&crate::language::tr("Session Map")),
        Some("win.session-map"),
    );
    menu.append(
        Some(&crate::language::tr("Translate selected text")),
        Some("win.translate-selection"),
    );
    menu.append(
        Some(&crate::language::tr("Settings…")),
        Some("win.settings"),
    );
    menu.append(
        Some(&crate::language::tr("Commands…")),
        Some("win.commands"),
    );
    menu.append(
        Some(&crate::language::tr("Thread Router settings…")),
        Some("win.router-settings"),
    );
    menu.append(
        Some(&crate::language::tr("Mark visible Router replies read")),
        Some("win.router-mark-visible-read"),
    );
    menu.append(
        Some(&crate::language::tr("Favorites…")),
        Some("win.favorites"),
    );
    menu.append(
        Some(&crate::language::tr("Comments…")),
        Some("win.annotations"),
    );
    menu.append(
        Some(&crate::language::tr("Browse conversation history…")),
        Some("win.history"),
    );
    menu.append(
        Some(&crate::language::tr("System appearance")),
        Some("win.theme-system"),
    );
    menu.append(
        Some(&crate::language::tr("Light appearance")),
        Some("win.theme-light"),
    );
    menu.append(
        Some(&crate::language::tr("Dark appearance")),
        Some("win.theme-dark"),
    );
    menu.append(
        Some(&crate::language::tr("Reconnect backend")),
        Some("win.reconnect"),
    );
    menu.append(Some(&crate::language::tr("About")), Some("win.about"));
    let session_menu = gio::Menu::new();
    for (label, action) in [
        ("Rename session…", "rename-session"),
        ("Fork session", "fork-session"),
        ("Archive session", "archive-session"),
        ("Restore archived session", "restore-session"),
        ("Delete session…", "delete-session"),
    ] {
        session_menu.append(
            Some(&crate::language::tr(label)),
            Some(&format!("win.{action}")),
        );
    }
    menu.append_section(
        Some(&crate::language::tr("Selected session")),
        &session_menu,
    );
}

fn icon_button(icon: &str, tooltip: &str, action: &str) -> gtk::Button {
    let button = gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(tooltip)
        .action_name(action)
        .build();
    crate::i18n_ui::bind(&button, "tooltip-text", tooltip);
    if icon == "vcs-branch-symbolic" {
        button.set_child(Some(&gtk::Image::from_gicon(&gio::ThemedIcon::from_names(
            &[icon, "view-list-symbolic"],
        ))));
    }
    button
}

fn connect(ui: &Rc<Ui>) {
    crate::translation_ui::bind(&ui.composer);
    let action = gio::SimpleAction::new("translate-selection", None);
    let weak = Rc::downgrade(ui);
    action.connect_activate(move |_, _| {
        if let Some(ui) = weak.upgrade() {
            ui.translation.translate_selection();
        }
    });
    ui.window.add_action(&action);
    if let Some(app) = ui.window.application() {
        app.set_accels_for_action("win.translate-selection", &["<Control><Shift>t"]);
    }

    let weak = Rc::downgrade(ui);
    *ui.language.on_changed.borrow_mut() = Some(Box::new(move || {
        if let Some(ui) = weak.upgrade() {
            update_menu(&ui.menu);
            ui.message_flow.refresh_language();
        }
    }));
    let weak = Rc::downgrade(ui);
    *ui.message_flow.on_changed.borrow_mut() = Some(Box::new(move |depth| {
        if let Some(ui) = weak.upgrade() {
            {
                let mut state = ui.state.borrow_mut();
                for entry in state.queues.entries.values_mut() {
                    entry.limit = depth as usize;
                    entry.touch();
                }
                state.queues.dirty = true;
            }
            ui.update_composer();
        }
    }));
    ui.connect_history_pages();
    ui.connect_router_bar();
    ui.connect_queue();
    ui.connect_references();
    ui.connect_continuation();
    ui.connect_attachments();
    let weak = Rc::downgrade(ui);
    *ui.finder.on_request.borrow_mut() =
        Some(Box::new(move |serial, query, case_sensitive, filter| {
            if let Some(ui) = weak.upgrade() {
                let request = {
                    let state = ui.state.borrow();
                    state
                        .selected
                        .as_ref()
                        .map(|thread| crate::session_search::Request {
                            context: crate::session_search::Context {
                                backend: state.backend.clone(),
                                thread: thread.id.clone(),
                                generation: state.socket_generation,
                            },
                            serial,
                            items: ui
                                .history_pages
                                .items()
                                .unwrap_or_else(|| state.items.clone()),
                            query,
                            case_sensitive,
                            filter,
                            directory: thread.cwd.clone(),
                            backend_kind: state
                                .backends
                                .iter()
                                .find(|backend| backend.id == state.backend)
                                .map(|backend| backend.kind.clone())
                                .unwrap_or_default(),
                        })
                };
                if let Some(request) = request {
                    if !ui.command(Command::SessionSearch(Some(request))) {
                        ui.finder
                            .apply(serial, Err("Search could not be started".into()));
                    }
                } else {
                    ui.finder
                        .apply(serial, Err("Choose a session to search".into()));
                }
            }
        }));
    let weak = Rc::downgrade(ui);
    *ui.finder.on_select.borrow_mut() = Some(Box::new(move |hit| {
        if let Some(ui) = weak.upgrade() {
            if let Some(position) = crate::chat_ui::locate(&ui.transcript, &hit) {
                ui.transcript_navigation.jump(
                    &ui.transcript_view,
                    position,
                    gtk::ListScrollFlags::NONE,
                );
            } else {
                if hit.source.is_none() {
                    ui.open_history_context(hit);
                } else {
                    ui.finder.perform();
                }
            }
        }
    }));
    let weak = Rc::downgrade(ui);
    *ui.turn_navigation.on_jump.borrow_mut() = Some(Box::new(move |context, marker| {
        let Some(ui) = weak.upgrade() else { return };
        let state = ui.state.borrow();
        let current = state.backend == context.backend
            && state.socket_generation == context.generation
            && state
                .selected
                .as_ref()
                .is_some_and(|t| t.id == context.thread);
        drop(state);
        if !current {
            return;
        }
        let position = (0..ui.transcript.n_items()).find(|position| {
            ui.transcript
                .item(*position)
                .and_downcast::<glib::BoxedAnyObject>()
                .is_some_and(|object| {
                    let row = object.borrow::<crate::chat_ui::Row>();
                    row.part == crate::chat_ui::Part::Header
                        && row.item.role == "You"
                        && row.item.id == marker.item
                        && row.item.turn_id == marker.turn
                })
        });
        if let Some(position) = position {
            let visibility = Rc::downgrade(&ui.chat_visibility);
            ui.transcript_navigation.jump_start(
                &ui.transcript_view,
                &ui.transcript_scroll,
                position,
                move || {
                    visibility
                        .upgrade()
                        .and_then(|v| v.header(&marker.item, &marker.turn))
                },
            );
        }
    }));
    let weak = Rc::downgrade(ui);
    *ui.finder.on_cancel.borrow_mut() = Some(Box::new(move || {
        if let Some(ui) = weak.upgrade() {
            ui.command(Command::SessionSearch(None));
        }
    }));
    for control in [&ui.model, &ui.effort] {
        let weak = Rc::downgrade(ui);
        control.connect_selected_notify(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.save_options(false);
            }
        });
    }
    let weak = Rc::downgrade(ui);
    ui.fast.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.save_options(true);
        }
    });
    let weak = Rc::downgrade(ui);
    ui.scope.connect_selected_notify(move |scope| {
        if let Some(ui) = weak.upgrade() {
            if ui.state.borrow().session_mutation {
                return;
            }
            let archived = scope.selected() == 2;
            let open_code = {
                let state = ui.state.borrow();
                state
                    .backends
                    .iter()
                    .any(|backend| backend.id == state.backend && backend.kind == "opencode")
            };
            if archived && open_code {
                scope.set_selected(0);
                ui.error("OpenCode does not provide archived sessions");
                return;
            }
            let changed = ui.state.borrow().archived != archived;
            {
                let mut state = ui.state.borrow_mut();
                state.archived = archived;
                state.reference_selection = None;
                state.pinned_only = scope.selected() == 1;
            }
            if changed {
                ui.stash_current();
                ui.remember_draft();
                {
                    let mut state = ui.state.borrow_mut();
                    state.reconnecting = None;
                    state.selected = None;
                    state.items.clear();
                    state.active_turn = None;
                    state.render_dirty = true;
                }
                ui.composer.buffer().set_text("");
                ui.catalog(false);
            } else {
                ui.render_sessions();
            }
            ui.update_composer();
        }
    });
    let weak = Rc::downgrade(ui);
    ui.pin.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.toggle_pin();
        }
    });
    let weak = Rc::downgrade(ui);
    *ui.compare.on_reader.borrow_mut() = Some(Box::new(move || {
        if let Some(ui) = weak.upgrade() {
            if let Some((root, path)) = ui.compare.left.location() {
                if ui.reader.location().as_ref() != Some(&(root.clone(), path.clone())) {
                    ui.reader.open_project_path(&root, &path, None, None);
                    if ui.reader.location().as_ref() != Some(&(root, path)) {
                        return;
                    }
                }
            }
            ui.show_tool("document");
        }
    }));
    let weak = Rc::downgrade(ui);
    let open_workspace = Rc::new(move |root: &str, path: &str| {
        if let Some(ui) = weak.upgrade() {
            let extension = std::path::Path::new(path)
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if matches!(extension.as_str(), "md" | "markdown" | "mdown") {
                ui.reader.open_project_path(
                    std::path::Path::new(root),
                    &std::path::Path::new(root).join(path),
                    None,
                    None,
                );
            } else if crate::epub::supports(std::path::Path::new(path)) {
                ui.epub.open(root, path);
            } else if crate::pdf::supports(std::path::Path::new(path)) {
                ui.pdf.open(root, path, 0);
            } else if crate::images::supports(std::path::Path::new(path)) {
                ui.image_viewer.open(root, path);
            } else if crate::tables::supports(std::path::Path::new(path)) {
                ui.tables.open(root, path);
            } else {
                ui.source.open(root, path);
                ui.show_tool("source");
            }
        }
    });
    let open = open_workspace.clone();
    *ui.files.on_open.borrow_mut() = Some(Box::new(move |root, path| open(root, path)));
    *ui.git.on_open.borrow_mut() = Some(Box::new(move |root, path| open_workspace(root, path)));
    let weak = Rc::downgrade(ui);
    *ui.source.on_comment.borrow_mut() = Some(Box::new(move |capture| {
        if let Some(ui) = weak.upgrade() {
            ui.comment_panel(Some(capture));
        }
    }));
    for reader in [&ui.reader, &ui.compare.left, &ui.compare.right] {
        let weak = Rc::downgrade(ui);
        *reader.on_comment.borrow_mut() = Some(Box::new(move |capture| {
            if let Some(ui) = weak.upgrade() {
                ui.comment_panel(Some(capture));
            }
        }));
        let weak = Rc::downgrade(ui);
        *reader.on_file.borrow_mut() = Some(Box::new(move |root, path, line| {
            if let Some(ui) = weak.upgrade() {
                if crate::epub::supports(std::path::Path::new(path)) {
                    ui.epub.open(root, path);
                } else if crate::pdf::supports(std::path::Path::new(path)) {
                    ui.pdf.open(root, path, line.unwrap_or(1).saturating_sub(1));
                } else if line.is_none() && crate::images::supports(std::path::Path::new(path)) {
                    ui.image_viewer.open(root, path);
                } else if line.is_none() && crate::tables::supports(std::path::Path::new(path)) {
                    ui.tables.open(root, path);
                } else {
                    if let Some(line) = line {
                        ui.source.open_at(root, path, line);
                    } else {
                        ui.source.open(root, path);
                    }
                    ui.show_tool("source");
                }
            }
        }));
    }
    let weak = Rc::downgrade(ui);
    let comments = Rc::new(crate::annotations_ui::CaptureTarget {
        owner: Box::new({
            let weak = weak.clone();
            move || weak.upgrade().and_then(|ui| ui.composer_key())
        }),
        capture: Box::new(move |owner, capture| {
            let ui = weak.upgrade().ok_or("Studio window is closed")?;
            if ui.composer_key().as_deref() != Some(owner.as_str()) {
                return Err(
                    "Conversation changed; select the passage again before commenting".into(),
                );
            }
            let existing = ui.annotations.borrow().clone();
            if existing.is_some_and(|p| p.key != owner) {
                return Err("Close the other session's Comments panel first".into());
            }
            ui.comment_panel(Some(capture));
            Ok(())
        }),
    });
    for reader in [&ui.reader, &ui.compare.left, &ui.compare.right] {
        reader.comments.replace(Some(comments.clone()));
    }
    ui.browser.comments.replace(Some(comments.clone()));
    ui.epub.comments.replace(Some(comments.clone()));
    ui.pdf.comments.replace(Some(comments.clone()));
    ui.tables.comments.replace(Some(comments));
    for (slot, name) in [
        (&ui.epub.on_close, "epub"),
        (&ui.pdf.on_close, "pdf"),
        (&ui.image_viewer.on_close, "image"),
        (&ui.tables.on_close, "table"),
        (&ui.files.on_close, "files"),
        (&ui.source.on_close, "source"),
        (&ui.git.on_close, "git"),
        (&ui.reader.on_closing, "document"),
        (&ui.terminal.on_close, "terminal"),
        (&ui.browser.on_hide, "browser"),
        (&ui.session_map.on_close, "map"),
    ] {
        let weak = Rc::downgrade(ui);
        *slot.borrow_mut() = Some(Box::new(move || {
            if let Some(ui) = weak.upgrade() {
                ui.close_tool(name);
            }
        }));
    }
    for (slot, name) in [
        (&ui.reader.on_open, "document"),
        (&ui.source.on_open, "source"),
        (&ui.image_viewer.on_open, "image"),
        (&ui.pdf.on_open, "pdf"),
        (&ui.epub.on_open, "epub"),
        (&ui.tables.on_open, "table"),
        (&ui.browser.on_open, "browser"),
    ] {
        let weak = Rc::downgrade(ui);
        *slot.borrow_mut() = Some(Box::new(move || {
            if let Some(ui) = weak.upgrade() {
                ui.present_viewer(name);
            }
        }));
    }
    let weak = Rc::downgrade(ui);
    ui.reader.widget.connect_visible_notify(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.update_workspace();
        }
    });
    let weak = Rc::downgrade(ui);
    let previous_width = std::cell::Cell::new(0);
    ui.workspace.add_tick_callback(move |_, _| {
        let Some(ui) = weak.upgrade() else {
            return glib::ControlFlow::Break;
        };
        let width = ui.toast.width();
        if width != previous_width.replace(width) {
            ui.update_workspace();
        }
        glib::ControlFlow::Continue
    });
    for (name, handler) in [
        ("new-session", Ui::new_session as fn(&Rc<Ui>)),
        ("refresh", Ui::refresh),
        ("sidebar", Ui::toggle_sidebar),
        ("reconnect", Ui::reconnect),
        ("about", Ui::about),
        ("settings", Ui::open_settings),
        ("commands", Ui::open_commands),
        ("router-settings", Ui::open_router_settings),
        ("router-mark-visible-read", Ui::router_mark_visible_read),
        ("favorites", Ui::open_favorites),
        ("annotations", Ui::open_annotations),
        ("find-conversation", Ui::find_conversation),
        ("history", Ui::open_history_pages),
        ("open-document", Ui::open_document),
        ("open-file", Ui::choose_document),
        ("terminal", Ui::open_terminal),
        ("files", Ui::open_files),
        ("browser", Ui::open_browser),
        ("session-map", Ui::open_session_map),
        ("resources", Ui::open_resources),
        ("session-info", Ui::open_session_info),
        ("connections", Ui::open_connections),
        ("git", Ui::open_git),
        ("compare", Ui::open_compare),
        ("rename-session", Ui::rename_session),
        ("fork-session", Ui::fork_session),
        ("archive-session", Ui::archive_session),
        ("restore-session", Ui::restore_session),
        ("delete-session", Ui::delete_session),
    ] {
        let action = gio::SimpleAction::new(name, None);
        let weak = Rc::downgrade(ui);
        action.connect_activate(move |_, _| {
            if let Some(ui) = weak.upgrade() {
                handler(&ui);
            }
        });
        ui.window.add_action(&action);
    }
    for (name, scheme) in [
        ("theme-system", adw::ColorScheme::Default),
        ("theme-light", adw::ColorScheme::ForceLight),
        ("theme-dark", adw::ColorScheme::ForceDark),
    ] {
        let action = gio::SimpleAction::new(name, None);
        let weak = Rc::downgrade(ui);
        action.connect_activate(move |_, _| {
            adw::StyleManager::default().set_color_scheme(scheme);
            if let Some(ui)=weak.upgrade(){ui.command(Command::Preferences(json!({"theme":match scheme{adw::ColorScheme::ForceDark=>"dark",adw::ColorScheme::ForceLight=>"light",_=>"system"}})));}
        });
        ui.window.add_action(&action);
    }
    let weak = Rc::downgrade(ui);
    ui.backend.connect_selected_notify(move |dropdown| {
        if let Some(ui) = weak.upgrade() {
            let backend = ui
                .state
                .borrow()
                .backends
                .get(dropdown.selected() as usize)
                .cloned();
            if let Some(backend) = backend {
                ui.switch_backend(backend);
            }
        }
    });
    let weak = Rc::downgrade(ui);
    ui.selection.connect_selected_item_notify(move |selection| {
        if let Some(ui) = weak.upgrade() {
            if let Some(item) = selection
                .selected_item()
                .and_downcast::<glib::BoxedAnyObject>()
            {
                let thread = item.borrow::<Thread>().clone();
                ui.select(thread);
            }
        }
    });
    let weak = Rc::downgrade(ui);
    ui.search.connect_search_changed(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.render_sessions();
        }
    });
    let weak = Rc::downgrade(ui);
    ui.more.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.catalog(true);
        }
    });
    let weak = Rc::downgrade(ui);
    ui.send.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.submit();
        }
    });
    let weak = Rc::downgrade(ui);
    ui.stop.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.interrupt();
        }
    });
    let keys = gtk::EventControllerKey::new();
    let weak = Rc::downgrade(ui);
    keys.connect_key_pressed(move |_, key, _, modifiers| {
        if key == gtk::gdk::Key::Return && modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK)
        {
            if let Some(ui) = weak.upgrade() {
                if modifiers.contains(gtk::gdk::ModifierType::SHIFT_MASK) {
                    ui.start_continuation();
                } else {
                    ui.submit();
                }
            }
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    ui.composer.add_controller(keys);
    let sender = ui.sender.clone();
    let launch_control = ui.launch_control.clone();
    let weak = Rc::downgrade(ui);
    let unmap_owner = Rc::downgrade(ui);
    ui.window.connect_unmap(move |_| {
        if let Some(ui) = unmap_owner.upgrade() {
            ui.file_picker.cancel();
        }
    });
    ui.window.connect_close_request(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.file_picker.cancel();
            ui.image_viewer.cancel_pending_choices();
            if !ui.language.allow_window_close(){return glib::Propagation::Stop;}
            if ui.write_feedback.blocks_close() { return glib::Propagation::Stop; }
            if !ui.queue_may_close() { return glib::Propagation::Stop; }
            let information=ui.session_info.borrow().clone();
            if information.is_some_and(|p|!p.allow_window_close()){return glib::Propagation::Stop;}
            let router=ui.router_settings.borrow().clone();
            if router.is_some_and(|panel|!panel.allow_window_close()){return glib::Propagation::Stop;}
            let favorites = ui.favorites.borrow().clone();
            if let Some(favorites) = favorites.filter(|favorites| !favorites.allow_window_close()) {
                favorites.dialog.present(Some(&ui.window));
                ui.notice("Save or discard favorite changes before closing");
                return glib::Propagation::Stop;
            }
            let settings = ui.settings.borrow().clone();
            if let Some(settings) = settings.filter(|settings| !settings.may_close()) {
                settings.dialog.present(Some(&ui.window));
                ui.error("Save or discard settings changes before closing");
                return glib::Propagation::Stop;
            }
            if let Some(panel)=ui.annotations.borrow().clone(){if !panel.allow_window_close(){return glib::Propagation::Stop;}}
            if !ui.output.allow_window_close() {
                ui.write_feedback.waiting("output","Wait for the output export to finish before closing");
                return glib::Propagation::Stop;
            }
            if !ui.source.allow_window_close() { return glib::Propagation::Stop; }
            if !ui.reader.allow_window_close() {
                return glib::Propagation::Stop;
            }
            if !ui.image_viewer.allow_window_close(){return glib::Propagation::Stop;}
            if !ui.git.allow_window_close() {
                return glib::Propagation::Stop;
            }
            if ui.state.borrow().session_mutation{ui.error("Wait for the session change before closing");return glib::Propagation::Stop;}
            if !ui.translation.allow_window_close(){return glib::Propagation::Stop;}
            if !ui.message_flow.allow_window_close(){return glib::Propagation::Stop;}
            if !ui.epub.allow_window_close(){return glib::Propagation::Stop;}
            if !ui.browser.allow_window_close(){return glib::Propagation::Stop;}
            ui.recovery.borrow_mut().stop();
            ui.notifications.flush();
            let maximized=ui.window.is_maximized();
            let (width,height)=if maximized {(ui.window.default_width(),ui.window.default_height())}else{(ui.window.width(),ui.window.height())};
            ui.command(Command::Preferences(json!({"windowWidth":width.clamp(640,8192),"windowHeight":height.clamp(480,8192),"maximized":maximized})));
        }
        launch_control.send_replace(crate::gateway_launch::Control::Close);
        let _ = sender.try_send(Command::Shutdown);
        glib::Propagation::Proceed
    });
    if let Some(app) = ui.window.application() {
        app.set_accels_for_action("win.new-session", &["<Control>n"]);
        app.set_accels_for_action("win.open-file", &["<Control>o"]);
        app.set_accels_for_action("win.refresh", &["<Control>r"]);
        app.set_accels_for_action("win.settings", &["<Control>comma"]);
        app.set_accels_for_action("win.commands", &["<Control><Shift>p"]);
        app.set_accels_for_action("win.find-conversation", &["<Control><Shift>f"]);
        app.set_accels_for_action("win.history", &["<Control><Shift>h"]);
    }
}

impl Ui {
    fn reset_search_context(&self) {
        self.transcript_navigation.reset();
        self.turn_navigation.reset();
        self.finder.clear();
        self.command(Command::ConversationFocus(None));
        let previous = self.history_context.borrow_mut().take();
        if let Some(previous) = previous {
            previous.dialog.force_close();
        }
    }
    fn open_history_context(self: &Rc<Self>, hit: crate::session_search::Occurrence) {
        let target = {
            let mut state = self.state.borrow_mut();
            state.sequence += 1;
            state.selected.as_ref().map(|thread| {
                (
                    crate::session_search::Context {
                        backend: state.backend.clone(),
                        thread: thread.id.clone(),
                        generation: state.socket_generation,
                    },
                    state.sequence,
                    thread.title.clone(),
                    thread.cwd.clone(),
                )
            })
        };
        let Some((context, serial, title, root)) = target else {
            return;
        };
        self.present_history_context(context, serial, title, root, hit);
    }
    fn present_history_context(
        self: &Rc<Self>,
        context: crate::session_search::Context,
        serial: u64,
        title: String,
        root: String,
        hit: crate::session_search::Occurrence,
    ) {
        let directory = root.clone();
        let backend_kind = self
            .state
            .borrow()
            .backends
            .iter()
            .find(|backend| backend.id == context.backend)
            .map(|backend| backend.kind.clone())
            .unwrap_or_default();
        let previous = self.history_context.borrow_mut().take();
        if let Some(previous) = previous {
            previous.dialog.force_close();
        }
        let weak = Rc::downgrade(self);
        let image_root = root.clone();
        let open: crate::content_widgets::OpenLink = Rc::new(move |reference| {
            if let Some(ui) = weak.upgrade() {
                let dialog = ui
                    .history_context
                    .borrow()
                    .as_ref()
                    .map(|view| view.dialog.clone());
                if let Some(dialog) = dialog {
                    dialog.close();
                }
                ui.activate_reference_from(reference, &root);
            }
        });
        let view = crate::conversation_context_ui::View::new(
            &self.window,
            context.clone(),
            serial,
            &title,
            self.document_sender.clone(),
            open,
            Some({
                let images = self.images.clone();
                Rc::new(move |_, content, open| {
                    crate::images_ui::chat(&images, content, &image_root, open)
                })
            }),
        );
        let weak = Rc::downgrade(self);
        view.dialog.connect_closed(move |_| {
            if let Some(ui) = weak.upgrade() {
                let current = ui
                    .history_context
                    .borrow()
                    .as_ref()
                    .is_some_and(|view| view.serial == serial);
                if current {
                    ui.history_context.borrow_mut().take();
                    ui.command(Command::ConversationFocus(None));
                }
            }
        });
        self.history_context.replace(Some(view.clone()));
        if !self.command(Command::ConversationFocus(Some(
            crate::conversation_focus::Request {
                backend_kind,
                directory,
                context,
                serial,
                hit,
            },
        ))) {
            view.apply(Err("History retrieval could not be started".into()));
        }
    }
    fn find_conversation(self: &Rc<Self>) {
        if self.router_selected() {
            self.finder.close();
            self.router_bar.search_toggle.set_active(true);
            self.router_bar.search.grab_focus();
            self.router_bar.search.select_region(0, -1);
        } else {
            self.finder.open();
        }
    }
    fn open_favorites(self: &Rc<Self>) {
        self.favorites_library();
    }
    fn favorites_library(self: &Rc<Self>) -> Rc<crate::favorites_ui::Favorites> {
        let current = self.favorites.borrow().clone();
        if let Some(current) = current {
            current.dialog.present(Some(&self.window));
            return current;
        }
        let id = {
            let mut state = self.state.borrow_mut();
            state.sequence += 1;
            state.sequence
        };
        let weak = Rc::downgrade(self);
        let owner: Rc<RefCell<std::rc::Weak<crate::favorites_ui::Favorites>>> =
            Rc::new(RefCell::new(std::rc::Weak::new()));
        let link_owner = owner.clone();
        let open: crate::content_widgets::OpenLink = Rc::new(move |reference| {
            if let (Some(ui), Some(favorites)) = (weak.upgrade(), link_owner.borrow().upgrade()) {
                if favorites.busy() {
                    ui.notice("Save or discard favorite changes before opening a link");
                    return;
                }
                let root = favorites.project_path();
                favorites.dialog.close();
                ui.activate_reference_from(reference, &root);
            }
        });
        let favorites = crate::favorites_ui::Favorites::new(
            &self.window,
            self.write_feedback.clone(),
            id,
            self.sender.clone(),
            self.document_sender.clone(),
            open,
            Some({
                let owner = owner.clone();
                let images = self.images.clone();
                Rc::new(move |_, content, open| {
                    let root = owner
                        .borrow()
                        .upgrade()
                        .map(|favorites| favorites.project_path())
                        .unwrap_or_default();
                    crate::images_ui::chat(&images, content, &root, open)
                })
            }),
        );
        *owner.borrow_mut() = Rc::downgrade(&favorites);
        let weak = Rc::downgrade(self);
        *favorites.on_source.borrow_mut() = Some(Box::new(move |favorite| {
            if let Some(ui) = weak.upgrade() {
                ui.open_favorite_source(favorite);
            }
        }));
        let weak = Rc::downgrade(self);
        favorites.dialog.connect_closed(move |_| {
            if let Some(ui) = weak.upgrade() {
                let current = ui
                    .favorites
                    .borrow()
                    .as_ref()
                    .is_some_and(|favorites| favorites.id == id);
                if current {
                    ui.favorites.borrow_mut().take();
                }
            }
        });
        self.favorites.replace(Some(favorites.clone()));
        favorites
    }
    fn save_favorite(self: &Rc<Self>, item: Arc<Item>, selection: Option<String>) {
        let source = {
            let state = self.state.borrow();
            state
                .selected
                .as_ref()
                .filter(|_| {
                    !state.loading
                        && state
                            .items
                            .iter()
                            .any(|visible| Arc::ptr_eq(visible, &item))
                        && state.active_turn.as_deref() != Some(item.turn_id.as_str())
                })
                .map(|thread| {
                    (
                        state.backend.clone(),
                        thread.clone(),
                        state
                            .items
                            .iter()
                            .find(|question| {
                                question.role == "You" && question.turn_id == item.turn_id
                            })
                            .map(|question| question.body.clone())
                            .unwrap_or_default(),
                    )
                })
        };
        let Some((mut backend, mut thread, mut question)) = source else {
            self.error("Wait for the response to finish loading before saving it");
            return;
        };
        let (turn_id, item_id) = if let Some(origin) = &item.origin {
            backend = origin.backend.clone();
            thread.id = origin.thread.clone();
            thread.title = origin.title.clone();
            thread.cwd = origin.root.clone();
            question = origin.question.as_ref().clone();
            (origin.turn.clone(), origin.item.clone())
        } else {
            (item.turn_id.clone(), item.id.clone())
        };
        if turn_id.is_empty() || item_id.is_empty() {
            self.error("This response has no persistent source anchor");
            return;
        }
        let library = self.favorites_library();
        if library.busy() {
            self.notice("Save or discard the current favorite before adding another");
            return;
        }
        let excerpt = item.origin.as_ref().is_some_and(|origin| origin.excerpt)
            || item
                .body
                .ends_with("… Output truncated in the native view.")
            || item.body.starts_with("[Excerpt around the selected match]");
        let selected = selection.is_some() || excerpt;
        let content = selection.unwrap_or_else(|| item.body.clone());
        if content.trim().is_empty() {
            self.error("Select non-empty response text");
            return;
        }
        let favorite = crate::favorites::Favorite {
            id: glib::uuid_string_random().to_string(),
            scope: if selected { "selection" } else { "message" }.into(),
            backend,
            thread_id: thread.id,
            thread_title: thread.title,
            project_path: thread.cwd,
            turn_id,
            item_id,
            title: crate::favorites::title(&content),
            question,
            content,
            note: if excerpt {
                "Saved from the excerpt displayed in the native view."
            } else {
                ""
            }
            .into(),
            tags: Vec::new(),
            created_at: glib::DateTime::now_utc()
                .and_then(|date| date.format_iso8601())
                .map(|value| value.to_string())
                .unwrap_or_else(|_| "unknown".into()),
        };
        library.draft(favorite);
    }
    fn open_favorite_source(self: &Rc<Self>, favorite: crate::favorites::Favorite) {
        let target = {
            let mut state = self.state.borrow_mut();
            let kind = state
                .backends
                .iter()
                .find(|backend| backend.id == favorite.backend)
                .map(|backend| backend.kind.clone());
            state.sequence += 1;
            (
                kind,
                state.sequence,
                *state
                    .backend_generations
                    .get(&favorite.backend)
                    .unwrap_or(&0),
            )
        };
        let snippet: String = favorite.content.chars().take(160).collect();
        let highlight = 0..snippet.len();
        let hit = crate::session_search::Occurrence {
            turn_id: favorite.turn_id.clone(),
            item_id: favorite.item_id.clone(),
            kind: crate::session_search::Filter::Assistant,
            snippet,
            highlight,
            source: None,
            block: None,
            table_row: None,
            paragraph_page: None,
            paragraph_range: None,
        };
        if matches!(target.0.as_deref(), Some("codex" | "opencode")) {
            self.present_history_context(
                crate::session_search::Context {
                    backend: favorite.backend,
                    thread: favorite.thread_id,
                    generation: target.2,
                },
                target.1,
                favorite.thread_title,
                favorite.project_path,
                hit,
            );
        } else {
            self.notice("The favorite's backend is no longer configured");
        }
    }
    fn open_annotations(self: &Rc<Self>) {
        self.comment_panel(None);
    }
    fn comment_reply(self: &Rc<Self>, item: Arc<Item>, selection: Option<String>) {
        let historical = self.history_pages.items();
        let owner = {
            let state = self.state.borrow();
            let displayed = historical.as_deref().unwrap_or(&state.items);
            state
                .selected
                .as_ref()
                .filter(|_| {
                    !state.loading && displayed.iter().any(|visible| Arc::ptr_eq(visible, &item))
                })
                .map(|thread| format!("{}:{}", state.backend, thread.id))
        };
        let result = owner
            .ok_or("The selected reply is no longer in the current conversation")
            .and_then(|owner| crate::annotations::chat_capture(&item, &owner, selection));
        match result {
            Ok(capture) => self.comment_panel(Some(capture)),
            Err(error) => self.error(&crate::language::tr(error)),
        }
    }
    fn comment_panel(self: &Rc<Self>, capture: Option<crate::annotations::Capture>) {
        let (key, title, id, project) = {
            let mut state = self.state.borrow_mut();
            let Some(thread) = state.selected.as_ref() else {
                drop(state);
                self.notice("Select a session before adding comments");
                return;
            };
            let key = format!("{}:{}", state.backend, thread.id);
            let title = thread.title.clone();
            let project = thread.cwd.clone();
            state.sequence += 1;
            (key, title, state.sequence, project)
        };
        if let Some(panel) = self.annotations.borrow().clone() {
            if panel.key != key {
                self.notice("Close the current session's Comments panel first");
                return;
            }
            if let Some(capture) = capture {
                panel.add(capture);
            }
            panel.dialog.present(Some(&self.window));
            return;
        }
        let panel = crate::annotations_ui::Panel::new(
            &self.window,
            id,
            key.clone(),
            &title,
            self.sender.clone(),
            capture,
        );
        let weak = Rc::downgrade(self);
        panel.dialog.connect_closed(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.annotations.borrow_mut().take();
            }
        });
        let weak = Rc::downgrade(self);
        *panel.on_source.borrow_mut() = Some(Box::new(move |source, excerpt| {
            let ui = weak.upgrade().ok_or("Studio window is closed")?;
            match crate::annotations::reopen(&source)? {
                crate::annotations::Reopen::Document {
                    root, path, anchor, ..
                } => {
                    let root = if root.is_empty() {
                        project.clone()
                    } else {
                        root
                    };
                    let markdown = std::path::Path::new(&path)
                        .extension()
                        .and_then(|s| s.to_str())
                        .is_some_and(|s| {
                            matches!(
                                s.to_ascii_lowercase().as_str(),
                                "md" | "markdown" | "mdown" | "mkd"
                            )
                        });
                    if markdown {
                        ui.reader.open_comment(
                            std::path::Path::new(&root),
                            std::path::Path::new(&path),
                            anchor,
                            &excerpt,
                        )?;
                    } else {
                        ui.source.open_comment(&root, &path, anchor, &excerpt)?;
                    }
                }
                crate::annotations::Reopen::Epub(mut anchor) => {
                    if anchor.root.is_empty() {
                        anchor.root = project.clone();
                    }
                    ui.epub.open_comment(anchor, &excerpt);
                }
                crate::annotations::Reopen::Browser(url) => {
                    ui.browser.open_comment(&url, &excerpt)?;
                }
                crate::annotations::Reopen::Pdf(mut anchor) => {
                    if anchor.root.is_empty() {
                        anchor.root = project.clone();
                    }
                    ui.pdf.open_comment(anchor, &excerpt);
                }
                crate::annotations::Reopen::Table(mut anchor) => {
                    if anchor.root.is_empty() {
                        anchor.root = project.clone();
                    }
                    ui.tables.open_comment(anchor, &excerpt);
                }
            }
            Ok(())
        }));
        *panel.on_send.borrow_mut() = Some({
            let weak = Rc::downgrade(self);
            Box::new(move |ticket, prompt| {
                let ui = weak.upgrade().ok_or("Conversation is closed")?;
                if ui.composer_key().as_deref() != Some(ticket.key.as_str()) {
                    return Err("Return to this comment's session before sending".into());
                }
                if !ui.send.is_sensitive() {
                    return Err("The conversation is not ready to accept a message".into());
                }
                if ui.router_selected() && ui.router.borrow().pending_for(&ticket.key) {
                    return Err(
                        "Finish the existing Router request before sending these comments".into(),
                    );
                }
                let current = ui.composer_text();
                let current = current.trim();
                let message = if current == prompt || current.ends_with(&format!("\n\n{prompt}")) {
                    current.to_owned()
                } else if current.is_empty() {
                    prompt
                } else {
                    format!("{current}\n\n{prompt}")
                };
                if crate::command_tools::shell(&message).is_some()
                    || crate::command_tools::slash(&message).is_some()
                {
                    return Err(
                        "Remove the composer command prefix before sending comments as a message"
                            .into(),
                    );
                }
                ui.composer.buffer().set_text(&message);
                ui.annotation_prepared.replace(Some(ticket));
                ui.submit();
                if ui.annotation_prepared.borrow_mut().take().is_some() {
                    return Err("Comments were not sent; check the conversation state".into());
                }
                Ok(())
            })
        });
        let weak = Rc::downgrade(self);
        *panel.on_prompt.borrow_mut() = Some(Box::new(move |prompt| {
            let ui = weak.upgrade().ok_or("Composer is closed")?;
            if ui.composer_key().as_deref() != Some(key.as_str()) {
                return Err("Return to this comment's session before inserting its prompt".into());
            }
            let current = ui.composer_text();
            let current = current.trim();
            if current != prompt && !current.ends_with(&format!("\n\n{prompt}")) {
                ui.composer.buffer().set_text(&if current.is_empty() {
                    prompt
                } else {
                    format!("{current}\n\n{prompt}")
                });
            }
            Ok(())
        }));
        panel.dialog.present(Some(&self.window));
        self.annotations.replace(Some(panel));
    }
    fn annotation_result(&self, ticket: &Arc<crate::annotations::Submission>, accepted: bool) {
        let panel = self.annotations.borrow().clone();
        if let Some(panel) = panel.filter(|p| p.id == ticket.dialog && p.key == ticket.key) {
            panel.submission_result(ticket, accepted);
        }
    }
    fn annotations_disconnected(&self, backend: Option<&str>) {
        let tickets = self
            .state
            .borrow()
            .pending
            .values()
            .filter(|p| backend.is_none_or(|backend| p.backend == backend))
            .filter_map(|p| {
                if let Pending::Send { annotation, .. } = &p.action {
                    annotation.clone()
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        for ticket in tickets {
            self.annotation_result(&ticket, false);
        }
    }
    fn open_browser(self: &Rc<Self>) {
        self.browser.open(None);
    }
    fn open_settings(self: &Rc<Self>) {
        let existing = self.settings.borrow().clone();
        if let Some(settings) = existing {
            settings.dialog.present(Some(&self.window));
            return;
        }
        let root = self
            .state
            .borrow()
            .selected
            .as_ref()
            .map(|thread| thread.cwd.clone())
            .unwrap_or_default();
        let backend = self.state.borrow().backend.clone();
        let settings = crate::settings_ui::Settings::new(
            &self.window,
            self.sender.clone(),
            &root,
            &backend,
            self.appearance.clone(),
            self.notifications.clone(),
            self.language.clone(),
            self.message_flow.clone(),
        );
        let weak = Rc::downgrade(self);
        *settings.on_visibility_changed.borrow_mut() = Some(Box::new(move |reply| {
            if let Some(ui) = weak.upgrade() {
                ui.apply_visibility(reply);
            }
        }));
        let weak = Rc::downgrade(self);
        *settings.on_shared_changed.borrow_mut() = Some(Box::new(move || {
            if let Some(ui) = weak.upgrade() {
                ui.files.request_roots();
                ui.resources.refresh();
            }
        }));
        let weak = Rc::downgrade(self);
        *settings.on_browser_changed.borrow_mut() = Some(Box::new(move |values, restore| {
            if let Some(ui) = weak.upgrade() {
                ui.browser.apply_navigation(values, restore);
            }
        }));
        let weak = Rc::downgrade(self);
        settings.dialog.connect_closed(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.settings.borrow_mut().take();
            }
        });
        settings.dialog.present(Some(&self.window));
        self.settings.replace(Some(settings));
    }
    fn open_local_file(self: &Rc<Self>, path: &std::path::Path) {
        self.file_picker.cancel();
        if matches!(
            path.extension()
                .and_then(|extension| extension.to_str())
                .unwrap_or("")
                .to_ascii_lowercase()
                .as_str(),
            "md" | "markdown" | "mdown"
        ) {
            self.reader.open_path(path);
        } else if let Some(parent) = path.parent() {
            if crate::epub::supports(path) {
                self.epub
                    .open(&parent.to_string_lossy(), &path.to_string_lossy());

                return;
            }
            if crate::pdf::supports(path) {
                self.pdf
                    .open(&parent.to_string_lossy(), &path.to_string_lossy(), 0);

                return;
            }
            if crate::images::supports(path) {
                self.image_viewer
                    .open(&parent.to_string_lossy(), &path.to_string_lossy());

                return;
            }
            if crate::tables::supports(path) {
                self.tables
                    .open(&parent.to_string_lossy(), &path.to_string_lossy());

                return;
            }
            self.source
                .open(&parent.to_string_lossy(), &path.to_string_lossy());
            self.show_tool("source");
        }
    }
    fn stash_current(&self) {
        self.remember_draft();
        let mut state = self.state.borrow_mut();
        let Some(thread) = state.selected.clone() else {
            return;
        };
        let backend = state.backend.clone();
        let key = format!("{backend}:{}", thread.id);
        state.last_selected.insert(backend.clone(), thread.clone());
        let view = crate::conversation_cache::View {
            backend,
            thread,
            generation: state.socket_generation,
            history_generation: state.history_generation,
            history_changes: std::mem::take(&mut state.history_changes),
            history_verified: state.history_verified,
            items: std::mem::take(&mut state.items),
            active_turn: state.active_turn.clone(),
            loading: state.loading,
            sending: state.sending,
            valid: state.ready && !state.loading && !state.sending && state.active_turn.is_none(),
            session_model: state.session_model.clone(),
            session_tier: state.session_tier.clone(),
        };
        state.cache.0.insert(key, view);
    }

    fn inactive_response(
        &self,
        backend: &str,
        pending: Pending,
        payload: &Value,
        prepared: protocol::Prepared,
    ) {
        if let Pending::Operation {
            thread,
            name,
            original,
        } = &pending
        {
            self.operation_response(
                backend,
                thread,
                name,
                original.clone(),
                payload,
                prepared,
                false,
            );
            return;
        }
        if let Pending::Send { thread, .. } = &pending {
            self.complete_manual_reply(backend, thread, payload);
        }
        let mut state = self.state.borrow_mut();
        match pending {
            Pending::Send {
                thread,
                text,
                images,
                references,
                information,
                ..
            } => {
                let key = format!("{backend}:{thread}");
                if payload.get("error").is_none() {
                    state.references.remove(&key, &references);
                }
                state
                    .images
                    .acknowledged(&key, &images, payload.get("error").is_none());
                if information.is_none()
                    && payload.get("error").is_none()
                    && state.drafts.get(&key) == Some(&text)
                {
                    state.drafts.remove(&key);
                }
                state.cache.0.update(&key, |view| {
                    view.sending = false;
                    view.valid = false;
                });
                if payload.get("error").is_some() {
                    if let Some(entry) = state.queues.entries.get_mut(&key) {
                        entry.pause("Send failed. Check before resuming queued messages.");
                    }
                }
                if let Some(error) = payload.get("error") {
                    let message = error["message"].as_str().unwrap_or("Send failed");
                    self.toast
                        .add_toast(adw::Toast::new(&format!("{backend}: {message}")));
                }
            }
            Pending::Select(thread, generation) => {
                let key = format!("{backend}:{thread}");
                if payload.get("error").is_some() {
                    if let Some(entry) = state.queues.entries.get_mut(&key) {
                        entry.pause("History could not be verified. Refresh the conversation before resuming.");
                    }
                }
                let reconciled = state.cache.0.update(&key, move |view| {
                    if view.history_generation != generation {
                        return None;
                    }
                    view.loading = false;
                    view.valid = false;
                    view.history_verified = false;
                    if payload.get("error").is_none() {
                        let (items, lifecycle) = crate::history_state::merge(
                            prepared.history.unwrap_or_default(),
                            &view.items,
                            crate::history_state::Lifecycle {
                                active_turn: prepared.active_turn,
                                active_shell: prepared.active_shell,
                            },
                            view.active_turn.as_deref(),
                            &view.history_changes,
                        );
                        view.items = items;
                        view.active_turn = lifecycle.active_turn;
                        view.history_changes.settled();
                        view.history_verified = true;
                        if let Some(thread) = prepared
                            .thread_summary
                            .as_ref()
                            .and_then(|summary| summary.as_ref().ok())
                            .map(|agent| agent.thread.clone())
                        {
                            view.thread = thread;
                        }
                        return Some(view.active_turn.is_some());
                    }
                    None
                });
                let verified = Self::history_ready(&state, &key);
                if let Some(entry) = state.queues.entries.get_mut(&key) {
                    match reconciled {
                        Some(Some(active)) if verified => {
                            entry.connected = true;
                            if entry.flight.is_none() {
                                entry.active = active;
                                entry.released = !active;
                            }
                        }
                        None => entry.pause(
                            "Conversation was evicted while loading. Reopen it before resuming.",
                        ),
                        _ => {}
                    }
                }
                state.queues.dirty = true;
            }
            _ => {}
        }
    }
    fn load_options(&self) {
        let (key, request) = {
            let mut state = self.state.borrow_mut();
            let Some(thread) = &state.selected else {
                return;
            };
            let key = format!("{}:{}", state.backend, thread.id);
            state.options_request += 1;
            state.options_loaded = false;
            state.options = crate::turn_options::Options::default();
            (key, state.options_request)
        };
        self.command(Command::SessionOptions {
            key,
            request,
            options: None,
        });
        self.update_composer();
    }
    fn apply_options(&self) {
        let (model, effort, efforts, missing) = {
            let mut state = self.state.borrow_mut();
            state.updating_options = true;
            let missing = if !state.options.model.is_empty()
                && !state.models.contains(&state.options.model)
            {
                let model = state.options.model.clone();
                state.models.push(model.clone());
                Some(model)
            } else {
                None
            };
            let model = state
                .models
                .iter()
                .position(|model| model == &state.options.model)
                .map_or(0, |index| index + 1);
            let effective = if !state.options.model.is_empty() {
                &state.options.model
            } else if !state.session_model.is_empty() {
                &state.session_model
            } else {
                &state.default_model
            };
            let mut efforts = vec![String::new()];
            efforts.extend(
                state
                    .model_efforts
                    .get(effective)
                    .cloned()
                    .unwrap_or_else(|| {
                        vec!["low".into(), "medium".into(), "high".into(), "xhigh".into()]
                    }),
            );
            if !state.options.effort.is_empty() && !efforts.contains(&state.options.effort) {
                efforts.push(state.options.effort.clone());
            }
            let effort = efforts
                .iter()
                .position(|effort| effort == &state.options.effort)
                .unwrap_or(0);
            state.efforts = efforts.clone();
            (model, effort, efforts, missing)
        };
        let labels = efforts
            .iter()
            .map(|effort| {
                if effort.is_empty() {
                    "Default effort"
                } else {
                    effort.as_str()
                }
            })
            .collect::<Vec<_>>();
        if let Some(missing) = missing {
            if let Some(models) = self.model.model().and_downcast::<gtk::StringList>() {
                models.append(&missing);
            }
        }
        if self.model.selected() != model as u32 {
            self.model.set_selected(model as u32);
        }
        let same =
            self.effort
                .model()
                .and_downcast::<gtk::StringList>()
                .is_some_and(|model| {
                    model.n_items() as usize == labels.len()
                        && labels.iter().enumerate().all(|(index, label)| {
                            model.string(index as u32).as_deref() == Some(*label)
                        })
                });
        if !same {
            self.effort.set_model(Some(&gtk::StringList::new(&labels)));
        }
        if self.effort.selected() != effort as u32 {
            self.effort.set_selected(effort as u32);
        }
        self.state.borrow_mut().updating_options = false;
        self.update_composer();
    }
    fn save_options(&self, fast_changed: bool) {
        let (key, options) = {
            let mut state = self.state.borrow_mut();
            if state.updating_options || !state.options_loaded || state.loading || state.archived {
                return;
            }
            let Some(thread) = &state.selected else {
                return;
            };
            let key = format!("{}:{}", state.backend, thread.id);
            let model = if self.model.selected() == 0 {
                String::new()
            } else {
                state
                    .models
                    .get(self.model.selected().saturating_sub(1) as usize)
                    .cloned()
                    .unwrap_or_default()
            };
            let effort = state
                .efforts
                .get(self.effort.selected() as usize)
                .cloned()
                .unwrap_or_default();
            let effective = if !model.is_empty() {
                &model
            } else if !state.session_model.is_empty() {
                &state.session_model
            } else {
                &state.default_model
            };
            let tier = if fast_changed {
                if self.fast.is_active() {
                    "fast".into()
                } else {
                    "default".into()
                }
            } else if self.fast.is_active() && !state.fast_models.contains(effective) {
                "default".into()
            } else {
                state.options.service_tier.clone()
            };
            let options = crate::turn_options::Options {
                model,
                effort,
                service_tier: tier,
            };
            if options == state.options {
                return;
            }
            state.options = options.clone();
            (key, options)
        };
        self.supervision_options_changed(&key, &options);
        self.command(Command::SessionOptions {
            key: key.clone(),
            request: 0,
            options: Some(options.clone()),
        });
        self.apply_options();
    }
    fn record_activity(&self, backend: &str, payload: &Value) {
        let method = model::text(payload, "method");
        if !matches!(
            method,
            "turn/started" | "turn/completed" | "thread/status/changed"
        ) {
            return;
        }
        let params = &payload["params"];
        let thread = model::text(params, "threadId");
        if thread.is_empty() || thread.len() > 256 {
            return;
        }
        let mut state = self.state.borrow_mut();
        let key = format!("{backend}:{thread}");
        if self.router_managed(&key) {
            return;
        }
        if state.activity.len() >= 8192 && !state.activity.contains_key(&key) {
            state
                .activity
                .retain(|_, activity| activity.active || activity.attention);
            if state.activity.len() >= 8192 {
                return;
            }
        }
        let selected = state.backend == backend
            && state
                .selected
                .as_ref()
                .is_some_and(|selected| selected.id == thread);
        let completed = state
            .activity
            .entry(key.clone())
            .or_default()
            .apply(method, params, selected);
        state.cache.0.update(&key, |view| view.valid = false);
        let visible = state.backend == backend;
        drop(state);
        if completed
            && !matches!(
                params.pointer("/turn/status").and_then(Value::as_str),
                Some("interrupted" | "cancelled" | "canceled")
            )
        {
            let body = self.notification_caption(backend, thread);
            self.notifications.completion(
                backend,
                thread,
                &body,
                params.pointer("/turn/status").and_then(Value::as_str) == Some("failed"),
            );
        }
        if visible {
            self.render_sessions();
        }
    }
    fn notification_caption(&self, backend: &str, thread: &str) -> String {
        let state = self.state.borrow();
        let name = state
            .backends
            .iter()
            .find(|b| b.id == backend)
            .map(|b| b.name.as_str())
            .unwrap_or("Agent");
        let title = if state.backend == backend {
            state
                .threads
                .iter()
                .find(|t| t.id == thread)
                .map(|t| t.title.as_str())
        } else {
            state
                .last_selected
                .get(backend)
                .filter(|t| t.id == thread)
                .map(|t| t.title.as_str())
        };
        format!("{name} · {}", title.unwrap_or("Agent session"))
    }
    fn session_target(&self) -> Option<(Thread, String, u64)> {
        let state = self.state.borrow();
        if !state.ready
            || !state.options_loaded
            || state.loading
            || !state.history_verified
            || state.sending
            || state.session_mutation
            || state.active_turn.is_some()
        {
            return None;
        }
        Some((
            state.selected.clone()?,
            state.backend.clone(),
            state.socket_generation,
        ))
    }

    fn change_session(
        &self,
        thread: &Thread,
        backend: &str,
        generation: u64,
        kind: &'static str,
        name: String,
    ) {
        {
            let state = self.state.borrow();
            if state.backend != backend
                || state.socket_generation != generation
                || !state.ready
                || state.session_mutation
            {
                self.error("The backend changed; reopen the session action");
                return;
            }
        }
        let method = match kind {
            "rename" => "thread/name/set",
            "fork" => "thread/fork",
            "archive" => "thread/archive",
            "restore" => "thread/unarchive",
            "delete" => "thread/delete",
            _ => return,
        };
        self.state.borrow_mut().session_mutation = true;
        self.remember_draft();
        let mut params = json!({"threadId":thread.id});
        if kind == "rename" {
            params["name"] = name.clone().into();
        }
        self.rpc(
            Pending::SessionMutation {
                thread: thread.id.clone(),
                kind,
                name,
            },
            method,
            params,
        );
        self.update_composer();
    }

    fn rename_session(self: &Rc<Self>) {
        let Some((thread, backend, generation)) = self.session_target() else {
            return;
        };
        let dialog = adw::MessageDialog::builder()
            .transient_for(&self.window)
            .heading("Rename session")
            .build();
        let entry = gtk::Entry::builder()
            .text(&thread.title)
            .max_length(200)
            .activates_default(true)
            .build();
        dialog.set_extra_child(Some(&entry));
        dialog.add_responses(&[("cancel", "Cancel"), ("rename", "Rename")]);
        dialog.set_close_response("cancel");
        dialog.set_default_response(Some("rename"));
        dialog.set_response_appearance("rename", adw::ResponseAppearance::Suggested);
        let weak = Rc::downgrade(self);
        dialog.connect_response(None, move |_, response| {
            if response == "rename" {
                if let Some(ui) = weak.upgrade() {
                    let name = entry.text().trim().to_owned();
                    if name.is_empty() {
                        ui.error("Session name cannot be empty");
                        return;
                    }
                    ui.change_session(&thread, &backend, generation, "rename", name);
                }
            }
        });
        dialog.present();
    }
    fn fork_session(self: &Rc<Self>) {
        if let Some((thread, backend, generation)) = self.session_target() {
            self.change_session(&thread, &backend, generation, "fork", String::new());
        }
    }
    fn archive_session(self: &Rc<Self>) {
        if let Some((thread, backend, generation)) = self.session_target() {
            self.change_session(&thread, &backend, generation, "archive", String::new());
        }
    }
    fn restore_session(self: &Rc<Self>) {
        if let Some((thread, backend, generation)) = self.session_target() {
            self.change_session(&thread, &backend, generation, "restore", String::new());
        }
    }
    fn delete_session(self: &Rc<Self>) {
        let Some((thread, backend, generation)) = self.session_target() else {
            return;
        };
        let dialog = adw::MessageDialog::builder()
            .transient_for(&self.window)
            .heading("Delete session permanently?")
            .body("The session and its stored history will be deleted. This cannot be undone.")
            .build();
        dialog.add_responses(&[("cancel", "Cancel"), ("delete", "Delete")]);
        dialog.set_close_response("cancel");
        dialog.set_default_response(Some("cancel"));
        dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        let weak = Rc::downgrade(self);
        dialog.connect_response(None, move |_, response| {
            if response == "delete" {
                if let Some(ui) = weak.upgrade() {
                    ui.change_session(&thread, &backend, generation, "delete", String::new());
                }
            }
        });
        dialog.present();
    }
    fn toggle_pin(&self) {
        let (key, pinned) = {
            let mut state = self.state.borrow_mut();
            if !state.pins_loaded || state.pin_pending {
                return;
            }
            let Some(thread) = &state.selected else {
                return;
            };
            let key = format!("{}:{}", state.backend, thread.id);
            let pinned = !state.pins.contains(&key);
            state.pin_pending = true;
            (key, pinned)
        };
        self.command(Command::Http {
            tag: "session-pin".into(),
            method: reqwest::Method::PUT,
            path: "/studio/session-state/pin".into(),
            body: Some(json!({"sessionKey":key,"pinned":pinned})),
            document: None,
        });
        self.update_composer();
    }
    fn activate_reference(self: &Rc<Self>, reference: &str) {
        let root = self
            .state
            .borrow()
            .selected
            .as_ref()
            .map(|thread| thread.cwd.clone());
        self.activate_reference_from(reference, root.as_deref().unwrap_or(""));
    }
    fn activate_reference_from(self: &Rc<Self>, reference: &str, root: &str) {
        if let Some(token) = reference
            .strip_prefix("studio-attachment:")
            .filter(|s| s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            self.image_viewer.open_inline(token);

            return;
        }
        let project = std::path::Path::new(root);
        match crate::links::resolve(reference, project, None) {
            Ok(crate::links::Target::External(uri)) => {
                if let Err(error) =
                    gio::AppInfo::launch_default_for_uri(&uri, gio::AppLaunchContext::NONE)
                {
                    self.error(&error.to_string());
                }
            }
            Ok(crate::links::Target::Document {
                path,
                line,
                heading,
            }) => {
                if root.is_empty() {
                    self.error("Select a project session before opening file links");
                    return;
                }
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
                    self.reader.open_project_path(
                        std::path::Path::new(&root),
                        &path,
                        line,
                        heading,
                    );
                } else if crate::epub::supports(&path) {
                    self.epub.open(&root, &path.to_string_lossy());
                } else if crate::pdf::supports(&path) {
                    let page = heading
                        .as_deref()
                        .and_then(|h| h.strip_prefix("page="))
                        .and_then(|p| p.parse::<usize>().ok())
                        .or(line)
                        .unwrap_or(1)
                        .saturating_sub(1);
                    self.pdf.open(&root, &path.to_string_lossy(), page);
                } else if line.is_none() && crate::images::supports(&path) {
                    self.image_viewer.open(&root, &path.to_string_lossy());
                } else if line.is_none() && crate::tables::supports(&path) {
                    self.tables.open(&root, &path.to_string_lossy());
                } else {
                    self.source
                        .open_at(&root, &path.to_string_lossy(), line.unwrap_or(1));
                    self.show_tool("source");
                }
            }
            Ok(crate::links::Target::Heading(_)) => {}
            Err(error) => self.error(&error),
        }
    }
    fn update_workspace(&self) {
        let width = self.toast.width().max(1);
        let document = self.tools.is_visible();
        // Give the document its own full pane on narrow windows. On wider
        // windows the divider remains available without squeezing controls.
        let minimum = if matches!(
            self.tools.visible_child_name().as_deref(),
            Some("git" | "compare" | "table" | "image" | "pdf" | "epub" | "browser")
        ) {
            1600
        } else {
            1100
        };
        self.conversation.set_visible(!document || width >= minimum);
        self.split
            .set_collapsed(width < 800 || (document && width < 1700));
        if document && self.split.is_collapsed() {
            self.split.set_show_sidebar(false);
        }
    }
    fn open_document(self: &Rc<Self>) {
        if self.tools.visible_child_name().as_deref() == Some("compare") {
            if let Some(callback) = self.compare.on_reader.borrow().as_ref() {
                callback();
            }
            return;
        }
        if self.reader.widget.is_visible() {
            self.show_tool("document");
        } else {
            self.choose_document();
        }
    }
    fn choose_document(self: &Rc<Self>) {
        let Some(cancel) = self.file_picker.begin() else {
            return;
        };
        let dialog = gtk::FileDialog::builder()
            .build()
            .ui_text("title", "Open document")
            .ui_text("accept-label", "Open");
        let filter = gtk::FileFilter::new().ui_text("name", "Documents, tables and images");
        for suffix in [
            "md", "markdown", "mdown", "csv", "tsv", "xlsx", "pdf", "epub", "png", "jpg", "jpeg",
            "gif", "webp", "svg",
        ] {
            filter.add_suffix(suffix);
        }
        // Source/Edit handles local UTF-8 text independently of its extension.
        let all = gtk::FileFilter::new().ui_text("name", "All files");
        all.add_pattern("*");
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        filters.append(&all);
        dialog.set_filters(Some(&filters));
        let weak = Rc::downgrade(self);
        let request = cancel.clone();
        dialog.open(Some(&self.window), Some(&cancel), move |result| {
            let Some(ui) = weak.upgrade() else { return };
            if !ui.file_picker.complete(&request) {
                return;
            }
            match result {
                Ok(file) => match file.path().filter(|p| p.to_str().is_some()) {
                    Some(path) => ui.open_local_file(&path),
                    None => ui.error(&crate::language::tr(
                        "Choose a local file with a UTF-8 path",
                    )),
                },
                Err(error)
                    if error.matches(gtk::DialogError::Dismissed)
                        || error.matches(gio::IOErrorEnum::Cancelled) => {}
                Err(error) => ui.error(&crate::language::format(
                    "Unable to open file chooser: {error}",
                    &[("error", &error.to_string())],
                )),
            }
        });
    }

    fn open_compare(self: &Rc<Self>) {
        self.compare.open(self.reader.location());
        self.show_tool("compare");
    }

    fn current_tool(&self) -> Option<crate::workspace_navigation::Pane> {
        self.tools
            .is_visible()
            .then(|| self.tools.visible_child_name())
            .flatten()
            .and_then(|name| crate::workspace_navigation::Pane::from_name(&name))
    }
    fn tool_available(&self, pane: crate::workspace_navigation::Pane) -> bool {
        use crate::workspace_navigation::Pane;
        match pane {
            Pane::Document => self.reader.location().is_some(),
            Pane::Source => self.source.has_document(),
            Pane::Image => self.image_viewer.has_document(),
            Pane::Pdf => self.pdf.has_document(),
            Pane::Epub => self.epub.has_document(),
            Pane::Table => self.tables.has_document(),
            // These remain useful workspaces even when no file/tab is loaded.
            Pane::Browser
            | Pane::Files
            | Pane::Git
            | Pane::Terminal
            | Pane::Compare
            | Pane::Resources
            | Pane::Map => true,
        }
    }
    fn present_viewer(&self, name: &str) {
        self.file_picker.cancel();
        let Some(pane) = crate::workspace_navigation::Pane::from_name(name) else {
            return;
        };
        self.tool_navigation
            .borrow_mut()
            .open(pane, self.current_tool());
        self.display_tool(name);
    }
    fn close_tool(&self, name: &str) {
        let Some(pane) = crate::workspace_navigation::Pane::from_name(name) else {
            return;
        };
        let decision =
            self.tool_navigation
                .borrow_mut()
                .close(pane, self.current_tool(), |parent| {
                    self.tool_available(parent)
                });
        match decision {
            crate::workspace_navigation::Closed::Ignore => {}
            crate::workspace_navigation::Closed::Hide => self.hide_tools(),
            crate::workspace_navigation::Closed::Show(parent) => self.display_tool(parent.name()),
        }
    }
    fn hide_tools(&self) {
        self.file_picker.cancel();
        self.tool_navigation.borrow_mut().reset();
        self.tools.set_visible(false);
        self.update_workspace();
    }
    fn show_tool(&self, name: &str) {
        self.file_picker.cancel();
        let Some(pane) = crate::workspace_navigation::Pane::from_name(name) else {
            return;
        };
        if self.current_tool() != Some(pane) || self.tool_navigation.borrow().active() != Some(pane)
        {
            self.tool_navigation.borrow_mut().select(pane);
        }
        self.display_tool(name);
    }
    fn display_tool(&self, name: &str) {
        self.tools.set_visible_child_name(name);
        self.tools.set_visible(true);
        self.update_workspace();
    }

    fn open_terminal(self: &Rc<Self>) {
        if self.terminal.is_started() {
            self.show_tool("terminal");
            return;
        }
        let root = self
            .state
            .borrow()
            .selected
            .as_ref()
            .map(|thread| thread.cwd.clone())
            .filter(|root| !root.is_empty());
        if let Some(root) = root {
            self.show_tool("terminal");
            self.terminal.open(&root);
            return;
        }
        let dialog = gtk::FileDialog::builder()
            .title("Choose terminal project")
            .build();
        let weak = Rc::downgrade(self);
        dialog.select_folder(Some(&self.window), gio::Cancellable::NONE, move |result| {
            if let (Some(ui), Ok(folder)) = (weak.upgrade(), result) {
                if let Some(path) = folder.path() {
                    ui.show_tool("terminal");
                    ui.terminal.open(&path.to_string_lossy());
                }
            }
        });
    }

    fn open_files(self: &Rc<Self>) {
        self.show_tool("files");
        if let Some(root) = self
            .state
            .borrow()
            .selected
            .as_ref()
            .map(|thread| thread.cwd.clone())
            .filter(|root| !root.is_empty())
        {
            self.files.open(&root);
        } else if !self.files.has_root() {
            self.files.choose();
        }
    }
    fn open_git(self: &Rc<Self>) {
        if let Some(root) = self
            .state
            .borrow()
            .selected
            .as_ref()
            .map(|thread| thread.cwd.clone())
            .filter(|root| !root.is_empty())
        {
            self.git.open(&root);
            self.show_tool("git");
            return;
        }
        let dialog = gtk::FileDialog::builder()
            .title("Choose Git repository")
            .build();
        let weak = Rc::downgrade(self);
        dialog.select_folder(Some(&self.window), gio::Cancellable::NONE, move |result| {
            if let (Some(ui), Ok(folder)) = (weak.upgrade(), result) {
                if let Some(path) = folder.path() {
                    ui.git.open(&path.to_string_lossy());
                    ui.show_tool("git");
                }
            }
        });
    }
    fn command(&self, command: Command) -> bool {
        if self.sender.try_send(command).is_err() {
            self.error("The backend is busy or unavailable. Reconnect and try again.");
            false
        } else {
            true
        }
    }

    fn rpc(&self, action: Pending, method: &str, params: Value) {
        if matches!(
            method,
            "turn/start"
                | "turn/interrupt"
                | "thread/archive"
                | "thread/delete"
                | "thread/rollback"
                | "thread/compact/start"
                | "thread/shellCommand"
                | "review/start"
        ) {
            if let Some(thread) = params["threadId"].as_str() {
                let backend = self.state.borrow().backend.clone();
                self.supervision_human(&format!("{backend}:{thread}"));
            }
        }
        if matches!(method, "turn/start" | "turn/steer" | "turn/interrupt")
            && self.router_selected()
        {
            self.error("Use the Thread Router send and stop controls for this controller");
            return;
        }
        if matches!(action, Pending::Create) {
            self.state.borrow_mut().session_mutation = true;
            self.update_composer();
        }
        let id = {
            let mut state = self.state.borrow_mut();
            state.sequence += 1;
            state.sequence
        };
        let root = self
            .state
            .borrow()
            .selected
            .as_ref()
            .map(|thread| thread.cwd.clone())
            .filter(|root| !root.is_empty());
        let (target_backend, target_generation) = {
            let state = self.state.borrow();
            (state.backend.clone(), state.socket_generation)
        };
        if self.command(Command::Rpc {
            images: match &action {
                Pending::Send { images, .. } => self
                    .state
                    .borrow()
                    .images
                    .images(&self.composer_key().unwrap_or_default())
                    .into_iter()
                    .filter(|image| images.contains(&image.id))
                    .collect(),
                _ => Vec::new(),
            },
            backend: target_backend,
            generation: target_generation,
            id,
            method: method.into(),
            params,
            root,
        }) {
            let mut state = self.state.borrow_mut();
            let backend = state.backend.clone();
            let generation = state.socket_generation;
            if let Pending::Send { thread, images, .. } = &action {
                state.images.sending(&format!("{backend}:{thread}"), images);
            }
            state.pending.insert(
                id,
                PendingRequest {
                    backend,
                    generation,
                    action,
                },
            );
        } else {
            if let Pending::Send {
                information: Some(ticket),
                ..
            } = &action
            {
                self.information_ack(
                    *ticket,
                    Err("Session responsibility generation could not be sent".into()),
                );
            }
            if let Pending::Send {
                annotation: Some(ticket),
                ..
            } = &action
            {
                self.annotation_result(ticket, false);
            }

            if matches!(action, Pending::SessionMutation { .. } | Pending::Create) {
                self.state.borrow_mut().session_mutation = false;
            }
            if matches!(action, Pending::Catalog { .. }) {
                self.state.borrow_mut().catalog_loading = false;
            }
            if matches!(action, Pending::Send { .. }) {
                self.state.borrow_mut().sending = false;
            }
            if let Pending::Operation { thread, .. } = &action {
                self.state.borrow_mut().sending = false;
                let key = format!("{}:{thread}", self.state.borrow().backend);
                self.state.borrow_mut().shells.forget(&key);
                self.pause_selected_queue(
                    "Command was not sent; check the conversation before resuming",
                );
                self.read_session(thread, true);
            }
            if matches!(action, Pending::Select(..)) {
                self.state.borrow_mut().loading = false;
            }
            self.update_composer();
        }
    }

    fn error(&self, message: &str) {
        if self.gateway_stopped.get() {
            crate::i18n_ui::bind(
                &self.status,
                "label",
                "Gateway stopped. Use Reconnect to restart it.",
            );
        } else {
            crate::i18n_ui::literal(&self.status, "label", message);
        }
        self.toast_error(message);
    }

    fn toast_error(&self, message: &str) {
        let toast = adw::Toast::new(message);
        toast.set_use_markup(false);
        self.toast.add_toast(toast);
    }

    fn notice(&self, source: &'static str) {
        crate::i18n_ui::bind(&self.status, "label", source);
        let toast = adw::Toast::new("").ui_text("title", source);
        toast.set_use_markup(false);
        self.toast.add_toast(toast);
    }

    fn composer_text(&self) -> String {
        let buffer = self.composer.buffer();
        buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .into()
    }

    fn remember_draft(&self) {
        let text = self.composer_text();
        let mut state = self.state.borrow_mut();
        if let Some(thread) = state
            .selected
            .as_ref()
            .map(|t| &t.id)
            .or(state.reconnecting.as_ref())
        {
            let key = format!("{}:{thread}", state.backend);
            state.drafts.insert(key, text);
        }
    }

    fn switch_backend(self: &Rc<Self>, backend: Backend) {
        self.connect_backend(backend, false);
    }

    fn connect_backend(self: &Rc<Self>, backend: Backend, force: bool) {
        self.connect_backend_to(backend, force, None);
    }
    fn connect_backend_to(self: &Rc<Self>, backend: Backend, force: bool, wanted: Option<Thread>) {
        self.connect_backend_attempt(backend, force, wanted, None);
    }
    fn connect_backend_attempt(
        self: &Rc<Self>,
        backend: Backend,
        force: bool,
        wanted: Option<Thread>,
        retry_from: Option<u64>,
    ) {
        if self.gateway_stopped.get() {
            self.notice("Gateway stopped. Use Reconnect to restart it.");
            return;
        }
        if self.state.borrow().session_mutation {
            self.error("Wait for the session change before switching backends");
            return;
        }
        if !force && self.state.borrow().backend == backend.id {
            return;
        }
        let reuse = !force && self.state.borrow().backend_ready.contains(&backend.id);
        if !reuse {
            self.supervision_backend_lost(&backend.id);
        }
        let preserve_composer = {
            let state = self.state.borrow();
            // Discovery can finish after the user has already started typing.
            state.backend.is_empty() || (force && state.backend == backend.id)
        };
        if !reuse {
            self.annotations_disconnected(Some(&backend.id));
        }
        self.stash_current();
        if backend.kind == "opencode" {
            self.scope.set_selected(0);
        }
        self.remember_draft();
        {
            let mut state = self.state.borrow_mut();
            state.restore_selection = if wanted.is_some() {
                wanted
            } else if force {
                state
                    .selected
                    .clone()
                    .or_else(|| state.restore_selection.clone())
            } else if let Some(thread) = state.last_selected.get(&backend.id) {
                Some(thread.clone())
            } else if state.saved_backend == backend.id && !state.saved_session.is_empty() {
                Some(Thread {
                    id: state.saved_session.clone(),
                    parent: None,
                    title: "Restoring session…".into(),
                    cwd: String::new(),
                    pinned: false,
                    activity: String::new(),
                })
            } else {
                None
            };
            if reuse {
                state.socket_generation = *state.backend_generations.get(&backend.id).unwrap();
            } else {
                state.connection_sequence += 1;
                state.socket_generation = state.connection_sequence;
                state.backend_ready.remove(&backend.id);
                state.shells.disconnected(Some(&backend.id));
                state.images.disconnected(Some(&backend.id));
                state.queues.pause_backend(
                    &backend.id,
                    "Connection changed. Check before resuming queued messages.",
                );
            }
            let generation = state.socket_generation;
            // Keep the editable draft owned while the catalog has no selected
            // row. A later selection must not replace text typed in this gap.
            state.reconnecting = if preserve_composer {
                state.restore_selection.as_ref().map(|t| t.id.clone())
            } else {
                None
            };
            state
                .backend_generations
                .insert(backend.id.clone(), generation);
            state.backend = backend.id.clone();
            state.ready = reuse;
            state.selected = None;
            state.history_verified = false;
            state.history_changes = Default::default();
            state.active_turn = None;
            if !reuse {
                state
                    .pending
                    .retain(|_, request| request.backend != backend.id);
            }
            if !reuse {
                state.cache.invalidate(&backend.id);
            }
            state.threads.clear();
            state.session_rows_serial += 1;
            state.items.clear();
            state.sending = false;
            state.loading = false;
            state.models.clear();
            state.fast_models.clear();
            state.default_model.clear();
            state.session_model.clear();
            state.session_tier.clear();
            state.options_loaded = false;
            state.updating_options = true;
        }
        if !reuse {
            self.expire_inputs(Some(&backend.id), None, None);
        }
        if !preserve_composer {
            self.composer.buffer().set_text("");
        }
        self.model
            .set_model(Some(&gtk::StringList::new(&["Default model"])));
        self.model.set_selected(0);
        self.state.borrow_mut().updating_options = false;
        self.reset_search_context();
        self.chat_diagrams.reset();
        self.transcript.remove_all();
        self.sessions.remove_all();
        self.status
            .set_text(&format!("Connecting to {}…", backend.name));
        let generation = self.state.borrow().socket_generation;
        let saved = self
            .state
            .borrow()
            .restore_selection
            .as_ref()
            .map(|thread| thread.id.clone())
            .unwrap_or_default();
        self.command(Command::Preferences(
            json!({"selectedBackend":backend.id,"selectedSession":saved}),
        ));
        if reuse {
            self.catalog(false);
            self.rpc(Pending::Models, "model/list", json!({"limit":100}));
        } else {
            self.begin_backend_connection(&backend.id, generation, retry_from);
            self.command(Command::Connect {
                backend,
                generation,
            });
        }
        self.update_composer();
    }

    fn reconnect(self: &Rc<Self>) {
        if self.gateway_stopped.get() {
            let generation = {
                let mut state = self.state.borrow_mut();
                state.startup_generation += 1;
                state.startup_ready = false;
                state.startup_generation
            };
            self.launch_control
                .send_replace(crate::gateway_launch::Control::Attempt(generation));
            self.notice("Starting gateway…");
            return;
        }
        self.reader.gateway_restored();
        self.compare.gateway_restored();
        self.tables.gateway_restored();
        self.pdf.gateway_restored();
        self.epub.gateway_restored();
        if !self.state.borrow().startup_ready {
            let generation = {
                let mut state = self.state.borrow_mut();
                state.startup_generation += 1;
                state.startup_generation
            };
            crate::i18n_ui::bind(&self.status, "label", "Loading backends…");
            self.launch_control
                .send_replace(crate::gateway_launch::Control::Attempt(generation));
            return;
        }
        let id = self.state.borrow().backend.clone();
        self.recovery.borrow_mut().forget(&id);
        let backend = {
            let state = self.state.borrow();
            state
                .backends
                .iter()
                .find(|b| b.id == state.backend)
                .cloned()
        };
        if let Some(backend) = backend {
            self.connect_backend(backend, true);
        }
    }

    fn catalog(&self, append: bool) {
        if !self.state.borrow().ready {
            return;
        }
        let generation = {
            let mut state = self.state.borrow_mut();
            if !append {
                state.catalog_generation += 1;
                state.catalog_loading = true;
            }
            state.catalog_generation
        };
        let mut params =
            json!({"limit":100, "sortKey":"updated_at", "archived":self.state.borrow().archived});
        if append {
            if let Some(cursor) = self.state.borrow().cursor.clone() {
                params["cursor"] = cursor.into();
            }
        }
        self.more.set_sensitive(false);
        self.rpc(
            Pending::Catalog { append, generation },
            "thread/list",
            params,
        );
        self.update_composer();
    }

    fn refresh(self: &Rc<Self>) {
        self.catalog(false);
        let selected = self.state.borrow().selected.clone();
        if let Some(thread) = selected {
            self.read_session(&thread.id, false);
        }
    }

    fn apply_visibility(&self, reply: crate::session_visibility::Reply) {
        {
            let mut state = self.state.borrow_mut();
            if state.visibility_revision == reply.snapshot.revision {
                return;
            }
            state.visibility_revision = reply.snapshot.revision;
            state.visibility = reply.filter.ok();
        }
        self.router.borrow_mut().next_poll = None;
        self.sessions.remove_all();
        self.render_sessions();
    }
    fn render_sessions(&self) {
        let mut state = self.state.borrow_mut();
        if state.threads.len() > crate::agents::MAX_AGENTS {
            state.threads.truncate(crate::agents::MAX_AGENTS);
            state.catalog_truncated = true;
            state.cursor = None;
        }
        state.session_rows_serial += 1;
        let serial = state.session_rows_serial;
        let Some(filter) = state.visibility.clone() else {
            drop(state);
            self.sessions.remove_all();
            crate::i18n_ui::bind(
                &self.status,
                "label",
                "Session visibility is unavailable; reload it in Settings",
            );
            return;
        };
        let request = crate::session_list::Request {
            serial,
            backend: state.backend.clone(),
            rows: Arc::new(
                state
                    .threads
                    .iter()
                    .map(|thread| crate::session_list::Row {
                        thread: thread.clone(),
                        status: state
                            .activity
                            .get(&format!("{}:{}", state.backend, thread.id))
                            .map(|a| crate::session_list::Status {
                                active: a.active,
                                attention: a.attention,
                                stale: a.stale,
                            }),
                    })
                    .collect(),
            ),
            pins: state.pins.clone(),
            pinned_only: state.pinned_only,
            scope: self.scope.selected(),
            query: self.search.text().to_string(),
            filter,
        };
        drop(state);
        self.session_rows_sender.send_replace(Some(request));
    }
    fn session_rows(self: &Rc<Self>, serial: u64, result: Result<Vec<Thread>, String>) {
        let state = self.state.borrow();
        if serial != state.session_rows_serial {
            return;
        }
        let threads = match result {
            Ok(rows) => rows,
            Err(e) => {
                drop(state);
                self.error(&e);
                return;
            }
        };
        let rows = threads
            .into_iter()
            .map(glib::BoxedAnyObject::new)
            .collect::<Vec<_>>();
        let more = state.cursor.is_some();
        let selected = state.selected.as_ref().and_then(|selected| {
            rows.iter()
                .position(|row| row.borrow::<Thread>().id == selected.id)
        });
        drop(state);
        self.sessions.splice(0, self.sessions.n_items(), &rows);
        if let Some(index) = selected {
            self.selection.set_selected(index as u32);
        }
        self.more.set_visible(more);
        self.more.set_sensitive(true);
    }

    fn select(self: &Rc<Self>, thread: Thread) {
        if self.gateway_stopped.get() {
            self.notice("Gateway stopped. Use Reconnect to restart it.");
            return;
        }
        if self.state.borrow().session_mutation {
            return;
        }
        if self
            .state
            .borrow()
            .selected
            .as_ref()
            .is_some_and(|t| t.id == thread.id)
        {
            return;
        }
        self.stash_current();
        self.remember_draft();
        let preserve_composer =
            self.state.borrow().reconnecting.as_deref() == Some(thread.id.as_str());
        let (draft, cached) = {
            let mut state = self.state.borrow_mut();
            state.reconnecting = None;
            let key = format!("{}:{}", state.backend, thread.id);
            if state.reference_selection.as_deref() != Some(key.as_str()) {
                state.reference_selection = None;
            }
            let cached = state.cache.0.remove(&key);
            let draft = state
                .drafts
                .get(&format!("{}:{}", state.backend, thread.id))
                .cloned()
                .unwrap_or_default();
            state.selected = Some(thread.clone());
            state.loading = true;
            state.history_verified = false;
            state.history_changes = Default::default();
            state.active_turn = None;
            state.items.clear();
            state.sending = false;
            let restored = cached.is_some();
            if let Some(view) = cached {
                state.sending = view.sending && view.generation == state.socket_generation;
                state.session_model = view.session_model;
                state.session_tier = view.session_tier;
                state.items = view.items;
                state.active_turn = if view.generation == state.socket_generation {
                    view.active_turn
                } else {
                    None
                };
                state.history_changes = view.history_changes;
                if view.generation != state.socket_generation {
                    state.history_changes.keep_active = false;
                }
                state.render_dirty = true;
            }
            (draft, restored)
        };
        if !preserve_composer || self.composer_text() != draft {
            self.composer.buffer().set_text(&draft);
        }
        self.command(Command::Preferences(
            json!({"selectedBackend":self.state.borrow().backend,"selectedSession":thread.id}),
        ));
        self.reset_search_context();
        self.chat_diagrams.reset();
        self.transcript.remove_all();
        self.title.set_title(&thread.title);
        crate::i18n_ui::literal(&self.title, "subtitle", &thread.cwd);
        if self.tools.is_visible()
            && self.tools.visible_child_name().as_deref() == Some("files")
            && !thread.cwd.is_empty()
        {
            self.files.open(&thread.cwd);
        }
        crate::i18n_ui::bind(
            &self.status,
            "label",
            if cached {
                "Restored from memory · Refreshing session…"
            } else {
                "Loading session…"
            },
        );
        self.update_composer();
        if self.split.is_collapsed() {
            self.split.set_show_sidebar(false);
        }
        let resume = !self.state.borrow().archived;
        self.read_session(&thread.id, resume);
    }

    fn read_session(&self, thread: &str, resume: bool) {
        if self.router.borrow().registry.is_none() {
            self.state.borrow_mut().loading = true;
            self.update_composer();
            return;
        }
        if self.router_selected() {
            if let Some(key) = self.composer_key() {
                self.router_focus(&key);
            }
            self.load_queue(false);
            self.load_options();
            self.router_render();
            self.router.borrow_mut().next_poll = None;
            let generation = {
                let mut state = self.state.borrow_mut();
                state.history_generation += 1;
                state.history_generation
            };
            self.rpc(
                Pending::Select(thread.into(), generation),
                "thread/read",
                json!({"threadId":thread,"includeTurns":false}),
            );
            return;
        }
        self.load_queue(false);
        self.load_options();
        let generation = {
            let mut state = self.state.borrow_mut();
            state.history_generation += 1;
            state.loading = true;
            state.history_verified = false;
            state.history_changes.begin();
            let key = format!("{}:{thread}", state.backend);
            if let Some(entry) = state.queues.entries.get_mut(&key) {
                entry.connected = false;
            }
            state.history_generation
        };
        let mut params = json!({"threadId":thread});
        if !resume {
            params["includeTurns"] = true.into();
        }
        self.rpc(
            Pending::Select(thread.into(), generation),
            if resume {
                "thread/resume"
            } else {
                "thread/read"
            },
            params,
        );
        self.update_composer();
    }

    fn update_composer(&self) {
        self.render_continuation();
        self.render_supervision();
        self.sync_session_info();
        self.sync_connections();
        self.sync_resources();
        self.translation.refresh_context();
        self.sync_history_pages();
        self.router_back_visibility();
        self.router_target_control();
        self.render_attachments();
        let key = self.composer_key().unwrap_or_default();
        self.composer_tools
            .render_refs(&key, &self.state.borrow().references);
        let obsolete = self
            .composer_tools
            .query
            .borrow()
            .as_ref()
            .is_some_and(|query| self.reference_context().as_ref() != Some(&query.context));
        if obsolete {
            self.composer_tools.close();
        }
        let state = self.state.borrow();
        self.pin
            .set_sensitive(state.pins_loaded && !state.pin_pending && state.selected.is_some());
        self.pin
            .set_active(state.selected.as_ref().is_some_and(|thread| {
                state
                    .pins
                    .contains(&format!("{}:{}", state.backend, thread.id))
            }));
        let open_code = state
            .backends
            .iter()
            .any(|backend| backend.id == state.backend && backend.kind == "opencode");
        let editable = state.ready
            && state.history_verified
            && !self.queue_busy(&state)
            && state.options_loaded
            && state.selected.is_some()
            && !state.session_mutation
            && !state.catalog_loading
            && !state.loading
            && !state.sending
            && state.active_turn.is_none();
        self.backend
            .set_sensitive(!state.session_mutation && !self.gateway_stopped.get());
        self.session_list
            .set_sensitive(!state.session_mutation && !self.gateway_stopped.get());
        self.scope
            .set_sensitive(!state.session_mutation && !self.gateway_stopped.get());
        for name in ["new-session", "refresh", "reconnect"] {
            if let Some(action) = self
                .window
                .lookup_action(name)
                .and_downcast::<gio::SimpleAction>()
            {
                action.set_enabled(
                    !state.session_mutation && (!self.gateway_stopped.get() || name == "reconnect"),
                );
            }
        }
        for name in [
            "rename-session",
            "fork-session",
            "archive-session",
            "restore-session",
            "delete-session",
        ] {
            if let Some(action) = self
                .window
                .lookup_action(name)
                .and_downcast::<gio::SimpleAction>()
            {
                action.set_enabled(
                    editable
                        && match name {
                            "archive-session" => !open_code && !state.archived,
                            "restore-session" => !open_code && state.archived,
                            "fork-session" => !state.archived,
                            _ => true,
                        },
                );
            }
        }
        self.effort.set_visible(!open_code);
        let controls = state.ready
            && state.history_verified
            && state.selected.is_some()
            && state.options_loaded
            && !state.loading
            && !state.archived
            && !state.session_mutation;
        self.model.set_sensitive(controls);
        self.effort.set_sensitive(controls);
        let effective_model = if !state.options.model.is_empty() {
            &state.options.model
        } else if !state.session_model.is_empty() {
            &state.session_model
        } else {
            &state.default_model
        };
        let enabled = state.options.effective_tier(&state.session_tier) == "fast";
        self.fast.set_visible(!open_code);
        self.fast.set_active(enabled);
        self.fast
            .set_sensitive(controls && (state.fast_models.contains(effective_model) || enabled));
        let images_loading = state.selected.as_ref().is_some_and(|thread| {
            state
                .images
                .pending
                .contains_key(&format!("{}:{}", state.backend, thread.id))
        });
        let composer_text = self.composer_text();
        let shell_command = crate::command_tools::shell(&composer_text);
        let shell_mode = shell_command.is_some();
        let shell_running = state
            .shells
            .contains(&self.composer_key().unwrap_or_default());
        self.shell_hint.set_visible(shell_mode || shell_running);
        crate::i18n_ui::bind(
            &self.shell_hint,
            "label",
            if open_code {
                "Shell commands run through OpenCode with its project permissions and the selected model."
            } else {
                "Shell commands use your local user access and do not inherit model sandbox permissions."
            },
        );
        self.send.set_sensitive(
            state.ready
                && state.history_verified
                && !shell_running
                && (!shell_mode
                    || (state.active_turn.is_none()
                        && shell_command.is_some_and(|command| !command.is_empty())))
                && !images_loading
                && !self.queue_busy(&state)
                && state.selected.is_some()
                && !state.sending
                && !state.loading
                && state.options_loaded
                && !state.archived
                && !state.session_mutation
                && !(open_code && state.active_turn.is_some()),
        );
        crate::i18n_ui::bind(
            &self.send,
            "label",
            if shell_running {
                "Running shell…"
            } else if shell_mode {
                "Run"
            } else if open_code && state.active_turn.is_some() {
                "Working…"
            } else if state.active_turn.is_some() {
                "Steer"
            } else {
                "Send"
            },
        );
        self.stop
            .set_sensitive(state.ready && state.active_turn.is_some());
        let key = state
            .selected
            .as_ref()
            .map(|thread| format!("{}:{}", state.backend, thread.id))
            .unwrap_or_default();
        let entry = state.queues.entries.get(&key);
        self.queue_button.set_sensitive(
            controls
                && !shell_mode
                && entry.is_some_and(|entry| {
                    entry.loaded && !entry.busy() && entry.messages.len() < entry.limit
                })
                && (state.active_turn.is_some()
                    || state.sending
                    || entry.is_some_and(|entry| !entry.messages.is_empty())),
        );
        self.queue.render(&key, entry);
        if self.router.borrow().registry.is_none() {
            self.send.set_sensitive(false);
        }
        self.router_bar.widget.set_visible(self.router_selected());
        if let Some(action) = self
            .window
            .lookup_action("router-mark-visible-read")
            .and_downcast::<gio::SimpleAction>()
        {
            action.set_enabled(self.router_selected());
        }
        if self.router_selected() {
            let router = self.router.borrow();
            let current = router
                .histories
                .iter()
                .find(|history| history.controller == key);
            let routing = current.is_some_and(|h| h.controller_busy);
            let active = current.is_some_and(|h| !h.active_ids.is_empty());
            let allowed =
                router.registry.as_ref().and_then(|r| r.active()).as_deref() == Some(key.as_str());
            crate::i18n_ui::bind(
                &self.send,
                "label",
                if router.submitting {
                    "Sending…"
                } else if router.pending_retry() {
                    "Retry delivery"
                } else {
                    "Send"
                },
            );
            self.send.set_sensitive(
                ((allowed && !routing) || router.pending_for(&key))
                    && !router.submitting
                    && (!self.router_files_preparing() || router.pending_for(&key))
                    && !images_loading
                    && (!self.router_inputs_waiting() || router.pending_for(&key))
                    && state.options_loaded
                    && !state.loading
                    && !state.archived
                    && !state.session_mutation,
            );
            self.stop.set_sensitive(active && !router.stopping);
            self.stop
                .set_tooltip_text(Some("Stop the latest routed request in this conversation"));
            self.queue_button.set_sensitive(
                allowed
                    && !self.router_files_preparing()
                    && !state.loading
                    && !state.archived
                    && !state.session_mutation
                    && !images_loading
                    && entry.is_some_and(|entry| {
                        entry.loaded && !entry.busy() && entry.messages.len() < entry.limit
                    })
                    && (active
                        || router.submitting
                        || entry.is_some_and(|entry| !entry.messages.is_empty())),
            );
        }
    }

    fn submit(self: &Rc<Self>) {
        if self.router_selected() {
            self.router_submit();
            return;
        }
        if self.router.borrow().registry.is_none() {
            self.error("Wait for Thread Router settings to load before sending");
            return;
        }
        let message = self.composer_text();
        if let Some(name) = crate::command_tools::slash(&message) {
            if let Err(error) = self.run_command(name, Some(message.clone())) {
                self.error(&error);
            }
            return;
        }
        if crate::command_tools::shell(&message).is_some() {
            if let Err(error) = self.start_operation("shell", Some(message)) {
                self.error(&error);
            }
            return;
        }
        let references = self
            .composer_key()
            .map(|key| self.state.borrow().references.get(&key))
            .unwrap_or_default();
        let images = self
            .composer_key()
            .map(|key| self.state.borrow().images.images(&key))
            .unwrap_or_default();
        if message.trim().is_empty() && images.is_empty() && references.is_empty() {
            return;
        }
        let (thread, active) = {
            let state = self.state.borrow();
            if !state.ready
                || self.queue_busy(&state)
                || state.sending
                || state.loading
                || !state.history_verified
                || !state.options_loaded
                || state.archived
                || state.session_mutation
                || state
                    .shells
                    .contains(&self.composer_key().unwrap_or_default())
                || state.selected.as_ref().is_some_and(|thread| {
                    state
                        .images
                        .pending
                        .contains_key(&format!("{}:{}", state.backend, thread.id))
                })
            {
                return;
            }
            let Some(thread) = &state.selected else {
                return;
            };
            (thread.id.clone(), state.active_turn.clone())
        };
        let mut params = json!({"threadId":thread,"input":[{"type":"text","text":message}]});
        params["input"]
            .as_array_mut()
            .unwrap()
            .extend(references.iter().map(|reference| reference.input.clone()));
        let method = if let Some(turn) = active {
            params["expectedTurnId"] = turn.into();
            "turn/steer"
        } else {
            let state = self.state.borrow();
            let codex = state
                .backends
                .iter()
                .any(|backend| backend.id == state.backend && backend.kind == "codex");
            state.options.apply(&mut params, codex);
            let root = state
                .selected
                .as_ref()
                .map(|thread| thread.cwd.as_str())
                .unwrap_or("");
            if let Err(error) = state.policies.apply(
                &format!("{}:{thread}", state.backend),
                root,
                codex,
                &mut params,
            ) {
                drop(state);
                self.error(&error);
                return;
            }
            "turn/start"
        };
        if self.history_pages.widget.is_visible() {
            self.history_pages.close();
        }
        self.state.borrow_mut().sending = true;
        self.queue_manual_start(&thread);
        self.update_composer();
        self.rpc(
            Pending::Send {
                information: None,
                annotation: self.annotation_prepared.borrow_mut().take(),
                thread,
                text: message,
                images: images.iter().map(|image| image.id.clone()).collect(),
                references: references
                    .iter()
                    .map(|reference| reference.id.clone())
                    .collect(),
            },
            method,
            params,
        );
    }

    fn interrupt(&self) {
        if let Some(key) = self.composer_key() {
            self.supervision_human(&key);
        }
        if self.router_selected() {
            self.router_stop();
            return;
        }
        self.pause_selected_queue("Stopped by user");
        let (thread, turn) = {
            let state = self.state.borrow();
            let (Some(thread), Some(turn)) = (&state.selected, &state.active_turn) else {
                return;
            };
            (thread.id.clone(), turn.clone())
        };
        self.rpc(
            Pending::Stop(thread.clone()),
            "turn/interrupt",
            json!({"threadId":thread,"turnId":turn}),
        );
    }

    fn event(self: &Rc<Self>, mut event: Event) {
        if self.gateway_stopped.get()
            && matches!(
                &event,
                Event::Backends(_)
                    | Event::Wire { .. }
                    | Event::BackendDisconnected { .. }
                    | Event::BackendError { .. }
                    | Event::MapSync { .. }
                    | Event::MapStoreChanged { .. }
                    | Event::SessionRows { .. }
                    | Event::FilesWatch { .. }
                    | Event::Terminal { .. }
            )
        {
            return;
        }
        if self.gateway_stopped.get() {
            let error = "Gateway stopped; the pending read was not applied".to_owned();
            match &mut event {
                Event::ConversationPage(r) => r.result = Err(error),
                Event::ConversationFocus(r) => r.result = Err(error),
                Event::SessionSearch(r) => r.result = Err(error),
                Event::SessionResources(r) => r.result = Err(error),
                Event::Streaming(r) => r.result = Err(error),
                Event::Connections { result, .. } => *result = Err(error),
                _ => {}
            }
        }
        match event {
            Event::TurnNavigation(response) => {
                let state = self.state.borrow();
                let current = state.backend == response.context.backend
                    && state.socket_generation == response.context.generation
                    && state
                        .selected
                        .as_ref()
                        .is_some_and(|t| t.id == response.context.thread);
                drop(state);
                if current {
                    self.turn_navigation.apply(response);
                }
            }
            Event::Supervision {
                backend,
                generation,
                update,
            } => {
                if self.state.borrow().backend_generations.get(&backend) != Some(&generation) {
                    return;
                }
                self.supervision_update(backend, generation, update);
            }
            Event::GatewayAvailable { generation } => {
                if generation != self.state.borrow().startup_generation {
                    return;
                }
                self.gateway_stopped.set(false);
                self.reader.gateway_restored();
                self.compare.gateway_restored();
                self.tables.gateway_restored();
                self.pdf.gateway_restored();
                self.epub.gateway_restored();
            }
            Event::Language { serial, result } => self.language.event(serial, result),
            Event::ConversationPage(response) => self.history_pages.apply(response),
            Event::ConversationOutput(response) => self.output.apply(response),
            Event::OutputExport {
                dialog,
                serial,
                result,
            } => {
                self.output.exported(dialog, serial, result);
                if !self.output.writing() {
                    self.write_feedback.finished("output");
                }
            }
            Event::NotificationSettings { serial, result } => {
                self.notifications.event(serial, result);
                self.router.borrow_mut().next_poll = None;
            }
            Event::SessionRows { serial, result } => self.session_rows(serial, result),
            Event::SessionVisibility {
                dialog,
                serial,
                result,
            } => {
                if dialog == 0 {
                    match result {
                        Ok(reply) => self.apply_visibility(reply),
                        Err(e) => self.error(&e),
                    }
                } else {
                    let settings = self.settings.borrow().clone();
                    if let Some(settings) = settings {
                        settings.visibility_event(dialog, serial, result);
                    }
                }
            }
            Event::SharedDirectories {
                dialog,
                serial,
                result,
            } => {
                let settings = self.settings.borrow().clone();
                if let Some(settings) = settings {
                    settings.shared_event(dialog, serial, result);
                }
            }
            Event::BackendConfig {
                dialog,
                serial,
                result,
            } => {
                let settings = self.settings.borrow().clone();
                if let Some(settings) = settings {
                    settings.backend_config_event(dialog, serial, result);
                }
            }
            Event::Speech { id, result } => self.translation.speech_event(id, result),
            Event::Translation { ticket, update } => self.translation.event(ticket, update),
            Event::Continuation { ticket, result } => self.continuation_result(ticket, result),
            Event::MessageFlow { serial, result } => self.message_flow.event(serial, result),
            Event::TranslationSettings {
                dialog,
                serial,
                result,
            } => {
                if let Some(settings) = self.settings.borrow().clone() {
                    settings.translation_event(dialog, serial, result);
                }
            }
            Event::BrowserTabs { serial, result } => self.browser.tabs_event(serial, result),
            Event::BrowserSettings {
                dialog,
                serial,
                result,
            } => {
                if let Some(settings) = self.settings.borrow().clone() {
                    settings.browser_event(dialog, serial, result);
                }
            }
            Event::Appearance {
                dialog,
                serial,
                result,
            } => {
                if dialog == 0 {
                    match result {
                        Ok(reply) => self.appearance.apply(&reply),
                        Err(error) => self.error(&error),
                    }
                } else if let Some(settings) = self.settings.borrow().clone() {
                    settings.appearance_event(dialog, serial, result);
                }
            }
            Event::Router { serial, result } => self.router_event(serial, result),
            Event::RouterConfig {
                dialog,
                serial,
                result,
            } => {
                let panel = self.router_settings.borrow().clone();
                self.router.borrow_mut().next_poll = None;
                if let Some(panel) = panel.filter(|panel| panel.id == dialog) {
                    panel.event(serial, result);
                }
            }
            Event::AgentTree {
                dialog,
                revision,
                result,
            } => self.agents_tree(dialog, revision, result),
            Event::CommandPalette {
                dialog,
                serial,
                result,
            } => {
                let palette = self.palette.borrow().clone();
                if let Some(palette) = palette {
                    palette.apply(dialog, serial, result);
                }
            }
            Event::FilterTools { serial, result } => self.composer_tools.filtered(serial, result),
            Event::Images {
                key,
                serial,
                result,
            } => {
                let result = self.state.borrow_mut().images.finish(&key, serial, result);
                if let Err(error) = result {
                    self.error(&error);
                }
                self.update_composer();
            }
            Event::Queue {
                key,
                serial,
                result,
            } => self.queue_result(key, serial, result),
            Event::Favorites {
                dialog,
                serial,
                result,
            } => {
                let favorites = self.favorites.borrow().clone();
                if let Some(favorites) = favorites.filter(|favorites| favorites.id == dialog) {
                    favorites.event(serial, result);
                }
            }
            Event::ConversationFocus(response) => {
                let view = self.history_context.borrow().clone();
                if let Some(view) = view.filter(|view| {
                    view.context == response.context && view.serial == response.serial
                }) {
                    view.apply(response.result);
                }
            }
            Event::SessionSearch(response) => {
                let current = {
                    let state = self.state.borrow();
                    state.backend == response.context.backend
                        && state.socket_generation == response.context.generation
                        && state
                            .selected
                            .as_ref()
                            .is_some_and(|thread| thread.id == response.context.thread)
                };
                if current && response.serial == self.finder.serial() {
                    self.finder
                        .apply_scope(response.serial, response.result, response.scope);
                }
            }
            Event::Table(response) => self.tables.event(response),
            Event::SessionResources(response) => self.resources.event(response),
            Event::SessionInfo {
                dialog,
                serial,
                result,
            } => {
                let panel = self.session_info.borrow().clone();
                if let Some(panel) = panel.filter(|p| p.id == dialog) {
                    panel.event(serial, result);
                }
            }
            Event::Connections {
                dialog,
                serial,
                result,
            } => {
                let panel = self.connections.borrow().clone();
                if let Some(panel) = panel.filter(|p| p.id == dialog) {
                    panel.event(serial, result);
                }
            }
            Event::MapStoreChanged {
                backend,
                generation,
                thread,
                message,
            } => self
                .session_map
                .external_update(&backend, generation, &thread, &message),
            Event::MapSync {
                backend,
                generation,
                thread,
                result,
                message,
            } => self
                .session_map
                .synchronized(&backend, generation, &thread, result, &message),
            Event::SessionMap { serial, result } => self.session_map.event(serial, result),
            Event::Pdf(response) => self.pdf.event(response),
            Event::Annotations {
                dialog,
                serial,
                result,
            } => {
                let panel = self.annotations.borrow().clone();
                if let Some(panel) = panel.filter(|p| p.id == dialog) {
                    panel.event(serial, result);
                }
            }
            Event::Epub(response) => self.epub.event(response),
            Event::NativeImage { id, result } => self.images.event(id, result),
            Event::ImageExport { id, result } => self.image_viewer.exported(id, result),
            Event::Workspace(response) => match response.panel {
                1 => self.files.event(response),
                2 => self.git.event(response),
                3 => self.source.event(response),
                4 => self.compare.event(response),
                _ => {}
            },
            Event::FilesWatch { generation, update } => self.files.watch_event(generation, update),
            Event::Terminal { generation, update } => self.terminal.event(generation, update),
            Event::Document(response) => {
                if response.key == crate::inline_math::KEY {
                    self.inline_math.event(response);
                    return;
                }
                if response.key == crate::favorites_ui::DIAGRAM_KEY {
                    let favorites = self.favorites.borrow().clone();
                    if let Some(favorites) = favorites {
                        favorites.diagrams.event(response);
                    }
                    return;
                }
                if response.key == crate::conversation_context_ui::DIAGRAM_KEY {
                    if let Some(view) = self.history_context.borrow().as_ref() {
                        view.diagrams.event(response);
                    }
                    return;
                }
                if response.key == crate::chat_diagrams::KEY {
                    self.chat_diagrams.event(response);
                } else if self.reader.accepts_key(response.key) {
                    self.reader.event(response)
                } else {
                    self.compare.document_event(response)
                }
            }
            Event::Startup { generation, update } => {
                if generation != self.state.borrow().startup_generation {
                    return;
                }
                match update {
                    crate::startup::Update::Launching => {
                        crate::i18n_ui::bind(&self.status, "label", "Starting gateway…");
                    }
                    crate::startup::Update::LaunchFailed(error) => {
                        self.notice("Gateway could not start. Use Reconnect to try again.");
                        self.toast_error(&error);
                    }
                    crate::startup::Update::Loaded(event) => {
                        if !self.gateway_stopped.get() {
                            self.event(*event);
                        }
                    }
                    crate::startup::Update::Retry { attempt, delay } => {
                        crate::i18n_ui::bind_format(
                            &self.status,
                            "label",
                            "Loading backends · Retrying in {seconds}s ({attempt}/8)",
                            &[
                                ("seconds", &format!("{:.1}", delay.as_secs_f64())),
                                ("attempt", &attempt.to_string()),
                            ],
                        );
                    }
                    crate::startup::Update::Failed(error) => {
                        self.notice("Backend discovery failed. Use Reconnect to try again.");
                        self.toast_error(&error);
                    }
                }
            }
            Event::Backends(backends) => {
                self.state.borrow_mut().startup_ready = true;
                let backends = backends
                    .into_iter()
                    .filter(|b| matches!(b.kind.as_str(), "codex" | "opencode"))
                    .collect::<Vec<_>>();
                let names = backends.iter().map(|b| b.name.as_str()).collect::<Vec<_>>();
                self.backend.set_model(Some(&gtk::StringList::new(&names)));
                self.state.borrow_mut().backends = backends;
                self.command(Command::Http {
                    tag: "gateway-info".into(),
                    method: reqwest::Method::GET,
                    path: "/studio/native/info".into(),
                    body: None,
                    document: None,
                });
                self.command(Command::Http {
                    tag: "session-pins".into(),
                    method: reqwest::Method::GET,
                    path: "/studio/session-state/pin".into(),
                    body: None,
                    document: None,
                });
                self.backend.set_sensitive(true);
                let index = {
                    let state = self.state.borrow();
                    state
                        .backends
                        .iter()
                        .position(|backend| backend.id == state.saved_backend)
                        .unwrap_or(0)
                };
                self.backend.set_selected(index as u32);
                let first = self.state.borrow().backends.get(index).cloned();
                if let Some(first) = first {
                    let reconnect = {
                        let state = self.state.borrow();
                        state.backend == first.id && !state.ready
                    };
                    self.connect_backend(first, reconnect);
                }
            }
            Event::Streaming(response) => self.streaming_response(response),
            Event::Wire {
                backend,
                generation,
                payload,
                prepared,
            } => {
                if self.state.borrow().backend_generations.get(&backend) != Some(&generation) {
                    return;
                }
                self.record_activity(&backend, &payload);
                self.observe_cached_history(&backend, generation, &payload, &prepared);
                if let Some(update) = prepared.command_data.as_ref() {
                    self.state
                        .borrow_mut()
                        .command_data
                        .observe(&backend, update);
                }
                self.observe_operations(&backend, generation, &payload, &prepared);
                if model::text(&payload, "method") == "studio/appServer/status" {
                    let ready =
                        payload.pointer("/params/state").and_then(Value::as_str) == Some("ready");
                    let mut state = self.state.borrow_mut();
                    if ready {
                        state.backend_ready.insert(backend.clone());
                    } else {
                        state.backend_ready.remove(&backend);
                    }
                    drop(state);
                    if ready {
                        self.backend_recovered(&backend, generation);
                    }
                    if !ready {
                        self.supervision_backend_lost(&backend);
                        self.expire_inputs(Some(&backend), Some(generation), None);
                    }
                }
                self.observe_queue(&backend, generation, &payload);
                if let Some(method) = payload
                    .get("method")
                    .and_then(Value::as_str)
                    .filter(|_| payload.get("id").is_some())
                {
                    self.interaction(
                        method,
                        &payload,
                        prepared
                            .interaction_details
                            .as_deref()
                            .unwrap_or("No details supplied"),
                        &backend,
                        generation,
                        prepared.interaction.as_ref(),
                        prepared.approval.as_ref(),
                    );
                    return;
                }
                if payload["method"] == "serverRequest/resolved" {
                    if let Some(id) = payload.pointer("/params/requestId") {
                        self.expire_inputs(Some(&backend), Some(generation), Some(id));
                    }
                }
                if let Some(id) = payload["id"].as_u64() {
                    let matches = self.state.borrow().pending.get(&id).is_some_and(|request| {
                        request.backend == backend && request.generation == generation
                    });
                    if !matches {
                        return;
                    }
                    let request = self.state.borrow_mut().pending.remove(&id).unwrap();
                    if let Pending::Send {
                        information: Some(ticket),
                        ..
                    } = &request.action
                    {
                        let result = if let Some(error) = payload.get("error") {
                            Err(error["message"]
                                .as_str()
                                .unwrap_or("Session responsibility generation failed")
                                .into())
                        } else {
                            payload.pointer("/result/turn/id").and_then(Value::as_str).filter(|s|!s.is_empty() && s.len()<=256).map(str::to_owned).ok_or_else(||"The backend did not identify the generation turn; inspect the conversation before retrying".into())
                        };
                        self.information_ack(*ticket, result);
                    }
                    if let Pending::Send {
                        annotation: Some(ticket),
                        ..
                    } = &request.action
                    {
                        self.annotation_result(ticket, payload.get("error").is_none());
                    }

                    if matches!(request.action, Pending::Tools(_)) {
                        self.response(request.action, &payload, prepared);
                        return;
                    }
                    if let Pending::Agents {
                        dialog,
                        revision,
                        step,
                    } = request.action
                    {
                        self.agents_response(dialog, revision, step, &payload, prepared);
                        return;
                    }
                    if let Pending::Mcp { dialog, revision } = request.action {
                        self.mcp_response(dialog, revision, &payload, prepared);
                        return;
                    }
                    if let Pending::Queue { key, id, .. } = &request.action {
                        self.queue_ack(key, id, &backend, generation, &payload);
                        let visible = self.composer_key().as_deref() == Some(key.as_str());
                        if visible {
                            self.present_reply(prepared.reply_items);
                        }
                        return;
                    }
                    let visible = {
                        let state = self.state.borrow();
                        state.backend == backend
                            && request.action.thread().is_none_or(|thread| {
                                state
                                    .selected
                                    .as_ref()
                                    .is_some_and(|selected| selected.id == thread)
                            })
                    };
                    if visible {
                        self.response(request.action, &payload, prepared);
                    } else {
                        self.inactive_response(&backend, request.action, &payload, prepared);
                    }
                    return;
                }
                if self.state.borrow().backend != backend
                    || self.state.borrow().socket_generation != generation
                {
                    return;
                }
                if let Some(method) = payload.get("method").and_then(Value::as_str) {
                    self.notification(method, &payload, prepared);
                }
            }
            Event::Environment { tag, result } => {
                let settings = self.settings.borrow().clone();
                if let Some(settings) = settings {
                    settings.environment_event(&tag, result);
                }
            }
            Event::Http { tag, result } if tag.starts_with("settings:") => {
                let settings = self.settings.borrow().clone();
                if let Some(settings) = settings {
                    settings.event(&tag, result);
                }
            }
            Event::Http { tag, result } if tag.starts_with("native-supervision-scope:") => {
                if let Ok(request) = tag.trim_start_matches("native-supervision-scope:").parse() {
                    self.supervision_scope_result(request, result);
                }
            }
            Event::Http { tag, result } if tag.starts_with("native-supervision-options:") => {
                if let Ok(serial) = tag
                    .trim_start_matches("native-supervision-options:")
                    .parse()
                {
                    self.supervision_options_result(serial, result);
                }
            }
            Event::Http { tag, result } => match result {
                Ok(value) if tag.starts_with("turn-options-load:") => {
                    let request = tag
                        .trim_start_matches("turn-options-load:")
                        .parse::<u64>()
                        .unwrap_or(0);
                    if request != self.state.borrow().options_request
                        || self.state.borrow().selected.is_none()
                    {
                        return;
                    }
                    match serde_json::from_value::<crate::turn_options::Options>(value) {
                        Ok(options) => {
                            let mut state = self.state.borrow_mut();
                            state.options = options;
                            state.options_loaded = true;
                            drop(state);
                            self.apply_options();
                        }
                        Err(error) => self.error(&format!("Invalid session options: {error}")),
                    }
                }
                Ok(value) if matches!(tag.as_str(), "session-pins" | "session-pin") => {
                    let mut state = self.state.borrow_mut();
                    state.pins = value["pinnedSessions"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .take(10)
                        .map(str::to_owned)
                        .collect();
                    state.pins_loaded = true;
                    state.pin_pending = false;
                    drop(state);
                    self.render_sessions();
                    self.update_composer();
                }
                Ok(value) if tag == "native-preferences-load" => {
                    self.notifications.restore_ids(&value["notificationIds"]);
                    self.state.borrow_mut().saved_backend =
                        value["selectedBackend"].as_str().unwrap_or("").into();
                    self.state.borrow_mut().saved_session =
                        value["selectedSession"].as_str().unwrap_or("").into();
                    self.window.set_default_size(
                        value["windowWidth"].as_i64().unwrap_or(1280) as i32,
                        value["windowHeight"].as_i64().unwrap_or(850) as i32,
                    );
                    if value["maximized"] == true {
                        self.window.maximize();
                    }
                    self.split
                        .set_show_sidebar(value["sidebarCollapsed"] != true);
                    adw::StyleManager::default().set_color_scheme(match value["theme"].as_str() {
                        Some("dark") => adw::ColorScheme::ForceDark,
                        Some("light") => adw::ColorScheme::ForceLight,
                        _ => adw::ColorScheme::Default,
                    });
                }
                Ok(value) if tag == "gateway-info" => {
                    self.state.borrow_mut().gateway_version =
                        model::text(&value, "appVersion").to_owned();
                }
                Err(error) => {
                    if tag == "session-pin" {
                        self.state.borrow_mut().pin_pending = false;
                        self.update_composer();
                    }
                    self.error(&format!("{tag}: {error}"));
                }
                _ => {}
            },
            Event::GatewayStopped(error) => {
                self.remember_draft();
                self.recovery.borrow_mut().stop();
                self.expire_inputs(None, None, None);
                self.annotations_disconnected(None);
                let connections = self
                    .state
                    .borrow()
                    .backend_generations
                    .iter()
                    .map(|(id, generation)| (id.clone(), *generation))
                    .collect::<Vec<_>>();
                for (id, generation) in connections {
                    self.supervision_backend_lost(&id);
                    self.mcp_disconnected(&id, generation);
                    self.agents_disconnected(&id, generation);
                }
                {
                    let mut state = self.state.borrow_mut();
                    let ids = state
                        .backends
                        .iter()
                        .map(|b| b.id.clone())
                        .collect::<Vec<_>>();
                    for id in ids {
                        state.cache.invalidate(&id);
                    }
                    self.gateway_stopped.set(true);
                    state.backend_generations.clear();
                    state.backend_ready.clear();
                    state.pending.clear();
                    state.ready = false;
                    state.sending = false;
                    state.loading = false;
                    state.catalog_loading = false;
                    state.history_verified = false;
                    state.session_mutation = false;
                    state.shells.disconnected(None);
                    state.images.disconnected(None);
                    for activity in state.activity.values_mut() {
                        activity.stale = true;
                    }
                }
                self.queue_gateway_failed(&error);
                self.reader.gateway_failed(&error);
                self.compare.gateway_failed(&error);
                self.terminal.gateway_failed(&error);
                self.tables.gateway_failed(&error);
                self.pdf.gateway_failed(&error);
                self.epub.gateway_failed(&error);
                self.error(&error);
                self.update_composer();
            }
            Event::Error(error) => {
                self.language.gateway_failed();
                self.message_flow.gateway_failed();
                let settings = self.settings.borrow().clone();
                if let Some(settings) = settings {
                    settings.backend_config_failed(&error);
                }
                self.expire_inputs(None, None, None);
                self.annotations_disconnected(None);
                let comments = self.annotations.borrow().clone();
                if let Some(comments) = comments {
                    comments.gateway_failed();
                }
                if let Some(information) = self.session_info.borrow().clone() {
                    information.gateway_failed();
                }
                self.state.borrow_mut().shells.disconnected(None);
                self.state.borrow_mut().images.disconnected(None);
                self.queue_gateway_failed(&error);
                let favorites = self.favorites.borrow().clone();
                if let Some(favorites) = favorites {
                    favorites.gateway_failed(&error);
                }
                self.state.borrow_mut().session_mutation = false;
                self.reader.gateway_failed(&error);
                self.compare.gateway_failed(&error);
                self.state.borrow_mut().ready = false;
                self.state.borrow_mut().sending = false;
                self.error(&error);
                self.update_composer();
            }
            Event::BackendDisconnected {
                backend,
                generation,
                error,
            } => {
                if self.state.borrow().backend_generations.get(&backend) == Some(&generation) {
                    self.supervision_backend_lost(&backend);
                }
                self.backend_disconnected(backend.clone(), generation, error);
                self.schedule_backend_recovery(&backend, generation);
            }
            Event::BackendError {
                backend,
                generation,
                error,
            } => {
                if self.state.borrow().backend_generations.get(&backend) == Some(&generation) {
                    self.supervision_backend_lost(&backend);
                }
                self.backend_error(backend.clone(), generation, error);
            }
        }
        self.sync_connections();
    }

    fn response(self: &Rc<Self>, pending: Pending, payload: &Value, prepared: protocol::Prepared) {
        if let Pending::Operation {
            thread,
            name,
            original,
        } = &pending
        {
            let backend = self.state.borrow().backend.clone();
            self.operation_response(
                &backend,
                thread,
                name,
                original.clone(),
                payload,
                prepared,
                true,
            );
            return;
        }
        if let Pending::Send { thread, .. } = &pending {
            let backend = self.state.borrow().backend.clone();
            self.complete_manual_reply(&backend, thread, payload);
        }
        if let Pending::Tools(serial) = pending {
            self.tools_reply(
                serial,
                if let Some(error) = payload.get("error") {
                    Err(error["message"]
                        .as_str()
                        .unwrap_or("Reference discovery failed")
                        .into())
                } else {
                    Ok(prepared.tools)
                },
            );
            return;
        }
        if let Pending::Select(thread, generation) = &pending {
            let state = self.state.borrow();
            if *generation != state.history_generation
                || state
                    .selected
                    .as_ref()
                    .is_none_or(|selected| &selected.id != thread)
            {
                return;
            }
        }
        if matches!(pending, Pending::SessionMutation { .. } | Pending::Create) {
            self.state.borrow_mut().session_mutation = false;
            self.update_composer();
        }
        if let Pending::Send { thread, .. } = &pending {
            if payload.get("error").is_some() {
                let backend = self.state.borrow().backend.clone();
                self.state.borrow_mut().images.acknowledged(
                    &format!("{backend}:{thread}"),
                    &[],
                    false,
                );
                self.queue_send_failed(&backend, thread);
            }
        }
        if let Some(error) = payload.get("error") {
            if let Pending::Select(id, _) = &pending {
                self.router_navigation_failed(id);
            }
            if let Pending::Catalog { generation, .. } = &pending {
                if *generation == self.state.borrow().catalog_generation {
                    self.state.borrow_mut().catalog_loading = false;
                }
            }
            if matches!(&pending,Pending::Select(id,_) if self.state.borrow().selected.as_ref().is_some_and(|t|t.id==*id))
            {
                let mut state = self.state.borrow_mut();
                state.loading = false;
                state.history_verified = false;
                if let Some(thread) = &state.selected {
                    let key = format!("{}:{}", state.backend, thread.id);
                    if let Some(entry) = state.queues.entries.get_mut(&key) {
                        entry.pause("History could not be verified. Refresh the conversation before resuming.");
                    }
                }
            }
            if matches!(pending, Pending::Send { .. }) {
                self.state.borrow_mut().sending = false;
            }
            self.error(
                error["message"]
                    .as_str()
                    .unwrap_or("Backend request failed"),
            );
            let reconcile = match &pending {
                Pending::Send { thread, .. } | Pending::Stop(thread) => Some(thread.clone()),
                _ => None,
            };
            if let Some(thread) = reconcile {
                if self
                    .state
                    .borrow()
                    .selected
                    .as_ref()
                    .is_some_and(|selected| selected.id == thread)
                {
                    self.state.borrow_mut().loading = true;
                    self.read_session(&thread, false);
                }
            }
            self.update_composer();
            return;
        }
        let result = &payload["result"];
        match pending {
            Pending::Agents { .. } => {}
            Pending::Mcp { .. } => {}
            Pending::Tools(_) => {}
            Pending::Operation { .. } => {}
            Pending::Queue { .. } => {}
            Pending::Catalog { append, generation } => {
                if generation != self.state.borrow().catalog_generation {
                    return;
                }
                let page = match prepared
                    .catalog
                    .unwrap_or_else(|| Err("Invalid session catalog response".into()))
                {
                    Ok(page) => page,
                    Err(error) => {
                        self.state.borrow_mut().catalog_loading = false;
                        self.error(&error);
                        self.update_composer();
                        return;
                    }
                };
                {
                    let mut state = self.state.borrow_mut();
                    state.catalog_loading = false;
                    let retained = state
                        .selected
                        .as_ref()
                        .filter(|thread| {
                            thread.parent.is_some()
                                || state.reference_selection.as_deref()
                                    == Some(format!("{}:{}", state.backend, thread.id).as_str())
                        })
                        .cloned();
                    if !append {
                        state.threads.clear();
                        state.catalog_truncated = false;
                    }
                    for agent in page.agents {
                        let thread = agent.thread.clone();
                        if !state.threads.iter().any(|t| t.id == thread.id) {
                            if state.threads.len() < crate::agents::MAX_AGENTS {
                                state.threads.push(thread);
                            } else {
                                state.catalog_truncated = true;
                            }
                        }
                    }
                    if let Some(retained) = retained.filter(|retained| {
                        !state.threads.iter().any(|thread| thread.id == retained.id)
                    }) {
                        state.threads.insert(0, retained);
                    }
                    if state.threads.len() >= crate::agents::MAX_AGENTS && page.cursor.is_some() {
                        state.catalog_truncated = true;
                    }
                    state.cursor = if state.catalog_truncated {
                        None
                    } else {
                        page.cursor
                    };
                }
                self.render_sessions();
                self.update_composer();
                crate::i18n_ui::bind(
                    &self.status,
                    "label",
                    if self.state.borrow().visibility.is_none() {
                        "Session visibility is unavailable; reload it in Settings"
                    } else if self.state.borrow().catalog_truncated {
                        "Session list limited to 10,000 entries; additional sessions are not loaded"
                    } else {
                        "Connected"
                    },
                );
                let restore = self.state.borrow_mut().restore_selection.take();
                if let Some(thread) = restore {
                    if self.state.borrow().selected.is_none() {
                        self.select(thread);
                    }
                }
            }
            Pending::Models => {
                let entries = result["data"].as_array().cloned().unwrap_or_default();
                let models = entries
                    .iter()
                    .filter_map(|model| model["model"].as_str().map(str::to_owned))
                    .collect::<Vec<_>>();
                let fast_models = entries
                    .iter()
                    .filter(|model| crate::turn_options::supports_fast(model))
                    .filter_map(|model| model["model"].as_str().map(str::to_owned))
                    .collect();
                let default_model = entries
                    .iter()
                    .find(|model| model["isDefault"] == true)
                    .and_then(|model| model["model"].as_str())
                    .unwrap_or("")
                    .to_owned();
                let model_efforts = entries
                    .iter()
                    .filter_map(|model| {
                        let id = model["model"].as_str()?;
                        let efforts = model["supportedReasoningEfforts"]
                            .as_array()?
                            .iter()
                            .filter_map(|effort| {
                                effort["reasoningEffort"].as_str().map(str::to_owned)
                            })
                            .collect::<Vec<_>>();
                        Some((id.to_owned(), efforts))
                    })
                    .collect();
                let mut labels = vec!["Default model".to_owned()];
                labels.extend(models.iter().cloned());
                let labels = labels.iter().map(String::as_str).collect::<Vec<_>>();
                {
                    let mut state = self.state.borrow_mut();
                    state.updating_options = true;
                    state.models = models;
                    state.fast_models = fast_models;
                    state.default_model = default_model;
                    state.model_efforts = model_efforts;
                }
                self.model.set_model(Some(&gtk::StringList::new(&labels)));
                self.apply_options();
            }
            Pending::Select(id, _) => {
                if prepared
                    .thread_summary
                    .as_ref()
                    .and_then(|s| s.as_ref().ok())
                    .is_some_and(|agent| agent.thread.id != id)
                {
                    self.state.borrow_mut().loading = false;
                    self.router_navigation_failed(&id);
                    self.error("Backend history belongs to a different session");
                    self.update_composer();
                    return;
                }
                if self.router_selected() {
                    self.state.borrow_mut().history_verified = true;
                    if let Some(thread) = prepared
                        .thread_summary
                        .as_ref()
                        .and_then(|s| s.as_ref().ok())
                        .map(|agent| agent.thread.clone())
                    {
                        self.title.set_title(&thread.title);
                        crate::i18n_ui::literal(&self.title, "subtitle", &thread.cwd);
                        let mut state = self.state.borrow_mut();
                        state.reference_selection =
                            Some(format!("{}:{}", state.backend, thread.id));
                        state.selected = Some(thread);
                    }
                    self.router_render();
                    return;
                }
                if self
                    .state
                    .borrow()
                    .selected
                    .as_ref()
                    .is_none_or(|t| t.id != id)
                {
                    return;
                }
                {
                    let mut state = self.state.borrow_mut();
                    if let Some(thread) = prepared
                        .thread_summary
                        .as_ref()
                        .and_then(|summary| summary.as_ref().ok())
                        .map(|agent| agent.thread.clone())
                    {
                        self.title.set_title(&thread.title);
                        crate::i18n_ui::literal(&self.title, "subtitle", &thread.cwd);
                        if !state
                            .threads
                            .iter()
                            .any(|candidate| candidate.id == thread.id)
                        {
                            state.threads.insert(0, thread.clone());
                        }
                        state.reference_selection =
                            Some(format!("{}:{}", state.backend, thread.id));
                        state.selected = Some(thread);
                    }
                    let (items, lifecycle) = crate::history_state::merge(
                        prepared.history.unwrap_or_default(),
                        &state.items,
                        crate::history_state::Lifecycle {
                            active_turn: prepared.active_turn,
                            active_shell: prepared.active_shell,
                        },
                        state.active_turn.as_deref(),
                        &state.history_changes,
                    );
                    state.items = items;
                    state.history_changes.settled();
                    state.history_verified = true;
                    state.loading = false;
                    state.active_turn = lifecycle.active_turn;
                    let key = format!("{}:{id}", state.backend);
                    let generation = state.socket_generation;
                    let active = state.active_turn.clone();
                    state.shells.history(
                        &key,
                        generation,
                        active.as_deref(),
                        lifecycle.active_shell,
                    );
                    if let Some(entry) = state.queues.entries.get_mut(&key) {
                        entry.connected = true;
                        if entry.flight.is_none() {
                            entry.active = active.is_some();
                            entry.released = active.is_none();
                        }
                    }
                    state.queues.dirty = true;
                    state.session_model = result["model"]
                        .as_str()
                        .or(result.pointer("/thread/model").and_then(Value::as_str))
                        .unwrap_or("")
                        .into();
                    state.session_tier = result["serviceTier"]
                        .as_str()
                        .or(result
                            .pointer("/thread/serviceTier")
                            .and_then(Value::as_str))
                        .unwrap_or("")
                        .into();
                    let key = format!("{}:{id}", state.backend);
                    let active = state.active_turn.is_some();
                    state.activity.entry(key).or_default().read(active);
                    state.render_dirty = true;
                }
                crate::i18n_ui::bind(
                    &self.status,
                    "label",
                    if self.state.borrow().active_turn.is_some() {
                        "Working…"
                    } else {
                        "Connected"
                    },
                );
                self.render_sessions();
                self.apply_options();
            }
            Pending::Create => {
                if let Some(thread) = prepared
                    .thread_summary
                    .as_ref()
                    .and_then(|summary| summary.as_ref().ok())
                    .map(|agent| agent.thread.clone())
                {
                    self.state.borrow_mut().threads.insert(0, thread.clone());
                    self.render_sessions();
                    // A newly started thread is already prepared in this App Server process.
                    self.stash_current();
                    self.state.borrow_mut().reconnecting = None;
                    self.state.borrow_mut().selected = Some(thread.clone());
                    self.state.borrow_mut().items = prepared.history.unwrap_or_default();
                    self.state.borrow_mut().render_dirty = true;
                    self.state.borrow_mut().active_turn = None;
                    self.state.borrow_mut().session_model =
                        result["model"].as_str().unwrap_or("").into();
                    self.state.borrow_mut().session_tier =
                        result["serviceTier"].as_str().unwrap_or("").into();
                    self.state.borrow_mut().loading = false;
                    self.state.borrow_mut().history_verified = true;
                    self.state.borrow_mut().history_changes.settled();
                    self.title.set_title(&thread.title);
                    crate::i18n_ui::literal(&self.title, "subtitle", &thread.cwd);
                    self.composer.buffer().set_text("");
                    self.reset_search_context();
                    self.chat_diagrams.reset();
                    self.transcript.remove_all();
                    self.update_composer();
                    self.composer.grab_focus();
                    self.load_options();
                    self.command(Command::Preferences(json!({"selectedBackend":self.state.borrow().backend,"selectedSession":thread.id})));
                }
            }
            Pending::Send {
                thread,
                text,
                images,
                references,
                information,
                ..
            } => {
                self.state.borrow_mut().sending = false;
                self.present_reply(prepared.reply_items);
                if result.pointer("/turn/status").and_then(Value::as_str) == Some("failed") {
                    self.error(
                        result
                            .pointer("/turn/error/message")
                            .and_then(Value::as_str)
                            .unwrap_or("The backend command failed"),
                    );
                }
                let key = format!("{}:{thread}", self.state.borrow().backend);
                self.state
                    .borrow_mut()
                    .images
                    .acknowledged(&key, &images, true);
                self.state.borrow_mut().references.remove(&key, &references);
                if self
                    .state
                    .borrow()
                    .selected
                    .as_ref()
                    .is_some_and(|t| t.id == thread)
                {
                    if information.is_none() && self.composer_text() == text {
                        self.composer.buffer().set_text("");
                    }
                    let open_code = {
                        let state = self.state.borrow();
                        state.backends.iter().any(|backend| {
                            backend.id == state.backend && backend.kind == "opencode"
                        })
                    };
                    // HTTP prompt acknowledgement may arrive after SSE idle.
                    // OpenCode activity is owned by status snapshots/events.
                    if !open_code
                        && matches!(
                            result.pointer("/turn/status").and_then(Value::as_str),
                            Some("inProgress") | None
                        )
                    {
                        if let Some(turn) = result.pointer("/turn/id").and_then(Value::as_str) {
                            let mut state = self.state.borrow_mut();
                            if !state
                                .activity
                                .get(&key)
                                .is_some_and(|activity| activity.finished(turn))
                            {
                                state.active_turn = Some(turn.into());
                            }
                        }
                    }
                }
                self.update_composer();
            }
            Pending::Stop(thread) => {
                if self
                    .state
                    .borrow()
                    .selected
                    .as_ref()
                    .is_some_and(|t| t.id == thread)
                {
                    crate::i18n_ui::bind(&self.status, "label", "Interrupt requested…");
                }
            }
            Pending::SessionMutation { thread, kind, name } => {
                if kind == "fork" {
                    let options = self.state.borrow().options.clone();
                    self.response(Pending::Create, payload, prepared);
                    let key = {
                        let state = self.state.borrow();
                        state
                            .selected
                            .as_ref()
                            .map(|thread| format!("{}:{}", state.backend, thread.id))
                    };
                    if let Some(key) = key {
                        self.command(Command::SessionOptions {
                            key,
                            request: 0,
                            options: Some(options),
                        });
                    }
                    self.refresh();
                    return;
                }
                if kind == "rename" {
                    let mut state = self.state.borrow_mut();
                    for candidate in &mut state.threads {
                        if candidate.id == thread {
                            candidate.title = name.clone();
                        }
                    }
                    if let Some(selected) = state
                        .selected
                        .as_mut()
                        .filter(|selected| selected.id == thread)
                    {
                        selected.title = name;
                        self.title.set_title(&selected.title);
                    }
                    drop(state);
                    self.render_sessions();
                } else {
                    let key = format!("{}:{thread}", self.state.borrow().backend);
                    {
                        let mut state = self.state.borrow_mut();
                        if matches!(kind, "delete" | "archive") {
                            if kind == "delete" {
                                state.queues.entries.remove(&key);
                            } else if let Some(entry) = state.queues.entries.get_mut(&key) {
                                entry.pause("Session archived");
                            }
                        }
                        if kind == "delete" {
                            state.images.forget(&key);
                            state.references.forget(&key);
                            state.policies.forget(&key);
                            state.shells.forget(&key);
                            state.command_data.forget(&key);
                            state.cache.0.remove(&key);
                            state.drafts.remove(&key);
                            let backend = state.backend.clone();
                            if state
                                .last_selected
                                .get(&backend)
                                .is_some_and(|last| last.id == thread)
                            {
                                state.last_selected.remove(&backend);
                            }
                        }
                        state.threads.retain(|candidate| candidate.id != thread);
                        if state
                            .selected
                            .as_ref()
                            .is_some_and(|selected| selected.id == thread)
                        {
                            state.selected = None;
                            state.active_turn = None;
                            state.items.clear();
                            state.render_dirty = true;
                        }
                        if kind != "restore" {
                            state.pins.retain(|pin| pin != &key);
                        }
                    }
                    if kind == "delete" {
                        self.command(Command::Http {
                            tag: "session-delete-cleanup".into(),
                            method: reqwest::Method::DELETE,
                            path: "/studio/session-state/session".into(),
                            body: Some(json!({"sessionKey":key})),
                            document: None,
                        });
                    }
                    if kind == "archive" {
                        self.command(Command::Http {
                            tag: "session-pin".into(),
                            method: reqwest::Method::PUT,
                            path: "/studio/session-state/pin".into(),
                            body: Some(json!({"sessionKey":key,"pinned":false})),
                            document: None,
                        });
                    }
                    self.composer.buffer().set_text("");
                    self.title.set_title("Codex Thread Studio");
                    self.title.set_subtitle("Choose a session");
                    self.command(Command::Preferences(json!({"selectedSession":""})));
                    self.catalog(false);
                    self.render_sessions();
                    self.update_composer();
                }
            }
        }
    }

    fn present_reply(&self, items: Vec<Arc<Item>>) {
        if items.is_empty() {
            return;
        }
        let mut state = self.state.borrow_mut();
        for item in items {
            if let Some(old) = state.items.iter_mut().find(|old| old.id == item.id) {
                *old = item;
            } else {
                state.items.push(item);
            }
        }
        model::trim_items(&mut state.items);
        state.render_dirty = true;
    }
    fn observe_cached_history(
        &self,
        backend: &str,
        generation: u64,
        payload: &Value,
        prepared: &protocol::Prepared,
    ) {
        let method = model::text(payload, "method");
        let params = &payload["params"];
        let thread = model::text(params, "threadId");
        if thread.is_empty() {
            return;
        }
        let mut state = self.state.borrow_mut();
        if state.backend == backend && state.selected.as_ref().is_some_and(|t| t.id == thread) {
            return;
        }
        let key = format!("{backend}:{thread}");
        state.cache.0.update(&key, |view| {
            if view.generation != generation {
                return;
            }
            if crate::history_state::observe(
                &mut view.items,
                &mut view.active_turn,
                &mut view.history_changes,
                view.loading,
                method,
                params,
                prepared.item.as_ref(),
            ) {
                view.valid = false;
            }
        });
    }

    fn notification(self: &Rc<Self>, method: &str, payload: &Value, prepared: protocol::Prepared) {
        let params = &payload["params"];
        if method == "studio/appServer/status" {
            match model::text(params, "state") {
                "ready" => {
                    self.state.borrow_mut().ready = true;
                    crate::i18n_ui::bind(&self.status, "label", "Connected");
                    self.catalog(false);
                    self.rpc(Pending::Models, "model/list", json!({"limit":100}));
                }
                "error" | "stopped" => {
                    self.state.borrow_mut().ready = false;
                    self.error(
                        params["message"]
                            .as_str()
                            .or(params["reason"].as_str())
                            .unwrap_or("Backend unavailable"),
                    );
                }
                _ => {}
            }
            self.update_composer();
            return;
        }
        if self.router_selected() {
            return;
        }
        let selected = self
            .state
            .borrow()
            .selected
            .as_ref()
            .map(|t| t.id.clone())
            .unwrap_or_default();
        if model::text(params, "threadId") != selected || selected.is_empty() {
            return;
        }
        {
            let mut state = self.state.borrow_mut();
            let State {
                items,
                active_turn,
                history_changes,
                loading,
                ..
            } = &mut *state;
            if crate::history_state::observe(
                items,
                active_turn,
                history_changes,
                *loading,
                method,
                params,
                prepared.item.as_ref(),
            ) {
                if method != "item/agentMessage/delta" {
                    state.render_dirty = true;
                }
            }
        }
        match method {
            "turn/started" => crate::i18n_ui::bind(&self.status, "label", "Working…"),
            "turn/completed" => {
                crate::i18n_ui::bind(&self.status, "label", "Connected · Turn complete")
            }
            "error" => self.error(
                params
                    .pointer("/error/message")
                    .and_then(Value::as_str)
                    .unwrap_or("Turn failed"),
            ),
            _ => {}
        }
        self.update_composer();
    }

    fn render_items(&self) {
        self.sync_resources();
        self.session_map.sync();
        let history_scroll = self.history_pages.take_scroll();
        let (items, follow, context) = {
            let adjustment = self.transcript_scroll.vadjustment();
            let follow = adjustment.value() + adjustment.page_size() >= adjustment.upper() - 80.0;
            let mut state = self.state.borrow_mut();
            state.render_dirty = false;
            // Live events update the authoritative tail but never reconcile or
            // scroll a historical window that the user is reading.
            if self.history_pages.active() && history_scroll.is_none() {
                return;
            }
            (
                self.history_pages
                    .items()
                    .unwrap_or_else(|| state.items.clone()),
                follow,
                state
                    .selected
                    .as_ref()
                    .map(|thread| crate::session_search::Context {
                        backend: state.backend.clone(),
                        thread: thread.id.clone(),
                        generation: state.socket_generation,
                    }),
            )
        };
        crate::chat_ui::reconcile(&self.transcript, &items);
        self.turn_navigation.refresh(context, &items);
        self.sync_turn_unread();
        let router_scroll = history_scroll.or_else(|| self.router.borrow_mut().scroll.take());
        if router_scroll == Some(false) {
            self.transcript_navigation.defer_edge(
                &self.transcript_view,
                &self.transcript_scroll,
                false,
            );
        } else if router_scroll == Some(true) {
            self.transcript_navigation.defer_edge(
                &self.transcript_view,
                &self.transcript_scroll,
                true,
            );
        } else if follow {
            self.transcript_navigation
                .follow_tail(&self.transcript_view, &self.transcript_scroll);
        }
    }

    fn interaction(
        self: &Rc<Self>,
        method: &str,
        payload: &Value,
        details: &str,
        backend: &str,
        generation: u64,
        prepared: Option<&Result<Arc<crate::interaction::Request>, String>>,
        approval: Option<&Result<Arc<crate::approval::Request>, String>>,
    ) {
        let id = payload["id"].clone();
        if !self.notifications.claim_request(backend, generation, &id) {
            return;
        }
        let params = &payload["params"];
        let thread = params["threadId"]
            .as_str()
            .or(params["sessionID"].as_str())
            .unwrap_or("workspace");
        let notice = self.notifications.approval(
            backend,
            thread,
            &self.notification_caption(backend, thread),
        );
        if matches!(
            method,
            "item/tool/requestUserInput" | "mcpServer/elicitation/request" | "opencode/question"
        ) {
            self.native_input(payload, prepared, backend, generation, notice);
            return;
        }
        let Some(scope) = crate::interaction_ui::Scope::new(backend, generation, &id) else {
            return;
        };
        if self.approvals.borrow().len() + self.interactions.borrow().len() >= 8 {
            let _ = self.command(Command::Reply {
                id,
                result: Err(protocol::ReplyError {
                    code: -32000,
                    message: "Native input dialog capacity reached".into(),
                }),
                backend: backend.into(),
                generation,
            });
            if let Some(notice) = notice {
                self.notifications.withdraw(&notice);
            }
            self.error("Resolve an existing input request before opening another");
            return;
        }
        let backend_name = self
            .state
            .borrow()
            .backends
            .iter()
            .find(|candidate| candidate.id == backend)
            .map(|backend| backend.name.clone())
            .unwrap_or_else(|| backend.into());
        let origin = format!("{backend_name} · {thread}");
        self.present_approval(scope, id, notice, &origin, approval, details);
    }

    fn new_session(self: &Rc<Self>) {
        if !self.state.borrow().ready || self.state.borrow().session_mutation {
            self.error("Connect a backend before creating a session.");
            return;
        }
        let dialog = adw::MessageDialog::new(
            Some(&self.window),
            Some("New session"),
            Some("Choose the project directory for this session."),
        );
        crate::i18n_ui::bind(&dialog, "heading", "New session");
        crate::i18n_ui::bind(
            &dialog,
            "body",
            "Choose the project directory for this session.",
        );
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let path = gtk::Entry::builder()
            .hexpand(true)
            .placeholder_text("/home/you/project")
            .build();
        crate::i18n_ui::accessible_label(&path, "Project directory");
        if let Some(thread) = &self.state.borrow().selected {
            path.set_text(&thread.cwd);
        }
        let browse = gtk::Button::from_icon_name("folder-open-symbolic");
        crate::i18n_ui::bind(&browse, "tooltip-text", "Choose project folder");
        let window = self.window.clone();
        let entry = path.clone();
        browse.connect_clicked(move |_| {
            let chooser = gtk::FileDialog::builder()
                .title("Choose project folder")
                .modal(true)
                .build()
                .ui_text("title", "Choose project folder");
            let entry = entry.clone();
            let window = window.clone();
            glib::MainContext::default().spawn_local(async move {
                if let Ok(folder) = chooser.select_folder_future(Some(&window)).await {
                    if let Some(path) = folder.path() {
                        entry.set_text(&path.to_string_lossy());
                    }
                }
            });
        });
        row.append(&path);
        row.append(&browse);
        dialog.set_extra_child(Some(&row));
        dialog.add_response("cancel", "Cancel");
        dialog.add_response("create", "Create");
        crate::i18n_ui::response(&dialog, "cancel", "Cancel");
        crate::i18n_ui::response(&dialog, "create", "Create");
        dialog.set_close_response("cancel");
        dialog.set_response_appearance("create", adw::ResponseAppearance::Suggested);
        let backend = self.state.borrow().backend.clone();
        let generation = self.state.borrow().socket_generation;
        let weak = Rc::downgrade(self);
        dialog.connect_response(None, move |_, response| {
            if response != "create" {
                return;
            }
            let Some(ui) = weak.upgrade() else {
                return;
            };
            if ui.state.borrow().backend != backend
                || ui.state.borrow().socket_generation != generation
                || ui.state.borrow().session_mutation
            {
                ui.error("The backend changed; reopen New session");
                return;
            }
            let cwd = path.text().to_string();
            if !std::path::Path::new(&cwd).is_absolute() {
                ui.error("Choose an absolute project directory.");
                return;
            }
            ui.rpc(Pending::Create, "thread/start", json!({"cwd":cwd}));
        });
        dialog.present();
    }

    fn toggle_sidebar(self: &Rc<Self>) {
        self.split.set_show_sidebar(!self.split.shows_sidebar());
        self.command(Command::Preferences(
            json!({"sidebarCollapsed":!self.split.shows_sidebar()}),
        ));
    }

    fn about(self: &Rc<Self>) {
        let dialog=adw::AboutDialog::builder().application_name("Codex Thread Studio · GNOME")
            .application_icon("io.github.messon007.codexthreadstudio.Gnome").version(format!("{} · gateway {}",env!("CARGO_PKG_VERSION"),self.state.borrow().gateway_version))
            .developer_name("Codex Thread Studio contributors").website("https://github.com/messon007/codex-thread-studio")
            .comments("Native GTK4/libadwaita interface with a shared Rust gateway. Experimental; see docs/gnome-native.md for feature coverage.").build();
        dialog.present(Some(&self.window));
    }
}

#[cfg(debug_assertions)]
mod smoke {
    use super::*;
    use std::{path::PathBuf, time::Instant};

    async fn conversation_cache(ui: &Rc<Ui>) -> Result<Value, String> {
        async fn until(test: impl Fn() -> bool) -> Result<(), String> {
            let started = Instant::now();
            while !test() {
                if started.elapsed() > Duration::from_secs(8) {
                    return Err("Native conversation cache timed out".into());
                }
                glib::timeout_future(Duration::from_millis(40)).await;
            }
            Ok(())
        }
        until(|| ui.send.is_sensitive() && ui.state.borrow().active_turn.is_none()).await?;
        let primary = ui
            .state
            .borrow()
            .selected
            .clone()
            .ok_or("No selected cache source")?;
        let original = ui
            .state
            .borrow()
            .items
            .first()
            .cloned()
            .ok_or("Cache source history is empty")?;
        let sibling = ui
            .state
            .borrow()
            .threads
            .iter()
            .find(|thread| thread.id == "native-cache-thread")
            .cloned()
            .ok_or("Cache sibling missing")?;
        ui.composer.buffer().set_text("native-smoke-delayed-send");
        ui.submit();
        if !ui.state.borrow().sending {
            return Err("Delayed native send was not started".into());
        }
        ui.select(sibling);
        until(|| !ui.state.borrow().loading && ui.send.is_sensitive()).await?;
        if ui
            .state
            .borrow()
            .drafts
            .get("codex:native-smoke-thread")
            .is_some_and(|draft| draft == "native-smoke-delayed-send")
        {
            return Err("A late send acknowledgement left submitted text in the old draft".into());
        }
        ui.select(primary);
        let cached = ui
            .state
            .borrow()
            .items
            .first()
            .cloned()
            .ok_or("Cached history was not immediately restored")?;
        if !Arc::ptr_eq(&original, &cached) {
            return Err("Native cached history copied/reparsed immutable message text".into());
        }
        if !ui.state.borrow().loading {
            return Err("Cached history skipped authoritative refresh".into());
        }
        until(|| !ui.state.borrow().loading && ui.send.is_sensitive()).await?;
        if ui.composer_text() == "native-smoke-delayed-send" {
            return Err("Returning to the old view restored a sent prompt".into());
        }
        Ok(
            json!({"ok":true,"immediateSnapshot":true,"sharedImmutableRows":true,"authoritativeRefresh":true,"lateSendReceipt":true}),
        )
    }

    async fn sessions(ui: &Rc<Ui>) -> Result<Value, String> {
        async fn until(test: impl Fn() -> bool) -> Result<(), String> {
            let start = Instant::now();
            while !test() {
                if start.elapsed() > Duration::from_secs(8) {
                    return Err("Native session management timed out".into());
                }
                glib::timeout_future(Duration::from_millis(40)).await;
            }
            Ok(())
        }
        fn dialog(heading: &str) -> Option<adw::MessageDialog> {
            gtk::Window::list_toplevels()
                .into_iter()
                .filter_map(|window| window.downcast::<adw::MessageDialog>().ok())
                .find(|dialog| dialog.heading().as_deref() == Some(heading))
        }
        until(|| ui.session_target().is_some() && !ui.state.borrow().catalog_loading).await?;
        ui.rename_session();
        let rename = dialog("Rename session").ok_or("Native rename dialog missing")?;
        rename
            .extra_child()
            .and_downcast::<gtk::Entry>()
            .ok_or("Native rename entry missing")?
            .set_text("Renamed native session");
        rename.response("rename");
        until(|| {
            !ui.state.borrow().session_mutation
                && ui
                    .state
                    .borrow()
                    .selected
                    .as_ref()
                    .is_some_and(|thread| thread.title == "Renamed native session")
        })
        .await?;
        ui.fork_session();
        until(|| {
            !ui.state.borrow().loading
                && ui.state.borrow().options_loaded
                && !ui.state.borrow().session_mutation
                && !ui.state.borrow().catalog_loading
                && ui
                    .state
                    .borrow()
                    .selected
                    .as_ref()
                    .is_some_and(|thread| thread.id == "native-fork-thread")
        })
        .await?;
        ui.archive_session();
        until(|| {
            !ui.state.borrow().session_mutation
                && !ui.state.borrow().catalog_loading
                && ui.state.borrow().selected.is_none()
        })
        .await?;
        ui.scope.set_selected(2);
        until(|| {
            !ui.state.borrow().catalog_loading
                && ui
                    .state
                    .borrow()
                    .threads
                    .iter()
                    .any(|thread| thread.id == "native-fork-thread")
        })
        .await?;
        let fork = ui
            .state
            .borrow()
            .threads
            .iter()
            .find(|thread| thread.id == "native-fork-thread")
            .cloned()
            .unwrap();
        ui.select(fork);
        until(|| !ui.state.borrow().loading && ui.state.borrow().options_loaded).await?;
        if ui.send.is_sensitive() {
            return Err("Archived chat allows sending".into());
        }
        ui.restore_session();
        until(|| {
            !ui.state.borrow().session_mutation
                && !ui.state.borrow().catalog_loading
                && ui.state.borrow().selected.is_none()
        })
        .await?;
        ui.scope.set_selected(0);
        until(|| {
            !ui.state.borrow().catalog_loading
                && ui
                    .state
                    .borrow()
                    .threads
                    .iter()
                    .any(|thread| thread.id == "native-fork-thread")
        })
        .await?;
        let fork = ui
            .state
            .borrow()
            .threads
            .iter()
            .find(|thread| thread.id == "native-fork-thread")
            .cloned()
            .unwrap();
        ui.select(fork);
        until(|| !ui.state.borrow().loading && ui.state.borrow().options_loaded).await?;
        ui.delete_session();
        dialog("Delete session permanently?")
            .ok_or("Native delete confirmation missing")?
            .response("cancel");
        if ui.state.borrow().session_mutation {
            return Err("Cancelled deletion was sent".into());
        }
        ui.delete_session();
        dialog("Delete session permanently?")
            .ok_or("Native delete confirmation missing")?
            .response("delete");
        until(|| {
            !ui.state.borrow().session_mutation
                && !ui.state.borrow().catalog_loading
                && ui.state.borrow().selected.is_none()
        })
        .await?;
        let primary = ui
            .state
            .borrow()
            .threads
            .iter()
            .find(|thread| thread.id == "native-smoke-thread")
            .cloned()
            .ok_or("Original session disappeared")?;
        ui.select(primary);
        until(|| !ui.state.borrow().loading && ui.state.borrow().options_loaded).await?;
        let generation = ui.state.borrow().catalog_generation.saturating_sub(1);
        ui.response(
            Pending::Catalog {
                append: false,
                generation,
            },
            &json!({"result":{"data":[{"id":"stale-archive-result","name":"stale"}]}}),
            protocol::Prepared::default(),
        );
        if ui
            .state
            .borrow()
            .threads
            .iter()
            .any(|thread| thread.id == "stale-archive-result")
        {
            return Err("Stale archived catalog replaced the active list".into());
        }
        Ok(
            json!({"ok":true,"rename":true,"fork":true,"archiveRestore":true,"archivedReadOnly":true,"deleteConfirmation":true,"catalogGeneration":true}),
        )
    }

    async fn opencode(ui: &Rc<Ui>, directory: &std::path::Path) -> Result<Value, String> {
        async fn until(test: impl Fn() -> bool) -> Result<(), String> {
            let started = Instant::now();
            while !test() {
                if started.elapsed() > Duration::from_secs(10) {
                    return Err("Native OpenCode acceptance timed out".into());
                }
                glib::timeout_future(Duration::from_millis(40)).await;
            }
            Ok(())
        }
        fn contains(widget: &gtk::Widget, text: &str) -> bool {
            if widget
                .downcast_ref::<gtk::Label>()
                .is_some_and(|label| label.text().contains(text))
            {
                return true;
            }
            let mut child = widget.first_child();
            while let Some(widget) = child {
                child = widget.next_sibling();
                if contains(&widget, text) {
                    return true;
                }
            }
            false
        }
        fn interaction(text: &str) -> Option<adw::MessageDialog> {
            gtk::Window::list_toplevels()
                .into_iter()
                .filter_map(|window| window.downcast::<adw::MessageDialog>().ok())
                .find(|dialog| {
                    dialog
                        .extra_child()
                        .is_some_and(|child| contains(&child, text))
                })
        }
        let index = ui
            .state
            .borrow()
            .backends
            .iter()
            .position(|backend| backend.kind == "opencode")
            .ok_or("Missing OpenCode backend")?;
        ui.composer.buffer().set_text("native-smoke-background");
        ui.submit();
        until(|| !ui.state.borrow().sending && ui.state.borrow().active_turn.is_some()).await?;
        ui.backend.set_selected(index as u32);
        until(|| {
            ui.state.borrow().ready
                && ui
                    .state
                    .borrow()
                    .threads
                    .iter()
                    .any(|thread| thread.id == "native-oc-thread")
        })
        .await?;
        let thread = ui
            .state
            .borrow()
            .threads
            .iter()
            .find(|thread| thread.id == "native-oc-thread")
            .cloned()
            .unwrap();
        ui.select(thread);
        until(|| {
            !ui.state.borrow().loading
                && ui
                    .state
                    .borrow()
                    .items
                    .iter()
                    .any(|item| item.body == "Native OpenCode history")
        })
        .await?;
        if ui.effort.is_visible() {
            return Err("Unsupported OpenCode effort control is visible".into());
        }
        until(|| interaction("native-background").is_some()).await?;
        interaction("native-background").unwrap().response("accept");
        until(|| {
            ui.state
                .borrow()
                .activity
                .get("codex:native-smoke-thread")
                .is_some_and(|activity| activity.attention && !activity.active)
        })
        .await?;
        if ui.state.borrow().backend != "opencode" {
            return Err("Background approval changed the selected backend".into());
        }
        until(|| ui.state.borrow().pins_loaded && ui.pin.is_sensitive()).await?;
        ui.pin.emit_clicked();
        until(|| {
            !ui.state.borrow().pin_pending
                && ui
                    .state
                    .borrow()
                    .pins
                    .contains(&"opencode:native-oc-thread".into())
        })
        .await?;
        ui.pin.emit_clicked();
        until(|| {
            !ui.state.borrow().pin_pending
                && !ui
                    .state
                    .borrow()
                    .pins
                    .contains(&"opencode:native-oc-thread".into())
        })
        .await?;
        ui.pin.emit_clicked();
        until(|| !ui.state.borrow().pin_pending && ui.pin.is_active()).await?;
        ui.composer.buffer().set_text("native-oc-fast");
        ui.submit();
        until(|| {
            !ui.state.borrow().sending
                && ui
                    .state
                    .borrow()
                    .items
                    .iter()
                    .any(|item| item.body == "Native OpenCode 中文😀")
        })
        .await?;
        until(|| ui.state.borrow().active_turn.is_none() && ui.send.is_sensitive()).await?;
        until(|| {
            ui.state.borrow().items.iter().any(|item| {
                item.id.ends_with("-structured") && item.body.contains("\"nativeStructured\":true")
            })
        })
        .await?;
        let structured_turn = {
            let state = ui.state.borrow();
            let item = state
                .items
                .iter()
                .find(|item| item.id.ends_with("-structured"))
                .ok_or("Structured OpenCode output was not mounted")?;
            item.turn_id.clone()
        };
        if !ui
            .state
            .borrow()
            .items
            .iter()
            .any(|item| item.id == structured_turn && item.body == "native-oc-fast")
        {
            return Err("Structured OpenCode output lost its server-owned parent anchor".into());
        }
        ui.render_items();
        let structured_position = (0..ui.transcript.n_items())
            .find(|position| {
                ui.transcript
                    .item(*position)
                    .and_downcast::<glib::BoxedAnyObject>()
                    .is_some_and(|object| {
                        let row = object.borrow::<crate::chat_ui::Row>();
                        row.item.id.ends_with("-structured")
                            && row.part != crate::chat_ui::Part::Header
                    })
            })
            .ok_or("Structured OpenCode output has no native display row")?;
        ui.transcript_view
            .scroll_to(structured_position, gtk::ListScrollFlags::NONE, None);
        until(|| contains(ui.transcript_view.upcast_ref(), "nativeStructured")).await?;
        capture(&ui.window, &directory.join("opencode-structured.png"));
        ui.composer.buffer().set_text("native-oc-stuck");
        ui.submit();
        until(|| {
            !ui.state.borrow().sending
                && ui.state.borrow().active_turn.is_some()
                && interaction("native-permission").is_some()
        })
        .await?;
        let permission = interaction("native-permission").unwrap();
        permission.response("accept");
        let question_scope = crate::interaction_ui::Scope::new(
            "opencode",
            ui.state.borrow().socket_generation,
            &json!("question:native-question"),
        )
        .unwrap();
        until(|| ui.interactions.borrow().contains_key(&question_scope)).await?;
        let question = ui
            .interactions
            .borrow()
            .get(&question_scope)
            .cloned()
            .unwrap();
        question.smoke_set(0, crate::interaction::Input::Choices(vec![0]))?;
        question.smoke_click("Submit")?;
        until(|| {
            std::fs::read_to_string(directory.join("opencode-requests.jsonl"))
                .unwrap_or_default()
                .contains("/question/native-question/reply")
        })
        .await?;
        glib::timeout_future(Duration::from_millis(250)).await;
        if ui.state.borrow().active_turn.is_none() || ui.send.is_sensitive() {
            return Err("Assistant completion invented OpenCode idle status".into());
        }
        capture(&ui.window, &directory.join("opencode.png"));
        ui.interrupt();
        until(|| ui.state.borrow().active_turn.is_none() && ui.send.is_sensitive()).await?;
        queues::smoke_opencode(ui, directory).await?;
        attachment_controller::smoke_opencode(ui, directory).await?;
        refs::smoke_opencode(ui, directory).await?;
        commands::smoke_opencode(ui, directory, |dialog, name| {
            capture_dialog(dialog, &directory.join(name))
        })
        .await?;
        ui.composer
            .buffer()
            .set_text("Unsaved OpenCode history draft");
        let before = {
            let state = ui.state.borrow();
            (
                state.backend.clone(),
                state.selected.clone(),
                state
                    .items
                    .iter()
                    .map(|item| item.id.clone())
                    .collect::<Vec<_>>(),
                state.active_turn.clone(),
                ui.composer_text(),
            )
        };
        ui.finder.smoke_opencode_history().await?;
        until(|| {
            ui.history_context
                .borrow()
                .as_ref()
                .is_some_and(|view| view.contains("oc-old-answer-part"))
        })
        .await?;
        let context = ui.history_context.borrow().clone().unwrap();
        crate::window_capture::capture_dialog(
            &ui.window,
            &context.dialog,
            &directory.join("opencode-history-context.png"),
        )
        .await?;
        context.dialog.close();
        ui.finder.close();
        std::fs::write(directory.join("native-oc-no-cursor.flag"), "test")
            .map_err(|error| error.to_string())?;
        ui.finder.smoke_opencode_legacy().await?;
        std::fs::remove_file(directory.join("native-oc-no-cursor.flag"))
            .map_err(|error| error.to_string())?;
        std::fs::write(directory.join("native-oc-repeat-cursor.flag"), "test")
            .map_err(|error| error.to_string())?;
        ui.finder.smoke_opencode_cycle().await?;
        std::fs::remove_file(directory.join("native-oc-repeat-cursor.flag"))
            .map_err(|error| error.to_string())?;
        ui.finder.close();
        ui.finder.clear();
        let unchanged = {
            let state = ui.state.borrow();
            state.backend == before.0
                && state.selected == before.1
                && state
                    .items
                    .iter()
                    .map(|item| item.id.clone())
                    .collect::<Vec<_>>()
                    == before.2
                && state.active_turn == before.3
                && ui.composer_text() == before.4
        };
        if !unchanged {
            return Err("OpenCode history search modified the live session or draft".into());
        }
        std::fs::write(directory.join("opencode-history.json"),serde_json::to_vec_pretty(&json!({"ok":true,"olderPages":true,"unicodeLiteral":true,"legacyScope":true,"cursorCycle":true,"readOnlyContext":true,"liveConversationUnchanged":true})).unwrap()).map_err(|error|error.to_string())?;
        ui.composer.buffer().set_text("native-oc-background");
        ui.submit();
        until(|| !ui.state.borrow().sending && ui.state.borrow().active_turn.is_some()).await?;
        let codex_index = ui
            .state
            .borrow()
            .backends
            .iter()
            .position(|backend| backend.id == "codex")
            .unwrap();
        ui.backend.set_selected(codex_index as u32);
        until(|| ui.state.borrow().ready && ui.state.borrow().backend == "codex").await?;
        std::fs::write(
            directory.join("release-native-oc-background.flag"),
            "offscreen",
        )
        .map_err(|error| error.to_string())?;
        until(|| {
            ui.state
                .borrow()
                .activity
                .get("opencode:native-oc-thread")
                .is_some_and(|activity| activity.attention && !activity.active)
        })
        .await?;
        let expected_attention = {
            let state = ui.state.borrow();
            state
                .threads
                .iter()
                .filter(|thread| {
                    state
                        .activity
                        .get(&format!("codex:{}", thread.id))
                        .is_some_and(|activity| activity.attention)
                })
                .count()
        };
        ui.scope.set_selected(4);
        until(|| {
            !ui.state.borrow().catalog_loading
                && ui.sessions.n_items() as usize == expected_attention
        })
        .await?;
        ui.scope.set_selected(0);
        ui.backend.set_selected(index as u32);
        until(|| {
            ui.state.borrow().ready
                && !ui.state.borrow().catalog_loading
                && ui
                    .state
                    .borrow()
                    .threads
                    .iter()
                    .any(|thread| thread.id == "native-oc-thread")
        })
        .await?;
        let thread = ui
            .state
            .borrow()
            .threads
            .iter()
            .find(|thread| thread.id == "native-oc-thread")
            .cloned()
            .unwrap();
        ui.select(thread);
        until(|| !ui.state.borrow().loading && ui.send.is_sensitive()).await?;
        ui.rename_session();
        let rename = gtk::Window::list_toplevels()
            .into_iter()
            .filter_map(|window| window.downcast::<adw::MessageDialog>().ok())
            .find(|dialog| dialog.heading().as_deref() == Some("Rename session"))
            .ok_or("OpenCode rename dialog missing")?;
        rename
            .extra_child()
            .and_downcast::<gtk::Entry>()
            .unwrap()
            .set_text("Renamed OpenCode session");
        rename.response("rename");
        until(|| {
            !ui.state.borrow().session_mutation
                && ui
                    .state
                    .borrow()
                    .selected
                    .as_ref()
                    .is_some_and(|thread| thread.title == "Renamed OpenCode session")
        })
        .await?;
        ui.fork_session();
        until(|| {
            !ui.state.borrow().session_mutation
                && !ui.state.borrow().loading
                && ui.state.borrow().options_loaded
                && !ui.state.borrow().catalog_loading
                && ui
                    .state
                    .borrow()
                    .selected
                    .as_ref()
                    .is_some_and(|thread| thread.id == "native-oc-fork")
        })
        .await?;
        let archive = ui
            .window
            .lookup_action("archive-session")
            .and_downcast::<gio::SimpleAction>()
            .unwrap();
        if archive.is_enabled() {
            return Err("OpenCode exposes unsupported archive action".into());
        }
        ui.delete_session();
        let deletion = gtk::Window::list_toplevels()
            .into_iter()
            .filter_map(|window| window.downcast::<adw::MessageDialog>().ok())
            .find(|dialog| dialog.heading().as_deref() == Some("Delete session permanently?"))
            .ok_or("OpenCode deletion confirmation missing")?;
        deletion.response("delete");
        until(|| {
            !ui.state.borrow().session_mutation
                && !ui.state.borrow().catalog_loading
                && ui.state.borrow().selected.is_none()
        })
        .await?;
        let primary = ui
            .state
            .borrow()
            .threads
            .iter()
            .find(|thread| thread.id == "native-oc-thread")
            .cloned()
            .ok_or("OpenCode original session was lost")?;
        ui.select(primary);
        until(|| !ui.state.borrow().loading && ui.send.is_sensitive()).await?;
        Ok(
            json!({"ok":true,"history":true,"sseUnicode":true,"structuredOutput":true,"fastIdleOrdering":true,"authoritativeBusy":true,"stop":true,"permissionOnce":true,"questionChoice":true,"sharedPins":true,"backgroundApproval":true,"backgroundSse":true,"attentionFilter":true,"renameForkDelete":true}),
        )
    }

    pub(super) fn start(ui: &Rc<Ui>) {
        let Some(directory) = std::env::var_os("STUDIO_GNOME_SMOKE_DIR").map(PathBuf::from) else {
            return;
        };
        // Automated sends are restricted to the private fake-backend fixture.
        if std::env::var_os("CODEX_THREAD_STUDIO_CODEX_BIN").map(PathBuf::from)
            != Some(directory.join("fake-codex.py"))
            || std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from)
                != Some(directory.join("config"))
        {
            eprintln!("Native smoke test requires its isolated fake backend and profile");
            ui.window.close();
            return;
        }
        crate::window_capture::observe_window(&ui.window);
        if std::env::var_os("STUDIO_GNOME_SMOKE_CAPTURE_ONLY").is_some() {
            crate::window_capture::observe_requests(&ui.window, &directory);
            crate::window_capture::observe_widget_requests(
                &ui.window,
                &ui.turn_navigation.capture_widget(),
                &directory,
                "native-turn-capture-request",
            );
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_EDIT_FIND").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let value = ui
                    .source
                    .smoke_edit_find(&directory, |name| {
                        capture(&ui.window, &directory.join(name))
                    })
                    .await
                    .unwrap_or_else(|e| json!({"ok":false,"error":e}));
                let path = directory.join("edit-find-result.json");
                let bytes = serde_json::to_vec_pretty(&value).unwrap();
                let _ = gio::spawn_blocking(move || std::fs::write(path, bytes)).await;
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_SOURCE_RELOAD").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let mut value = ui
                    .source
                    .smoke_reload(&directory)
                    .await
                    .unwrap_or_else(|e| json!({"ok":false,"error":e}));
                value["eventMaxMs"] = json!(ui.event_max_ms.get());
                let path = directory.join("source-reload-result.json");
                let bytes = serde_json::to_vec_pretty(&value).unwrap();
                let _ = gio::spawn_blocking(move || std::fs::write(path, bytes)).await;
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_HTML").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let mut value = ui
                    .source
                    .smoke_html(&directory)
                    .await
                    .unwrap_or_else(|e| json!({"ok":false,"error":e}));
                value["eventMaxMs"] = json!(ui.event_max_ms.get());
                let path = directory.join("html-result.json");
                let bytes = serde_json::to_vec_pretty(&value).unwrap();
                let _ = gio::spawn_blocking(move || std::fs::write(path, bytes)).await;
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_STRUCTURED").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let mut value = ui
                    .source
                    .smoke_structured(&directory)
                    .await
                    .unwrap_or_else(|e| json!({"ok":false,"error":e}));
                value["eventMaxMs"] = json!(ui.event_max_ms.get());
                let path = directory.join("structured-result.json");
                let bytes = serde_json::to_vec_pretty(&value).unwrap();
                let _ = gio::spawn_blocking(move || std::fs::write(path, bytes)).await;
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_SOURCE_EDIT").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let value = ui
                    .source
                    .smoke_edit(&directory, |name| {
                        capture(&ui.window, &directory.join(name))
                    })
                    .await
                    .unwrap_or_else(|e| json!({"ok":false,"error":e}));
                let path = directory.join("source-edit-result.json");
                let bytes = serde_json::to_vec_pretty(&value).unwrap();
                let _ = gio::spawn_blocking(move || std::fs::write(path, bytes)).await;
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_SOURCE_SEARCH").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let mut value = ui
                    .source
                    .smoke_search(&directory, |name| {
                        capture(&ui.window, &directory.join(name))
                    })
                    .await
                    .unwrap_or_else(|e| json!({"ok":false,"error":e}));
                value["eventMaxMs"] = json!(ui.event_max_ms.get());
                value["renderMaxMs"] = json!(ui.render_max_ms.get());
                let path = directory.join("source-search-result.json");
                let bytes = serde_json::to_vec_pretty(&value).unwrap();
                let _ = gio::spawn_blocking(move || std::fs::write(path, bytes)).await;
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_SOURCE_COMMENTS").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let value = source_comment_workflow_acceptance::run(&ui, &directory, |name| {
                    capture(&ui.window, &directory.join(name))
                })
                .await
                .unwrap_or_else(|e| json!({"ok":false,"error":e}));
                let path = directory.join("source-comments-result.json");
                let bytes = serde_json::to_vec_pretty(&value).unwrap();
                let _ = gio::spawn_blocking(move || std::fs::write(path, bytes)).await;
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_CHAT_COMMENTS").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let value = chat_comment_acceptance::run(&ui, &directory, |name| {
                    capture(&ui.window, &directory.join(name))
                })
                .await
                .unwrap_or_else(|error| json!({"ok":false,"error":error}));
                let path = directory.join("chat-comments-result.json");
                let bytes = serde_json::to_vec_pretty(&value).unwrap();
                let _ = gio::spawn_blocking(move || std::fs::write(path, bytes)).await;
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_COMMENT_LOCATION").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let value = annotation_location_acceptance::run(&ui, &directory, |name| {
                    capture(&ui.window, &directory.join(name))
                })
                .await
                .unwrap_or_else(|error| json!({"ok":false,"error":error}));
                let path = directory.join("comment-location-result.json");
                let bytes = serde_json::to_vec_pretty(&value).unwrap();
                let _ = gio::spawn_blocking(move || std::fs::write(path, bytes)).await;
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_ACTIVITY").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let value = activity_details_acceptance::run(&ui, &directory)
                    .await
                    .unwrap_or_else(|error| {
                        capture(&ui.window, &directory.join("activity-failure.png"));
                        json!({"ok":false,"error":error})
                    });
                let path = directory.join("activity-result.json");
                let bytes = serde_json::to_vec_pretty(&value).unwrap();
                let _ = gio::spawn_blocking(move || std::fs::write(path, bytes)).await;
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_CONNECTIONS").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let value=connections_acceptance::run(&ui,&directory).await.unwrap_or_else(|error|{
                    capture(&ui.window,&directory.join("connections-failure.png"));
                    json!({"ok":false,"error":error,"panel":ui.connections.borrow().as_ref().map(|p|p.smoke_state())})
                });
                let path = directory.join("connections-result.json");
                let bytes = serde_json::to_vec_pretty(&value).unwrap();
                let _ = gio::spawn_blocking(move || std::fs::write(path, bytes)).await;
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_SESSION_INFO").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let value = session_info_acceptance::run(&ui, &directory, |dialog, name| {
                    capture_dialog(dialog, &directory.join(name))
                })
                .await
                .unwrap_or_else(|error| {
                    capture(&ui.window, &directory.join("session-info-failure.png"));
                    let panel = ui.session_info.borrow().as_ref().map(|p| p.smoke_state());
                    json!({"ok":false,"error":error,"panel":panel})
                });
                let path = directory.join("session-info-result.json");
                let bytes = serde_json::to_vec_pretty(&value).unwrap();
                let _ = gio::spawn_blocking(move || std::fs::write(path, bytes)).await;
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_RESOURCES").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let value = resource_acceptance::run(&ui, &directory, |name| {
                    capture(&ui.window, &directory.join(name))
                })
                .await
                .unwrap_or_else(|error| {
                    capture(&ui.window, &directory.join("resources-failure.png"));
                    json!({"ok":false,"error":error,"panel":ui.resources.smoke_state()})
                });
                let path = directory.join("resources-result.json");
                let bytes = serde_json::to_vec_pretty(&value).unwrap();
                let _ = gio::spawn_blocking(move || std::fs::write(path, bytes)).await;
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_WORKSPACE_NAVIGATION").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let value = navigation_acceptance::run(&ui, &directory, |name| {
                    capture(&ui.window, &directory.join(name))
                })
                .await
                .unwrap_or_else(|error| json!({"ok":false,"error":error}));
                let path = directory.join("navigation-result.json");
                let bytes = serde_json::to_vec_pretty(&value).unwrap();
                let _ = gio::spawn_blocking(move || std::fs::write(path, bytes)).await;
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_BACKENDS").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let value = backend_settings_acceptance::run(&ui, &directory)
                    .await
                    .unwrap_or_else(|error| json!({"ok":false,"error":error}));
                let path = directory.join("backend-config-result.json");
                let bytes = serde_json::to_vec_pretty(&value).unwrap();
                let _ = gio::spawn_blocking(move || std::fs::write(path, bytes)).await;
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_LANGUAGE").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let result = language_acceptance::run(&ui, &directory).await;
                let value = result.unwrap_or_else(|error| json!({"ok":false,"error":error}));
                let bytes = serde_json::to_vec_pretty(&value).unwrap();
                let path = directory.join("language-result.json");
                let _ = gio::spawn_blocking(move || std::fs::write(path, bytes)).await;
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_OPENCODE_INPUT").is_some() {
            if let Some(settings) = gtk::Settings::default() {
                settings.set_gtk_enable_animations(false);
            }
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let result = opencode_input_acceptance::run(&ui, &directory).await;
                let value = result.unwrap_or_else(|error| json!({"ok":false,"error":error}));
                let _ = std::fs::write(
                    directory.join("opencode-input.json"),
                    serde_json::to_vec_pretty(&value).unwrap(),
                );
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_INPUT").is_some() {
            if let Some(settings) = gtk::Settings::default() {
                settings.set_gtk_enable_animations(false);
            }
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let result = interaction_acceptance::run(&ui, &directory).await;
                let value = result.unwrap_or_else(|error| json!({"ok":false,"error":error}));
                let _ = std::fs::write(
                    directory.join("inputs.json"),
                    serde_json::to_vec_pretty(&value).unwrap(),
                );
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_TRANSLATION").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let result = translation_acceptance::run(&ui, &directory).await;
                let value = result.unwrap_or_else(|error| json!({"ok":false,"error":error}));
                let path = directory.join("translation-result.json");
                let bytes = serde_json::to_vec_pretty(&value).unwrap();
                let _ = gio::spawn_blocking(move || std::fs::write(path, bytes)).await;
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if let Ok(phase) = std::env::var("STUDIO_GNOME_SMOKE_MAP") {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let result = session_map_acceptance::run(&ui, &directory, &phase).await;
                let value = result.unwrap_or_else(|error| json!({"ok":false,"error":error}));
                let path = directory.join("session-map-result.json");
                let bytes = serde_json::to_vec_pretty(&value).unwrap();
                let _ = gio::spawn_blocking(move || std::fs::write(path, bytes)).await;
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_SUPERVISION").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let mut screen = |name: &str| {
                    capture(&ui.window, &directory.join(name));
                    Ok(())
                };
                let result = if std::env::var("STUDIO_GNOME_SMOKE_SUPERVISION").as_deref()
                    == Ok("restore")
                {
                    supervision_acceptance::restore(&ui, &directory, &mut screen).await
                } else {
                    supervision_acceptance::run(&ui, &directory, &mut screen).await
                };
                let value = result.unwrap_or_else(|error| json!({"ok":false,"error":error}));
                let path = directory.join("supervision-result.json");
                let bytes = serde_json::to_vec_pretty(&value).unwrap();
                let _ = gio::spawn_blocking(move || std::fs::write(path, bytes)).await;
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_MAP_SYNC").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let result = session_map_sync_acceptance::run(&ui, &directory).await;
                let value = result.unwrap_or_else(|error| json!({"ok":false,"error":error}));
                let path = directory.join("map-sync-result.json");
                let bytes = serde_json::to_vec_pretty(&value).unwrap();
                let _ = gio::spawn_blocking(move || std::fs::write(path, bytes)).await;
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_ROUTER_MAP").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let result = router_map_acceptance::run(&ui, &directory).await;
                let value = result.unwrap_or_else(|error| json!({"ok":false,"error":error}));
                let path = directory.join("routed-map-result.json");
                let bytes = serde_json::to_vec_pretty(&value).unwrap();
                let _ = gio::spawn_blocking(move || std::fs::write(path, bytes)).await;
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_BROWSER_SETTINGS").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let result = browser_settings_acceptance::run(&ui, &directory).await;
                let value = result.unwrap_or_else(|error| json!({"ok":false,"error":error}));
                let _ = std::fs::write(
                    directory.join("browser-settings.json"),
                    serde_json::to_vec_pretty(&value).unwrap(),
                );
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_FILES").is_some() {
            if let Some(settings) = gtk::Settings::default() {
                settings.set_gtk_enable_animations(false);
            }
            let ui = ui.clone();
            ui.show_tool("files");
            glib::spawn_future_local(async move {
                let result = ui
                    .files
                    .smoke_test(&directory.join("project"), || {
                        capture(&ui.window, &directory.join("files-tree.png"))
                    })
                    .await;
                let result = if std::env::var_os("STUDIO_GNOME_SMOKE_FILES_SHARED").is_some() {
                    match result {
                        Ok(mut value) => {
                            let shared = directory.join("shared docs 中文 #");
                            match ui
                                .files
                                .smoke_shared(&directory.join("project"), &shared, || {
                                    capture(&ui.window, &directory.join("files-shared-menu.png"))
                                })
                                .await
                            {
                                Ok(shared_result) => {
                                    value["shared"] = shared_result;
                                    match ui
                                        .reader
                                        .smoke_shared_read_only(
                                            &directory.join("project"),
                                            &shared.join("notes.md"),
                                            || {
                                                capture(
                                                    &ui.window,
                                                    &directory.join("files-shared-reader.png"),
                                                )
                                            },
                                        )
                                        .await
                                    {
                                        Ok(reader) => {
                                            value["sharedReader"] = reader;
                                            value["ok"] = json!(
                                                value["shared"]["ok"] == true
                                                    && value["sharedReader"]["ok"] == true
                                            );
                                            Ok(value)
                                        }
                                        Err(error) => Err(error),
                                    }
                                }
                                Err(error) => Err(error),
                            }
                        }
                        Err(error) => Err(error),
                    }
                } else {
                    result
                };
                let value = match result {
                    Ok(value) => value,
                    Err(error) => json!({"ok":false,"error":error}),
                };
                let _ = std::fs::write(
                    directory.join("files-ui.json"),
                    serde_json::to_vec_pretty(&value).unwrap(),
                );
                if value["ok"] != true {
                    if let Some(app) = ui.window.application() {
                        app.quit();
                    }
                } else {
                    ui.window.close();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_HISTORY_IMAGES").is_some() {
            if let Some(settings) = gtk::Settings::default() {
                settings.set_gtk_enable_animations(false);
            }
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let result = history_images_acceptance::run(&ui, &directory, |name| {
                    capture(&ui.window, &directory.join(name))
                })
                .await;
                let value = match result {
                    Ok(value) => value,
                    Err(error) => json!({"ok":false,"error":error}),
                };
                let _ = std::fs::write(
                    directory.join("history-images.json"),
                    serde_json::to_vec_pretty(&value).unwrap(),
                );
                if value["ok"] != true {
                    if let Some(app) = ui.window.application() {
                        app.quit();
                    }
                } else {
                    ui.window.close();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_LAYOUT").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let result = layout_acceptance::run(&ui, &directory, |name| {
                    capture(&ui.window, &directory.join(name))
                })
                .await;
                let value = match result {
                    Ok(value) => value,
                    Err(error) => json!({"ok":false,"error":error}),
                };
                let _ = std::fs::write(
                    directory.join("layout.json"),
                    serde_json::to_vec_pretty(&value).unwrap(),
                );
                if value["ok"] != true {
                    if let Some(app) = ui.window.application() {
                        app.quit();
                    }
                } else {
                    ui.window.close();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_INLINE_LABELS").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let value = inline_math_acceptance::run(&ui, &directory)
                    .await
                    .unwrap_or_else(|error| json!({"ok":false,"error":error}));
                let path = directory.join("inline-labels.json");
                let bytes = serde_json::to_vec_pretty(&value).unwrap();
                let _ = gio::spawn_blocking(move || std::fs::write(path, bytes)).await;
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_WRITE_FAILURES").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let at = std::time::Instant::now();
                while !ui.state.borrow().history_verified && at.elapsed() < Duration::from_secs(25)
                {
                    glib::timeout_future(Duration::from_millis(25)).await;
                }
                ui.composer
                    .buffer()
                    .set_text("Unsent write failure draft 中文😀 ");
                let result=async {
                    crate::smoke_input::request(&directory,json!({"action":"focus"})).await?;
                    let output=ui.output.smoke_write_failures(&directory,||ui.window.close()).await?;
                    let favorites=ui.favorites_library().smoke_write_failures(&directory).await?;
                    if ui.composer_text()!="Unsent write failure draft 中文😀 " {return Err("Write failure handling changed the conversation draft".to_string());}
                    if ui.status.text()=="Wait for the output export to finish before closing"||ui.write_feedback.smoke_waiting("output"){return Err("Completed exports left a stale close notice".to_string());}
                    Ok::<_,String>(json!({"ok":true,"output":output,"favorites":favorites,"draftRetained":true}))
                }.await;
                let value = result.unwrap_or_else(|error| json!({"ok":false,"error":error}));
                let bytes = serde_json::to_vec_pretty(&value).unwrap();
                let path = directory.join("write-failures.json");
                let _ = gio::spawn_blocking(move || std::fs::write(path, bytes)).await;
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_SAVE_PORTAL").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let at = std::time::Instant::now();
                while !ui.state.borrow().history_verified && at.elapsed() < Duration::from_secs(25)
                {
                    glib::timeout_future(Duration::from_millis(25)).await;
                }
                ui.composer
                    .buffer()
                    .set_text("Unsent output export draft 中文😀 ");
                let value = match ui
                    .output
                    .smoke_save_portal(
                        &directory,
                        |dialog| {
                            capture_dialog(dialog, &directory.join("output-save-viewer.png"));
                        },
                        || ui.window.close(),
                    )
                    .await
                {
                    Ok(mut value) => {
                        let library = ui.favorites_library();
                        let favorites_ok = match library
                            .smoke_save_portal(
                                &directory,
                                || ui.window.close(),
                                |dialog| {
                                    capture_dialog(
                                        dialog,
                                        &directory.join("favorites-save-viewer.png"),
                                    )
                                },
                            )
                            .await
                        {
                            Ok(favorites) => {
                                value["favorites"] = favorites;
                                true
                            }
                            Err(error) => {
                                capture(&ui.window, &directory.join("favorites-save-error.png"));
                                value["error"] = json!(error);
                                false
                            }
                        };
                        value["ok"] = json!(
                            favorites_ok
                                && ui.composer_text() == "Unsent output export draft 中文😀 "
                        );
                        value
                    }
                    Err(error) => json!({"ok":false,"error":error}),
                };
                let path = directory.join("save-portal-ui.json");
                let bytes = serde_json::to_vec_pretty(&value).unwrap();
                let _ = gio::spawn_blocking(move || std::fs::write(path, bytes)).await;
                if value["ok"] == true {
                    ui.window.close();
                    glib::timeout_future(Duration::from_millis(300)).await;
                    if ui.window.is_mapped() {
                        capture(&ui.window, &directory.join("save-portal-close-blocked.png"));
                        let state = json!({
                            "favoritesBusy":ui.favorites.borrow().as_ref().map(|f|f.busy()),
                            "dialog":ui.window.visible_dialog().map(|d|d.title().to_string()),
                            "sessionMutation":ui.state.borrow().session_mutation,
                        });
                        let path = directory.join("save-portal-close-blocked.json");
                        let bytes = serde_json::to_vec_pretty(&state).unwrap();
                        let _ = gio::spawn_blocking(move || std::fs::write(path, bytes)).await;
                        if let Some(app) = ui.window.application() {
                            app.quit();
                        }
                    }
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_FILE_PORTAL").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let value = match file_portal_acceptance::run(&ui, &directory).await {
                    Ok(value) => value,
                    Err(error) => json!({"ok":false,"error":error}),
                };
                let path = directory.join("file-portal-ui.json");
                let encoded = serde_json::to_vec_pretty(&value).unwrap();
                let _ = gio::spawn_blocking(move || std::fs::write(path, encoded)).await;
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_FALLBACK").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let value = match fallback_acceptance::run(&ui, &directory).await {
                    Ok(value) => value,
                    Err(error) => json!({"ok":false,"error":error}),
                };
                let _ = std::fs::write(
                    directory.join("fallback.json"),
                    serde_json::to_vec_pretty(&value).unwrap(),
                );
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_STREAMING").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let value = match streaming_acceptance::run(&ui, &directory).await {
                    Ok(value) => value,
                    Err(error) => json!({"ok":false,"error":error}),
                };
                let _ = std::fs::write(
                    directory.join("streaming.json"),
                    serde_json::to_vec_pretty(&value).unwrap(),
                );
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_TABLE_IMAGES").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let value = match table_image_acceptance::run(&ui, &directory).await {
                    Ok(value) => value,
                    Err(error) => json!({"ok":false,"error":error}),
                };
                let _ = std::fs::write(
                    directory.join("table-images.json"),
                    serde_json::to_vec_pretty(&value).unwrap(),
                );
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_EMBEDDED_IMAGES").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let value = match embedded_image_acceptance::run(&ui, &directory).await {
                    Ok(value) => value,
                    Err(error) => json!({"ok":false,"error":error}),
                };
                let _ = std::fs::write(
                    directory.join("embedded-images.json"),
                    serde_json::to_vec_pretty(&value).unwrap(),
                );
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_REMOTE_IMAGES").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let value = remote_image_acceptance::run(&ui, &directory)
                    .await
                    .unwrap_or_else(|error| json!({"ok":false,"error":error}));
                let bytes = serde_json::to_vec_pretty(&value).unwrap();
                let path = directory.join("remote-images.json");
                let _ = gio::spawn_blocking(move || std::fs::write(path, bytes)).await;
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_TABLE_CELLS").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let value = match table_cell_acceptance::run(&ui, &directory).await {
                    Ok(value) => value,
                    Err(error) => json!({"ok":false,"error":error}),
                };
                let _ = std::fs::write(
                    directory.join("table-cells.json"),
                    serde_json::to_vec_pretty(&value).unwrap(),
                );
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_RICH_TEXT").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let result = rich_text_acceptance::run(&ui, &directory, |name| {
                    capture(&ui.window, &directory.join(name))
                })
                .await;
                let value = match result {
                    Ok(value) => value,
                    Err(error) => json!({"ok":false,"error":error}),
                };
                let _ = std::fs::write(
                    directory.join("rich-text.json"),
                    serde_json::to_vec_pretty(&value).unwrap(),
                );
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_OUTPUT").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let result = output_acceptance::run(&ui, &directory).await;
                let value = match result {
                    Ok(value) => value,
                    Err(error) => json!({"ok":false,"error":error}),
                };
                let _ =
                    crate::window_capture::capture(&ui.window, &directory.join("output-main.png"))
                        .await;
                let _ = std::fs::write(
                    directory.join("outputs.json"),
                    serde_json::to_vec_pretty(&value).unwrap(),
                );
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_FLOW_PIPELINE").is_some()
            || std::env::var_os("STUDIO_GNOME_SMOKE_FLOW_INPUT").is_some()
        {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                // This fixture invokes the reader actor directly, rather than
                // using normal file activation's startup deferral.
                let result = async {
                    crate::acceptance_wait::until("reader gateway initialization", || {
                        ui.state.borrow().startup_ready
                    })
                    .await?;
                    if std::env::var_os("STUDIO_GNOME_SMOKE_FLOW_INPUT").is_some() {
                        ui.reader.smoke_flow_input(&directory).await
                    } else {
                        ui.reader.smoke_flow_pipeline(&directory).await
                    }
                }
                .await;
                let mut value = result.unwrap_or_else(|error| json!({"ok":false,"error":error}));
                value["eventMaxMs"] = json!(ui.event_max_ms.get());
                value["renderMaxMs"] = json!(ui.render_max_ms.get());
                value["renderer"] =
                    json!(ui.window.renderer().map(|r| r.type_().name().to_string()));
                let path = directory.join(
                    if std::env::var_os("STUDIO_GNOME_SMOKE_FLOW_INPUT").is_some() {
                        "flow-input.json"
                    } else {
                        "flow-pipeline.json"
                    },
                );
                let bytes = serde_json::to_vec_pretty(&value).unwrap();
                let _ = gio::spawn_blocking(move || std::fs::write(path, bytes)).await;
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_DISCOVERY").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let value = startup_acceptance::run(&ui, &directory)
                    .await
                    .unwrap_or_else(|error| json!({"ok":false,"error":error}));
                let path = directory.join("discovery.json");
                let bytes = serde_json::to_vec_pretty(&value).unwrap();
                let _ = gio::spawn_blocking(move || std::fs::write(path, bytes)).await;
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_BACKEND_RECOVERY").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let result = backend_recovery_acceptance::run(&ui, &directory).await;
                let value = result.unwrap_or_else(|error| json!({"ok":false,"error":error}));
                let path = directory.join("backend-recovery.json");
                let bytes = serde_json::to_vec_pretty(&value).unwrap();
                let _ = gio::spawn_blocking(move || std::fs::write(path, bytes)).await;
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_HYDRATION").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let result = hydration_acceptance::run(&ui, &directory, |name| {
                    capture(&ui.window, &directory.join(name))
                })
                .await;
                let value = match result {
                    Ok(v) => v,
                    Err(error) => json!({"ok":false,"error":error}),
                };
                let _ = std::fs::write(
                    directory.join("hydration.json"),
                    serde_json::to_vec_pretty(&value).unwrap(),
                );
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_HISTORY_PAGES").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let result = conversation_pages_acceptance::run(&ui, &directory, |name| {
                    capture(&ui.window, &directory.join(name))
                })
                .await;
                let value = match result {
                    Ok(v) => v,
                    Err(error) => json!({"ok":false,"error":error}),
                };
                let _ = std::fs::write(
                    directory.join("history-pages.json"),
                    serde_json::to_vec_pretty(&value).unwrap(),
                );
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if let Ok(phase) = std::env::var("STUDIO_GNOME_SMOKE_NOTIFICATIONS") {
            if let Some(settings) = gtk::Settings::default() {
                settings.set_gtk_enable_animations(false);
            }
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let result =
                    notification_acceptance::run(&ui, &directory, phase == "restore").await;
                let value = match result {
                    Ok(v) => v,
                    Err(e) => json!({"ok":false,"error":e}),
                };
                let _ = std::fs::write(
                    directory.join(format!("notifications-{phase}.json")),
                    serde_json::to_vec_pretty(&value).unwrap(),
                );
                if value["ok"] != true {
                    if let Some(app) = ui.window.application() {
                        app.quit();
                    }
                } else {
                    ui.window.close();
                }
            });
            return;
        }
        if let Ok(phase) = std::env::var("STUDIO_GNOME_SMOKE_VISIBILITY") {
            if let Some(settings) = gtk::Settings::default() {
                settings.set_gtk_enable_animations(false);
            }
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let result: Result<Value, String> = async {
                    async fn until(test: impl Fn() -> bool) -> Result<(), String> {
                        let started = std::time::Instant::now();
                        while !test() {
                            if started.elapsed() > Duration::from_secs(12) {
                                return Err("Visibility app acceptance timed out".into());
                            }
                            glib::timeout_future(Duration::from_millis(25)).await;
                        }
                        Ok(())
                    }
                    let ids = || {
                        (0..ui.sessions.n_items())
                            .filter_map(|i| {
                                ui.sessions
                                    .item(i)
                                    .and_downcast::<glib::BoxedAnyObject>()
                                    .map(|o| o.borrow::<Thread>().id.clone())
                            })
                            .collect::<HashSet<_>>()
                    };
                    until(|| {
                        ui.state
                            .borrow()
                            .threads
                            .iter()
                            .any(|t| t.id == "build-keep")
                            && (if phase=="recovery"{ui.state.borrow().visibility.is_none()&&!ui.state.borrow().visibility_revision.is_empty()}else{ids().contains("target")})
                    })
                    .await?;
                    let mut catalogue=Value::Null;
                    if std::env::var_os("STUDIO_GNOME_SMOKE_VISIBILITY_LARGE").is_some() {
                        let ticks=Rc::new(std::cell::Cell::new(0u64));let maximum=Rc::new(std::cell::Cell::new(0u128));
                        let previous=Rc::new(std::cell::Cell::new(std::time::Instant::now()));
                        let t=ticks.clone();let m=maximum.clone();let p=previous.clone();
                        let timer=glib::timeout_add_local(Duration::from_millis(10),move||{t.set(t.get()+1);m.set(m.get().max(p.replace(std::time::Instant::now()).elapsed().as_millis()));glib::ControlFlow::Continue});
                        let started=std::time::Instant::now();
                        let tested:Result<(),String>=async{
                            while ui.state.borrow().cursor.is_some(){
                                until(||ui.more.is_sensitive()).await?;
                                let before=ui.state.borrow().threads.len();ui.more.emit_clicked();
                                until(||ui.state.borrow().threads.len()>before).await?;
                            }
                            let expected=if phase=="restore" {9998}else{9999};
                            until(||ui.sessions.n_items()==expected&&!ui.more.is_visible()).await?;
                            if !ui.state.borrow().catalog_truncated||!ui.status.text().contains("10,000"){return Err("Catalog budget was not disclosed".into());}
                            Ok(())
                        }.await;
                        timer.remove();tested?;
                        catalogue=json!({"loaded":ui.state.borrow().threads.len(),"available":10030,"visible":ui.sessions.n_items(),"limitDisclosed":true,"paginationMs":started.elapsed().as_millis(),"mainLoopTicks":ticks.get(),"maximumMainLoopGapMs":maximum.get()});
                    }
                    let target = ui
                        .state
                        .borrow()
                        .threads
                        .iter()
                        .find(|t| t.id == "target")
                        .cloned()
                        .unwrap();
                    ui.select(target);
                    if phase!="recovery" {until(|| !ui.state.borrow().loading && ui.state.borrow().items.iter().any(|item|item.id=="visibility-history")).await?;}
                    ui.composer.buffer().set_text("Unsaved visibility draft 中文😀");
                    ui.open_settings();
                    let settings = ui.settings.borrow().clone().unwrap();
                    if phase=="recovery" {let first=settings.smoke_visibility();first.smoke_close_replacement().await?;}
                    let panel = settings.smoke_visibility();
                    let mut result = if phase=="recovery" {
                        let history_loaded=std::cell::Cell::new(false);
                        panel.smoke_recovery(&directory,||{settings.dialog.close();!settings.may_close()&&settings.dialog.is_visible()},|valid|{
                            let visible=ids();let state=ui.state.borrow();
                            let history=state.items.iter().any(|item|item.id=="visibility-history");
                            if valid&&history&&!state.loading {history_loaded.set(true);}
                            let kept=state.selected.as_ref().is_some_and(|t|t.id=="target")&&(!history_loaded.get()||history)&&ui.composer_text()=="Unsaved visibility draft 中文😀";
                            kept&&if valid {history_loaded.get()&&state.visibility.is_some()&&visible.contains("target")&&visible.contains("build-keep")&&!visible.contains("hidden")&&!visible.contains("build-drop")}else{state.visibility.is_none()&&visible.is_empty()}
                        },|dialog,name|capture_dialog(dialog,&directory.join(name))).await?
                    }else{panel.smoke_test(
                            &directory,
                            phase == "restore",
                            || {
                                settings.dialog.close();
                                !settings.may_close() && settings.dialog.is_visible()
                            },
                            |stage| {
                                let visible = ids();
                                let kept = ui
                                    .state
                                    .borrow()
                                    .selected
                                    .as_ref()
                                    .is_some_and(|t| t.id == "target")
                                    && ui.state.borrow().items.iter().any(|item|item.id=="visibility-history")
                                    && ui.composer_text()=="Unsaved visibility draft 中文😀";
                                kept && !visible.contains("hidden")
                                    && visible.contains("build-keep")
                                    && visible.contains("target") == (stage != "hide")
                                    && visible.contains("build-drop") == (stage == "reset")
                            },
                            |dialog, name| capture_dialog(dialog, &directory.join(name)),
                        )
                        .await?};
                    panel.dialog.close();
                    settings.dialog.close();
                    let stale = ui.state.borrow().session_rows_serial;
                    ui.search.set_text("__missing_visibility_row__");
                    ui.render_sessions();
                    ui.event(Event::SessionRows {
                        serial: stale,
                        result: Ok(vec![Thread::from_value(
                            &json!({"id":"foreign-stale","cwd":"/foreign"}),
                        )
                        .unwrap()]),
                    });
                    if ids().contains("foreign-stale") {
                        return Err("Stale session rows changed the current list".into());
                    }
                    until(|| ui.sessions.n_items() == 0).await?;
                    ui.search.set_text("");
                    ui.render_sessions();
                    until(|| ids().contains("target")).await?;
                    capture(
                        &ui.window,
                        &directory.join(format!("visibility-list-{phase}.png")),
                    );
                    result["staleRowsIgnored"] = json!(true);
                    result["literalSearch"] = json!(true);
                    result["historyAndDraftPreserved"] = json!(true);
                    result["largeCatalog"] = catalogue;
                    ui.composer.buffer().set_text("");
                    Ok(result)
                }
                .await;
                let result = match result {
                    Ok(v) => v,
                    Err(e) => {
                        capture(&ui.window, &directory.join("visibility-error.png"));
                        let state = ui.state.borrow();
                        json!({"ok":false,"error":e,"backend":state.backend,"ready":state.ready,"threads":state.threads.iter().map(|t|&t.id).collect::<Vec<_>>(),"policyRevisionLoaded":!state.visibility_revision.is_empty(),"policyAvailable":state.visibility.is_some(),"selected":state.selected.as_ref().map(|t|&t.id),"loading":state.loading,"status":ui.status.text().to_string()})
                    }
                };
                let _ = std::fs::write(
                    directory.join(format!("visibility-{phase}.json")),
                    serde_json::to_vec_pretty(&result).unwrap(),
                );
                if result["ok"] != true {
                    if let Some(app) = ui.window.application() {
                        app.quit();
                    }
                } else {
                    ui.window.close();
                }
            });
            return;
        }
        if let Ok(phase) = std::env::var("STUDIO_GNOME_SMOKE_SHARED_SETTINGS") {
            if let Some(settings) = gtk::Settings::default() {
                settings.set_gtk_enable_animations(false);
            }
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let result: Result<Value, String> = async {
                    let root = directory.join("project").to_string_lossy().into_owned();
                    ui.files.open(&root);
                    ui.show_tool("files");
                    ui.open_settings();
                    let settings = ui.settings.borrow().clone().unwrap();
                    let panel = settings.smoke_shared();
                    let mut value = panel
                        .smoke_test(
                            &directory,
                            phase == "restore",
                            || {
                                settings.dialog.close();
                                !settings.may_close() && settings.dialog.is_visible()
                            },
                            |dialog, name| capture_dialog(dialog, &directory.join(name)),
                        )
                        .await?;
                    let shared = directory
                        .join("shared docs 中文 #")
                        .to_string_lossy()
                        .into_owned();
                    let started = std::time::Instant::now();
                    while !ui.files.smoke_shared_root(&shared) {
                        if started.elapsed() > Duration::from_secs(12) {
                            return Err("Files did not apply shared-folder settings live".into());
                        }
                        glib::timeout_future(Duration::from_millis(25)).await;
                    }
                    value["liveFilesRoots"] = json!(true);
                    panel.dialog.close();
                    settings.dialog.close();
                    capture(
                        &ui.window,
                        &directory.join(format!("shared-files-{phase}.png")),
                    );
                    Ok(value)
                }
                .await;
                let value = match result {
                    Ok(v) => v,
                    Err(e) => json!({"ok":false,"error":e}),
                };
                let _ = std::fs::write(
                    directory.join(format!("shared-settings-{phase}.json")),
                    serde_json::to_vec_pretty(&value).unwrap(),
                );
                if value["ok"] != true {
                    if let Some(app) = ui.window.application() {
                        app.quit();
                    }
                } else {
                    ui.window.close();
                }
            });
            return;
        }
        if let Ok(phase) = std::env::var("STUDIO_GNOME_SMOKE_ROUTER_RECOVERY") {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let result: Result<Value, String> = async {
                    let at = std::time::Instant::now();
                    while !ui.state.borrow().ready
                        || ui.state.borrow().loading
                        || !ui.state.borrow().options_loaded
                    {
                        if at.elapsed() > Duration::from_secs(15) {
                            return Err("Recovery fixture backend did not load".into());
                        }
                        glib::timeout_future(Duration::from_millis(20)).await;
                    }
                    ui.composer
                        .buffer()
                        .set_text("Unsaved controller recovery draft 中文😀");
                    ui.open_router_settings();
                    let panel = ui
                        .router_settings
                        .borrow()
                        .clone()
                        .ok_or("No Router settings")?;
                    let value = panel
                        .smoke_recovery(&directory, phase == "restore", |dialog, name| {
                            capture_dialog(dialog, &directory.join(name))
                        })
                        .await?;
                    if ui.composer_text() != "Unsaved controller recovery draft 中文😀" {
                        return Err("Controller recovery changed the conversation draft".into());
                    }
                    Ok(value)
                }
                .await;
                let value = result.unwrap_or_else(|error| json!({"ok":false,"error":error}));
                let _ = std::fs::write(
                    directory.join(format!("controller-recovery-{phase}.json")),
                    serde_json::to_vec_pretty(&value).unwrap(),
                );
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if let Ok(phase) = std::env::var("STUDIO_GNOME_SMOKE_APPEARANCE") {
            if let Some(settings) = gtk::Settings::default() {
                settings.set_gtk_enable_animations(false);
            }
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let result: Result<Value, String> = async {
                    async fn until(test: impl Fn() -> bool) -> Result<(), String> {
                        let start = std::time::Instant::now();
                        while !test() {
                            if start.elapsed() > Duration::from_secs(12) {
                                return Err("Shared appearance did not load".into());
                            }
                            glib::timeout_future(Duration::from_millis(25)).await;
                        }
                        Ok(())
                    }
                    until(|| ui.appearance.snapshot.borrow().is_some()).await?;
                    ui.open_settings();
                    let settings = ui.settings.borrow().clone().unwrap();
                    let panel = settings.smoke_appearance();
                    let mut value = panel.smoke_test(&directory, phase == "restore").await?;
                    panel.dialog.force_close();
                    if phase != "restore" {
                        let project = directory.join("project");
                        ui.activate_reference_from(
                            "diagram-settings.md",
                            &project.to_string_lossy(),
                        );
                        let diagrams = settings.smoke_appearance();
                        value["diagramPreferences"] = diagrams
                            .smoke_diagram_styles(&ui.chat_diagrams, &ui.reader, &directory)
                            .await?;
                        diagrams.dialog.force_close();
                    } else {
                        let snapshot = ui.appearance.snapshot.borrow().clone().unwrap();
                        if snapshot.values["mermaidFontSize"] != 20
                            || snapshot.values["mermaidDensity"] != "loose"
                            || snapshot.values["mermaidCurve"] != "step"
                            || snapshot.values["mermaidStyle"] != "handDrawn"
                            || snapshot.values["mermaidLayout"] != "elk"
                        {
                            return Err("Diagram preferences did not survive restart".into());
                        }
                        value["diagramPreferencesRestored"] = json!(true);
                    }
                    // Capture flushes the actual style/layout frame before reading Pango.
                    panel.dialog.force_close();
                    settings.dialog.force_close();
                    capture(&ui.window, &directory.join("appearance-applied.png"));
                    let font = ui
                        .composer
                        .pango_context()
                        .font_description()
                        .ok_or("Composer font missing")?;
                    if font.size() != 19 * gtk::pango::SCALE {
                        return Err(format!(
                            "Existing composer did not receive content typography: {font}"
                        ));
                    }
                    value["composerFont"] = json!(font.to_string());
                    Ok(value)
                }
                .await;
                let value = match result {
                    Ok(v) => v,
                    Err(e) => json!({"ok":false,"error":e}),
                };
                let _ = std::fs::write(
                    directory.join(format!("appearance-{phase}.json")),
                    serde_json::to_vec_pretty(&value).unwrap(),
                );
                let settings = ui.settings.borrow().clone();
                if let Some(settings) = settings {
                    settings.dialog.force_close();
                }
                // On a failed fixture, allow its private unsaved dialog to be destroyed.
                if value["ok"] != true {
                    if let Some(app) = ui.window.application() {
                        app.quit();
                    }
                } else {
                    ui.window.close();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_ANNOTATIONS").is_some() {
            if let Some(settings) = gtk::Settings::default() {
                settings.set_gtk_enable_animations(false);
            }
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let result: Result<Value, String> = async {
                    async fn until(test: impl Fn() -> bool) -> Result<(), String> {
                        let started = std::time::Instant::now();
                        while !test() {
                            if started.elapsed() > Duration::from_secs(15) {
                                return Err("Comment session did not become ready".into());
                            }
                            glib::timeout_future(Duration::from_millis(25)).await;
                        }
                        Ok(())
                    }

                    let submit = std::env::var("STUDIO_GNOME_SMOKE_COMMENT_SUBMIT").ok();
                    let expected = match submit.as_deref() {
                        Some("opencode") => "opencode:oc-target",
                        Some("router") => "codex:router",
                        _ => "codex:target",
                    };
                    until(|| {
                        ui.state.borrow().ready
                            && !ui.state.borrow().loading
                            && ui.composer_key().as_deref() == Some(expected)
                            && ui.send.is_sensitive()
                    })
                    .await?;
                    let project = directory.join("project");
                    let restore =
                        std::env::var_os("STUDIO_GNOME_SMOKE_ANNOTATIONS_RESTORE").is_some();
                    if let Ok(provider) = std::env::var("STUDIO_GNOME_SMOKE_COMMENT_SOURCE") {
                        if !matches!(provider.as_str(), "epub" | "browser" | "pdf" | "table") || submit.is_some() {
                            return Err("Invalid comment source fixture".into());
                        }
                        let url =
                            std::env::var("STUDIO_GNOME_SMOKE_COMMENT_URL").unwrap_or_default();
                        if provider=="table" {
                            let kind=std::env::var("STUDIO_GNOME_SMOKE_TABLE_KIND").unwrap_or_else(|_|"csv".into());
                            if !restore {
                                ui.show_tool("table");
                                ui.tables.open(project.to_str().unwrap(),if kind=="xlsx"{"book.xlsx"}else{"comments.csv"});
                                ui.tables.smoke_comment(&kind,||capture(&ui.window,&directory.join("table-comment-selection.png"))).await?;
                                let original=ui.state.borrow().selected.clone().unwrap();
                                let mut other=original.clone();other.id="capture-other".into();ui.select(other);
                                until(||!ui.tables.smoke_comment_pending()).await?;
                                if ui.annotations.borrow().is_some(){return Err("A table value was captured into another conversation".into());}
                                ui.select(original);until(||!ui.state.borrow().loading && ui.send.is_sensitive()).await?;
                                ui.tables.smoke_comment(&kind,||capture(&ui.window,&directory.join("table-comment-selection.png"))).await?;
                            }else{ui.open_annotations();}
                            until(||ui.annotations.borrow().is_some()).await?;
                            let panel=ui.annotations.borrow().clone().unwrap();
                            let (source,quote)=panel.smoke_provider("table",restore,|dialog|capture_dialog(dialog,&directory.join("annotations.png"))).await?;
                            if !restore {
                                let prompt=ui.composer_text();
                                if !prompt.contains(&quote) || !prompt.contains("Explain this passage 中文😀") || !prompt.contains("header") || kind=="sql" && (!prompt.contains("SQL result cell") || !prompt.contains("SELECT note AS quoted") || prompt.contains("not the selected result")) {return Err("Table prompt lost its quote or query provenance".into());}
                                if kind!="xlsx" && quote.len()<=2048{return Err("Table comment used the clipped preview".into());}
                                if kind=="sql" && source["anchor"]["range"]!="A1"{return Err("SQL result coordinates were assigned to the raw source".into());}
                            }
                            let variant=std::env::var("STUDIO_GNOME_SMOKE_SOURCE_VARIANT").unwrap_or_default();
                            let expected=variant.is_empty() || variant=="legacy" && kind!="sql";
                            ui.tables.smoke_comment_reopened(expected,kind=="sql",||capture(&ui.window,&directory.join("comment-source.png"))).await?;
                            return Ok(json!({"ok":true,"provider":"table","kind":kind,"variant":variant,"restore":restore,"sessionRaceGuard":!restore,"sourceReopened":true}));
                        }
                        if !restore && provider=="pdf" {
                            ui.show_tool("pdf");
                            ui.pdf.open(project.to_str().unwrap(),"sample.pdf",0);
                            let selection = ui.pdf.smoke_comment(||capture(&ui.window,&directory.join("pdf-preview.png"))).await?;
                            std::fs::write(directory.join("pdf-selection.json"),serde_json::to_vec_pretty(&selection).unwrap()).map_err(|e|e.to_string())?;
                        } else if !restore {
                            let target = if provider == "epub" {
                                ui.show_tool("epub");
                                ui.epub.open(project.to_str().unwrap(), "sample.epub");
                                &ui.epub.comments
                            } else {
                                let parsed = url::Url::parse(&url).map_err(|e| e.to_string())?;
                                if parsed.scheme() != "http"
                                    || parsed.host_str() != Some("127.0.0.1")
                                    || parsed.port().is_none()
                                {
                                    return Err("Comment fixture requires a private URL".into());
                                }
                                ui.show_tool("browser");
                                &ui.browser.comments
                            };
                            // Force a conversation switch while the asynchronous DOM read
                            // is pending. The real capture callback must reject it.
                            let original = ui.state.borrow().selected.clone().unwrap();
                            let previous = target.borrow().clone().unwrap();
                            let observed = Rc::new(RefCell::new(None));
                            target.replace(Some(Rc::new(crate::annotations_ui::CaptureTarget {
                                owner: Box::new({
                                    let previous = previous.clone();
                                    move || (previous.owner)()
                                }),
                                capture: Box::new({
                                    let previous = previous.clone();
                                    let observed = observed.clone();
                                    move |owner, capture| {
                                        let result = (previous.capture)(owner, capture);
                                        observed.replace(Some(result.clone()));
                                        result
                                    }
                                }),
                            })));
                            if provider == "epub" {
                                ui.epub.smoke_comment().await?;
                            } else {
                                ui.browser.smoke_comment(&url).await?;
                            }
                            let mut other = original.clone();
                            other.id = "capture-other".into();
                            ui.select(other);
                            until(|| observed.borrow().is_some()).await?;
                            target.replace(Some(previous));
                            if !observed.borrow().as_ref().unwrap().is_err()
                                || ui.annotations.borrow().is_some()
                            {
                                return Err("A delayed selection reached another session".into());
                            }
                            ui.select(original);
                            until(|| !ui.state.borrow().loading && ui.send.is_sensitive()).await?;
                            if provider == "epub" {
                                ui.epub.smoke_comment().await?;
                            } else {
                                ui.browser.smoke_comment(&url).await?;
                            }
                        } else {
                            ui.open_annotations();
                        }
                        until(|| ui.annotations.borrow().is_some()).await?;
                        let panel = ui.annotations.borrow().clone().unwrap();
                        let (source, quote) = panel
                            .smoke_provider(&provider, restore, |dialog| {
                                capture_dialog(dialog, &directory.join("annotations.png"))
                            })
                            .await?;
                        if !restore {
                            let message = ui.composer_text();
                            if message.matches("Please address every comment").count() != 1
                                || !message.contains("Explain this passage 中文😀")
                                || !message.contains(&quote)
                                || provider == "epub" && !message.contains("epubcfi(")
                                || provider == "browser" && !message.contains(&url)
                                || provider == "pdf" && (!message.contains("page 2") || !message.contains("regions") || !message.contains("PDF excerpts are read-only"))
                            {
                                return Err("Provider prompt lost its source or quote".into());
                            }
                        }
                        let variant =
                            std::env::var("STUDIO_GNOME_SMOKE_SOURCE_VARIANT").unwrap_or_default();
                        if provider == "epub" {
                            ui.epub
                                .smoke_comment_reopened(
                                    &quote,
                                    variant.is_empty(),
                                    variant == "changed",
                                )
                                .await?;
                        } else if provider=="pdf" {
                            ui.pdf.smoke_comment_reopened(variant.is_empty() || variant=="legacy",variant=="changed",variant=="legacy").await?;
                        } else {
                            ui.browser
                                .smoke_comment_reopened(
                                    source["anchor"]["url"].as_str().unwrap(),
                                    &quote,
                                )
                                .await?;
                        }
                        capture(&ui.window, &directory.join("comment-source.png"));
                        return Ok(json!({"ok":true,"provider":provider,"restore":restore,
                            "variant":variant,"sessionRaceGuard":!restore && provider!="pdf","sourceReopened":true}));
                    }
                    ui.reader.open_path(&project.join("annotation.md"));
                    ui.show_tool("document");
                    ui.reader.smoke_comment(restore).await?;
                    let panel = ui
                        .annotations
                        .borrow()
                        .clone()
                        .ok_or("Comments dialog did not open")?;
                    if let Some(mode) = submit {
                        let value = panel
                            .smoke_submit(
                                &mode,
                                &directory,
                                || {
                                    ui.composer
                                        .buffer()
                                        .set_text("Keep my newer composer draft 中文😀")
                                },
                                |dialog| {
                                    capture_dialog(
                                        dialog,
                                        &directory.join("annotations-submit.png"),
                                    )
                                },
                            )
                            .await?;
                        if mode != "reject"
                            && ui.composer_text() != "Keep my newer composer draft 中文😀"
                        {
                            return Err(
                                "Comment acknowledgement consumed a newer composer draft".into()
                            );
                        }
                        return Ok(value);
                    }
                    let value = panel
                        .smoke_test(restore, &directory, |dialog| {
                            capture_dialog(dialog, &directory.join("annotations.png"))
                        })
                        .await?;
                    if !restore {
                        let message = ui.composer_text();
                        if message.matches("Please address every comment").count() != 1
                            || !message.contains("/ line 3")
                            || !message.contains("Please clarify 中文😀")
                            || !message.contains("Keep all rows.")
                        {
                            return Err(
                                "Comment prompt was duplicated or lost its source/note".into()
                            );
                        }
                    }
                    Ok(value)
                }
                .await;
                let value = match result {
                    Ok(v) => v,
                    Err(e) => json!({"ok":false,"error":e}),
                };
                let _ = std::fs::write(
                    directory.join("annotations-ui.json"),
                    serde_json::to_vec_pretty(&value).unwrap(),
                );
                let panel = ui.annotations.borrow().clone();
                if let Some(panel) = panel {
                    panel.dialog.force_close();
                }
                ui.window.close();
            });
            return;
        }
        if let Ok(url) = std::env::var("STUDIO_GNOME_SMOKE_BROWSER_URL") {
            let parsed = url::Url::parse(&url).ok();
            if !parsed.is_some_and(|u| {
                u.scheme() == "http"
                    && u.host_str() == Some("127.0.0.1")
                    && u.port().is_some()
                    && u.path() == "/one"
            }) {
                ui.window.close();
                return;
            }
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                ui.show_tool("browser");
                let result = if std::env::var_os("STUDIO_GNOME_SMOKE_BROWSER_FILES").is_some() {
                    ui.browser
                        .smoke_files(&directory, &url, |name| {
                            capture(&ui.window, &directory.join(name))
                        })
                        .await
                } else {
                    ui.browser
                        .smoke_test(&url, |name| capture(&ui.window, &directory.join(name)))
                        .await
                };
                let value = match result {
                    Ok(value) => value,
                    Err(error) => json!({"ok":false,"error":error}),
                };
                let _ = std::fs::write(
                    directory.join("browser-ui.json"),
                    serde_json::to_vec_pretty(&value).unwrap(),
                );
                ui.window.close();
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_EPUB_SEARCH").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let project = directory.join("project");
                ui.activate_reference_from("sample.epub", &project.to_string_lossy());
                let result = ui
                    .epub
                    .smoke_book_search(&project, |dialog, name| {
                        capture_mapped_dialog(dialog, &directory.join(name))
                    })
                    .await;
                let value = result.unwrap_or_else(|error| json!({"ok":false,"error":error}));
                let _ = std::fs::write(
                    directory.join("epub-search.json"),
                    serde_json::to_vec_pretty(&value).unwrap(),
                );
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_EPUB_CLIPBOARD").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let project = directory.join("project");
                ui.activate_reference_from("sample.epub", &project.to_string_lossy());
                let result = ui
                    .epub
                    .smoke_clipboard(&directory, || {
                        capture(&ui.window, &directory.join("epub-clipboard.png"))
                    })
                    .await;
                let value = result.unwrap_or_else(|error| json!({"ok":false,"error":error}));
                let _ = std::fs::write(
                    directory.join("epub-clipboard.json"),
                    serde_json::to_vec_pretty(&value).unwrap(),
                );
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_EPUB").is_some() {
            if let Some(settings) = gtk::Settings::default() {
                settings.set_gtk_enable_animations(false);
            }
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let project = directory.join("project");
                ui.activate_reference_from("sample.epub", &project.to_string_lossy());
                let restore = std::env::var_os("STUDIO_GNOME_SMOKE_EPUB_RESTORE").is_some();
                let reference = std::env::var_os("STUDIO_GNOME_SMOKE_EPUB_REFERENCE").is_some();
                let result = if reference {
                    ui.epub
                        .smoke_reference(|name| capture(&ui.window, &directory.join(name)))
                        .await
                } else {
                    ui.epub
                        .smoke_test(restore, |name| capture(&ui.window, &directory.join(name)))
                        .await
                };
                let value = match result {
                    Ok(value) => value,
                    Err(error) => json!({"ok":false,"error":error}),
                };
                let name = if reference {
                    "epub-reference.json"
                } else if restore {
                    "epub-restored.json"
                } else {
                    "epub-ui.json"
                };
                let _ = std::fs::write(
                    directory.join(name),
                    serde_json::to_vec_pretty(&value).unwrap(),
                );
                ui.window.close();
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_PDF_LINKS").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let project = directory.join("project");
                ui.activate_reference_from("links-0.pdf", &project.to_string_lossy());
                let result = ui
                    .pdf
                    .smoke_links(&directory, || {
                        capture(&ui.window, &directory.join("pdf-links.png"))
                    })
                    .await;
                let value = result.unwrap_or_else(|error| json!({"ok":false,"error":error}));
                let _ = std::fs::write(
                    directory.join("pdf-links.json"),
                    serde_json::to_vec_pretty(&value).unwrap(),
                );
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_PDF").is_some() {
            if let Some(settings) = gtk::Settings::default() {
                settings.set_gtk_enable_animations(false);
            }
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let project = directory.join("project");
                ui.activate_reference_from("sample.pdf", &project.to_string_lossy());
                let result = ui
                    .pdf
                    .smoke_test(&project, |name| capture(&ui.window, &directory.join(name)))
                    .await;
                let value = match result {
                    Ok(value) => value,
                    Err(error) => json!({"ok":false,"error":error}),
                };
                let _ = std::fs::write(
                    directory.join("pdf-ui.json"),
                    serde_json::to_vec_pretty(&value).unwrap(),
                );
                ui.window.close();
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_IMAGE_SAVE").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let at = std::time::Instant::now();
                while !ui.state.borrow().history_verified && at.elapsed() < Duration::from_secs(25)
                {
                    glib::timeout_future(Duration::from_millis(25)).await;
                }
                ui.composer
                    .buffer()
                    .set_text("Unsent image save draft 中文😀 ");
                ui.show_tool("image");
                let result = async {
                    crate::smoke_input::request(&directory, json!({"action":"focus"})).await?;
                    let checks = ui
                        .image_viewer
                        .smoke_save(&directory, || ui.window.close())
                        .await?;
                    if ui.composer_text() != "Unsent image save draft 中文😀 " {
                        return Err("Image saving changed the conversation draft".to_string());
                    }
                    Ok::<_, String>(json!({"ok":true,"checks":checks,"draftRetained":true}))
                }
                .await;
                let value = result.unwrap_or_else(|error| json!({"ok":false,"error":error}));
                let bytes = serde_json::to_vec_pretty(&value).unwrap();
                let path = directory.join("image-save.json");
                let _ = gio::spawn_blocking(move || std::fs::write(path, bytes)).await;
                if value["ok"] == true {
                    ui.window.close();
                } else if let Some(app) = ui.window.application() {
                    app.quit();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_IMAGE_VIEWER").is_some() {
            if let Some(settings) = gtk::Settings::default() {
                settings.set_gtk_enable_animations(false);
            }
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let result = async {
                    let project = directory.join("project");
                    ui.activate_reference_from("sample.png", &project.to_string_lossy());
                    let viewer = ui
                        .image_viewer
                        .smoke_test(&project, |name| capture(&ui.window, &directory.join(name)))
                        .await?;
                    let animation=ui.image_viewer.smoke_animation(&project,&directory).await?;
                    ui.reader.open_project_path(&project,&project.join("animation-gif.md"),None,None);
                    let snapshot_animation=ui.images.smoke_snapshot_animation(&ui.reader,&ui.image_viewer,&project,&directory).await?;
                    ui.reader.open_project_path(
                        &project,
                        &project.join("nested/images.md"),
                        None,
                        None,
                    );
                    let inline = ui.reader.smoke_images().await?;
                    let chat = ui
                        .images
                        .smoke_chat(&ui.window, &project, ui.document_sender.clone(), |dialog| {
                            capture_dialog(dialog, &directory.join("image-chat.png"))
                        })
                        .await?;
                    let animated_chat=ui.images.smoke_chat_animation(&ui.window,&project,ui.document_sender.clone(),&directory).await?;
                    capture(&ui.window, &directory.join("image-inline.png"));
                    Ok::<_, String>(json!({"ok":true,"viewer":viewer,"animation":animation,"snapshotAnimation":snapshot_animation,"animatedChat":animated_chat,"inline":inline,"chat":chat}))
                }
                .await;
                let value = match result {
                    Ok(value) => value,
                    Err(error) => json!({"ok":false,"error":error}),
                };
                let _ = std::fs::write(
                    directory.join("images-ui.json"),
                    serde_json::to_vec_pretty(&value).unwrap(),
                );
                ui.window.close();
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_TABLES").is_some() {
            if let Some(settings) = gtk::Settings::default() {
                settings.set_gtk_enable_animations(false);
            }
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                ui.activate_reference_from(
                    "data.csv",
                    &directory.join("project").to_string_lossy(),
                );
                let result = ui
                    .tables
                    .smoke_test(
                        &directory,
                        |name| capture(&ui.window, &directory.join(name)),
                        |dialog, name| capture_dialog(dialog, &directory.join(name)),
                    )
                    .await;
                let value = match result {
                    Ok(value) => value,
                    Err(error) => json!({"ok":false,"error":error}),
                };
                let _ = std::fs::write(
                    directory.join("tables-ui.json"),
                    serde_json::to_vec_pretty(&value).unwrap(),
                );
                ui.window.close();
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_ROUTER").is_some() {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                let tested = router_chat::smoke(&ui, &directory).await;
                let value = match tested {
                    Ok(value) => value,
                    Err(error) => json!({"ok":false,"error":error}),
                };
                let _ = std::fs::write(
                    directory.join("router-ui.json"),
                    serde_json::to_vec_pretty(&value).unwrap(),
                );
                ui.window.close();
            });
            return;
        }
        let started = Instant::now();
        if let Ok(phase) = std::env::var("STUDIO_GNOME_SMOKE_AGENT_RESTART") {
            let weak = Rc::downgrade(ui);
            glib::spawn_future_local(async move {
                if let Some(ui) = weak.upgrade() {
                    let tested = agent_commands::restart_smoke(&ui, phase == "seed").await;
                    let result = match tested {
                        Ok(value) => value,
                        Err(error) => json!({"ok":false,"error":error}),
                    };
                    let _ = std::fs::write(
                        directory.join(format!("agent-restart-{phase}.json")),
                        serde_json::to_vec_pretty(&result).unwrap(),
                    );
                    ui.window.close();
                }
            });
            return;
        }
        if std::env::var_os("STUDIO_GNOME_SMOKE_RESTORE").is_some() {
            let weak = Rc::downgrade(ui);
            glib::timeout_add_local(Duration::from_millis(100), move || {
                let Some(ui) = weak.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                let opened = ui.reader.is_ready()
                    && ui
                        .reader
                        .location()
                        .is_some_and(|(_, path)| path == directory.join("project/nested/note.md"));
                let restored = opened && {
                    let state = ui.state.borrow();
                    state.pins_loaded
                        && state.pins.contains(&"opencode:native-oc-thread".into())
                        && state.ready
                        && !state.loading
                        && state.backend == "opencode"
                        && state.selected.as_ref().is_some_and(|thread| {
                            thread.id == "native-oc-thread"
                                && thread.cwd == directory.join("project").to_string_lossy()
                        })
                };
                let queue_restored = {
                    let state = ui.state.borrow();
                    state
                        .queues
                        .entries
                        .get("opencode:native-oc-thread")
                        .is_some_and(|entry| {
                            entry.loaded
                                && entry.paused
                                && entry.messages.len() == 1
                                && entry.messages[0].text == "native-oc-restore-do-not-send"
                                && entry.flight.is_none()
                                && entry.preparing.is_none()
                        })
                };
                if restored && queue_restored || started.elapsed() > Duration::from_secs(15) {
                    let dark = adw::StyleManager::default().is_dark();
                    let value = json!({"ok":restored&&dark&&queue_restored,"selectionRestored":restored,"themeRestored":dark,"desktopFileOpen":opened,"queueRestoredPaused":queue_restored});
                    let _ = std::fs::write(
                        directory.join("restored.json"),
                        serde_json::to_vec_pretty(&value).unwrap(),
                    );
                    ui.window.close();
                    return glib::ControlFlow::Break;
                }
                glib::ControlFlow::Continue
            });
            return;
        }
        if let Some(settings) = gtk::Settings::default() {
            settings.set_gtk_enable_animations(false);
        }
        let weak = Rc::downgrade(ui);
        let mut step = 0;
        let mut ticks = 0;
        let mut compact_window: Option<adw::ApplicationWindow> = None;
        glib::timeout_add_local(Duration::from_millis(150), move || {
            let Some(ui) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            ticks += 1;
            // This integration now runs document, workspace, session, queue,
            // command, agent and Router suites before the OpenCode workflow.
            // Their individual waits keep their own shorter deadlines. A
            // one-minute total cut off a progressing final provider workflow.
            if started.elapsed() > Duration::from_secs(180) {
                let _=std::fs::write(directory.join("result.json"),json!({"error":"native smoke timed out","step":step,"status":ui.status.text().as_str()}).to_string());
                ui.window.close();
                return glib::ControlFlow::Break;
            }
            match step {
                0 => {
                    let thread = ui
                        .state
                        .borrow()
                        .threads
                        .iter()
                        .find(|t| t.id == "native-smoke-thread")
                        .cloned();
                    if let Some(thread) = thread {
                        ui.select(thread);
                        step = 1;
                    }
                }
                1 if !ui.state.borrow().loading
                    && ui.state.borrow().options_loaded
                    && ui.fast.is_sensitive()
                    && ui
                        .state
                        .borrow()
                        .items
                        .iter()
                        .any(|i| i.id == "history-message") =>
                {
                    capture(&ui.window, &directory.join("light.png"));
                    gtk::prelude::WidgetExt::activate_action(&ui.window, "win.theme-dark", None)
                        .unwrap();
                    assert!(
                        ui.fast.is_active(),
                        "Native composer did not reflect inherited Fast"
                    );
                    ui.model.set_selected(1);
                    let high = ui
                        .state
                        .borrow()
                        .efforts
                        .iter()
                        .position(|effort| effort == "high")
                        .unwrap();
                    ui.effort.set_selected(high as u32);
                    ui.fast.emit_clicked();
                    ui.composer.buffer().set_text("native-smoke-send");
                    ui.submit();
                    step = 2;
                }
                2 if !ui.state.borrow().sending
                    && ui
                        .state
                        .borrow()
                        .items
                        .iter()
                        .any(|i| i.id == "native-smoke-reply")
                    && ui.state.borrow().active_turn.is_none() =>
                {
                    ui.fast.emit_clicked();
                    ui.composer.buffer().set_text("native-smoke-active");
                    ui.submit();
                    step = 3;
                }
                3 if !ui.state.borrow().sending && ui.state.borrow().active_turn.is_some() => {
                    ui.composer.buffer().set_text("native-smoke-steer");
                    ui.submit();
                    step = 4;
                }
                4 if !ui.state.borrow().sending
                    && ui
                        .state
                        .borrow()
                        .items
                        .iter()
                        .any(|i| i.id == "native-smoke-steered") =>
                {
                    ui.interrupt();
                    step = 5;
                }
                5 if ui.state.borrow().active_turn.is_none() => {
                    ui.composer.buffer().set_text("native-smoke-stale");
                    ui.submit();
                    step = 51;
                }
                51 if !ui.state.borrow().sending && ui.state.borrow().active_turn.is_some() => {
                    ui.interrupt();
                    step = 52;
                }
                52 if !ui.state.borrow().loading && ui.state.borrow().active_turn.is_none() => {
                    ui.composer
                        .buffer()
                        .set_text("Unsaved native reconnect draft");
                    ui.reconnect();
                    step = 53;
                }
                53 if ui.state.borrow().ready
                    && !ui.state.borrow().loading
                    && ui.state.borrow().selected.is_some() =>
                {
                    assert_eq!(ui.composer_text(), "Unsaved native reconnect draft");
                    let (backend, generation) = {
                        let state = ui.state.borrow();
                        (state.backend.clone(), state.socket_generation - 1)
                    };
                    ui.event(Event::BackendError {
                        backend: backend.clone(),
                        generation,
                        error: "Stale connection must be ignored".into(),
                    });
                    ui.event(Event::Wire {backend,generation,payload:json!({"method":"studio/appServer/status","params":{"state":"error","message":"Stale event must be ignored"}}),prepared:protocol::Prepared::default()});
                    assert!(ui.state.borrow().ready);
                    capture(&ui.window, &directory.join("dark.png"));
                    ui.search.set_text("nonmatching-native-filter");
                    step = 6;
                }
                6 if ui.sessions.n_items() == 0 => {
                    ui.search.set_text("");
                    let ui = ui.clone();
                    let directory = directory.clone();
                    glib::MainContext::default().spawn_local(async move {
                        let tested=crate::chat_ui::smoke_test(&ui.transcript_view,&ui.transcript,&ui.transcript_scroll,|name|capture(&ui.window,&directory.join(name))).await;
                        let chat=match tested{Ok(value)=>value,Err(error)=>json!({"ok":false,"error":error})};
                        let _=std::fs::write(directory.join("chat.json"),serde_json::to_vec_pretty(&chat).unwrap());
                        let tested=ui.chat_diagrams.smoke_test(&ui.transcript_view,&ui.transcript,&ui.transcript_scroll,|name|capture(&ui.window,&directory.join(name))).await;
                        let diagrams=match tested{Ok(value)=>value,Err(error)=>json!({"ok":false,"error":error})};
                        let _=std::fs::write(directory.join("chat-diagrams.json"),serde_json::to_vec_pretty(&diagrams).unwrap());
                        let tested=ui.finder.smoke_test(||capture(&ui.window,&directory.join("conversation-search.png")),|hit|crate::chat_ui::target_visible(&ui.transcript,hit)).await;
                        let search=match tested{Ok(value)=>value,Err(error)=>json!({"ok":false,"error":error})};
                        let _=std::fs::write(directory.join("conversation-search.json"),serde_json::to_vec_pretty(&search).unwrap());
                        let before = { let state = ui.state.borrow(); (state.selected.clone(), state.active_turn.clone(), state.items.iter().map(|item|item.id.clone()).collect::<Vec<_>>(), ui.composer_text()) };
                        let fetched = async {
                            ui.finder.smoke_old_target().await?;
                            let start = Instant::now();
                            loop {
                                let ready = ui.history_context.borrow().as_ref().is_some_and(|view|view.contains("native-old-response"));
                                if ready { break; }
                                if start.elapsed()>Duration::from_secs(8) { return Err("Historical target did not load from its older page".to_owned()); }
                                glib::timeout_future(Duration::from_millis(25)).await;
                            }
                            let view = ui.history_context.borrow().clone().unwrap();
                            capture_dialog(&view.dialog,&directory.join("historical-context.png"));
                            let unchanged = { let state=ui.state.borrow(); state.selected==before.0 && state.active_turn==before.1 && state.items.iter().map(|item|item.id.clone()).collect::<Vec<_>>()==before.2 && ui.composer_text()==before.3 };
                            if !unchanged { return Err("Historical navigation modified the live conversation".to_owned()); }
                            view.dialog.close(); ui.finder.close(); ui.finder.clear();
                            std::fs::write(directory.join("native-legacy-read.flag"), "test").map_err(|error|error.to_string())?;
                            ui.finder.smoke_old_target().await?;
                            let start=Instant::now();
                            while !ui.history_context.borrow().as_ref().is_some_and(|view|view.contains("native-old-response")) {
                                if start.elapsed()>Duration::from_secs(8) { return Err("Legacy history read did not retrieve its source".to_owned()); }
                                glib::timeout_future(Duration::from_millis(25)).await;
                            }
                            let legacy=ui.history_context.borrow().clone().unwrap(); legacy.dialog.close();
                            std::fs::remove_file(directory.join("native-legacy-read.flag")).map_err(|error|error.to_string())?;
                            std::fs::write(directory.join("native-no-search.flag"), "test").map_err(|error|error.to_string())?;
                            ui.finder.smoke_legacy_search().await?;
                            std::fs::remove_file(directory.join("native-no-search.flag")).map_err(|error|error.to_string())?;
                            Ok::<_,String>(json!({"ok":true,"remoteOccurrence":true,"olderPage":true,"liveConversationUnchanged":true,"readOnlyContext":true,"legacyHistoryRead":true,"unsupportedSearchFallback":true}))
                        }.await;
                        let fetched=match fetched{Ok(value)=>value,Err(error)=>json!({"ok":false,"error":error})};
                        let _=std::fs::write(directory.join("historical-search.json"),serde_json::to_vec_pretty(&fetched).unwrap());
                        let favorites_result=async {
                            // A history refresh may return equal content in new
                            // immutable objects. Exercise actual row callbacks
                            // after that refresh, not only the model lookup.
                            let refreshed=ui.state.borrow().items.iter().map(|item|Arc::new((**item).clone())).collect();
                            ui.state.borrow_mut().items=refreshed;
                            ui.render_items();
                            fn find_widget<T:glib::object::IsA<gtk::Widget>+glib::object::IsA<glib::Object>+Clone+glib::types::StaticType>(root:&gtk::Widget,test:&impl Fn(&T)->bool)->Option<T>{
                                if let Some(widget)=root.downcast_ref::<T>(){if test(widget){return Some(widget.clone());}}
                                let mut child=root.first_child();while let Some(widget)=child{child=widget.next_sibling();if let Some(found)=find_widget(&widget,test){return Some(found);}}None
                            }
                            let header=(0..ui.transcript.n_items()).find(|index|ui.transcript.item(*index).and_downcast::<glib::BoxedAnyObject>().is_some_and(|object|{let row=object.borrow::<crate::chat_ui::Row>();row.item.id=="history-message"&&row.part==crate::chat_ui::Part::Header})).ok_or("Favorite message header was not retained")?;
                            ui.transcript_view.scroll_to(header,gtk::ListScrollFlags::NONE,None);
                            capture(&ui.window,&directory.join("before-favorite.png"));
                            let start=Instant::now();
                            let button=loop {
                                if let Some(button)=find_widget::<gtk::Button>(ui.toast.upcast_ref(),&|button|button.tooltip_text().as_deref()==Some("Save displayed response to favorites")){break button;}
                                if start.elapsed()>Duration::from_secs(5){return Err("Native favorite message action was not mounted".to_owned());}
                                glib::timeout_future(Duration::from_millis(25)).await;
                            };
                            button.emit_clicked();
                            let library=ui.favorites.borrow().clone().ok_or("Native favorites did not open")?;
                            let saved=library.smoke_save(&ui.window,|dialog,name|capture_dialog(dialog,&directory.join(name))).await?;
                            library.dialog.close();
                            let start=Instant::now();while ui.favorites.borrow().is_some(){if start.elapsed()>Duration::from_secs(5){return Err("Favorites did not close after saving".to_owned());}glib::timeout_future(Duration::from_millis(25)).await;}
                            let label=find_widget::<gtk::Label>(ui.toast.upcast_ref(),&|label|label.text().starts_with("This response uses GTK4")).ok_or("Selectable native response text was not mounted")?;
                            let text=label.text();let offset=text.find("GTK4").ok_or("Missing selection text")?;let offset=text[..offset].chars().count() as i32;
                            label.select_region(offset,offset+4);label.activate_action("favorite.save-selection",None).map_err(|error|error.to_string())?;
                            let library=ui.favorites.borrow().clone().ok_or("Selection favorite did not open")?;
                            library.smoke_selection(&ui.window).await?;
                            library.smoke_export(&saved,&directory.join("favorites-export.md")).await?;
                            let start=Instant::now();while !ui.history_context.borrow().as_ref().is_some_and(|view|view.contains("history-message")){if start.elapsed()>Duration::from_secs(5){return Err("Favorite source did not open in historical context".to_owned());}glib::timeout_future(Duration::from_millis(25)).await;}
                            let view=ui.history_context.borrow().clone().unwrap();
                            crate::window_capture::ready(&view.dialog.child().ok_or("History context has no content")?).await?;
                            capture_dialog(&view.dialog,&directory.join("favorite-context.png"));view.dialog.close();
                            let unchanged={let state=ui.state.borrow();state.selected==before.0&&state.active_turn==before.1&&state.items.iter().map(|item|item.id.clone()).collect::<Vec<_>>()==before.2&&ui.composer_text()==before.3};
                            if !unchanged{return Err("Favorites changed the live conversation or draft".to_owned());}
                            Ok::<_,String>(json!({"ok":true,"messageAction":true,"selectionAction":true,"metadataRoundTrip":true,"sharedStorage":true,"copy":true,"previewSource":true,"cancelClose":true,"discardEdit":true,"failedSaveRetention":true,"confirmedDelete":true,"export":true,"sourceContext":true,"liveConversationUnchanged":true}))
                        }.await;
                        let favorites_result=match favorites_result{Ok(value)=>value,Err(error)=>json!({"ok":false,"error":error})};
                        let _=std::fs::write(directory.join("favorites.json"),serde_json::to_vec_pretty(&favorites_result).unwrap());
                        let library=ui.favorites.borrow_mut().take();if let Some(library)=library{library.dialog.force_close();}
                        let opencode_source=async {
                            let favorite:crate::favorites::Favorite=serde_json::from_value(json!({"id":"native-readonly-other-source","scope":"message","backend":"opencode","threadId":"native-oc-other-thread","threadTitle":"Other OpenCode source","projectPath":directory.join("project").to_string_lossy(),"turnId":"oc-old-user","itemId":"oc-old-answer-part","title":"Earlier OpenCode answer","question":"An earlier question","content":"native-oc-old-marker 中文😀 ","note":"","tags":[],"createdAt":"2026-10-24T00:00:00Z"})).map_err(|error|error.to_string())?;
                            ui.open_favorite_source(favorite);
                            let start=Instant::now();while !ui.history_context.borrow().as_ref().is_some_and(|view|view.contains("oc-old-answer-part")){if start.elapsed()>Duration::from_secs(6){return Err("Other-session OpenCode favorite source did not load older pages".to_owned());}glib::timeout_future(Duration::from_millis(25)).await;}
                            let context=ui.history_context.borrow().clone().unwrap();
                            crate::window_capture::ready(&context.dialog.child().ok_or("History context has no content")?).await?;
                            capture_dialog(&context.dialog,&directory.join("opencode-favorite-context.png"));context.dialog.close();
                            let unchanged={let state=ui.state.borrow();state.backend=="codex"&&state.selected==before.0&&state.active_turn==before.1&&state.items.iter().map(|item|item.id.clone()).collect::<Vec<_>>()==before.2&&ui.composer_text()==before.3};
                            if !unchanged{return Err("Cross-backend source navigation modified the live Codex session".to_owned());}
                            Ok::<_,String>(json!({"ok":true,"otherBackend":true,"otherSession":true,"olderPages":true,"readOnly":true,"liveConversationUnchanged":true}))
                        }.await;
                        let opencode_source=match opencode_source{Ok(value)=>value,Err(error)=>json!({"ok":false,"error":error})};
                        let _=std::fs::write(directory.join("opencode-favorite-source.json"),serde_json::to_vec_pretty(&opencode_source).unwrap());
                        ui.transcript_view.scroll_to(0,gtk::ListScrollFlags::NONE,None);
                        fn find_link(widget:&gtk::Widget)->Option<gtk::Label>{
                            if let Some(label)=widget.downcast_ref::<gtk::Label>(){if label.label().contains("nested/note.md:1"){return Some(label.clone());}}
                            let mut child=widget.first_child();while let Some(widget)=child{child=widget.next_sibling();if let Some(link)=find_link(&widget){return Some(link);}}None
                        }
                        capture(&ui.window,&directory.join("before-link.png"));
                        let link=if let Some(label)=find_link(ui.toast.upcast_ref()){
                            label.emit_by_name::<bool>("activate-link",&[&"nested/note.md:1"]);
                            ui.reader.smoke_link(&directory.join("project/nested/note.md")).await
                        }else{Err("The native conversation link label was not mounted".into())};
                        let link_value=match link{Ok(value)=>value,Err(error)=>json!({"ok":false,"error":error})};
                        let _=std::fs::write(directory.join("links.json"),serde_json::to_vec_pretty(&link_value).unwrap());
                        let result = async {
                            let mut value=ui.reader.smoke_test(&directory, |path|capture(&ui.window,path)).await?;
                            source_comment_workflow_acceptance::save_flow(&ui,&directory,&mut value).await?;
                            Ok::<_,String>(value)
                        }.await;
                        let checkpoint=match &result {Ok(value)=>value.clone(),Err(error)=>json!({"ok":false,"error":error})};
                        let _=std::fs::write(directory.join("reader-checkpoint.json"),serde_json::to_vec_pretty(&checkpoint).unwrap());
                        ui.show_tool("compare");
                        let compared=ui.compare.smoke_test(&directory,|path|capture(&ui.window,path)).await;
                        let comparison=match compared{Ok(value)=>value,Err(error)=>json!({"ok":false,"error":error})};
                        let _=std::fs::write(directory.join("compare.json"),serde_json::to_vec_pretty(&comparison).unwrap());
                        ui.show_tool("terminal");
                        let terminal_result = ui
                            .terminal
                            .smoke_test(&directory, || {
                                capture(&ui.window, &directory.join("terminal.png"))
                            })
                            .await;
                        let terminal_value = match terminal_result {
                            Ok(value) => value,
                            Err(error) => json!({"ok":false,"error":error}),
                        };
                        let _ = std::fs::write(
                            directory.join("terminal.json"),
                            serde_json::to_vec_pretty(&terminal_value).unwrap(),
                        );
                        ui.show_tool("files");
                        let files=ui.files.smoke_test(&directory.join("project"),||capture(&ui.window,&directory.join("files.png"))).await;
                        ui.show_tool("source");
                        let source=ui.source.smoke_test(&directory.join("project"),||capture(&ui.window,&directory.join("source.png"))).await;
                        ui.show_tool("git");
                        let git=async {
                            let mut value=ui.git.smoke_test(&directory.join("git-project"),||capture(&ui.window,&directory.join("git.png"))).await?;
                            ui.reader.smoke_link(&directory.join("git-project/README.md")).await?;
                            if ui.tools.visible_child_name().as_deref()!=Some("document") { return Err("Git Open file did not present the native Markdown reader".to_owned()); }
                            ui.git.smoke_close();
                            if ui.tools.visible_child_name().as_deref()!=Some("document") || !ui.tools.is_visible() { return Err("A delayed Git close hid the subsequently opened document".to_owned()); }
                            ui.reader.smoke_close();
                            if ui.tools.visible_child_name().as_deref()!=Some("git") || !ui.tools.is_visible() { return Err("Closing the Git-opened Markdown file did not restore Review".to_owned()); }
                            ui.git.smoke_open_working_file("untracked.txt").await?;
                            let started=std::time::Instant::now();
                            while ui.source.visible_text()!="New file.\n" {
                                if started.elapsed()>std::time::Duration::from_secs(10) { return Err("Git Open file did not load the selected native source".to_owned()); }
                                glib::timeout_future(std::time::Duration::from_millis(25)).await;
                            }
                            if ui.tools.visible_child_name().as_deref()!=Some("source") { return Err("Git source handoff opened the wrong workspace".to_owned()); }
                            ui.source.smoke_close();
                            if ui.tools.visible_child_name().as_deref()!=Some("git") || !ui.tools.is_visible() { return Err("Closing the Git-opened source did not restore Review".to_owned()); }
                            value["openFile"]=json!(true);value["returnToReview"]=json!(true);value["stalePanelClose"]=json!(true);
                            Ok::<_,String>(value)
                        }.await;
                        let result_json=|result:Result<Value,String>|match result{Ok(value)=>value,Err(error)=>json!({"ok":false,"error":error})};
                        let workspace=json!({"files":result_json(files),"source":result_json(source),"git":result_json(git)});
                        let _=std::fs::write(directory.join("workspace.json"),serde_json::to_vec_pretty(&workspace).unwrap());
                        ui.hide_tools();
                        let managed=sessions(&ui).await;
                        let management=match managed{Ok(value)=>value,Err(error)=>json!({"ok":false,"error":error})};
                        let _=std::fs::write(directory.join("sessions.json"),serde_json::to_vec_pretty(&management).unwrap());
                        let cached=conversation_cache(&ui).await;
                        let cache_result=match cached{Ok(value)=>value,Err(error)=>json!({"ok":false,"error":error})};
                        let _=std::fs::write(directory.join("conversation-cache.json"),serde_json::to_vec_pretty(&cache_result).unwrap());
                        ui.open_settings();
                        let settings=ui.settings.borrow().clone().unwrap();
                        let tested=settings.smoke_test(|dialog|capture_dialog(dialog,&directory.join("settings.png"))).await;
                        let settings_result=match tested{Ok(value)=>value,Err(error)=>json!({"ok":false,"error":error})};
                        let _=std::fs::write(directory.join("settings.json"),serde_json::to_vec_pretty(&settings_result).unwrap());
                        let references=refs::smoke(&ui,&directory,|name|capture(&ui.window,&directory.join(name))).await;
                        let references=match references {Ok(value)=>value,Err(error)=>json!({"ok":false,"error":error})};
                        let _=std::fs::write(directory.join("references.json"),serde_json::to_vec_pretty(&references).unwrap());
                        let attachments=attachment_controller::smoke(&ui,&directory,|name|capture(&ui.window,&directory.join(name))).await;
                        let attachments=match attachments {Ok(value)=>value,Err(error)=>json!({"ok":false,"error":error})};
                        let _=std::fs::write(directory.join("attachments.json"),serde_json::to_vec_pretty(&attachments).unwrap());
                        let command_result=commands::smoke(&ui,&directory,|dialog,name|capture_dialog(dialog,&directory.join(name)),|name|capture(&ui.window,&directory.join(name))).await;
                        let command_result=match command_result{Ok(value)=>value,Err(error)=>json!({"ok":false,"error":error})};
                        let _=std::fs::write(directory.join("commands.json"),serde_json::to_vec_pretty(&command_result).unwrap());
                        let agent_result=agent_commands::smoke(&ui,&directory,&|dialog,name|capture_dialog(dialog,&directory.join(name))).await;
                        let agent_result=match agent_result{Ok(value)=>value,Err(error)=>json!({"ok":false,"error":error})};
                        let _=std::fs::write(directory.join("agents.json"),serde_json::to_vec_pretty(&agent_result).unwrap());
                        let router_result=router_settings::smoke(&ui,&directory,|dialog,name|capture_dialog(dialog,&directory.join(name))).await;
                        let router_result=match router_result{Ok(value)=>value,Err(error)=>json!({"ok":false,"error":error})};
                        let _=std::fs::write(directory.join("router-settings.json"),serde_json::to_vec_pretty(&router_result).unwrap());
                        let tested=queues::smoke(&ui,&directory,|name|capture(&ui.window,&directory.join(name)),|dialog,name|capture_dialog(dialog,&directory.join(name))).await;
                        let queue_result=match tested{Ok(value)=>value,Err(error)=>json!({"ok":false,"error":error})};
                        let _=std::fs::write(directory.join("queue.json"),serde_json::to_vec_pretty(&queue_result).unwrap());
                        let tested=opencode(&ui,&directory).await;
                        let opencode_result=match tested{Ok(value)=>value,Err(error)=>json!({"ok":false,"error":error})};
                        let _=std::fs::write(directory.join("opencode.json"),serde_json::to_vec_pretty(&opencode_result).unwrap());
                        let value = match result {
                            Ok(value) => value,
                            Err(error) => json!({"ok":false,"error":error}),
                        };
                        let _ = std::fs::write(
                            directory.join("documents.json"),
                            serde_json::to_vec_pretty(&value).unwrap(),
                        );
                    });
                    step = 60;
                }
                60 if directory.join("documents.json").exists() => {
                    // Let Wayland configure a real narrow surface rather than
                    // changing a GtkWindow allocation behind its compositor.
                    let compact = adw::ApplicationWindow::builder()
                        .application(&ui.window.application().unwrap())
                        .title("Native GNOME compact layout check")
                        .default_width(720)
                        .default_height(850)
                        .build();
                    let breakpoint = adw::Breakpoint::new(
                        adw::BreakpointCondition::parse("max-width: 800sp").unwrap(),
                    );
                    breakpoint.add_setter(&ui.split, "collapsed", Some(&true.to_value()));
                    compact.add_breakpoint(breakpoint);
                    ui.window.set_content(None::<&gtk::Widget>);
                    compact.set_content(Some(&ui.toast));
                    compact.present();
                    compact_window = Some(compact);
                    step = 7;
                }
                7 if ticks > 10
                    && ui.sessions.n_items() > 0
                    && compact_window
                        .as_ref()
                        .is_some_and(|window| window.width() > 0) =>
                {
                    capture(
                        compact_window.as_ref().unwrap(),
                        &directory.join("compact.png"),
                    );
                    let display =
                        gtk::gdk::Display::default().map(|d| d.type_().name().to_string());
                    let result = json!({"ok":true,"display":display,"elapsedMs":started.elapsed().as_millis(),"gtk":format!("{}.{}.{}",gtk::major_version(),gtk::minor_version(),gtk::micro_version()),
                        "history":true,"send":true,"steer":true,"stop":true,"staleTurnRecovery":true,"reconnect":true,"draftRetained":true,"generationIsolation":true,"search":true,"idle":ui.state.borrow().active_turn.is_none(),"mainLoopTicks":ticks,"collapsed":ui.split.is_collapsed(),"items":ui.transcript.n_items()});
                    let _ = std::fs::write(
                        directory.join("result.json"),
                        serde_json::to_vec_pretty(&result).unwrap(),
                    );
                    if let Some(window) = &compact_window {
                        window.close();
                    }
                    ui.window.close();
                    return glib::ControlFlow::Break;
                }
                _ => {}
            }
            glib::ControlFlow::Continue
        });
    }

    // Named theme colors need this compatibility accessor on our GTK 4.14 floor.
    #[allow(deprecated)]
    pub(super) fn capture_mapped_dialog(
        dialog: &adw::Dialog,
        path: &std::path::Path,
    ) -> Result<(), String> {
        let content = dialog.child().ok_or("Dialog has no content")?;
        let (width, height) = (content.width(), content.height());
        if !content.is_mapped() || width <= 0 || height <= 0 || width > 4096 || height > 4096 {
            return Err("Dialog is not mapped within the capture budget".into());
        }
        let snapshot = gtk::Snapshot::new();
        let color = content
            .style_context()
            .lookup_color("window_bg_color")
            .unwrap_or_else(|| gtk::gdk::RGBA::new(0.98, 0.98, 0.98, 1.0));
        snapshot.append_color(
            &color,
            &gtk::graphene::Rect::new(0.0, 0.0, width as f32, height as f32),
        );
        // Observe actual GTK allocation/rendering; do not allocate widgets or
        // synthesize frame-clock callbacks to make a screenshot pass.
        gtk::WidgetPaintable::new(Some(&content)).snapshot(&snapshot, width as f64, height as f64);
        let node = snapshot
            .to_node()
            .ok_or("Mapped dialog has no render node")?;
        let renderer = dialog
            .native()
            .and_then(|native| native.renderer())
            .ok_or("Mapped dialog has no renderer")?;
        let texture = renderer.render_texture(&node, None);
        let stride = texture.width() as usize * 4;
        let mut pixels = vec![0u8; stride * texture.height() as usize];
        texture.download(&mut pixels, stride);
        if pixels.chunks_exact(4).all(|pixel| pixel == &pixels[..4]) {
            return Err("Mapped dialog capture is blank".into());
        }
        texture.save_to_png(path).map_err(|e| e.to_string())
    }

    // Named theme colors need this compatibility accessor on our GTK 4.14 floor.
    pub(super) fn capture_dialog(dialog: &impl IsA<adw::Dialog>, path: &std::path::Path) {
        let dialog = dialog.as_ref();
        if let Err(error) = capture_mapped_dialog(dialog, path) {
            eprintln!("Native dialog capture {} failed: {error}", path.display());
            if let Some(window) = dialog.root().and_downcast::<gtk::Window>() {
                if let Some(app) = window.application() {
                    app.quit();
                }
            }
        }
    }
    #[allow(deprecated)]
    pub(super) fn capture(window: &impl IsA<gtk::Window>, path: &std::path::Path) {
        let window = window.as_ref();
        // Dialogs live in AdwWindow's overlay outside its application content.
        // Snapshot the GTK root child so floating native preferences are included.
        let content =
            gtk::prelude::GtkWindowExt::child(window).expect("Capture window has no child");
        if let Err(error) = crate::window_capture::capture_current(window, &content, path) {
            eprintln!("Native capture {} failed: {error}", path.display());
            if let Some(app) = window.application() {
                app.quit();
            }
        }
    }
}
