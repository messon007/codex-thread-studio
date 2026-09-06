//! Optional Windows-native backend runtime; WSL remains the default.
//! Built only with the windows-native Cargo feature.
use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};
use tokio::process::{Child, Command};

pub fn augmented_path(original: std::ffi::OsString) -> std::ffi::OsString {
    let mut paths: Vec<_> = std::env::split_paths(&original).collect();
    let candidates = [
        std::env::var_os("NVM_HOME").map(PathBuf::from),
        std::env::var_os("NVM_SYMLINK").map(PathBuf::from),
        std::env::var_os("APPDATA").map(|value| PathBuf::from(value).join("npm")),
        std::env::var_os("LOCALAPPDATA")
            .map(|value| PathBuf::from(value).join("Microsoft/WinGet/Links")),
        std::env::var_os("USERPROFILE").map(|value| PathBuf::from(value).join(".bun/bin")),
        std::env::var_os("USERPROFILE").map(|value| PathBuf::from(value).join(".cargo/bin")),
    ];
    for candidate in candidates.into_iter().flatten() {
        if candidate.is_dir() && !paths.contains(&candidate) {
            paths.push(candidate);
        }
    }
    std::env::join_paths(paths).unwrap_or(original)
}

pub fn command(binary: &str, args: &[&str], path_env: &OsStr) -> io::Result<Command> {
    let requested = Path::new(binary);
    let path = if has_explicit_path(requested) {
        resolve_candidate(requested)
    } else {
        find_on_path(binary, path_env)
    }
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("backend executable `{binary}` was not found on the configured PATH"),
        )
    })?;
    // Rust handles .cmd/.bat quoting and rejects unsafe arguments. Do not use raw_arg.
    let mut command = Command::new(path);
    command.args(args).env("PATH", path_env);
    command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    Ok(command)
}
#[cfg(windows)]
pub struct WindowsJob {
    handle: std::os::windows::io::OwnedHandle,
}

#[cfg(windows)]
impl WindowsJob {
    pub fn attach(child: &Child) -> io::Result<Self> {
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

    pub fn terminate(&self) -> io::Result<()> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::JobObjects::TerminateJobObject;

        if unsafe { TerminateJobObject(self.handle.as_raw_handle(), 1) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unresolved_commands_do_not_fall_back_to_working_directory() {
        assert_eq!(
            command("studio-missing-executable", &[], OsStr::new(""))
                .unwrap_err()
                .kind(),
            io::ErrorKind::NotFound
        );
    }
    #[test]
    fn discovery_prefers_executables_then_npm_shims() {
        assert_eq!(
            path_variants("codex")[..3],
            ["codex.exe", "codex.cmd", "codex.bat"]
        );
    }
    #[tokio::test]
    async fn batch_shim_runs_with_spaces_in_its_path() {
        let directory =
            std::env::temp_dir().join(format!("studio native {}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let shim = directory.join("fixture.cmd");
        std::fs::write(&shim, "@echo off\r\necho %~1\r\n").unwrap();
        let output = command(shim.to_str().unwrap(), &["hello world"], OsStr::new(""))
            .unwrap()
            .output()
            .await
            .unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "hello world"
        );
        std::fs::remove_dir_all(directory).unwrap();
    }
}
