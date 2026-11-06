# Persistent comment underlines

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

## Exact references and compatibility

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

## Ordinary DOM hosts

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

## Embedded ownership

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

## Local checks

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
after trusted double-click selection. CI remains paused during this acceptance.

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
