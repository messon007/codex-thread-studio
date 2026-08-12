#![cfg(all(target_os = "linux", debug_assertions))]

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

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "command", rename_all = "kebab-case")]
enum DeveloperRequest {
    Screenshot,
    ShowBrowser,
    OpenBrowser { url: String },
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
            Some("--dev-screenshot" | "--dev-show-browser" | "--dev-open-browser")
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

fn parse_cli_request(arguments: &[OsString]) -> Result<Option<DeveloperRequest>, String> {
    let Some(command) = arguments.first().and_then(|argument| argument.to_str()) else {
        return Ok(None);
    };
    match command {
        "--dev-screenshot" if arguments.len() == 1 => Ok(Some(DeveloperRequest::Screenshot)),
        "--dev-show-browser" if arguments.len() == 1 => Ok(Some(DeveloperRequest::ShowBrowser)),
        "--dev-open-browser" if arguments.len() == 2 => {
            let url = arguments[1]
                .to_str()
                .ok_or("developer browser URL must be valid UTF-8")?;
            Ok(Some(DeveloperRequest::OpenBrowser {
                url: url.to_owned(),
            }))
        }
        "--dev-screenshot" | "--dev-show-browser" => {
            Err(format!("{command} does not accept additional arguments"))
        }
        "--dev-open-browser" => Err("--dev-open-browser requires exactly one URL".to_owned()),
        _ => Ok(None),
    }
}

fn send_request(request: &DeveloperRequest) -> Result<DeveloperResponse, String> {
    let socket = socket_path()?;
    let mut stream = UnixStream::connect(&socket)
        .map_err(|error| format!("unable to connect to {}: {error}", socket.display()))?;
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
        let result = match request {
            DeveloperRequest::Screenshot => next_screenshot_path().and_then(|output| {
                crate::embedded_browser::capture_screenshot(&output)?;
                Ok(Some(output))
            }),
            DeveloperRequest::ShowBrowser => {
                crate::embedded_browser::show_for_debug(None).map(|_| None)
            }
            DeveloperRequest::OpenBrowser { url } => {
                crate::embedded_browser::show_for_debug(Some(url)).map(|_| None)
            }
        };
        let _ = sender.send(result);
    });
    receiver
        .recv_timeout(RESPONSE_TIMEOUT)
        .map_err(|_| "Studio did not complete the developer command in time".to_owned())?
}

fn socket_path() -> Result<PathBuf, String> {
    socket_path_from(env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from))
}

fn socket_path_from(runtime_directory: Option<PathBuf>) -> Result<PathBuf, String> {
    let runtime_directory = runtime_directory
        .filter(|path| path.is_absolute())
        .ok_or("XDG_RUNTIME_DIR is unavailable; developer control is disabled")?;
    Ok(runtime_directory.join(SOCKET_NAME))
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
            socket_path_from(Some(PathBuf::from("/run/user/1000"))).unwrap(),
            PathBuf::from("/run/user/1000").join(SOCKET_NAME)
        );
        assert!(socket_path_from(Some(PathBuf::from("relative"))).is_err());
        assert!(socket_path_from(None).is_err());
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
            parse_cli_request(&arguments(&["--dev-open-browser", "https://example.com"])).unwrap(),
            Some(DeveloperRequest::OpenBrowser { url }) if url == "https://example.com"
        ));
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
        assert!(requested(&arguments(&["--dev-open-browser"])));
        assert!(!requested(&arguments(&["--normal-option"])));
        assert!(!requested(&[]));
    }
}
