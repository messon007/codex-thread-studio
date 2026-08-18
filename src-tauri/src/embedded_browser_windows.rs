#![cfg(windows)]

//! Windows WebView2 implementation of the global Embedded Browser Workspace.
//! Linux keeps its GTK/WebKit container in `embedded_browser.rs`; this module
//! owns only WRY child HWND/WebView2 layout and reuses the shared domain model.

use std::{
    cell::RefCell,
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::OnceLock,
};

use serde::{Deserialize, Serialize};
use url::Url;
use windows_sys::{
    core::w,
    Win32::{
        Foundation::{GetLastError, GlobalFree, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
        Graphics::Gdi::{
            BeginPaint, ClientToScreen, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint,
            FillRect, FrameRect, GetStockObject, InvalidateRect, SelectObject, SetBkMode,
            SetTextColor, DEFAULT_GUI_FONT, DT_CENTER, DT_END_ELLIPSIS, DT_LEFT, DT_RIGHT,
            DT_SINGLELINE, DT_VCENTER, PAINTSTRUCT, TRANSPARENT,
        },
        System::{
            DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData},
            LibraryLoader::GetModuleHandleW,
            Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE},
            Ole::CF_UNICODETEXT,
        },
        UI::{
            Input::KeyboardAndMouse::{GetCapture, ReleaseCapture, SetCapture},
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, GetCursorPos,
                LoadCursorW, RegisterClassW, SetCursor, SetWindowPos, ShowWindow, CS_DROPSHADOW,
                HWND_TOP, IDC_ARROW, IDC_SIZEWE, SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_NOOWNERZORDER,
                SWP_SHOWWINDOW, SW_SHOWNOACTIVATE, WM_CAPTURECHANGED, WM_ERASEBKGND,
                WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT, WM_SETCURSOR, WNDCLASSW,
                WS_CHILD, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
            },
        },
    },
};
use wry::{
    dpi::{LogicalPosition, LogicalSize, PhysicalSize},
    PageLoadEvent, Rect, WebContext, WebView, WebViewBuilder,
};

use crate::browser_runtime::{
    display_title, download_filename, embedded_browser_platform_available, parse_toolbar_action,
    sanitize_download_filename, unique_download_path, validate_browser_url, BrowserLifecycle,
    BrowserPreferences, ToolbarAction, DEFAULT_BROWSER_URL, MIN_BROWSER_WIDTH, MIN_STUDIO_WIDTH,
};

const TOOLBAR_HEIGHT: u32 = 75;
const SPLITTER_WIDTH: u32 = 8;
const BROWSER_DIVIDER_WIDTH: u32 = 2;
const BROWSER_MENU_WIDTH: i32 = 292;
const BROWSER_MENU_HEIGHT: i32 = 347;
const BROWSER_INFO_HEIGHT: u32 = 252;
const BROWSER_DOWNLOADS_EMPTY_HEIGHT: u32 = 150;
const MAX_SELECTION_BYTES: usize = 16 * 1024;
const MIN_ZOOM: f64 = 0.5;
const MAX_ZOOM: f64 = 2.0;
const TOOLBAR_HTML: &str = include_str!("../../ui/embedded-browser.html");
const BROWSER_PANEL_HTML: &str = include_str!("../../ui/embedded-browser-panel.html");
const READ_SELECTION_SCRIPT: &str = r#"(()=>window.ipc?.postMessage(JSON.stringify({type:'selection',text:(window.getSelection()?.toString()||'').trim().slice(0,16384),url:location.href,title:document.title||''})))()"#;
const STUDIO_BROWSER_IPC_INIT: &str = "window.__studioEmbeddedBrowserIpc = true;";
const TOOLBAR_IPC_INIT: &str = "window.__embeddedBrowserToolbarIpc = true;";
const PANEL_IPC_INIT: &str = "window.__embeddedBrowserPanelIpc = true;";

thread_local! {
    static WORKSPACE: RefCell<Option<EmbeddedBrowserWorkspace>> = const { RefCell::new(None) };
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
struct FitMetrics {
    request_id: u64,
    inner_width: f64,
    scroll_width: f64,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum PageIpcMessage {
    Selection(BrowserSelection),
    FitMetrics(FitMetrics),
}

struct BrowserTab {
    id: u64,
    url: String,
    title: String,
    zoom: f64,
    fit_width: bool,
    webview: WebView,
}

#[derive(Clone, Copy, Eq, PartialEq)]
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

struct NativeSplitter {
    hwnd: HWND,
}

struct NativeBrowserMenu {
    hwnd: HWND,
}

impl Drop for NativeBrowserMenu {
    fn drop(&mut self) {
        unsafe {
            if GetCapture() == self.hwnd {
                let _ = ReleaseCapture();
            }
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum BrowserMenuHit {
    #[default]
    None,
    NewTab,
    Reload,
    CopyUrl,
    ZoomOut,
    ZoomReset,
    ZoomIn,
    FitWidth,
    CommentSelection,
    ShowInfo,
    ShowDownloads,
    Exit,
}

impl Drop for NativeSplitter {
    fn drop(&mut self) {
        unsafe {
            if GetCapture() == self.hwnd {
                let _ = ReleaseCapture();
            }
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WorkspaceLayout {
    visible: bool,
    total_width: u32,
    total_height: u32,
    browser_left: u32,
    browser_width: u32,
    page_height: u32,
}

struct EmbeddedBrowserWorkspace {
    window: tauri::Window,
    studio_webview: WebView,
    toolbar_webview: Option<WebView>,
    browser_menu: Option<NativeBrowserMenu>,
    browser_menu_hover: BrowserMenuHit,
    browser_panel_webview: Option<WebView>,
    browser_panel_height: u32,
    splitter: Option<NativeSplitter>,
    tabs: Vec<BrowserTab>,
    active_tab_id: u64,
    next_tab_id: u64,
    lifecycle: BrowserLifecycle,
    preferences: BrowserPreferences,
    browser_width: u32,
    _studio_context: WebContext,
    toolbar_context: Option<WebContext>,
    browser_context: Option<WebContext>,
    browser_profile_directory: PathBuf,
    browser_cache_directory: PathBuf,
    downloads_directory: PathBuf,
    pending_selection_tab_id: Option<u64>,
    pending_fit_request: Option<(u64, u64)>,
    next_fit_request_id: u64,
    splitter_dragging: bool,
    splitter_preview_width: Option<u32>,
    splitter_drag_origin_x: Option<i32>,
    last_layout: Option<WorkspaceLayout>,
    visible_tab_id: Option<u64>,
    recent_recoveries: HashMap<u64, (u8, std::time::Instant)>,
    downloads: Vec<BrowserDownload>,
    translations: HashMap<String, String>,
}

pub fn is_supported() -> bool {
    embedded_browser_platform_available()
}

pub fn build(
    app: &tauri::App,
    studio_url: Url,
    initialization_script: &str,
    preferences: BrowserPreferences,
) -> Result<(), Box<dyn std::error::Error>> {
    let window = tauri::window::WindowBuilder::new(app, "main")
        .title("Codex Thread Studio")
        .inner_size(1400.0, 900.0)
        .min_inner_size((MIN_STUDIO_WIDTH + MIN_BROWSER_WIDTH) as f64, 660.0)
        .build()?;
    let studio_profile = profile_directory("studio-shell");
    fs::create_dir_all(&studio_profile)?;
    let mut studio_context = WebContext::new(Some(studio_profile));
    let studio_action_window = window.clone();
    let studio_webview = WebViewBuilder::with_web_context(&mut studio_context)
        .with_url(studio_url.as_str())
        .with_initialization_script(&format!(
            "{initialization_script}\n{STUDIO_BROWSER_IPC_INIT}"
        ))
        .with_clipboard(true)
        .with_ipc_handler(move |request| handle_studio_ipc(&studio_action_window, request))
        .with_navigation_handler(intercept_studio_navigation)
        .with_bounds(bounds(0, 0, 1, 1))
        .build_as_child(&window)?;
    let profile = profile_directory("embedded-browser-webview2");
    let workspace = EmbeddedBrowserWorkspace {
        window: window.clone(),
        studio_webview,
        toolbar_webview: None,
        browser_menu: None,
        browser_menu_hover: BrowserMenuHit::None,
        browser_panel_webview: None,
        browser_panel_height: 0,
        splitter: None,
        tabs: Vec::new(),
        active_tab_id: 0,
        next_tab_id: 1,
        lifecycle: BrowserLifecycle::Cold,
        browser_width: preferences.embedded_width.clamp(MIN_BROWSER_WIDTH, 1200),
        preferences,
        _studio_context: studio_context,
        toolbar_context: None,
        browser_context: None,
        browser_cache_directory: profile.join("Default").join("Cache"),
        browser_profile_directory: profile,
        downloads_directory: download_directory(),
        pending_selection_tab_id: None,
        pending_fit_request: None,
        next_fit_request_id: 1,
        splitter_dragging: false,
        splitter_preview_width: None,
        splitter_drag_origin_x: None,
        last_layout: None,
        visible_tab_id: None,
        recent_recoveries: HashMap::new(),
        downloads: Vec::new(),
        translations: HashMap::new(),
    };
    WORKSPACE.with(|slot| *slot.borrow_mut() = Some(workspace));
    window.on_window_event(|event| {
        if matches!(event, tauri::WindowEvent::Resized(_)) {
            close_native_browser_menu();
            layout_workspace();
            request_active_fit_measurement();
        }
    });
    layout_workspace();
    sync_studio_state();
    Ok(())
}

fn create_native_splitter(window: &tauri::Window) -> Result<NativeSplitter, String> {
    static CLASS_REGISTRATION: OnceLock<Result<(), String>> = OnceLock::new();

    let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
    if instance.is_null() {
        return Err(format!(
            "Unable to read the Browser divider module handle: {}",
            unsafe { GetLastError() }
        ));
    }
    CLASS_REGISTRATION
        .get_or_init(|| {
            let class = WNDCLASSW {
                lpfnWndProc: Some(native_splitter_wnd_proc),
                hInstance: instance,
                hCursor: unsafe { LoadCursorW(std::ptr::null_mut(), IDC_SIZEWE) },
                lpszClassName: w!("CodexThreadStudioBrowserSplitter"),
                ..Default::default()
            };
            if unsafe { RegisterClassW(&class) } == 0 {
                Err(format!(
                    "Unable to register the Browser divider window class: {}",
                    unsafe { GetLastError() }
                ))
            } else {
                Ok(())
            }
        })
        .clone()?;

    let parent = window
        .hwnd()
        .map_err(|error| format!("Unable to read the Studio HWND: {error}"))?;
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            w!("CodexThreadStudioBrowserSplitter"),
            w!(""),
            WS_CHILD,
            0,
            0,
            1,
            1,
            parent.0 as HWND,
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        )
    };
    if hwnd.is_null() {
        return Err(format!(
            "Unable to create the native Browser divider: {}",
            unsafe { GetLastError() }
        ));
    }
    Ok(NativeSplitter { hwnd })
}

unsafe extern "system" fn native_splitter_wnd_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_SETCURSOR => {
            unsafe {
                SetCursor(LoadCursorW(std::ptr::null_mut(), IDC_SIZEWE));
            }
            1
        }
        WM_LBUTTONDOWN => {
            if let Some(cursor_x) = native_cursor_x() {
                begin_native_splitter_drag(cursor_x);
                unsafe {
                    SetCapture(hwnd);
                    let _ = InvalidateRect(hwnd, std::ptr::null(), 0);
                }
            }
            0
        }
        WM_MOUSEMOVE => {
            if unsafe { GetCapture() } == hwnd {
                if let Some(cursor_x) = native_cursor_x() {
                    update_native_splitter_drag(cursor_x);
                }
            }
            0
        }
        WM_LBUTTONUP => {
            if unsafe { GetCapture() } == hwnd {
                unsafe {
                    let _ = ReleaseCapture();
                }
            }
            end_native_splitter_drag();
            unsafe {
                let _ = InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            0
        }
        WM_CAPTURECHANGED => {
            end_native_splitter_drag();
            unsafe {
                let _ = InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            0
        }
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            let mut paint = PAINTSTRUCT::default();
            let hdc = unsafe { BeginPaint(hwnd, &mut paint) };
            let dragging = WORKSPACE.with(|slot| {
                slot.try_borrow()
                    .ok()
                    .and_then(|slot| slot.as_ref().map(|workspace| workspace.splitter_dragging))
                    .unwrap_or(false)
            });
            let mut client = RECT::default();
            unsafe {
                let _ = GetClientRect(hwnd, &mut client);
            }
            let background = unsafe { CreateSolidBrush(0x00FF_FFFF) };
            if !background.is_null() {
                unsafe {
                    let _ = FillRect(hdc, &client, background);
                    let _ = DeleteObject(background);
                }
            }
            let line_width = if dragging { 4 } else { 2 };
            let center = (client.right - client.left) / 2;
            let line = RECT {
                left: center - line_width / 2,
                top: client.top,
                right: center + (line_width + 1) / 2,
                bottom: client.bottom,
            };
            let line_color = if dragging { 0x0098_A36A } else { 0x00E2_DED5 };
            let line_brush = unsafe { CreateSolidBrush(line_color) };
            if !line_brush.is_null() {
                unsafe {
                    let _ = FillRect(hdc, &line, line_brush);
                    let _ = DeleteObject(line_brush);
                }
            }
            unsafe {
                let _ = EndPaint(hwnd, &paint);
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

fn native_cursor_x() -> Option<i32> {
    let mut cursor = POINT::default();
    (unsafe { GetCursorPos(&mut cursor) } != 0).then_some(cursor.x)
}

fn set_native_splitter_bounds(
    splitter: &NativeSplitter,
    window: &tauri::Window,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    visible: bool,
) {
    let scale = window.scale_factor().unwrap_or(1.0).max(1.0);
    let physical = |value: u32| ((value as f64 * scale).round() as i32).max(1);
    let flags = SWP_NOACTIVATE
        | SWP_NOOWNERZORDER
        | if visible {
            SWP_SHOWWINDOW
        } else {
            SWP_HIDEWINDOW
        };
    unsafe {
        let _ = SetWindowPos(
            splitter.hwnd,
            HWND_TOP,
            (x as f64 * scale).round() as i32,
            (y as f64 * scale).round() as i32,
            physical(width),
            physical(height),
            flags,
        );
    }
}

fn create_native_browser_menu(window: &tauri::Window) -> Result<NativeBrowserMenu, String> {
    static CLASS_REGISTRATION: OnceLock<Result<(), String>> = OnceLock::new();

    let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
    if instance.is_null() {
        return Err(format!(
            "Unable to read the Browser menu module handle: {}",
            unsafe { GetLastError() }
        ));
    }
    CLASS_REGISTRATION
        .get_or_init(|| {
            let class = WNDCLASSW {
                style: CS_DROPSHADOW,
                lpfnWndProc: Some(native_browser_menu_wnd_proc),
                hInstance: instance,
                hCursor: unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) },
                lpszClassName: w!("CodexThreadStudioBrowserMenu"),
                ..Default::default()
            };
            if unsafe { RegisterClassW(&class) } == 0 {
                Err(format!(
                    "Unable to register the Browser menu window class: {}",
                    unsafe { GetLastError() }
                ))
            } else {
                Ok(())
            }
        })
        .clone()?;

    let parent = window
        .hwnd()
        .map_err(|error| format!("Unable to read the Studio HWND: {error}"))?;
    let scale = window.scale_factor().unwrap_or(1.0).max(1.0);
    let width = (BROWSER_MENU_WIDTH as f64 * scale).round() as i32;
    let height = (BROWSER_MENU_HEIGHT as f64 * scale).round() as i32;
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            w!("CodexThreadStudioBrowserMenu"),
            w!(""),
            WS_POPUP,
            0,
            0,
            width,
            height,
            parent.0 as HWND,
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        )
    };
    if hwnd.is_null() {
        return Err(format!(
            "Unable to create the native Browser menu: {}",
            unsafe { GetLastError() }
        ));
    }

    let mut origin = POINT::default();
    let mut parent_rect = RECT::default();
    unsafe {
        let _ = ClientToScreen(parent.0 as HWND, &mut origin);
        let _ = GetClientRect(parent.0 as HWND, &mut parent_rect);
        let x = origin.x + parent_rect.right - width - (8.0 * scale).round() as i32;
        let y = origin.y + (TOOLBAR_HEIGHT as f64 * scale).round() as i32;
        let _ = SetWindowPos(
            hwnd,
            HWND_TOP,
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        SetCapture(hwnd);
    }
    Ok(NativeBrowserMenu { hwnd })
}

unsafe extern "system" fn native_browser_menu_wnd_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_SETCURSOR => {
            unsafe {
                SetCursor(LoadCursorW(std::ptr::null_mut(), IDC_ARROW));
            }
            1
        }
        WM_MOUSEMOVE => {
            let hit = native_browser_menu_hit(hwnd, lparam).unwrap_or(BrowserMenuHit::None);
            let changed = WORKSPACE.with(|slot| {
                let Ok(mut slot) = slot.try_borrow_mut() else {
                    return false;
                };
                let Some(workspace) = slot.as_mut() else {
                    return false;
                };
                if workspace.browser_menu_hover == hit {
                    return false;
                }
                workspace.browser_menu_hover = hit;
                true
            });
            if changed {
                unsafe {
                    let _ = InvalidateRect(hwnd, std::ptr::null(), 0);
                }
            }
            0
        }
        WM_LBUTTONDOWN => 0,
        WM_LBUTTONUP => {
            let Some(hit) = native_browser_menu_hit(hwnd, lparam) else {
                close_native_browser_menu();
                return 0;
            };
            if hit == BrowserMenuHit::None {
                return 0;
            }
            let (action, keep_open) = match hit {
                BrowserMenuHit::NewTab => (ToolbarAction::NewTab(None), false),
                BrowserMenuHit::Reload => (ToolbarAction::Reload, false),
                BrowserMenuHit::CopyUrl => (ToolbarAction::CopyUrl, false),
                BrowserMenuHit::ZoomOut => (ToolbarAction::ZoomOut, true),
                BrowserMenuHit::ZoomReset => (ToolbarAction::ZoomReset, true),
                BrowserMenuHit::ZoomIn => (ToolbarAction::ZoomIn, true),
                BrowserMenuHit::FitWidth => (ToolbarAction::FitWidth, false),
                BrowserMenuHit::CommentSelection => (ToolbarAction::CommentSelection, false),
                BrowserMenuHit::ShowInfo => (ToolbarAction::ShowInfo, false),
                BrowserMenuHit::ShowDownloads => (ToolbarAction::ShowDownloads, false),
                BrowserMenuHit::Exit => (ToolbarAction::Exit, false),
                BrowserMenuHit::None => return 0,
            };
            if !keep_open {
                close_native_browser_menu();
            }
            dispatch_toolbar_action(action);
            if keep_open {
                unsafe {
                    let _ = InvalidateRect(hwnd, std::ptr::null(), 0);
                }
            }
            0
        }
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            paint_native_browser_menu(hwnd);
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

fn native_browser_menu_hit(hwnd: HWND, lparam: LPARAM) -> Option<BrowserMenuHit> {
    let raw = lparam as u32;
    let x = (raw as u16 as i16) as i32;
    let y = ((raw >> 16) as u16 as i16) as i32;
    let mut client = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut client);
    }
    if x < 0 || y < 0 || x >= client.right || y >= client.bottom {
        return None;
    }
    let scale = (client.right as f64 / BROWSER_MENU_WIDTH as f64).max(1.0);
    let x = x as f64 / scale;
    let y = y as f64 / scale;
    let hit = match y as i32 {
        6..=39 => BrowserMenuHit::NewTab,
        40..=73 => BrowserMenuHit::Reload,
        74..=107 => BrowserMenuHit::CopyUrl,
        117..=152 => match x as i32 {
            165..=192 => BrowserMenuHit::ZoomOut,
            199..=248 => BrowserMenuHit::ZoomReset,
            255..=282 => BrowserMenuHit::ZoomIn,
            _ => BrowserMenuHit::None,
        },
        153..=186 => BrowserMenuHit::FitWidth,
        187..=220 => BrowserMenuHit::CommentSelection,
        230..=263 => BrowserMenuHit::ShowInfo,
        264..=297 => BrowserMenuHit::ShowDownloads,
        307..=340 => BrowserMenuHit::Exit,
        _ => BrowserMenuHit::None,
    };
    Some(hit)
}

fn paint_native_browser_menu(hwnd: HWND) {
    let mut paint = PAINTSTRUCT::default();
    let hdc = unsafe { BeginPaint(hwnd, &mut paint) };
    let mut client = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut client);
    }
    let scale = (client.right as f64 / BROWSER_MENU_WIDTH as f64).max(1.0);
    let (hover, zoom_percent, translations) = WORKSPACE.with(|slot| {
        slot.try_borrow()
            .ok()
            .and_then(|slot| {
                let workspace = slot.as_ref()?;
                Some((
                    workspace.browser_menu_hover,
                    active_tab(workspace)
                        .map(|tab| (tab.zoom * 100.0).round() as u32)
                        .unwrap_or(100),
                    workspace.translations.clone(),
                ))
            })
            .unwrap_or((BrowserMenuHit::None, 100, HashMap::new()))
    });
    let background = unsafe { CreateSolidBrush(0x00FF_FFFF) };
    let border = unsafe { CreateSolidBrush(0x00E6_DDD7) };
    unsafe {
        let _ = FillRect(hdc, &client, background);
        let _ = FrameRect(hdc, &client, border);
        let _ = DeleteObject(background);
        let _ = DeleteObject(border);
        let font = GetStockObject(DEFAULT_GUI_FONT);
        let previous_font = SelectObject(hdc, font);
        let _ = SetBkMode(hdc, TRANSPARENT as i32);

        draw_native_menu_item(
            hdc,
            scale,
            6,
            translated(&translations, "New tab"),
            Some("Ctrl+T"),
            hover == BrowserMenuHit::NewTab,
        );
        draw_native_menu_item(
            hdc,
            scale,
            40,
            translated(&translations, "Reload"),
            Some("Ctrl+R"),
            hover == BrowserMenuHit::Reload,
        );
        draw_native_menu_item(
            hdc,
            scale,
            74,
            translated(&translations, "Copy current link"),
            None,
            hover == BrowserMenuHit::CopyUrl,
        );
        draw_native_menu_separator(hdc, scale, 112);
        draw_native_zoom_row(hdc, scale, hover, zoom_percent, &translations);
        draw_native_menu_item(
            hdc,
            scale,
            153,
            translated(&translations, "Fit page width"),
            None,
            hover == BrowserMenuHit::FitWidth,
        );
        draw_native_menu_item(
            hdc,
            scale,
            187,
            translated(&translations, "Comment on selection"),
            None,
            hover == BrowserMenuHit::CommentSelection,
        );
        draw_native_menu_separator(hdc, scale, 225);
        draw_native_menu_item(
            hdc,
            scale,
            230,
            translated(&translations, "Browser information"),
            Some("›"),
            hover == BrowserMenuHit::ShowInfo,
        );
        draw_native_menu_item(
            hdc,
            scale,
            264,
            translated(&translations, "Downloads"),
            Some("›"),
            hover == BrowserMenuHit::ShowDownloads,
        );
        draw_native_menu_separator(hdc, scale, 302);
        draw_native_menu_item(
            hdc,
            scale,
            307,
            translated(&translations, "Exit browser"),
            None,
            hover == BrowserMenuHit::Exit,
        );

        let _ = SelectObject(hdc, previous_font);
        let _ = EndPaint(hwnd, &paint);
    }
}

unsafe fn draw_native_menu_item(
    hdc: windows_sys::Win32::Graphics::Gdi::HDC,
    scale: f64,
    top: i32,
    label: &str,
    trailing: Option<&str>,
    hovered: bool,
) {
    let row = scaled_rect(scale, 6, top, 286, top + 34);
    if hovered {
        let brush = unsafe { CreateSolidBrush(0x00F8_F5F3) };
        unsafe {
            let _ = FillRect(hdc, &row, brush);
            let _ = DeleteObject(brush);
        }
    }
    unsafe {
        let _ = SetTextColor(hdc, if hovered { 0x005D_6B17 } else { 0x0033_2017 });
        draw_native_text(
            hdc,
            scale,
            label,
            (15, top, 218, top + 34),
            DT_LEFT | DT_END_ELLIPSIS,
        );
        if let Some(trailing) = trailing {
            let _ = SetTextColor(hdc, 0x0096_847A);
            draw_native_text(hdc, scale, trailing, (218, top, 277, top + 34), DT_RIGHT);
        }
    }
}

fn translated<'a>(translations: &'a HashMap<String, String>, source: &'static str) -> &'a str {
    translations
        .get(source)
        .map(String::as_str)
        .unwrap_or(source)
}

unsafe fn draw_native_zoom_row(
    hdc: windows_sys::Win32::Graphics::Gdi::HDC,
    scale: f64,
    hover: BrowserMenuHit,
    zoom_percent: u32,
    translations: &HashMap<String, String>,
) {
    unsafe {
        let _ = SetTextColor(hdc, 0x0033_2017);
        draw_native_text(
            hdc,
            scale,
            translated(translations, "Zoom"),
            (15, 117, 155, 153),
            DT_LEFT,
        );
        draw_native_square(
            hdc,
            scale,
            165,
            121,
            28,
            "−",
            hover == BrowserMenuHit::ZoomOut,
        );
        draw_native_square(
            hdc,
            scale,
            199,
            121,
            50,
            &format!("{zoom_percent}%"),
            hover == BrowserMenuHit::ZoomReset,
        );
        draw_native_square(
            hdc,
            scale,
            255,
            121,
            28,
            "+",
            hover == BrowserMenuHit::ZoomIn,
        );
    }
}

unsafe fn draw_native_square(
    hdc: windows_sys::Win32::Graphics::Gdi::HDC,
    scale: f64,
    left: i32,
    top: i32,
    width: i32,
    text: &str,
    hovered: bool,
) {
    let rect = scaled_rect(scale, left, top, left + width, top + 28);
    let background = unsafe { CreateSolidBrush(if hovered { 0x00F8_F5F3 } else { 0x00FF_FFFF }) };
    let border = unsafe { CreateSolidBrush(0x00E6_DDD7) };
    unsafe {
        let _ = FillRect(hdc, &rect, background);
        let _ = FrameRect(hdc, &rect, border);
        let _ = DeleteObject(background);
        let _ = DeleteObject(border);
        let _ = SetTextColor(hdc, if hovered { 0x005D_6B17 } else { 0x007A_6458 });
        draw_native_text(
            hdc,
            scale,
            text,
            (left, top, left + width, top + 28),
            DT_CENTER,
        );
    }
}

unsafe fn draw_native_menu_separator(
    hdc: windows_sys::Win32::Graphics::Gdi::HDC,
    scale: f64,
    y: i32,
) {
    let line = scaled_rect(scale, 9, y, 283, y + 1);
    let brush = unsafe { CreateSolidBrush(0x00EC_E4DF) };
    unsafe {
        let _ = FillRect(hdc, &line, brush);
        let _ = DeleteObject(brush);
    }
}

unsafe fn draw_native_text(
    hdc: windows_sys::Win32::Graphics::Gdi::HDC,
    scale: f64,
    text: &str,
    bounds: (i32, i32, i32, i32),
    align: u32,
) {
    let (left, top, right, bottom) = bounds;
    let mut rect = scaled_rect(scale, left, top, right, bottom);
    let text = text.encode_utf16().collect::<Vec<_>>();
    unsafe {
        let _ = DrawTextW(
            hdc,
            text.as_ptr(),
            text.len() as i32,
            &mut rect,
            align | DT_SINGLELINE | DT_VCENTER,
        );
    }
}

fn scaled_rect(scale: f64, left: i32, top: i32, right: i32, bottom: i32) -> RECT {
    RECT {
        left: (left as f64 * scale).round() as i32,
        top: (top as f64 * scale).round() as i32,
        right: (right as f64 * scale).round() as i32,
        bottom: (bottom as f64 * scale).round() as i32,
    }
}

fn close_native_browser_menu() {
    WORKSPACE.with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut() {
            if let Some(workspace) = slot.as_mut() {
                workspace.browser_menu = None;
                workspace.browser_menu_hover = BrowserMenuHit::None;
            }
        }
    });
}

fn intercept_studio_navigation(raw: String) -> bool {
    // On Windows, trusted Studio and toolbar documents use WebView2 IPC. A
    // WebView2 navigation callback cannot synchronously create another child
    // controller: it is a COM callback and would re-enter the controller
    // creation message pump. Keep the old custom scheme blocked as a safe
    // fallback, but do not dispatch work from this callback.
    parse_toolbar_action(&raw).is_none()
}

fn handle_studio_ipc(window: &tauri::Window, request: wry::http::Request<String>) {
    handle_browser_action_ipc(window, request.body(), "studio-browser-action");
}

fn handle_toolbar_ipc(window: &tauri::Window, request: wry::http::Request<String>) {
    handle_browser_action_ipc(window, request.body(), "browser-action");
}

fn handle_browser_panel_ipc(window: &tauri::Window, request: wry::http::Request<String>) {
    #[derive(Deserialize)]
    struct BrowserPanelIpc {
        #[serde(rename = "type")]
        kind: String,
        #[serde(default)]
        height: f64,
    }

    if let Ok(message) = serde_json::from_str::<BrowserPanelIpc>(request.body()) {
        if message.kind == "browser-panel-size" && message.height.is_finite() {
            let height = message.height.round().clamp(80.0, 360.0) as u32;
            queue_browser_action(window, move || set_browser_panel_height(height));
            return;
        }
    }
    handle_browser_action_ipc(window, request.body(), "browser-panel-action");
}

fn handle_browser_action_ipc(window: &tauri::Window, raw: &str, expected_type: &str) {
    #[derive(Deserialize)]
    struct BrowserActionIpc {
        #[serde(rename = "type")]
        kind: String,
        url: String,
    }

    let Ok(message) = serde_json::from_str::<BrowserActionIpc>(raw) else {
        return;
    };
    if message.kind != expected_type {
        return;
    }
    let requested_width = Url::parse(&message.url).ok().and_then(|url| {
        matches!(url.host_str(), Some("show-browser") | Some("open-browser"))
            .then(|| {
                url.query_pairs()
                    .find(|(name, _)| name == "width")
                    .and_then(|(_, value)| value.parse::<u32>().ok())
            })
            .flatten()
    });
    if let Some(action) = parse_toolbar_action(&message.url) {
        queue_browser_action(window, move || {
            eprintln!("Embedded Browser: executing deferred toolbar action");
            if let Some(width) = requested_width {
                set_browser_width(width);
            }
            dispatch_toolbar_action(action);
        });
    }
}

fn dispatch_toolbar_action(action: ToolbarAction) {
    match action {
        ToolbarAction::Toggle => toggle_workspace(),
        ToolbarAction::Show => show_workspace(),
        ToolbarAction::Open(url) => {
            show_workspace();
            navigate_active(&url);
        }
        ToolbarAction::Exit => exit_workspace(),
        ToolbarAction::Navigate(url) => navigate_active(&url),
        ToolbarAction::NewTab(url) => new_tab(url.as_deref()),
        ToolbarAction::ActivateTab(id) => activate_tab(id),
        ToolbarAction::CloseTab(id) => close_tab(id),
        ToolbarAction::NextTab => cycle_tab(1),
        ToolbarAction::PreviousTab => cycle_tab(-1),
        ToolbarAction::Back => evaluate_active("history.back()"),
        ToolbarAction::Forward => evaluate_active("history.forward()"),
        ToolbarAction::Reload => evaluate_active("location.reload()"),
        ToolbarAction::ZoomIn => adjust_zoom(0.1),
        ToolbarAction::ZoomOut => adjust_zoom(-0.1),
        ToolbarAction::ZoomReset => set_active_zoom(1.0, false),
        ToolbarAction::FitWidth => fit_active_width(),
        ToolbarAction::CommentSelection => request_selection(),
        ToolbarAction::CopyUrl => copy_active_url(),
        ToolbarAction::ShowMenu => toggle_browser_panel(),
        ToolbarAction::ShowInfo => show_browser_info(),
        ToolbarAction::ShowDownloads => show_downloads(),
        ToolbarAction::OpenDownloadsDirectory => open_downloads_directory(),
        ToolbarAction::SetBrowserWidth(width) => set_browser_width(width),
        ToolbarAction::SetTranslations(translations) => set_browser_translations(translations),
    }
}

fn set_browser_translations(translations: HashMap<String, String>) {
    WORKSPACE.with(|slot| {
        let Ok(mut slot) = slot.try_borrow_mut() else {
            return;
        };
        let Some(workspace) = slot.as_mut() else {
            return;
        };
        workspace.translations = translations;
        workspace.browser_menu = None;
        workspace.browser_panel_webview = None;
    });
    sync_toolbar();
}

fn toggle_workspace() {
    let changed = WORKSPACE.with(|slot| {
        let Ok(mut slot) = slot.try_borrow_mut() else {
            return false;
        };
        let Some(workspace) = slot.as_mut() else {
            return false;
        };
        if workspace.lifecycle.visible() {
            workspace.lifecycle = workspace.lifecycle.hide();
        } else {
            if let Err(error) = ensure_browser_runtime(workspace) {
                eprintln!("Embedded Browser Workspace initialization failed: {error}");
                notify(
                    &workspace.studio_webview,
                    &format!("Unable to initialize the embedded browser: {error}"),
                );
                return false;
            }
            workspace.lifecycle = workspace.lifecycle.show();
        }
        true
    });
    if changed {
        layout_workspace();
        sync_toolbar();
        sync_studio_state();
        if browser_visible() {
            focus_active_tab();
            // The initial page is deliberately constructed while the Browser
            // is hidden. It can therefore finish loading before the page has
            // real bounds, so its load callback must not be our only fit
            // trigger.
            request_active_fit_measurement();
        }
    }
}

fn show_workspace() {
    let visible = WORKSPACE.with(|slot| {
        slot.try_borrow()
            .ok()
            .and_then(|slot| slot.as_ref().map(|w| w.lifecycle.visible()))
            .unwrap_or(false)
    });
    if !visible {
        toggle_workspace();
    }
}

fn exit_workspace() {
    let changed = WORKSPACE.with(|slot| {
        let Ok(mut slot) = slot.try_borrow_mut() else {
            return false;
        };
        let Some(workspace) = slot.as_mut() else {
            return false;
        };
        workspace.pending_selection_tab_id = None;
        workspace.pending_fit_request = None;
        workspace.browser_menu = None;
        workspace.browser_menu_hover = BrowserMenuHit::None;
        workspace.browser_panel_webview = None;
        workspace.browser_panel_height = 0;
        workspace.tabs.clear();
        workspace.active_tab_id = 0;
        workspace.next_tab_id = 1;
        workspace.splitter = None;
        workspace.toolbar_webview = None;
        workspace.browser_context = None;
        workspace.toolbar_context = None;
        workspace.lifecycle = workspace.lifecycle.exit();
        true
    });
    if changed {
        layout_workspace();
        sync_studio_state();
    }
}

fn ensure_browser_runtime(workspace: &mut EmbeddedBrowserWorkspace) -> Result<(), String> {
    if workspace.lifecycle.runtime_loaded()
        && workspace.toolbar_webview.is_some()
        && !workspace.tabs.is_empty()
    {
        return Ok(());
    }
    workspace.tabs.clear();
    workspace.toolbar_webview = None;
    workspace.browser_menu = None;
    workspace.browser_menu_hover = BrowserMenuHit::None;
    workspace.browser_panel_webview = None;
    workspace.browser_panel_height = 0;
    workspace.splitter = None;
    workspace.toolbar_context = None;
    workspace.browser_context = None;
    workspace.active_tab_id = 0;
    workspace.next_tab_id = 1;
    workspace.pending_fit_request = None;
    workspace.splitter_dragging = false;
    workspace.splitter_preview_width = None;
    workspace.splitter_drag_origin_x = None;
    workspace.visible_tab_id = None;
    workspace.last_layout = None;
    fs::create_dir_all(&workspace.browser_profile_directory).map_err(|e| e.to_string())?;
    fs::create_dir_all(&workspace.downloads_directory).map_err(|e| e.to_string())?;
    let toolbar_profile = profile_directory("browser-toolbar-webview2");
    fs::create_dir_all(&toolbar_profile).map_err(|e| e.to_string())?;
    workspace.toolbar_context = Some(WebContext::new(Some(toolbar_profile)));
    workspace.browser_context = Some(WebContext::new(Some(
        workspace.browser_profile_directory.clone(),
    )));
    eprintln!("Embedded Browser: creating trusted toolbar WebView");
    let toolbar_action_window = workspace.window.clone();
    let toolbar = WebViewBuilder::with_web_context(
        workspace.toolbar_context.as_mut().expect("toolbar context"),
    )
    .with_html(TOOLBAR_HTML)
    .with_initialization_script(TOOLBAR_IPC_INIT)
    .with_clipboard(true)
    .with_ipc_handler(move |request| handle_toolbar_ipc(&toolbar_action_window, request))
    .with_navigation_handler(intercept_studio_navigation)
    .with_bounds(bounds(0, 0, 1, TOOLBAR_HEIGHT))
    .with_visible(false)
    .build_as_child(&workspace.window)
    .map_err(|error| format!("Unable to create the trusted Browser Toolbar WebView: {error:?}"))?;
    eprintln!("Embedded Browser: trusted toolbar WebView created");
    eprintln!("Embedded Browser: creating native splitter HWND");
    let splitter = create_native_splitter(&workspace.window)?;
    eprintln!("Embedded Browser: native splitter HWND created");
    workspace.toolbar_webview = Some(toolbar);
    workspace.splitter = Some(splitter);
    eprintln!("Embedded Browser: creating initial page Tab");
    let tab = build_browser_tab(workspace, 1, DEFAULT_BROWSER_URL)
        .map_err(|error| format!("Unable to create the initial Browser page tab: {error}"))?;
    eprintln!("Embedded Browser: initial page Tab created");
    workspace.tabs.push(tab);
    workspace.active_tab_id = 1;
    workspace.next_tab_id = 2;
    workspace.lifecycle = BrowserLifecycle::Hidden;
    Ok(())
}

fn build_browser_tab(
    workspace: &mut EmbeddedBrowserWorkspace,
    id: u64,
    raw_url: &str,
) -> Result<BrowserTab, String> {
    let url = validate_browser_url(raw_url, &workspace.preferences)
        .map_err(|e| e.to_string())?
        .to_string();
    let preferences = workspace.preferences.clone();
    let navigation_preferences = preferences.clone();
    let download_directory = workspace.downloads_directory.clone();
    let context = workspace
        .browser_context
        .as_mut()
        .ok_or("The browser profile is unavailable")?;
    eprintln!("Embedded Browser: creating WebView2 page container for {url}");
    let webview = WebViewBuilder::with_web_context(context)
        .with_url(&url)
        .with_bounds(bounds(0, TOOLBAR_HEIGHT, 1, 1))
        .with_visible(false)
        .with_hotkeys_zoom(false)
        .with_navigation_handler(move |next| {
            allow_remote_navigation(&next, &navigation_preferences)
        })
        .with_new_window_req_handler(move |next| {
            request_popup_tab(&next);
            false
        })
        .with_document_title_changed_handler(move |title| update_tab_title(id, title))
        .with_on_page_load_handler(move |event, loaded| {
            if matches!(event, PageLoadEvent::Finished) {
                update_tab_url(id, loaded);
                request_fit_measurement_for(id);
            }
        })
        .with_ipc_handler(move |request| handle_page_ipc(id, request.body()))
        .with_download_started_handler(move |url, path| {
            begin_download(&preferences, &download_directory, &url, path)
        })
        .with_download_completed_handler(move |url, path, success| {
            finish_download(&url, path.as_deref(), success)
        })
        .build_as_child(&workspace.window)
        .map_err(|error| format!("Unable to create the WebView2 page container: {error:?}"))?;
    eprintln!("Embedded Browser: page container created; binding security callbacks");
    deny_page_permissions(&webview)
        .map_err(|error| format!("Unable to bind the WebView2 page permission policy: {error}"))?;
    bind_page_failure_handler(&webview, id, &workspace.window)
        .map_err(|error| format!("Unable to bind WebView2 page failure handling: {error}"))?;
    bind_tab_shortcuts(&webview, &workspace.window)
        .map_err(|error| format!("Unable to bind WebView2 page shortcuts: {error}"))?;
    eprintln!("Embedded Browser: page callbacks bound");
    Ok(BrowserTab {
        id,
        title: display_title("", &url),
        url,
        zoom: 1.0,
        // Keep responsive pages at 100%, while fixed-width pages are fitted
        // after WebView2 has reported its actual viewport width.
        fit_width: true,
        webview,
    })
}

fn allow_remote_navigation(raw: &str, preferences: &BrowserPreferences) -> bool {
    let allowed = validate_browser_url(raw, preferences).is_ok();
    if !allowed {
        eprintln!("Blocked unsafe Browser navigation: {raw}");
    }
    allowed
}

fn queue_browser_action(window: &tauri::Window, action: impl FnOnce() + Send + 'static) {
    // `Window::run_on_main_thread` is intentionally synchronous when its
    // caller is already on the UI thread. WebView2 invokes IPC, popup,
    // shortcut and renderer-failure handlers on that thread, so calling it
    // directly would still build a child controller before the COM callback
    // returns. Handoff through Tauri's async runtime first; its subsequent
    // main-thread event is processed only after WebView2 has returned.
    let window = window.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = window.run_on_main_thread(action) {
            eprintln!("Embedded Browser: failed to queue Browser action: {error}");
        }
    });
}

fn request_popup_tab(url: &str) {
    let (allowed, window) = WORKSPACE.with(|slot| {
        slot.try_borrow()
            .ok()
            .and_then(|slot| {
                slot.as_ref().map(|w| {
                    (
                        validate_browser_url(url, &w.preferences).is_ok(),
                        Some(w.window.clone()),
                    )
                })
            })
            .unwrap_or((false, None))
    });
    if allowed {
        let url = url.to_owned();
        if let Some(window) = window {
            queue_browser_action(&window, move || new_tab(Some(&url)));
        }
    } else {
        notify_current(&format!("Blocked an unsafe browser URL: {url}"));
    }
}

fn new_tab(url: Option<&str>) {
    show_workspace();
    let changed = WORKSPACE.with(|slot| {
        let Ok(mut slot) = slot.try_borrow_mut() else {
            return false;
        };
        let Some(workspace) = slot.as_mut() else {
            return false;
        };
        let id = workspace.next_tab_id;
        match build_browser_tab(workspace, id, url.unwrap_or(DEFAULT_BROWSER_URL)) {
            Ok(tab) => {
                workspace.tabs.push(tab);
                workspace.active_tab_id = id;
                workspace.next_tab_id = id.wrapping_add(1).max(1);
                true
            }
            Err(error) => {
                notify(
                    &workspace.studio_webview,
                    &format!("Unable to create a browser tab: {error}"),
                );
                false
            }
        }
    });
    if changed {
        close_browser_panel();
        layout_workspace();
        sync_toolbar();
        focus_active_tab();
    }
}

fn activate_tab(id: u64) {
    let changed = WORKSPACE.with(|slot| {
        let Ok(mut slot) = slot.try_borrow_mut() else {
            return false;
        };
        let Some(w) = slot.as_mut() else { return false };
        if w.tabs.iter().any(|tab| tab.id == id) {
            w.active_tab_id = id;
            true
        } else {
            false
        }
    });
    if changed {
        close_browser_panel();
        layout_workspace();
        sync_toolbar();
        focus_active_tab();
    }
}

fn close_tab(id: u64) {
    let changed = WORKSPACE.with(|slot| {
        let Ok(mut slot) = slot.try_borrow_mut() else {
            return false;
        };
        let Some(w) = slot.as_mut() else { return false };
        let Some(index) = w.tabs.iter().position(|tab| tab.id == id) else {
            return false;
        };
        if w.tabs.len() == 1 {
            w.tabs.remove(0);
            let replacement_id = w.next_tab_id;
            match build_browser_tab(w, replacement_id, DEFAULT_BROWSER_URL) {
                Ok(tab) => {
                    w.tabs.push(tab);
                    w.active_tab_id = replacement_id;
                    w.next_tab_id = replacement_id.wrapping_add(1).max(1);
                }
                Err(error) => {
                    notify(
                        &w.studio_webview,
                        &format!("Unable to create a blank tab: {error}"),
                    );
                    return false;
                }
            }
        } else {
            let active = w.active_tab_id == id;
            w.tabs.remove(index);
            if active {
                w.active_tab_id = w.tabs[index.min(w.tabs.len() - 1)].id;
            }
        }
        true
    });
    if changed {
        close_browser_panel();
        layout_workspace();
        sync_toolbar();
        focus_active_tab();
    }
}

fn cycle_tab(delta: isize) {
    let changed = WORKSPACE.with(|slot| {
        let Ok(mut slot) = slot.try_borrow_mut() else {
            return false;
        };
        let Some(w) = slot.as_mut() else { return false };
        let Some(current) = w.tabs.iter().position(|tab| tab.id == w.active_tab_id) else {
            return false;
        };
        let next = (current as isize + delta).rem_euclid(w.tabs.len() as isize) as usize;
        w.active_tab_id = w.tabs[next].id;
        true
    });
    if changed {
        close_browser_panel();
        layout_workspace();
        sync_toolbar();
        focus_active_tab();
    }
}

fn navigate_active(raw: &str) {
    let result = WORKSPACE.with(|slot| {
        let Ok(mut slot) = slot.try_borrow_mut() else {
            return Err("The browser is busy. Try again shortly.".to_owned());
        };
        let w = slot
            .as_mut()
            .ok_or("The browser workspace is not initialized")?;
        let url = validate_browser_url(raw, &w.preferences)
            .map_err(|e| e.to_string())?
            .to_string();
        let tab = active_tab_mut(w).ok_or("No browser tab is available")?;
        tab.fit_width = true;
        tab.zoom = 1.0;
        tab.webview.zoom(1.0).map_err(|e| e.to_string())?;
        tab.url = url.clone();
        tab.title = display_title("", &url);
        tab.webview.load_url(&url).map_err(|e| e.to_string())
    });
    if let Err(error) = result {
        notify_current(&error);
    } else {
        sync_toolbar();
    }
}

fn evaluate_active(script: &str) {
    let result = WORKSPACE.with(|slot| {
        let slot = slot
            .try_borrow()
            .map_err(|_| "The browser is busy. Try again shortly.".to_owned())?;
        active_tab(
            slot.as_ref()
                .ok_or("The browser workspace is not initialized")?,
        )
        .ok_or("No browser tab is available")?
        .webview
        .evaluate_script(script)
        .map_err(|e| e.to_string())
    });
    if let Err(error) = result {
        notify_current(&error);
    } else {
        sync_toolbar();
    }
}

fn adjust_zoom(delta: f64) {
    let zoom = WORKSPACE.with(|slot| {
        slot.try_borrow().ok().and_then(|slot| {
            active_tab(slot.as_ref()?).map(|tab| (tab.zoom + delta).clamp(MIN_ZOOM, MAX_ZOOM))
        })
    });
    if let Some(zoom) = zoom {
        set_active_zoom(zoom, false);
    }
}

fn set_active_zoom(zoom: f64, fit_width: bool) {
    let result = WORKSPACE.with(|slot| {
        let Ok(mut slot) = slot.try_borrow_mut() else {
            return Err("The browser is busy. Try again shortly.".to_owned());
        };
        let tab = active_tab_mut(
            slot.as_mut()
                .ok_or("The browser workspace is not initialized")?,
        )
        .ok_or("No browser tab is available")?;
        let zoom = (zoom.clamp(MIN_ZOOM, MAX_ZOOM) * 100.0).round() / 100.0;
        tab.webview.zoom(zoom).map_err(|e| e.to_string())?;
        tab.zoom = zoom;
        tab.fit_width = fit_width;
        Ok(())
    });
    if let Err(error) = result {
        notify_current(&error);
    }
    sync_toolbar();
}

fn fit_active_width() {
    let id = WORKSPACE.with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut() {
            if let Some(w) = slot.as_mut() {
                let id = w.active_tab_id;
                if let Some(tab) = w.tabs.iter_mut().find(|tab| tab.id == id) {
                    tab.fit_width = true;
                    return Some(id);
                }
            }
        }
        None
    });
    if let Some(id) = id {
        request_fit_measurement_for(id);
    }
    sync_toolbar();
}

fn request_active_fit_measurement() {
    let id = WORKSPACE.with(|slot| {
        slot.try_borrow().ok().and_then(|slot| {
            let workspace = slot.as_ref()?;
            (!workspace.splitter_dragging)
                .then_some(())
                .and_then(|_| active_tab(workspace))
                .filter(|tab| tab.fit_width)
                .map(|tab| tab.id)
        })
    });
    if let Some(id) = id {
        request_fit_measurement_for(id);
    }
}

fn request_fit_measurement_for(id: u64) {
    WORKSPACE.with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut() {
            if let Some(w) = slot.as_mut() {
                if w.lifecycle.visible() && w.active_tab_id == id {
                    let should_measure = w.tabs.iter().any(|tab| tab.id == id && tab.fit_width);
                    if should_measure {
                        let request_id = w.next_fit_request_id;
                        w.next_fit_request_id = request_id.wrapping_add(1).max(1);
                        w.pending_fit_request = Some((id, request_id));
                        if let Some(tab) = w.tabs.iter().find(|tab| tab.id == id) {
                            // Controller bounds are applied asynchronously by WebView2.
                            // Measuring on the next rendered frame avoids using a stale
                            // viewport after splitter drags or window resizes.
                            let _ = tab.webview.evaluate_script(&fit_width_script(request_id));
                        }
                    }
                }
            }
        }
    });
}

fn request_selection() {
    WORKSPACE.with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut() {
            if let Some(w) = slot.as_mut() {
                let id = w.active_tab_id;
                if let Some(tab) = w.tabs.iter().find(|tab| tab.id == id) {
                    let _ = tab.webview.evaluate_script(READ_SELECTION_SCRIPT);
                    w.pending_selection_tab_id = Some(id);
                }
            }
        }
    });
}

fn copy_active_url() {
    let url = WORKSPACE.with(|slot| {
        slot.try_borrow().ok().and_then(|slot| {
            slot.as_ref()
                .and_then(active_tab)
                .map(|tab| tab.url.clone())
        })
    });
    let Some(url) = url else { return };
    if copy_to_clipboard(&url) {
        notify_current("Current link copied");
    } else {
        notify_current("Unable to copy the current link");
    }
}

fn copy_to_clipboard(value: &str) -> bool {
    let mut utf16 = value.encode_utf16().collect::<Vec<_>>();
    utf16.push(0);
    let bytes = std::mem::size_of_val(utf16.as_slice());

    // The clipboard takes ownership of a movable global allocation only after
    // SetClipboardData succeeds. Keep every failure path responsible for the
    // allocation it still owns.
    unsafe {
        let memory = GlobalAlloc(GMEM_MOVEABLE, bytes);
        if memory.is_null() {
            return false;
        }
        let destination = GlobalLock(memory) as *mut u16;
        if destination.is_null() {
            let _ = GlobalFree(memory);
            return false;
        }
        std::ptr::copy_nonoverlapping(utf16.as_ptr(), destination, utf16.len());
        let _ = GlobalUnlock(memory);
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            let _ = GlobalFree(memory);
            return false;
        }
        if EmptyClipboard() == 0 {
            let _ = CloseClipboard();
            let _ = GlobalFree(memory);
            return false;
        }
        if SetClipboardData(CF_UNICODETEXT as u32, memory).is_null() {
            let _ = CloseClipboard();
            let _ = GlobalFree(memory);
            return false;
        }
        let _ = CloseClipboard();
    }
    true
}

fn handle_page_ipc(id: u64, raw: &str) {
    if raw.len() > MAX_SELECTION_BYTES * 2 {
        return;
    }
    let Ok(message) = serde_json::from_str::<PageIpcMessage>(raw) else {
        return;
    };
    match message {
        PageIpcMessage::Selection(selection) => {
            let selection = WORKSPACE.with(|slot| {
                let Ok(mut slot) = slot.try_borrow_mut() else {
                    return None;
                };
                let w = slot.as_mut()?;
                (w.pending_selection_tab_id == Some(id)).then_some(())?;
                w.pending_selection_tab_id = None;
                parse_selection(selection, &w.preferences)
            });
            if let Some(selection) = selection {
                open_comment(&selection);
            } else {
                notify_current("Select text on the web page first");
            }
        }
        PageIpcMessage::FitMetrics(metrics) => {
            let zoom = WORKSPACE.with(|slot| {
                let Ok(mut slot) = slot.try_borrow_mut() else {
                    return None;
                };
                let w = slot.as_mut()?;
                (w.pending_fit_request == Some((id, metrics.request_id))).then_some(())?;
                w.pending_fit_request = None;
                let tab = w.tabs.iter().find(|tab| tab.id == id && tab.fit_width)?;
                let target = (metrics.inner_width.max(1.0)
                    / metrics.scroll_width.max(metrics.inner_width.max(1.0)))
                .clamp(MIN_ZOOM, 1.0);
                ((target - tab.zoom).abs() >= 0.01).then_some(target)
            });
            if let Some(zoom) = zoom {
                set_active_zoom(zoom, true);
            }
        }
    }
}

fn fit_width_script(request_id: u64) -> String {
    format!(
        r#"(()=>{{const report=()=>window.ipc?.postMessage(JSON.stringify({{type:'fit-metrics',requestId:{request_id},innerWidth:window.innerWidth||0,scrollWidth:Math.max(document.documentElement?.scrollWidth||0,document.body?.scrollWidth||0)}}));requestAnimationFrame(()=>requestAnimationFrame(report))}})()"#
    )
}

fn begin_native_splitter_drag(cursor_x: i32) {
    WORKSPACE.with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut() {
            if let Some(w) = slot.as_mut().filter(|w| w.lifecycle.visible()) {
                w.splitter_dragging = true;
                // A fit result started before the pointer was pressed describes
                // an obsolete viewport and must not zoom during the native drag.
                w.pending_fit_request = None;
                w.splitter_preview_width = Some(w.browser_width);
                w.splitter_drag_origin_x = Some(cursor_x);
            }
        }
    });
}

fn update_native_splitter_drag(cursor_x: i32) {
    let changed = WORKSPACE.with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut() {
            if let Some(w) = slot.as_mut() {
                if !w.splitter_dragging {
                    return false;
                }
                let Some(origin_x) = w.splitter_drag_origin_x else {
                    return false;
                };
                let scale = w.window.scale_factor().unwrap_or(1.0).max(1.0);
                let delta = ((cursor_x - origin_x) as f64 / scale).round() as i32;
                let current = w.splitter_preview_width.unwrap_or(w.browser_width);
                let next = clamp_browser_width(
                    (w.browser_width as i32 - delta).max(0) as u32,
                    available_width(w),
                );
                if next != current {
                    w.splitter_preview_width = Some(next);
                    return true;
                }
            }
        }
        false
    });
    if changed {
        layout_splitter_preview();
    }
}

fn end_native_splitter_drag() {
    let changed = WORKSPACE.with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut() {
            if let Some(w) = slot.as_mut() {
                if !w.splitter_dragging {
                    return false;
                }
                w.splitter_dragging = false;
                w.splitter_drag_origin_x = None;
                let preview = w.splitter_preview_width.take();
                if let Some(width) = preview {
                    let width = clamp_browser_width(width, available_width(w));
                    if width != w.browser_width {
                        w.browser_width = width;
                        return true;
                    }
                }
            }
        }
        false
    });
    if changed {
        layout_workspace();
        sync_studio_width();
        request_active_fit_measurement();
    }
}

fn set_browser_width(width: u32) {
    let changed = WORKSPACE.with(|slot| {
        let Ok(mut slot) = slot.try_borrow_mut() else {
            return false;
        };
        let Some(w) = slot.as_mut() else { return false };
        let next = clamp_browser_width(width, available_width(w));
        if next == w.browser_width {
            return false;
        }
        w.browser_width = next;
        w.splitter_preview_width = None;
        true
    });
    if changed {
        layout_workspace();
        sync_studio_width();
        request_active_fit_measurement();
    }
}

/// Moving WebView2 child controllers on every pointer frame makes their
/// renderer surface briefly flash. During a resize drag, keep the page and
/// Studio controllers stable and move only the lightweight splitter guide.
/// The full layout is committed by the matching `splitter-end` event.
fn layout_splitter_preview() {
    WORKSPACE.with(|slot| {
        let Ok(slot) = slot.try_borrow() else {
            return;
        };
        let Some(w) = slot.as_ref() else { return };
        if !w.lifecycle.visible() {
            return;
        }
        let Some(preview_width) = w.splitter_preview_width else {
            return;
        };
        let (total_width, total_height) = logical_window_size(&w.window);
        let browser_width = clamp_browser_width(preview_width, total_width);
        let browser_left = total_width.saturating_sub(browser_width);
        if let Some(splitter) = w.splitter.as_ref() {
            set_native_splitter_bounds(
                splitter,
                &w.window,
                browser_left.saturating_sub(SPLITTER_WIDTH / 2),
                0,
                SPLITTER_WIDTH,
                total_height,
                true,
            );
        }
    });
}

fn layout_workspace() {
    WORKSPACE.with(|slot| {
        let Ok(mut slot) = slot.try_borrow_mut() else {
            return;
        };
        let Some(w) = slot.as_mut() else { return };
        let (total_width, total_height) = logical_window_size(&w.window);
        let visible = w.lifecycle.visible();
        let browser_width = if visible {
            clamp_browser_width(w.browser_width, total_width)
        } else {
            0
        };
        let layout = WorkspaceLayout {
            visible,
            total_width,
            total_height,
            browser_left: total_width.saturating_sub(browser_width),
            browser_width,
            page_height: total_height.saturating_sub(TOOLBAR_HEIGHT).max(1),
        };
        let previous = w.last_layout;
        let geometry_changed = previous != Some(layout);
        let was_visible = previous.is_some_and(|previous| previous.visible);

        if !visible {
            if geometry_changed {
                let _ = w
                    .studio_webview
                    .set_bounds(bounds(0, 0, total_width, total_height));
            }
            if was_visible {
                if let Some(toolbar) = w.toolbar_webview.as_ref() {
                    let _ = toolbar.set_visible(false);
                }
                if let Some(panel) = w.browser_panel_webview.as_ref() {
                    let _ = panel.set_visible(false);
                }
                if let Some(splitter) = w.splitter.as_ref() {
                    set_native_splitter_bounds(splitter, &w.window, 0, 0, 1, 1, false);
                }
                for tab in &w.tabs {
                    let _ = tab.webview.set_visible(false);
                }
            }
            w.visible_tab_id = None;
            w.last_layout = Some(layout);
            return;
        }

        if geometry_changed {
            let _ =
                w.studio_webview
                    .set_bounds(bounds(0, 0, layout.browser_left, layout.total_height));
            let browser_content_left = layout.browser_left.saturating_add(BROWSER_DIVIDER_WIDTH);
            let browser_content_width = layout
                .browser_width
                .saturating_sub(BROWSER_DIVIDER_WIDTH)
                .max(1);
            if let Some(toolbar) = w.toolbar_webview.as_ref() {
                let _ = toolbar.set_bounds(bounds(
                    browser_content_left,
                    0,
                    browser_content_width,
                    TOOLBAR_HEIGHT,
                ));
            }
            if let Some(panel) = w.browser_panel_webview.as_ref() {
                let _ = panel.set_bounds(browser_panel_bounds(
                    layout,
                    w.browser_panel_height,
                    panel_bounds_scale(&w.window),
                ));
            }
            if let Some(splitter) = w.splitter.as_ref() {
                set_native_splitter_bounds(
                    splitter,
                    &w.window,
                    layout.browser_left.saturating_sub(SPLITTER_WIDTH / 2),
                    0,
                    SPLITTER_WIDTH,
                    layout.total_height,
                    true,
                );
            }
        }
        if !was_visible {
            if let Some(toolbar) = w.toolbar_webview.as_ref() {
                let _ = toolbar.set_visible(true);
            }
            if let Some(splitter) = w.splitter.as_ref() {
                set_native_splitter_bounds(
                    splitter,
                    &w.window,
                    layout.browser_left.saturating_sub(SPLITTER_WIDTH / 2),
                    0,
                    SPLITTER_WIDTH,
                    layout.total_height,
                    true,
                );
            }
        }

        let active_id = w.active_tab_id;
        let active_changed = w.visible_tab_id != Some(active_id);
        if geometry_changed || active_changed {
            if let Some(tab) = w.tabs.iter().find(|tab| tab.id == active_id) {
                let _ = tab.webview.set_bounds(bounds(
                    layout.browser_left.saturating_add(BROWSER_DIVIDER_WIDTH),
                    TOOLBAR_HEIGHT,
                    layout
                        .browser_width
                        .saturating_sub(BROWSER_DIVIDER_WIDTH)
                        .max(1),
                    layout.page_height,
                ));
            }
        }
        if active_changed {
            if let Some(previous_id) = w.visible_tab_id.filter(|id| *id != active_id) {
                if let Some(tab) = w.tabs.iter().find(|tab| tab.id == previous_id) {
                    let _ = tab.webview.set_visible(false);
                }
            }
            if let Some(tab) = w.tabs.iter().find(|tab| tab.id == active_id) {
                let _ = tab.webview.set_visible(true);
            }
            w.visible_tab_id = Some(active_id);
        }
        w.last_layout = Some(layout);
    });
}

/// WebView2's child-controller bounds are device pixels. The rest of the
/// workspace intentionally uses the window's physical layout coordinates, so
/// the standalone HTML panel needs its CSS target size converted explicitly on
/// high-DPI Windows displays. Without this conversion, a 340px CSS menu gets
/// only 226px of viewport at 150% scaling and its lower rows are clipped.
fn browser_panel_bounds(layout: WorkspaceLayout, requested_height: u32, scale: f64) -> Rect {
    let scale = scale.max(1.0);
    let browser_left = layout.browser_left.saturating_add(BROWSER_DIVIDER_WIDTH);
    let browser_width = layout
        .browser_width
        .saturating_sub(BROWSER_DIVIDER_WIDTH)
        .max(1);
    let browser_css_width = (browser_width as f64 / scale).floor() as u32;
    let width =
        ((browser_css_width.saturating_sub(16).clamp(1, 340) as f64) * scale).round() as u32;
    let height = ((requested_height.max(1) as f64) * scale).round() as u32;
    let height = height.min(layout.total_height.saturating_sub(68).max(1));
    bounds(
        browser_left.saturating_add(browser_width.saturating_sub(width + 8)),
        68,
        width,
        height,
    )
}

fn panel_bounds_scale(window: &tauri::Window) -> f64 {
    window.scale_factor().unwrap_or(1.0)
}

fn available_width(workspace: &EmbeddedBrowserWorkspace) -> u32 {
    logical_window_size(&workspace.window).0
}
fn clamp_browser_width(requested: u32, total: u32) -> u32 {
    requested.clamp(
        MIN_BROWSER_WIDTH,
        total
            .saturating_sub(MIN_STUDIO_WIDTH)
            .max(MIN_BROWSER_WIDTH),
    )
}
fn logical_window_size(window: &tauri::Window) -> (u32, u32) {
    let scale = window.scale_factor().unwrap_or(1.0);
    let size = window.inner_size().unwrap_or(PhysicalSize::new(1400, 900));
    (
        ((size.width as f64 / scale).round() as u32).max(1),
        ((size.height as f64 / scale).round() as u32).max(1),
    )
}
fn bounds(x: u32, y: u32, width: u32, height: u32) -> Rect {
    Rect {
        position: LogicalPosition::new(x as f64, y as f64).into(),
        size: LogicalSize::new(width.max(1) as f64, height.max(1) as f64).into(),
    }
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

fn browser_visible() -> bool {
    WORKSPACE.with(|slot| {
        slot.try_borrow()
            .ok()
            .and_then(|slot| slot.as_ref().map(|workspace| workspace.lifecycle.visible()))
            .unwrap_or(false)
    })
}

fn focus_active_tab() {
    WORKSPACE.with(|slot| {
        if let Ok(slot) = slot.try_borrow() {
            if let Some(tab) = slot.as_ref().and_then(active_tab) {
                let _ = tab.webview.focus();
            }
        }
    });
}

fn update_tab_title(id: u64, title: String) {
    WORKSPACE.with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut() {
            if let Some(w) = slot.as_mut() {
                if let Some(tab) = w.tabs.iter_mut().find(|tab| tab.id == id) {
                    tab.title = display_title(&title, &tab.url);
                }
            }
        }
    });
    sync_toolbar();
}

fn update_tab_url(id: u64, url: String) {
    WORKSPACE.with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut() {
            if let Some(w) = slot.as_mut() {
                if validate_browser_url(&url, &w.preferences).is_ok() {
                    if let Some(tab) = w.tabs.iter_mut().find(|tab| tab.id == id) {
                        tab.url = url;
                        tab.title = display_title(&tab.title, &tab.url);
                    }
                }
            }
        }
    });
    sync_toolbar();
}

fn parse_selection(
    mut selection: BrowserSelection,
    preferences: &BrowserPreferences,
) -> Option<BrowserSelection> {
    selection.text = bounded(selection.text.trim(), MAX_SELECTION_BYTES);
    selection.url = bounded(&selection.url, 8 * 1024);
    selection.title = bounded(&selection.title, 512);
    (!selection.text.is_empty() && validate_browser_url(&selection.url, preferences).is_ok())
        .then_some(selection)
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

fn open_comment(selection: &BrowserSelection) {
    WORKSPACE.with(|slot| {
        if let Ok(slot) = slot.try_borrow() {
            if let Some(w) = slot.as_ref() {
                evaluate(
                    &w.studio_webview,
                    "window.__studioEmbeddedBrowser?.openComment",
                    selection,
                );
            }
        }
    });
}

fn begin_download(
    preferences: &BrowserPreferences,
    directory: &Path,
    url: &str,
    path: &mut PathBuf,
) -> bool {
    if validate_browser_url(url, preferences).is_err() || fs::create_dir_all(directory).is_err() {
        return false;
    }
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(sanitize_download_filename)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| download_filename(url));
    let destination = unique_download_path(directory, &filename);
    *path = destination.clone();
    WORKSPACE.with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut() {
            if let Some(w) = slot.as_mut() {
                w.downloads.insert(
                    0,
                    BrowserDownload {
                        url: url.to_owned(),
                        path: destination,
                        state: BrowserDownloadState::Downloading,
                    },
                );
                w.downloads.truncate(100);
            }
        }
    });
    show_downloads();
    true
}

fn finish_download(url: &str, path: Option<&Path>, success: bool) {
    let message = WORKSPACE.with(|slot| {
        let Ok(mut slot) = slot.try_borrow_mut() else {
            return None;
        };
        let w = slot.as_mut()?;
        let index = path
            .and_then(|path| {
                w.downloads
                    .iter()
                    .position(|download| download.path == path)
            })
            .or_else(|| {
                w.downloads.iter().position(|download| {
                    download.url == url && download.state == BrowserDownloadState::Downloading
                })
            })?;
        let download = &mut w.downloads[index];
        download.state = if success {
            BrowserDownloadState::Completed
        } else {
            BrowserDownloadState::Failed
        };
        let name = download
            .path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("download");
        Some(format!(
            "{}：{name}",
            if success {
                "Download completed"
            } else {
                "Download failed"
            }
        ))
    });
    if let Some(message) = message {
        notify_current(&message);
    }
    show_downloads();
}

fn toggle_browser_panel() {
    let open = WORKSPACE.with(|slot| {
        slot.try_borrow()
            .ok()
            .and_then(|slot| slot.as_ref().map(|w| w.browser_menu.is_none()))
            .unwrap_or(false)
    });
    if !open {
        close_native_browser_menu();
        return;
    }

    let result = WORKSPACE.with(|slot| {
        let Ok(mut slot) = slot.try_borrow_mut() else {
            return Err("The browser is busy. Try again shortly.".to_owned());
        };
        let w = slot
            .as_mut()
            .ok_or("The browser workspace is not initialized")?;
        if !w.lifecycle.visible() {
            return Ok(());
        }
        w.browser_panel_webview = None;
        w.browser_panel_height = 0;
        w.browser_menu_hover = BrowserMenuHit::None;
        w.browser_menu = Some(create_native_browser_menu(&w.window)?);
        Ok(())
    });
    if let Err(error) = result {
        notify_current(&error);
    }
}

fn close_browser_panel() {
    WORKSPACE.with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut() {
            if let Some(w) = slot.as_mut() {
                w.browser_menu = None;
                w.browser_menu_hover = BrowserMenuHit::None;
                w.browser_panel_webview = None;
                w.browser_panel_height = 0;
            }
        }
    });
}

fn show_browser_info() {
    let info = WORKSPACE.with(|slot| {
        let slot = slot.try_borrow().ok()?; let w = slot.as_ref()?;
        Some(serde_json::json!({
            "profile": w.browser_profile_directory.display().to_string(), "cache": resolved_cache_directory(w).display().to_string(),
            "downloads": w.downloads_directory.display().to_string(),
            "engine": wry::webview_version().unwrap_or_else(|_| "WebView2".to_owned()),
            "tls": "Windows WebView2 uses system TLS validation with no bypass option.",
            "translations": w.translations,
        }))
    });
    if let Some(info) = info {
        if let Err(error) = open_browser_detail_panel(BROWSER_INFO_HEIGHT, "showInfo", &info) {
            notify_current(&error);
        }
    }
}

fn resolved_cache_directory(workspace: &EmbeddedBrowserWorkspace) -> PathBuf {
    [
        workspace
            .browser_profile_directory
            .join("Default")
            .join("Cache"),
        workspace
            .browser_profile_directory
            .join("EBWebView")
            .join("Default")
            .join("Cache"),
        workspace.browser_cache_directory.clone(),
    ]
    .into_iter()
    .find(|path| path.is_dir())
    .unwrap_or_else(|| workspace.browser_cache_directory.clone())
}

fn show_downloads() {
    let downloads = WORKSPACE.with(|slot| {
        let slot = slot.try_borrow().ok()?; let w = slot.as_ref()?;
        let items = w.downloads.iter().map(|download| serde_json::json!({
            "url": download.url, "path": download.path.display().to_string(),
            "name": download.path.file_name().and_then(|name| name.to_str()).unwrap_or("download"),
            "state": match download.state { BrowserDownloadState::Downloading => "Downloading", BrowserDownloadState::Completed => "Completed", BrowserDownloadState::Failed => "Failed" }
        })).collect::<Vec<_>>();
        Some(serde_json::json!({ "items": items, "translations": w.translations }))
    });
    if let Some(downloads) = downloads {
        let item_count = downloads
            .get("items")
            .and_then(|items| items.as_array())
            .map(Vec::len)
            .unwrap_or(0);
        let height = (BROWSER_DOWNLOADS_EMPTY_HEIGHT + item_count.min(4) as u32 * 48).min(360);
        if let Err(error) = open_browser_detail_panel(height, "showDownloads", &downloads) {
            notify_current(&error);
        }
    }
}

fn open_browser_detail_panel<T: Serialize>(
    height: u32,
    method: &str,
    value: &T,
) -> Result<(), String> {
    close_native_browser_menu();
    let value = serde_json::to_string(value).map_err(|error| error.to_string())?;
    let initialization_script = format!(
        "{PANEL_IPC_INIT}\naddEventListener('DOMContentLoaded',()=>window.__embeddedBrowserPanel?.{method}({value}),{{once:true}});"
    );
    WORKSPACE.with(|slot| {
        let Ok(mut slot) = slot.try_borrow_mut() else {
            return Err("The browser is busy. Try again shortly.".to_owned());
        };
        let w = slot
            .as_mut()
            .ok_or("The browser workspace is not initialized")?;
        if !w.lifecycle.visible() {
            return Ok(());
        }
        w.browser_panel_webview = None;
        w.browser_panel_height = height;
        let panel_window = w.window.clone();
        let context = w
            .toolbar_context
            .as_mut()
            .ok_or("Browser Toolbar is unavailable")?;
        let panel = WebViewBuilder::with_web_context(context)
            .with_html(BROWSER_PANEL_HTML)
            .with_initialization_script(&initialization_script)
            .with_ipc_handler(move |request| handle_browser_panel_ipc(&panel_window, request))
            .with_navigation_handler(intercept_studio_navigation)
            .with_bounds(browser_panel_bounds(
                w.last_layout.unwrap_or_else(|| WorkspaceLayout {
                    visible: true,
                    total_width: logical_window_size(&w.window).0,
                    total_height: logical_window_size(&w.window).1,
                    browser_left: 0,
                    browser_width: w.browser_width,
                    page_height: 1,
                }),
                height,
                panel_bounds_scale(&w.window),
            ))
            .with_visible(true)
            .build_as_child(&w.window)
            .map_err(|error| format!("Unable to create the Browser detail panel: {error:?}"))?;
        w.browser_panel_webview = Some(panel);
        Ok(())
    })
}

fn set_browser_panel_height(height: u32) {
    WORKSPACE.with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut() {
            if let Some(w) = slot.as_mut() {
                if w.browser_panel_webview.is_none() || w.browser_panel_height == height {
                    return;
                }
                w.browser_panel_height = height;
                if let Some(layout) = w.last_layout {
                    if let Some(panel) = w.browser_panel_webview.as_ref() {
                        let _ = panel.set_bounds(browser_panel_bounds(
                            layout,
                            height,
                            panel_bounds_scale(&w.window),
                        ));
                    }
                }
            }
        }
    });
}

fn open_downloads_directory() {
    let directory = WORKSPACE.with(|slot| {
        slot.try_borrow()
            .ok()
            .and_then(|slot| slot.as_ref().map(|w| w.downloads_directory.clone()))
    });
    let Some(directory) = directory else { return };
    if let Err(error) = fs::create_dir_all(&directory).and_then(|_| {
        Command::new("explorer.exe")
            .arg(&directory)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(|_| ())
    }) {
        notify_current(&format!("Unable to open the downloads folder: {error}"));
    }
}

fn sync_toolbar() {
    WORKSPACE.with(|slot| if let Ok(slot) = slot.try_borrow() { if let Some(w) = slot.as_ref() { if let Some(toolbar) = w.toolbar_webview.as_ref() {
        let active = active_tab(w);
        let tabs = w.tabs.iter().map(|tab| serde_json::json!({"id":tab.id,"title":display_title(&tab.title,&tab.url),"url":tab.url,"active":tab.id==w.active_tab_id})).collect::<Vec<_>>();
        let state = serde_json::json!({
            "tabs": tabs, "url": active.map(|tab| tab.url.as_str()).unwrap_or(DEFAULT_BROWSER_URL),
            "title": active.map(|tab| display_title(&tab.title, &tab.url)).unwrap_or_else(|| "Browser".to_owned()),
            "zoomPercent": active.map(|tab| (tab.zoom * 100.0).round() as u32).unwrap_or(100), "fitWidth": active.is_some_and(|tab| tab.fit_width),
            "translations": w.translations,
        });
        evaluate(toolbar, "window.__embeddedBrowserToolbar?.setState", &state);
        if let Some(panel) = w.browser_panel_webview.as_ref() {
            evaluate(panel, "window.__embeddedBrowserPanel?.setState", &state);
        }
        if let Some(menu) = w.browser_menu.as_ref() {
            unsafe {
                let _ = InvalidateRect(menu.hwnd, std::ptr::null(), 0);
            }
        }
    }}});
}

fn sync_studio_width() {
    WORKSPACE.with(|slot| {
        if let Ok(slot) = slot.try_borrow() {
            if let Some(w) = slot.as_ref() {
                evaluate(
                    &w.studio_webview,
                    "window.__studioEmbeddedBrowser?.setWidth",
                    &w.browser_width,
                );
            }
        }
    });
}
fn sync_studio_state() {
    WORKSPACE.with(|slot| {
        if let Ok(slot) = slot.try_borrow() {
            if let Some(w) = slot.as_ref() {
                evaluate(
                    &w.studio_webview,
                    "window.__studioEmbeddedBrowser?.setVisible",
                    &w.lifecycle.visible(),
                );
                evaluate(
                    &w.studio_webview,
                    "window.__studioEmbeddedBrowser?.setRuntimeLoaded",
                    &w.lifecycle.runtime_loaded(),
                );
            }
        }
    });
}
fn notify_current(message: &str) {
    WORKSPACE.with(|slot| {
        if let Ok(slot) = slot.try_borrow() {
            if let Some(w) = slot.as_ref() {
                notify(&w.studio_webview, message);
            }
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

fn profile_directory(name: &str) -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(env::temp_dir)
        .join("codex-thread-studio")
        .join(name)
}
fn download_directory() -> PathBuf {
    if let Some(path) = env::var_os("CODEX_THREAD_STUDIO_DOWNLOAD_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
    {
        return path;
    }
    dirs::download_dir().unwrap_or_else(|| env::temp_dir().join("codex-thread-studio-downloads"))
}

fn deny_page_permissions(webview: &WebView) -> Result<(), String> {
    use webview2_com::{
        Microsoft::Web::WebView2::Win32::COREWEBVIEW2_PERMISSION_STATE_DENY,
        PermissionRequestedEventHandler,
    };
    use wry::WebViewExtWindows;
    let controller = webview.controller();
    let core = unsafe { controller.CoreWebView2() }.map_err(|e| e.to_string())?;
    unsafe {
        core.add_PermissionRequested(
            &PermissionRequestedEventHandler::create(Box::new(|_, args| {
                if let Some(args) = args {
                    args.SetState(COREWEBVIEW2_PERMISSION_STATE_DENY)?;
                }
                Ok(())
            })),
            &mut Default::default(),
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn bind_page_failure_handler(
    webview: &WebView,
    id: u64,
    window: &tauri::Window,
) -> Result<(), String> {
    use webview2_com::{
        Microsoft::Web::WebView2::Win32::COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED,
        ProcessFailedEventHandler,
    };
    use wry::WebViewExtWindows;

    let controller = webview.controller();
    let window = window.clone();
    let core = unsafe { controller.CoreWebView2() }.map_err(|e| e.to_string())?;
    unsafe {
        core.add_ProcessFailed(
            &ProcessFailedEventHandler::create(Box::new(move |_, args| {
                if let Some(args) = args {
                    let mut kind = Default::default();
                    args.ProcessFailedKind(&mut kind)?;
                    if kind == COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED {
                        queue_browser_action(&window, move || recover_tab(id));
                    }
                }
                Ok(())
            })),
            &mut Default::default(),
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn bind_tab_shortcuts(webview: &WebView, window: &tauri::Window) -> Result<(), String> {
    use webview2_com::{
        AcceleratorKeyPressedEventHandler,
        Microsoft::Web::WebView2::Win32::{
            COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN, COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN,
        },
    };
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_SHIFT};
    use wry::WebViewExtWindows;

    let controller = webview.controller();
    let window = window.clone();
    unsafe {
        controller
            .add_AcceleratorKeyPressed(
                &AcceleratorKeyPressedEventHandler::create(Box::new(move |_, args| {
                    let Some(args) = args else { return Ok(()) };
                    let mut kind = Default::default();
                    args.KeyEventKind(&mut kind)?;
                    if kind != COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN
                        && kind != COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN
                    {
                        return Ok(());
                    }
                    if GetKeyState(VK_CONTROL as i32) >= 0 {
                        return Ok(());
                    }
                    let mut key = 0;
                    args.VirtualKey(&mut key)?;
                    let action = match key {
                        0x54 => Some(ToolbarAction::NewTab(None)),
                        0x57 => WORKSPACE.with(|slot| {
                            slot.try_borrow().ok().and_then(|slot| {
                                slot.as_ref().map(|workspace| {
                                    ToolbarAction::CloseTab(workspace.active_tab_id)
                                })
                            })
                        }),
                        0x09 if GetKeyState(VK_SHIFT as i32) < 0 => {
                            Some(ToolbarAction::PreviousTab)
                        }
                        0x09 => Some(ToolbarAction::NextTab),
                        _ => None,
                    };
                    if let Some(action) = action {
                        args.SetHandled(true)?;
                        queue_browser_action(&window, move || dispatch_toolbar_action(action));
                    }
                    Ok(())
                })),
                &mut Default::default(),
            )
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn recover_tab(id: u64) {
    let recovery = WORKSPACE.with(|slot| {
        let Ok(mut slot) = slot.try_borrow_mut() else {
            return None;
        };
        let workspace = slot.as_mut()?;
        let url = workspace.tabs.iter().find(|tab| tab.id == id)?.url.clone();
        let attempts = workspace
            .recent_recoveries
            .entry(id)
            .or_insert_with(|| (0, std::time::Instant::now()));
        if attempts.0 > 0 && attempts.1.elapsed() < std::time::Duration::from_secs(30) {
            return Some(Err(()));
        }
        *attempts = (attempts.0 + 1, std::time::Instant::now());
        Some(Ok(url))
    });
    match recovery {
        Some(Ok(url)) => {
            WORKSPACE.with(|slot| {
                if let Ok(mut slot) = slot.try_borrow_mut() {
                    if let Some(workspace) = slot.as_mut() {
                        if let Some(index) = workspace.tabs.iter().position(|tab| tab.id == id) {
                            workspace.tabs.remove(index);
                            if let Ok(tab) = build_browser_tab(workspace, id, &url) {
                                workspace.tabs.insert(index, tab);
                                workspace.active_tab_id = id;
                            }
                        }
                    }
                }
            });
            layout_workspace();
            sync_toolbar();
        }
        Some(Err(())) => {
            notify_current("The browser page process failed repeatedly; automatic recovery stopped")
        }
        None => {}
    }
}
