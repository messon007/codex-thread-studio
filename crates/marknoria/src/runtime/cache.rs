//! Private disk-backed caches shared by every frontend.
use std::{fs, io, path::PathBuf};

pub fn cache_directory() -> io::Result<tempfile::TempDir> {
    temporary_directory("markdown-reader-")
}

fn storage_root(
    configured: Option<PathBuf>,
    temporary: Option<PathBuf>,
    xdg: Option<PathBuf>,
    home: Option<PathBuf>,
) -> io::Result<PathBuf> {
    if let Some(path) = configured.or(temporary) {
        return Ok(path);
    }
    if let Some(path) = xdg.filter(|path| path.is_absolute()) {
        return Ok(path.join("markdown-reader"));
    }
    if let Some(path) = home.filter(|path| path.is_absolute()) {
        return Ok(path.join(".cache/markdown-reader"));
    }
    Err(io::Error::other(
        "Set READER_CACHE_DIR to a disk-backed directory; HOME/XDG_CACHE_HOME are unavailable",
    ))
}

pub fn temporary_directory(prefix: &str) -> io::Result<tempfile::TempDir> {
    let variable = |name| {
        std::env::var_os(name)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };
    let cache_home = variable("XDG_CACHE_HOME").or_else(|| {
        if cfg!(windows) {
            variable("LOCALAPPDATA")
        } else {
            None
        }
    });
    let home = variable("HOME").or_else(|| variable("USERPROFILE"));
    let root = storage_root(
        variable("READER_CACHE_DIR"),
        variable("TMPDIR"),
        cache_home,
        home,
    )?;
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(&root)?;
    tempfile::Builder::new()
        .prefix(prefix)
        .tempdir_in(&root)
        .map_err(|error| io::Error::new(error.kind(), format!("Cannot create reader cache in {}: {error}. Set READER_CACHE_DIR to a disk-backed directory with available quota.", root.display())))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_storage_prefers_explicit_paths_and_defaults_outside_tmp() {
        let rooted = |value: &str| if cfg!(windows) { PathBuf::from(format!("C:{value}")) } else { PathBuf::from(value) };
        let path = |value: &str| Some(if value.starts_with('/') { rooted(value) } else { PathBuf::from(value) });
        assert_eq!(
            storage_root(
                path("/explicit"),
                path("/temporary"),
                path("/cache"),
                path("/home/user")
            )
            .unwrap(),
            rooted("/explicit")
        );
        assert_eq!(
            storage_root(None, path("/temporary"), path("/cache"), path("/home/user")).unwrap(),
            rooted("/temporary")
        );
        assert_eq!(
            storage_root(None, None, path("/cache"), path("/home/user")).unwrap(),
            rooted("/cache/markdown-reader")
        );
        assert_eq!(
            storage_root(None, None, path("relative"), path("/home/user")).unwrap(),
            rooted("/home/user/.cache/markdown-reader")
        );
        assert!(storage_root(None, None, None, None).is_err());
    }
}
