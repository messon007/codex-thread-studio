# Ordinary DOM projection windows

`mountProjectionView` mounts bounded immutable Rust projections in an ordinary
host element. The standalone virtual reader uses the same `projectionHtml`
sanitizer; both paths use `mountTableView`, including independent complete-table
pagination, column controls, widths, cell references and horizontal scrolling.
Both paths also use `document-prose.css`, including identical default headings,
paragraphs, code and quote typography. Host content font/size/weight settings use
the same CSS variables and `readerPresentation` adapter.
The component does not run a second Markdown parser or treat a partial DOM table
as the query dataset.

```js
import { mountProjectionView, readerProjectionProvider } from './projection-view.mjs'
const owner = await client.ready()
const view = mountProjectionView(container, {
  provider: readerProjectionProvider(client, owner),
  viewport: scrollingElement,
  workspace: scrollingElement,
  translate,
  onSelection(selection) { /* Notes or host actions */ },
  onQuery(table) { /* mountTableQuery with readerQueryProvider */ },
  onChart(table) { /* mountTablePlotDialog with readerChartProvider */ },
})
await view.show(0, 5)
const selection = view.selection()
view.dispose()
```

The caller owns its surrounding navigation, virtualization and viewport. `show`
replaces one window; it does not append the complete document. A headless host
can supply a provider using `Reader::window` and `Reader::table` directly rather
than using an iframe service. Such a provider supplies `documentId`, `revision`,
`window({start,count,signal})`, and optionally
`table(id,{signal}) -> {table,revision,metadata,load}` with the existing table-view
provider contract. Notes, SQL, charts, diagram/image hydration and surrounding
controls remain explicit separately owned component integrations. This window
component does not replace the full reader's format renderers or editor.

At most five pages, 128 KiB UTF-8 HTML per page, 16,384 parsed nodes per page and
20,000 nodes per window are accepted. The immutable total, revision, contiguous
page numbers and source coordinates are checked. Unsupported/stale/oversized
candidates fail without replacing the previous window. A newer request aborts
the previous one; late responses cannot commit. Table fragments sharing one
original table identity are coalesced into one complete-table view per window.
Per-instance column/row state and widths survive remounts, capped at 256 tables;
different instances never share that state. Disposal cancels pending windows and
table requests and releases DOM, observers, floating tracks and scoped styles.

`selection()` / `onSelection` return the public typed selection shape. Ordinary
text has approximate one-based source-line precision; it never invents exact
Markdown byte offsets. A single original table cell has exact table/row/original
column coordinates. Quotes are bounded to 16,384 UTF-16 units without splitting a
surrogate pair. Rectangles are viewport CSS pixels of the host document. The host
can pass these references to the shared Notes component.

The v1 parent protocol adds `projectionWindow({start,count})`,
`sourceWindow({line})`, `sourceWindow({offset,previous})`, or
`sourceWindow({offset,end})` (an explicit range of at most 128 KiB), and
`sourceSelection({start,end,fromUtf16,toUtf16})`. All retain document/revision
checks and request cancellation. `sourceSelection` converts a source window's
LF-normalized UTF-16 positions to exact original UTF-8 bytes in Rust, preserving
BOM/CRLF/Unicode. `navigate` also accepts the resulting `source` byte range:
it opens Source, selects that range, and preserves its original bytes in the
returned selection. Ranges crossing the normal 200-line boundary use an explicit
bounded range window. Invalid UTF-8 boundaries and ranges over 128 KiB fail;
there is no silent clamping or byte-to-DOM-offset substitution.
Original-container PDF/EPUB use their own public components;
projection windows explicitly reject their placeholders. Binary formats reject
original UTF-8 source requests. No route grants an additional file.

```sh
node tests/browser/projection-view-check.mjs
node tests/browser/projection-view-check.mjs --webkit
node tests/browser/embedded-check.mjs target/release/examples/embedded-host
```

For durable Comments, await `view.captureSelection({signal})` and retain its
optional `textRange` alongside the typed location. The legacy `selection()` stays
synchronous. The [shared comment marker](comment-marks.md) paints exact selectors
without changing the projection tree.
