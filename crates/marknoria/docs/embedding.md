# Single-document Web embedding v1

The public entry is `marknoria::web::{Service, ServiceOptions, DiagramMode}`.
Hosts serving ordinary components can use the static-only
[`web::reader_assets()` router](reader-assets.md), without creating a document
service or granting a file.
The core API is described in [reader-api.md](reader-api.md). Both adapters use
the same source-coordinate conversion, parsers, complete-input query engines and
save validation. Neither requires a privileged host DOM or an AI account.

## Trusted startup

```rust,ignore
let grant = marknoria::reader::FileGrant::read_only(authorized_file)?;
let options = marknoria::web::ServiceOptions::default()
    .queries(true)
    .diagrams(marknoria::web::DiagramMode::Browser)
    .parent_origin("http://127.0.0.1:4000")?;
let service = marknoria::web::Service::start_with_options(grant, options).await?;
let frame_url = service.url();
// Destroy the frame/client, then await cleanup before dropping the host runtime.
service.close().await?;
```

The host authorizes the canonical file before constructing the grant. A writable
grant enables text editing; binary documents remain read only. Query authority is
independent. Notes require `notes_directory` supplied by trusted startup code;
they are disabled by default. No browse, open, pane or comparison routes exist in
this service. Knowing a sibling path does not grant access to it.

`Service::start(path)` remains a read-only compatibility entry with queries enabled
and notes disabled. The host executable calls `reader::worker_entry()` before
normal startup. The headless [embedded-host example](../examples/embedded-host.rs)
owns the service until stdin receives a newline, then awaits cleanup:

```sh
cargo build --locked --example embedded-host
node tests/server/embedded-service-check.mjs target/debug/examples/embedded-host
node tests/browser/embedded-check.mjs target/debug/examples/embedded-host
```

The service binds an ephemeral loopback port and protects APIs with its own
scoped token. Host/Origin checks remain active. A parent origin permits framing
and protocol messages only; it does not enable cross-origin API fetches. The
allowlist contains at most eight exact HTTP(S) origins, never `*`, `null`, a path
or credentials. Host gateways may proxy this service under a file-scoped URL;
they must preserve these boundaries and authorize their own requests. Gateway/AI
credentials never enter the reader frame.

## Public parent protocol

Bundle the local `web/host-client.mjs` and its `reader-bridge.mjs` dependency into
the host's resources. A cross-origin parent must not import private service APIs
or inspect `contentDocument`/`readerExperiment`.

```js
import { createReaderClient } from './host-client.mjs'
const client = createReaderClient(frame, {
  onEvent(type, payload) { /* update the host's controls */ },
})
const ready = await client.ready()
await client.request('setView', { view: 'source' })
const selection = await client.request('selection')
await client.request('navigate', { location: { kind: 'line', line: 12 } })
await client.request('settings', { toolbar: 'host', theme: 'dark', fontSize: 16 })
// Consult dirty state and use force only after the host's discard decision.
await client.dispose()
frame.remove()
```

Normal disposal waits for the child's acknowledgement. A rejected dirty-close
keeps the client usable. After an explicit forced close, the client also releases
its own listeners and pending requests if the frame disappeared or timed out;
the host still owns removing the frame and awaiting its backend service cleanup.

`toolbar: 'host'` hides the reader header and status footer together, so the
host can present one set of document controls and status. The default `reader`
owner keeps both visible. The host also owns Save, Reload and editor status in this mode;
editor page navigation and undo/redo remain inside the reader. The host must
guard reloading/replacing a dirty document before disposing it. This setting
does not hide document overlays or their controls.

`notes: 'host'` independently delegates the Notes UI to an ordinary host mounting
the shared `mountNotes` component. It disposes the child Notes controls/dialogs
without revoking the explicitly granted Notes storage API. The default `reader`
owner mounts the child UI; switching back recreates it over the same records.
`capabilities.presentation.notesOwner` declares support. This is presentation
ownership only: it never grants storage, queries or file access.

Envelopes use `protocol: "marknoria.reader"`, `version: 1`, a UUID `requestId`, and
`kind: request/response/event/cancel`. Requests name a method and parameters;
responses contain `result` or `{error:{code,message}}`. Both ends check source
window and exact origin. Unknown versions, stale identity/revision, unsupported
commands/locations, cancellation and deadlines report errors. Child concurrency
is at most eight commands; host pending requests are at most 32. `ready()` retries
the initial handshake and waits for a useful document, including indexing errors. Its readiness budget defaults to 75 seconds, separate
from the 30-second command timeout. `timings` distinguishes index duration/peak RSS,
first useful content and the heartbeat gap; an index-ready document may still be
mounting its first view.

| Command | Parameters / result |
| --- | --- |
| `hello`, `state` | State, identities, revisions, capabilities, view, dirty/busy and approximate reading position |
| `setView` | `view: preview/source/edit`, checked against granted format capabilities |
| `contents` | `open:boolean`; uses the same saved-outline refresh and busy handling as the reader's Contents control |
| `navigate` | `location: source/line/cell/pdf/epub/epubChapter`; exact source byte ranges open Source and select the LF-normalized DOM range; unsupported locations return an error intact |
| `save` | Saves the current editor page; separate edit and preview revisions remain explicit |
| `assetInfo` | `id` from `state.document.assets`; returns bounded preview dimensions/frame timing without cache filenames |
| `assetFrame` | `id, frame`; returns `{revision,id,frame,blob}` with a projected PNG `Blob`, at most 20 MiB; two concurrent frame commands |
| `outline` | `offset` (default 0), at most 256 items, total and revision |
| `search` | `query`, 1–256 UTF-8 bytes; bounded source lines or complete-document PDF/EPUB search |
| `selection` | Typed selection or null, with quote, units, precision and viewport CSS rectangle |
| `projectionWindow` | `start,count` (at most five); immutable Rust HTML/source/table projections for ordinary DOM hosts; see [projection-components.md](projection-components.md) |
| `sourceWindow`, `sourceSelection` | Bounded original source by `line`, `offset,previous`, or explicit `offset,end` (at most 128 KiB); exact conversion of a returned window's LF-normalized UTF-16 selection to original UTF-8 bytes |
| `tableChart` | Complete source/result projection; `source:{kind:table,table}` or `{kind:query,queryId}`, `x` (nullable), `y`; see [table-components.md](table-components.md) |
| `sql`, `jq` | Complete-input query, returns a bounded result page and result ID |
| `notes` | Nested `request` using the public Notes operations; explicit storage grant required |
| `tableInfo`, `tableWindow` | `table`; or `table,row,columns` (original column IDs); scoped projection for ordinary DOM hosts |
| `sqlSchema`, `sqlPage`, `sqlClose` | `table`; or `id,row`; or `id` |
| `jqPage`, `jqClose` | `id,page`; or `id` |
| `settings` | Toolbar owner, light/dark theme, typography/wrap; `pdf:{zoom:'width' or .25–4,continuous:boolean}`, `epub:{theme,flow,fontScale,typography}` |
| `dispose` | Refuses dirty content with `DIRTY`; `force:true` explicitly discards it, then stops owned DOM listeners, observers, timers, requests and mounted content |

Stateful commands include `documentId` and immutable projection `revision`.
`capabilities.sourceRangeNavigation` declares exact source-range restoration.
Hosts retain the reference's revision when reopening a file and explicitly
refuse or request a stale-location choice if that revision has changed; a new
session ID does not make old byte offsets valid for new contents. Transient
SQL result IDs and unsaved draft references are not durable reading positions.
State also reports document byte/line/page/table counts and
`outline:{open,available,total}`. An original-container outline's total is null
until its renderer resolves it. Source search results include a navigation
`location` and `locationPrecision` (`line` or `row`); a row target uses the first
original column as its navigation anchor, without claiming that the match was
in that column.
The client supplies its latest state by default; explicitly supplied expectations
are retained, so stale commands fail. Events are `ready`, `state`, `saved`,
`selection`, `error`, `openRequested`, and `disposed`. Repeated identical state is suppressed.
Selection notifications are debounced after the host handshake and previous requests cancelled.
Embedded Ctrl/Command+O delegates to the host; it cannot browse the service filesystem.
Saved `position` has the core ReadingPosition shape; `positionPrecision` is a
separate state field. Dirty drafts do not produce a durable saved position. A source
selection is converted in Rust from LF-normalized UTF-16 to original UTF-8 bytes,
including BOM/CRLF/surrogates. Draft editor selections declare `draft` and a line;
ordinary rendered text declares `approximate`. Table cells retain original column
indices. PDF text/Shift-region selections use one-based original pages and
normalized display-page rectangles, after crop/rotation, with viewport CSS bounds.
These are exact renderer selections, not invented UTF-8 offsets. PDF navigation,
outline and search resolve in the same component. Full EPUB CFIs remain intact in selection, search, navigation and saved positions.
`epubChapter` is an original archive href for outline navigation; it is not an
exact text selection. Continuous viewport positions describe the topmost visible
text, which can belong to the preceding chapter when the final chapter is short.

## Original-container PDF profile

Web services default to `ProjectionProfile::Web`. For PDF this publishes an
immutable, hashed original snapshot with `index.renderer = "pdfjs"`, without
Poppler extraction or rasterization. The shared PDF.js component owns page count,
outline, cross-page search and text/region mapping. `formatState` reports its
page, zoom, flow, mounted-page count and component timing; the main state timing
still measures first useful content from reader startup. Host commands use the
public protocol, including state/selection events and durable PDF positions.

`GET /api/original?revision=…` requires the same document/token authority as other
APIs. It exposes only the granted immutable PDF/EPUB container, streams in 64 KiB
chunks, and supports explicit HTTP byte ranges up to 8 MiB. No path argument or
adjacent-file authority is accepted. Service shutdown interrupts even an unread,
backpressured response and closes its file before removing the cache.

The component keeps at most three page canvases (8 Mi pixels each), 5,000 text
items/512 Ki characters per page, 2,000 original pages, 2,000 outline items and
256 selection rectangles. Outline transport is paged at 256 entries. A search
scans all pages until its visible 500-page result cap, reporting `complete:false`
if that cap stops the scan; query cancellation never masquerades as empty results.
Document/page work has deadlines and disposal awaits the PDF worker's destruction.
These are input/DOM/canvas budgets, not a claim of a hard browser-process RSS cap.

Native readers and the core facade keep `ProjectionProfile::Indexed` by default.
A host can explicitly select either profile through `reader::Options::projection`
or `ServiceOptions::projection`. Renderer-dependent core search/outline calls
return `Unsupported`; original-container locations return `requiresRenderer`
and retain their original page/rectangles. They are never mapped to the loading
placeholder's projection page. No GUI toolkit enters the headless profile.

## Original-container EPUB profile

The Web profile marks EPUB snapshots `renderer:"epubjs"`; platform-native
readers retain indexed chapters. The local epub.js component reads the original
OPF, spine, navigation and chapter DOM, preserving full CFIs across font, paper,
flow, search and selection. Settings accept `theme:light/sepia/dark`,
`flow:paginated/scrolled`, `fontScale:.75–2`, and typography. Default progress is
explicitly chapter based; opening a book does not first format/index every chapter.
Exact whole-book progress is opt-in: `epubProgress` accepts `operation:build`,
`operation:cancel`, and `operation:seek, percentage:0..1`. Ordinary components expose
`buildProgressIndex({signal})`, `cancelProgressIndex()` and `seekProgress(percentage)`.
Reading settings expose the same build/cancel action; the footer slider is enabled
only after the complete index commits. This uses epub.js's original CFI algorithm
with 1,600 UTF-16 units per location, matching the frozen Studio profile. Saved CFIs
and the current chapter survive indexing, cancellation, budget errors and timeouts.

The published `EPUB_PROGRESS_PROFILE` bounds this opt-in work to 256 linear
chapters, 256 Ki UTF-16 units per chapter, 16 Mi total, 32,768 nodes per chapter,
64 levels, 128-unit IDs, 16,384 locations, 2 MiB of CFIs and a 15-second deadline.
Chapters are scanned sequentially with an event-loop yield between them, abortable
resource requests, and unloading of inactive sections. Partial candidates never
become a complete index. State reports `progressPrecision:chapter/indexed` and
`progressIndex` status/counts/error; an oversized book remains readable through
Contents and exact saved CFIs. These are work/data bounds, not a hard renderer RSS
limit. The index is session-local and rebuilt only on explicit request.

`GET /api/epub` requires normal document/token authority and returns a separate
read-only resource capability. Its URL can read members of this immutable EPUB
only; it conveys no reader API token, adjacent-directory or write authority.
The server validates archive names and compressed/expanded limits before serving
members. Chapters are at most 2 MiB, 10,000 nodes and 64 levels; images at most
10 MiB/16 MP; fonts at most 8 MiB. Internal DTDs are rejected. Resource requests
share the service work budget. Closing/replacing the document revokes its capability.
Book frames exclude scripts and popups, retain the no-base/no-form CSP and resolve
local assets within this scope. External links delegate to the host callback.

Search scans the complete spine until 500 results, reports `complete:false` when
capped, and retains exact match CFIs. Chapter text work is bounded at 10,000 text
nodes/2 Mi UTF-16 units, with a 30-second search deadline and cancellation between
chapters and during requests. No CFI is shortened to fit a field: invalid or
oversized (>8 KiB UTF-8) references fail intact. Component disposal aborts owned
requests, destroys the rendition and clears selection listeners/fallback timers.

```sh
node tests/browser/epub-reader-check.mjs
node tests/browser/epub-reader-check.mjs --webkit
node tests/browser/embedded-epub-check.mjs target/release/examples/embedded-host
```

## Computation and disposal

HTTP consumers reserve `/api/operations` with `{action:"begin",revision}` before
dispatching SQL/jq using the returned `operationId`. Cancellation uses
`{action:"cancel",revision,id}` and reports whether the operation existed.
Unknown IDs are not a confirmed cancellation. At most eight operations are
registered per document; unused reservations expire after 30 seconds. Busy
rejections preserve reservations until the computation is actually accepted. Aborting a
network fetch alone is not cancellation of Rust computation. The shared
`query-operation.mjs` owns this handshake for dialogs and protocol queries.

`Service::close` stops acceptance, cancels indexing/query/render work, drains
accepted work, stops maintenance and removes owned caches; cleanup errors are
returned. Dropping sends the same shutdown signal, but hosts should await `close`
before destroying their runtime. Host-owned note storage persists. Complete-input
query budgets remain separate from visible DOM paging, with no silent row limit.

Browser diagrams use local locked resources and do not start an extra backend
Chromium. Backend rendering remains available. Diagram SVG caching is bounded and
scoped to document/revision/backend; disposal cancels fetches, revokes object URLs
and restores owned controls. The shared ordinary-DOM lifecycle contract tests
same-node remounts and prevents another document's diagram from being reused.

This is the protocol foundation for issues #4/#5/#7. The bounded host presentation
interface is described below; remaining format UI localization and verified
Studio adoption remain tracked work, not completed acceptance claims.

## Shared cell interaction primitive

`web/table-cells.mjs` exports `mountTableCells(content, options)` for ordinary DOM,
projected document tables and SQL results. The provider owns the complete dataset;
this primitive reads only its bounded displayed window. `controls` is a toolbar
slot, `locate(cell)` returns original `cell`/transient `queryCell` coordinates,
`onSelection` receives a typed selection, and `translate`/`copy` are host adapters.
It owns a scoped, reference-counted stylesheet and does not require host-private
reader DOM or document/AI credentials.

Call `update()` after replacing a window. The window is limited to 512 rows/cells,
32,768 descendant nodes and 128 Ki UTF-16 text units; oversized ordinary DOM is
rejected explicitly, not silently shortened and presented as complete input.
The component caches that displayed snapshot, preserves existing tabindex values,
and restores owned focus/style/listener/dialog resources on `dispose()`.

Click/focus selects a cell; arrows move within the displayed grid. Enter or a
double-click opens the complete bounded cell as inert text. Copy cell and
Ctrl/Command+C (without an active text selection) copy the complete cell; a separate selection excerpt is bounded at
32 KiB UTF-8 without splitting Unicode and declares `quoteTruncated`. Saved notes
retain that flag and copied references identify excerpts. Column resizing remains
the same shared `tableLayout` component, including keyboard resizing/fit.

Query-cell references carry their owning result ID and expire when that result
closes, is evicted or changes revision. Core validation checks result ownership
and coordinates; saved reading positions cannot treat these as durable source
locations. The current Notes adapter exposes copying for these transient cells;
a durable query-note adapter remains open work. The shared table mount and plots
are described in [table-components.md](table-components.md).

The shared Notes storage/action/presentation adapters are documented in
[notes-components.md](notes-components.md).


## Host presentation settings v1

`state.presentation` reports the current scoped configuration, and
`capabilities.presentation` advertises `{version:1,locale:true,palette:true,fonts:true}`.
The existing settings command accepts:

```js
await client.request('settings', {
  locale: 'fr', messages: { Preview: 'Aperçu', Columns: 'Colonnes' },
  theme: 'dark', toolbar: 'host', wrap: true,
  fontFamily: 'Noto Sans, sans-serif', contentFontFamily: 'serif',
  codeFontFamily: 'monospace', fontSize: 14, contentFontSize: 20, codeFontSize: 16,
  palette: { brand: '#123456', 'brand-soft': '#dbe9ed', panel: '#ffffff' }
})
```

The English UI keys and translation budgets are documented in `translation.md`.
Supplying `messages` replaces the dictionary; omitting it retains the dictionary.
State reports the locale/count, never the dictionary. Text/ARIA bindings address
explicit reader chrome and component controls, without scanning document data.
Unsaved Edit markers and source/edit range labels update without replacing the
editor or saving its draft. Column names, values, source text, errors and quotes
remain literal. Other format/auxiliary UI translation is still tracked work.

UI (`fontSize`), projected document (`contentFontSize`) and source/code
(`codeFontSize`) sizes are separate, finite 11–32 CSS pixels. PDF retains its
original typefaces and uses `pdf.zoom`; EPUB content uses `epub.typography` and
`epub.fontScale`, independent of its UI text. An omitted code size retains the
existing format default; `codeFontSize:null` removes the override. Family names
are at most 256 characters, checked against the CSS font-family grammar and an
inert character allowlist. Null removes a family override. Fonts are system/host
fonts; this does not authorize downloading resources.

Palette keys are `panel`, `soft`, `text`, `muted`, `faint`, `border`, `brand`,
`brand-soft`, with literal 6/8-digit hex colors. Supplying a palette replaces its
previous overrides; `{}` restores CSS defaults. Invalid common presentation
settings or dictionaries leave the previous presentation unchanged. PDF/EPUB
format commands retain their own validation and work budgets.

The same `readerPresentation(root,{translate,themeRoot})` primitive is available to
ordinary hosts; by default all attributes/styles are scoped to its explicit root.
The full reader uses its own iframe document's theme root and synchronizes the
syntax stylesheet through the existing theme controller. Hosts still own their
outer application's appearance. Disposal restores prior host styles/attributes.
The public settings state is session-local and never grants file/write authority.


The jq window is also available as `mountJqQuery`/`readerJqProvider` for ordinary
hosts using the same public commands and complete Rust input. See
[jq-components.md](jq-components.md) for provider contracts, paging/input budgets,
locale, clipboard, cancellation and disposal. It does not parse query output or
map its transient output text to invented source offsets.
