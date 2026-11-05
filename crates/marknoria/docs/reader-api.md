# Public reader API v1

The stable namespace is `marknoria::reader`. Parsing, source snapshots, tables,
queries, notes and positions do not require HTTP, a desktop toolkit or an AI
account. The [headless host](../examples/reader-host.rs) is an executable consumer:

```sh
cargo run --locked --no-default-features --example reader-host -- public.csv
cargo build --locked --no-default-features --example reader-host
node tests/server/public-core-check.mjs target/debug/examples/reader-host
```

## Authority and lifecycle

The trusted host authorizes the canonical file under its own project/grant policy
before constructing `FileGrant::read_only` or `FileGrant::read_write`. A grant
opens one document, not its parent directory. Read/query access is separate from
source-write permission. Binary formats are not editable. Memory bytes/tables
create an owned immutable input and never grant folder access.

`Reader::open` is synchronous and belongs on a worker, not a UI thread. Use the
owned web service for supervised untrusted parsing. A host executable calls
`reader::worker_entry()` before normal startup so its own supervised index/jq
workers work without another MarkNoria installation. A configured worker path is
trusted startup configuration, not an option accepted from document requests.

Reader descriptor, projection-window, outline, source, search, table/schema,
SQL/jq page, notes, edit/save/refresh and original-byte/asset APIs keep cache paths
private. Projection/outline windows are bounded. `Reader::close` cancels work and
removes owned inputs/results/caches; dropping it does the same. Host-owned note
storage persists. `operations()` returns a cloneable cancellation registry so
cancel/close does not wait for a host mutex held by computation. Register a UUID
before dispatch, retain its operation lease through completion, and check the
cancel result; an unknown ID is not a confirmed cancellation. Registries are
bounded and document-owned, and do not leave background watchdog threads.

`descriptor.assets` counts projected image assets. `asset_info(revision, id)`
returns delivered dimensions, alt text, indexed frame delays/play count and an
optional one-based original PDF page; it exposes no cache filenames. Request the
corresponding bounded PNG bytes with `asset(revision, id, frame)`. Inspecting
metadata does not render lazy PDF pages. Original-container Web PDF/EPUB views
may have no projected assets and use their separate original-byte/resource APIs.
Lazy raster PDF layout estimates are marked `dimensionsExact:false`; ordinary
decoded image previews report exact projected dimensions.
The Web image profile preserves source resolution within the declared pixel,
frame and timeline budgets; the default indexed profile retains bounded previews.
Neither profile silently calls a reduced bitmap original 100%.
Metadata has at most 128 frame entries and 1 KiB of alt text, keeping an encoded
record within 16 KiB. It does not decode image pixels or advance animation.

The explicit Web projection also exposes bounded parser-owned Markdown
`reference`/`reference_target` APIs with original source byte ranges. They do not
grant access to a target file; directory authority and supervised lazy decoding
belong to the [Web reference adapter](markdown-references.md). The default
indexed projection remains unchanged.

`ProjectionPage` additionally carries the original optional `table` identity and
`rowStart`, alongside original byte/line coordinates. This additive v1 metadata
lets ordinary DOM hosts coalesce table fragments without reconstructing source
coordinates or treating a fragment as a complete SQL dataset. The shared Web
[projection component](projection-components.md) accepts these windows directly.

## Revisions, source and positions

Every read/query uses the immutable projection revision. Editing has its own
explicit revision. Save success and re-index success are separate outcomes:
`Saved` reports both revisions, the new edit window and `needsRefresh`; a failed
refresh retains the old projection and does not pretend the file was unsaved.
External changes still fail revision/path/identity checks before replacement.

| Location | Units and base |
| --- | --- |
| `source` | Original input UTF-8 bytes, zero-based, half-open |
| `line` | One-based source line (derived projection text for binary indexing) |
| `projection` | Zero-based projection page and UTF-16 code units in that projection |
| `cell` | Zero-based table, data row and original column; header is not a data row |
| `queryCell` | Transient document-owned result ID and zero-based result coordinates |
| `pdf` | One-based original page; top-left normalized rectangles after crop/rotation |
| `epub` | Full EPUB CFI retained verbatim, with optional internal href |
| `epubChapter` | Original relative archive member href, optionally with a fragment; outline navigation only |

Selection includes session document ID, revision, bounded quote, location and
`exact`/`approximate`/`draft` precision. Browser rectangles are viewport CSS pixels;
the host applies frame offset/transform. They are distinct from PDF page-relative
rectangles. Never infer original UTF-8 offsets from rendered DOM text counts.

`source_selection` converts LF-normalized UTF-16 ranges to exact original UTF-8
boundaries, preserving BOM, CRLF and surrogate pairs. Exact source quotes are
verified against those bytes. Truncated quotes declare that fact. Two sessions
over identical bytes still have different document IDs. `origin` distinguishes a
real file, supplied bytes, and generated CSV representing complete memory-table
input: generated CSV byte offsets are not offsets in a host's original HTML.

Saved positions carry version, stable resource ID and revision. Restoring a stale
position requires an explicit choice and reports it; renderer-dependent PDF/CFI
positions are returned intact rather than replaced by a guessed source page.
DOM-dependent resolution remains the renderer's responsibility. Expiring query
IDs are not accepted as durable positions. `Reader::notes` can resolve an active
`queryCell` into durable `NoteAnchor.query` provenance after checking the result
and quote. It retains SQL/input/result coordinates and null identity, not a
source-cell guess. The resulting note remains readable after result close and
reopening; copying does not execute the saved SQL. `capabilities.queryNotes`
requires explicit note storage and queries. A position can never grant edit permission.

## Query and rendering profiles

File and memory-table queries cover complete accepted input. Over-budget input or
results fail explicitly; visible grid paging is not a query dataset. SQL result
pages declare total rows; jq declares total values/pages. SQL has its own 32 MiB
result cap; jq uses the existing source-scaled 8–256 MiB cap. Descriptor budgets
report the actual document-specific values. Search returns at most 100 matching
lines and declares whether its bounded result is complete, not an unbounded
occurrence count. Source windows are 128 KiB, edits 512 KiB, and original-byte
transport slices at most 512 KiB. The original 128 MiB source limit is retained.

The default view is Preview; Source/Edit require format and host capability.
Memory inputs have no implicit source-write authority. Notes require explicit
storage; file keys preserve existing notebooks and memory identities persist
across reopening the same input. Host note/action adapters are part of the Web
embedding delivery and must obey the same identity/version contract.

HTML projection is **not** permission to insert arbitrary markup into a privileged
host. Use the shared Web reader's sanitizer/CSP, or provide an equivalent reviewed
boundary. The descriptor declares `htmlPolicy: sanitize-at-mount`. Runtime and
grammar crates import no UI toolkit. Keep MSVC UTF-8 C/C++ flags from
`.cargo/config.toml` when consuming the static grammar catalog.

Complete-input plots are exposed through `Reader::chart` and typed
`ChartSource`/`Chart`/`ChartPoint`, with query authority, immutable revisions,
owned cancellation and visible row/transport budgets. See
[table-components.md](table-components.md) for numeric precision, source/query
coordinates and the shared Web component.

CSV/TSV auto-detection retains a single selected delimiter across projection,
complete-input SQL and source auditing. Delimiter detection is bounded to 64
records / 256 KiB; all accepted data records are then processed. Workbook dates
and cached/uncached formulas use the documented shared
[table scalar contract](table-components.md#delimited-input-and-workbook-scalar-compatibility).
