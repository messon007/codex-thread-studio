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

The isolated Studio consumer lives under ignored `target/studio-reader-adoption`,
based only on frozen Git objects. Its public API adapter passes two unit tests
that throw on private child DOM/global access. The application compiles against
the exact verified package below without source or manifest patches, using its
own public dependency cache and compatible serde_json/sha2 patch lock. Actual
Studio runtime, UI adoption and downstream CI acceptance remain open.

Fixed package `13b26b569df987b6a42bcd401acb48403fa89e10` has two identical
779-file exports, archive SHA-256
`934ca3b1c37263e6cbe88ab16d6dff028f338c394649c274ab9b83da708596f9`,
verified inventory SHA-256
`107de6d1a351568e69b3e6b2535ab5ceeb6e58256165bba52d0bcf38b3ecdeda`.
Its gitless consumer passes 9 facade + 1 Markdown tests, builds independent
headless/HTTP examples, and passes the packaged two-pane embedding checks,
323 table/comparison checks and 165 static SDK/component checks. These runs use
the consumer's own public dependency/build caches and no application binaries,
private credentials or sibling runtime assets.

The independent browser attempt exposed a missing filesystem sanitizer
dependency: the HTTP reader's alias concealed the absence of
`web/vendor/purify.mjs` in a fresh source package. The build now creates that
entry/license from the same lock and verifies all 35 owned static module import
graphs, including explicit route aliases and root boundaries. The fixed consumer
above passed the complete packaged browser harness after this correction.
These package results do not substitute for actual Studio runtime acceptance.

Still required before closing #4/#5/#6/#7: finish remaining general feature gaps,
complete the actual fixed Studio import and remove absorbed patches, execute the
same generated corpus/actions in its normal and embedded containers, record
visual/accessibility/default-state and performance conclusions, and verify the
downstream matrix/merge. Current remote checks have not executed, and a pending
workflow-scope update still prevents a valid current remote CI conclusion.

At the owner's request, subsequent stages are committed locally; remote pushes
are deferred until the complete change set and local validation are ready.
