# Shared Notes component

`web/notes.mjs` exports `mountNotes(options)`. The main reader delegates to this
same component; the UI owns selection controls, comment/favorite dialogs, category
filters, confirmation, copying and presentation. Storage and additional actions
are explicit host adapters. There is no AI account, gateway, file picker or file
path capability in the component.

```js
import { mountNotes, readerNoteStorage } from './marknoria/notes.mjs'
const owner = await client.ready()
// Hide the child's own Notes UI while retaining its explicitly granted store.
await client.request('settings', { notes: 'host' })
const notes = mountNotes({
  owner, // documentId, immutable revision, display name
  root: readingSurface,
  toolbar: documentTools,
  overlayRoot: overlayLayer,
  presentationRoot: readingSurface,
  selection: async signal => {
    const value = await client.request('selection', {}, { signal })
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

The ordinary-host contract passes 20 assertions in each engine and 21 in the
compact viewport. Compact list summaries keep one complete text line plus
metadata; full text remains in the independently scrolling detail pane. The
production regression also verifies embedded asset bytes match the checkout.

`translate` is an optional `translate(key, variables)` host callback. Passing the
shared `createTranslator` also updates existing dialog/toolbar/accessibility and
date labels through bounded subscriptions. Locale changes do not fetch records
or translate quotes/comments/names. Dispose releases the subscription. See
[translation.md](translation.md) for dictionary/work bounds and fallback behavior.
