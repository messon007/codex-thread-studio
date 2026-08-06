use std::ffi::OsStr;
#[cfg(any(windows, test))]
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;

use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};

#[derive(Clone, Debug)]
pub struct CommandSpec {
    pub path: PathBuf,
    pub args: Vec<String>,
    pub shell_wrapped: bool,
}

impl CommandSpec {
    pub fn new(path: PathBuf, args: Vec<String>, shell_wrapped: bool) -> Self {
        Self {
            path,
            args,
            shell_wrapped,
        }
    }

    pub fn command(self, path_env: &OsStr) -> io::Result<Command> {
        if !has_explicit_path(&self.path) {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "backend executable `{}` was not resolved from the configured PATH",
                    self.path.display()
                ),
            ));
        }

        let mut command = if self.shell_wrapped {
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;

                let mut raw_command = OsString::from("\"");
                raw_command.push(self.shell_escaped_command_line());
                raw_command.push("\"");

                let mut command = Command::new("cmd.exe");
                command.args(["/D", "/S", "/C"]);
                command.as_std_mut().raw_arg(raw_command);
                const CREATE_NO_WINDOW: u32 = 0x08000000;
                command.creation_flags(CREATE_NO_WINDOW);
                command
            }
            #[cfg(not(windows))]
            {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "batch shims are supported only on Windows",
                ));
            }
        } else {
            let mut command = Command::new(&self.path);
            command.args(self.args.clone());
            #[cfg(windows)]
            {
                const CREATE_NO_WINDOW: u32 = 0x08000000;
                command.creation_flags(CREATE_NO_WINDOW);
            }
            command
        };
        command.env("PATH", path_env);
        Ok(command)
    }

    #[cfg(any(windows, test))]
    fn shell_escaped_command_line(&self) -> OsString {
        let mut command_line = OsString::from("\"");
        command_line.push(&self.path);
        command_line.push("\"");
        for argument in &self.args {
            command_line.push(" ");
            command_line.push(quote_windows_arg(argument));
        }
        command_line
    }
}

pub struct ManagedChild {
    #[cfg(windows)]
    job: WindowsJob,
    child: Child,
}

impl ManagedChild {
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
        self.child.wait().await
    }

    pub async fn kill_tree(&mut self) -> io::Result<()> {
        #[cfg(windows)]
        self.job.terminate()?;
        #[cfg(not(windows))]
        self.child.start_kill()?;

        let _ = self.child.wait().await?;
        Ok(())
    }
}

pub fn spawn_managed(mut command: Command) -> io::Result<ManagedChild> {
    command.kill_on_drop(true);
    #[allow(unused_mut)]
    let mut child = command.spawn()?;

    #[cfg(windows)]
    let job = match WindowsJob::attach(&child) {
        Ok(job) => job,
        Err(error) => {
            let _ = child.start_kill();
            return Err(error);
        }
    };

    Ok(ManagedChild {
        #[cfg(windows)]
        job,
        child,
    })
}

#[cfg(windows)]
struct WindowsJob {
    handle: std::os::windows::io::OwnedHandle,
}

#[cfg(windows)]
impl WindowsJob {
    fn attach(child: &Child) -> io::Result<Self> {
        use std::mem::size_of;
        use std::os::windows::io::{AsRawHandle, FromRawHandle};
        use std::ptr;
        use windows_sys::Win32::System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
            SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };

        let raw_job = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if raw_job.is_null() {
            return Err(io::Error::last_os_error());
        }
        let handle = unsafe { std::os::windows::io::OwnedHandle::from_raw_handle(raw_job) };

        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let configured = unsafe {
            SetInformationJobObject(
                raw_job,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if configured == 0 {
            return Err(io::Error::last_os_error());
        }

        let process = child
            .raw_handle()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "child already exited"))?;
        if unsafe { AssignProcessToJobObject(raw_job, process) } == 0 {
            return Err(io::Error::last_os_error());
        }

        debug_assert_eq!(handle.as_raw_handle(), raw_job);
        Ok(Self { handle })
    }

    fn terminate(&self) -> io::Result<()> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::JobObjects::TerminateJobObject;

        if unsafe { TerminateJobObject(self.handle.as_raw_handle(), 1) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

pub fn command_spec(name: &str, args: &[&str]) -> CommandSpec {
    let path = PathBuf::from(name);
    let shell_wrapped = is_batch_like(&path);
    CommandSpec::new(
        path,
        args.iter().map(std::string::ToString::to_string).collect(),
        shell_wrapped,
    )
}

pub fn resolve_command_binary(name: &str, override_env: &str, path_env: &OsStr) -> String {
    if let Ok(configured) = std::env::var(override_env) {
        let configured = configured.trim();
        if !configured.is_empty() {
            let configured_path = Path::new(configured);
            let resolved = if has_explicit_path(configured_path) {
                resolve_candidate(configured_path)
            } else {
                find_on_path(configured, path_env)
            };
            return resolved
                .unwrap_or_else(|| configured_path.to_path_buf())
                .to_string_lossy()
                .into_owned();
        }
    }

    find_on_path(name, path_env)
        .unwrap_or_else(|| PathBuf::from(name))
        .to_string_lossy()
        .into_owned()
}

fn find_on_path(name: &str, path_env: &OsStr) -> Option<PathBuf> {
    let candidates = path_variants(name);
    for directory in std::env::split_paths(path_env) {
        for candidate in &candidates {
            let path = directory.join(candidate);
            if is_executable(&path) {
                return Some(path);
            }
        }
    }
    None
}

fn resolve_candidate(path: &Path) -> Option<PathBuf> {
    if !has_explicit_path(path) {
        return None;
    }
    if is_executable(path) {
        return Some(path.to_path_buf());
    }

    let name = path.file_name().and_then(|name| name.to_str())?;
    let parent = path.parent();
    for variant in path_variants(name) {
        let candidate = parent
            .filter(|directory| !directory.as_os_str().is_empty())
            .map_or_else(
                || PathBuf::from(&variant),
                |directory| directory.join(&variant),
            );
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }

    None
}

fn has_explicit_path(path: &Path) -> bool {
    path.is_absolute()
        || path
            .parent()
            .is_some_and(|parent| !parent.as_os_str().is_empty())
}

pub fn path_variants(name: &str) -> Vec<String> {
    if !cfg!(windows) {
        return vec![name.to_string()];
    }

    if Path::new(name).extension().is_some() {
        return vec![name.to_string()];
    }

    vec![
        format!("{name}.exe"),
        format!("{name}.cmd"),
        format!("{name}.bat"),
        format!("{name}.com"),
        name.to_string(),
    ]
}

fn is_batch_like(path: &Path) -> bool {
    if !cfg!(windows) {
        return false;
    }

    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("cmd") | Some("bat")
    )
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    path.metadata()
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

#[cfg(any(windows, test))]
fn quote_windows_arg(value: &str) -> String {
    if value.is_empty() {
        return "\"\"".to_string();
    }

    let mut escaped = String::new();
    for ch in value.chars() {
        if ch == '"' {
            escaped.push('"');
            escaped.push('"');
        } else if matches!(
            ch,
            '&' | '|' | '^' | '<' | '>' | '%' | '!' | '(' | ')' | '{' | '}'
        ) {
            escaped.push('^');
            escaped.push(ch);
        } else {
            escaped.push(ch);
        }
    }

    if value
        .chars()
        .any(|ch| ch.is_ascii_whitespace() || ch == '"')
    {
        format!("\"{escaped}\"")
    } else {
        escaped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn windows_variants_include_script_extensions() {
        let variants = path_variants("codex");
        assert_eq!(variants[0], "codex.exe");
        assert_eq!(variants[1], "codex.cmd");
        assert_eq!(variants[2], "codex.bat");
        assert_eq!(variants[3], "codex.com");
    }

    #[test]
    fn quote_windows_arg_keeps_simple_tokens() {
        assert_eq!(quote_windows_arg("hello"), "hello");
        assert_eq!(quote_windows_arg("a b"), "\"a b\"");
        assert_eq!(quote_windows_arg("a&b"), "a^&b");
    }

    #[test]
    fn cmd_raw_argument_preserves_required_outer_quotes() {
        let spec = CommandSpec::new(
            PathBuf::from(r"C:\Program Files\npm\codex.cmd"),
            vec!["app-server".to_string(), "--stdio".to_string()],
            true,
        );
        let mut raw = OsString::from("\"");
        raw.push(spec.shell_escaped_command_line());
        raw.push("\"");
        assert_eq!(
            raw.to_string_lossy(),
            r#"""C:\Program Files\npm\codex.cmd" app-server --stdio""#
        );
    }

    #[test]
    fn unresolved_bare_commands_are_not_executed_from_the_working_directory() {
        let spec = CommandSpec::new(
            PathBuf::from("codex"),
            vec!["app-server".to_string()],
            false,
        );
        let error = spec.command(OsStr::new("")).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    #[cfg(not(windows))]
    #[test]
    fn find_on_path_prefers_extension_variants_in_parent_dir() {
        use std::fs::OpenOptions;
        use std::os::unix::fs::OpenOptionsExt;
        use std::os::unix::fs::PermissionsExt;
        let temp_dir = std::env::temp_dir().join("codex-thread-studio-bin-test");
        std::fs::create_dir_all(&temp_dir).expect("create temp test dir");
        let target = temp_dir.join("mycodex");

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o755)
            .open(&target)
            .expect("create target binary");
        use std::io::Write;
        file.write_all(b"#!/bin/sh\necho ok\n").expect("write file");
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755))
            .expect("set mode");

        let path_env = temp_dir.as_os_str();
        let resolved = find_on_path("mycodex", path_env).expect("should find exact binary");
        assert_eq!(resolved, target);

        std::fs::remove_file(&target).unwrap();
        if let Some(parent) = temp_dir.parent() {
            let _ = std::fs::remove_dir_all(parent.join("codex-thread-studio-bin-test"));
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn resolve_candidate_prefers_parent_directory_and_basename() {
        use std::fs::OpenOptions;
        use std::os::unix::fs::OpenOptionsExt;
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = std::env::temp_dir().join("codex-thread-studio-candidate-test");
        std::fs::create_dir_all(&temp_dir).expect("create test dir");
        let target = temp_dir.join("bin");
        std::fs::create_dir_all(&target).expect("create nested bin dir");
        let target_file = target.join("codex");

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o755)
            .open(&target_file)
            .expect("write candidate");
        use std::io::Write;
        file.write_all(b"#!/bin/sh\n").expect("write file");
        std::fs::set_permissions(&target_file, std::fs::Permissions::from_mode(0o755))
            .expect("set perms");

        let configured = temp_dir.join("bin/codex");
        let resolved = resolve_candidate(&configured).expect("should resolve");
        assert_eq!(resolved, target_file);

        let _ = std::fs::remove_file(&target_file);
        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
