use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use zip::ZipArchive;

pub const MAX_EPUB_BYTES: u64 = 128 * 1024 * 1024;
const MAX_EPUB_ENTRIES: usize = 10_000;
const MAX_EPUB_ENTRY_BYTES: u64 = 64 * 1024 * 1024;
const MAX_EPUB_EXPANDED_BYTES: u64 = 512 * 1024 * 1024;
const MAX_CONTAINER_XML_BYTES: u64 = 1024 * 1024;

#[derive(Debug)]
pub struct EpubAsset {
    pub bytes: Vec<u8>,
    pub hash: String,
    pub size: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadingState {
    pub root: String,
    pub path: String,
    pub book_hash: String,
    #[serde(default)]
    pub cfi: String,
    #[serde(default)]
    pub chapter_label: String,
    #[serde(default)]
    pub progress: f64,
    #[serde(default = "default_font_scale")]
    pub font_scale: f64,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_flow")]
    pub flow: String,
    #[serde(default)]
    pub toc_open: bool,
    #[serde(default)]
    pub updated_at: i64,
}

pub fn initialize(path: &Path) -> Result<(), String> {
    connection(path).map(|_| ())
}

pub fn load(path: &Path) -> Result<EpubAsset, String> {
    let metadata =
        fs::metadata(path).map_err(|error| format!("unable to inspect EPUB: {error}"))?;
    if !metadata.is_file() {
        return Err("EPUB path is not a regular file".to_owned());
    }
    if metadata.len() == 0 || metadata.len() > MAX_EPUB_BYTES {
        return Err("EPUB must be between 1 byte and 128 MiB".to_owned());
    }
    validate_archive(path)?;
    let bytes = fs::read(path).map_err(|error| format!("unable to read EPUB: {error}"))?;
    Ok(EpubAsset {
        hash: stable_content_hash(&bytes),
        size: bytes.len() as u64,
        bytes,
    })
}

pub fn find_state(
    database: &Path,
    canonical_book_path: &Path,
    book_hash: &str,
) -> Result<Option<ReadingState>, String> {
    validate_hash(book_hash)?;
    let path = canonical_book_path.to_string_lossy();
    connection(database)?
        .query_row(
            "SELECT root, path, book_hash, cfi, chapter_label, progress, font_scale,
                    theme, flow, toc_open, updated_at
               FROM epub_reading_state
              WHERE canonical_path = ?1 AND book_hash = ?2",
            params![path.as_ref(), book_hash],
            |row| {
                Ok(ReadingState {
                    root: row.get(0)?,
                    path: row.get(1)?,
                    book_hash: row.get(2)?,
                    cfi: row.get(3)?,
                    chapter_label: row.get(4)?,
                    progress: row.get(5)?,
                    font_scale: row.get(6)?,
                    theme: row.get(7)?,
                    flow: row.get(8)?,
                    toc_open: row.get::<_, i64>(9)? != 0,
                    updated_at: row.get(10)?,
                })
            },
        )
        .optional()
        .map_err(sql_error)
}

pub fn save_state(
    database: &Path,
    canonical_book_path: &Path,
    mut state: ReadingState,
) -> Result<ReadingState, String> {
    validate_state(&state)?;
    state.updated_at = now_epoch_seconds()?;
    let canonical_path = canonical_book_path.to_string_lossy();
    connection(database)?
        .execute(
            "INSERT INTO epub_reading_state (
                 canonical_path, root, path, book_hash, cfi, chapter_label, progress,
                 font_scale, theme, flow, toc_open, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(canonical_path, book_hash) DO UPDATE SET
                 root = excluded.root,
                 path = excluded.path,
                 cfi = excluded.cfi,
                 chapter_label = excluded.chapter_label,
                 progress = excluded.progress,
                 font_scale = excluded.font_scale,
                 theme = excluded.theme,
                 flow = excluded.flow,
                 toc_open = excluded.toc_open,
                 updated_at = excluded.updated_at",
            params![
                canonical_path.as_ref(),
                state.root,
                state.path,
                state.book_hash,
                state.cfi,
                state.chapter_label,
                state.progress,
                state.font_scale,
                state.theme,
                state.flow,
                i64::from(state.toc_open),
                state.updated_at,
            ],
        )
        .map_err(sql_error)?;
    Ok(state)
}

fn validate_archive(path: &Path) -> Result<(), String> {
    let file = File::open(path).map_err(|error| format!("unable to open EPUB: {error}"))?;
    let mut archive =
        ZipArchive::new(file).map_err(|error| format!("invalid EPUB ZIP: {error}"))?;
    if archive.is_empty() || archive.len() > MAX_EPUB_ENTRIES {
        return Err(format!(
            "EPUB contains too many entries (maximum {MAX_EPUB_ENTRIES})"
        ));
    }
    let mut expanded = 0_u64;
    let mut container_name = None;
    let mut has_encryption = false;
    for index in 0..archive.len() {
        let entry = archive
            .by_index_raw(index)
            .map_err(|error| format!("unable to inspect EPUB entry: {error}"))?;
        if entry.encrypted() {
            return Err("encrypted or DRM-protected EPUB files are not supported".to_owned());
        }
        if entry.enclosed_name().is_none() {
            return Err("EPUB contains an unsafe archive path".to_owned());
        }
        if entry.size() > MAX_EPUB_ENTRY_BYTES {
            return Err("EPUB contains an entry larger than 64 MiB".to_owned());
        }
        expanded = expanded
            .checked_add(entry.size())
            .ok_or_else(|| "EPUB expanded size is invalid".to_owned())?;
        if expanded > MAX_EPUB_EXPANDED_BYTES {
            return Err("EPUB expanded data exceeds 512 MiB".to_owned());
        }
        let normalized = entry.name().replace('\\', "/");
        if normalized.eq_ignore_ascii_case("META-INF/container.xml") {
            container_name = Some(entry.name().to_owned());
        }
        has_encryption |= normalized.eq_ignore_ascii_case("META-INF/encryption.xml");
    }
    let container_name =
        container_name.ok_or_else(|| "EPUB does not contain META-INF/container.xml".to_owned())?;
    if has_encryption {
        return Err("encrypted or DRM-protected EPUB files are not supported".to_owned());
    }
    let container = archive
        .by_name(&container_name)
        .map_err(|_| "EPUB container metadata is unavailable".to_owned())?;
    if container.size() == 0 || container.size() > MAX_CONTAINER_XML_BYTES {
        return Err("EPUB container metadata is invalid".to_owned());
    }
    let mut xml = String::new();
    container
        .take(MAX_CONTAINER_XML_BYTES + 1)
        .read_to_string(&mut xml)
        .map_err(|_| "EPUB container metadata is not valid UTF-8 XML".to_owned())?;
    if !xml.contains("rootfile") || !xml.contains("full-path") {
        return Err("EPUB container does not identify a package document".to_owned());
    }
    Ok(())
}

fn connection(path: &Path) -> Result<Connection, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let connection = Connection::open(path).map_err(sql_error)?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(sql_error)?;
    connection
        .execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS epub_reading_state (
                 canonical_path TEXT NOT NULL,
                 root TEXT NOT NULL,
                 path TEXT NOT NULL,
                 book_hash TEXT NOT NULL,
                 cfi TEXT NOT NULL DEFAULT '',
                 chapter_label TEXT NOT NULL DEFAULT '',
                 progress REAL NOT NULL DEFAULT 0,
                 font_scale REAL NOT NULL DEFAULT 1,
                 theme TEXT NOT NULL DEFAULT 'light',
                 flow TEXT NOT NULL DEFAULT 'paginated',
                 toc_open INTEGER NOT NULL DEFAULT 0,
                 updated_at INTEGER NOT NULL,
                 PRIMARY KEY (canonical_path, book_hash)
             );
             CREATE INDEX IF NOT EXISTS epub_reading_state_updated
                 ON epub_reading_state(updated_at DESC);",
        )
        .map_err(sql_error)?;
    Ok(connection)
}

fn validate_state(state: &ReadingState) -> Result<(), String> {
    validate_hash(&state.book_hash)?;
    if state.root.len() > 4096 || state.path.len() > 4096 {
        return Err("EPUB path is too long".to_owned());
    }
    if state.cfi.len() > 8192 || state.chapter_label.len() > 1024 {
        return Err("EPUB reading position is too large".to_owned());
    }
    if !state.progress.is_finite() || !(0.0..=1.0).contains(&state.progress) {
        return Err("EPUB progress must be between 0 and 1".to_owned());
    }
    if !state.font_scale.is_finite() || !(0.75..=2.0).contains(&state.font_scale) {
        return Err("EPUB font scale must be between 0.75 and 2".to_owned());
    }
    if !matches!(state.theme.as_str(), "light" | "sepia" | "dark") {
        return Err("unknown EPUB paper theme".to_owned());
    }
    if !matches!(state.flow.as_str(), "paginated" | "scrolled") {
        return Err("unknown EPUB reading flow".to_owned());
    }
    Ok(())
}

fn validate_hash(value: &str) -> Result<(), String> {
    if value.len() != 24
        || !value.starts_with("fnv1a64:")
        || !value[8..].bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("invalid EPUB content hash".to_owned());
    }
    Ok(())
}

fn stable_content_hash(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}

fn default_font_scale() -> f64 {
    1.0
}

fn default_theme() -> String {
    "light".to_owned()
}

fn default_flow() -> String {
    "paginated".to_owned()
}

fn now_epoch_seconds() -> Result<i64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .map_err(|error| error.to_string())
}

fn sql_error(error: rusqlite::Error) -> String {
    format!("EPUB reading database error: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;

    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    fn temp_path(label: &str, extension: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "codex-thread-studio-{label}-{}.{}",
            uuid::Uuid::new_v4(),
            extension
        ))
    }

    fn write_epub(path: &Path) {
        let output = File::create(path).expect("create EPUB fixture");
        let mut archive = ZipWriter::new(output);
        let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        archive.start_file("mimetype", stored).unwrap();
        archive.write_all(b"application/epub+zip").unwrap();
        archive
            .start_file("META-INF/container.xml", stored)
            .unwrap();
        archive.write_all(br#"<?xml version="1.0"?><container><rootfiles><rootfile full-path="EPUB/book.opf"/></rootfiles></container>"#).unwrap();
        archive.start_file("EPUB/book.opf", stored).unwrap();
        archive.write_all(b"<package/>").unwrap();
        archive.finish().unwrap();
    }

    #[test]
    fn validates_a_bounded_epub_and_hashes_its_bytes() {
        let path = temp_path("epub-valid", "epub");
        write_epub(&path);
        let asset = load(&path).expect("load valid EPUB");
        assert!(asset.hash.starts_with("fnv1a64:"));
        assert_eq!(asset.size, asset.bytes.len() as u64);
        fs::remove_file(path).ok();
    }

    #[test]
    fn rejects_a_zip_without_epub_container_metadata() {
        let path = temp_path("epub-invalid", "epub");
        let output = File::create(&path).unwrap();
        let mut archive = ZipWriter::new(output);
        archive
            .start_file("README.txt", SimpleFileOptions::default())
            .unwrap();
        archive.write_all(b"not an EPUB").unwrap();
        archive.finish().unwrap();
        assert!(load(&path).unwrap_err().contains("container.xml"));
        fs::remove_file(path).ok();
    }

    #[test]
    fn reading_state_is_versioned_by_book_hash_and_validated() {
        let database = temp_path("epub-state", "sqlite3");
        let book = PathBuf::from("/tmp/book.epub");
        initialize(&database).unwrap();
        let state = ReadingState {
            root: "/tmp".to_owned(),
            path: "book.epub".to_owned(),
            book_hash: "fnv1a64:0123456789abcdef".to_owned(),
            cfi: "epubcfi(/6/2!/4/2/1:0)".to_owned(),
            chapter_label: "Chapter 1".to_owned(),
            progress: 0.25,
            font_scale: 1.1,
            theme: "sepia".to_owned(),
            flow: "paginated".to_owned(),
            toc_open: true,
            updated_at: 0,
        };
        let stored = save_state(&database, &book, state.clone()).unwrap();
        assert!(stored.updated_at > 0);
        assert_eq!(
            find_state(&database, &book, &state.book_hash)
                .unwrap()
                .unwrap()
                .cfi,
            state.cfi
        );
        assert!(find_state(&database, &book, "fnv1a64:ffffffffffffffff")
            .unwrap()
            .is_none());
        fs::remove_file(database).ok();
    }
}
