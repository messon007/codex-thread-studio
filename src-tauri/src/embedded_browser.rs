#![cfg(target_os = "linux")]

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    rc::Rc,
    time::{Duration, Instant},
};

use gtk::prelude::*;
use serde::{Deserialize, Serialize};
use url::Url;
use webkit2gtk::{
    DownloadExt, PermissionRequestExt, URIRequestExt, URIResponseExt, WebContextExt,
    WebProcessTerminationReason, WebViewExt as WebKitWebViewExt,
};
#[cfg(debug_assertions)]
use webkit2gtk::{SnapshotOptions, SnapshotRegion};
use wry::{
    PageLoadEvent, WebContext, WebView, WebViewBuilder, WebViewBuilderExtUnix, WebViewExtUnix,
};

use crate::browser_runtime::{validate_browser_url, BrowserPreferences};

const DEFAULT_URL: &str = "https://example.com";
const MIN_STUDIO_WIDTH: i32 = 480;
const MIN_BROWSER_WIDTH: i32 = 480;
// Keep the two-row browser chrome aligned with the 75px Studio thread toolbar.
const TOOLBAR_HEIGHT: i32 = 75;
const MAX_SELECTION_BYTES: usize = 16 * 1024;
const MIN_ZOOM: f64 = 0.5;
const MAX_ZOOM: f64 = 2.0;
const ZOOM_STEP: f64 = 0.1;
const RECOVERY_WINDOW: Duration = Duration::from_secs(30);
const MAX_DOWNLOAD_RECORDS: usize = 100;
const TOOLBAR_HTML: &str = include_str!("../../ui/embedded-browser.html");

thread_local! {
    static WORKSPACE: RefCell<Option<EmbeddedBrowserWorkspace>> = const { RefCell::new(None) };
}

#[derive(Debug)]
enum BrowserAction {
    Toggle,
    Show(Option<i32>),
    Hide,
    Open(String, Option<i32>),
    Resize(i32),
    SetTranslations(HashMap<String, String>),
    Exit,
    Navigate(String),
    NewTab(Option<String>),
    ActivateTab(u64),
    CloseTab(u64),
    NextTab,
    PreviousTab,
    Back,
    Forward,
    Reload,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    FitWidth,
    CommentSelection,
    CopyUrl,
    ShowMenu,
    ShowInfo,
    ShowDownloads,
    PageTerminated {
        id: u64,
        reason: WebProcessTerminationReason,
    },
    ToolbarTerminated(WebProcessTerminationReason),
    DownloadStarted {
        url: String,
        path: PathBuf,
    },
    DownloadFinished {
        url: String,
        path: Option<PathBuf>,
        success: bool,
    },
    DownloadRejected(String),
    NavigationRejected {
        url: String,
        reason: String,
    },
    TlsError {
        id: u64,
        url: String,
        details: String,
    },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct BrowserSelection {
    text: String,
    url: String,
    title: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ViewportMetrics {
    inner_width: f64,
    scroll_width: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BrowserWidthMetrics {
    width: i32,
    total_width: i32,
}

struct BrowserTab {
    id: u64,
    title: String,
    url: String,
    host: gtk::Box,
    webview: WebView,
    zoom: f64,
    fit_width: bool,
    crashed: bool,
    internal_navigation: Rc<Cell<bool>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BrowserDownloadState {
    Downloading,
    Completed,
    Failed,
}

struct BrowserDownload {
    url: String,
    path: PathBuf,
    state: BrowserDownloadState,
}

struct EmbeddedBrowserWorkspace {
    _window: tauri::Window,
    split: gtk::Paned,
    browser_column: gtk::Fixed,
    toolbar_host: gtk::Fixed,
    browser_stack: gtk::Stack,
    studio_webview: WebView,
    toolbar_webview: Option<WebView>,
    tabs: Vec<BrowserTab>,
    active_tab_id: u64,
    next_tab_id: u64,
    resize_generation: u64,
    preferences: BrowserPreferences,
    visible: bool,
    browser_width: i32,
    _studio_context: WebContext,
    _toolbar_context: Option<WebContext>,
    browser_context: Option<WebContext>,
    download_handler_installed: bool,
    browser_profile_directory: PathBuf,
    browser_cache_directory: PathBuf,
    webkit_version: String,
    recent_recoveries: HashMap<u64, Instant>,
    toolbar_recovery: Option<Instant>,
    downloads: Vec<BrowserDownload>,
    overflow_menu: Option<gtk::Menu>,
    info_dialog: Option<gtk::Dialog>,
    downloads_dialog: Option<gtk::Dialog>,
    translations: HashMap<String, String>,
}

pub fn is_supported() -> bool {
    env::var_os("WAYLAND_DISPLAY")
        .filter(|value| !value.is_empty())
        .is_some()
}

#[cfg(debug_assertions)]
pub fn capture_studio_screenshot(
    path: PathBuf,
    complete: impl FnOnce(Result<(), String>) + 'static,
) {
    let webview = WORKSPACE.with(|slot| {
        let slot = slot
            .try_borrow()
            .map_err(|_| "Studio window is busy".to_owned())?;
        let workspace = slot
            .as_ref()
            .ok_or("embedded Studio workspace is not initialized")?;
        Ok::<_, String>(workspace.studio_webview.webview().clone())
    });
    let webview = match webview {
        Ok(webview) => webview,
        Err(error) => return complete(Err(error)),
    };
    let allocation = webview.allocation();
    webview.snapshot(
        SnapshotRegion::Visible,
        SnapshotOptions::NONE,
        None::<&gtk::gio::Cancellable>,
        move |result| {
            let result = result
                .map_err(|error| error.to_string())
                .and_then(|surface| {
                    if allocation.width() <= 0 || allocation.height() <= 0 {
                        return Err("Studio WebView has no drawable area".to_owned());
                    }
                    let pixbuf = gtk::gdk::pixbuf_get_from_surface(
                        &surface,
                        0,
                        0,
                        allocation.width(),
                        allocation.height(),
                    )
                    .ok_or("Studio WebView snapshot could not be converted")?;
                    pixbuf
                        .savev(&path, "png", &[])
                        .map_err(|error| error.to_string())?;
                    fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o600))
                        .map_err(|error| error.to_string())
                });
            complete(result);
        },
    );
}

#[cfg(debug_assertions)]
pub fn evaluate_studio_for_debug(script: &str) -> Result<(), String> {
    WORKSPACE.with(|slot| {
        let slot = slot
            .try_borrow()
            .map_err(|_| "embedded Studio workspace is busy".to_owned())?;
        let workspace = slot
            .as_ref()
            .ok_or("embedded Studio workspace is not initialized")?;
        workspace
            .studio_webview
            .evaluate_script(script)
            .map_err(|error| error.to_string())
    })
}

#[cfg(debug_assertions)]
pub fn show_for_debug(url: Option<String>) -> Result<(), String> {
    if let Some(raw) = url.as_deref() {
        WORKSPACE.with(|slot| {
            let slot = slot
                .try_borrow()
                .map_err(|_| "Studio browser is busy".to_owned())?;
            let workspace = slot.as_ref().ok_or("Studio browser is not initialized")?;
            validate_browser_url(raw, &workspace.preferences)
                .map(|_| ())
                .map_err(|error| error.to_string())
        })?;
    }

    let visible = WORKSPACE.with(|slot| {
        slot.try_borrow()
            .ok()
            .and_then(|slot| slot.as_ref().map(|workspace| workspace.visible))
            .ok_or("Studio browser is not initialized")
    })?;
    if !visible {
        toggle_workspace();
    }
    let ready = WORKSPACE.with(|slot| {
        slot.try_borrow()
            .ok()
            .and_then(|slot| {
                slot.as_ref()
                    .map(|workspace| workspace.visible && browser_runtime_loaded(workspace))
            })
            .unwrap_or(false)
    });
    if !ready {
        return Err("Studio browser could not be initialized".to_owned());
    }
    if let Some(url) = url {
        dispatch_action(BrowserAction::Navigate(url));
    }
    Ok(())
}

#[cfg(debug_assertions)]
pub fn hide_for_debug() -> Result<(), String> {
    let visible = WORKSPACE.with(|slot| {
        slot.try_borrow()
            .ok()
            .and_then(|slot| slot.as_ref().map(|workspace| workspace.visible))
            .ok_or("Studio browser is not initialized")
    })?;
    if visible {
        toggle_workspace();
    }
    let hidden = WORKSPACE.with(|slot| {
        slot.try_borrow()
            .ok()
            .and_then(|slot| slot.as_ref().map(|workspace| !workspace.visible))
            .unwrap_or(false)
    });
    hidden
        .then_some(())
        .ok_or_else(|| "Studio browser could not be hidden".to_owned())
}

#[cfg(debug_assertions)]
pub fn exit_for_debug() -> Result<(), String> {
    dispatch_action(BrowserAction::Exit);
    let released = WORKSPACE.with(|slot| {
        slot.try_borrow()
            .ok()
            .and_then(|slot| {
                slot.as_ref()
                    .map(|workspace| !browser_runtime_loaded(workspace))
            })
            .unwrap_or(false)
    });
    released
        .then_some(())
        .ok_or_else(|| "Studio browser runtime could not be released".to_owned())
}

#[cfg(debug_assertions)]
pub fn new_tab_for_debug(url: String) -> Result<(), String> {
    show_for_debug(None)?;
    let expected = WORKSPACE.with(|slot| {
        let slot = slot
            .try_borrow()
            .map_err(|_| "Studio browser is busy".to_owned())?;
        let workspace = slot.as_ref().ok_or("Studio browser is not initialized")?;
        validate_browser_url(&url, &workspace.preferences)
            .map(|url| url.to_string())
            .map_err(|error| error.to_string())
    })?;
    dispatch_action(BrowserAction::NewTab(Some(expected.clone())));
    WORKSPACE.with(|slot| {
        let slot = slot
            .try_borrow()
            .map_err(|_| "Studio browser is busy".to_owned())?;
        let workspace = slot.as_ref().ok_or("Studio browser is not initialized")?;
        workspace
            .tabs
            .iter()
            .any(|tab| tab.url == expected)
            .then_some(())
            .ok_or_else(|| "Studio browser did not create the requested tab".to_owned())
    })
}

#[cfg(debug_assertions)]
pub fn show_info_for_debug() -> Result<(), String> {
    show_for_debug(None)?;
    WORKSPACE.with(|slot| {
        let mut slot = slot
            .try_borrow_mut()
            .map_err(|_| "Studio browser is busy".to_owned())?;
        let workspace = slot.as_mut().ok_or("Studio browser is not initialized")?;
        show_browser_info(workspace);
        Ok(())
    })
}

#[cfg(debug_assertions)]
pub fn show_downloads_for_debug() -> Result<(), String> {
    show_for_debug(None)?;
    dispatch_action(BrowserAction::ShowDownloads);
    Ok(())
}

#[cfg(debug_assertions)]
pub fn crash_active_tab_for_debug() -> Result<(), String> {
    show_for_debug(None)?;
    WORKSPACE.with(|slot| {
        let slot = slot
            .try_borrow()
            .map_err(|_| "Studio browser is busy".to_owned())?;
        let workspace = slot.as_ref().ok_or("Studio browser is not initialized")?;
        let tab = active_tab(workspace).ok_or("Studio browser has no active tab")?;
        let id = tab.id;
        tab.webview.webview().terminate_web_process();
        // API termination is normally ignored because destroying a WebView also emits it. The
        // debug command is the intentional exception: enqueue exactly one synthetic failure so
        // the same recovery and circuit-breaker path used for real renderer crashes is exercised.
        queue_action(BrowserAction::PageTerminated {
            id,
            reason: WebProcessTerminationReason::TerminatedByApi,
        });
        Ok(())
    })
}

#[cfg(debug_assertions)]
pub fn show_menu_for_debug() -> Result<(), String> {
    show_for_debug(None)?;
    WORKSPACE.with(|slot| {
        let mut slot = slot
            .try_borrow_mut()
            .map_err(|_| "Studio browser is busy".to_owned())?;
        let workspace = slot.as_mut().ok_or("Studio browser is not initialized")?;
        show_browser_menu(workspace);
        Ok(())
    })
}

pub fn build(
    app: &tauri::App,
    studio_url: Url,
    initialization_script: &str,
    preferences: BrowserPreferences,
) -> Result<(), Box<dyn std::error::Error>> {
    if !is_supported() {
        return Err("embedded browser requires a native Wayland session".into());
    }

    env::set_var("GDK_BACKEND", "wayland");
    install_browser_native_styles();
    let window = tauri::window::WindowBuilder::new(app, "main")
        .title("Codex Thread Studio")
        .inner_size(1400.0, 900.0)
        .min_inner_size(1040.0, 660.0)
        .build()?;
    let root = window.default_vbox()?;

    let split = gtk::Paned::new(gtk::Orientation::Horizontal);
    split.set_wide_handle(true);
    split.style_context().add_class("studio-browser-split");

    let studio_host = gtk::Box::new(gtk::Orientation::Vertical, 0);
    studio_host.set_size_request(MIN_STUDIO_WIDTH, -1);
    // WebKitGTK reports a large natural height for a WebView. A GtkBox uses that
    // natural height even when the toolbar host has a fixed size request, which
    // can leave the page stack with only half of the available height. GtkFixed
    // lets this split pane own the geometry explicitly: toolbar=75px, page=rest.
    let browser_column = gtk::Fixed::new();
    browser_column.set_size_request(MIN_BROWSER_WIDTH, -1);
    let toolbar_host = gtk::Fixed::new();
    toolbar_host.set_size_request(-1, TOOLBAR_HEIGHT);
    let browser_stack = gtk::Stack::new();
    browser_stack.set_transition_type(gtk::StackTransitionType::None);
    browser_stack.set_hexpand(true);
    browser_stack.set_vexpand(true);

    browser_column.put(&toolbar_host, 0, 0);
    browser_column.put(&browser_stack, 0, TOOLBAR_HEIGHT);
    let toolbar_layout_host = toolbar_host.clone();
    let browser_layout_stack = browser_stack.clone();
    browser_column.connect_size_allocate(move |column, allocation| {
        let width = allocation.width().max(1);
        let page_height = browser_page_height(allocation.height());
        column.move_(&toolbar_layout_host, 0, 0);
        toolbar_layout_host.size_allocate(&gtk::Allocation::new(0, 0, width, TOOLBAR_HEIGHT));
        column.move_(&browser_layout_stack, 0, TOOLBAR_HEIGHT);
        browser_layout_stack.size_allocate(&gtk::Allocation::new(
            0,
            TOOLBAR_HEIGHT,
            width,
            page_height,
        ));
    });
    split.pack1(&studio_host, true, false);
    // The browser keeps its width while the window resizes; extra space belongs to Studio.
    split.pack2(&browser_column, false, false);
    root.pack_start(&split, true, true, 0);
    split.show_all();

    let studio_profile = profile_directory("studio-shell");
    let browser_profile = profile_directory("embedded-browser");
    fs::create_dir_all(&studio_profile)?;
    let mut studio_context = WebContext::new(Some(studio_profile));
    let browser_cache = browser_cache_directory();
    let webkit_version = wry::webview_version().unwrap_or_else(|_| "Unknown".to_owned());

    let studio_webview = WebViewBuilder::with_web_context(&mut studio_context)
        .with_url(studio_url.as_str())
        .with_initialization_script(initialization_script)
        .with_clipboard(true)
        .with_navigation_handler(|url| intercept_action(&url))
        .build_gtk(&studio_host)?;

    toolbar_host.connect_size_allocate(|host, allocation| {
        for child in host.children() {
            host.move_(&child, 0, 0);
            child.size_allocate(&gtk::Allocation::new(
                0,
                0,
                allocation.width().max(1),
                TOOLBAR_HEIGHT,
            ));
        }
    });

    split.show_all();
    browser_column.hide();
    let split_signal = split.clone();
    let browser_width = preferred_browser_width(&preferences);
    WORKSPACE.with(|slot| {
        *slot.borrow_mut() = Some(EmbeddedBrowserWorkspace {
            _window: window,
            split,
            browser_column,
            toolbar_host,
            browser_stack,
            studio_webview,
            toolbar_webview: None,
            tabs: Vec::new(),
            active_tab_id: 0,
            next_tab_id: 1,
            resize_generation: 0,
            preferences,
            visible: false,
            browser_width,
            _studio_context: studio_context,
            _toolbar_context: None,
            browser_context: None,
            download_handler_installed: false,
            browser_profile_directory: browser_profile,
            browser_cache_directory: browser_cache,
            webkit_version,
            recent_recoveries: HashMap::new(),
            toolbar_recovery: None,
            downloads: Vec::new(),
            overflow_menu: None,
            info_dialog: None,
            downloads_dialog: None,
            translations: HashMap::new(),
        });
    });
    split_signal.connect_position_notify(|split| {
        let generation = WORKSPACE.with(|slot| {
            let Ok(mut slot) = slot.try_borrow_mut() else {
                return None;
            };
            let workspace = slot.as_mut().filter(|workspace| workspace.visible)?;
            let width = workspace
                .browser_column
                .allocation()
                .width()
                .max(MIN_BROWSER_WIDTH);
            workspace.browser_width = width;
            workspace.resize_generation = workspace.resize_generation.wrapping_add(1);
            evaluate(
                &workspace.studio_webview,
                "window.__studioEmbeddedBrowser?.setWidth",
                &BrowserWidthMetrics {
                    width,
                    total_width: split.allocation().width(),
                },
            );
            Some(workspace.resize_generation)
        });
        if let Some(generation) = generation {
            gtk::glib::timeout_add_local_once(Duration::from_millis(180), move || {
                let should_measure = WORKSPACE.with(|slot| {
                    slot.try_borrow()
                        .ok()
                        .and_then(|slot| slot.as_ref().map(|workspace| workspace.resize_generation))
                        == Some(generation)
                });
                if should_measure {
                    measure_active_fit();
                }
            });
        }
    });
    sync_toolbar();
    Ok(())
}

fn browser_runtime_loaded(workspace: &EmbeddedBrowserWorkspace) -> bool {
    browser_runtime_parts_loaded(
        workspace.toolbar_webview.is_some(),
        workspace._toolbar_context.is_some(),
        workspace.browser_context.is_some(),
        workspace.tabs.len(),
    )
}

fn browser_runtime_parts_loaded(
    toolbar_webview: bool,
    toolbar_context: bool,
    browser_context: bool,
    tab_count: usize,
) -> bool {
    toolbar_webview && toolbar_context && browser_context && tab_count > 0
}

fn ensure_browser_runtime(workspace: &mut EmbeddedBrowserWorkspace) -> Result<(), String> {
    if browser_runtime_loaded(workspace) {
        return Ok(());
    }

    // A previous failed attempt must not leave native children behind. Runtime fields are
    // assigned only after both WebViews build successfully, so retrying remains deterministic.
    for child in workspace.toolbar_host.children() {
        workspace.toolbar_host.remove(&child);
    }
    for child in workspace.browser_stack.children() {
        workspace.browser_stack.remove(&child);
    }
    workspace.toolbar_webview = None;
    workspace.tabs.clear();
    workspace.active_tab_id = 0;
    workspace.next_tab_id = 1;

    let toolbar_profile = profile_directory("browser-toolbar");
    fs::create_dir_all(&toolbar_profile).map_err(|error| error.to_string())?;
    fs::create_dir_all(&workspace.browser_profile_directory).map_err(|error| error.to_string())?;

    if workspace._toolbar_context.is_none() {
        workspace._toolbar_context = Some(WebContext::new(Some(toolbar_profile)));
    }
    if workspace.browser_context.is_none() {
        workspace.browser_context = Some(WebContext::new(Some(
            workspace.browser_profile_directory.clone(),
        )));
        workspace.download_handler_installed = false;
    }
    let toolbar_host = workspace.toolbar_host.clone();
    let browser_stack = workspace.browser_stack.clone();
    let toolbar_context = workspace
        ._toolbar_context
        .as_mut()
        .expect("toolbar context was initialized");
    let toolbar_webview = WebViewBuilder::with_web_context(toolbar_context)
        .with_html(TOOLBAR_HTML)
        .with_clipboard(true)
        .with_navigation_handler(|url| intercept_action(&url))
        .build_gtk(&toolbar_host)
        .map_err(|error| error.to_string())?;
    toolbar_webview
        .webview()
        .connect_web_process_terminated(|_, reason| {
            if reason != WebProcessTerminationReason::TerminatedByApi {
                queue_action(BrowserAction::ToolbarTerminated(reason));
            }
        });
    toolbar_webview
        .webview()
        .set_size_request(1, TOOLBAR_HEIGHT);

    let browser_context = workspace
        .browser_context
        .as_mut()
        .expect("browser context was initialized");
    let initial_tab = match build_browser_tab(
        browser_context,
        &browser_stack,
        1,
        DEFAULT_URL,
        &workspace.preferences,
    ) {
        Ok(tab) => tab,
        Err(error) => {
            drop(toolbar_webview);
            for child in workspace.toolbar_host.children() {
                workspace.toolbar_host.remove(&child);
            }
            for child in workspace.browser_stack.children() {
                workspace.browser_stack.remove(&child);
            }
            return Err(error.to_string());
        }
    };
    workspace.browser_stack.set_visible_child(&initial_tab.host);
    if !workspace.download_handler_installed {
        install_download_handler(&initial_tab.webview);
        workspace.download_handler_installed = true;
    }
    workspace.toolbar_webview = Some(toolbar_webview);
    workspace.tabs = vec![initial_tab];
    workspace.active_tab_id = 1;
    workspace.next_tab_id = 2;
    Ok(())
}

fn build_browser_tab(
    context: &mut WebContext,
    stack: &gtk::Stack,
    id: u64,
    url: &str,
    preferences: &BrowserPreferences,
) -> Result<BrowserTab, wry::Error> {
    let host = gtk::Box::new(gtk::Orientation::Vertical, 0);
    host.set_hexpand(true);
    host.set_vexpand(true);
    stack.add_named(&host, &format!("tab-{id}"));

    let navigation_preferences = preferences.clone();
    let internal_navigation = Rc::new(Cell::new(false));
    let internal_navigation_for_policy = internal_navigation.clone();
    let internal_navigation_for_title = internal_navigation.clone();
    let internal_navigation_for_load = internal_navigation.clone();
    let builder = WebViewBuilder::with_web_context(context)
        .with_url(url)
        .with_clipboard(true)
        .with_navigation_handler(move |url| {
            if internal_navigation_for_policy.get()
                && matches!(url.as_str(), "about:blank" | "about:srcdoc")
            {
                return true;
            }
            match validate_browser_url(&url, &navigation_preferences) {
                Ok(_) => true,
                Err(error) => {
                    queue_action(BrowserAction::NavigationRejected {
                        url,
                        reason: error.to_string(),
                    });
                    false
                }
            }
        })
        .with_document_title_changed_handler(move |title| {
            if !internal_navigation_for_title.get() {
                update_tab_navigation(id, None, Some(title), false);
            }
        })
        .with_on_page_load_handler(move |event, url| {
            let finished = matches!(event, PageLoadEvent::Finished);
            if internal_navigation_for_load.get() {
                if finished {
                    internal_navigation_for_load.set(false);
                }
                return;
            }
            update_tab_navigation(id, Some(url), None, finished);
        })
        .with_new_window_req_handler(move |url| {
            queue_action(BrowserAction::NewTab(Some(url)));
            false
        });
    let webview = builder.build_gtk(&host)?;
    install_page_guards(&webview, id);
    webview.webview().connect_key_press_event(move |_, event| {
        use gtk::gdk::{keys::constants, ModifierType};

        let modifiers = event.state();
        if !modifiers.contains(ModifierType::CONTROL_MASK)
            || modifiers.contains(ModifierType::MOD1_MASK)
        {
            return gtk::glib::Propagation::Proceed;
        }
        let action = match event.keyval() {
            constants::t | constants::T if !modifiers.contains(ModifierType::SHIFT_MASK) => {
                Some(BrowserAction::NewTab(None))
            }
            constants::w | constants::W if !modifiers.contains(ModifierType::SHIFT_MASK) => {
                Some(BrowserAction::CloseTab(id))
            }
            constants::Tab => Some(BrowserAction::NextTab),
            constants::ISO_Left_Tab => Some(BrowserAction::PreviousTab),
            _ => None,
        };
        if let Some(action) = action {
            queue_action(action);
            gtk::glib::Propagation::Stop
        } else {
            gtk::glib::Propagation::Proceed
        }
    });
    host.show_all();

    Ok(BrowserTab {
        id,
        title: display_title("", url),
        url: url.to_owned(),
        host,
        webview,
        zoom: 1.0,
        fit_width: true,
        crashed: false,
        internal_navigation,
    })
}

fn install_download_handler(webview: &WebView) {
    let Some(context) = webview.webview().context() else {
        return;
    };
    context.connect_download_started(|_, download| {
        let url = download
            .request()
            .and_then(|request| request.uri())
            .map(|value| value.to_string())
            .unwrap_or_default();
        let destination = Rc::new(RefCell::new(None::<PathBuf>));
        let failed = Rc::new(Cell::new(false));

        let url_for_response = url.clone();
        let destination_for_response = destination.clone();
        download.connect_response_notify(move |download| {
            if destination_for_response.borrow().is_some() {
                return;
            }
            let filename = download
                .response()
                .and_then(|response| response.suggested_filename())
                .map(|value| sanitize_download_filename(&value))
                .unwrap_or_else(|| download_filename(&url_for_response));
            match download_destination_for_name(&filename) {
                Ok(path) => {
                    download.set_destination(&path.to_string_lossy());
                    *destination_for_response.borrow_mut() = Some(path.clone());
                    queue_action(BrowserAction::DownloadStarted {
                        url: url_for_response.clone(),
                        path,
                    });
                }
                Err(error) => {
                    download.cancel();
                    queue_action(BrowserAction::DownloadRejected(error));
                }
            }
        });
        let failed_for_signal = failed.clone();
        download.connect_failed(move |_, _| failed_for_signal.set(true));
        download.connect_finished(move |_| {
            let path = destination.borrow().clone();
            queue_action(BrowserAction::DownloadFinished {
                url: url.clone(),
                success: !failed.get() && path.is_some(),
                path,
            });
        });
    });
}

fn install_page_guards(webview: &WebView, id: u64) {
    webview
        .webview()
        .connect_web_process_terminated(move |_, reason| {
            if reason != WebProcessTerminationReason::TerminatedByApi {
                queue_action(BrowserAction::PageTerminated { id, reason });
            }
        });
    webview
        .webview()
        .connect_load_failed_with_tls_errors(move |_, url, _, errors| {
            queue_action(BrowserAction::TlsError {
                id,
                url: url.to_owned(),
                details: format!("{errors:?}"),
            });
            true
        });
    webview.webview().connect_permission_request(|_, request| {
        let sensitive = request.is::<webkit2gtk::UserMediaPermissionRequest>()
            || request.is::<webkit2gtk::DeviceInfoPermissionRequest>()
            || request.is::<webkit2gtk::GeolocationPermissionRequest>()
            || request.is::<webkit2gtk::NotificationPermissionRequest>()
            || request.is::<webkit2gtk::PointerLockPermissionRequest>();
        if sensitive {
            request.deny();
            true
        } else {
            false
        }
    });
}

fn intercept_action(url: &str) -> bool {
    if let Some(action) = parse_action(url) {
        queue_action(action);
        false
    } else {
        true
    }
}

fn queue_action(action: BrowserAction) {
    // Navigation and key handlers run inside GTK/WebKit callbacks. Defer mutations until the
    // callback unwinds so closing or creating a WebView cannot re-enter WebKit's signal stack.
    gtk::glib::idle_add_local_once(move || dispatch_action(action));
}

fn dispatch_action(action: BrowserAction) {
    if let BrowserAction::Open(url, width) = &action {
        let url = url.clone();
        set_browser_width(*width);
        show_workspace();
        dispatch_action(BrowserAction::Navigate(url));
        return;
    }
    if matches!(action, BrowserAction::Exit) {
        exit_workspace();
        return;
    }
    if let BrowserAction::Show(width) = &action {
        set_browser_width(*width);
        show_workspace();
        return;
    }
    if matches!(action, BrowserAction::Hide) {
        hide_workspace();
        return;
    }
    if matches!(action, BrowserAction::Toggle) {
        toggle_workspace();
        return;
    }
    if let BrowserAction::Resize(width) = &action {
        resize_browser(*width);
        return;
    }

    WORKSPACE.with(|slot| {
        let Ok(mut slot) = slot.try_borrow_mut() else {
            return;
        };
        let Some(workspace) = slot.as_mut() else {
            return;
        };
        if !matches!(&action, BrowserAction::ShowMenu) {
            if let Some(menu) = workspace.overflow_menu.take() {
                menu.popdown();
            }
        }
        match action {
            BrowserAction::Toggle
            | BrowserAction::Show(_)
            | BrowserAction::Hide
            | BrowserAction::Open(_, _)
            | BrowserAction::Resize(_) => {
                unreachable!("workspace visibility actions are handled without a nested borrow")
            }
            BrowserAction::Exit => unreachable!("exit is handled without a nested borrow"),
            BrowserAction::SetTranslations(translations) => {
                workspace.translations = translations;
                if let Some(menu) = workspace.overflow_menu.take() {
                    menu.popdown();
                }
                if let Some(dialog) = workspace.info_dialog.take() {
                    dialog.close();
                }
                if let Some(dialog) = workspace.downloads_dialog.take() {
                    dialog.close();
                }
            }
            BrowserAction::Navigate(raw) => {
                match validate_browser_url(&raw, &workspace.preferences) {
                    Ok(url) => {
                        if let Some(tab) = active_tab_mut(workspace) {
                            tab.fit_width = true;
                            tab.zoom = 1.0;
                            let _ = tab.webview.zoom(1.0);
                            let _ = tab.webview.load_url(url.as_str());
                            let _ = tab.webview.focus();
                        }
                    }
                    Err(error) => notify(&workspace.studio_webview, &error.to_string()),
                }
            }
            BrowserAction::NewTab(raw) => {
                let raw = raw.unwrap_or_else(|| DEFAULT_URL.to_owned());
                match validate_browser_url(&raw, &workspace.preferences) {
                    Ok(url) => {
                        let id = workspace.next_tab_id;
                        workspace.next_tab_id = workspace.next_tab_id.wrapping_add(1).max(1);
                        let Some(browser_context) = workspace.browser_context.as_mut() else {
                            notify(
                                &workspace.studio_webview,
                                "Embedded browser is not initialized",
                            );
                            return;
                        };
                        match build_browser_tab(
                            browser_context,
                            &workspace.browser_stack,
                            id,
                            url.as_str(),
                            &workspace.preferences,
                        ) {
                            Ok(tab) => {
                                workspace.browser_stack.set_visible_child(&tab.host);
                                workspace.tabs.push(tab);
                                workspace.active_tab_id = id;
                            }
                            Err(error) => notify(
                                &workspace.studio_webview,
                                &format!("Unable to create browser tab: {error}"),
                            ),
                        }
                    }
                    Err(error) => notify(&workspace.studio_webview, &error.to_string()),
                }
            }
            BrowserAction::ActivateTab(id) => {
                if let Some(tab) = workspace.tabs.iter().find(|tab| tab.id == id) {
                    workspace.browser_stack.set_visible_child(&tab.host);
                    workspace.active_tab_id = id;
                    let _ = tab.webview.focus();
                }
            }
            BrowserAction::CloseTab(id) => close_tab(workspace, id),
            BrowserAction::NextTab => cycle_tab(workspace, 1),
            BrowserAction::PreviousTab => cycle_tab(workspace, -1),
            BrowserAction::Back => {
                if let Some(tab) = active_tab(workspace) {
                    let _ = tab.webview.evaluate_script("history.back()");
                }
            }
            BrowserAction::Forward => {
                if let Some(tab) = active_tab(workspace) {
                    let _ = tab.webview.evaluate_script("history.forward()");
                }
            }
            BrowserAction::Reload => {
                let active = workspace.active_tab_id;
                if active_tab(workspace).is_some_and(|tab| tab.crashed) {
                    workspace.recent_recoveries.remove(&active);
                    recover_tab(workspace, active, false);
                } else if let Some(tab) = active_tab(workspace) {
                    let _ = tab.webview.load_url(&tab.url);
                }
            }
            BrowserAction::ZoomIn => adjust_zoom(workspace, ZOOM_STEP),
            BrowserAction::ZoomOut => adjust_zoom(workspace, -ZOOM_STEP),
            BrowserAction::ZoomReset => {
                if let Some(tab) = active_tab_mut(workspace) {
                    tab.fit_width = false;
                    tab.zoom = 1.0;
                    let _ = tab.webview.zoom(1.0);
                }
            }
            BrowserAction::FitWidth => {
                if let Some(tab) = active_tab_mut(workspace) {
                    tab.fit_width = true;
                }
                schedule_fit_measure(workspace.active_tab_id, 20);
            }
            BrowserAction::CommentSelection => {
                if let Some(tab) = active_tab(workspace) {
                    let result =
                        tab.webview
                            .evaluate_script_with_callback(READ_SELECTION_SCRIPT, |value| {
                                gtk::glib::MainContext::default().invoke(move || {
                                    open_comment(parse_selection(&value));
                                });
                            });
                    if let Err(error) = result {
                        notify(
                            &workspace.studio_webview,
                            &format!("Unable to read browser selection: {error}"),
                        );
                    }
                }
            }
            BrowserAction::CopyUrl => {
                if let Some(tab) = active_tab(workspace) {
                    copy_to_clipboard(&tab.url);
                }
            }
            BrowserAction::ShowMenu => show_browser_menu(workspace),
            BrowserAction::ShowInfo => show_browser_info(workspace),
            BrowserAction::ShowDownloads => show_downloads(workspace),
            BrowserAction::PageTerminated { id, reason } => {
                handle_page_termination(workspace, id, reason)
            }
            BrowserAction::ToolbarTerminated(reason) => {
                handle_toolbar_termination(workspace, reason)
            }
            BrowserAction::DownloadStarted { url, path } => {
                workspace.downloads.insert(
                    0,
                    BrowserDownload {
                        url,
                        path,
                        state: BrowserDownloadState::Downloading,
                    },
                );
                workspace.downloads.truncate(MAX_DOWNLOAD_RECORDS);
                refresh_downloads_if_open(workspace);
            }
            BrowserAction::DownloadFinished { url, path, success } => {
                finish_download(workspace, &url, path.as_deref(), success);
            }
            BrowserAction::DownloadRejected(error) => {
                notify(&workspace.studio_webview, &error);
            }
            BrowserAction::NavigationRejected { url, reason } => {
                notify(
                    &workspace.studio_webview,
                    &format!("Blocked unsafe navigation: {reason} ({url})"),
                );
            }
            BrowserAction::TlsError { id, url, details } => {
                show_tls_error(workspace, id, &url, &details);
            }
        }
    });
    sync_toolbar();
}

fn close_tab(workspace: &mut EmbeddedBrowserWorkspace, id: u64) {
    let Some(index) = workspace.tabs.iter().position(|tab| tab.id == id) else {
        return;
    };
    if workspace.tabs.len() == 1 {
        let tab = &mut workspace.tabs[0];
        tab.title = display_title("", DEFAULT_URL);
        tab.url = DEFAULT_URL.to_owned();
        tab.zoom = 1.0;
        tab.fit_width = true;
        let _ = tab.webview.zoom(1.0);
        let _ = tab.webview.load_url(DEFAULT_URL);
        return;
    }

    let was_active = workspace.active_tab_id == id;
    let tab = workspace.tabs.remove(index);
    workspace.browser_stack.remove(&tab.host);
    drop(tab);
    if was_active {
        let next_index = index.min(workspace.tabs.len() - 1);
        let next = &workspace.tabs[next_index];
        workspace.active_tab_id = next.id;
        workspace.browser_stack.set_visible_child(&next.host);
        let _ = next.webview.focus();
    }
}

fn cycle_tab(workspace: &mut EmbeddedBrowserWorkspace, delta: isize) {
    if workspace.tabs.len() < 2 {
        return;
    }
    let current = workspace
        .tabs
        .iter()
        .position(|tab| tab.id == workspace.active_tab_id)
        .unwrap_or(0) as isize;
    let len = workspace.tabs.len() as isize;
    let next = (current + delta).rem_euclid(len) as usize;
    let tab = &workspace.tabs[next];
    workspace.active_tab_id = tab.id;
    workspace.browser_stack.set_visible_child(&tab.host);
    let _ = tab.webview.focus();
}

fn adjust_zoom(workspace: &mut EmbeddedBrowserWorkspace, delta: f64) {
    if let Some(tab) = active_tab_mut(workspace) {
        tab.fit_width = false;
        tab.zoom = round_zoom((tab.zoom + delta).clamp(MIN_ZOOM, MAX_ZOOM));
        let _ = tab.webview.zoom(tab.zoom);
    }
}

fn handle_page_termination(
    workspace: &mut EmbeddedBrowserWorkspace,
    id: u64,
    reason: WebProcessTerminationReason,
) {
    let repeated = workspace
        .recent_recoveries
        .get(&id)
        .is_some_and(|attempt| attempt.elapsed() < RECOVERY_WINDOW);
    if repeated {
        if let Some(tab) = workspace.tabs.iter_mut().find(|tab| tab.id == id) {
            tab.crashed = true;
            tab.title = match reason {
                WebProcessTerminationReason::ExceededMemoryLimit => {
                    "Page memory limit exceeded".to_owned()
                }
                _ => "Page process exited unexpectedly".to_owned(),
            };
        }
        notify(
            &workspace.studio_webview,
            "The page repeatedly crashed. Reload it manually to recover.",
        );
        return;
    }
    workspace.recent_recoveries.insert(id, Instant::now());
    recover_tab(workspace, id, true);
}

fn recover_tab(workspace: &mut EmbeddedBrowserWorkspace, id: u64, automatic: bool) {
    let Some(index) = workspace.tabs.iter().position(|tab| tab.id == id) else {
        return;
    };
    let was_active = workspace.active_tab_id == id;
    let old = workspace.tabs.remove(index);
    let url = old.url.clone();
    let zoom = old.zoom;
    let fit_width = old.fit_width;
    workspace.browser_stack.remove(&old.host);
    drop(old);

    let Some(context) = workspace.browser_context.as_mut() else {
        return;
    };
    match build_browser_tab(
        context,
        &workspace.browser_stack,
        id,
        &url,
        &workspace.preferences,
    ) {
        Ok(mut tab) => {
            tab.zoom = zoom;
            tab.fit_width = fit_width;
            let _ = tab.webview.zoom(zoom);
            workspace.tabs.insert(index, tab);
            if was_active {
                workspace.active_tab_id = id;
                workspace
                    .browser_stack
                    .set_visible_child(&workspace.tabs[index].host);
            }
            if automatic {
                notify(
                    &workspace.studio_webview,
                    "The page renderer recovered automatically",
                );
            }
        }
        Err(error) => {
            notify(
                &workspace.studio_webview,
                &format!("Unable to recover browser tab: {error}"),
            );
            if workspace.tabs.is_empty() {
                queue_action(BrowserAction::Exit);
            }
        }
    }
}

fn handle_toolbar_termination(
    workspace: &mut EmbeddedBrowserWorkspace,
    _reason: WebProcessTerminationReason,
) {
    let repeated = workspace
        .toolbar_recovery
        .is_some_and(|attempt| attempt.elapsed() < RECOVERY_WINDOW);
    if repeated {
        notify(
            &workspace.studio_webview,
            "The browser toolbar repeatedly crashed, so the browser was closed",
        );
        queue_action(BrowserAction::Exit);
        return;
    }
    workspace.toolbar_recovery = Some(Instant::now());
    if let Some(toolbar) = workspace.toolbar_webview.as_ref() {
        if let Err(error) = toolbar.load_html(TOOLBAR_HTML) {
            notify(
                &workspace.studio_webview,
                &format!("Unable to recover browser toolbar: {error}"),
            );
            queue_action(BrowserAction::Exit);
        }
    }
}

fn show_tls_error(workspace: &mut EmbeddedBrowserWorkspace, id: u64, url: &str, details: &str) {
    let Some(tab) = workspace.tabs.iter_mut().find(|tab| tab.id == id) else {
        return;
    };
    tab.url = url.to_owned();
    tab.title = "Certificate error".to_owned();
    tab.crashed = false;
    tab.internal_navigation.set(true);
    let html = tls_error_html(url, details);
    if let Err(error) = tab.webview.load_html(&html) {
        tab.internal_navigation.set(false);
        notify(
            &workspace.studio_webview,
            &format!(
                "TLS certificate error for {url}: {details}; unable to show error page: {error}"
            ),
        );
    } else {
        notify(
            &workspace.studio_webview,
            &format!("Blocked a page with an invalid certificate: {url}"),
        );
    }
}

fn tls_error_html(url: &str, details: &str) -> String {
    format!(
        r#"<!doctype html><meta charset="utf-8"><meta name="color-scheme" content="light dark">
<title>Certificate error</title><style>
html,body{{height:100%;margin:0}}body{{display:grid;place-items:center;background:#f6f8fa;color:#172033;font:15px/1.6 system-ui,sans-serif}}
main{{width:min(620px,calc(100% - 48px));padding:30px;border:1px solid #d8e0e6;border-radius:14px;background:#fff;box-shadow:0 12px 32px #17203312}}
h1{{margin:0 0 10px;font-size:22px}}p{{margin:8px 0;color:#4d5968}}code{{display:block;margin-top:18px;padding:12px;overflow-wrap:anywhere;border-radius:8px;background:#f0f3f5;color:#263241}}
@media(prefers-color-scheme:dark){{body{{background:#101719;color:#edf3f4}}main{{background:#182124;border-color:#344246}}p{{color:#b7c2c5}}code{{background:#11191c;color:#dbe6e8}}}}
</style><main><h1>Unable to establish a secure connection</h1><p>Studio blocked a page with an invalid certificate. Check the system clock, URL, or site certificate, then reload.</p><code>{}</code><p>{}</p></main>"#,
        escape_html(url),
        escape_html(details)
    )
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn show_browser_menu(workspace: &mut EmbeddedBrowserWorkspace) {
    if let Some(menu) = workspace.overflow_menu.take() {
        let was_visible = menu.is_visible();
        menu.popdown();
        if was_visible {
            return;
        }
    }
    install_browser_native_styles();
    let menu = gtk::Menu::new();
    menu.set_size_request(292, -1);
    menu.style_context().add_class("browser-overflow-menu");
    menu.append(&browser_native_menu_item(
        browser_text(workspace, "New tab"),
        Some("Ctrl+T"),
        || queue_action(BrowserAction::NewTab(None)),
    ));
    menu.append(&browser_native_menu_item(
        browser_text(workspace, "Reload"),
        Some("Ctrl+R"),
        || queue_action(BrowserAction::Reload),
    ));
    menu.append(&browser_native_menu_item(
        browser_text(workspace, "Copy current link"),
        None,
        || queue_action(BrowserAction::CopyUrl),
    ));
    menu.append(&gtk::SeparatorMenuItem::new());
    menu.append(&browser_native_zoom_item(workspace));
    menu.append(&browser_native_menu_item(
        browser_text(workspace, "Fit page width"),
        None,
        || queue_action(BrowserAction::FitWidth),
    ));
    menu.append(&browser_native_menu_item(
        browser_text(workspace, "Comment on selection"),
        None,
        || queue_action(BrowserAction::CommentSelection),
    ));
    menu.append(&gtk::SeparatorMenuItem::new());
    menu.append(&browser_native_menu_item(
        browser_text(workspace, "Browser information"),
        Some("›"),
        || queue_action(BrowserAction::ShowInfo),
    ));
    menu.append(&browser_native_menu_item(
        browser_text(workspace, "Downloads"),
        Some("›"),
        || queue_action(BrowserAction::ShowDownloads),
    ));
    menu.append(&gtk::SeparatorMenuItem::new());
    menu.append(&browser_native_menu_item(
        browser_text(workspace, "Exit browser"),
        None,
        || queue_action(BrowserAction::Exit),
    ));
    menu.show_all();
    let Some(toolbar_webview) = workspace.toolbar_webview.as_ref() else {
        return;
    };
    menu.popup_at_widget(
        &toolbar_webview.webview(),
        gtk::gdk::Gravity::SouthEast,
        gtk::gdk::Gravity::NorthEast,
        None::<&gtk::gdk::Event>,
    );
    workspace.overflow_menu = Some(menu);
}

fn browser_native_menu_item(
    label: &str,
    trailing: Option<&str>,
    action: impl Fn() + 'static,
) -> gtk::MenuItem {
    let item = gtk::MenuItem::new();
    item.style_context().add_class("browser-native-menu-item");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.set_margin_start(9);
    row.set_margin_end(9);
    let label = gtk::Label::new(Some(label));
    label.set_xalign(0.0);
    label.set_hexpand(true);
    row.pack_start(&label, true, true, 0);
    if let Some(trailing) = trailing {
        let trailing = gtk::Label::new(Some(trailing));
        trailing.style_context().add_class("browser-menu-shortcut");
        row.pack_end(&trailing, false, false, 0);
    }
    item.add(&row);
    item.connect_activate(move |_| action());
    item
}

fn browser_native_zoom_item(workspace: &EmbeddedBrowserWorkspace) -> gtk::MenuItem {
    let item = gtk::MenuItem::new();
    item.style_context().add_class("browser-native-zoom-item");
    item.add(&browser_zoom_row(workspace));
    item
}

fn show_browser_info(workspace: &mut EmbeddedBrowserWorkspace) {
    if let Some(dialog) = workspace.info_dialog.take() {
        if dialog.is_visible() {
            dialog.present();
            workspace.info_dialog = Some(dialog);
            return;
        }
        dialog.close();
    }

    let parent = match workspace._window.gtk_window() {
        Ok(parent) => parent,
        Err(error) => {
            notify(
                &workspace.studio_webview,
                &format!("Unable to open browser information: {error}"),
            );
            return;
        }
    };
    let dialog = gtk::Dialog::new();
    dialog.set_title(browser_text(workspace, "Browser information"));
    dialog.set_transient_for(Some(&parent));
    dialog.set_destroy_with_parent(true);
    dialog.set_modal(false);
    dialog.set_resizable(false);
    dialog.set_default_size(520, -1);
    dialog.set_position(gtk::WindowPosition::CenterOnParent);
    install_browser_native_styles();
    dialog.style_context().add_class("browser-info-dialog");

    let info = gtk::Box::new(gtk::Orientation::Vertical, 9);
    info.set_margin_start(18);
    info.set_margin_end(18);
    info.set_margin_top(14);
    info.set_margin_bottom(14);

    let summary = gtk::Label::new(Some(&format!(
        "WebKitGTK {}  ·  {} {}",
        workspace.webkit_version,
        workspace.tabs.len(),
        browser_text(workspace, "tabs")
    )));
    summary.set_xalign(0.0);
    summary.style_context().add_class("dim-label");
    info.pack_start(&summary, false, false, 0);
    info.pack_start(
        &gtk::Separator::new(gtk::Orientation::Horizontal),
        false,
        false,
        1,
    );

    let profile = workspace.browser_profile_directory.display().to_string();
    let cookies = workspace
        .browser_profile_directory
        .join("cookies")
        .display()
        .to_string();
    let local_storage = workspace
        .browser_profile_directory
        .join("localstorage")
        .display()
        .to_string();
    let cache = workspace.browser_cache_directory.display().to_string();
    for (label, value) in [
        ("Profile", profile.as_str()),
        ("Cookies", cookies.as_str()),
        ("Local Storage", local_storage.as_str()),
        ("WebKit Cache", cache.as_str()),
    ] {
        info.pack_start(
            &browser_info_value_row(
                browser_text(workspace, label),
                value,
                browser_text(workspace, "Copy"),
                browser_text(workspace, "Copied"),
            ),
            false,
            false,
            0,
        );
    }

    info.pack_start(
        &gtk::Separator::new(gtk::Orientation::Horizontal),
        false,
        false,
        1,
    );
    let storage_title = gtk::Label::new(Some(browser_text(workspace, "Storage & cleanup")));
    storage_title.set_xalign(0.0);
    storage_title
        .style_context()
        .add_class("browser-menu-subheading");
    info.pack_start(&storage_title, false, false, 0);
    let storage = gtk::Label::new(Some(browser_text(
        workspace,
        "Cache, cookies, Local Storage, and other site data are stored on disk. Tab navigation history stays in memory and disappears when Studio exits.",
    )));
    storage.set_xalign(0.0);
    storage.set_line_wrap(true);
    storage.set_max_width_chars(62);
    storage.style_context().add_class("dim-label");
    info.pack_start(&storage, false, false, 0);

    let clear_status = gtk::Label::new(None);
    clear_status.set_xalign(0.0);
    clear_status.set_line_wrap(true);
    clear_status.style_context().add_class("dim-label");
    let clear_data = gtk::Button::with_label(browser_text(workspace, "Clear browsing data…"));
    clear_data.set_halign(gtk::Align::Start);
    clear_data.style_context().add_class("browser-clear-data");
    let dialog_for_confirmation = dialog.clone();
    let clear_status_for_confirmation = clear_status.clone();
    let translations = workspace.translations.clone();
    clear_data.connect_clicked(move |_| {
        confirm_clear_browser_data(
            &dialog_for_confirmation,
            clear_status_for_confirmation.clone(),
            &translations,
        );
    });
    info.pack_start(&clear_data, false, false, 0);
    info.pack_start(&clear_status, false, false, 0);

    dialog.content_area().add(&info);
    dialog.show_all();
    dialog.present();
    workspace.info_dialog = Some(dialog);
}

fn show_downloads(workspace: &mut EmbeddedBrowserWorkspace) {
    if let Some(dialog) = workspace.downloads_dialog.take() {
        dialog.close();
    }
    let parent = match workspace._window.gtk_window() {
        Ok(parent) => parent,
        Err(error) => {
            notify(
                &workspace.studio_webview,
                &format!("Unable to open downloads: {error}"),
            );
            return;
        }
    };
    let dialog = gtk::Dialog::new();
    dialog.set_title(browser_text(workspace, "Downloads"));
    dialog.set_transient_for(Some(&parent));
    dialog.set_destroy_with_parent(true);
    dialog.set_modal(false);
    dialog.set_resizable(true);
    dialog.set_default_size(560, 420);
    dialog.set_position(gtk::WindowPosition::CenterOnParent);
    dialog.style_context().add_class("browser-info-dialog");

    let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
    content.set_margin_start(16);
    content.set_margin_end(16);
    content.set_margin_top(14);
    content.set_margin_bottom(14);
    if workspace.downloads.is_empty() {
        let empty = gtk::Label::new(Some(browser_text(workspace, "No downloads yet")));
        empty.set_xalign(0.0);
        empty.style_context().add_class("dim-label");
        content.pack_start(&empty, false, false, 0);
    } else {
        let scroller = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
        scroller.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scroller.set_vexpand(true);
        let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        for (index, download) in workspace.downloads.iter().enumerate() {
            if index > 0 {
                list.pack_start(
                    &gtk::Separator::new(gtk::Orientation::Horizontal),
                    false,
                    false,
                    4,
                );
            }
            list.pack_start(
                &download_row(download, &workspace.translations),
                false,
                false,
                0,
            );
        }
        scroller.add(&list);
        content.pack_start(&scroller, true, true, 0);
    }
    dialog.content_area().add(&content);
    dialog.show_all();
    dialog.present();
    workspace.downloads_dialog = Some(dialog);
}

fn download_row(download: &BrowserDownload, translations: &HashMap<String, String>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.set_margin_top(6);
    row.set_margin_bottom(6);
    let copy = gtk::Box::new(gtk::Orientation::Vertical, 3);
    copy.set_hexpand(true);
    let name = gtk::Label::new(download.path.file_name().and_then(|value| value.to_str()));
    name.set_xalign(0.0);
    name.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    name.set_tooltip_text(Some(&download.url));
    let detail = gtk::Label::new(Some(&format!(
        "{}  ·  {}",
        translated_text(translations, download_state_label(download.state)),
        download.path.display()
    )));
    detail.set_xalign(0.0);
    detail.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    detail.style_context().add_class("dim-label");
    copy.pack_start(&name, false, false, 0);
    copy.pack_start(&detail, false, false, 0);
    row.pack_start(&copy, true, true, 0);
    let open = gtk::Button::with_label(translated_text(translations, "Open folder"));
    open.set_sensitive(download.state == BrowserDownloadState::Completed);
    let directory = download.path.parent().map(Path::to_path_buf);
    open.connect_clicked(move |_| {
        if let Some(directory) = directory.as_ref() {
            let _ = Command::new("xdg-open")
                .arg(directory)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
        }
    });
    row.pack_end(&open, false, false, 0);
    row
}

fn download_state_label(state: BrowserDownloadState) -> &'static str {
    match state {
        BrowserDownloadState::Downloading => "Downloading",
        BrowserDownloadState::Completed => "Completed",
        BrowserDownloadState::Failed => "Failed",
    }
}

fn finish_download(
    workspace: &mut EmbeddedBrowserWorkspace,
    url: &str,
    path: Option<&Path>,
    success: bool,
) {
    let index = path
        .and_then(|path| {
            workspace
                .downloads
                .iter()
                .position(|download| download.path == path)
        })
        .or_else(|| {
            workspace.downloads.iter().position(|download| {
                download.url == url && download.state == BrowserDownloadState::Downloading
            })
        });
    if let Some(index) = index {
        let record = &mut workspace.downloads[index];
        record.state = if success {
            BrowserDownloadState::Completed
        } else {
            BrowserDownloadState::Failed
        };
        let filename = record
            .path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("download");
        notify(
            &workspace.studio_webview,
            &format!(
                "{}：{filename}",
                if success {
                    "Download completed"
                } else {
                    "Download failed"
                }
            ),
        );
    } else if !success {
        notify(&workspace.studio_webview, "Download failed");
    }
    refresh_downloads_if_open(workspace);
}

fn refresh_downloads_if_open(workspace: &mut EmbeddedBrowserWorkspace) {
    let visible = workspace
        .downloads_dialog
        .as_ref()
        .is_some_and(gtk::prelude::WidgetExt::is_visible);
    if visible {
        show_downloads(workspace);
    }
}

fn confirm_clear_browser_data(
    parent: &gtk::Dialog,
    status: gtk::Label,
    translations: &HashMap<String, String>,
) {
    let confirmation = gtk::MessageDialog::new(
        Some(parent),
        gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
        gtk::MessageType::Warning,
        gtk::ButtonsType::None,
        translated_text(translations, "Clear all embedded browser data?"),
    );
    confirmation.set_secondary_text(Some(translated_text(
        translations,
        "This clears shared site data including cache, cookies, Local Storage, IndexedDB, and service workers. Website sign-ins may be lost. This action cannot be undone.",
    )));
    confirmation.add_button(
        translated_text(translations, "Cancel"),
        gtk::ResponseType::Cancel,
    );
    let clear = confirmation.add_button(
        translated_text(translations, "Clear"),
        gtk::ResponseType::Accept,
    );
    let cleanup_requested = translated_text(
        translations,
        "Cleanup requested. The current page may need to be reloaded, and website sign-ins may be lost.",
    )
    .to_owned();
    clear.style_context().add_class("browser-clear-data");
    confirmation.connect_response(move |confirmation, response| {
        if response == gtk::ResponseType::Accept {
            let result = WORKSPACE.with(|slot| {
                let slot = slot
                    .try_borrow()
                    .map_err(|_| "The browser is busy. Try again shortly.".to_owned())?;
                let workspace = slot
                    .as_ref()
                    .ok_or("The browser workspace is not initialized")?;
                let tab = active_tab(workspace).ok_or("No browser tab is available")?;
                tab.webview
                    .clear_all_browsing_data()
                    .map_err(|error| error.to_string())
            });
            status.set_text(match result {
                Ok(()) => &cleanup_requested,
                Err(ref error) => error,
            });
        }
        confirmation.close();
    });
    confirmation.show_all();
}

fn browser_zoom_row(workspace: &EmbeddedBrowserWorkspace) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 5);
    row.style_context().add_class("browser-zoom-row");
    let label = gtk::Label::new(Some(browser_text(workspace, "Zoom")));
    label.set_xalign(0.0);
    label.set_hexpand(true);
    let zoom = active_tab(workspace)
        .map(|tab| format!("{}%", (tab.zoom * 100.0).round() as u32))
        .unwrap_or_else(|| "100%".to_owned());
    let minus = browser_menu_square_button("−", browser_text(workspace, "Zoom out"), || {
        queue_action(BrowserAction::ZoomOut)
    });
    let reset = browser_menu_square_button(&zoom, browser_text(workspace, "Reset to 100%"), || {
        queue_action(BrowserAction::ZoomReset)
    });
    reset.style_context().add_class("browser-zoom-value");
    let plus = browser_menu_square_button("+", browser_text(workspace, "Zoom in"), || {
        queue_action(BrowserAction::ZoomIn)
    });
    row.pack_start(&label, true, true, 0);
    row.pack_end(&plus, false, false, 0);
    row.pack_end(&reset, false, false, 0);
    row.pack_end(&minus, false, false, 0);
    row
}

fn browser_menu_square_button(
    label: &str,
    tooltip: &str,
    action: impl Fn() + 'static,
) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    button.set_tooltip_text(Some(tooltip));
    button.set_relief(gtk::ReliefStyle::None);
    button.style_context().add_class("browser-menu-square");
    button.connect_clicked(move |_| action());
    button
}

fn install_browser_native_styles() {
    let Some(screen) = gtk::gdk::Screen::default() else {
        return;
    };
    let provider = gtk::CssProvider::new();
    if provider
        .load_from_data(
            br#"
            dialog.browser-info-dialog {
              background: #ffffff;
              color: #172033;
              font-family: Ubuntu, "Noto Sans SC", "Microsoft YaHei", sans-serif;
              font-size: 11px;
            }
            menu.browser-overflow-menu {
              padding: 6px;
              border: 1px solid #d7dde6;
              border-radius: 10px;
              background: #ffffff;
              color: #172033;
              font-family: Ubuntu, "Noto Sans SC", "Microsoft YaHei", sans-serif;
              font-size: 11px;
            }
            menu.browser-overflow-menu menuitem {
              min-height: 34px;
              padding: 0;
              border: 0;
              border-radius: 7px;
              background: transparent;
              color: #172033;
            }
            menu.browser-overflow-menu menuitem:hover { background: #f3f5f8; color: #176b5d; }
            menu.browser-overflow-menu separator { margin: 4px 0; background: #dfe4ec; }
            .browser-menu-shortcut { color: #7a8496; }
            .browser-zoom-row { min-height: 36px; padding: 3px 9px; color: #172033; }
            .browser-menu-square, .browser-info-copy {
              min-height: 28px;
              padding: 3px 8px;
              border: 1px solid #d7dde6;
              border-radius: 7px;
              background: #ffffff;
              color: #58647a;
            }
            .browser-menu-square:hover, .browser-info-copy:hover {
              background: #f3f5f8;
              color: #176b5d;
            }
            .browser-zoom-value { min-width: 50px; font-size: 10px; }
            .browser-menu-subheading { color: #172033; font-weight: 700; }
            .browser-info-copy { font-size: 10px; }
            .browser-clear-data {
              min-height: 30px;
              padding: 4px 10px;
              border: 1px solid #e2a9ae;
              border-radius: 7px;
              background: #fff7f7;
              color: #b4232f;
              font-weight: 600;
            }
            .browser-clear-data:hover { background: #fdebec; color: #9e1c27; }
            paned.studio-browser-split > separator {
              min-width: 9px;
              border-left: 4px solid #ffffff;
              border-right: 4px solid #ffffff;
              background: #d5dee2;
            }
            paned.studio-browser-split > separator:hover {
              border-left-width: 3px;
              border-right-width: 3px;
              background: #6aa398;
            }
            "#,
        )
        .is_ok()
    {
        gtk::StyleContext::add_provider_for_screen(
            &screen,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

fn browser_info_value_row(
    label: &str,
    value: &str,
    copy_label: &str,
    copied_label: &str,
) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Vertical, 3);
    let heading = gtk::Label::new(Some(label));
    heading.set_xalign(0.0);
    heading.style_context().add_class("dim-label");
    row.pack_start(&heading, false, false, 0);

    let value_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let path = gtk::Label::new(Some(value));
    path.set_xalign(0.0);
    path.set_selectable(true);
    path.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    path.set_tooltip_text(Some(value));
    path.set_hexpand(true);
    let copy = gtk::Button::with_label(copy_label);
    copy.style_context().add_class("browser-info-copy");
    connect_copy_button(
        &copy,
        value.to_owned(),
        copy_label.to_owned(),
        copied_label.to_owned(),
    );
    value_row.pack_start(&path, true, true, 0);
    value_row.pack_end(&copy, false, false, 0);
    row.pack_start(&value_row, false, false, 0);
    row
}

fn connect_copy_button(
    button: &gtk::Button,
    value: String,
    copy_label: String,
    copied_label: String,
) {
    button.connect_clicked(move |button| {
        if copy_to_clipboard(&value) {
            button.set_label(&copied_label);
            let button = button.clone();
            let copy_label = copy_label.clone();
            gtk::glib::timeout_add_local_once(Duration::from_millis(900), move || {
                button.set_label(&copy_label);
            });
        }
    });
}

fn copy_to_clipboard(value: &str) -> bool {
    let Some(display) = gtk::gdk::Display::default() else {
        return false;
    };
    let Some(clipboard) = gtk::Clipboard::default(&display) else {
        return false;
    };
    clipboard.set_text(value);
    true
}

fn toggle_workspace() {
    let handles = WORKSPACE.with(|slot| {
        let Ok(mut slot) = slot.try_borrow_mut() else {
            return None;
        };
        let workspace = slot.as_mut()?;
        if workspace.visible {
            workspace.visible = false;
            if let Some(menu) = workspace.overflow_menu.take() {
                menu.popdown();
            }
            if let Some(dialog) = workspace.info_dialog.take() {
                dialog.close();
            }
        } else {
            if let Err(error) = ensure_browser_runtime(workspace) {
                notify(
                    &workspace.studio_webview,
                    &format!("Unable to initialize embedded browser: {error}"),
                );
                return None;
            }
            workspace.visible = true;
        }
        Some((
            workspace.visible,
            workspace.browser_width,
            workspace.browser_column.clone(),
            workspace.split.clone(),
        ))
    });
    let Some((visible, browser_width, browser_column, split)) = handles else {
        return;
    };

    // set_position synchronously emits position-notify. Do not hold the WORKSPACE RefCell borrow
    // across these GTK calls or the callback would panic across FFI and abort the process.
    if visible {
        browser_column.show_all();
        position_browser_split(&split, browser_width);
        // During startup GTK may not have allocated the Paned yet. Reapply after the first
        // layout pass so an early developer/browser action cannot expand the browser to 100%.
        let split_after_layout = split.clone();
        gtk::glib::timeout_add_local_once(Duration::from_millis(50), move || {
            position_browser_split(&split_after_layout, browser_width);
        });
    } else {
        browser_column.hide();
    }

    WORKSPACE.with(|slot| {
        let Ok(slot) = slot.try_borrow() else {
            return;
        };
        let Some(workspace) = slot.as_ref() else {
            return;
        };
        if visible {
            if let Some(tab) = active_tab(workspace) {
                let _ = tab.webview.focus();
            }
        }
        evaluate(
            &workspace.studio_webview,
            "window.__studioEmbeddedBrowser?.setVisible",
            &visible,
        );
        if visible {
            evaluate(
                &workspace.studio_webview,
                "window.__studioEmbeddedBrowser?.setRuntimeLoaded",
                &true,
            );
        }
    });
    if visible {
        measure_active_fit();
    }
    sync_toolbar();
}

fn set_browser_width(width: Option<i32>) {
    let Some(width) = width.filter(|value| (MIN_BROWSER_WIDTH..=2400).contains(value)) else {
        return;
    };
    WORKSPACE.with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut() {
            if let Some(workspace) = slot.as_mut() {
                workspace.browser_width = width;
            }
        }
    });
}

fn resize_browser(width: i32) {
    let handles = WORKSPACE.with(|slot| {
        let Ok(mut slot) = slot.try_borrow_mut() else {
            return None;
        };
        let workspace = slot.as_mut().filter(|workspace| workspace.visible)?;
        if !(MIN_BROWSER_WIDTH..=2400).contains(&width) {
            return None;
        }
        workspace.browser_width = width;
        Some((workspace.split.clone(), width))
    });
    if let Some((split, width)) = handles {
        position_browser_split(&split, width);
    }
}

fn show_workspace() {
    let visible = WORKSPACE.with(|slot| {
        slot.try_borrow()
            .ok()
            .and_then(|slot| slot.as_ref().map(|workspace| workspace.visible))
            .unwrap_or(false)
    });
    if !visible {
        toggle_workspace();
    }
}

fn hide_workspace() {
    let visible = WORKSPACE.with(|slot| {
        slot.try_borrow()
            .ok()
            .and_then(|slot| slot.as_ref().map(|workspace| workspace.visible))
            .unwrap_or(false)
    });
    if visible {
        toggle_workspace();
    }
}

fn exit_workspace() {
    let released = WORKSPACE.with(|slot| {
        let Ok(mut slot) = slot.try_borrow_mut() else {
            return None;
        };
        let workspace = slot.as_mut()?;
        workspace.visible = false;
        workspace.active_tab_id = 0;
        workspace.next_tab_id = 1;
        workspace.recent_recoveries.clear();
        workspace.toolbar_recovery = None;
        Some((
            workspace.overflow_menu.take(),
            workspace.info_dialog.take(),
            workspace.downloads_dialog.take(),
            std::mem::take(&mut workspace.tabs),
            workspace.toolbar_webview.take(),
            workspace.browser_column.clone(),
            workspace.browser_stack.clone(),
            workspace.toolbar_host.clone(),
        ))
    });
    let Some((
        menu,
        dialog,
        downloads_dialog,
        tabs,
        toolbar_webview,
        browser_column,
        browser_stack,
        toolbar_host,
    )) = released
    else {
        return;
    };
    if let Some(menu) = menu {
        menu.popdown();
    }
    if let Some(dialog) = dialog {
        dialog.close();
    }
    if let Some(dialog) = downloads_dialog {
        dialog.close();
    }
    browser_column.hide();
    drop(tabs);
    drop(toolbar_webview);
    for child in browser_stack.children() {
        browser_stack.remove(&child);
    }
    for child in toolbar_host.children() {
        toolbar_host.remove(&child);
    }
    WORKSPACE.with(|slot| {
        let Ok(slot) = slot.try_borrow() else {
            return;
        };
        let Some(workspace) = slot.as_ref() else {
            return;
        };
        evaluate(
            &workspace.studio_webview,
            "window.__studioEmbeddedBrowser?.setVisible",
            &false,
        );
        evaluate(
            &workspace.studio_webview,
            "window.__studioEmbeddedBrowser?.setRuntimeLoaded",
            &false,
        );
    });
}

fn position_browser_split(split: &gtk::Paned, browser_width: i32) {
    let available = split.allocation().width();
    if available < MIN_STUDIO_WIDTH + MIN_BROWSER_WIDTH {
        return;
    }
    let max_width = (available - MIN_STUDIO_WIDTH).max(MIN_BROWSER_WIDTH);
    let width = browser_width.clamp(MIN_BROWSER_WIDTH, max_width);
    split.set_position(available - width);
}

fn update_tab_navigation(id: u64, url: Option<String>, title: Option<String>, finished: bool) {
    gtk::glib::MainContext::default().invoke(move || {
        let should_measure = WORKSPACE.with(|slot| {
            let Ok(mut slot) = slot.try_borrow_mut() else {
                return false;
            };
            let Some(workspace) = slot.as_mut() else {
                return false;
            };
            let Some(tab) = workspace.tabs.iter_mut().find(|tab| tab.id == id) else {
                return false;
            };
            if let Some(url) = url {
                if tab.url != url {
                    tab.title = display_title("", &url);
                    tab.url = url;
                }
            }
            if let Some(title) = title.filter(|title| !title.trim().is_empty()) {
                tab.title = title;
            } else if tab.title.trim().is_empty() {
                tab.title = display_title("", &tab.url);
            }
            finished && tab.fit_width
        });
        sync_toolbar();
        if should_measure {
            schedule_fit_measure(id, 80);
        }
    });
}

fn schedule_fit_measure(id: u64, delay_ms: u64) {
    gtk::glib::timeout_add_local_once(Duration::from_millis(delay_ms), move || measure_tab_fit(id));
}

fn measure_active_fit() {
    let id = WORKSPACE.with(|slot| {
        slot.try_borrow()
            .ok()
            .and_then(|slot| slot.as_ref().map(|workspace| workspace.active_tab_id))
    });
    if let Some(id) = id {
        measure_tab_fit(id);
    }
}

fn measure_tab_fit(id: u64) {
    let should_measure = WORKSPACE.with(|slot| {
        let Ok(mut slot) = slot.try_borrow_mut() else {
            return false;
        };
        let Some(workspace) = slot.as_mut() else {
            return false;
        };
        // A hidden GtkStack can be 1x1 while WebKit still reports its previous viewport.
        // Measuring then leaves the backing surface with stale geometry when the pane opens.
        if !workspace.visible {
            return false;
        }
        let Some(tab) = workspace
            .tabs
            .iter_mut()
            .find(|tab| tab.id == id && tab.fit_width)
        else {
            return false;
        };
        tab.zoom = 1.0;
        let _ = tab.webview.zoom(1.0);
        true
    });
    if !should_measure {
        return;
    }

    gtk::glib::timeout_add_local_once(Duration::from_millis(45), move || {
        WORKSPACE.with(|slot| {
            let Ok(slot) = slot.try_borrow() else {
                return;
            };
            let Some(workspace) = slot.as_ref() else {
                return;
            };
            if !workspace.visible {
                return;
            }
            let Some(tab) = workspace
                .tabs
                .iter()
                .find(|tab| tab.id == id && tab.fit_width)
            else {
                return;
            };
            let _ = tab
                .webview
                .evaluate_script_with_callback(READ_VIEWPORT_SCRIPT, move |value| {
                    gtk::glib::MainContext::default().invoke(move || apply_fit_metrics(id, &value));
                });
        });
    });
}

fn apply_fit_metrics(id: u64, raw: &str) {
    let Ok(metrics) = serde_json::from_str::<ViewportMetrics>(raw) else {
        return;
    };
    let Some(target) = fit_zoom(metrics.inner_width, metrics.scroll_width) else {
        return;
    };
    WORKSPACE.with(|slot| {
        let Ok(mut slot) = slot.try_borrow_mut() else {
            return;
        };
        let Some(workspace) = slot.as_mut() else {
            return;
        };
        // The pane may have been hidden while the asynchronous JS measurement was in flight.
        if !workspace.visible {
            return;
        }
        let Some(tab) = workspace
            .tabs
            .iter_mut()
            .find(|tab| tab.id == id && tab.fit_width)
        else {
            return;
        };
        tab.zoom = round_zoom(target);
        let _ = tab.webview.zoom(tab.zoom);
        eprintln!(
            "Embedded Browser tab {id}: viewport {:.0}px, content {:.0}px, zoom {:.0}%",
            metrics.inner_width,
            metrics.scroll_width,
            tab.zoom * 100.0
        );
    });
    sync_toolbar();
}

fn fit_zoom(inner_width: f64, scroll_width: f64) -> Option<f64> {
    if !inner_width.is_finite()
        || !scroll_width.is_finite()
        || inner_width <= 0.0
        || scroll_width <= 0.0
    {
        return None;
    }
    let overflow_ratio = inner_width / scroll_width;
    Some(if overflow_ratio < 0.98 {
        overflow_ratio.clamp(MIN_ZOOM, 1.0)
    } else {
        1.0
    })
}

fn sync_toolbar() {
    gtk::glib::MainContext::default().invoke(|| {
        WORKSPACE.with(|slot| {
            let Ok(slot) = slot.try_borrow() else {
                return;
            };
            let Some(workspace) = slot.as_ref() else {
                return;
            };
            let Some(toolbar_webview) = workspace.toolbar_webview.as_ref() else {
                return;
            };
            let tabs = workspace
                .tabs
                .iter()
                .map(|tab| {
                    serde_json::json!({
                        "id": tab.id,
                        "title": display_title(&tab.title, &tab.url),
                        "url": tab.url,
                        "active": tab.id == workspace.active_tab_id,
                    })
                })
                .collect::<Vec<_>>();
            let active = active_tab(workspace);
            let payload = serde_json::json!({
                "tabs": tabs,
                "url": active.map(|tab| tab.url.as_str()).unwrap_or(DEFAULT_URL),
                "title": active.map(|tab| display_title(&tab.title, &tab.url)).unwrap_or_else(|| "Browser".to_owned()),
                "zoomPercent": active.map(|tab| (tab.zoom * 100.0).round() as u32).unwrap_or(100),
                "fitWidth": active.is_some_and(|tab| tab.fit_width),
                "translations": workspace.translations,
            });
            evaluate(
                toolbar_webview,
                "window.__embeddedBrowserToolbar?.setState",
                &payload,
            );
        });
    });
}

fn active_tab(workspace: &EmbeddedBrowserWorkspace) -> Option<&BrowserTab> {
    workspace
        .tabs
        .iter()
        .find(|tab| tab.id == workspace.active_tab_id)
}

fn active_tab_mut(workspace: &mut EmbeddedBrowserWorkspace) -> Option<&mut BrowserTab> {
    workspace
        .tabs
        .iter_mut()
        .find(|tab| tab.id == workspace.active_tab_id)
}

fn display_title(title: &str, url: &str) -> String {
    let title = title.trim();
    if !title.is_empty() {
        return title.to_owned();
    }
    Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .filter(|host| !host.is_empty())
        .unwrap_or_else(|| "New Tab".to_owned())
}

fn round_zoom(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn browser_page_height(column_height: i32) -> i32 {
    (column_height - TOOLBAR_HEIGHT).max(1)
}

fn open_comment(selection: Option<BrowserSelection>) {
    WORKSPACE.with(|slot| {
        let Ok(slot) = slot.try_borrow() else {
            return;
        };
        let Some(workspace) = slot.as_ref() else {
            return;
        };
        if let Some(selection) = selection {
            evaluate(
                &workspace.studio_webview,
                "window.__studioEmbeddedBrowser?.openComment",
                &selection,
            );
        } else {
            notify(
                &workspace.studio_webview,
                "Select text on the web page first",
            );
        }
    });
}

fn notify(webview: &WebView, message: &str) {
    evaluate(webview, "window.__studioEmbeddedBrowser?.notify", &message);
}

fn browser_text<'a>(workspace: &'a EmbeddedBrowserWorkspace, source: &'static str) -> &'a str {
    translated_text(&workspace.translations, source)
}

fn translated_text<'a>(translations: &'a HashMap<String, String>, source: &'static str) -> &'a str {
    translations
        .get(source)
        .map(String::as_str)
        .unwrap_or(source)
}

fn evaluate<T: Serialize>(webview: &WebView, function: &str, payload: &T) {
    if let Ok(payload) = serde_json::to_string(payload) {
        let _ = webview.evaluate_script(&format!("{function}?.({payload})"));
    }
}

fn parse_action(raw: &str) -> Option<BrowserAction> {
    let url = Url::parse(raw).ok()?;
    if url.scheme() != "studio-action" {
        return None;
    }
    match url.host_str()? {
        "toggle-browser" => Some(BrowserAction::Toggle),
        "show-browser" => Some(BrowserAction::Show(query_i32(&url, "width"))),
        "hide-browser" => Some(BrowserAction::Hide),
        "open-browser" => query_value(&url, "url")
            .map(|target| BrowserAction::Open(target, query_i32(&url, "width"))),
        "resize-browser" => query_i32(&url, "width").map(BrowserAction::Resize),
        "set-browser-translations" => query_value(&url, "messages")
            .and_then(|value| serde_json::from_str(&value).ok())
            .map(BrowserAction::SetTranslations),
        "navigate" => query_value(&url, "url").map(BrowserAction::Navigate),
        "new-tab" => Some(BrowserAction::NewTab(query_value(&url, "url"))),
        "activate-tab" => query_u64(&url, "id").map(BrowserAction::ActivateTab),
        "close-tab" => query_u64(&url, "id").map(BrowserAction::CloseTab),
        "next-tab" => Some(BrowserAction::NextTab),
        "previous-tab" => Some(BrowserAction::PreviousTab),
        "back" => Some(BrowserAction::Back),
        "forward" => Some(BrowserAction::Forward),
        "reload" => Some(BrowserAction::Reload),
        "zoom-in" => Some(BrowserAction::ZoomIn),
        "zoom-out" => Some(BrowserAction::ZoomOut),
        "zoom-reset" => Some(BrowserAction::ZoomReset),
        "fit-width" => Some(BrowserAction::FitWidth),
        "comment-selection" => Some(BrowserAction::CommentSelection),
        "browser-menu" => Some(BrowserAction::ShowMenu),
        _ => None,
    }
}

fn query_value(url: &Url, name: &str) -> Option<String> {
    url.query_pairs()
        .find_map(|(key, value)| (key == name).then(|| value.into_owned()))
}

fn query_i32(url: &Url, name: &str) -> Option<i32> {
    query_value(url, name)?.parse().ok()
}

fn query_u64(url: &Url, name: &str) -> Option<u64> {
    query_value(url, name)?.parse().ok()
}

fn parse_selection(raw: &str) -> Option<BrowserSelection> {
    if raw.len() > MAX_SELECTION_BYTES * 2 {
        return None;
    }
    let mut value: BrowserSelection = serde_json::from_str(raw).ok()?;
    value.text = bounded(value.text.trim(), MAX_SELECTION_BYTES);
    value.url = bounded(&value.url, 8 * 1024);
    value.title = bounded(&value.title, 512);
    if value.text.is_empty()
        || Url::parse(&value.url).ok().is_none_or(|url| {
            !matches!(url.scheme(), "http" | "https")
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
        })
    {
        return None;
    }
    Some(value)
}

fn bounded(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_owned()
}

fn preferred_browser_width(preferences: &BrowserPreferences) -> i32 {
    preferences.embedded_width.clamp(480, 1200) as i32
}

fn download_destination_for_name(filename: &str) -> Result<PathBuf, String> {
    let directory = download_directory();
    fs::create_dir_all(&directory).map_err(|error| {
        format!(
            "Unable to create downloads directory {}: {error}",
            directory.display()
        )
    })?;
    Ok(unique_download_path(
        &directory,
        &sanitize_download_filename(filename),
    ))
}

fn download_directory() -> PathBuf {
    if let Some(path) = env::var_os("CODEX_THREAD_STUDIO_DOWNLOAD_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
    {
        return path;
    }
    if let Some(path) = env::var_os("XDG_DOWNLOAD_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
    {
        return path;
    }
    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home).join("Downloads");
    }
    env::temp_dir().join("codex-thread-studio-downloads")
}

fn download_filename(raw_url: &str) -> String {
    let candidate = Url::parse(raw_url)
        .ok()
        .and_then(|url| {
            url.path_segments()
                .and_then(|mut segments| segments.rfind(|segment| !segment.is_empty()))
                .map(percent_decode_filename)
        })
        .unwrap_or_else(|| "download".to_owned());
    sanitize_download_filename(&candidate)
}

fn percent_decode_filename(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let high = (bytes[index + 1] as char).to_digit(16);
            let low = (bytes[index + 2] as char).to_digit(16);
            if let (Some(high), Some(low)) = (high, low) {
                decoded.push(((high << 4) | low) as u8);
                index += 3;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    String::from_utf8(decoded).unwrap_or_else(|_| value.to_owned())
}

fn sanitize_download_filename(value: &str) -> String {
    let mut sanitized = String::with_capacity(value.len().min(180));
    for character in value.chars().take(180) {
        if character.is_control()
            || matches!(
                character,
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
            )
        {
            sanitized.push('_');
        } else {
            sanitized.push(character);
        }
    }
    let sanitized =
        sanitized.trim_matches(|character: char| character == '.' || character.is_whitespace());
    if sanitized.is_empty() {
        "download".to_owned()
    } else {
        sanitized.to_owned()
    }
}

fn unique_download_path(directory: &Path, filename: &str) -> PathBuf {
    let initial = directory.join(filename);
    if !initial.exists() {
        return initial;
    }
    let path = Path::new(filename);
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("download");
    let extension = path.extension().and_then(|value| value.to_str());
    for suffix in 1..=10_000 {
        let name = match extension {
            Some(extension) => format!("{stem} ({suffix}).{extension}"),
            None => format!("{stem} ({suffix})"),
        };
        let candidate = directory.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    directory.join(format!("download-{}", std::process::id()))
}

fn profile_directory(name: &str) -> PathBuf {
    if let Some(path) = env::var_os("XDG_DATA_HOME").filter(|value| !value.is_empty()) {
        return PathBuf::from(path).join("codex-thread-studio").join(name);
    }
    if let Some(home) = env::var_os("HOME").filter(|value| !value.is_empty()) {
        return PathBuf::from(home)
            .join(".local/share/codex-thread-studio")
            .join(name);
    }
    env::temp_dir().join("codex-thread-studio").join(name)
}

fn browser_cache_directory() -> PathBuf {
    if let Some(path) = env::var_os("XDG_CACHE_HOME").filter(|value| !value.is_empty()) {
        return PathBuf::from(path).join("codex-thread-studio/WebKitCache");
    }
    if let Some(home) = env::var_os("HOME").filter(|value| !value.is_empty()) {
        return PathBuf::from(home).join(".cache/codex-thread-studio/WebKitCache");
    }
    env::temp_dir().join("codex-thread-studio/WebKitCache")
}

const READ_SELECTION_SCRIPT: &str = r#"
(() => ({
  text: (window.getSelection()?.toString() || '').trim().slice(0, 16384),
  url: location.href,
  title: document.title || ''
}))()
"#;

const READ_VIEWPORT_SCRIPT: &str = r#"
(() => ({
  innerWidth: window.innerWidth,
  scrollWidth: Math.max(document.documentElement?.scrollWidth || 0, document.body?.scrollWidth || 0)
}))()
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_known_actions() {
        assert!(matches!(
            parse_action("studio-action://toggle-browser"),
            Some(BrowserAction::Toggle)
        ));
        assert!(matches!(
            parse_action("studio-action://show-browser"),
            Some(BrowserAction::Show(None))
        ));
        assert!(matches!(
            parse_action("studio-action://show-browser?width=712"),
            Some(BrowserAction::Show(Some(712)))
        ));
        assert!(matches!(
            parse_action("studio-action://resize-browser?width=680"),
            Some(BrowserAction::Resize(680))
        ));
        assert!(matches!(
            parse_action("studio-action://hide-browser"),
            Some(BrowserAction::Hide)
        ));
        assert!(matches!(
            parse_action("studio-action://open-browser?url=https%3A%2F%2Fexample.com"),
            Some(BrowserAction::Open(url, None)) if url == "https://example.com"
        ));
        assert!(matches!(
            parse_action("studio-action://navigate?url=https%3A%2F%2Fexample.com"),
            Some(BrowserAction::Navigate(url)) if url == "https://example.com"
        ));
        assert!(matches!(
            parse_action("studio-action://activate-tab?id=42"),
            Some(BrowserAction::ActivateTab(42))
        ));
        assert!(matches!(
            parse_action("studio-action://new-tab"),
            Some(BrowserAction::NewTab(None))
        ));
        assert!(matches!(
            parse_action("studio-action://browser-menu"),
            Some(BrowserAction::ShowMenu)
        ));
        assert!(parse_action("https://example.com").is_none());
        assert!(parse_action("studio-action://run-shell").is_none());
    }

    #[test]
    fn selection_bridge_is_bounded_and_http_only() {
        assert!(parse_selection(
            r#"{"text":"selected","url":"https://example.com","title":"Example"}"#
        )
        .is_some());
        assert!(
            parse_selection(r#"{"text":"secret","url":"file:///etc/passwd","title":"Local"}"#)
                .is_none()
        );
    }

    #[test]
    fn display_title_prefers_title_then_host() {
        assert_eq!(
            display_title(" Example ", "https://ignored.test"),
            "Example"
        );
        assert_eq!(display_title("", "https://example.com/path"), "example.com");
    }

    #[test]
    fn fit_zoom_uses_native_zoom_for_fixed_width_pages() {
        assert_eq!(fit_zoom(580.0, 1000.0), Some(0.58));
        assert_eq!(fit_zoom(720.0, 720.0), Some(1.0));
        assert_eq!(fit_zoom(320.0, 1000.0), Some(MIN_ZOOM));
        assert_eq!(fit_zoom(0.0, 1000.0), None);
    }

    #[test]
    fn browser_page_always_uses_height_below_the_toolbar() {
        assert_eq!(browser_page_height(853), 853 - TOOLBAR_HEIGHT);
        assert_eq!(browser_page_height(TOOLBAR_HEIGHT), 1);
        assert_eq!(browser_page_height(1), 1);
    }

    #[test]
    fn lazy_browser_runtime_is_ready_only_when_every_component_exists() {
        assert!(!browser_runtime_parts_loaded(false, false, false, 0));
        assert!(!browser_runtime_parts_loaded(true, true, true, 0));
        assert!(!browser_runtime_parts_loaded(true, false, true, 1));
        assert!(!browser_runtime_parts_loaded(false, true, true, 1));
        assert!(browser_runtime_parts_loaded(true, true, true, 1));
    }

    #[test]
    fn download_names_are_decoded_sanitized_and_never_traverse_directories() {
        assert_eq!(
            download_filename("https://example.com/files/Studio%20Guide.pdf?token=1"),
            "Studio Guide.pdf"
        );
        assert_eq!(
            sanitize_download_filename("../../unsafe\\name?.txt"),
            "_.._unsafe_name_.txt"
        );
        assert_eq!(download_filename("https://example.com/"), "download");
    }

    #[test]
    fn existing_downloads_receive_a_collision_suffix() {
        let directory = env::temp_dir().join(format!(
            "codex-thread-studio-download-test-{}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("test download directory");
        fs::write(directory.join("report.pdf"), b"existing").expect("existing download");
        assert_eq!(
            unique_download_path(&directory, "report.pdf"),
            directory.join("report (1).pdf")
        );
        fs::remove_dir_all(directory).expect("remove test download directory");
    }

    #[test]
    fn tls_error_page_escapes_untrusted_error_details() {
        let html = tls_error_html(
            "https://example.com/?value=<script>alert(1)</script>",
            "<img src=x onerror=alert(2)>",
        );

        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(html.contains("&lt;img src=x onerror=alert(2)&gt;"));
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(!html.contains("<img src=x onerror=alert(2)>"));
    }
}
