#![cfg(all(target_os = "linux", debug_assertions))]

use std::cell::RefCell;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const SOCKET_NAME: &str = "codex-thread-studio-dev-capture.sock";
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(8);
const MAX_MESSAGE_BYTES: u64 = 16 * 1024;

thread_local! {
    static TAURI_WEBVIEW: RefCell<Option<webkit2gtk::WebView>> = const { RefCell::new(None) };
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "command", rename_all = "kebab-case")]
enum DeveloperRequest {
    Screenshot,
    ShowBrowser,
    HideBrowser,
    ExitBrowser,
    NewBrowserTab { url: String },
    ShowBrowserMenu,
    ShowBrowserInfo,
    ShowBrowserDownloads,
    CrashBrowserTab,
    OpenBrowser { url: String },
    OpenArtifact { root: String, path: String },
    OpenWorkspace { root: String, tool: String },
    OpenResources { root: String },
    OpenEnvironmentSettings { root: Option<String> },
    Click { selector: String },
    Drag { selector: String, delta: [i32; 2] },
    Input { selector: String, value: String },
}

#[derive(Debug, Deserialize, Serialize)]
struct DeveloperResponse {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

pub fn requested(arguments: &[OsString]) -> bool {
    arguments.first().is_some_and(|argument| {
        matches!(
            argument.to_str(),
            Some(
                "--dev-screenshot"
                    | "--dev-show-browser"
                    | "--dev-hide-browser"
                    | "--dev-exit-browser"
                    | "--dev-new-browser-tab"
                    | "--dev-show-browser-menu"
                    | "--dev-show-browser-info"
                    | "--dev-show-browser-downloads"
                    | "--dev-crash-browser-tab"
                    | "--dev-open-browser"
                    | "--dev-open-artifact"
                    | "--dev-open-workspace"
                    | "--dev-open-resources"
                    | "--dev-open-environment-settings"
                    | "--dev-click"
                    | "--dev-drag"
                    | "--dev-input"
            )
        )
    })
}

pub fn run_cli(arguments: &[OsString]) -> Result<bool, String> {
    let Some(command) = parse_cli_request(arguments)? else {
        return Ok(false);
    };
    let response = send_request(&command)?;
    if !response.ok {
        return Err(response
            .error
            .unwrap_or_else(|| "Studio rejected the developer command".to_owned()));
    }
    if let Some(path) = response.path {
        println!("{path}");
    } else {
        println!("ok");
    }
    Ok(true)
}

pub fn start_server() -> Result<(), String> {
    let socket = socket_path()?;
    remove_stale_socket(&socket)?;
    let listener = UnixListener::bind(&socket)
        .map_err(|error| format!("unable to bind {}: {error}", socket.display()))?;
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("unable to protect {}: {error}", socket.display()))?;

    thread::Builder::new()
        .name("studio-dev-control".to_owned())
        .spawn(move || serve(listener))
        .map_err(|error| error.to_string())?;
    eprintln!("Developer control socket: {}", socket.display());
    Ok(())
}

pub fn register_tauri_window(window: &tauri::WebviewWindow) -> Result<(), String> {
    window
        .with_webview(|webview| {
            TAURI_WEBVIEW.with(|slot| slot.replace(Some(webview.inner())));
        })
        .map_err(|error| error.to_string())
}

fn parse_cli_request(arguments: &[OsString]) -> Result<Option<DeveloperRequest>, String> {
    let Some(command) = arguments.first().and_then(|argument| argument.to_str()) else {
        return Ok(None);
    };
    match command {
        "--dev-screenshot" if arguments.len() == 1 => Ok(Some(DeveloperRequest::Screenshot)),
        "--dev-show-browser" if arguments.len() == 1 => Ok(Some(DeveloperRequest::ShowBrowser)),
        "--dev-hide-browser" if arguments.len() == 1 => Ok(Some(DeveloperRequest::HideBrowser)),
        "--dev-exit-browser" if arguments.len() == 1 => Ok(Some(DeveloperRequest::ExitBrowser)),
        "--dev-new-browser-tab" if arguments.len() == 2 => {
            let url = arguments[1]
                .to_str()
                .ok_or("developer browser URL must be valid UTF-8")?;
            Ok(Some(DeveloperRequest::NewBrowserTab {
                url: url.to_owned(),
            }))
        }
        "--dev-show-browser-menu" if arguments.len() == 1 => {
            Ok(Some(DeveloperRequest::ShowBrowserMenu))
        }
        "--dev-show-browser-info" if arguments.len() == 1 => {
            Ok(Some(DeveloperRequest::ShowBrowserInfo))
        }
        "--dev-show-browser-downloads" if arguments.len() == 1 => {
            Ok(Some(DeveloperRequest::ShowBrowserDownloads))
        }
        "--dev-crash-browser-tab" if arguments.len() == 1 => {
            Ok(Some(DeveloperRequest::CrashBrowserTab))
        }
        "--dev-open-browser" if arguments.len() == 2 => {
            let url = arguments[1]
                .to_str()
                .ok_or("developer browser URL must be valid UTF-8")?;
            Ok(Some(DeveloperRequest::OpenBrowser {
                url: url.to_owned(),
            }))
        }
        "--dev-open-artifact" if arguments.len() == 3 => {
            let root = arguments[1]
                .to_str()
                .ok_or("developer artifact root must be valid UTF-8")?;
            let path = arguments[2]
                .to_str()
                .ok_or("developer artifact path must be valid UTF-8")?;
            Ok(Some(DeveloperRequest::OpenArtifact {
                root: root.to_owned(),
                path: path.to_owned(),
            }))
        }
        "--dev-open-workspace" if arguments.len() == 3 => {
            let root = arguments[1]
                .to_str()
                .ok_or("developer workspace root must be valid UTF-8")?;
            let tool = arguments[2]
                .to_str()
                .ok_or("developer workspace tool must be valid UTF-8")?;
            if !matches!(tool, "files" | "terminal" | "review") {
                return Err(
                    "developer workspace tool must be files, terminal, or review".to_owned(),
                );
            }
            Ok(Some(DeveloperRequest::OpenWorkspace {
                root: root.to_owned(),
                tool: tool.to_owned(),
            }))
        }
        "--dev-open-resources" if arguments.len() == 2 => {
            let root = arguments[1]
                .to_str()
                .ok_or("developer resources root must be valid UTF-8")?;
            Ok(Some(DeveloperRequest::OpenResources {
                root: root.to_owned(),
            }))
        }
        "--dev-open-environment-settings" if arguments.len() <= 2 => {
            let root = arguments
                .get(1)
                .map(|value| {
                    value
                        .to_str()
                        .map(str::to_owned)
                        .ok_or("developer environment root must be valid UTF-8")
                })
                .transpose()?;
            Ok(Some(DeveloperRequest::OpenEnvironmentSettings { root }))
        }
        "--dev-click" if arguments.len() == 2 => {
            let selector = arguments[1]
                .to_str()
                .ok_or("developer click selector must be valid UTF-8")?;
            if selector.is_empty() || selector.len() > 512 || selector.contains(['\n', '\r', '\0'])
            {
                return Err("developer click selector is invalid".to_owned());
            }
            Ok(Some(DeveloperRequest::Click {
                selector: selector.to_owned(),
            }))
        }
        "--dev-drag" if arguments.len() == 4 => {
            let selector = arguments[1]
                .to_str()
                .ok_or("developer drag selector must be valid UTF-8")?;
            let delta_x = arguments[2]
                .to_str()
                .and_then(|value| value.parse::<i32>().ok())
                .ok_or("developer drag delta-x must be an integer")?;
            let delta_y = arguments[3]
                .to_str()
                .and_then(|value| value.parse::<i32>().ok())
                .ok_or("developer drag delta-y must be an integer")?;
            if selector.is_empty()
                || selector.len() > 512
                || selector.contains(['\n', '\r', '\0'])
                || delta_x.unsigned_abs() > 4_096
                || delta_y.unsigned_abs() > 4_096
            {
                return Err("developer drag is invalid".to_owned());
            }
            Ok(Some(DeveloperRequest::Drag {
                selector: selector.to_owned(),
                delta: [delta_x, delta_y],
            }))
        }
        "--dev-input" if arguments.len() == 3 => {
            let selector = arguments[1]
                .to_str()
                .ok_or("developer input selector must be valid UTF-8")?;
            let value = arguments[2]
                .to_str()
                .ok_or("developer input value must be valid UTF-8")?;
            if selector.is_empty()
                || selector.len() > 512
                || selector.contains(['\n', '\r', '\0'])
                || value.len() > 16 * 1024
                || value.contains('\0')
            {
                return Err("developer input is invalid".to_owned());
            }
            Ok(Some(DeveloperRequest::Input {
                selector: selector.to_owned(),
                value: value.to_owned(),
            }))
        }
        "--dev-screenshot"
        | "--dev-show-browser"
        | "--dev-hide-browser"
        | "--dev-exit-browser"
        | "--dev-show-browser-menu"
        | "--dev-show-browser-info"
        | "--dev-show-browser-downloads"
        | "--dev-crash-browser-tab" => {
            Err(format!("{command} does not accept additional arguments"))
        }
        "--dev-open-browser" | "--dev-new-browser-tab" => {
            Err(format!("{command} requires exactly one URL"))
        }
        "--dev-open-artifact" => Err(format!(
            "{command} requires a project root and project-relative file path"
        )),
        "--dev-open-workspace" => Err(format!(
            "{command} requires a project root and one of files, terminal, or review"
        )),
        "--dev-open-resources" => Err(format!("{command} requires a project root")),
        "--dev-open-environment-settings" => {
            Err(format!("{command} accepts at most one project root"))
        }
        "--dev-click" => Err(format!("{command} requires one CSS selector")),
        "--dev-drag" => Err(format!(
            "{command} requires one CSS selector, delta-x, and delta-y"
        )),
        "--dev-input" => Err(format!("{command} requires a CSS selector and value")),
        _ => Ok(None),
    }
}

fn send_request(request: &DeveloperRequest) -> Result<DeveloperResponse, String> {
    let socket = socket_path()?;
    let mut stream = UnixStream::connect(&socket).map_err(|error| match error.kind() {
        std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::NotFound => format!(
            "Studio development control is not running. Start a Linux debug build of Studio before using developer commands ({})",
            socket.display()
        ),
        _ => format!("unable to connect to {}: {error}", socket.display()),
    })?;
    stream
        .set_read_timeout(Some(RESPONSE_TIMEOUT))
        .map_err(|error| error.to_string())?;
    let payload = serde_json::to_vec(request).map_err(|error| error.to_string())?;
    stream
        .write_all(&payload)
        .map_err(|error| error.to_string())?;
    stream
        .shutdown(Shutdown::Write)
        .map_err(|error| error.to_string())?;

    let mut response = Vec::new();
    stream
        .take(MAX_MESSAGE_BYTES)
        .read_to_end(&mut response)
        .map_err(|error| error.to_string())?;
    serde_json::from_slice(&response)
        .map_err(|error| format!("Studio returned an invalid developer response: {error}"))
}

fn serve(listener: UnixListener) {
    for connection in listener.incoming() {
        let mut stream = match connection {
            Ok(stream) => stream,
            Err(error) => {
                eprintln!("Developer control listener failed: {error}");
                break;
            }
        };
        let response = match handle_request(&mut stream) {
            Ok(path) => DeveloperResponse {
                ok: true,
                path: path.map(|path| path.display().to_string()),
                error: None,
            },
            Err(error) => DeveloperResponse {
                ok: false,
                path: None,
                error: Some(error),
            },
        };
        if let Ok(response) = serde_json::to_vec(&response) {
            let _ = stream.write_all(&response);
        }
    }
}

fn handle_request(stream: &mut UnixStream) -> Result<Option<PathBuf>, String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| error.to_string())?;
    let mut request = Vec::new();
    stream
        .take(MAX_MESSAGE_BYTES)
        .read_to_end(&mut request)
        .map_err(|error| error.to_string())?;
    let request: DeveloperRequest = serde_json::from_slice(&request)
        .map_err(|error| format!("invalid developer command: {error}"))?;

    let (sender, receiver) = mpsc::sync_channel(1);
    gtk::glib::MainContext::default().invoke(move || {
        if matches!(request, DeveloperRequest::Screenshot) {
            match next_screenshot_path() {
                Ok(output) => {
                    let response_path = output.clone();
                    capture_studio_screenshot(output, move |result| {
                        let _ = sender.send(result.map(|_| Some(response_path)));
                    });
                }
                Err(error) => {
                    let _ = sender.send(Err(error));
                }
            }
            return;
        }
        let result = match request {
            DeveloperRequest::Screenshot => unreachable!("screenshot handled asynchronously"),
            DeveloperRequest::ShowBrowser => {
                crate::embedded_browser::show_for_debug(None).map(|_| None)
            }
            DeveloperRequest::HideBrowser => {
                crate::embedded_browser::hide_for_debug().map(|_| None)
            }
            DeveloperRequest::ExitBrowser => {
                crate::embedded_browser::exit_for_debug().map(|_| None)
            }
            DeveloperRequest::NewBrowserTab { url } => {
                crate::embedded_browser::new_tab_for_debug(url).map(|_| None)
            }
            DeveloperRequest::ShowBrowserMenu => {
                crate::embedded_browser::show_menu_for_debug().map(|_| None)
            }
            DeveloperRequest::ShowBrowserInfo => {
                crate::embedded_browser::show_info_for_debug().map(|_| None)
            }
            DeveloperRequest::ShowBrowserDownloads => {
                crate::embedded_browser::show_downloads_for_debug().map(|_| None)
            }
            DeveloperRequest::CrashBrowserTab => {
                crate::embedded_browser::crash_active_tab_for_debug().map(|_| None)
            }
            DeveloperRequest::OpenBrowser { url } => {
                crate::embedded_browser::show_for_debug(Some(url)).map(|_| None)
            }
            DeveloperRequest::OpenArtifact { root, path } => {
                open_artifact_for_debug(root, path).map(|_| None)
            }
            DeveloperRequest::OpenWorkspace { root, tool } => {
                evaluate_studio_developer_call("openWorkspace", serde_json::json!([root, tool]))
                    .map(|_| None)
            }
            DeveloperRequest::OpenResources { root } => {
                evaluate_studio_developer_call("openResources", serde_json::json!([root]))
                    .map(|_| None)
            }
            DeveloperRequest::OpenEnvironmentSettings { root } => evaluate_studio_developer_call(
                "openEnvironmentSettings",
                serde_json::json!([root.unwrap_or_default()]),
            )
            .map(|_| None),
            DeveloperRequest::Click { selector } => {
                evaluate_studio_developer_call("click", serde_json::json!([selector])).map(|_| None)
            }
            DeveloperRequest::Drag { selector, delta } => evaluate_studio_developer_call(
                "drag",
                serde_json::json!([selector, delta[0], delta[1]]),
            )
            .map(|_| None),
            DeveloperRequest::Input { selector, value } => {
                evaluate_studio_developer_call("input", serde_json::json!([selector, value]))
                    .map(|_| None)
            }
        };
        let _ = sender.send(result);
    });
    receiver
        .recv_timeout(RESPONSE_TIMEOUT)
        .map_err(|_| "Studio did not complete the developer command in time".to_owned())?
}

fn open_artifact_for_debug(root: String, path: String) -> Result<(), String> {
    let payload = serde_json::to_string(&serde_json::json!({ "root": root, "path": path }))
        .map_err(|error| error.to_string())?;
    let script = format!("window.__studioDeveloper?.openArtifact?.({payload})");
    let handled = TAURI_WEBVIEW.with(|slot| {
        let slot = slot
            .try_borrow()
            .map_err(|_| "Studio window is busy".to_owned())?;
        let Some(webview) = slot.as_ref() else {
            return Ok::<bool, String>(false);
        };
        use webkit2gtk::WebViewExt;
        #[allow(deprecated)]
        webview.run_javascript(&script, None::<&gtk::gio::Cancellable>, |_| {});
        Ok::<bool, String>(true)
    })?;
    if handled {
        Ok(())
    } else {
        crate::embedded_browser::evaluate_studio_for_debug(&script)
    }
}

fn evaluate_studio_developer_call(
    method: &str,
    arguments: serde_json::Value,
) -> Result<(), String> {
    let arguments = arguments
        .as_array()
        .ok_or("developer call arguments must be an array")?
        .iter()
        .map(|value| serde_json::to_string(value).map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?
        .join(",");
    let script = format!("window.__studioDeveloper?.{method}?.({arguments})");
    let handled = TAURI_WEBVIEW.with(|slot| {
        let slot = slot
            .try_borrow()
            .map_err(|_| "Studio window is busy".to_owned())?;
        let Some(webview) = slot.as_ref() else {
            return Ok::<bool, String>(false);
        };
        use webkit2gtk::WebViewExt;
        #[allow(deprecated)]
        webview.run_javascript(&script, None::<&gtk::gio::Cancellable>, |_| {});
        Ok::<bool, String>(true)
    })?;
    if handled {
        Ok(())
    } else {
        crate::embedded_browser::evaluate_studio_for_debug(&script)
    }
}

fn capture_studio_screenshot(path: PathBuf, complete: impl FnOnce(Result<(), String>) + 'static) {
    let webview = TAURI_WEBVIEW.with(|slot| {
        let slot = slot
            .try_borrow()
            .map_err(|_| "Studio window is busy".to_owned())?;
        Ok::<_, String>(slot.clone())
    });
    match webview {
        Err(error) => complete(Err(error)),
        Ok(Some(webview)) => {
            use gtk::prelude::WidgetExt;
            use webkit2gtk::{SnapshotOptions, SnapshotRegion, WebViewExt};
            let allocation = webview.allocation();
            webview.snapshot(
                SnapshotRegion::Visible,
                SnapshotOptions::NONE,
                None::<&gtk::gio::Cancellable>,
                move |result| {
                    let result = result
                        .map_err(|error| error.to_string())
                        .and_then(|surface| {
                            write_snapshot_surface(
                                &surface,
                                &path,
                                allocation.width(),
                                allocation.height(),
                            )
                        });
                    complete(result);
                },
            );
        }
        Ok(None) => crate::embedded_browser::capture_studio_screenshot(path, complete),
    }
}

fn write_snapshot_surface(
    surface: &gtk::cairo::Surface,
    path: &Path,
    width: i32,
    height: i32,
) -> Result<(), String> {
    if width <= 0 || height <= 0 {
        return Err("Studio WebView has no drawable area".to_owned());
    }
    let pixbuf = gtk::gdk::pixbuf_get_from_surface(surface, 0, 0, width, height)
        .ok_or("Studio WebView snapshot could not be converted")?;
    pixbuf
        .savev(path, "png", &[])
        .map_err(|error| error.to_string())?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|error| error.to_string())
}

fn socket_path() -> Result<PathBuf, String> {
    let name =
        env::var("CODEX_THREAD_STUDIO_DEV_SOCKET").unwrap_or_else(|_| SOCKET_NAME.to_owned());
    if name.is_empty()
        || name.len() > 120
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err("CODEX_THREAD_STUDIO_DEV_SOCKET must be a safe socket filename".to_owned());
    }
    socket_path_from(env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from), &name)
}

fn socket_path_from(runtime_directory: Option<PathBuf>, name: &str) -> Result<PathBuf, String> {
    let runtime_directory = runtime_directory
        .filter(|path| path.is_absolute())
        .ok_or("XDG_RUNTIME_DIR is unavailable; developer control is disabled")?;
    Ok(runtime_directory.join(name))
}

fn remove_stale_socket(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    if UnixStream::connect(path).is_ok() {
        return Err(format!(
            "another Studio developer control server is already using {}",
            path.display()
        ));
    }
    fs::remove_file(path)
        .map_err(|error| format!("unable to remove stale {}: {error}", path.display()))
}

fn next_screenshot_path() -> Result<PathBuf, String> {
    let runtime_directory = env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or("XDG_RUNTIME_DIR is unavailable; developer capture is disabled")?;
    let directory = runtime_directory.join("codex-thread-studio-dev-captures");
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
        .map_err(|error| error.to_string())?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    Ok(directory.join(format!(
        "studio-window-{}-{timestamp}.png",
        std::process::id()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn runtime_socket_must_live_in_an_absolute_runtime_directory() {
        assert_eq!(
            socket_path_from(Some(PathBuf::from("/run/user/1000")), SOCKET_NAME).unwrap(),
            PathBuf::from("/run/user/1000").join(SOCKET_NAME)
        );
        assert!(socket_path_from(Some(PathBuf::from("relative")), SOCKET_NAME).is_err());
        assert!(socket_path_from(None, SOCKET_NAME).is_err());
    }

    #[test]
    fn developer_cli_is_explicit_and_validates_url_arity() {
        assert!(matches!(
            parse_cli_request(&arguments(&["--dev-screenshot"])).unwrap(),
            Some(DeveloperRequest::Screenshot)
        ));
        assert!(matches!(
            parse_cli_request(&arguments(&["--dev-show-browser"])).unwrap(),
            Some(DeveloperRequest::ShowBrowser)
        ));
        assert!(matches!(
            parse_cli_request(&arguments(&["--dev-hide-browser"])).unwrap(),
            Some(DeveloperRequest::HideBrowser)
        ));
        assert!(matches!(
            parse_cli_request(&arguments(&["--dev-exit-browser"])).unwrap(),
            Some(DeveloperRequest::ExitBrowser)
        ));
        assert!(matches!(
            parse_cli_request(&arguments(&[
                "--dev-new-browser-tab",
                "https://example.org"
            ]))
            .unwrap(),
            Some(DeveloperRequest::NewBrowserTab { url }) if url == "https://example.org"
        ));
        assert!(matches!(
            parse_cli_request(&arguments(&["--dev-show-browser-menu"])).unwrap(),
            Some(DeveloperRequest::ShowBrowserMenu)
        ));
        assert!(matches!(
            parse_cli_request(&arguments(&["--dev-show-browser-info"])).unwrap(),
            Some(DeveloperRequest::ShowBrowserInfo)
        ));
        assert!(matches!(
            parse_cli_request(&arguments(&["--dev-show-browser-downloads"])).unwrap(),
            Some(DeveloperRequest::ShowBrowserDownloads)
        ));
        assert!(matches!(
            parse_cli_request(&arguments(&["--dev-crash-browser-tab"])).unwrap(),
            Some(DeveloperRequest::CrashBrowserTab)
        ));
        assert!(matches!(
            parse_cli_request(&arguments(&["--dev-open-browser", "https://example.com"])).unwrap(),
            Some(DeveloperRequest::OpenBrowser { url }) if url == "https://example.com"
        ));
        assert!(matches!(
            parse_cli_request(&arguments(&["--dev-open-artifact", "/books", "guide.epub"])).unwrap(),
            Some(DeveloperRequest::OpenArtifact { root, path }) if root == "/books" && path == "guide.epub"
        ));
        assert!(matches!(
            parse_cli_request(&arguments(&["--dev-open-workspace", "/project", "review"])).unwrap(),
            Some(DeveloperRequest::OpenWorkspace { root, tool }) if root == "/project" && tool == "review"
        ));
        assert!(matches!(
            parse_cli_request(&arguments(&["--dev-open-resources", "/project"])).unwrap(),
            Some(DeveloperRequest::OpenResources { root }) if root == "/project"
        ));
        assert!(matches!(
            parse_cli_request(&arguments(&["--dev-input", "[data-pdf-search]", "Git"])).unwrap(),
            Some(DeveloperRequest::Input { selector, value }) if selector == "[data-pdf-search]" && value == "Git"
        ));
        assert!(matches!(
            parse_cli_request(&arguments(&["--dev-drag", ".table-column-resizer", "120", "0"])).unwrap(),
            Some(DeveloperRequest::Drag { selector, delta: [120, 0] }) if selector == ".table-column-resizer"
        ));
        assert!(parse_cli_request(&arguments(&["--dev-input", "input"])).is_err());
        assert!(parse_cli_request(&arguments(&["--dev-click", "bad\nselector"])).is_err());
        assert!(parse_cli_request(&arguments(&["--dev-drag", "selector", "5000", "0"])).is_err());
        assert!(parse_cli_request(&arguments(&["--dev-open-browser"])).is_err());
        assert!(parse_cli_request(&arguments(&["--dev-screenshot", "extra"])).is_err());
        assert!(parse_cli_request(&arguments(&["--normal-option"]))
            .unwrap()
            .is_none());
    }

    #[test]
    fn only_known_developer_commands_claim_the_process() {
        assert!(requested(&arguments(&["--dev-screenshot"])));
        assert!(requested(&arguments(&["--dev-show-browser"])));
        assert!(requested(&arguments(&["--dev-hide-browser"])));
        assert!(requested(&arguments(&["--dev-exit-browser"])));
        assert!(requested(&arguments(&["--dev-new-browser-tab"])));
        assert!(requested(&arguments(&["--dev-show-browser-menu"])));
        assert!(requested(&arguments(&["--dev-show-browser-info"])));
        assert!(requested(&arguments(&["--dev-show-browser-downloads"])));
        assert!(requested(&arguments(&["--dev-crash-browser-tab"])));
        assert!(requested(&arguments(&["--dev-open-browser"])));
        assert!(requested(&arguments(&["--dev-open-artifact"])));
        assert!(requested(&arguments(&["--dev-open-workspace"])));
        assert!(requested(&arguments(&["--dev-open-resources"])));
        assert!(requested(&arguments(&["--dev-open-environment-settings"])));
        assert!(requested(&arguments(&["--dev-click"])));
        assert!(requested(&arguments(&["--dev-drag"])));
        assert!(requested(&arguments(&["--dev-input"])));
        assert!(!requested(&arguments(&["--normal-option"])));
        assert!(!requested(&[]));
    }
}
