use std::env;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::thread;

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::mpsc;

const MAX_INPUT_BYTES: usize = 1024 * 1024;
const MIN_COLUMNS: u16 = 2;
const MIN_ROWS: u16 = 1;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum TerminalControl {
    Start { root: String, cols: u16, rows: u16 },
    Resize { cols: u16, rows: u16 },
    Stop,
}

pub async fn bridge(mut socket: WebSocket) {
    let Some(Ok(Message::Text(message))) = socket.recv().await else {
        return;
    };
    if message.len() > 16 * 1024 {
        send_error(&mut socket, "terminal start request is too large").await;
        return;
    }
    let start = match serde_json::from_str::<TerminalControl>(&message) {
        Ok(TerminalControl::Start { root, cols, rows }) => (root, cols, rows),
        Ok(_) => {
            send_error(
                &mut socket,
                "the first terminal message must start a session",
            )
            .await;
            return;
        }
        Err(error) => {
            send_error(
                &mut socket,
                &format!("invalid terminal start request: {error}"),
            )
            .await;
            return;
        }
    };
    let root = match terminal_root(&start.0) {
        Ok(root) => root,
        Err(error) => {
            send_error(&mut socket, &error).await;
            return;
        }
    };
    let size = pty_size(start.1, start.2);
    let system = native_pty_system();
    let pair = match system.openpty(size) {
        Ok(pair) => pair,
        Err(error) => {
            send_error(&mut socket, &format!("unable to create terminal: {error}")).await;
            return;
        }
    };
    let shell = default_shell();
    let mut command = CommandBuilder::new(&shell);
    command.cwd(&root);
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");
    let mut child = match pair.slave.spawn_command(command) {
        Ok(child) => child,
        Err(error) => {
            send_error(
                &mut socket,
                &format!("unable to start terminal shell: {error}"),
            )
            .await;
            return;
        }
    };
    drop(pair.slave);
    let mut reader = match pair.master.try_clone_reader() {
        Ok(reader) => reader,
        Err(error) => {
            let _ = child.kill();
            send_error(
                &mut socket,
                &format!("unable to read terminal output: {error}"),
            )
            .await;
            return;
        }
    };
    let mut writer = match pair.master.take_writer() {
        Ok(writer) => writer,
        Err(error) => {
            let _ = child.kill();
            send_error(
                &mut socket,
                &format!("unable to open terminal input: {error}"),
            )
            .await;
            return;
        }
    };
    let mut killer = child.clone_killer();
    let (output_tx, mut output_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (exit_tx, mut exit_rx) = mpsc::unbounded_channel::<serde_json::Value>();
    thread::spawn(move || read_output(&mut reader, output_tx));
    thread::spawn(move || {
        let event = match child.wait() {
            Ok(status) => json!({
                "type": "exit",
                "code": status.exit_code(),
                "signal": status.signal(),
            }),
            Err(error) => {
                json!({ "type": "error", "message": format!("terminal wait failed: {error}") })
            }
        };
        let _ = exit_tx.send(event);
    });

    let (mut output, mut input) = socket.split();
    if output
        .send(Message::Text(
            json!({
                "type": "ready",
                "shell": shell_label(&shell),
                "cwd": root.to_string_lossy(),
            })
            .to_string()
            .into(),
        ))
        .await
        .is_err()
    {
        let _ = killer.kill();
        return;
    }

    let mut output_open = true;
    loop {
        tokio::select! {
            browser_message = input.next() => {
                let Some(Ok(browser_message)) = browser_message else { break };
                match browser_message {
                    Message::Binary(bytes) => {
                        if bytes.len() > MAX_INPUT_BYTES {
                            let _ = output.send(Message::Text(json!({ "type": "error", "message": "terminal input is too large" }).to_string().into())).await;
                            continue;
                        }
                        if writer.write_all(&bytes).and_then(|_| writer.flush()).is_err() { break; }
                    }
                    Message::Text(text) => match serde_json::from_str::<TerminalControl>(&text) {
                        Ok(TerminalControl::Resize { cols, rows }) => {
                            if let Err(error) = pair.master.resize(pty_size(cols, rows)) {
                                let _ = output.send(Message::Text(json!({ "type": "error", "message": format!("terminal resize failed: {error}") }).to_string().into())).await;
                            }
                        }
                        Ok(TerminalControl::Stop) => break,
                        Ok(TerminalControl::Start { .. }) => {}
                        Err(error) => {
                            let _ = output.send(Message::Text(json!({ "type": "error", "message": format!("invalid terminal control: {error}") }).to_string().into())).await;
                        }
                    },
                    Message::Ping(value) => {
                        if output.send(Message::Pong(value)).await.is_err() { break; }
                    }
                    Message::Close(_) => break,
                    Message::Pong(_) => {}
                }
            }
            bytes = output_rx.recv(), if output_open => {
                match bytes {
                    Some(bytes) => {
                        if output.send(Message::Binary(bytes.into())).await.is_err() { break; }
                    }
                    None => output_open = false,
                }
            }
            event = exit_rx.recv() => {
                let Some(event) = event else { break };
                let _ = output.send(Message::Text(event.to_string().into())).await;
                break;
            }
        }
    }
    let _ = killer.kill();
    drop(writer);
    drop(pair.master);
}

fn read_output(reader: &mut Box<dyn Read + Send>, output: mpsc::UnboundedSender<Vec<u8>>) {
    let mut buffer = vec![0_u8; 16 * 1024];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(size) if output.send(buffer[..size].to_vec()).is_err() => break,
            Ok(_) => {}
        }
    }
}

fn terminal_root(value: &str) -> Result<PathBuf, String> {
    let root = std::fs::canonicalize(value)
        .map_err(|error| format!("terminal working directory is unavailable: {error}"))?;
    if root.is_dir() {
        Ok(root)
    } else {
        Err("terminal working directory is not a directory".to_string())
    }
}

fn pty_size(cols: u16, rows: u16) -> PtySize {
    PtySize {
        cols: cols.max(MIN_COLUMNS),
        rows: rows.max(MIN_ROWS),
        pixel_width: 0,
        pixel_height: 0,
    }
}

fn default_shell() -> PathBuf {
    #[cfg(windows)]
    {
        PathBuf::from(env::var_os("COMSPEC").unwrap_or_else(|| "powershell.exe".into()))
    }
    #[cfg(not(windows))]
    {
        env::var_os("SHELL")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute() && path.is_file())
            .unwrap_or_else(|| PathBuf::from("/bin/sh"))
    }
}

fn shell_label(shell: &Path) -> String {
    shell
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("shell")
        .to_string()
}

async fn send_error(socket: &mut WebSocket, message: &str) {
    let _ = socket
        .send(Message::Text(
            json!({ "type": "error", "message": message })
                .to_string()
                .into(),
        ))
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::sync::mpsc as std_mpsc;
    #[cfg(unix)]
    use std::time::Duration;

    #[test]
    fn clamps_pty_dimensions_to_valid_values() {
        let size = pty_size(0, 0);
        assert_eq!(size.cols, 2);
        assert_eq!(size.rows, 1);
    }

    #[test]
    fn rejects_non_directory_terminal_roots() {
        let file = std::env::temp_dir().join(format!("terminal-root-{}", uuid::Uuid::new_v4()));
        std::fs::write(&file, "not a directory").expect("fixture");
        assert!(terminal_root(file.to_str().expect("utf8 path")).is_err());
        std::fs::remove_file(file).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn native_pty_runs_the_shell_in_the_requested_directory() {
        let root = std::env::temp_dir().join(format!("studio-pty-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).expect("terminal root");
        let pair = native_pty_system()
            .openpty(PtySize::default())
            .expect("create PTY");
        let mut command = CommandBuilder::new(default_shell());
        command.cwd(&root);
        command.env("TERM", "xterm-256color");
        let mut child = pair.slave.spawn_command(command).expect("start shell");
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().expect("reader");
        let mut writer = pair.master.take_writer().expect("writer");
        let mut killer = child.clone_killer();
        let (output_tx, output_rx) = std_mpsc::channel();
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = reader.read_to_end(&mut bytes);
            let _ = output_tx.send(bytes);
        });
        writer
            .write_all(b"printf '__STUDIO_PTY__:%s\\n' \"$PWD\"; exit\r\n")
            .expect("write command");
        writer.flush().expect("flush command");
        let (exit_tx, exit_rx) = std_mpsc::channel();
        thread::spawn(move || {
            let _ = exit_tx.send(child.wait());
        });
        let status = match exit_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(status) => status.expect("wait for shell"),
            Err(error) => {
                let _ = killer.kill();
                panic!("shell did not exit: {error}");
            }
        };
        assert!(status.success());
        drop(writer);
        drop(pair.master);
        let output = output_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("terminal output");
        let output = String::from_utf8_lossy(&output);
        assert!(output.contains("__STUDIO_PTY__:"));
        assert!(output.contains(&root.to_string_lossy().to_string()));
        std::fs::remove_dir(root).expect("cleanup");
    }
}
