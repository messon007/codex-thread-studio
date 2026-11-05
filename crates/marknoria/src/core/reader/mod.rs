//! Stable toolkit-free reader facade. Internal cache paths and wire-cache layouts
//! stay private. Hosts authorize one canonical file before constructing its grant.
mod operations;
mod types;
pub use crate::core::edit::{Change, Save as SaveRequest};
pub use crate::core::jq_query::Page as JsonQueryPage;
pub use crate::core::notes::{Anchor as NoteAnchor, Request as NoteRequest, QueryProvenance};
pub use crate::core::source::SourceWindow;
pub use crate::core::table_query::{Schema as TableSchema, Window as QueryWindow, Chart, ChartPoint, ChartSource};
use crate::core::{self, Index, edit, jq_query, source, table_query};
pub use crate::core::document_format::DocumentFormat;
pub use crate::core::{Heading, Match as SearchMatch, TableWindow};
pub use operations::{Operation, Operations};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::Mutex,
};
pub use types::*;
pub use crate::core::{DocumentRenderer, ProjectionProfile};

fn finish_hash(hash: Sha256) -> String {
    hash.finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
pub(crate) fn file_identity(path: &Path) -> String {
    let mut hash = Sha256::new();
    hash.update(path.as_os_str().as_encoded_bytes());
    format!("file:{}", finish_hash(hash))
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
fn denied(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message)
}
fn conflict(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::WouldBlock, message)
}

/// Authority is supplied by trusted host code, never by a browser request body.
/// Canonical-path project/grant policy remains the host's responsibility.
#[derive(Clone, Debug)]
pub struct FileGrant {
    path: PathBuf,
    write: bool,
}
impl FileGrant {
    pub fn read_only(path: impl AsRef<Path>) -> io::Result<Self> {
        Self::new(path.as_ref(), false)
    }
    pub fn read_write(path: impl AsRef<Path>) -> io::Result<Self> {
        Self::new(path.as_ref(), true)
    }
    fn new(path: &Path, write: bool) -> io::Result<Self> {
        let path = path.canonicalize()?;
        let metadata = path.metadata()?;
        if !metadata.is_file() || metadata.len() > core::MAX_SOURCE {
            return Err(invalid(
                "Grant requires one regular document of at most 128 MiB",
            ));
        }
        if DocumentFormat::for_path(&path).is_none() {
            return Err(invalid("Unsupported document format"));
        }
        Ok(Self { path, write })
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn writable(&self) -> bool {
        self.write
    }
}

#[derive(Clone, Debug)]
pub struct Options {
    projection: ProjectionProfile,
    queries: bool,
    notes_directory: Option<PathBuf>,
    cache_root: Option<PathBuf>,
    resource_id: Option<String>,
    worker: Option<PathBuf>,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            projection: ProjectionProfile::Indexed,
            queries: true,
            notes_directory: None,
            cache_root: None,
            resource_id: None,
            worker: None,
        }
    }
}
impl Options {
    pub fn projection(mut self, profile: ProjectionProfile) -> Self {
        self.projection = profile;
        self
    }
    pub fn queries(mut self, enabled: bool) -> Self {
        self.queries = enabled;
        self
    }
    pub fn notes_directory(mut self, path: PathBuf) -> Self {
        self.notes_directory = Some(path);
        self
    }
    pub fn cache_root(mut self, path: PathBuf) -> Self {
        self.cache_root = Some(path);
        self
    }
    pub fn resource_id(mut self, id: String) -> Self {
        self.resource_id = Some(id);
        self
    }
    pub fn worker_executable(mut self, path: PathBuf) -> Self {
        self.worker = Some(path);
        self
    }
    fn temporary(&self, prefix: &str) -> io::Result<tempfile::TempDir> {
        if let Some(root) = &self.cache_root {
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            builder.create(root)?;
            tempfile::Builder::new().prefix(prefix).tempdir_in(root)
        } else {
            crate::runtime::cache::temporary_directory(prefix)
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectionPage {
    pub page: usize,
    pub source_line: usize,
    pub source_start_byte: Option<usize>,
    pub source_end_byte: Option<usize>,
    /// Original table identity, never an index into a selected-column window.
    pub table: Option<usize>,
    pub row_start: usize,
    /// Inert cache projection, requiring the shared reader's DOM sanitizer/CSP.
    /// Do not insert into an arbitrary privileged host without sanitization.
    pub html: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Window {
    pub revision: String,
    pub total_pages: usize,
    pub pages: Vec<ProjectionPage>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outline {
    pub revision: String,
    pub offset: usize,
    pub total: usize,
    pub items: Vec<Heading>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResults {
    pub revision: String,
    pub matches: Vec<SearchMatch>,
    pub complete: bool,
    pub limit: usize,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditWindow {
    pub revision: String,
    pub window: SourceWindow,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Saved {
    pub revision: String,
    pub preview_revision: String,
    pub needs_refresh: bool,
    pub warning: Option<String>,
    pub window: SourceWindow,
    pub source_bytes: usize,
    pub source_lines: usize,
}

/// Synchronous bounded core calls belong on a host worker, not a UI thread.
/// Use the owned web service for supervised parsing of untrusted documents.
pub struct Reader {
    grant: FileGrant,
    options: Options,
    id: String,
    resource_id: String,
    origin: Origin,
    index: Index,
    sql: table_query::Queries,
    jq: jq_query::Queries,
    editor: Option<edit::Snapshot>,
    operations: Operations,
    assets: Mutex<()>,
    cache: Option<tempfile::TempDir>,
    owned_source: Option<tempfile::TempDir>,
}

impl Reader {
    pub fn open(grant: FileGrant, options: Options) -> io::Result<Self> {
        if options
            .resource_id
            .as_ref()
            .is_some_and(|id| id.is_empty() || id.len() > 256 || id.contains('\0'))
        {
            return Err(invalid("Resource identity exceeds its budget"));
        }
        let cache = options.temporary("mn-reader-")?;
        let index = core::build_index_with_profile(&grant.path, cache.path(), options.projection)?;
        let resource_id = options
            .resource_id
            .clone()
            .unwrap_or_else(|| file_identity(&grant.path));
        Ok(Self {
            grant,
            options,
            id: uuid::Uuid::new_v4().to_string(),
            resource_id,
            origin: Origin::File,
            index,
            sql: Default::default(),
            jq: Default::default(),
            editor: None,
            operations: Default::default(),
            assets: Mutex::new(()),
            cache: Some(cache),
            owned_source: None,
        })
    }

    /// Own a bounded immutable input without granting access to a filesystem folder.
    pub fn from_bytes(name: &str, bytes: &[u8], options: Options) -> io::Result<Self> {
        if name.len() > 255
            || name.is_empty()
            || name.contains(['/', '\\', '\0'])
            || matches!(name, "." | "..")
            || bytes.len() > core::MAX_SOURCE as usize
        {
            return Err(invalid("Invalid in-memory document name or byte budget"));
        }
        let source = options.temporary("mn-reader-input-")?;
        let path = source.path().join(name);
        fs::write(&path, bytes)?;
        let mut options = options;
        if options.resource_id.is_none() {
            options.resource_id = Some({
                let mut hash = Sha256::new();
                hash.update(name.as_bytes());
                hash.update([0]);
                hash.update(bytes);
                format!("memory:{}", finish_hash(hash))
            });
        }
        let mut reader = Self::open(FileGrant::read_only(path)?, options)?;
        reader.owned_source = Some(source);
        reader.origin = Origin::Bytes;
        Ok(reader)
    }

    /// Complete plain-text table input. No row truncation; reject over-budget data.
    pub fn from_table(
        columns: &[String],
        rows: &[Vec<String>],
        mut options: Options,
    ) -> io::Result<Self> {
        if columns.is_empty()
            || columns.len() > 256
            || rows.len() > 1_000_000
            || rows.len().saturating_mul(columns.len()) > 2_000_000
        {
            return Err(invalid("Memory table exceeds its row/column/cell budget"));
        }
        let source = options.temporary("mn-reader-table-")?;
        let path = source.path().join("memory.csv");
        let mut bytes = columns
            .iter()
            .try_fold(0usize, |n, cell| n.checked_add(cell.len()))
            .ok_or_else(|| invalid("Memory table header size overflow"))?;
        if bytes > core::MAX_SOURCE as usize {
            return Err(invalid("Memory table header exceeds 128 MiB"));
        }
        let mut writer = csv::Writer::from_path(&path)?;
        writer.write_record(columns)?;
        for row in rows {
            if row.len() != columns.len() {
                return Err(invalid(
                    "Every memory row must match the complete column list",
                ));
            }
            bytes = bytes
                .checked_add(row.iter().map(|c| c.len()).sum::<usize>())
                .ok_or_else(|| invalid("Memory table size overflow"))?;
            if bytes > core::MAX_SOURCE as usize {
                return Err(invalid("Memory table exceeds 128 MiB"));
            }
            writer.write_record(row)?;
        }
        writer.flush()?;
        drop(writer);
        if path.metadata()?.len() > core::MAX_SOURCE {
            return Err(invalid("Encoded memory table exceeds 128 MiB"));
        }
        if options.resource_id.is_none() {
            let mut file = File::open(&path)?;
            let mut hash = Sha256::new();
            let mut chunk = [0; 64 * 1024];
            loop {
                let size = file.read(&mut chunk)?;
                if size == 0 {
                    break;
                }
                hash.update(&chunk[..size]);
            }
            options.resource_id = Some(format!("table:{}", finish_hash(hash)));
        }
        let mut reader = Self::open(FileGrant::read_only(path)?, options)?;
        reader.owned_source = Some(source);
        reader.origin = Origin::MemoryTable;
        Ok(reader)
    }

    fn directory(&self) -> io::Result<&Path> {
        self.cache
            .as_ref()
            .map(|c| c.path())
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "Reader is closed"))
    }
    fn check(&self, revision: &str) -> io::Result<()> {
        self.directory()?;
        if revision != self.index.revision {
            return Err(conflict("Document revision changed"));
        }
        Ok(())
    }
    pub fn operations(&self) -> Operations {
        self.operations.clone()
    }
    pub fn descriptor(&self) -> Descriptor {
        let source = self.index.format.is_text();
        let mut budgets = Budgets::default();
        budgets.jq_result_bytes = jq_query::output_budget(self.index.source_bytes as u64);
        let editing_revision = self.editor.as_ref().map(|s| s.index.revision.clone());
        let needs_refresh = editing_revision
            .as_ref()
            .is_some_and(|r| r != &self.index.revision);
        Descriptor {
            id: self.id.clone(),
            resource_id: self.resource_id.clone(),
            origin: self.origin,
            revision: self.index.revision.clone(),
            name: self.index.name.clone(),
            format: self.index.format,
            renderer: self.index.renderer,
            epub_package: self.index.epub_package.clone(),
            language: core::highlight::language_for_name(&self.index.name),
            file_bytes: self.index.original_bytes.unwrap_or(self.index.source_bytes),
            source_bytes: self.index.source_bytes,
            source_lines: self.index.source_lines,
            projection_pages: self.index.pages.len(),
            tables: self.index.tables.len(),
            outline_items: self.index.toc.len(),
            index_ms: self.index.index_ms,
            peak_rss_kib: self.index.peak_rss_kib,
            capabilities: Capabilities {
                api_version: API_VERSION,
                preview: true,
                source,
                edit: source && self.grant.write,
                charts: self.options.queries && !self.index.tables.is_empty(),
                sql: self.options.queries && !self.index.tables.is_empty(),
                jq: self.options.queries && jq_query::supported(&self.index.name),
                notes: self.options.notes_directory.is_some(),
                query_notes: self.options.notes_directory.is_some() && self.options.queries,
                assets: !self.index.assets.is_empty(),
                original_bytes: true,
                html_policy: "sanitize-at-mount",
                dataset_scope: "complete-input",
                budgets,
            },
            editing_revision,
            needs_refresh,
        }
    }

    pub fn window(&self, revision: &str, start: usize, count: usize) -> io::Result<Window> {
        self.check(revision)?;
        if count == 0
            || count > Budgets::default().max_window_pages
            || (start >= self.index.pages.len() && !(self.index.pages.is_empty() && start == 0))
        {
            return Err(invalid("Projection window exceeds its page budget"));
        }
        let mut pages = Vec::new();
        for page in start..start.saturating_add(count).min(self.index.pages.len()) {
            let metadata = &self.index.pages[page];
            let source = self.index.format.is_text();
            pages.push(ProjectionPage {
                page,
                source_line: metadata.line,
                source_start_byte: source.then_some(metadata.start),
                source_end_byte: source.then_some(metadata.end),
                table: metadata.table,
                row_start: metadata.row_start,
                html: core::read_page(self.directory()?, &self.index, page)?,
            });
        }
        Ok(Window {
            revision: revision.into(),
            total_pages: self.index.pages.len(),
            pages,
        })
    }

    pub fn outline(&self, revision: &str, offset: usize) -> io::Result<Outline> {
        self.check(revision)?;
        if self.index.renderer.is_some() { return Err(io::Error::new(io::ErrorKind::Unsupported, "Outline requires the active container renderer")); }
        if offset > self.index.toc.len() {
            return Err(invalid("Outline offset out of range"));
        }
        Ok(Outline {
            revision: revision.into(),
            offset,
            total: self.index.toc.len(),
            items: self.index.toc[offset
                ..self
                    .index
                    .toc
                    .len()
                    .min(offset + Budgets::default().max_outline_items)]
                .to_vec(),
        })
    }
    pub fn source(
        &self,
        revision: &str,
        line: Option<usize>,
        offset: Option<usize>,
        previous: bool,
    ) -> io::Result<SourceWindow> {
        self.check(revision)?;
        if !self.index.format.is_text() {
            return Err(denied(
                "Original UTF-8 source is unavailable for this binary document",
            ));
        }
        source::read_source(self.directory()?, &self.index, line, offset, previous)
    }

    /// Convert an original-source window's LF-normalized UTF-16 selection into
    /// exact original UTF-8 bytes, including BOM, CRLF and surrogate pairs.
    pub fn source_selection(
        &self,
        revision: &str,
        start: usize,
        end: usize,
        from_utf16: usize,
        to_utf16: usize,
    ) -> io::Result<Selection> {
        self.check(revision)?;
        selection_from_source(
            self.directory()?,
            &self.index,
            &self.id,
            revision,
            start,
            end,
            from_utf16,
            to_utf16,
        )
    }

    pub fn search(&self, revision: &str, query: &str) -> io::Result<SearchResults> {
        self.check(revision)?;
        if self.index.renderer.is_some() { return Err(io::Error::new(io::ErrorKind::Unsupported, "Search requires the active container renderer")); }
        let matches = core::search(self.directory()?, &self.index, query)?;
        let complete = matches.len() < 100;
        Ok(SearchResults {
            revision: revision.into(),
            matches,
            complete,
            limit: 100,
        })
    }
    pub fn table(
        &self,
        revision: &str,
        table: usize,
        row: usize,
        columns: &[usize],
    ) -> io::Result<TableWindow> {
        self.check(revision)?;
        core::read_table(self.directory()?, &self.index, table, row, columns)
    }
    pub fn table_schema(&self, revision: &str, table: usize) -> io::Result<TableSchema> {
        self.check(revision)?;
        table_query::schema(&self.index, table)
    }
    pub fn sql(
        &mut self,
        revision: &str,
        table: usize,
        sql: &str,
        operation: &Operation,
    ) -> io::Result<QueryWindow> {
        self.check(revision)?;
        self.operations.check(operation)?;
        if !self.options.queries {
            return Err(denied("Query capability is disabled"));
        }
        let path = self.directory()?.to_owned();
        self.sql
            .run(&path, &self.index, table, sql, operation.flag.clone())
    }
    pub fn chart(
        &mut self,
        revision: &str,
        source: ChartSource,
        x: Option<usize>,
        y: usize,
        operation: &Operation,
    ) -> io::Result<Chart> {
        self.check(revision)?;
        self.operations.check(operation)?;
        if !self.options.queries {
            return Err(denied("Query capability is disabled"));
        }
        let path = self.directory()?.to_owned();
        self.sql.chart(&path, &self.index, source, x, y, operation.flag.clone())
    }
    pub fn sql_page(&self, revision: &str, id: &str, row: usize) -> io::Result<QueryWindow> {
        self.check(revision)?;
        if !self.options.queries {
            return Err(denied("Query capability is disabled"));
        }
        self.sql.page(&self.index, id, row)
    }
    pub fn close_sql(&mut self, id: &str) {
        self.sql.close(id);
    }
    pub fn jq(
        &mut self,
        revision: &str,
        filter: &str,
        slurp: bool,
        operation: &Operation,
    ) -> io::Result<JsonQueryPage> {
        self.check(revision)?;
        self.operations.check(operation)?;
        if !self.options.queries {
            return Err(denied("Query capability is disabled"));
        }
        let path = self.directory()?.to_owned();
        let worker = match &self.options.worker {
            Some(worker) => worker.clone(),
            None => crate::runtime::process::self_executable()?,
        };
        self.jq
            .run(&path, &self.index, filter, slurp, &worker, &operation.flag)
    }
    pub fn jq_page(&self, revision: &str, id: &str, page: usize) -> io::Result<JsonQueryPage> {
        self.check(revision)?;
        if !self.options.queries {
            return Err(denied("Query capability is disabled"));
        }
        self.jq.page(&self.index, id, page)
    }
    pub fn close_jq(&mut self, id: &str) {
        self.jq.close(id);
    }
    pub fn notes(&self, mut request: core::notes::Request) -> io::Result<serde_json::Value> {
        self.directory()?;
        let path = self
            .options
            .notes_directory
            .as_ref()
            .ok_or_else(|| denied("Note storage was not granted by the host"))?;
        self.sql.prepare_note(&self.index, &mut request)?;
        if let Some(identity) = &self.options.resource_id {
            core::notes::execute_for_identity(path, identity, &self.index, request)
        } else {
            core::notes::execute(path, &self.grant.path, &self.index, request)
        }
    }

    pub fn begin_edit(
        &mut self,
        revision: &str,
        line: Option<usize>,
        offset: Option<usize>,
        previous: bool,
    ) -> io::Result<EditWindow> {
        self.check(revision)?;
        if !self.grant.write || !self.index.format.is_text() {
            return Err(denied(
                "Editing was not explicitly granted for this document",
            ));
        }
        if self.editor.is_none() {
            let snapshot =
                edit::snapshot(&self.grant.path, self.options.temporary("mn-reader-edit-")?)?;
            if snapshot.index.revision != self.index.revision {
                return Err(conflict(
                    "Source changed since indexing; refresh before editing",
                ));
            }
            self.editor = Some(snapshot);
        }
        let editor = self.editor.as_ref().unwrap();
        Ok(EditWindow {
            revision: editor.index.revision.clone(),
            window: editor.page(line, offset, previous)?,
        })
    }

    pub fn edit_window(
        &self,
        revision: &str,
        line: Option<usize>,
        offset: Option<usize>,
        previous: bool,
    ) -> io::Result<EditWindow> {
        self.directory()?;
        let editor = self
            .editor
            .as_ref()
            .ok_or_else(|| invalid("Open an edit session first"))?;
        if revision != editor.index.revision {
            return Err(conflict("Editor revision changed"));
        }
        Ok(EditWindow {
            revision: revision.into(),
            window: editor.page(line, offset, previous)?,
        })
    }
    pub fn save(&mut self, request: &edit::Save) -> io::Result<Saved> {
        self.directory()?;
        if !self.grant.write {
            return Err(denied("Editing was not explicitly granted"));
        }
        let current = self
            .editor
            .as_ref()
            .ok_or_else(|| invalid("Open an edit window before saving"))?;
        let commit = edit::save(
            current,
            &self.grant.path,
            request,
            self.options.temporary("mn-reader-saved-")?,
        )?;
        let revision = commit.snapshot.index.revision.clone();
        let source_bytes = commit.snapshot.index.source_bytes;
        let source_lines = commit.snapshot.index.source_lines;
        self.editor = Some(commit.snapshot);
        Ok(Saved {
            needs_refresh: revision != self.index.revision,
            revision,
            preview_revision: self.index.revision.clone(),
            warning: commit.warning,
            window: commit.window,
            source_bytes,
            source_lines,
        })
    }

    /// A failed re-index leaves the previous immutable projection available;
    /// successful saving is a separate outcome and is never rolled back silently.
    pub fn refresh(&mut self) -> io::Result<Descriptor> {
        self.directory()?;
        let cache = self.options.temporary("mn-reader-refresh-")?;
        let index = core::build_index_with_profile(&self.grant.path, cache.path(), self.options.projection)?;
        self.sql = Default::default();
        self.jq = Default::default();
        self.operations.close();
        self.operations = Default::default();
        self.index = index;
        self.cache = Some(cache);
        self.editor = None;
        Ok(self.descriptor())
    }

    pub fn original_range(
        &self,
        revision: &str,
        start: usize,
        length: usize,
    ) -> io::Result<Vec<u8>> {
        self.check(revision)?;
        if length > edit::MAX_PAGE {
            return Err(invalid("Original byte range exceeds 512 KiB"));
        }
        let path = self.directory()?.join(if self.index.format.is_text() {
            "source.md"
        } else {
            "original.bin"
        });
        let mut file = File::open(path)?;
        let end = start
            .checked_add(length)
            .ok_or_else(|| invalid("Original byte range overflow"))?;
        if end as u64 > file.metadata()?.len() {
            return Err(invalid("Original byte range is out of bounds"));
        }
        file.seek(SeekFrom::Start(start as u64))?;
        let mut bytes = vec![0; length];
        file.read_exact(&mut bytes)?;
        Ok(bytes)
    }

    pub fn epub_resource(&self, revision: &str, member: &str) -> io::Result<(Vec<u8>, &'static str)> {
        self.check(revision)?;
        core::documents::epub_resource(self.directory()?, &self.index, member)
    }

    pub fn asset(&self, revision: &str, id: usize, frame: usize) -> io::Result<Vec<u8>> {
        self.check(revision)?;
        let _guard = self
            .assets
            .lock()
            .map_err(|_| io::Error::other("Asset worker unavailable"))?;
        let path = core::documents::asset(self.directory()?, &self.index, id, frame)?;
        let mut file = File::open(path)?;
        if file.metadata()?.len() > core::MAX_SOURCE {
            return Err(invalid("Projected asset exceeds its transport budget"));
        }
        let mut bytes = Vec::new();
        file.by_ref()
            .take(core::MAX_SOURCE + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > core::MAX_SOURCE as usize {
            return Err(invalid("Projected asset exceeds its transport budget"));
        }
        Ok(bytes)
    }

    pub fn validate_selection(&self, selection: &Selection) -> io::Result<()> {
        selection.validate()?;
        self.check(&selection.revision)?;
        if selection.document_id != self.id {
            return Err(denied("Selection belongs to another document"));
        }
        match &selection.location {
            Location::Source {
                start_byte,
                end_byte,
            } => {
                if !self.index.format.is_text() {
                    return Err(invalid(
                        "Binary positions cannot claim original UTF-8 offsets",
                    ));
                }
                let window =
                    source::read_range(self.directory()?, &self.index, *start_byte, *end_byte)?;
                if selection.precision == Precision::Exact {
                    let original = window.text.replace("\r\n", "\n");
                    let quote = selection.quote.replace("\r\n", "\n");
                    if !(if selection.quote_truncated {
                        original.starts_with(&quote)
                    } else {
                        original == quote
                    }) {
                        return Err(invalid(
                            "Selection quote does not match its exact source bytes",
                        ));
                    }
                }
            }
            Location::Cell { table, row, column } => {
                let table = self
                    .index
                    .tables
                    .get(*table)
                    .ok_or_else(|| invalid("Selection table out of range"))?;
                if *row >= table.rows || *column >= table.columns.len() {
                    return Err(invalid("Selection cell out of range"));
                }
            }
            Location::QueryCell { query_id, row, column } => {
                let window = self.sql_page(&selection.revision, query_id, *row)?;
                if *row >= window.total_rows || *column >= window.columns.len() {
                    return Err(invalid("Selection query cell out of range"));
                }
                if selection.precision == Precision::Exact {
                    let value = window.rows[*row - window.row_start][*column].as_deref().unwrap_or("NULL");
                    if !(if selection.quote_truncated { value.starts_with(&selection.quote) } else { value == selection.quote }) {
                        return Err(invalid("Selection quote does not match its query cell"));
                    }
                }
            }
            Location::Line { line } if *line > self.index.source_lines => {
                return Err(invalid("Selection line out of range"));
            }
            Location::Projection { page, .. } if *page >= self.index.pages.len() => {
                return Err(invalid("Selection page out of range"));
            }
            Location::Pdf { pages } => {
                let count = self
                    .index
                    .assets
                    .iter()
                    .filter_map(|a| a.pdf_page)
                    .max()
                    .map(|p| p + 1)
                    .unwrap_or(0);
                if self.index.format != DocumentFormat::Pdf || pages.iter().any(|p| p.page > 2000) || (self.index.renderer.is_none() && pages.iter().any(|p| p.page > count))
                {
                    return Err(invalid("PDF location is out of range for this document"));
                }
            }
            Location::Epub { .. } | Location::EpubChapter { .. } if self.index.format != DocumentFormat::Epub => {
                return Err(invalid("CFI location requires an EPUB document"));
            }
            _ => (),
        }
        Ok(())
    }

    pub fn reading_position(&self, view: View, location: Location) -> io::Result<ReadingPosition> {
        let position = ReadingPosition {
            version: API_VERSION,
            resource_id: self.resource_id.clone(),
            revision: self.index.revision.clone(),
            view,
            location,
        };
        self.restore_position(&position, false)?;
        Ok(position)
    }

    /// Never overwrite an unresolvable/stale position with a guessed page.
    /// Renderer-dependent positions stay intact for that renderer to resolve.
    pub fn restore_position(
        &self,
        position: &ReadingPosition,
        allow_stale: bool,
    ) -> io::Result<Restoration> {
        self.directory()?;
        if position.version != API_VERSION || !types::valid_revision(&position.revision) {
            return Err(invalid("Unsupported reading-position version"));
        }
        if position.resource_id != self.resource_id {
            return Err(denied("Reading position belongs to another resource"));
        }
        position.location.validate()?;
        let same_revision = position.revision == self.index.revision;
        if !same_revision && !allow_stale {
            return Err(conflict("Saved position is from another document revision"));
        }
        if position.view != View::Preview && !self.index.format.is_text() {
            return Err(denied("Source view is unavailable for this format"));
        }
        if position.view == View::Edit && !self.grant.write {
            return Err(denied("A saved position cannot grant edit permission"));
        }
        match &position.location {
            Location::Source {
                start_byte,
                end_byte,
            } => {
                if !self.index.format.is_text() {
                    return Err(invalid("Original UTF-8 position requires text input"));
                }
                source::read_range(self.directory()?, &self.index, *start_byte, *end_byte)?;
            }
            Location::Line { line } if *line > self.index.source_lines => {
                return Err(invalid("Saved source line out of range"));
            }
            Location::Projection { page, .. } if *page >= self.index.pages.len() => {
                return Err(invalid("Saved projection page out of range"));
            }
            Location::Cell { table, row, column } => {
                let table = self
                    .index
                    .tables
                    .get(*table)
                    .ok_or_else(|| invalid("Saved table out of range"))?;
                if *row >= table.rows || *column >= table.columns.len() {
                    return Err(invalid("Saved table cell out of range"));
                }
            }
            Location::QueryCell { .. } => {
                return Err(invalid(
                    "Transient query IDs cannot be durable reading positions; retain the query definition or a source cell",
                ));
            }
            Location::Pdf { pages } => {
                let count = self
                    .index
                    .assets
                    .iter()
                    .filter_map(|a| a.pdf_page)
                    .max()
                    .map(|p| p + 1)
                    .unwrap_or(0);
                if self.index.format != DocumentFormat::Pdf || pages.iter().any(|p| p.page > 2000) || (self.index.renderer.is_none() && pages.iter().any(|p| p.page > count))
                {
                    return Err(invalid("Saved PDF page out of range"));
                }
            }
            Location::Epub { .. } | Location::EpubChapter { .. } if self.index.format != DocumentFormat::Epub => {
                return Err(invalid("Saved CFI requires an EPUB"));
            }
            _ => (),
        }
        Ok(Restoration {
            position: position.clone(),
            same_revision,
            requires_renderer: matches!(
                &position.location,
                Location::Epub { .. } | Location::EpubChapter { .. } | Location::Pdf { .. } | Location::Projection { .. }
            ),
        })
    }

    pub fn close(mut self) -> io::Result<()> {
        self.dispose()
    }
    fn dispose(&mut self) -> io::Result<()> {
        self.operations.close();
        self.sql = Default::default();
        self.jq = Default::default();
        self.editor = None;
        let cache = self.cache.take().map(tempfile::TempDir::close).transpose();
        let source = self
            .owned_source
            .take()
            .map(tempfile::TempDir::close)
            .transpose();
        cache?;
        source?;
        Ok(())
    }
}
impl Drop for Reader {
    fn drop(&mut self) {
        let _ = self.dispose();
    }
}

/// Call at the host executable's entry point before starting its UI or gateway.
/// The supervised jq worker uses the host executable, not a separate installation.
pub fn worker_entry() -> Option<io::Result<()>> {
    if let Some(result) = core::jq_query::worker_entry() {
        return Some(result);
    }
    let arguments: Vec<_> = std::env::args_os().collect();
    if arguments
        .get(1)
        .is_none_or(|arg| arg != "--marknoria-index")
    {
        return None;
    }
    Some((|| {
        let profile = if arguments.len() == 5 && arguments[4] == "--web-profile" {
            ProjectionProfile::Web
        } else if arguments.len() == 4 {
            ProjectionProfile::Indexed
        } else {
            return Err(invalid(
                "Index worker requires one source and one private output directory",
            ));
        };
        core::build_index_with_profile(Path::new(&arguments[2]), Path::new(&arguments[3]), profile)?;
        Ok(())
    })())
}

// Shared conversion for the core facade and the authenticated HTTP adapter.
pub(crate) fn selection_from_source(
    directory: &Path,
    index: &Index,
    document_id: &str,
    revision: &str,
    start: usize,
    end: usize,
    from_utf16: usize,
    to_utf16: usize,
) -> io::Result<Selection> {
    if revision != index.revision {
        return Err(conflict("Document revision changed"));
    }
    if !index.format.is_text() || from_utf16 > to_utf16 {
        return Err(invalid("Invalid original source selection"));
    }
    let window = source::read_range(directory, &index, start, end)?;
    let offsets = edit::byte_positions(&window.text, &[from_utf16, to_utf16])?;
    let mut quote = window.text[offsets[0]..offsets[1]].replace("\r\n", "\n");
    let quote_truncated = quote.len() > Budgets::default().quote_bytes;
    if quote_truncated {
        let mut end = Budgets::default().quote_bytes;
        while !quote.is_char_boundary(end) {
            end -= 1;
        }
        quote.truncate(end);
    }
    let selection = Selection {
        document_id: document_id.to_owned(),
        revision: revision.into(),
        quote,
        quote_truncated,
        precision: Precision::Exact,
        location: Location::Source {
            start_byte: start + offsets[0],
            end_byte: start + offsets[1],
        },
        viewport_rect: None,
    };
    selection.validate()?;
    Ok(selection)
}
