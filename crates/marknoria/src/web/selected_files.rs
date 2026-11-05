//! Per-window file capabilities written only by the trusted desktop picker.
//! HTTP callers receive opaque handles, never authority to submit absolute paths.
use std::{
    collections::HashMap,
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

pub const DIRECTORY_ENV: &str = "MARKNORIA_SELECTED_FILES";
const PREFIX: &str = "@desktop/";
const MAX_GRANTS: usize = 256;
const MAX_RECORD: u64 = 16 * 1024;

pub struct SelectionDirectory {
    directory: tempfile::TempDir,
    files: HashMap<PathBuf, String>,
}
impl SelectionDirectory {
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            directory: crate::runtime::cache::temporary_directory("marknoria-selected-")?,
            files: HashMap::new(),
        })
    }
    pub fn path(&self) -> &Path {
        self.directory.path()
    }
    pub fn grant(&mut self, path: &Path) -> io::Result<String> {
        let path = validate_file(path)?;
        // A cache cleaner may remove this window's private directory. Rebuild
        // only our own directory; missing capabilities stay revoked until the
        // trusted picker explicitly selects that file again.
        match fs::symlink_metadata(self.path()) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
            Ok(_) => {
                return Err(io::Error::other(
                    "Desktop selection storage was replaced; reopen this window",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let mut directory = fs::DirBuilder::new();
                directory.recursive(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::DirBuilderExt;
                    directory.mode(0o700);
                }
                directory.create(self.path()).map_err(|e| {
                    io::Error::new(
                        e.kind(),
                        format!("Cannot restore desktop selection storage: {e}"),
                    )
                })?;
            }
            Err(error) => return Err(error),
        }
        if let Some(handle) = self.files.get(&path) {
            let id = handle.strip_prefix(PREFIX).unwrap();
            if self.path().join(format!("{id}.json")).is_file() {
                return Ok(handle.clone());
            }
        }
        if self.files.len() >= MAX_GRANTS && !self.files.contains_key(&path) {
            return Err(io::Error::other(
                "File selection limit reached; reopen this desktop window",
            ));
        }
        let bytes = serde_json::to_vec(&path)?;
        if bytes.len() as u64 > MAX_RECORD {
            return Err(io::Error::other("Selected file path is too long"));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let mut record = tempfile::NamedTempFile::new_in(self.path()).map_err(|e| {
            io::Error::new(
                e.kind(),
                format!("Cannot create a desktop selection record: {e}"),
            )
        })?;
        record.write_all(&bytes)?;
        record
            .persist(self.path().join(format!("{id}.json")))
            .map_err(|error| error.error)?;
        let handle = format!("{PREFIX}{id}");
        self.files.insert(path, handle.clone());
        Ok(handle)
    }
}

fn validate_file(path: &Path) -> io::Result<PathBuf> {
    let path = path.canonicalize().map_err(|e| {
        io::Error::new(
            e.kind(),
            format!("Selected file is unavailable ({}): {e}", path.display()),
        )
    })?;
    let supported = crate::core::document_format::DocumentFormat::for_path(&path).is_some();
    let metadata = path.metadata()?;
    if !supported || !metadata.is_file() || metadata.len() > crate::core::MAX_SOURCE {
        return Err(io::Error::other(
            "Choose a regular supported file of at most 128 MiB",
        ));
    }
    Ok(path)
}

pub fn resolve(directory: Option<&Path>, handle: &str) -> io::Result<PathBuf> {
    let denied = || {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            "File was not selected in this desktop window",
        )
    };
    let directory = directory.ok_or_else(denied)?;
    let id = handle.strip_prefix(PREFIX).ok_or_else(denied)?;
    let id = uuid::Uuid::parse_str(id).map_err(|_| denied())?;
    if id.to_string() != handle[PREFIX.len()..] {
        return Err(denied());
    }
    let mut bytes = Vec::new();
    fs::File::open(directory.join(format!("{id}.json")))?
        .take(MAX_RECORD + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_RECORD {
        return Err(denied());
    }
    let selected: PathBuf = serde_json::from_slice(&bytes)?;
    let path = validate_file(&selected)?;
    if !selected.is_absolute() || selected != path {
        return Err(denied());
    }
    Ok(path)
}
pub fn is_handle(path: &str) -> bool {
    path.starts_with(PREFIX)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_file_capabilities_are_local_bounded_and_cleaned() {
        let source = crate::runtime::cache::temporary_directory("mn-selected-test-").unwrap();
        let file = source.path().join("中文.md");
        fs::write(&file, "# Original").unwrap();
        let mut grants = SelectionDirectory::new().unwrap();
        let directory = grants.path().to_owned();
        let handle = grants.grant(&file).unwrap();
        assert_eq!(grants.grant(&file).unwrap(), handle);
        assert_eq!(
            resolve(Some(&directory), &handle).unwrap(),
            file.canonicalize().unwrap()
        );
        assert!(resolve(None, &handle).is_err());
        for invalid in [
            "@desktop/../secret",
            "@desktop/not-a-uuid",
            &format!("@desktop/{}", uuid::Uuid::new_v4()),
        ] {
            assert!(resolve(Some(&directory), invalid).is_err());
        }
        let other = source.path().join("unsupported.exe");
        fs::write(&other, "private").unwrap();
        assert!(grants.grant(&other).is_err());
        drop(grants);
        assert!(!directory.exists());
    }

    #[test]
    fn removed_selection_storage_and_missing_file_do_not_poison_future_opens() {
        let source = crate::runtime::cache::temporary_directory("mn-grant-recovery-test-").unwrap();
        let image = source.path().join("image.jpg");
        let markdown = source.path().join("other.md");
        fs::write(&image, b"test image").unwrap();
        fs::write(&markdown, "# Next").unwrap();
        let mut grants = SelectionDirectory::new().unwrap();
        let old = grants.grant(&image).unwrap();
        // This is our isolated test-owned directory, never a shared cache root.
        fs::remove_dir_all(grants.path()).unwrap();
        let next = grants.grant(&image).unwrap();
        assert_ne!(next, old);
        assert_eq!(resolve(Some(grants.path()), &next).unwrap(), image.canonicalize().unwrap());
        assert!(resolve(Some(grants.path()), &old).is_err());
        fs::remove_file(&image).unwrap();
        assert!(
            grants
                .grant(&image)
                .unwrap_err()
                .to_string()
                .contains("Selected file is unavailable")
        );
        let next = grants.grant(&markdown).unwrap();
        assert_eq!(resolve(Some(grants.path()), &next).unwrap(), markdown.canonicalize().unwrap());
    }

    #[test]
    fn file_grants_bound_record_reads_and_selection_count() {
        let source = crate::runtime::cache::temporary_directory("mn-grant-limits-").unwrap();
        let mut grants = SelectionDirectory::new().unwrap();
        let mut first = String::new();
        for index in 0..MAX_GRANTS {
            let file = source.path().join(format!("{index}.md"));
            fs::write(&file, "# Test").unwrap();
            let handle = grants.grant(&file).unwrap();
            if index == 0 {
                first = handle;
            }
        }
        let extra = source.path().join("extra.md");
        fs::write(&extra, "# Extra").unwrap();
        assert!(grants.grant(&extra).is_err());
        let record = grants
            .path()
            .join(format!("{}.json", first.strip_prefix(PREFIX).unwrap()));
        fs::write(record, vec![b' '; MAX_RECORD as usize + 1]).unwrap();
        assert!(resolve(Some(grants.path()), &first).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn granted_paths_cannot_be_redirected_to_unselected_files() {
        let source = crate::runtime::cache::temporary_directory("mn-grant-symlink-").unwrap();
        let selected = source.path().join("selected.md");
        let other = source.path().join("other.md");
        fs::write(&selected, "# Selected").unwrap();
        fs::write(&other, "# Unselected").unwrap();
        let mut grants = SelectionDirectory::new().unwrap();
        let handle = grants.grant(&selected).unwrap();
        fs::remove_file(&selected).unwrap();
        std::os::unix::fs::symlink(&other, &selected).unwrap();
        assert!(resolve(Some(grants.path()), &handle).is_err());
        assert_eq!(fs::read_to_string(other).unwrap(), "# Unselected");
    }
}
