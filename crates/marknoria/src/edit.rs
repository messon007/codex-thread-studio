use crate::{
    Index, MAX_SOURCE,
    source::{SourceWindow, read_range, read_source},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{self, BufWriter, Read, Seek, SeekFrom, Write},
    path::Path,
};

pub const MAX_PAGE: usize = 512 * 1024;
pub const MAX_CHANGES: usize = 2048;

pub struct Snapshot {
    pub directory: tempfile::TempDir,
    pub index: Index,
    newline: &'static str,
}

#[derive(Deserialize, Serialize)]
pub struct Change {
    pub from: usize,
    pub to: usize,
    pub insert: String,
}

#[derive(Deserialize)]
pub struct Save {
    pub revision: String,
    pub start: usize,
    pub end: usize,
    pub changes: Vec<Change>,
}

pub struct Commit {
    pub snapshot: Snapshot,
    pub window: SourceWindow,
    pub warning: Option<String>,
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}
fn conflict(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::WouldBlock, message)
}
fn digest(hash: Sha256) -> String {
    hash.finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn open(path: &Path, write: bool) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(write);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(invalid("Choose a regular file"));
    }
    Ok(file)
}

fn hash_file(file: &mut File) -> io::Result<String> {
    file.seek(SeekFrom::Start(0))?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    let mut total = 0;
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total += count as u64;
        if total > MAX_SOURCE {
            return Err(invalid("File exceeds 128 MiB"));
        }
        hash.update(&buffer[..count]);
    }
    Ok(digest(hash))
}

fn copy_snapshot(file: &mut File, directory: tempfile::TempDir) -> io::Result<Snapshot> {
    file.seek(SeekFrom::Start(0))?;
    let mut source = BufWriter::new(File::create(directory.path().join("source.md"))?);
    let mut lines = BufWriter::new(File::create(directory.path().join("lines.bin"))?);
    lines.write_all(&0u64.to_le_bytes())?;
    let mut count = 1;
    let mut total = 0;
    let mut hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    let mut pending = Vec::new();
    let mut previous_cr = false;
    let mut newline = None;
    loop {
        let size = file.read(&mut buffer)?;
        if size == 0 {
            break;
        }
        if total + size > MAX_SOURCE as usize {
            return Err(invalid("File exceeds 128 MiB"));
        }
        pending.extend_from_slice(&buffer[..size]);
        let valid = match std::str::from_utf8(&pending) {
            Ok(_) => pending.len(),
            Err(error) if error.error_len().is_none() => error.valid_up_to(),
            Err(_) => return Err(invalid("Editing requires UTF-8 text")),
        };
        pending.drain(..valid);
        if buffer[..size].contains(&0) {
            return Err(invalid("NUL bytes cannot be edited"));
        }
        for (position, byte) in buffer[..size].iter().enumerate() {
            if previous_cr && *byte != b'\n' {
                return Err(invalid(
                    "Editing supports LF and CRLF, not lone CR line endings",
                ));
            }
            if *byte == b'\n' {
                newline.get_or_insert(if previous_cr { "\r\n" } else { "\n" });
                lines.write_all(&((total + position + 1) as u64).to_le_bytes())?;
                count += 1;
            }
            previous_cr = *byte == b'\r';
        }
        source.write_all(&buffer[..size])?;
        hash.update(&buffer[..size]);
        total += size;
    }
    if !pending.is_empty() {
        return Err(invalid("Incomplete UTF-8 text"));
    }
    if previous_cr {
        return Err(invalid(
            "Editing supports LF and CRLF, not lone CR line endings",
        ));
    }
    source.flush()?;
    lines.flush()?;
    Ok(Snapshot {
        directory,
        newline: newline.unwrap_or("\n"),
        index: Index {
            version: 1,
            source_bytes: total,
            source_lines: count,
            revision: digest(hash),
            ..Index::default()
        },
    })
}

pub fn snapshot(path: &Path, directory: tempfile::TempDir) -> io::Result<Snapshot> {
    let mut file = open(path, false)?;
    file.try_lock_shared()
        .map_err(|_| conflict("File is locked by another writer"))?;
    let snapshot = copy_snapshot(&mut file, directory)?;
    if hash_file(&mut file)? != snapshot.index.revision {
        return Err(conflict("File changed while opening the editor; reopen it"));
    }
    Ok(snapshot)
}

impl Snapshot {
    pub fn range(&self, start: usize, end: usize) -> io::Result<SourceWindow> {
        let mut result = read_range(self.directory.path(), &self.index, start, end)?;
        result.text = result.text.replace("\r\n", "\n");
        Ok(result)
    }

    pub fn page(
        &self,
        line: Option<usize>,
        offset: Option<usize>,
        previous: bool,
    ) -> io::Result<SourceWindow> {
        let result = read_source(self.directory.path(), &self.index, line, offset, previous)?;
        let mut end = result.end;
        if end < self.index.source_bytes && result.text.ends_with('\r') {
            let mut file = File::open(self.directory.path().join("source.md"))?;
            file.seek(SeekFrom::Start(end as u64))?;
            let mut byte = [0];
            file.read_exact(&mut byte)?;
            if byte[0] == b'\n' {
                end += 1;
            }
        }
        self.range(result.start, end)
    }
}

fn byte_positions(text: &str, positions: &[usize]) -> io::Result<Vec<usize>> {
    let mut result = Vec::with_capacity(positions.len());
    let mut position = 0;
    let mut utf16 = 0;
    let mut chars = text.char_indices().peekable();
    for target in positions {
        while utf16 < *target {
            let Some((offset, character)) = chars.next() else {
                return Err(invalid("Edit position exceeds the loaded page"));
            };
            position = offset + character.len_utf8();
            if character == '\r' && chars.peek().is_some_and(|(_, next)| *next == '\n') {
                position = chars.next().unwrap().0 + 1;
            }
            utf16 += character.len_utf16();
        }
        if utf16 != *target {
            return Err(invalid("Edit position splits a Unicode character"));
        }
        result.push(position);
    }
    Ok(result)
}

pub fn save(
    current: &Snapshot,
    path: &Path,
    request: &Save,
    directory: tempfile::TempDir,
) -> io::Result<Commit> {
    if request.revision != current.index.revision {
        return Err(conflict(
            "Saved revision changed; reload instead of overwriting it",
        ));
    }
    if request.changes.len() > MAX_CHANGES
        || request.start > request.end
        || request.end > current.index.source_bytes
        || request.end - request.start > MAX_PAGE
    {
        return Err(invalid("Edit exceeds the current page limit"));
    }
    let raw = read_range(
        current.directory.path(),
        &current.index,
        request.start,
        request.end,
    )?;
    let mut positions = Vec::new();
    let mut previous = 0;
    let mut inserted = 0;
    for change in &request.changes {
        if change.from < previous || change.to < change.from || change.insert.contains(['\r', '\0'])
        {
            return Err(invalid(
                "Edits must be ordered, non-overlapping normalized text",
            ));
        }
        positions.extend([change.from, change.to]);
        previous = change.to;
        inserted += change.insert.len();
        if inserted > MAX_PAGE {
            return Err(invalid("Inserted text exceeds 512 KiB"));
        }
    }
    let positions = byte_positions(&raw.text, &positions)?;
    let newline = raw
        .text
        .find('\n')
        .map(|position| {
            if position > 0 && raw.text.as_bytes()[position - 1] == b'\r' {
                "\r\n"
            } else {
                "\n"
            }
        })
        .unwrap_or(current.newline);
    let mut replacement = Vec::new();
    let mut cursor = 0;
    for (change, position) in request.changes.iter().zip(positions.chunks_exact(2)) {
        replacement.extend_from_slice(&raw.text.as_bytes()[cursor..position[0]]);
        replacement.extend_from_slice(change.insert.replace('\n', newline).as_bytes());
        cursor = position[1];
        if replacement.len() > MAX_PAGE {
            return Err(invalid("Edited page exceeds 512 KiB"));
        }
    }
    replacement.extend_from_slice(&raw.text.as_bytes()[cursor..]);
    if replacement.len() > MAX_PAGE {
        return Err(invalid("Edited page exceeds 512 KiB"));
    }
    let total = current.index.source_bytes - (request.end - request.start) + replacement.len();
    if total > MAX_SOURCE as usize {
        return Err(invalid("Edited document would exceed 128 MiB"));
    }
    let mut original = open(path, true)?;
    original
        .try_lock()
        .map_err(|_| conflict("File is locked by another writer"))?;
    let metadata = original.metadata()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() != 1
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o7000 != 0
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Editing linked, differently owned or special-permission files is not supported",
            ));
        }
    }
    if path.canonicalize()? != path || hash_file(&mut original)? != current.index.revision {
        return Err(conflict(
            "The file changed outside MarkNoria; your unsaved page has been kept",
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| invalid("Missing file directory"))?;
    let mut temporary = tempfile::Builder::new()
        .prefix(".marknoria-save-")
        .tempfile_in(parent)?;
    let mut source = File::open(current.directory.path().join("source.md"))?;
    io::copy(
        &mut Read::by_ref(&mut source).take(request.start as u64),
        temporary.as_file_mut(),
    )?;
    temporary.write_all(&replacement)?;
    source.seek(SeekFrom::Start(request.end as u64))?;
    io::copy(&mut source, temporary.as_file_mut())?;
    temporary
        .as_file()
        .set_permissions(metadata.permissions())?;
    temporary.as_file().sync_all()?;
    let next = copy_snapshot(temporary.as_file_mut(), directory)?;
    let window = next.range(request.start, request.start + replacement.len())?;
    let live = open(path, false)?;
    if hash_file(&mut original)? != current.index.revision
        || path.canonicalize()? != path
        || live.metadata()?.modified()? != metadata.modified()?
    {
        return Err(conflict(
            "The file changed during saving; the original was not replaced",
        ));
    }
    #[cfg(not(windows))]
    {
        let mut verify = live.try_clone()?;
        if hash_file(&mut verify)? != current.index.revision {
            return Err(conflict("The file changed during saving"));
        }
    }
    let original_identity = same_file::Handle::from_file(original.try_clone()?)?;
    let live_identity = same_file::Handle::from_file(live)?;
    if original_identity != live_identity {
        return Err(conflict("The file was replaced during saving"));
    }
    temporary.persist(path).map_err(|error| error.error)?;
    let warning = if cfg!(unix) {
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .err()
            .map(|error| format!("Saved, but directory sync failed: {error}"))
    } else {
        None
    };
    Ok(Commit {
        snapshot: next,
        window,
        warning,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn cache(root: &Path) -> tempfile::TempDir {
        tempfile::tempdir_in(root).unwrap()
    }

    fn apply(
        path: &Path,
        current: &Snapshot,
        start: usize,
        end: usize,
        changes: Vec<Change>,
    ) -> io::Result<Commit> {
        save(
            current,
            &path.canonicalize()?,
            &Save {
                revision: current.index.revision.clone(),
                start,
                end,
                changes,
            },
            cache(path.parent().unwrap()),
        )
    }

    #[test]
    fn unicode_changes_preserve_bom_mixed_newlines_and_final_line() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("test.md");
        let original = "\u{feff}A😀B\r\nC\nD\r\nlast";
        fs::write(&path, original).unwrap();
        let current = snapshot(&path, cache(root.path())).unwrap();
        let commit = apply(
            &path,
            &current,
            0,
            original.len(),
            vec![Change {
                from: 2,
                to: 4,
                insert: "中文\nnew".into(),
            }],
        )
        .unwrap();
        let expected = "\u{feff}A中文\r\nnewB\r\nC\nD\r\nlast";
        assert_eq!(fs::read_to_string(&path).unwrap(), expected);
        assert_eq!(commit.window.text, expected.replace("\r\n", "\n"));
        assert_eq!(commit.snapshot.index.source_bytes, expected.len());
        assert!(apply(&path, &current, 0, original.len(), vec![]).is_err());
    }

    #[test]
    fn external_changes_locks_and_malformed_patches_do_not_overwrite() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("test.md");
        fs::write(&path, "😀abc\n").unwrap();
        let current = snapshot(&path, cache(root.path())).unwrap();
        for changes in [
            vec![Change {
                from: 1,
                to: 1,
                insert: "bad".into(),
            }],
            vec![Change {
                from: 99,
                to: 99,
                insert: "bad".into(),
            }],
            vec![
                Change {
                    from: 2,
                    to: 4,
                    insert: "".into(),
                },
                Change {
                    from: 3,
                    to: 5,
                    insert: "bad".into(),
                },
            ],
            vec![Change {
                from: 0,
                to: 0,
                insert: "bad\rtext".into(),
            }],
            vec![Change {
                from: 0,
                to: 0,
                insert: "x".repeat(MAX_PAGE + 1),
            }],
        ] {
            assert!(apply(&path, &current, 0, current.index.source_bytes, changes).is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), "😀abc\n");
        }
        let lock = File::open(&path).unwrap();
        lock.try_lock().unwrap();
        assert!(snapshot(&path, cache(root.path())).is_err());
        assert!(apply(&path, &current, 0, 0, vec![]).is_err());
        drop(lock);
        fs::write(&path, "external\n").unwrap();
        assert!(apply(&path, &current, 0, 0, vec![]).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "external\n");
        assert!(!fs::read_dir(root.path()).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".marknoria-save-")
        }));
    }

    #[test]
    fn bounded_pages_preserve_long_unicode_lines_and_crlf_boundaries() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("test.md");
        let original = format!(
            "{}\r\n{}\r\nend",
            "a".repeat(crate::source::SOURCE_BYTES - 1),
            "😀中".repeat(80_000)
        );
        fs::write(&path, &original).unwrap();
        let current = snapshot(&path, cache(root.path())).unwrap();
        let mut offset = 0;
        let mut combined = String::new();
        while offset < original.len() {
            let page = current.page(None, Some(offset), false).unwrap();
            assert!(page.end > offset);
            assert!(page.text.len() <= crate::source::SOURCE_BYTES + 1);
            assert!(!page.text.contains('\r'));
            if offset > 0 {
                let previous = current.page(None, Some(offset), true).unwrap();
                assert_eq!(previous.end, offset);
                assert!(previous.start < offset);
            }
            offset = page.end;
            combined.push_str(&page.text);
        }
        assert_eq!(combined, original.replace("\r\n", "\n"));
    }

    #[test]
    fn streaming_middle_save_keeps_other_pages_and_file_permissions() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("test.md");
        let original = "中文 😀 long document\r\n".repeat(100_000);
        fs::write(&path, &original).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        }
        let current = snapshot(&path, cache(root.path())).unwrap();
        let page = current.page(Some(50_000), None, false).unwrap();
        let commit = apply(
            &path,
            &current,
            page.start,
            page.end,
            vec![Change {
                from: 0,
                to: 0,
                insert: "Inserted\n".into(),
            }],
        )
        .unwrap();
        let saved = fs::read_to_string(&path).unwrap();
        assert_eq!(
            saved,
            format!(
                "{}Inserted\r\n{}",
                &original[..page.start],
                &original[page.start..]
            )
        );
        assert_eq!(commit.window.text, format!("Inserted\n{}", page.text));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o640
            );
            let linked = root.path().join("linked.md");
            fs::hard_link(&path, &linked).unwrap();
            assert!(apply(&path, &commit.snapshot, 0, 0, vec![]).is_err());
            let symlink = root.path().join("symlink.md");
            std::os::unix::fs::symlink(&path, &symlink).unwrap();
            assert!(snapshot(&symlink, cache(root.path())).is_err());
        }
    }

    #[test]
    fn empty_files_and_deleting_final_line_keep_valid_snapshots() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("empty.md");
        fs::write(&path, "").unwrap();
        let current = snapshot(&path, cache(root.path())).unwrap();
        let first = apply(
            &path,
            &current,
            0,
            0,
            vec![Change {
                from: 0,
                to: 0,
                insert: "Hello 😀".into(),
            }],
        )
        .unwrap();
        let last = apply(
            &path,
            &first.snapshot,
            0,
            first.window.end,
            vec![Change {
                from: 0,
                to: 8,
                insert: String::new(),
            }],
        )
        .unwrap();
        assert_eq!(last.window.text, "");
        assert_eq!(last.snapshot.index.source_lines, 1);
        assert_eq!(fs::read(&path).unwrap().len(), 0);
    }

    #[test]
    fn failure_building_saved_snapshot_leaves_original_untouched() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("test.md");
        fs::write(&path, "original\n").unwrap();
        let current = snapshot(&path, cache(root.path())).unwrap();
        let failed = cache(root.path());
        fs::create_dir(failed.path().join("source.md")).unwrap();
        let request = Save {
            revision: current.index.revision.clone(),
            start: 0,
            end: 0,
            changes: vec![Change {
                from: 0,
                to: 0,
                insert: "unsaved\n".into(),
            }],
        };
        assert!(save(&current, &path.canonicalize().unwrap(), &request, failed).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "original\n");
        assert!(!fs::read_dir(root.path()).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".marknoria-save-")
        }));
    }
}
