# Shared tables and plots

Rust owns parsing, complete input, numeric projection, queries and caches. The
Web components own displayed windows, geometry, selection and SVG layout. These
are the same implementations for standalone, browser/WebView and embedded hosts.
A displayed 50-row page is never a complete-query or chart input.

## Complete-input plot API

```rust,ignore
use marknoria::reader::ChartSource;
let descriptor = reader.descriptor();
let operation = reader.operations().begin()?;
let chart = reader.chart(
    &descriptor.revision, ChartSource::Table { table: 0 }, Some(0), 1, &operation,
)?;
```

`x`/`y` are zero-based original column indices; `x:None` labels rows by their
one-based display number. `ChartSource::Query { query_id }` reads a complete live
SQL result, preserving result order. A temporary source-table chart does not
evict the host's four saved SQL results. Revision, query authority, operation
ownership, cancellation, result identity and column bounds are checked.

The response declares `datasetScope:complete-source-table` or
`complete-query-result`, `inputRows`, `rows`, axis names and all points. An
aggregated one-row result can truthfully report thousands of source rows
evaluated. Missing/null/blank numeric values retain their rows as gaps;
non-numeric and non-finite values fail with SQL guidance. No row subset is silently
plotted. Source data can be filtered/aggregated using the existing complete-input
SQL engine and the resulting query ID passed to the chart API.

Projection is limited to 2,000 points and 128 KiB of serialized transport. Oversized
results fail explicitly. Result projection has a two-second deadline, separate
from existing SQL input preparation/execution budgets. Plot values use `float64`;
`valueText` retains the exact numeric cell text for tooltips and references.
Category labels abbreviate at a valid 128-byte UTF-8 boundary and declare
`labelTruncated`. The source/result table remains available for complete values.
These are computation/input/transport budgets, not a hard process-RSS guarantee.

HTTP consumers use `POST /api/table-chart` with
`{revision,source:{kind:'table',table:0},x:0,y:1,operationId}` or
`source:{kind:'query',queryId}`. It uses the same scoped grant, operation
reservation/cancellation and work limits as SQL/jq. The public iframe command is
`tableChart`, and capabilities expose `charts` and `chartProfile`. A host disabling
queries also disables charts; neither endpoint accepts paths or expands grants.

## Ordinary DOM and embedded host

```js
import { mountTablePlot } from './marknoria/table-plot.mjs'
const owner = await client.ready()
const schema = await client.request('sqlSchema', {
  documentId: owner.documentId, revision: owner.revision, table: 0,
})
const plot = mountTablePlot(container, {
  columns: schema.columns,
  load: ({ x, y, signal }) => client.request('tableChart', {
    documentId: owner.documentId, revision: owner.revision,
    source: { kind: 'table', table: 0 }, x, y,
  }, { signal }),
  translate,
  onSelection(value) { /* attach the owning document/revision; host action slot */ },
})
await plot.ready
// Dispose/remount when the owning document/revision changes.
plot.dispose()
```

The low-level plot accepts a complete bounded provider instead of private reader
DOM or an AI account. State reports busy/error, rows and dataset scope. Native
keyboard focus and click select a bar; arrows move between numeric bars. Selection
returns original source-cell or transient query-cell coordinates, exact value
text and viewport CSS bounds. Plot positions are visual floating-point layouts;
they never become invented UTF-8 offsets. Query references expire with the result.

`tableChartDialog` is the HTTP reader wrapper used by source tables and SQL
results. It preserves the last valid graphic on errors, aborts on close, restores
focus and closes when the owning reader leaves its view. Bars/labels are SVG DOM
with inert text. Shared component styles are scoped and reference counted; redraw
aborts old bar listeners and disposal removes owned DOM/listeners/styles. Source
plot selections can use the common Notes actions inside the active modal;
query references can use verified durable SQL snapshot notes when the host explicitly grants query Notes storage (see `notes-components.md`).

`mountTableCells` is described in [embedding.md](embedding.md). `tableLayout`
remains the common measurement/column/scroll geometry primitive. The shared ordinary/virtual table mount is described below. Host note/action/storage adapters are documented in `notes-components.md`.

## Shared projected table mount

`web/table-view.mjs` exports `mountTableView(article, options)`. The main virtual
reader delegates to this implementation; an ordinary DOM host uses the same
module without a private reader shell, IDs or layout globals:

```js
import { mountTableView, readerTableProvider } from './marknoria/table-view.mjs'
const owner = await client.ready()
const provider = await readerTableProvider(client, owner, 0, { signal })
const table = mountTableView(article, {
  ...provider, viewport, workspace,
  readWidths: () => sessionWidths,
  saveWidths: value => { sessionWidths = value },
  onSelection(value) { /* owning document/revision remains the host's context */ },
  onQuery() { /* mount the shared SQL UI/provider */ },
  onChart() { /* mountTablePlot with the same complete-input provider */ },
})
await table.ready
await table.go(119, 2) // reveal and focus an original hidden column
// Before removing/replacing the owner or document revision:
table.dispose()
```

`viewport` is the scrolling outer document/container. `workspace` is its positioned
parent and owns the floating horizontal track. Neither requires a particular ID
or reader shell. `shell` optionally supplies layout settings (defaults to article).
The component loads scoped, reference-counted presentation styles; it does not
apply the reader's page/grid styles to the host. The article's children belong to
the component. Dispose aborts the active load, removes owned content/track/styles
and prevents late responses from resurrecting it. Same-node remount is supported.

A non-iframe host supplies `metadata:{columns,label}`, immutable `revision`, and
`load({row,columns,signal})`, returning `{revision,result:{html,rowStart,rowEnd,
totalRows,pageSize}}` from the public Rust table projection. This does not require
building an artificial document index. Keep complete ordinary input in
`Reader::from_table`, not in a JavaScript parsing/query fork. Providers must honor
the signal and do their bounded work outside the UI thread.

Windows allow at most 50 rows, 512 body cells, 128 KiB UTF-8 HTML and 32,768 nodes.
Metadata permits 512 names with 64 Ki UTF-16 aggregate text. Window coordinates,
shape and revision are checked before replacing the last valid grid. HTML passes
through the same DOMPurify policy as the main reader. An error retains valid
content and exposes Retry and `state().error`. `state()` also reports loading,
stopped, displayed start/total rows and selected original columns. `refresh`,
`position`, `anchor` and `capture` support a host virtualizer without observing
private nodes. Widths are original-column keyed and retained by the host's
session adapter; independent instances do not share mutable state.

`readerTableProvider` uses public `tableInfo` and `tableWindow` commands with an
explicit owning document/revision. It reads no iframe DOM and gains no query or
file-write authority. These commands remain available for read-only documents;
SQL/chart authority is separate. Shared contracts cover ordinary-host independent
windows, page/width retention, selection/hidden-column navigation, untrusted HTML,
foreign revisions, oversized responses, cancellation and same-node remount in
Chromium and mapped native Wayland WebKit. The production reader runs the same
contract in its browser regression. The shared SQL window is described below; verified query-note snapshots preserve result provenance without inventing source-cell offsets.

## Shared SQL window and plot dialog

`mountTableQuery(container, options)` and `readerQueryProvider(client, owner, table)`
are exported by `web/table-query.mjs`. The standalone reader's `tableQuery` wrapper
uses this same component with its HTTP adapter. An ordinary host can use:

```js
import { mountTableQuery, readerQueryProvider } from './marknoria/table-query.mjs'
const owner = await client.ready()
const query = mountTableQuery(overlayRoot, {
  revision: owner.revision,
  provider: readerQueryProvider(client, owner, 0),
  presentationRoot: readingSurface,
  onSelection(value) { /* typed transient query-cell or chart reference */ },
})
await query.ready
await query.execute('SELECT * FROM data')
await query.go(100)
const resultState = query.state()
await query.dispose()
```

The ordinary provider interface is `schema({signal})`, `run({sql,signal})`,
`page({id,row,signal})`, `close({id})`, and optional `chart({id,x,y,signal})`.
Schema provides `columns`, `sourceRows` and `defaultSql`. Run/page return the public
Rust SQL result window, retaining exact text/null, UUID result identity, revision,
row coordinates, complete-result total and elapsed time. Neither UI nor adapter
parses data or SQL. Complete source input and query authorization/budgets remain
in Rust. The iframe adapter uses the existing scoped public commands, including
their single operation reservation; it never creates a second lease.

The shared window validates at most 256 column names, 50 rows, 512 cells and
128 KiB serialized column/row data before replacing valid results. It preserves
leading zeros, large integer text and inert HTML-looking values. Invalid SQL,
foreign revisions and oversized provider responses retain the previous valid
window; a rejected new result is closed. Source/result column visibility, manual
widths, keyboard/full-cell/copy behavior and last-page navigation remain shared
with the main reader. A chart evaluates the entire immutable result and captures
its result identity when opened, rather than using the visible page or a later
query. With `queryNotes` and explicit storage, the Notes adapter verifies the live result and saves durable SQL provenance; it never stores an expiring result ID or automatically reruns stored SQL.

Dispose aborts active work, removes overlays/listeners/styles, restores focus and
returns a promise for closing the current result. A late provider response is
released without remounting content. Providers must honor cancellation; disposal
cannot forcibly stop arbitrary synchronous host code. Two instances keep their
result IDs, state, queries and overlays separate.

`mountTablePlotDialog(container,{columns,load,...options})` is the public modal
wrapper for `mountTablePlot`; the HTTP `tableChartDialog` delegates to it. It uses
only explicit providers/presentation roots and restores focus on disposal.
Opening the chart makes no data request and does not guess a numeric column.
The user selects that axis and presses Draw chart; `ready` means the controls
are mounted, while `state().rows` remains null until the first successful draw.
String values are never silently converted to zero or dropped.

`table-view.css` and `table-query.css` are the single shared table/query style
sources. The main HTML loads them before content to avoid layout changes from a
late stylesheet; ordinary hosts acquire them automatically. Reference counting
reuses an already loaded host stylesheet and never removes one owned by the host.
The table component has its own size container for narrow ordinary/virtual panes,
without applying the main reader's page grid to its host. Native GNOME retains
its separate toolkit stylesheet with matching values, documented in NATIVE.md.

Run `npm run check:query-component` (or the script with `--webkit`) for the ordinary
provider contract. It verifies paging/widths, exact text/coordinates, full-result
plots, invalid-query/oversized-result recovery, cancellation, result release and
remounting in Chromium and native Wayland WebKit. The production table regression
runs the same contract; the cross-origin embedded test also mounts this public
SQL UI with the actual Rust engine through `readerQueryProvider`.

## Scoped presentation and immutable windows

`tableControls`, `mountTableView` and `mountTableQuery` accept the shared `translate`
callback from `translation.mjs`. Explicit UI bindings update open controls,
column menus, paging counts, SQL help/status and accessibility labels. A locale
change does not reload the provider, rerun SQL, translate column names, edit SQL
text or modify data. UI numbers use the chosen locale; original column indices,
identifiers and values remain literal. An unsaved column-selection draft survives
row-only refreshes. Revealing a hidden column through `go` updates both the grid
and its Columns checkbox. Literal table captions use `labelKey:null`.

A source window must preserve the immutable total and include the requested row.
`metadata.rows` may declare that total; older providers can omit it and establish
it from the first accepted window. SQL page replies must preserve the active
result ID, schema and total and contain the requested row. A foreign page result
is rejected without closing another owner's result. Invalid replies retain the
last valid window. Candidate SQL controls are created before replacing the old
result; exhausted translation resources release the rejected candidate and keep
the valid result/plot usable. Accepting a replacement result closes its old plot
before releasing the old result ID.

`go(row,column)` scrolls only the explicit document viewport vertically, then
reveals/focuses the requested original cell horizontally. It does not scroll a
parent host or another pane. Late, superseded navigation cannot steal focus.
Disposal aborts loads and releases translation subscriptions and style leases,
including setup failures.

The ordinary component contracts cover 19 translator, 10 toolbar, 28 source-view
and 25 SQL assertions in Chromium 149 and native Wayland WebKit 2.52.6. The release
HTTP regression passes 231 shared-table assertions (including equality of 11
served source assets), 110 SQL/CSV assertions, the cross-origin two-pane public
host, and the actual Wayland WebView document suite. Whole-reader locale/settings
integration remains tracked separately; this does not claim all controls are
translated yet.


Layout accessibility, full-cell dialogs and complete-input plots also accept the
same translator. Open dialogs and chart descriptions update in place; the SVG
bars, original column names, values and selection are retained. No dataset is
fetched or measured again for translation. Resize labels use two bounded scopes
so the permitted 512 original columns fit without weakening the per-scope limit.
Closing dialogs releases their subscriptions immediately. Failed dialog or
partial table setup removes its owned scopes/styles/DOM and preserves the host.
The document's stable horizontal-scroll control identity is retained during
atomic setup; independent panes still get collision-free content IDs.

The extended ordinary contracts pass **19 layout**, **20 cells**, **20 plot**,
**29 source-view** and **25 SQL** assertions on Chromium 149 and native Wayland
WebKit 2.52.6, including maximum columns and exhausted subscription capacity.
`npm run check:table-layout` runs the standalone layout contract; all component
checks accept `--webkit` under a native Wayland session.

## Delimited input and workbook scalar compatibility

CSV/TSV detects comma, tab or semicolon using the same consistency, matching-row,
width and extension-fallback preference as Studio. Detection uses the mature CSV
parser over at most 256 KiB and 64 nonempty records per candidate. Incomplete
sample records are excluded; ties retain the extension's default delimiter.
The complete accepted input is then indexed and queried with that same selected
delimiter. Sampling does not truncate the dataset. BOM/CRLF, quoted multiline
records, source bytes and physical source lines remain intact. The additive
`csvDelimiter` cache field defaults to the format delimiter for old caches; it
is not a new file authority or a change to original source bytes.

Workbook serial/ISO dates use UTC ISO text with milliseconds, matching Studio's
Date conversion. Cached formula results remain the displayed/query values;
uncached formulas remain literal text, including shared formulas resolved by the
pinned Calamine parser. No formula is executed and hyperlink expressions do not
become active links. Date parsing/formatting uses the already locked Chrono
version with allocation support, without clock or system-timezone features.
Workbook headers/records are each bounded at 128 KiB; aggregate streamed table
JSON across every sheet is bounded at 128 MiB. Rejected records are checked
before writing, with a shared budget across sheets. This is separate from the
original ZIP/expanded-member, cell-count, projection and SQL-result budgets.
