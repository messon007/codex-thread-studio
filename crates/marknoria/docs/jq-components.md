# Shared jq query component

For query syntax, current-table examples, dialect restrictions and error recovery, see [SQL and jq query help](query-help.md).

Rust owns the complete JSON/JSONL/NDJSON input, filter evaluation and immutable
result cache. `web/jq-query.mjs` exports `mountJqQuery` and `readerJqProvider` for
the same ordinary DOM, browser/WebView and embedded query UI. The main reader's
`jqController` delegates to that component; it has no second query implementation.

```js
import { mountJqQuery, readerJqProvider } from './marknoria/jq-query.mjs'
const owner = await client.ready()
const query = mountJqQuery(overlayRoot, {
  revision: owner.revision,
  provider: readerJqProvider(client, owner),
  presentationRoot: readingSurface,
  translate,
})
await query.ready // no computation by default
await query.execute('select(.level == "error")', { slurp: false })
await query.go(1) // zero-based output page
console.log(query.state())
await query.dispose()
```

An ordinary provider implements `run({filter,slurp,signal})`,
`page({id,page,signal})` and `close({id})`. Run returns an owned result candidate;
page reads the same immutable result. Both return
`{kind:'jqQuery',id,revision,page,pages,totalValues,text,elapsedMs}`. IDs are UUIDs;
counts/positions are nonnegative safe integers and elapsed time is finite.
Providers must evaluate their complete declared input outside the UI thread,
honor cancellation and release their owned result caches. A host must not slice
records and present a partial calculation as a complete query.

The iframe adapter uses public `jq`, `jqPage` and `jqClose` commands with an
explicit owning document/revision. It preserves the abort signal and does not
create a second operation reservation. Query capability is independent of source
write permission. The HTTP adapter uses the same supervised Rust operations and
existing result authority; neither UI adapter accepts file paths.

Filters are nonempty and at most 16 KiB UTF-8 (the textarea also bounds UTF-16
units). Output windows contain at most 48 KiB UTF-8, matching the Rust pager.
`JQ_WINDOW_PROFILE` declares these limits and the maximum page count implied by
the existing 256 MiB maximum result budget and UTF-8 boundary adjustment. Rust's
source-scaled 8–256 MiB output cap, input budget, cancellation and execution
limits remain authoritative. An oversized/failed computation returns an explicit
error and keeps the last valid result; it never becomes a partial success.

Output pages are byte-bounded slices of a complete formatted result and may split
a JSON value across pages. The component does not parse or round them, invent
source offsets or call a page a complete JSON document. Copy result page copies
its exact bounded text. Empty results retain an empty copy value and display an
explicit empty-state label. Page replies must preserve result ID, revision, total
values, page count and requested page. A rejected page never closes a foreign ID;
a rejected new owned result is released. Valid replacement releases its old ID.

`autoRun` defaults to false. The main reader opts into streaming identity `.` only
for input at most 1 MiB; larger input starts with a filter prompt. Ordinary hosts
can explicitly opt in using their known input profile. `execute` preserves the
current slurp checkbox if omitted; Reset filter restores `.` and disables slurp.
Ctrl/Command+Enter runs the current filter. Local validation errors are visible.

Explicit UI bindings update an open dialog's controls, counters and accessibility
labels without running a filter or changing literal filter/result/example text.
The component acquires the shared SQL dialog and scoped jq styles, supports narrow
windows and copies only explicit presentation variables. Disposal aborts work,
removes owned DOM/listeners/styles, restores focus and returns a promise for
releasing the active result. A late candidate is released without reviving UI;
arbitrary host code that ignores cancellation cannot be forcibly stopped by DOM
code. Instances keep result IDs, filters and styles independent.

Run `npm run check:jq-component` (also `-- --webkit`) for ordinary-host contracts,
`npm run check:embedded:jq` for the actual read-only Rust/public-protocol consumer,
and `npm run check:jq-layout` for compact/full-window geometry and clipboard checks.
Generated documents, caches and screenshots stay in ignored `target/` directories.
