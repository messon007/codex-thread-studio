#![cfg(target_os = "linux")]

use std::{cell::RefCell, env, fs, path::PathBuf, time::Duration};

use gtk::prelude::*;
use serde::{Deserialize, Serialize};
use url::Url;
use wry::{
    PageLoadEvent, WebContext, WebView, WebViewBuilder, WebViewBuilderExtUnix, WebViewExtUnix,
};

use crate::browser_runtime::{validate_browser_url, BrowserPreferences};

const DEFAULT_URL: &str = "https://example.com";
const MIN_STUDIO_WIDTH: i32 = 520;
const MIN_BROWSER_WIDTH: i32 = 480;
const TOOLBAR_HEIGHT: i32 = 82;
const MAX_SELECTION_BYTES: usize = 16 * 1024;
const MIN_ZOOM: f64 = 0.5;
const MAX_ZOOM: f64 = 2.0;
const ZOOM_STEP: f64 = 0.1;
const TOOLBAR_HTML: &str = include_str!("../../ui/embedded-browser.html");

thread_local! {
    static WORKSPACE: RefCell<Option<EmbeddedBrowserWorkspace>> = const { RefCell::new(None) };
}

#[derive(Debug)]
enum BrowserAction {
    Toggle,
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

struct BrowserTab {
    id: u64,
    title: String,
    url: String,
    host: gtk::Box,
    webview: WebView,
    zoom: f64,
    fit_width: bool,
}

struct EmbeddedBrowserWorkspace {
    _window: tauri::Window,
    split: gtk::Paned,
    browser_column: gtk::Fixed,
    browser_stack: gtk::Stack,
    studio_webview: WebView,
    toolbar_webview: WebView,
    tabs: Vec<BrowserTab>,
    active_tab_id: u64,
    next_tab_id: u64,
    resize_generation: u64,
    preferences: BrowserPreferences,
    visible: bool,
    browser_width: i32,
    _studio_context: WebContext,
    _toolbar_context: WebContext,
    browser_context: WebContext,
    browser_profile_directory: PathBuf,
    browser_cache_directory: PathBuf,
    webkit_version: String,
    overflow_menu: Option<gtk::Menu>,
    info_dialog: Option<gtk::Dialog>,
}

pub fn is_supported() -> bool {
    env::var_os("WAYLAND_DISPLAY")
        .filter(|value| !value.is_empty())
        .is_some()
}

#[cfg(debug_assertions)]
pub fn capture_screenshot(path: &std::path::Path) -> Result<(), String> {
    WORKSPACE.with(|slot| {
        let slot = slot
            .try_borrow()
            .map_err(|_| "Studio window is busy".to_owned())?;
        let workspace = slot.as_ref().ok_or("Studio window is not initialized")?;
        let window = workspace
            ._window
            .gtk_window()
            .map_err(|error| error.to_string())?;
        if !window.is_mapped() || !window.is_visible() {
            return Err("Studio window is not visible".to_owned());
        }
        let allocation = window.allocation();
        if allocation.width() <= 0 || allocation.height() <= 0 {
            return Err("Studio window has no drawable area".to_owned());
        }
        let surface = window
            .window()
            .ok_or("Studio window surface is unavailable")?;
        use gtk::gdk::prelude::WindowExtManual;
        let pixbuf = surface
            .pixbuf(0, 0, allocation.width(), allocation.height())
            .ok_or("Studio window surface could not be captured")?;
        if let Some(dialog) = workspace
            .info_dialog
            .as_ref()
            .filter(|dialog| dialog.is_visible())
        {
            composite_widget_surface(dialog, &surface, &pixbuf);
        }
        if let Some(menu) = workspace
            .overflow_menu
            .as_ref()
            .filter(|menu| menu.is_visible())
        {
            composite_widget_surface(menu, &surface, &pixbuf);
        }
        pixbuf
            .savev(path, "png", &[])
            .map_err(|error| error.to_string())?;
        fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(0o600))
            .map_err(|error| error.to_string())?;
        Ok(())
    })
}

#[cfg(debug_assertions)]
fn composite_widget_surface<W: IsA<gtk::Widget>>(
    widget: &W,
    main_surface: &gtk::gdk::Window,
    target: &gtk::gdk_pixbuf::Pixbuf,
) {
    use gtk::gdk::prelude::WindowExtManual;
    let Some(widget_surface) = widget.window() else {
        return;
    };
    let allocation = widget.allocation();
    let Some(source) = widget_surface.pixbuf(0, 0, allocation.width(), allocation.height()) else {
        return;
    };
    let (main_valid, main_x, main_y) = main_surface.origin();
    let (widget_valid, widget_x, widget_y) = widget_surface.origin();
    if main_valid == 0 || widget_valid == 0 {
        return;
    }
    let destination_x = widget_x - main_x;
    let destination_y = widget_y - main_y;
    let destination_width = source.width().min(target.width() - destination_x.max(0));
    let destination_height = source.height().min(target.height() - destination_y.max(0));
    if destination_x < 0 || destination_y < 0 || destination_width <= 0 || destination_height <= 0 {
        return;
    }
    source.composite(
        target,
        destination_x,
        destination_y,
        destination_width,
        destination_height,
        destination_x as f64,
        destination_y as f64,
        1.0,
        1.0,
        gtk::gdk_pixbuf::InterpType::Bilinear,
        255,
    );
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
    if let Some(url) = url {
        dispatch_action(BrowserAction::Navigate(url));
    }
    Ok(())
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
    let window = tauri::window::WindowBuilder::new(app, "main")
        .title("Codex Thread Studio")
        .inner_size(1400.0, 900.0)
        .min_inner_size(1040.0, 660.0)
        .build()?;
    let root = window.default_vbox()?;

    let split = gtk::Paned::new(gtk::Orientation::Horizontal);
    split.set_wide_handle(true);

    let studio_host = gtk::Box::new(gtk::Orientation::Vertical, 0);
    studio_host.set_size_request(MIN_STUDIO_WIDTH, -1);
    // WebKitGTK reports a large natural height for a WebView. A GtkBox uses that
    // natural height even when the toolbar host has an 82px size request, which
    // can leave the page stack with only half of the available height. GtkFixed
    // lets this split pane own the geometry explicitly: toolbar=82px, page=rest.
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
    let toolbar_profile = profile_directory("browser-toolbar");
    let browser_profile = profile_directory("embedded-browser");
    fs::create_dir_all(&studio_profile)?;
    fs::create_dir_all(&toolbar_profile)?;
    fs::create_dir_all(&browser_profile)?;
    let mut studio_context = WebContext::new(Some(studio_profile));
    let mut toolbar_context = WebContext::new(Some(toolbar_profile));
    let mut browser_context = WebContext::new(Some(browser_profile.clone()));
    let browser_cache = browser_cache_directory();
    let webkit_version = wry::webview_version().unwrap_or_else(|_| "Unknown".to_owned());

    let studio_webview = WebViewBuilder::with_web_context(&mut studio_context)
        .with_url(studio_url.as_str())
        .with_initialization_script(initialization_script)
        .with_clipboard(true)
        .with_navigation_handler(|url| intercept_action(&url))
        .build_gtk(&studio_host)?;

    let toolbar_webview = WebViewBuilder::with_web_context(&mut toolbar_context)
        .with_html(TOOLBAR_HTML)
        .with_clipboard(true)
        .with_navigation_handler(|url| intercept_action(&url))
        .build_gtk(&toolbar_host)?;
    let toolbar_widget = toolbar_webview.webview();
    toolbar_widget.set_size_request(1, TOOLBAR_HEIGHT);
    toolbar_host.connect_size_allocate(move |host, allocation| {
        host.move_(&toolbar_widget, 0, 0);
        toolbar_widget.size_allocate(&gtk::Allocation::new(
            0,
            0,
            allocation.width().max(1),
            TOOLBAR_HEIGHT,
        ));
    });

    let initial_tab = build_browser_tab(&mut browser_context, &browser_stack, 1, DEFAULT_URL)?;
    browser_stack.set_visible_child(&initial_tab.host);

    split.show_all();
    browser_column.hide();
    let split_signal = split.clone();
    let browser_width = preferred_browser_width(&preferences);
    WORKSPACE.with(|slot| {
        *slot.borrow_mut() = Some(EmbeddedBrowserWorkspace {
            _window: window,
            split,
            browser_column,
            browser_stack,
            studio_webview,
            toolbar_webview,
            tabs: vec![initial_tab],
            active_tab_id: 1,
            next_tab_id: 2,
            resize_generation: 0,
            preferences,
            visible: false,
            browser_width,
            _studio_context: studio_context,
            _toolbar_context: toolbar_context,
            browser_context,
            browser_profile_directory: browser_profile,
            browser_cache_directory: browser_cache,
            webkit_version,
            overflow_menu: None,
            info_dialog: None,
        });
    });
    split_signal.connect_position_notify(|split| {
        let generation = WORKSPACE.with(|slot| {
            let Ok(mut slot) = slot.try_borrow_mut() else {
                return None;
            };
            let workspace = slot.as_mut().filter(|workspace| workspace.visible)?;
            let width = (split.allocation().width() - split.position()).max(MIN_BROWSER_WIDTH);
            workspace.browser_width = width;
            workspace.resize_generation = workspace.resize_generation.wrapping_add(1);
            evaluate(
                &workspace.studio_webview,
                "window.__studioEmbeddedBrowser?.setWidth",
                &width,
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

fn build_browser_tab(
    context: &mut WebContext,
    stack: &gtk::Stack,
    id: u64,
    url: &str,
) -> Result<BrowserTab, wry::Error> {
    let host = gtk::Box::new(gtk::Orientation::Vertical, 0);
    host.set_hexpand(true);
    host.set_vexpand(true);
    stack.add_named(&host, &format!("tab-{id}"));

    let webview = WebViewBuilder::with_web_context(context)
        .with_url(url)
        .with_clipboard(true)
        .with_document_title_changed_handler(move |title| {
            update_tab_navigation(id, None, Some(title), false)
        })
        .with_on_page_load_handler(move |event, url| {
            let finished = matches!(event, PageLoadEvent::Finished);
            update_tab_navigation(id, Some(url), None, finished);
        })
        .with_new_window_req_handler(move |url| {
            queue_action(BrowserAction::NewTab(Some(url)));
            false
        })
        .build_gtk(&host)?;
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
    })
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
    if matches!(action, BrowserAction::Toggle) {
        toggle_workspace();
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
            BrowserAction::Toggle => unreachable!("toggle is handled without a nested borrow"),
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
                        match build_browser_tab(
                            &mut workspace.browser_context,
                            &workspace.browser_stack,
                            id,
                            url.as_str(),
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
                if let Some(tab) = active_tab(workspace) {
                    let _ = tab.webview.evaluate_script("location.reload()");
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

fn show_browser_menu(workspace: &mut EmbeddedBrowserWorkspace) {
    if let Some(menu) = workspace.overflow_menu.take() {
        let was_visible = menu.is_visible();
        menu.popdown();
        if was_visible {
            return;
        }
    }
    install_browser_menu_styles();
    let menu = gtk::Menu::new();
    menu.set_size_request(292, -1);
    menu.style_context().add_class("browser-overflow-menu");
    menu.append(&browser_native_menu_item(
        "新建标签页",
        Some("Ctrl+T"),
        || queue_action(BrowserAction::NewTab(None)),
    ));
    menu.append(&browser_native_menu_item(
        "重新加载",
        Some("Ctrl+R"),
        || queue_action(BrowserAction::Reload),
    ));
    menu.append(&browser_native_menu_item(
        "复制当前链接",
        None,
        || queue_action(BrowserAction::CopyUrl),
    ));
    menu.append(&gtk::SeparatorMenuItem::new());
    menu.append(&browser_native_zoom_item(workspace));
    menu.append(&browser_native_menu_item(
        "适应页面宽度",
        None,
        || queue_action(BrowserAction::FitWidth),
    ));
    menu.append(&browser_native_menu_item(
        "批注选中内容",
        None,
        || queue_action(BrowserAction::CommentSelection),
    ));
    menu.append(&gtk::SeparatorMenuItem::new());
    menu.append(&browser_native_menu_item(
        "浏览器信息",
        Some("›"),
        || queue_action(BrowserAction::ShowInfo),
    ));
    menu.show_all();
    menu.popup_at_widget(
        &workspace.toolbar_webview.webview(),
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
    dialog.set_title("浏览器信息");
    dialog.set_transient_for(Some(&parent));
    dialog.set_destroy_with_parent(true);
    dialog.set_modal(false);
    dialog.set_resizable(false);
    dialog.set_default_size(520, -1);
    dialog.set_position(gtk::WindowPosition::CenterOnParent);
    install_browser_menu_styles();
    dialog.style_context().add_class("browser-info-dialog");

    let info = gtk::Box::new(gtk::Orientation::Vertical, 9);
    info.set_margin_start(18);
    info.set_margin_end(18);
    info.set_margin_top(14);
    info.set_margin_bottom(14);

    let summary = gtk::Label::new(Some(&format!(
        "WebKitGTK {}  ·  {} 个标签页",
        workspace.webkit_version,
        workspace.tabs.len()
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
        info.pack_start(&browser_info_value_row(label, value), false, false, 0);
    }

    info.pack_start(
        &gtk::Separator::new(gtk::Orientation::Horizontal),
        false,
        false,
        1,
    );
    let storage_title = gtk::Label::new(Some("存储与清理"));
    storage_title.set_xalign(0.0);
    storage_title
        .style_context()
        .add_class("browser-menu-subheading");
    info.pack_start(&storage_title, false, false, 0);
    let storage = gtk::Label::new(Some(
        "缓存、Cookies、Local Storage 等站点数据会保存在磁盘。标签页的前进/后退记录只存在于内存，退出 Studio 后自动消失。",
    ));
    storage.set_xalign(0.0);
    storage.set_line_wrap(true);
    storage.set_max_width_chars(62);
    storage.style_context().add_class("dim-label");
    info.pack_start(&storage, false, false, 0);

    let clear_status = gtk::Label::new(None);
    clear_status.set_xalign(0.0);
    clear_status.set_line_wrap(true);
    clear_status.style_context().add_class("dim-label");
    let clear_data = gtk::Button::with_label("清除浏览数据…");
    clear_data.set_halign(gtk::Align::Start);
    clear_data.style_context().add_class("browser-clear-data");
    let dialog_for_confirmation = dialog.clone();
    let clear_status_for_confirmation = clear_status.clone();
    clear_data.connect_clicked(move |_| {
        confirm_clear_browser_data(
            &dialog_for_confirmation,
            clear_status_for_confirmation.clone(),
        );
    });
    info.pack_start(&clear_data, false, false, 0);
    info.pack_start(&clear_status, false, false, 0);

    dialog.content_area().add(&info);
    dialog.show_all();
    dialog.present();
    workspace.info_dialog = Some(dialog);
}

fn confirm_clear_browser_data(parent: &gtk::Dialog, status: gtk::Label) {
    let confirmation = gtk::MessageDialog::new(
        Some(parent),
        gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
        gtk::MessageType::Warning,
        gtk::ButtonsType::None,
        "清除所有内嵌浏览器数据？",
    );
    confirmation.set_secondary_text(Some(
        "将清除缓存、Cookies、Local Storage、IndexedDB 和服务工作线程等共享站点数据。网站登录状态可能失效，此操作无法撤销。",
    ));
    confirmation.add_button("取消", gtk::ResponseType::Cancel);
    let clear = confirmation.add_button("清除", gtk::ResponseType::Accept);
    clear.style_context().add_class("browser-clear-data");
    confirmation.connect_response(move |confirmation, response| {
        if response == gtk::ResponseType::Accept {
            let result = WORKSPACE.with(|slot| {
                let slot = slot
                    .try_borrow()
                    .map_err(|_| "浏览器正忙，请稍后重试".to_owned())?;
                let workspace = slot.as_ref().ok_or("浏览器工作区尚未初始化")?;
                let tab = active_tab(workspace).ok_or("没有可用的浏览器标签页")?;
                tab.webview
                    .clear_all_browsing_data()
                    .map_err(|error| error.to_string())
            });
            status.set_text(match result {
                Ok(()) => "清理请求已提交。当前页面可能需要重新加载，网站登录状态可能失效。",
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
    let label = gtk::Label::new(Some("缩放"));
    label.set_xalign(0.0);
    label.set_hexpand(true);
    let zoom = active_tab(workspace)
        .map(|tab| format!("{}%", (tab.zoom * 100.0).round() as u32))
        .unwrap_or_else(|| "100%".to_owned());
    let minus =
        browser_menu_square_button("−", "缩小", || queue_action(BrowserAction::ZoomOut));
    let reset = browser_menu_square_button(&zoom, "恢复 100%", || {
        queue_action(BrowserAction::ZoomReset)
    });
    reset.style_context().add_class("browser-zoom-value");
    let plus = browser_menu_square_button("+", "放大", || queue_action(BrowserAction::ZoomIn));
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

fn install_browser_menu_styles() {
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

fn browser_info_value_row(label: &str, value: &str) -> gtk::Box {
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
    let copy = gtk::Button::with_label("复制");
    copy.style_context().add_class("browser-info-copy");
    connect_copy_button(&copy, value.to_owned());
    value_row.pack_start(&path, true, true, 0);
    value_row.pack_end(&copy, false, false, 0);
    row.pack_start(&value_row, false, false, 0);
    row
}

fn connect_copy_button(button: &gtk::Button, value: String) {
    button.connect_clicked(move |button| {
        if copy_to_clipboard(&value) {
            button.set_label("已复制");
            let button = button.clone();
            gtk::glib::timeout_add_local_once(Duration::from_millis(900), move || {
                button.set_label("复制");
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
        workspace.visible = !workspace.visible;
        if !workspace.visible {
            if let Some(menu) = workspace.overflow_menu.take() {
                menu.popdown();
            }
            if let Some(dialog) = workspace.info_dialog.take() {
                dialog.close();
            }
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
        let available = split.allocation().width();
        let max_width = (available - MIN_STUDIO_WIDTH).max(MIN_BROWSER_WIDTH);
        let width = browser_width.clamp(MIN_BROWSER_WIDTH, max_width);
        split.set_position(available - width);
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
    });
    if visible {
        measure_active_fit();
    }
    sync_toolbar();
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
            });
            evaluate(
                &workspace.toolbar_webview,
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
            notify(&workspace.studio_webview, "请先在网页中选择文本");
        }
    });
}

fn notify(webview: &WebView, message: &str) {
    evaluate(webview, "window.__studioEmbeddedBrowser?.notify", &message);
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
        assert_eq!(browser_page_height(853), 771);
        assert_eq!(browser_page_height(TOOLBAR_HEIGHT), 1);
        assert_eq!(browser_page_height(1), 1);
    }
}
