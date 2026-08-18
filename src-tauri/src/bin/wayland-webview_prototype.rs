#[cfg(not(target_os = "linux"))]
compile_error!("The Phase 2A prototype is intentionally Linux/Wayland-only.");

use std::{cell::RefCell, env, error::Error, fs, path::PathBuf, rc::Rc};

use gtk::prelude::*;
use serde::{Deserialize, Serialize};
use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::unix::WindowExtUnix,
    window::WindowBuilder,
};
use url::Url;
use wry::{PageLoadEvent, WebContext, WebView, WebViewBuilder, WebViewBuilderExtUnix};

const MIN_BROWSER_WIDTH: i32 = 380;
const MIN_STUDIO_WIDTH: i32 = 520;
const MAX_SELECTION_BYTES: usize = 16 * 1024;
const INITIAL_URL: &str = "https://example.com";
const SHELL_HTML: &str = include_str!("../../../prototypes/wayland-webview/index.html");
const TOOLBAR_HTML: &str = include_str!("../../../prototypes/wayland-webview/browser-toolbar.html");

#[derive(Debug, Clone)]
enum UserEvent {
    ShellCommand(ShellCommand),
    ToolbarReady,
    BrowserSelectionResult(Option<WebSelection>),
    BrowserTitle(String),
    BrowserLoad { phase: &'static str, url: String },
    BrowserNewWindow(String),
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ShellCommand {
    Navigate { url: String },
    Back,
    Forward,
    Reload,
    OpenSelectionComment,
    ToggleBrowser,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct WebSelection {
    text: String,
    url: String,
    title: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    require_wayland()?;

    // Must be set before Tao initializes GTK. This prototype deliberately has no X11 fallback.
    env::set_var("GDK_BACKEND", "wayland");

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let window = WindowBuilder::new()
        .with_title("Codex Thread Studio · Wayland WebView Prototype")
        .with_inner_size(LogicalSize::new(1440.0, 900.0))
        .with_min_inner_size(LogicalSize::new(920.0, 520.0))
        .build(&event_loop)?;

    let vbox = window
        .default_vbox()
        .ok_or("Tao did not expose its GTK container")?;
    let split = gtk::Paned::new(gtk::Orientation::Horizontal);
    split.set_wide_handle(true);
    split.set_position(820);

    let studio_host = gtk::Box::new(gtk::Orientation::Vertical, 0);
    studio_host.set_size_request(MIN_STUDIO_WIDTH, -1);
    let browser_column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    browser_column.set_size_request(MIN_BROWSER_WIDTH, -1);
    let toolbar_host = gtk::Box::new(gtk::Orientation::Vertical, 0);
    toolbar_host.set_size_request(-1, 58);
    let browser_host = gtk::Box::new(gtk::Orientation::Vertical, 0);

    browser_column.pack_start(&toolbar_host, false, false, 0);
    browser_column.pack_start(&browser_host, true, true, 0);
    split.pack1(&studio_host, true, false);
    split.pack2(&browser_column, true, false);
    vbox.pack_start(&split, true, true, 0);
    split.show_all();

    let shell_action_proxy = proxy.clone();
    let shell_webview = WebViewBuilder::new()
        .with_html(SHELL_HTML)
        .with_clipboard(true)
        .with_navigation_handler(move |url| {
            if let Some(command) = parse_action_url(&url) {
                eprintln!("Studio shell action: {command:?}");
                let _ = shell_action_proxy.send_event(UserEvent::ShellCommand(command));
                false
            } else {
                true
            }
        })
        .build_gtk(&studio_host)?;

    let toolbar_action_proxy = proxy.clone();
    let toolbar_load_proxy = proxy.clone();
    let toolbar_webview = WebViewBuilder::new()
        .with_html(TOOLBAR_HTML)
        .with_clipboard(true)
        .with_navigation_handler(move |url| {
            if let Some(command) = parse_action_url(&url) {
                eprintln!("Browser toolbar action: {command:?}");
                let _ = toolbar_action_proxy.send_event(UserEvent::ShellCommand(command));
                false
            } else {
                true
            }
        })
        .with_on_page_load_handler(move |event, _| {
            if matches!(event, PageLoadEvent::Finished) {
                let _ = toolbar_load_proxy.send_event(UserEvent::ToolbarReady);
            }
        })
        .build_gtk(&toolbar_host)?;

    let profile_dir = browser_profile_dir();
    fs::create_dir_all(&profile_dir)?;
    let mut browser_context = WebContext::new(Some(profile_dir.clone()));

    let title_proxy = proxy.clone();
    let load_proxy = proxy.clone();
    let popup_proxy = proxy.clone();
    let browser_webview = WebViewBuilder::with_web_context(&mut browser_context)
        .with_url(INITIAL_URL)
        .with_clipboard(true)
        .with_document_title_changed_handler(move |title| {
            let _ = title_proxy.send_event(UserEvent::BrowserTitle(truncate_utf8(title, 512)));
        })
        .with_on_page_load_handler(move |event, url| {
            let phase = match event {
                PageLoadEvent::Started => "started",
                PageLoadEvent::Finished => "finished",
            };
            let _ = load_proxy.send_event(UserEvent::BrowserLoad {
                phase,
                url: truncate_utf8(url, 4096),
            });
        })
        .with_new_window_req_handler(move |url| {
            let _ = popup_proxy.send_event(UserEvent::BrowserNewWindow(url));
            false
        })
        .build_gtk(&browser_host)?;
    split.show_all();
    browser_column.hide();

    let current_title = Rc::new(RefCell::new(String::new()));
    let browser_visible = Rc::new(RefCell::new(false));

    eprintln!("Wayland WebView prototype started");
    eprintln!("Persistent browser profile: {}", profile_dir.display());

    event_loop.run(move |event, _, control_flow| {
        // Keeping this value captured is required by WRY while its browser WebView is alive.
        let _keep_context_alive = &browser_context;
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            Event::UserEvent(UserEvent::ToolbarReady) => {
                send_navigation(&toolbar_webview, Some(INITIAL_URL), None, None);
            }
            Event::UserEvent(UserEvent::ShellCommand(command)) => match command {
                ShellCommand::Navigate { url } => match normalize_http_url(&url) {
                    Some(url) => {
                        eprintln!("Browser navigate: {url}");
                        if let Err(error) = browser_webview.load_url(url.as_str()) {
                            eprintln!("Browser navigation failed: {error}");
                            notify(
                                &shell_webview,
                                &format!("Unable to open the web page: {error}"),
                            );
                        } else {
                            let _ = browser_webview.focus();
                        }
                    }
                    None => notify(&shell_webview, "Only http:// and https:// URLs are allowed"),
                },
                ShellCommand::Back => {
                    let _ = browser_webview.evaluate_script("history.back()");
                }
                ShellCommand::Forward => {
                    let _ = browser_webview.evaluate_script("history.forward()");
                }
                ShellCommand::Reload => {
                    let _ = browser_webview.evaluate_script("location.reload()");
                }
                ShellCommand::OpenSelectionComment => {
                    let selection_proxy = proxy.clone();
                    if let Err(error) = browser_webview.evaluate_script_with_callback(
                        READ_SELECTION_SCRIPT,
                        move |value| {
                            let selection = parse_browser_selection(&value);
                            let _ = selection_proxy
                                .send_event(UserEvent::BrowserSelectionResult(selection));
                        },
                    ) {
                        notify(
                            &shell_webview,
                            &format!("Unable to read the web selection: {error}"),
                        );
                    }
                }
                ShellCommand::ToggleBrowser => {
                    let visible = !*browser_visible.borrow();
                    *browser_visible.borrow_mut() = visible;
                    if visible {
                        browser_column.show_all();
                    } else {
                        browser_column.hide();
                    }
                    call_webview(&shell_webview, "setBrowserVisible", &visible);
                }
            },
            Event::UserEvent(UserEvent::BrowserSelectionResult(selection)) => {
                if let Some(selection) = selection {
                    eprintln!(
                        "Browser selection: {} bytes from {}",
                        selection.text.len(),
                        selection.url
                    );
                    call_webview(&shell_webview, "openSelectionComment", &selection);
                } else {
                    notify(&shell_webview, "Select text on the web page first");
                }
            }
            Event::UserEvent(UserEvent::BrowserTitle(title)) => {
                *current_title.borrow_mut() = title.clone();
                send_navigation(&toolbar_webview, None, Some(&title), None);
            }
            Event::UserEvent(UserEvent::BrowserLoad { phase, url }) => {
                send_navigation(
                    &toolbar_webview,
                    Some(&url),
                    Some(current_title.borrow().as_str()),
                    Some(phase),
                );
            }
            Event::UserEvent(UserEvent::BrowserNewWindow(url)) => {
                if let Some(url) = normalize_http_url(&url) {
                    // Phase 2A keeps one native browser surface. Popup requests are safely routed
                    // into it; tab lifecycle is intentionally deferred to the next slice.
                    let _ = browser_webview.load_url(url.as_str());
                    notify(
                        &shell_webview,
                        "The new-window request opened in the current prototype page",
                    );
                }
            }
            _ => {}
        }
    });
}

fn require_wayland() -> Result<(), Box<dyn Error>> {
    if env::var_os("WAYLAND_DISPLAY")
        .filter(|value| !value.is_empty())
        .is_none()
    {
        return Err(
            "Phase 2A requires a native Wayland session (WAYLAND_DISPLAY is missing)".into(),
        );
    }
    Ok(())
}

fn browser_profile_dir() -> PathBuf {
    if let Some(data_home) = env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(data_home)
            .join("codex-thread-studio")
            .join("wayland-webview-profile");
    }
    PathBuf::from(env::var_os("HOME").unwrap_or_else(|| ".".into()))
        .join(".local/share/codex-thread-studio/wayland-webview-profile")
}

#[cfg(test)]
fn parse_shell_command(raw: &str) -> Option<ShellCommand> {
    if raw.len() > 8192 {
        return None;
    }
    serde_json::from_str(raw).ok()
}

fn parse_action_url(raw: &str) -> Option<ShellCommand> {
    let url = Url::parse(raw).ok()?;
    if url.scheme() != "studio-action" {
        return None;
    }
    match url.host_str()? {
        "navigate" => {
            let target = url
                .query_pairs()
                .find_map(|(key, value)| (key == "url").then(|| value.into_owned()))?;
            Some(ShellCommand::Navigate { url: target })
        }
        "back" => Some(ShellCommand::Back),
        "forward" => Some(ShellCommand::Forward),
        "reload" => Some(ShellCommand::Reload),
        "open-selection-comment" => Some(ShellCommand::OpenSelectionComment),
        "toggle-browser" => Some(ShellCommand::ToggleBrowser),
        _ => None,
    }
}

fn parse_browser_selection(raw: &str) -> Option<WebSelection> {
    if raw.len() > MAX_SELECTION_BYTES * 2 {
        return None;
    }
    let mut selection: WebSelection = serde_json::from_str(raw).ok()?;
    selection.text = truncate_utf8(selection.text.trim().to_owned(), MAX_SELECTION_BYTES);
    selection.url = truncate_utf8(selection.url, 4096);
    selection.title = truncate_utf8(selection.title, 512);
    if selection.text.is_empty() || normalize_http_url(&selection.url).is_none() {
        return None;
    }
    Some(selection)
}

fn normalize_http_url(raw: &str) -> Option<Url> {
    let parsed = Url::parse(raw.trim()).ok()?;
    matches!(parsed.scheme(), "http" | "https").then_some(parsed)
}

fn truncate_utf8(value: String, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value;
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_owned()
}

fn send_navigation(webview: &WebView, url: Option<&str>, title: Option<&str>, phase: Option<&str>) {
    let payload = serde_json::json!({ "url": url, "title": title, "phase": phase });
    call_webview(webview, "setNavigation", &payload);
}

fn notify(shell: &WebView, message: &str) {
    call_webview(shell, "notify", &message);
}

fn call_webview<T: Serialize>(webview: &WebView, method: &str, payload: &T) {
    if let Ok(payload) = serde_json::to_string(payload) {
        let script = format!(
            "window.prototype && window.prototype.{method}({payload});",
            method = method,
            payload = payload
        );
        let _ = webview.evaluate_script(&script);
    }
}

const READ_SELECTION_SCRIPT: &str = r#"
(() => ({
  text: (window.getSelection()?.toString() || '').trim().slice(0, 16384),
  url: location.href,
  title: document.title || ''
}))()
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_commands_are_typed_and_bounded() {
        assert_eq!(
            parse_shell_command(r#"{"type":"navigate","url":"https://example.com"}"#),
            Some(ShellCommand::Navigate {
                url: "https://example.com".to_owned()
            })
        );
        assert!(parse_shell_command(r#"{"type":"unknown"}"#).is_none());
        assert!(parse_shell_command(&"x".repeat(9000)).is_none());
        assert_eq!(
            parse_action_url(
                "studio-action://navigate?url=https%3A%2F%2Fexample.com%2Fdocs%3Fa%3D1"
            ),
            Some(ShellCommand::Navigate {
                url: "https://example.com/docs?a=1".to_owned()
            })
        );
        assert_eq!(
            parse_action_url("studio-action://toggle-browser"),
            Some(ShellCommand::ToggleBrowser)
        );
    }

    #[test]
    fn browser_bridge_rejects_privileged_or_non_http_messages() {
        let valid = parse_browser_selection(
            r#"{"text":"selected","url":"https://example.com/a","title":"Example"}"#,
        )
        .unwrap();
        assert_eq!(valid.text, "selected");
        assert!(parse_browser_selection(
            r#"{"text":"secret","url":"file:///etc/passwd","title":"Local"}"#
        )
        .is_none());
        assert!(parse_browser_selection(r#"{"type":"run_shell","command":"rm"}"#).is_none());
    }

    #[test]
    fn truncation_keeps_utf8_valid() {
        assert_eq!(
            truncate_utf8("\u{4e2d}\u{6587}\u{6d4b}\u{8bd5}".to_owned(), 7),
            "\u{4e2d}\u{6587}"
        );
    }
}
