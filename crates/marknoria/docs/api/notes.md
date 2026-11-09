# Shared Notes component

`web/notes.mjs` exports `mountNotes(options)`. The main reader delegates to this
same component; the UI owns selection controls, comment/favorite dialogs, category
filters, confirmation, copying and presentation. Storage and additional actions
are explicit host adapters. There is no AI account, gateway, file picker or file
path capability in the component.

```js
import { mountNotes, readerNoteStorage } from './marknoria/notes.mjs'
const owner = await client.ready()
if (!owner.capabilities?.presentation?.textSelection)
  throw new Error('This reader does not support text-only selection')
// Fully unmount the child's Notes UI while retaining its granted store.
await client.request('settings', { notes: 'host' })
const notes = mountNotes({
  owner, // documentId, immutable revision, display name
  root: readingSurface,
  toolbar: documentTools,
  overlayRoot: overlayLayer,
  presentationRoot: readingSurface,
  selectionAllowed: () => true, // provider below already filters live text
  selection: async signal => {
    const value = await client.request('selection', { textOnly: true }, { signal })
    // For an iframe, map its viewportRect into this host's viewport first.
    return mapSelectionRectToHost(value)
  },
  storage: readerNoteStorage(client, owner),
  canSave: () => !hostHasUnsavedDraft,
  copy: async text => hostClipboard.writeText(text),
  actions: [{
    id: 'host-action', label: 'Host action',
    run: async (selection, { signal }) => hostAction(selection, signal),
  }],
})
await notes.open(0, 'comment') // null, comment or favorite filter
await notes.selectionChanged() // renderer/host selection notification
const state = notes.state()
notes.dispose()
```

An ordinary DOM host supplies its own selection/storage implementations instead
of a client. `selection(signal)` returns the public typed reference: documentId,
revision, quote, optional quoteTruncated, precision, location and viewportRect.
The component checks owner/revision, a 32 KiB UTF-8 quote, bounded location and
finite CSS viewport bounds. It copies the reference before passing it to a host
action; it does not invent source byte offsets for cells, PDF regions or EPUB CFIs.
Hosts of iframes own frame offset/scale conversion. Use an untransformed overlay
layer when positioning viewport controls. Default selection events are scoped to
`root`; the main reader's wrapper explicitly uses its own document for renderer
modals. A host can supply `eventRoot` or call `selectionChanged()` directly.

`storage(request,{owner,signal})` implements the existing public core Notes
requests: list/add/update/delete/clear/copy. The owner is captured at mount, not
read from a later active pane. Add includes the immutable revision and typed
anchor. List returns items, total and limit; each item has id, kind, quote, comment,
anchor, revision, created and stale. Copy returns `{text}`. Filters/clear use
`kind:null|'comment'|'favorite'`; comments clear does not delete favorites, and
favorites clear does not delete comments. Custom storage must enforce its own
scope and persist the same typed anchors. The shared Rust Notes implementation
remains available through `Reader::notes` and the HTTP/public command adapter.

`readerNoteStorage` calls the public `notes` command with explicit document/revision
and a nested request. This command is denied unless the service was configured
with a Notes directory; read-only file access does not implicitly grant notebook
storage. The default embedded service still has Notes disabled. Unknown fields
and invalid typed locations continue to fail in the Rust request parser/validator.
The component never receives database paths or another application's credentials.

UI storage calls have a four-request limit. Mutations are ordered, and subsequent
list/clear operations wait for earlier writes. Requests and copied text are
bounded at 128 KiB; displayed quotes/comments retain the core's 32/16 KiB UTF-8
limits, and a list page has at most 64 records. Only compact row summaries and one
full detail card are mounted. The Rust list can return full bounded records; its
transport is distinct from the 128 KiB table window profile. Providers must honor
AbortSignal and return bounded results. A cancellation may follow an already
committed storage write; persistence is authoritative in the storage provider.

`state()` reports busy, pendingRequests, disposed, open and filter. New selections
cancel superseded selection requests; scrolling hides stale controls. Dispose
aborts owned listeners/storage/action signals, disconnects theme observation,
removes owned overlays/styles and prevents late responses from reopening UI.
Closing a comment editor while Save is pending likewise does not reopen it.
Instances keep their owner, selection, library and filters separate. Buttons and
host actions use inert text, and synchronous or asynchronous action errors become
feedback instead of uncaught event errors.

`notes.css` is scoped to owned roots and is shared by the main HTML and ordinary
hosts. The main HTML loads it eagerly. Components acquire/release the stylesheet
without removing one already owned by the host. `presentationRoot` supplies reader
palette variables; theme changes update existing controls and dialogs. The Notes
header button inherits its host toolbar presentation.

Additional actions get only a bounded typed selection and an abort signal. Keep
AI prompts/credentials, project grants, network policy and business persistence in
the host callback. Query-cell references require an explicit `queryNotes:true`
storage profile; without it only Copy/host actions are exposed. The Rust service
and facade advertise `capabilities.queryNotes` only with both query and note
storage authority. `readerNoteStorage` can be used with this capability in an
ordinary host; the document reader enables the same profile.

On save, the Rust adapter resolves the live document/revision-owned result ID,
verifies its cell coordinates and that the nonempty quote is an exact substring
of the owned cell, and replaces
the transient location with `anchor.query`: SQL text, input table and row count,
result row/column/name, result row count and a null-value flag. It never infers a
source row for aggregates, joins or reordered results. Client-supplied provenance,
expired/foreign results, changed revisions and mismatched quotes are rejected.
An expanded cell viewer can capture a middle substring. If that selection exceeds
32 KiB UTF-8, its bounded prefix still belongs to the cell and is explicitly
marked `quoteTruncated`; this flag does not mean the whole cell was selected.
The immutable query definition comes from the stored result, not the editor's
current text. These records survive result closure and document reopening; a
changed document revision is marked stale. Copy labels the data as an SQL result
snapshot and includes its provenance. Opening/copying a note does not re-run SQL
or expand its file/query authority. This is a durable reference, not a navigable
source-cell position. SQL text is bounded at 16 KiB and column labels at 4 KiB;
normal note quote/comment/storage bounds still apply.

Hosts saving their own drafts can call public Rust `Reader::query_snapshot` or
the parent command `querySnapshot` (`POST /api/query-snapshot`). The request has
revision, live `queryCell` location, quote and optional `quoteTruncated`; the
response is `{revision,quote,anchor}` with the same verified `anchor.query`.
`capabilities.querySnapshots` requires query authority, independently of Notes
storage. This bounded lookup creates no Note/database record and never reruns
SQL. Capture it before closing the result; expired/foreign results, mismatched
quotes and changed revisions fail. Persist the returned value as historical data,
display its provenance on reopening, and never reinterpret it as source offsets
or automatically run the recorded SQL. Hosts must cancel/discard a late capture
when the owning document changes.

`npm run check:notes-component` exercises ordinary-host storage/actions, owner and
revision isolation, literal text, comment/favorite category clearing, compact
confirmation, copying, transient references, cancellation and remounting. The
same script supports `--webkit` for mapped native Wayland WebKit and `--compact`
for a real 480×480 viewport, and the main
browser table regression runs the same public contract. The embedded PDF test
also mounts host Notes using the actual granted Rust store/public protocol with
PDF region anchors; disabled-service storage is tested separately.

The ordinary-host contract covers storage/actions, typed references, cancellation
and cleanup in Chromium and native Wayland WebKit. Compact list summaries keep
one complete text line plus metadata; full text remains in the independently
scrolling detail pane. The production regression also verifies embedded asset
bytes match the checkout.

`translate` is an optional `translate(key, variables)` host callback. Passing the
shared `createTranslator` also updates existing dialog/toolbar/accessibility and
date labels through bounded subscriptions. Locale changes do not fetch records
or translate quotes/comments/names. Dispose releases the subscription. See
[translation.md](translation.md) for dictionary/work bounds and fallback behavior.

## Text popup ownership

`mountNotes` is optional. A host can use its own styles, display rules and action
handlers with the public text-only selection command/events instead. Set the
embedded URL's `notes=host&selectionMenu=host` before navigation to prevent any
default Notes mount. These flags are presentation policy, not storage authority.

An ordinary DOM `mountNotes` can retain its list while disabling popup DOM and
selection listeners with `selectionMenu: false`. Its default display predicate
requires a nonempty, noncollapsed range inside its root. A trusted host can
provide `selectionAllowed(reference)` to override that predicate, for example
`selectionAllowed: () => true` **only when** its provider already requests
`client.request('selection', { textOnly: true }, { signal })`. A host may explicitly approve a PDF region reference (empty quote with a typed
PDF region) for its annotation policy. The default text policy and textOnly
command exclude these regions. Never approve an unfiltered focused-cell or
chart-object reference as text. Existing bounded
`actions` provide host handlers; none are added by default, including Ask AI.
`clearSelection()` cancels pending capture and hides the popup when the host
switches a view/pane; `dispose()` releases the component and all listeners.

Table cell and query components support `selection({ textOnly: true })` with
the same distinction; `selection()` preserves its existing reference shape.
Expanded cells use the selected substring and retain their owning coordinates.

Run `node tests/browser/selection-policy-check.mjs` (and `--webkit`) for focused
DOM policy/lifecycle checks; the embedded reader check covers public settings
and typed source references. These are MarkNoria-only tests.

Built-in Compare preserves the current pane's explicit ownership flags when it
replaces that pane, and preserves the comparison's startup policy on return to
the main reader. Compare Source selections contain only selected source text
(no gutters/signs/truncation labels), use an approximate line location, and do
not claim exact source byte offsets. Mode changes clear the previous selection.
`node tests/browser/compare-selection-check.mjs target/release/marknoria-server`
checks real pointer selection and policy carry through these transitions.

Saved Comments now have [persistent dashed underlines](notes.md#comment-markers). Use
`projection.captureSelection()` / `table.captureSelection()` to retain exact
rendered selectors, `onMutation` to invalidate a mounted marker, and
`notes.openComment(id)` to show its detail. Embedded hosts can explicitly replace
their own bounded mark set with `setCommentMarks`; host-owned Notes never cause
implicit local storage reads.

## Comment markers

The browser, WebView desktop and embedded shared reader draw saved Comments as
thin dashed underlines. Favorites do not create comment marks. A collapsed text
selection, closed Notes panel, replaced projection window or table page does not
delete the saved reference. Notes mutations invalidate the visible mark set;
clearing Favorites leaves Comments intact.

Underlines are non-destructive CSS Highlights, with pointer-transparent clipped
rectangle layers where that API is unavailable or PDF geometry is used. No span
wrappers are inserted into source text, PDF text layers or EPUB CFI trees. Hover a
marked passage to choose its associated comment; overlapping comments have separate
actions. The Notes toolbar also provides keyboard access to saved records. Normal
word selection, links, copy and explicit full-cell viewing retain their behavior.

### Exact references and compatibility

New projected selections carry optional `textRange` alongside the existing typed
`location`. Version 1 contains at most 32 segments:

```js
{
  version: 1,
  segments: [{
    scope: { kind: 'projection', page: 0 },
    startUtf16: 35,
    endUtf16: 46,
    textHash: '/* 64 lowercase SHA-256 hex characters */'
  }]
}
```

Offsets refer to the bounded rendered scope, never original source bytes. The
fingerprint covers its exact rendered text; both fingerprint and surrogate-safe
endpoints must match before painting. Cell scopes use original `{kind:'cell',
table,row,column}` coordinates regardless of visible-column order. SQL result
scopes use `{kind:'queryCell',queryId,row,column}` and remain attached to that live
result; their durable SQL provenance is retained and never re-executed or mapped
to a source cell for painting.

Source references retain original UTF-8 byte offsets and immutable revision.
The source adapter maps all required endpoints in one bounded scan, preserves
BOM/CRLF/CJK/astral characters, and resolves only mounted CodeMirror ranges. Draft
edits or a saved view awaiting refresh do not display revision-owned underlines.
PDF references retain normalized rectangles on original pages; geometry follows
the mounted page at its current zoom. EPUB resolves exact CFIs only within mounted
contents from the same immutable revision. Its display quote may include browser
layout newlines that differ from DOM Range serialization; text is never searched
to choose an alternative occurrence.

Old coarse line/cell notes remain in Notes, with an explanation that an exact
underline cannot safely be restored. Stale revisions and mismatched rendered
fingerprints receive no guessed mark. Old precise source/PDF/EPUB references are
indexed lazily once per notebook identity. Nothing searches the full document for
all copies of a quote.

### Ordinary DOM hosts

Use the same shared components rather than copying renderer code:

```js
import { mountCommentMarks } from './marknoria/comment-marks.mjs'
import { readerNoteStorage } from './marknoria/notes.mjs'

const marks = mountCommentMarks({
  owner,                         // documentId and revision
  root: projection.root,
  viewport,
  load: readerNoteStorage(client, owner),
  open: id => notes.openComment(id)
})
// Pass this to mountNotes's onMutation option:
const onMutation = () => marks.refresh(true)
// Use this instead of the legacy synchronous projection.selection() when saving:
const reference = await projection.captureSelection({ signal })
// MountTableView / mountTableQuery also expose captureSelection(options, signal).
// Keep reference.textRange in the saved anchor, alongside reference.location.
marks.dispose()
```

The projection component emits `marknoria:projectionchange`. No file authority is
granted by mounting static modules. Providers own their scoped storage and must
honor cancellation. A host may omit `load` and use `marks.replace(records)`;
replacing with an empty array clears its owned set without reading local Notes.

### Embedded ownership

`settings({notes:'host'})` still disables MarkNoria's Notes UI and **all implicit
local comment reads**. It is independent of `selectionMenu:'host'`. An explicit
host may call `setCommentMarks({documentId,revision,marks})`. Each replacement
record has `{id,kind:'comment',quote,anchor,revision}`; malformed nested geometry,
selectors, duplicate IDs, wrong revisions and excessive data are rejected before
replacing the prior set. Empty `marks` clears it. This command requires host-owned
Notes; reader-owned marks follow successful local or public Notes mutations.
`commentRequested` reports the requested ID instead of opening MarkNoria Notes in
host-owned mode. Capabilities advertise `commentMarks.version:1` and the record
budget. Studio integration remains the consuming application's responsibility.

The public `notes` command and `Reader::notes` support:

- `{operation:'get',id}`: one full note from this authorized notebook.
- `{operation:'marks',revision,scopes,offset:0}`: an indexed lightweight view of
  Comments for the requested scopes, excluding comment bodies. Scope keys are
  `p:page`, `c:table:row:column`, `q:queryId:row:column`, `s:byteBucket` (16 KiB),
  `pdf:originalPage` and `epub:archiveHref`.

Existing Notes authority is required. Responses include `revision,items,total,
nextOffset,complete`; each response contains at most 64 records and 128 KiB.
Requests contain at most 128 scope keys / 32 KiB and offset <= 10,000. The painter
keeps at most 512 visible scopes, 512 records / 2 MiB and 2,048 rectangles; a single
rendered scope has at most 262,144 UTF-16 units / 32,768 text nodes. Budget failures
are explicit (`data-comment-marks-error` on the main shell), never an unbounded
fallback. Scrolling repaints current geometry; unchanged scope keys do not reread
storage. Reads are serialized per marker owner, successful mutations cancel older
snapshots immediately, and disposal cancels requests and removes only owned layers.

### Local checks

```sh
npm run check:comment-marks
npm run check:comment-marks -- --fallback
npm run check:comment-marks -- --webkit
npm run check:comment-notes -- target/debug/marknoria-server
```

The component contract covers duplicate occurrences, Unicode endpoints, exact
hashes, remounts, overlaps, full-cell dialogs, Source bytes, cancellation, bounded
host replacement and pane isolation. The production check drives actual Notes UI
and Rust storage through save/reopen, overlaps, category clear, table pagination
and Source. Embedded PDF/EPUB checks verify renderer-owned marks. The existing
isolated Wayland desktop table check also saves, pages and deletes a real comment
after trusted double-click selection.

The persistent store uses WAL for reader/save overlap. Schema initialization is
versioned and atomic; first-open Busy/Locked retries recreate only idempotent
connection setup within a bounded two-second interval. Per-connection 64 MiB page
and 1 MiB cache limits remain in effect; retained journals use an 8 MiB size limit.
Scope migration reserves a writer before reading its snapshot, with a read-only
fast path once initialized. Counting, inserting and registering a note share one
immediate transaction, so simultaneous saves cannot exceed 10,000 records.
Transient SQLite contention is `io::ErrorKind::WouldBlock` / HTTP 429, allowing the
reader's existing bounded retry policy; invalid note data stays HTTP 400. User
mutations are never blindly replayed inside the storage layer.

## EPUB selection boundaries

A sandboxed chapter can silently collapse or clear its selection without firing a
usable parent event. The bounded selection-boundary poll must detect that
transition and notify null without traversing unchanged content. Scrolling hides
and suppresses the previous capture; delayed events/getters cannot revive it
without a new selection interaction. Page turns and CFI navigation clear chapter
ranges. Capture the selected spine section's href, not whichever chapter happens
to be at the top of a continuous viewport.
