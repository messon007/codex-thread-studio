//! Companion discovery and immutable installation, independent of the running UI inode.
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

type Result<T> = std::result::Result<T, String>;
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
const NAME: &str = if cfg!(windows) {
    "studio-teams-mcp.exe"
} else {
    "studio-teams-mcp"
};

pub fn companion(host: &Path) -> Result<PathBuf> {
    // On Linux current_exe may end in " (deleted)" after a rebuild. Only its
    // parent directory locates the independently built companion; never strip
    // the suffix and execute a replacement Studio, or persist /proc/self/exe.
    let path = host
        .parent()
        .ok_or("Studio executable has no parent")?
        .join(NAME);
    if !path.is_absolute() || !path.is_file() {
        return Err("studio-teams-mcp companion is missing. Build/install it alongside Studio before installing Teams tools.".into());
    }
    Ok(path)
}

pub fn install(directory: &Path, source: &Path) -> Result<PathBuf> {
    let mut bytes = Vec::new();
    fs::File::open(source)
        .map_err(err)?
        .take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(err)?;
    if bytes.is_empty() || bytes.len() > 64 * 1024 * 1024 {
        return Err("Invalid Teams MCP companion size".into());
    }
    let hash: String = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let bin_dir = directory.join("bin").join(hash);
    fs::create_dir_all(&bin_dir).map_err(err)?;
    let dest = bin_dir.join(NAME);
    if dest.exists() {
        if fs::read(&dest).map_err(err)? != bytes {
            return Err(
                "Installed Teams MCP companion was modified; refusing to overwrite it".into(),
            );
        }
        return Ok(dest);
    }
    let tmp = bin_dir.join(format!(".install-{}", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o700);
        }
        let mut file = options.open(&tmp).map_err(err)?;
        file.write_all(&bytes).map_err(err)?;
        file.sync_all().map_err(err)?;
        drop(file);
        // Publish without replacing an executable another helper may be running.
        match fs::hard_link(&tmp, &dest) {
            Ok(()) => (),
            Err(e)
                if e.kind() == std::io::ErrorKind::AlreadyExists
                    && fs::read(&dest).map_err(err)? == bytes =>
            {
                ()
            }
            Err(e) => return Err(err(e)),
        }
        #[cfg(unix)]
        {
            fs::File::open(&bin_dir)
                .and_then(|f| f.sync_all())
                .map_err(err)?;
        }
        Ok(dest)
    })();
    let _ = fs::remove_file(tmp);
    result
}

/// Compatibility for old, still-valid Studio registrations. New installs never use this.
pub fn legacy_entry() -> Option<Result<()>> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).and_then(|v| v.to_str()) != Some("--teams-mcp") {
        return None;
    }
    Some((|| {
        let exe = companion(&std::env::current_exe().map_err(err)?)?;
        let status = std::process::Command::new(exe)
            .args(&args[2..])
            .status()
            .map_err(err)?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("Teams MCP companion exited: {status}"))
        }
    })())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deleted_host_uses_a_separate_companion_and_missing_companion_fails() {
        let dir = tempfile::tempdir().unwrap();
        let host = dir.path().join("codex-thread-studio (deleted)");
        assert!(companion(&host).is_err());
        let helper = dir.path().join(NAME);
        fs::write(&helper, b"synthetic helper").unwrap();
        assert_eq!(companion(&host).unwrap(), helper);
    }
    #[test]
    fn installed_helper_survives_replacement_and_keeps_old_versions_immutable() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join(NAME);
        let destination = dir.path().join("settings");
        fs::write(&source, b"helper one").unwrap();
        let first = install(&destination, &source).unwrap();
        assert_eq!(install(&destination, &source).unwrap(), first);
        fs::remove_file(&source).unwrap();
        fs::write(&source, b"helper two").unwrap();
        let second = install(&destination, &source).unwrap();
        assert_ne!(first, second);
        fs::remove_file(&source).unwrap();
        assert_eq!(fs::read(&first).unwrap(), b"helper one");
        assert_eq!(fs::read(&second).unwrap(), b"helper two");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&first).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
        fs::write(&source, b"helper one").unwrap();
        fs::write(&first, b"external edit").unwrap();
        assert!(install(&destination, &source).is_err());
    }
}
