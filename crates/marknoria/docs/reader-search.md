# Reader search contract (version 1)

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
const first = await reader.request('search', { query: 'needle', offset: 0 })
const next = await reader.request('search', { query: 'needle', offset: 100 })
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

## Commands and limits

- `search({query, offset?})` starts or pages a search. `query` is a string of at
  most 256 UTF-8 bytes. `offset` defaults to zero and must be a nonnegative safe
  integer. A request returns at most 100 matches. A nonempty unchanged query in
  the same document/revision retains its opaque `searchId`; changing the query
  creates a new identity. Searching does not activate or scroll to a result.
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
rejected. Source queries retain the existing case-sensitive, nonoverlapping
original-line matching semantics.
PDF uses literal Unicode case-insensitive matching; EPUB uses literal
case-sensitive matching against original chapter text. Queries are not regular
expressions, and each renderer preserves its own text-boundary semantics.

`capabilities.search` declares `queryBytes: 256`, `windowLimit: 100`, `activation`,
`views`, `pagination`, and `resultLimit`. Source-backed search uses
`pagination: 'source'` and `resultLimit: null`: the complete source count is exact,
while only one 100-result window plus one active result is retained. Each requested
window scans the immutable source under its existing 128 MiB limit; no unbounded
match index is materialized. PDF/EPUB use `pagination: 'capped'`, with a bounded
500-result renderer snapshot paged in windows of 100. Their renderer work limits
remain unchanged. Activation is unavailable in Edit; Preview and supported Source
views are declared explicitly.

## Responses and state

A search response contains `version`, `documentId`, `revision`, `searchId`, `query`,
`offset`, `limit`, `total`, `totalExact`, `available`, `complete`, and `matches`.
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
  offset,                     // window currently retained by the controller
  current,                    // absolute zero-based ordinal, or null
  total, totalExact, available,
  loading,
  error,                      // null or bounded {code, message}
  active,                     // verified result record, or null
}
```

A clear resets the owner/identity to null and counts to zero. The host's reading
`position` remains distinct from the active search occurrence: manually scrolling
away does not change which occurrence was last activated.

## Navigation and cancellation

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
