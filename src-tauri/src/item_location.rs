use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub fn location(root: &str, relative: &str) -> Result<PathBuf, String> {
    let root = fs::canonicalize(root).map_err(|error| error.to_string())?;
    if !root.is_dir() || Path::new(relative).is_absolute() {
        return Err("Expected a project directory and a relative item path".into());
    }
    let item = fs::canonicalize(root.join(relative)).map_err(|error| error.to_string())?;
    if !item.starts_with(&root) {
        return Err("Item is outside the project directory".into());
    }
    if item.is_dir() {
        Ok(item)
    } else {
        item.parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| "Item has no parent directory".into())
    }
}

pub fn open(root: &str, relative: &str) -> Result<(), String> {
    let directory = location(root, relative)?;
    #[cfg(target_os = "linux")]
    let mut command = Command::new("xdg-open");
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(windows)]
    let mut command = Command::new("explorer.exe");
    let status = command
        .arg(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| error.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("System file manager exited with {status}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resolves_files_and_directories_without_escaping_root() {
        let root = std::env::temp_dir().join(format!("studio-location-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("nested")).unwrap();
        fs::write(root.join("nested/file.txt"), "test").unwrap();
        let expected = fs::canonicalize(root.join("nested")).unwrap();
        let path = root.to_str().unwrap();
        assert_eq!(location(path, "nested/file.txt").unwrap(), expected);
        assert_eq!(location(path, "nested").unwrap(), expected);
        assert!(location(path, "..").is_err());
        assert!(location(path, "missing").is_err());
        assert!(location(path, path).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
