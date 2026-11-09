# Public reader API v1

`Reader::query_snapshot(QuerySnapshotRequest)` resolves a live SQL result cell
into `QuerySnapshot {revision,quote,anchor}` without Notes storage. It shares the
Notes validation path and requires query authority. See the
[durable snapshot contract](notes.md) before saving host references.

The stable namespace is `marknoria::reader`. Parsing, source snapshots, tables,
queries, notes and positions do not require HTTP, a desktop toolkit or an AI
account. The [headless host](../../examples/reader-host.rs) is an executable consumer:

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
belong to the [Web reference adapter](documents.md#markdown-resources-and-links). The default
indexed projection remains unchanged.

`ProjectionPage` additionally carries the original optional `table` identity and
`rowStart`, alongside original byte/line coordinates. This additive v1 metadata
lets ordinary DOM hosts coalesce table fragments without reconstructing source
coordinates or treating a fragment as a complete SQL dataset. The shared Web
[projection component](documents.md#projection-windows) accepts these windows directly.

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
report the actual document-specific values. Source search returns at most 100
occurrences, including repeated matches on one line, and declares whether its
bounded result is complete. Matches include original UTF-8 byte `start`/`end`
coordinates alongside the original line and projection page; formatted display
lines are not source coordinates. Source windows are 128 KiB, edits 512 KiB, and original-byte
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
[Table components](tables.md) for numeric precision, source/query
coordinates and the shared Web component.

CSV/TSV auto-detection retains a single selected delimiter across projection,
complete-input SQL and source auditing. Delimiter detection is bounded to 64
records / 256 KiB; all accepted data records are then processed. Workbook dates
and cached/uncached formulas use the documented shared
[table scalar contract](tables.md#delimited-input-and-workbook-scalar-compatibility).

The optional [comment mark selectors and scoped Notes view](notes.md#comment-markers)
preserve typed source/PDF/EPUB locations. `Reader::notes` accepts `Marks` and `Get`
under the existing Notes grant; display coordinates never authorize source bytes.

## Document search

Standalone controls and embedded hosts share one revision-owned search controller
per reader pane. Hosts use `createReaderClient(...).request(...)`; no child DOM,
private globals, source parsing, or additional file authority is needed. The
parent protocol remains version 1. The additive `capabilities.search.version: 1`
field declares this search contract; clients must check it before using activation
or assuming exact totals.

```js
const reader = createReaderClient(frame)
let state = await reader.ready()
if (state.capabilities.search?.version !== 1) throw Error('Search activation unavailable')
if (state.capabilities.search.matchCase !== true) throw Error('Configurable match case unavailable')
const options = { query: 'needle', matchCase: false }
const first = await reader.request('search', { ...options, offset: 0 })
const next = await reader.request('search', { ...options, offset: 100 })
state = await reader.request('searchActivate', {
  searchId: next.searchId,
  index: next.matches[0].index,
})
console.log(state.search.current + 1, state.search.total, state.search.totalExact)
await reader.request('searchClear')
```

The generic client attaches its current `documentId` and immutable `revision` to
commands. Explicit caller expectations are also accepted and checked. A stale
document/revision fails with `REVISION`; it never silently searches another pane
or a newer saved snapshot. Search grants no new file, storage, or query authority.
`toolbar: 'host'` suppresses the reader toolbar. Public search commands update
state/events and highlights without opening the reader's search form or dropdown.

### Commands and limits

- `search({query, offset?, matchCase?})` starts or pages a search. `query` is a string of at
  most 256 UTF-8 bytes. `offset` defaults to zero and must be a nonnegative safe
  integer. A request returns at most 100 matches. A nonempty unchanged query in
  the same document/revision retains its opaque `searchId`; changing the query
  creates a new identity. Changing the resolved `matchCase` option also creates a
  new identity, cancels pending work and clears old owned results/highlights.
  Searching does not activate or scroll to a result.
- `searchActivate({searchId, index})` activates a zero-based absolute occurrence
  ordinal. It resolves the occurrence from owned results or fetches its bounded
  window. Hosts never provide or override navigation coordinates. The response
  is the whole current reader state, including `search` and reading `position`.
- `searchClear({})` cancels pending search work and navigation, removes owned
  highlights, and invalidates the identity. It returns the reader state.
  `search({query: '', offset: 0})` performs the same clear and returns an empty
  search response with `searchId: null`.

Only the named parameters plus `documentId` and `revision` are accepted. For
example, `page`, `limit`, activation coordinates, null/string/fractional offsets,
and unknown options are rejected instead of being ignored. Unsupported options
or malformed values produce `PARAMS`; unavailable offsets/indices produce
`RANGE`; an old or foreign search identity produces `SEARCH_ID`. The offset equal
to the available result count is a valid empty final window. A greater offset is
rejected. `matchCase` must be a boolean when supplied; null, strings and numbers
are rejected. `true` uses strict case matching, `false` uses locale-independent
Unicode simple case folding. Matching uses original text, never a lowercased
offset map: `k` may match the three-byte `K`, retaining that complete original
range. There is no accent removal, normalization or multi-character expansion
(`ss` does not match `ß`). Queries are literal, not regular expressions.
Source searches retain their nonoverlapping original-line matching; renderers
retain their existing text-boundary semantics.

Omission always resolves `capabilities.search.defaultMatchCase`, preserving
existing behavior: source/EPUB default to `true`, PDF to `false`. It does not use
the previous search's option. Repeat an explicit choice on every window request.
`capabilities.search.matchCase:true` advertises configurable matching. Existing
Rust `search()`/`search_window()` calls and native contracts remain case-sensitive;
the HTTP adapter uses additive configurable helpers and accepts `matchCase`.

The standalone Match case checkbox starts with that document's default, retains
its choice through clear/reopen within the pane, and mirrors a host's active
search option. Changing it reruns nonempty input at offset zero and activates
the first result. Empty input only changes the local checkbox; there is no global
preference or storage grant.

`capabilities.search` declares `queryBytes: 256`, `windowLimit: 100`, `activation`,
`views`, `pagination`, and `resultLimit`. Source-backed search uses
`pagination: 'source'` and `resultLimit: null`: the complete source count is exact,
while only one 100-result window plus one active result is retained. Each requested
window scans the immutable source under its existing 128 MiB limit; no unbounded
match index is materialized. PDF/EPUB use `pagination: 'capped'`, with a bounded
500-result renderer snapshot paged in windows of 100. Their renderer work limits
remain unchanged. Activation is unavailable in Edit; Preview and supported Source
views are declared explicitly.

### Responses and state

A search response contains `version`, `documentId`, `revision`, `searchId`, `query`,
`matchCase`, `offset`, `limit`, `total`, `totalExact`, `available`, `complete`, and `matches`.
It also contains the current search-state fields below.

- `totalExact: true` means `total` is the complete document match count. Otherwise
  `total` is only a lower bound; hosts should show a `+` or an explicit limit label.
- `available` is the number of occurrences that can currently be activated. It is
  the complete count for source-backed documents and the bounded renderer count
  for PDF/EPUB. Hosts must not invent results beyond this value.
- `complete` is true only when the returned window reaches the final result **and**
  the complete document count is known. Thus the first source window of a
  250-match search has `total: 250`, `totalExact: true`, and `complete: false`.
  The last window has 50 results and `complete: true`. A capped renderer search
  can have `total: 500`, `available: 500`, `totalExact: false`, and `complete: false`
  even at the end of its available windows.

Every result has `index`, its absolute zero-based ordinal. Source results retain
`page`, `line`, `snippet`, `tableRow`, `start`, `end`, and `occurrence` (the ordinal
within the original source line, including preceding windows). Their existing
`location` remains a typed line or table-row/cell location, with corresponding
`locationPrecision`. `sourceLocation: {kind:'source', startByte, endByte}` is added
only where the reader exposes original text Source navigation. Generated XLSX or
other container projections are never labeled as original container byte ranges.
PDF pages and EPUB CFIs remain their renderer's typed locations. Renderer-private
text offsets and DOM references stay private; opaque renderer result tokens do
not grant arbitrary coordinate navigation. The stable public occurrence identity
is the pair `{searchId, index}`.

Reader state and `state` events include:

```js
search: {
  version: 1,
  documentId, revision, searchId, query,
  matchCase,                  // resolved boolean; null after clear
  offset,                     // window currently retained by the controller
  current,                    // absolute zero-based ordinal, or null
  total, totalExact, available,
  loading,
  error,                      // null or bounded {code, message}
  active,                     // verified result record, or null
}
```

A clear resets the owner/identity and matchCase to null and counts to zero. The host's reading
`position` remains distinct from the active search occurrence: manually scrolling
away does not change which occurrence was last activated.

### Navigation and cancellation

Preview activation stays in Preview and uses shared highlights and the exact
available projection anchor. Source activation stays in Source and selects the
verified original byte range. Preview/Source changes retain a valid active
occurrence and repaint it when its content is present, without jumping back from
an independently scrolled reading position. Entering Edit clears saved-view search
state. A document/revision replacement invalidates the whole search session.

`request(..., {signal})` cancels only the corresponding operation. A new query,
activation, clear, disposal, or replacement supersedes conflicting pending work.
Cancelled and superseded calls report `CANCELLED`; late fetches and renderer
navigation cannot publish a new active result or restore cleared highlights.
A cancelled activation may be retried against the still-owned query. Pane
identities and renderer result maps are independent. Disposal/remount cannot
reuse an old search identity.

The standalone controls invoke the same controller. Their submit action searches
and then explicitly activates the first result; next/previous wraps within
`available`. Host controls may choose their own presentation and wrap policy while
using the same verified activation operation.
