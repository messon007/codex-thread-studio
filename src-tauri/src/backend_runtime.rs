use std::ffi::OsString;
use std::io;
use std::process::{ExitStatus, Stdio};

use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};

#[cfg(any(windows, test))]
const WSL_LAUNCH_SCRIPT: &str = r#"pid_file="$1"
binary="$2"
shift 2
mkdir -p /tmp/codex-thread-studio
if [[ "$binary" != */* ]]; then
  resolved=$(command -v -- "$binary" 2>/dev/null || true)
  if [ -z "$resolved" ]; then
    shopt -s nullglob
    for candidate in "$HOME"/.nvm/versions/node/*/bin/"$binary" "$HOME"/.local/share/fnm/node-versions/*/installation/bin/"$binary" "$HOME"/.local/share/fnm/aliases/default/bin/"$binary"; do
      [ -x "$candidate" ] && resolved="$candidate"
    done
  fi
  [ -n "$resolved" ] && binary="$resolved"
fi
if [[ "$binary" == */* ]]; then
  export PATH="$(dirname -- "$binary"):$PATH"
fi
/usr/bin/setsid --fork --wait /bin/bash -c 'pid_file="$1"; binary="$2"; shift 2; environment=(); while [ "$#" -gt 0 ] && [ "$1" != "--" ]; do environment+=("$1"); shift; done; [ "$#" -gt 0 ] && shift; printf "%s" "$$" > "$pid_file"; exec /usr/bin/env "${environment[@]}" "$binary" "$@"' codex-thread-studio-child "$pid_file" "$binary" "$@"
status=$?
rm -f "$pid_file"
exit $status"#;

#[cfg(any(windows, test))]
const WSL_CLEANUP_SCRIPT: &str = r#"pid_file="$1"
if [ -r "$pid_file" ]; then
  pid=$(cat "$pid_file")
  case "$pid" in
    ''|*[!0-9]*) ;;
    *)
      kill -TERM -- "-$pid" 2>/dev/null || true
      attempts=0
      while kill -0 -- "-$pid" 2>/dev/null && [ "$attempts" -lt 20 ]; do
        sleep 0.1
        attempts=$((attempts + 1))
      done
      kill -KILL -- "-$pid" 2>/dev/null || true
      ;;
  esac
  rm -f "$pid_file"
fi"#;

#[cfg(windows)]
const WSL_READ_FILE_SCRIPT: &str = r#"root=$(realpath -e -- "$1") || exit 20
requested=$2
case "$requested" in
  /*) candidate=$requested ;;
  *) candidate=$root/$requested ;;
esac
path=$(realpath -e -- "$candidate") || exit 21
case "$path" in
  "$root"|"$root"/*) ;;
  *) exit 22 ;;
esac
[ -f "$path" ] || exit 23
size=$(stat -c %s -- "$path") || exit 23
[ "$size" -le "$3" ] || exit 24
relative=${path#"$root"/}
printf '%s\0%s\0%s\0%s\0' "$root" "$path" "$relative" "$size"
cat -- "$path""#;

#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Clone, Debug, Default)]
pub struct WslSettings {
    pub distribution: Option<String>,
    pub user: Option<String>,
}

#[derive(Clone, Debug)]
pub struct BackendRuntime {
    path: OsString,
    #[cfg(windows)]
    wsl: WslSettings,
}

#[cfg(windows)]
pub struct RuntimeFile {
    pub root: String,
    pub path: String,
    pub relative_path: String,
    pub content: Vec<u8>,
    pub size: u64,
}

impl BackendRuntime {
    pub fn new(path: OsString, wsl: WslSettings) -> Self {
        #[cfg(not(windows))]
        let _ = wsl;
        Self {
            path,
            #[cfg(windows)]
            wsl,
        }
    }

    pub fn environment(&self) -> &'static str {
        if cfg!(windows) {
            "wsl"
        } else {
            "local"
        }
    }

    pub fn wsl_distribution(&self) -> Option<&str> {
        #[cfg(windows)]
        return self.wsl.distribution.as_deref();
        #[cfg(not(windows))]
        None
    }

    #[cfg(windows)]
    pub async fn read_wsl_file(
        &self,
        root: &str,
        path: &str,
        max_bytes: u64,
    ) -> io::Result<RuntimeFile> {
        if root.contains('\0') || path.contains('\0') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "WSL paths cannot contain NUL characters",
            ));
        }
        let launcher = std::env::var_os("CODEX_THREAD_STUDIO_WSL_BIN")
            .unwrap_or_else(|| OsString::from("wsl.exe"));
        let mut args = wsl_base_args(self.wsl.distribution.as_deref(), self.wsl.user.as_deref());
        args.extend([
            "--exec".to_string(),
            "/bin/bash".to_string(),
            "-c".to_string(),
            WSL_READ_FILE_SCRIPT.to_string(),
            "codex-thread-studio-read".to_string(),
            root.to_string(),
            path.to_string(),
            max_bytes.to_string(),
        ]);
        let output = Command::new(launcher)
            .args(args)
            .env("PATH", &self.path)
            .output()
            .await?;
        if !output.status.success() {
            return Err(io::Error::new(
                match output.status.code() {
                    Some(22) => io::ErrorKind::PermissionDenied,
                    Some(21 | 23) => io::ErrorKind::NotFound,
                    Some(24) => io::ErrorKind::FileTooLarge,
                    _ => io::ErrorKind::InvalidInput,
                },
                format!("WSL file inspection failed with {}", output.status),
            ));
        }
        parse_runtime_file(output.stdout)
    }

    pub fn command(
        &self,
        binary: &str,
        args: &[&str],
        environment: &[(&str, &str)],
    ) -> RuntimeCommand {
        #[cfg(windows)]
        {
            let pid_file = format!(
                "/tmp/codex-thread-studio/{}.pid",
                uuid::Uuid::new_v4().simple()
            );
            let launcher = std::env::var_os("CODEX_THREAD_STUDIO_WSL_BIN")
                .unwrap_or_else(|| OsString::from("wsl.exe"));
            let launch_args = wsl_launch_args(
                self.wsl.distribution.as_deref(),
                self.wsl.user.as_deref(),
                &pid_file,
                binary,
                args,
                environment,
            );
            let mut command = Command::new(&launcher);
            command.args(launch_args).env("PATH", &self.path);
            RuntimeCommand {
                command,
                cleanup: Some(WslCleanup {
                    launcher,
                    path: self.path.clone(),
                    settings: self.wsl.clone(),
                    pid_file,
                    armed: true,
                }),
            }
        }
        #[cfg(not(windows))]
        {
            let mut command = Command::new(binary);
            command.args(args).env("PATH", &self.path);
            for (key, value) in environment {
                command.env(key, value);
            }
            RuntimeCommand {
                command,
                cleanup: None,
            }
        }
    }
}

#[cfg(windows)]
fn parse_runtime_file(bytes: Vec<u8>) -> io::Result<RuntimeFile> {
    let mut separators = bytes
        .iter()
        .enumerate()
        .filter_map(|(index, byte)| (*byte == 0).then_some(index));
    let boundaries = [
        separators.next(),
        separators.next(),
        separators.next(),
        separators.next(),
    ];
    let [Some(root_end), Some(path_end), Some(relative_end), Some(size_end)] = boundaries else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "WSL file response is incomplete",
        ));
    };
    let text = |start, end| {
        std::str::from_utf8(&bytes[start..end])
            .map(str::to_string)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    };
    let size = text(relative_end + 1, size_end)?
        .parse::<u64>()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(RuntimeFile {
        root: text(0, root_end)?,
        path: text(root_end + 1, path_end)?,
        relative_path: text(path_end + 1, relative_end)?,
        content: bytes[size_end + 1..].to_vec(),
        size,
    })
}

pub struct RuntimeCommand {
    command: Command,
    cleanup: Option<WslCleanup>,
}

impl RuntimeCommand {
    pub fn stdin(&mut self, configuration: Stdio) -> &mut Self {
        self.command.stdin(configuration);
        self
    }

    pub fn stdout(&mut self, configuration: Stdio) -> &mut Self {
        self.command.stdout(configuration);
        self
    }

    pub fn stderr(&mut self, configuration: Stdio) -> &mut Self {
        self.command.stderr(configuration);
        self
    }

    pub fn spawn(mut self) -> io::Result<RuntimeChild> {
        self.command.kill_on_drop(true);
        let child = self.command.spawn()?;
        Ok(RuntimeChild {
            child,
            cleanup: self.cleanup,
        })
    }
}

pub struct RuntimeChild {
    child: Child,
    cleanup: Option<WslCleanup>,
}

impl RuntimeChild {
    pub fn take_stdin(&mut self) -> Option<ChildStdin> {
        self.child.stdin.take()
    }

    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.stdout.take()
    }

    pub fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child.stderr.take()
    }

    pub async fn wait(&mut self) -> io::Result<ExitStatus> {
        let result = self.child.wait().await;
        if result.is_ok() {
            if let Some(cleanup) = self.cleanup.as_mut() {
                let _ = cleanup.terminate().await;
            }
        }
        result
    }

    pub async fn kill_tree(&mut self) -> io::Result<()> {
        let cleanup_result = match self.cleanup.as_mut() {
            Some(cleanup) => cleanup.terminate().await,
            None => Ok(()),
        };
        let kill_result = self.child.start_kill();
        let wait_result = self.child.wait().await.map(|_| ());
        cleanup_result.and(kill_result).and(wait_result)
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
struct WslCleanup {
    launcher: OsString,
    path: OsString,
    settings: WslSettings,
    pid_file: String,
    armed: bool,
}

impl WslCleanup {
    #[cfg(windows)]
    async fn terminate(&mut self) -> io::Result<()> {
        if !self.armed {
            return Ok(());
        }
        self.armed = false;
        let mut command = self.command();
        command.kill_on_drop(true);
        let mut child = command.spawn()?;
        match tokio::time::timeout(std::time::Duration::from_secs(4), child.wait()).await {
            Ok(result) => result.map(|_| ()),
            Err(_) => {
                let _ = child.start_kill();
                Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "timed out while stopping the WSL backend process group",
                ))
            }
        }
    }

    #[cfg(not(windows))]
    async fn terminate(&mut self) -> io::Result<()> {
        self.armed = false;
        Ok(())
    }

    #[cfg(windows)]
    fn command(&self) -> Command {
        let mut command = Command::new(&self.launcher);
        command
            .args(wsl_cleanup_args(
                self.settings.distribution.as_deref(),
                self.settings.user.as_deref(),
                &self.pid_file,
            ))
            .env("PATH", &self.path);
        command
    }
}

#[cfg(windows)]
impl Drop for WslCleanup {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        self.armed = false;
        let mut command = std::process::Command::new(&self.launcher);
        command
            .args(wsl_cleanup_args(
                self.settings.distribution.as_deref(),
                self.settings.user.as_deref(),
                &self.pid_file,
            ))
            .env("PATH", &self.path);
        let _ = command.spawn();
    }
}

#[cfg(any(windows, test))]
fn wsl_base_args(distribution: Option<&str>, user: Option<&str>) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(distribution) = distribution.filter(|value| !value.is_empty()) {
        args.extend(["--distribution".to_string(), distribution.to_string()]);
    }
    if let Some(user) = user.filter(|value| !value.is_empty()) {
        args.extend(["--user".to_string(), user.to_string()]);
    }
    args
}

#[cfg(any(windows, test))]
fn wsl_launch_args(
    distribution: Option<&str>,
    user: Option<&str>,
    pid_file: &str,
    binary: &str,
    backend_args: &[&str],
    environment: &[(&str, &str)],
) -> Vec<String> {
    let mut args = wsl_base_args(distribution, user);
    args.extend([
        "--exec".to_string(),
        "/bin/bash".to_string(),
        "-lc".to_string(),
        WSL_LAUNCH_SCRIPT.to_string(),
        "codex-thread-studio".to_string(),
        pid_file.to_string(),
        binary.to_string(),
    ]);
    args.extend(
        environment
            .iter()
            .map(|(key, value)| format!("{key}={value}")),
    );
    args.push("--".to_string());
    args.extend(backend_args.iter().map(|value| (*value).to_string()));
    args
}

#[cfg(any(windows, test))]
fn wsl_cleanup_args(distribution: Option<&str>, user: Option<&str>, pid_file: &str) -> Vec<String> {
    let mut args = wsl_base_args(distribution, user);
    args.extend([
        "--exec".to_string(),
        "/bin/bash".to_string(),
        "-c".to_string(),
        WSL_CLEANUP_SCRIPT.to_string(),
        "codex-thread-studio-cleanup".to_string(),
        pid_file.to_string(),
    ]);
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wsl_launch_keeps_user_values_as_arguments() {
        let args = wsl_launch_args(
            Some("Ubuntu 24.04"),
            Some("rui"),
            "/tmp/codex-thread-studio/runtime.pid",
            "/home/rui/tools with spaces/codex",
            &["app-server", "--stdio"],
            &[("LOG_FORMAT", "json")],
        );
        assert_eq!(
            &args[..4],
            ["--distribution", "Ubuntu 24.04", "--user", "rui"]
        );
        assert!(args.iter().any(|value| value == "LOG_FORMAT=json"));
        assert!(args
            .iter()
            .any(|value| value == "/home/rui/tools with spaces/codex"));
        assert!(args.iter().any(|value| value == "--"));
        assert_eq!(&args[args.len() - 2..], ["app-server", "--stdio"]);
    }

    #[test]
    fn wsl_cleanup_uses_only_the_private_pid_file() {
        let args = wsl_cleanup_args(Some("Ubuntu"), None, "/tmp/runtime.pid");
        assert_eq!(&args[..2], ["--distribution", "Ubuntu"]);
        assert_eq!(args.last().map(String::as_str), Some("/tmp/runtime.pid"));
        assert!(args.iter().any(|value| value.contains("kill -TERM")));
    }

    #[cfg(unix)]
    #[test]
    fn wsl_launch_script_preserves_environment_and_cleans_pid_file() {
        let pid_file = std::env::temp_dir().join(format!(
            "codex-thread-studio-wsl-script-{}.pid",
            uuid::Uuid::new_v4().simple()
        ));
        let output = std::process::Command::new("/bin/bash")
            .args([
                "-lc",
                WSL_LAUNCH_SCRIPT,
                "codex-thread-studio",
                pid_file.to_str().unwrap(),
                "/bin/sh",
                "STUDIO_WSL_TEST=ready",
                "--",
                "-c",
                "printf '%s' \"$STUDIO_WSL_TEST\"",
            ])
            .output()
            .expect("run WSL launch script locally");
        assert!(output.status.success());
        assert_eq!(output.stdout, b"ready");
        assert!(!pid_file.exists());
    }
}
