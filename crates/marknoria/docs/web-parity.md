# Shared Web reader acceptance ledger

This ledger concerns browser/WebView reading and its actual Rust dependencies.
GTK/AppKit/WinUI remain maintained products, outside Studio's synchronization
package. Frozen Studio runtime evidence is commit
`a2f87a37d5a3d0de7c0dbe0f0f9666e0b13c3a4a`; maintenance rules are frozen at
`c73fdf4b6a5860afb7ef2335e781b86356ebbec3`. Source inspection, ordinary-host tests,
application execution and downstream adoption are different evidence levels.

| Format / action | Shared implementation and defaults | Coordinates and retained state | Verified upstream entry / remaining acceptance |
| --- | --- | --- | --- |
| Small/large Markdown | Rust GFM parsing, hard line breaks, safe HTML subset, shared bounded projection/sanitizer, table component; Preview by default | Original BOM/CRLF UTF-8 source; exact row/cell coordinates; rendered prose references declare approximate source lines | `markdown-profile-check`, `projection-view-check`, `embedded-check`; actual frozen Comrak WASM comparison recorded in `markdown-profile.md`; local-image hydration and relative-link host dispatch remain open |
| Text/code | Rust language registry/highlight projections and shared CodeMirror source/editor; no frontend reparse of complete large documents | Original source windows, line continuations, LF-normalized UTF-16 selections converted to original UTF-8 in Rust | `syntax-check`, `structured-check`, `embedded-check`; complete remaining file/status translation integration remains open |
| JSON/JSONL/TOML/YAML/XML | Rust structured projection and bounded highlighting; formatted JSONL Preview, original Source/Edit preserved; whole-input jq | Exact source revision; jq returns exact text windows without JS number rounding; source and formatted-output coordinates stay distinct | `structured-check`, `jq-query-check`, `embedded-jq-check`, actual Wry document checks; both products must adopt the same default, replacing Studio's old JSON Source default |
| CSV/TSV/XLSX | Shared table view, complete-input Rust SQL, compact defaults, independent table paging, column visibility/resize/fit, edge scrolling | Original table/row/column IDs and sheet identity; per-instance session widths/columns/row; immutable result IDs and revision | `csv-query-check`, `table-interactions-check`, `table-view-check`; dataset/budget profiles in `table-components.md` |
| Table cells / SQL results | Same cell selection, complete-cell dialog/copy, keyboard resize and complete-input charts in ordinary/virtual containers | Exact text/null values; source-cell references; SQL notes retain verified snapshot/query provenance after result close | `table-cells-check`, `table-plot-check`, `table-query-component-check`, real public two-pane host |
| PDF | Original-container PDF.js, text layer, cross-page search, outline, text and Shift-region selection; document-wide zoom and flow | One-based original pages and normalized display-page rectangles; global settings and saved original position; bounded canvases/text | `pdf-reader-check`, `embedded-pdf-check`; no embedded Poppler/backend Chromium requirement |
| EPUB | Original archive/epub.js, exact CFI, chapter outline/search, paginated/scrolled flow, paper/font settings; whole-book progress is explicit opt-in | Full CFIs survive selection/navigation/remount; chapter versus indexed progress precision is explicit | `epub-reader-check`, `embedded-epub-check`, `epub-progress-corpus-check`; protected archive-only resources retain their own revocable capability |
| HTML | Rust static projection and shared sanitizer; scripts, forms, CSS and active content excluded | Original Source/Edit when granted; rendered text is approximate | Frozen Studio's `STATIC_HTML_FORBIDDEN_*` also rejects styles/stylesheets and embedded media: unrestricted styled HTML was not a maintained baseline feature; remaining safe tag/anchor differences require common-corpus review |
| Images | Shared bounded Rust raster/SVG/animation decoding and Web image controls | Original image identity; bounded frames; fit/zoom/animation state | Existing browser/Wry document checks; Markdown-local image authorization/hydration is separate unfinished work |
| Mermaid/PlantUML/D2 | Same owned browser renderer and cancellation/lifecycle profile; host can select browser mode | Document/revision-scoped bounded source; renderer teardown releases workers/observers | `diagram-engine-check`, shared diagram lifecycle in `table-interactions-check`; no AI account dependency |
| Notes / selection actions | Shared Notes UI/storage interface and bounded host action extensions; comments/favorites are maintained reader features | Typed source/cell/query/PDF/EPUB references; scoped category Clear, copy and durable query snapshots; storage is an explicit host grant | `notes-component-check`, public two-pane query-note integration; Studio must connect its storage/AI actions through these interfaces without duplicate generic toolbars |
| Source/Edit/save | Read-only by default; queries independent of writes; explicit writable file grant enables paged editing | Dirty draft versus saved/preview revisions; original identity, atomic save, conflicts, undo and guarded close | Public core tests and `embedded-check`, `editor-check`, `compare-check`; Windows atomic replacement regression exists; current remote matrix is blocked externally |
| Host UI / lifecycle | Versioned exact-origin parent protocol; public view, Contents, search, selection, save, settings, disposal; ordinary components use scoped providers | Separate document IDs/revisions, bounded commands, cancellation, style leases and cleanup; shared locale/palette/fonts/typography | `embedding.md`, `reader-assets.md`, ordinary contracts and two-pane integration; no product host may read `readerExperiment` or private child controls |

## Interaction and settings contract

Table controls, SQL/jq, cells, charts, Notes, PDF and EPUB share their component
shortcuts, labels and defaults; their component docs enumerate provider profiles
and keyboard actions. SQL/jq Run supports Ctrl/Command+Enter. Embedded
Ctrl/Command+O delegates to the host. Editor save uses Ctrl/Command+S and its
shared CodeMirror undo/redo bindings. Escape closes owned overlays. Hosts supply
locale/message dictionaries, common palette and UI/content/code typography through
public settings; document literals and backend errors are not translated or
reinterpreted as UI metadata. Remaining file/status text and cross-shell search
shortcut integration still need final acceptance.

## Downstream acceptance remains open

The isolated Studio consumer is committed locally at
`6cb5ea3` on `sync/marknoria-public-reader`, based only on frozen Git objects.
The original Studio checkout and remote are untouched. Its product adapter uses
only the public client and facade: no child DOM, diagnostic globals, injected CSS,
HTML rewriting or weakened CSP. The old embedding/query patches and orphaned
reader bundle copies are removed. Ordinary Markdown table layout now uses the
complete public SDK namespace. Application formatting explicitly excludes the
unchanged vendor source; the normal Cargo matrix includes the SDK's original
public contract fixtures.

It imports exact fixed package `0861a7b6662581991cf31424962e1c4f26766b6d`:
780 inventory files, archive SHA-256
`136434e10b88314af6aad4fe407966a539aeb961b906c8c27f6a3c5aef58973f`,
tree SHA-256
`93e36a0dc1b4984123114c2250f3919aafc14672402ddeffaf1ac4b7b115f55a`.
Two clean offline exports match. Eight archive/inventory/Git checks include
staging every file with `core.autocrlf=true`; the package's scoped `.gitattributes`
preserves all source/resource/notice bytes. All 781 staged SDK Git blobs,
including the manifest, were independently checked against that inventory before
the local Studio commit. The importer has 13 synthetic tests, including failed
replacement and rollback; a failed rollback retains the previous source.

Studio's local full suite passes 750 UI tests, type checks, 146 application Rust
tests (one pre-existing ignored) and 10 original public SDK contract tests.
Actual gateway/workspace tests pass in Chromium and native Wayland WebKitGTK,
using synthetic documents/configuration and no inherited private credentials.
They cover a 1,941,749-byte Markdown document with a 24-column/120-row table,
complete-table SQL count, paging/column selection without viewport jumps,
Preview/Source/Edit draft preservation, save/undo/redo, conflict/discard protection,
three browser diagram engines, read-only/project/symlink/token/origin boundaries
and owned cache cleanup. Backend Chromium is deliberately unavailable. Runtime
inputs in package `0861a7b` are byte-identical to the tested `978df425` package;
the later package adds only byte-preserving Git attributes and its contract check.
The latest exact package also builds and passes the full UI/type/format checks.

In one warm local run, indexing took 298 / 233 ms with worker peak RSS
65,624 / 62,504 KiB for Markdown / diagrams. Host opening to first useful text
took 540 / 367 ms; reader-internal content times were 92 / 192 ms, with measured
opening heartbeat gaps 21 / 20 ms. Chromium mounted 739 / 15 nodes then; WebKit's
later source-check points mounted 1345 / 1574. These are different measurement
points, not matched visual snapshots, cold-start guarantees or whole-application
memory. First text in a diagram document is not SVG readiness; the test separately
waits for all three rendered figures. The shared host setting now also prevents
duplicate status/Save/Reload controls, and forced parent clients release their
listeners after a lost-frame timeout while rejected dirty-close remains usable.

Earlier independent gitless package `13b26b5` passed 9 facade + 1 Markdown tests,
its packaged two-pane embedding checks, 323 table/comparison checks and 165 static
SDK/component checks with only its own public dependencies and builds. This
exposed and fixed the fresh-package sanitizer import missing from the HTTP alias.
The build now verifies all 35 owned module import graphs. That independent
consumer evidence remains distinct from Studio runtime adoption and from final
all-format acceptance.

Still required before closing #4/#5/#6/#7: finish remaining general feature gaps,
complete the actual fixed Studio import and remove absorbed patches, execute the
same generated corpus/actions in its normal and embedded containers, record
visual/accessibility/default-state and performance conclusions, and verify the
downstream matrix/merge. Current remote checks have not executed, and a pending
workflow-scope update still prevents a valid current remote CI conclusion.

At the owner's request, subsequent stages are committed locally; remote pushes
are deferred until the complete change set and local validation are ready.

## Local follow-up: public images, exact source restoration and Studio PDF

Isolated Studio commit `691f612` consumes the unchanged public package from
`c71f5015294757e9177eb9c5a9ea72c8a1fd3006`: 791 inventory files, archive SHA-256
`0781fb4ca3ab53d90597ba831a9cabde651b44d9bb894c8d112f29f858a2c540`.
Two clean offline exports match; all eight export/Git-byte contracts pass.
The public image component/PNG Blob protocol passes Chromium and Wayland WebKit
contracts. Exact source-range navigation retains BOM/CRLF/Unicode byte anchors
and declares its capability; bounded explicit windows can cross 200 source lines.

Studio now delegates static module inventory to `reader_assets()` while retaining
its document API/origin/token/grant checks. This removes the duplicate resource
list that initially omitted the new image module. Typed comments preserve the
public location/revision/precision instead of reducing references to line numbers.
Actual Chromium workspace capture verifies this outer adapter; gateway-served
reader frames pass the existing native Wayland WebKit reading/editing checks.
These are separate validation scopes, not a full WebKit Studio shell test.

Studio's duplicate PDF renderer is replaced with a thin shared-component adapter.
Twelve assertions pass in each engine using its actual gateway/static assets and
authorized original bytes, including zoom/flow, outline/search, typed regions,
stale references and cleanup. The latest consuming checkout passes 759 UI tests,
type checks, 147 application Rust tests (one existing ignored) and 11 exact SDK
contracts; its inventory remains unchanged after building. One warm debug run
measured host first useful content at 467 ms for each generated document, with
opening heartbeat gaps 23.3 / 20 ms. These measurements precede selection actions
and do not establish release or cold-start performance.

No platform-native UI, shared native controller or FFI changes are part of this
stage. The original Studio checkout remains untouched; both repositories' new
commits are local. All-format adoption, remaining Notes/storage/AI wiring,
Markdown resources, visual/accessibility acceptance and current remote/platform
CI remain open; these checkpoints do not close the synchronization issues.
