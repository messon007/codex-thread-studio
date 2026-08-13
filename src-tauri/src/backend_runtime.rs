use std::ffi::OsString;
use std::io;
use std::process::{ExitStatus, Output, Stdio};

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
            configure_native_command(&mut command);
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
        #[cfg(unix)]
        let native_process_group = child.id();
        Ok(RuntimeChild {
            child,
            cleanup: self.cleanup,
            #[cfg(unix)]
            native_process_group,
        })
    }

    pub async fn output(mut self) -> io::Result<Output> {
        self.command.kill_on_drop(true);
        let result = self.command.output().await;
        if result.is_ok() {
            if let Some(cleanup) = self.cleanup.as_mut() {
                let _ = cleanup.terminate().await;
            }
        }
        result
    }
}

pub struct RuntimeChild {
    child: Child,
    cleanup: Option<WslCleanup>,
    #[cfg(unix)]
    native_process_group: Option<u32>,
}

impl RuntimeChild {
    #[cfg(test)]
    pub fn id(&self) -> Option<u32> {
        self.child.id()
    }

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
            #[cfg(unix)]
            self.terminate_native_process_group();
            if let Some(cleanup) = self.cleanup.as_mut() {
                let _ = cleanup.terminate().await;
            }
        }
        result
    }

    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        let result = self.child.try_wait();
        if result.as_ref().is_ok_and(Option::is_some) {
            #[cfg(unix)]
            self.terminate_native_process_group();
        }
        result
    }

    #[cfg(unix)]
    fn terminate_native_process_group(&mut self) {
        if let Some(process_group) = self.native_process_group.take() {
            let _ = terminate_native_process_group(process_group);
        }
    }

    pub async fn kill_tree(&mut self) -> io::Result<()> {
        let cleanup_result = match self.cleanup.as_mut() {
            Some(cleanup) => cleanup.terminate().await,
            None => Ok(()),
        };
        #[cfg(unix)]
        let group_result = self
            .native_process_group
            .take()
            .map_or(Ok(()), terminate_native_process_group);
        #[cfg(not(unix))]
        let group_result = Ok(());
        let kill_result = self.child.start_kill();
        let wait_result = self.child.wait().await.map(|_| ());
        cleanup_result
            .and(group_result)
            .and(kill_result)
            .and(wait_result)
    }
}

#[cfg(unix)]
impl Drop for RuntimeChild {
    fn drop(&mut self) {
        let _ = self.child.try_wait();
        self.terminate_native_process_group();
    }
}

#[cfg(unix)]
fn configure_native_command(command: &mut Command) {
    command.process_group(0);
    #[cfg(target_os = "linux")]
    {
        let expected_parent = unsafe { libc::getpid() };
        // SAFETY: this closure only invokes async-signal-safe libc calls before exec.
        unsafe {
            command.pre_exec(move || {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
                    return Err(io::Error::last_os_error());
                }
                if libc::getppid() != expected_parent {
                    libc::_exit(1);
                }
                Ok(())
            });
        }
    }
}

#[cfg(unix)]
fn terminate_native_process_group(process_group: u32) -> io::Result<()> {
    let result = unsafe { libc::kill(-(process_group as i32), libc::SIGKILL) };
    if result == 0 {
        return Ok(());
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(error)
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

    #[cfg(target_os = "linux")]
    const PARENT_DEATH_PID_FILE: &str = "CODEX_THREAD_STUDIO_PARENT_DEATH_PID_FILE";

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

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "test helper launched by native_parent_death_signal_terminates_the_backend"]
    fn native_parent_death_helper() {
        let Some(pid_file) = std::env::var_os(PARENT_DEATH_PID_FILE) else {
            return;
        };
        let tokio_runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build helper runtime");
        let _runtime_guard = tokio_runtime.enter();
        let runtime = BackendRuntime::new(
            std::env::var_os("PATH").unwrap_or_default(),
            WslSettings::default(),
        );
        let child = runtime
            .command("/bin/sleep", &["30"], &[])
            .spawn()
            .expect("spawn backend child");
        std::fs::write(pid_file, child.id().expect("backend child pid").to_string())
            .expect("write backend child pid");
        std::process::exit(0);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn native_parent_death_signal_terminates_the_backend() {
        let pid_file = std::env::temp_dir().join(format!(
            "codex-thread-studio-parent-death-{}.pid",
            uuid::Uuid::new_v4().simple()
        ));
        let status = std::process::Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--ignored",
                "--exact",
                "backend_runtime::tests::native_parent_death_helper",
            ])
            .env(PARENT_DEATH_PID_FILE, &pid_file)
            .status()
            .expect("run parent-death helper");
        assert!(status.success());
        let pid = std::fs::read_to_string(&pid_file)
            .expect("read backend child pid")
            .parse::<i32>()
            .expect("numeric backend child pid");
        let terminated = (0..100).any(|_| {
            if !native_process_is_running(pid) {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
            false
        });
        let _ = std::fs::remove_file(pid_file);
        assert!(
            terminated,
            "backend process {pid} survived its Studio parent"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn dropping_a_backend_terminates_its_process_group() {
        let pid_file = std::env::temp_dir().join(format!(
            "codex-thread-studio-process-group-{}.pid",
            uuid::Uuid::new_v4().simple()
        ));
        let tokio_runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build test runtime");
        let _runtime_guard = tokio_runtime.enter();
        let runtime = BackendRuntime::new(
            std::env::var_os("PATH").unwrap_or_default(),
            WslSettings::default(),
        );
        let child = runtime
            .command(
                "/bin/sh",
                &[
                    "-c",
                    "sleep 30 & printf '%s' \"$!\" > \"$1\"; wait",
                    "codex-thread-studio-test",
                    pid_file.to_str().expect("UTF-8 pid path"),
                ],
                &[],
            )
            .spawn()
            .expect("spawn backend process group");
        let descendant_pid = (0..100)
            .find_map(|_| {
                let pid = std::fs::read_to_string(&pid_file)
                    .ok()
                    .and_then(|value| value.parse::<i32>().ok());
                if pid.is_none() {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                pid
            })
            .expect("backend descendant pid");
        drop(child);
        let terminated = (0..100).any(|_| {
            if !native_process_is_running(descendant_pid) {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
            false
        });
        let _ = std::fs::remove_file(pid_file);
        assert!(
            terminated,
            "backend descendant {descendant_pid} survived RuntimeChild drop"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn reaping_an_exited_backend_terminates_remaining_descendants() {
        let pid_file = std::env::temp_dir().join(format!(
            "codex-thread-studio-reaped-process-group-{}.pid",
            uuid::Uuid::new_v4().simple()
        ));
        let tokio_runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build test runtime");
        let _runtime_guard = tokio_runtime.enter();
        let runtime = BackendRuntime::new(
            std::env::var_os("PATH").unwrap_or_default(),
            WslSettings::default(),
        );
        let mut child = runtime
            .command(
                "/bin/sh",
                &[
                    "-c",
                    "sleep 30 & printf '%s' \"$!\" > \"$1\"",
                    "codex-thread-studio-test",
                    pid_file.to_str().expect("UTF-8 pid path"),
                ],
                &[],
            )
            .spawn()
            .expect("spawn short-lived backend process group");
        let descendant_pid = (0..100)
            .find_map(|_| {
                let pid = std::fs::read_to_string(&pid_file)
                    .ok()
                    .and_then(|value| value.parse::<i32>().ok());
                if pid.is_none() {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                pid
            })
            .expect("backend descendant pid");
        tokio_runtime
            .block_on(child.wait())
            .expect("reap backend process");
        let terminated = (0..100).any(|_| {
            if !native_process_is_running(descendant_pid) {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
            false
        });
        let _ = std::fs::remove_file(pid_file);
        assert!(
            terminated,
            "backend descendant {descendant_pid} survived parent reaping"
        );
    }

    #[cfg(target_os = "linux")]
    fn native_process_is_running(pid: i32) -> bool {
        let stat = match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
            Ok(stat) => stat,
            Err(_) => return false,
        };
        stat.rsplit_once(')')
            .and_then(|(_, fields)| fields.split_whitespace().next())
            != Some("Z")
    }

    // WSL is Linux, and the launch contract intentionally relies on Linux's setsid.
    #[cfg(target_os = "linux")]
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
