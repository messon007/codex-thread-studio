# Full native GNOME migration

The goal is the entire Codex Thread Studio application on Ubuntu 26.04 GNOME
Wayland, using GTK4/libadwaita for its application UI. The working conversation
client is a checkpoint, not completion. Implementation order and technical details
are owned by this work; the user does not need to prioritize individual features.

## Constraints

- Preserve the existing Tauri frontend, remote server, supported platforms, and
  persisted data while developing the independent `native/gnome` workspace.
- Reuse the Rust gateway, backend isolation, authentication, profile locking,
  SQLite, workspace confinement, save conflicts, environment settings, and
  managed-process cleanup. Do not introduce incompatible copies of stored data.
- Application controls, conversations, composer, menus, navigation, dialogs,
  document controls, and settings use GTK4/libadwaita widgets.
- Parsing, indexing, search, transformations, protocol normalization, and caches
  belong on bounded Rust workers. GTK owns widgets, layout, painting, selection,
  accessibility, and input. Large files must remain bounded/paged in native UI.
- Evaluate mature native libraries first: GtkListView/ColumnView, GtkSourceView,
  GTK4 VTE, Gio/portals, and reviewed document/vector engines. Review licenses.
- Existing website browsing and actual HTML content may require a GTK4 web-content
  engine. Their application controls remain native. This must not become a hidden
  HTML application frontend or a WebView-based large-Markdown reader.
- Use Wayland for native Linux launches and tests; do not force X11.
- Preserve explicit save/page-change save, conflict checks, and in-memory dirty
  edits across view switches. Do not add persistent drafts or background saving.

## Order and parity inventory

Use the [current parity inventory](gnome-native-status.md) for present gaps and
implementation order. Native turn supervision now includes the Rust worker,
explicit running-turn controls, bounded isolated evaluation and continuations,
authoritative source/ownership checks, source/controller queue coordination,
Router handoff and shutdown without replay. Cold target-side opt-in resolves
indexed ownership metadata before evaluation; it does not load hidden replies
or start a provider. Current-turn menu status and Stop remain scoped to their
actual source and generation.

At the supervision checkpoint, 392 native tests and 257 headless gateway tests
passed (four and one opt-in
ignored respectively), with Rust 1.92 and the existing GTK3/Tauri Linux check.
Matched debug/release pairs are built. Private Wayland covers ordinary and
restart supervision, broad controls and both Map regressions. The installed
optimized pair passes AT-SPI supervision acceptance without application hooks,
including held/released queues, exact continuation, preserved drafts, durable
handoff and Stop without restarting automation. See
[restart and installed supervision](gnome-native.md#native-supervision-restart-and-installed-controls).

Browser-created Router history now has a separate native reading view over the
original shared records. It pages/searches metadata without starting a provider,
then hydrates bounded Codex/OpenCode previews with exact source identities and
without resending or assuming delivery ownership. All 393 native and 260 gateway
tests pass, with Rust 1.92 and the existing Tauri Linux check. Matched optimized
binaries are packaged, and installed archive controls pass without application
hooks or gateway overrides. Existing installed workflows, Router execution and
supervision restart also pass. The reading gap is closed; saved delivery status
remains historical and source previews are explicitly bounded. See
[browser Router history](gnome-native.md#native-browser-router-history).

Router history captions and sidebar activity/pin prefixes now follow live
language changes without translating literal session names or changing state.
Named GTK HTML-menu actions pass assistive-client Copy/Select all/Translate,
Menu/Shift+F10 and actual right-click coordinate checks. The matched optimized
package passes installed language/menu/Map controls, independent installer checks
and isolated Ubuntu runtime CLI startup. See
[native language and HTML menus](gnome-native.md#native-router-language-and-html-menus).

Native message-flow settings and Continue now preserve shared queue depth and
the three saved session-model/Ollama draft versus explicit quick-send behaviors.
Installed GTK acceptance verifies cancellation, stale draft protection, scoped
keyboard activation, model/effort/Fast options and exact utility cleanup. All
394 native, 264 gateway and 749 frontend tests pass, with Rust 1.92, existing
Tauri compatibility and matched package checks. The continuous-reader content
font regression is reproduced and fixed. See
[composer Continue and message flow](gnome-native.md#native-composer-continue-and-message-flow).

The compact native turn navigator now passes installed first/middle/last and
keyboard jumps, both providers' three-page history and source/draft restoration,
plus Codex live/historical reading and delayed old-source reply rejection.
Matched optimized packages and 20 layout cases pass, with all 395 native tests,
Rust 1.92 and the existing Tauri Linux check. Separate 640 px mapped popup captures
exposed and verified a shared popover-font fix. The latest candidate includes
navigation, Continue and the preceding features. See
[native turn navigation](gnome-native.md#native-turn-navigation).

Router unread cues now use owned terminal-job metadata for the actual prompt
page, with in-place live-language updates and unchanged read/ownership guards.
Installed no-hook acceptance covers actual question/response-end semantics,
paging, source restoration and untouched browser/foreign-controller records.
Matched packages pass 395 native tests, Rust 1.92, 20 layout cases and installed
navigation/provider/core regressions. See
[Router unread cues](gnome-native.md#native-router-unread-turn-cues).

New session and image controls now pass explicit language bindings and installed
portal/viewer acceptance, including preserved literal source values and zoom.
See [session/image language](gnome-native.md#native-session-and-image-language-controls).

Tables, EPUB, conversation search and Terminal now pass explicit language
bindings and full installed acceptance, with shared adaptive UI choices, literal
source preservation and a reproduced/fixed EPUB section-input relayout race.
See [reader/search/terminal language](gnome-native.md#native-reader-search-and-terminal-language-controls).

Next finish the status/inspector and remaining control audit, review and commit
the migration sources, then complete renderer compatibility and release/distribution
acceptance. Explicit pending-alignment wheel interruption, physical desktop and real-
provider acceptance remain separately recorded. The entire-application goal remains
active; the prompt-navigation checkpoint is not full migration completion.

The notes below are chronological checkpoints; later work
supersedes their older “remaining” lists. Structured-file and passive HTML
Preview and generic-file Reload are implemented. HTML persisted comments and
focused translation now have private workflow acceptance; menu assistive-client
activation remains unverified. Continuous native Preview is now the normal reader, including comparison panes.
Native session Resources now covers latest-turn indexing, search/type filters,
occurrences, viewer returns and shared favorites. Native session information now
supports opening-question editing/copy, explicit shared-store saves and same-session
AI responsibility generation/Stop, reusing Router's responsibility editor. Two-provider
Wayland checks cover stale contexts, draft preservation and conflicting saves.
Connection diagnostics now provide a native menu/settings view over existing
metadata endpoints, separate live event-stream status and bounded explicit Refresh.
Private Wayland verifies failures, literal text, stale reads and gateway recovery.
Native activity inspection now preserves reasoning-content fallback, command
arrays, tool input/error/status and file-change kinds through the existing bounded
snapshot viewer. OpenCode retains original tool metadata and server IDs; SDK
metadata-only updates rebind the action without rewriting an open snapshot. Plan
notifications replace a stable row without changing session activity. Both-provider
Wayland, 6.1 MB output, failed-write and installed AT-SPI checks pass, along with
330 native tests, 749 web tests and Rust 1.92 release-target checks. See
[activity inspection](gnome-native.md#native-activity-inspection). History paging
and shared transcript actions now have explicit live language bindings as well.
Preferred/compatibility Codex and OpenCode Wayland checks retain page/live handles,
selection, reading anchors and drafts; loading/error text and literal provider
metadata stay scoped. The installed release verifies the same labels through
Settings and real history controls. Remaining owned controls/accessibility,
compatibility and final release acceptance remain in the full migration. See
[history language](gnome-native.md#native-history-and-transcript-language). The composer accessible
instruction and Router target chooser now use explicit language bindings too.
Real three-launch Router checks retain target identity, query, row widgets,
model settings and drafts; installed AT-SPI checks verify the composer name in
both languages. This closes those control gaps while the remaining
compatibility/accessibility gates remain active.
Physical acceptance remains separately recorded. See [structured validation](gnome-native.md#native-structured-file-preview)
[HTML validation](gnome-native.md#native-passive-html-preview) and
[file Reload validation](gnome-native.md#native-ordinary-file-reload). Image-region
comments and advanced review comments are not assumed requirements absent an
equivalent existing application feature.

Turn-supervision backend integration now includes authoritative reads, isolated
evaluation, guarded source dispatch and socket-owned durable Router handoff.
Transactions prevent stale reply/source rebinding; Stop, recovery and prior Map
updates remain guards. Native controls, queue coordination and scoped settings/
human-action hooks are now present in the working tree, with 389 native tests and
actual private-provider Wayland workflows. The preceding gateway checkpoint
passes 255 tests. A separate debug bundle passes broad controls and both Map
regressions; final release/installed-control acceptance remains.
Before release, close target-side/cold-start Router ownership and stale terminal
status presentation; the qualified routed fixture starts from the selected
Router. See [supervision controls](gnome-native.md#native-turn-supervision-controls).

Continuous Preview is now enabled for ordinary launches. Installed controls
verify the default text surface and exact cross-block Copy without an override;
the same build passes all installed groups and the real report plus generated
32 MiB LF/CRLF reading/editing checks. Legacy label/model fixtures explicitly use
the retained diagnostic adapter. See [default-reader validation](gnome-native.md#default-native-markdown-reader).

Continuous-reader accessibility now retains child-anchor coordinates in its
native accessible text interface and exposes formula alternatives. A private
AT-SPI client verifies counts, ranged/line text, caret/selection and visible table
content. Opaque-object selection outlines pass light/dark pixel and invalidation
checks without boxing inline formula glyphs. Existing rich-text, translation,
actual-report and installed workflows pass. Workspace-return/transfer lifetime
is now also verified: retained documents use renewable visible-period scopes,
while immutable snapshot validity survives temporary hiding. Real pane returns,
delayed Copy/comment reads and stale drag offers pass alongside existing checks.
These preceding interaction checks are included in the default-reader promotion;
remaining application controls/compatibility and release acceptance stay active. See [native accessibility validation](gnome-native.md#continuous-reader-accessible-text-and-object-selection)
and [retained-reader validation](gnome-native.md#continuous-reader-workspace-return-and-transfer-lifetime).

Continuous-reader input now has actual keyboard/pointer/drop acceptance. Full
native document drags use the same bounded Rust projection as Copy/primary
selection, preserving formula alternatives and indexed-table rows beyond the
visible page. Stale ranges, replaced buffers and retired views reject transfer.
The matching rich-paragraph, translation, actual 6 MB report and installed workflow
checks pass. Default promotion still needs assistive-text, embedded-object
selection-presentation and compatibility acceptance. See
[projected drag validation](gnome-native.md#continuous-reader-projected-drag-and-input).

Continuous Preview now has projected Translate capture through the actual native
context action and Ctrl+Shift+T. A reusable Rust prefix cursor and snapshot-scoped
table endpoint return only the existing 16,000-character translation window,
without requiring a complete large-table copy. Real private Wayland checks use a
33.5 MB table, exact provider text/cache, and a paused read rejected after a Source
switch. An API measurement visits six fragments / 17,646 budgeted source bytes.
Core/gateway/native tests, Rust 1.92 and the existing Linux Tauri build pass. The
`native-flow-prefix-20261103` matched optimized pair also passes the current real
report and all installed workflow/recovery groups, preserving the earlier bundle.
This closes the missing translation action; normal continuous-reader promotion and
broader input/accessibility gates remain. See
[continuous-selection translation](gnome-native.md#native-continuous-selection-translation).


Public HTTP(S) images now use bounded Rust I/O, validated/pinned DNS per redirect,
Ubuntu system TLS trust and the existing native decode/viewer/original-Save path.
Private-address canaries stay unfetched; image requests never receive gateway or
browser credentials. Real private-Wayland chat/attachment/reader PNG/SVG checks,
exact saves, embedded-image regression and Rust 1.92 checks pass. The independent
GNOME TLS graph/notices do not alter the other clients' dependencies. Normal
unit tests remain offline; public artwork acceptance is an explicit opt-in.
The optimized `native-remote-images-20261103` bundle passes the installed workflow
with all groups, process/event-gap recovery and actual HTTPS original Save. The
preceding 486-file Mermaid package remains unchanged; default installation was
not touched. Exact source/package evidence is retained in its acceptance record.
See [remote images](gnome-native.md#native-remote-https-images).


The native diagram queue now uses bundled Merman 0.9.3 for Mermaid. The shared
appearance dialog exposes Auto/Classic/Neo/Hand drawn/Document and Auto/Dagre/ELK,
as well as text size, spacing and curves. Two real Wayland launches verify saved
styles/layouts, matching reader/chat pixels, font changes, restart, stale replies
and unsaved-buffer retention. The accepted private screenshots were inspected.
The independent worker requires Rust 1.95; the GTK parent stays on Rust 1.92.
Normal build/package/install scripts include the verified worker and exact
EPL/MIT/OFL notices. CI builds/tests the worker and rejects stale/tampered packages.

Font preparation and environment-aware image caching use the bounded Rust lane.
The adapter validates exact selected font/worker resources around fresh results
and cache hits; vector zoom does not enumerate system fonts again. Earlier
mixed-script, inline-family, code-role and author-color fixes retain their pinned
source provenance and geometry guards. The 59 valid corpus inputs pass through
the actual namespace and process limits. Their geometry/paint bytes are unchanged;
protocol 3 adds only a bounded count of actual generated math SVGs before
outlining, so GTK acceptance never infers success from source delimiters.
Supported math islands now survive an unsupported sibling, whose source stays
XML-escaped and literal; GTK still verifies real counts of 3/7. Class-member math
remains literal, and dynamic families or
unsupported HTML geometry remain explicitly guarded. The outlined 80-node case
still regresses against Pele; this adoption does not claim universal speedups.
See the [worker contract and evidence](../native/mermaid-worker/README.md).

The preceding `native-mermaid-gtk-math-20261103` optimized candidate includes
this renderer, appearance controls and verified worker packaging. Its complete
native, two-launch appearance and ordinary installed release workflows pass;
worker regressions pass 20 tests and the selected-feature renderer suite passes
1,029 tests. Source/package fingerprints and the original math failure are
preserved. The previous integration package remains unchanged.

The earlier `native-integration-20261103` optimized candidate combines the large-file
context fixes, PDF live language controls and Router alerts. The normal installed
workflow passes every existing group plus PDF page/query/text/clipboard retention
across English/Chinese changes, process/event-gap recovery, the actual 8.3 MB and
32 MiB LF/CRLF documents, and real GNOME/Nautilus chooser operations. Build inputs
and packaged files are fingerprinted; old candidates/default installation remain
unchanged. Linux Tauri compilation still passes with the isolated GTK3 SDK. This
is a testing candidate, not full migration completion or physical/provider/
fresh-machine/platform sign-off. See [earlier integrated candidate](gnome-native.md#earlier-flow-prefix-candidate).

The installed large-document checkpoint now covers the actual 8.3 MB report and
both LF/CRLF 32 MiB generated tables through the normal launcher and GNOME
controls, without application hooks. It exposed and fixed blank EOF Source,
post-save editor/undo context loss, and column changes rounding a wide table's
top row back to zero. The matched optimized `native-large-context-20261103` pair
passes this workflow, the complete installed core groups/recovery and the document
API regression; earlier packages remain unchanged. The API row-cursor check is
included in CI. See [large-context acceptance](gnome-native.md#current-large-context-candidate).

A current optimized native/gateway pair is now packaged with its pinned optional
runtimes and notices. The refreshed `native-full-refresh-20261103` pair includes
the current recovery/rendering/animation source and passes the installed workflow
with all optional groups and process/event-gap recovery. Actual native
GIF/APNG/WebP controls, Map/Undo, bilingual Favorites/Comments and optimized image
API checks pass without application hooks. Previous installed packages and the
20261102 candidate remain unchanged. A staged installed-pair check uses the private GNOME
accessibility bus and owned Wayland input, without application test hooks or a
gateway-location override. It covers conversation loading, selected formula
Copy, Source/Preview/Edit, Undo/Redo, dirty view switches, explicit save,
desktop-entry file handoff, draft retention, lazy web engines and process cleanup.
It exposed and verified a fix for new Source/Edit pages leaving their cursor at
EOF and causing the next view switch to open the empty last page. Native tests
and the four-launch real-report regression pass after that fix. This is staged
host acceptance, not a fresh-machine/physical/provider sign-off or completion of
the full migration. See [native release validation](gnome-native.md#native-release-validation).

### 1. Shared gateway and shell

Initial checkpoint implemented: separate GTK4 frontend, gateway-only build without
Tauri/GTK3/WebKit, owned authenticated loopback process, parent-pipe cleanup,
GNOME identity/launcher, build/install scripts, native session catalog and search,
Codex history/streaming, Send/Steer/Stop, model/effort selection, initial approval/
input dialogs, virtualized transcript, escaped worker-side Pango chat markup,
adaptive sidebar, and system/light/dark appearance.

Backend connection generations now isolate stale events/replies; forced reconnect
restores the selected conversation and its in-memory draft. Remaining shell work
includes background lifecycle coverage, persisted
advanced settings, IME/clipboard/accessibility polish, and
expanded native approval/input acceptance tests.
Native appearance/window/sidebar/selection preferences now persist through the
gateway using atomic worker writes. A second Wayland launch verifies the saved
theme/session. Native pin/unpin shares the existing SQLite store and ordering.
Model, effort and Fast controls now share per-session SQLite options. Native
capabilities/inheritance handling supports Fast from resumed CLI sessions, explicit
`default`/`fast` overrides and copying options to forks. Wayland checks verify the
actual request parameters and persisted values; OpenCode hides Codex-only options.

### 2. Native documents and large Markdown

Implemented checkpoint: GTK-independent Rust block/edit library; native
Preview/Source/Edit and bounded context windows; native paged ColumnView tables,
column selection, wrapping/resizing; exact source gutters; outline/search;
explicit/page-change saves, atomic conflict checks, Unicode/CRLF and undo/redo.
Large-report and 32 MB table tests pass with the GTK main loop running. Native
Mermaid generation/rasterization passes 16 representative diagram families;
PlantUML/D2 subprocess rendering and private-file isolation pass. Visual diagram
acceptance and remaining document parity below are still being completed.
Diagram work now uses an independent bounded render lane with cancellation and
a shared memory-weighted raster cache. Tests keep document control requests
responsive while an engine is busy and cancel queued/running work on close.

Standalone display formulas and `math`/`latex`/`tex` fences now use native RaTeX
0.1.15 Rust parsing/layout and glyph-outline SVG through the same bounded render
lane/cache. A resource-limited worker starts before GTK/profile initialization;
no TeX or browser runtime is needed. A reproducible two-engine comparison,
Rust 1.92 compatibility check, private-worker limits/errors and actual Wayland
reader/chat theme and Source/Edit checks pass. Inline/mixed math now renders in
short native labels, ordinary table cells and complete formulas in bounded
excerpts. Expanded cell/header reading windows now use bounded formatted label
pages. Bounded multiline paragraphs now use native text child anchors with
original-text selection maps. Long paragraphs now share this renderer through
formula-aware native pages. The earlier Mermaid math checkpoint used Pele's resolver hook and
the existing isolated Rust math worker; the GTK queue now uses Merman's native
RaTeX path through the isolated Mermaid worker described above. Formula metrics participate in diagram
layout; per-diagram source/job/output bounds, cancellation and the remaining
Mermaid deadline apply. Real flowchart/sequence images, original-source Copy,
unsupported literal labels and theme replay are covered; broader type/style
acceptance remains. See [Mermaid label math](gnome-native.md#math-inside-mermaid-labels).
See [math measurements and bounds](gnome-native.md#native-display-math).

Inline-math preparation now preserves original source byte/line ranges and
rendered byte/Unicode ranges on Rust workers, with bounded metadata and shared
cache accounting. The isolated math worker supplies compact text/display styles
and baseline metrics. Production short labels and ordinary table cells use
Pango/Cairo shapes while retaining original TeX characters and selection/copy
offsets. The shared queue/cache has bounded admission and Cairo pixel accounting;
theme/typography changes, equal-text label reuse, late conversions, unmap/remap,
Source/Preview, exact native copy, source-line comments and link dispatch pass
the owned Wayland fixture. Long-cell excerpts now retain worker-selected indices
for complete formulas within the 256-byte prefix and share the same native
renderer, without copying the full cell or changing TeX/source offsets. Clipped
formulas remain literal. The normal/continuous table fixture checks themes,
independent native Copy, original line-14 comments, complete-cell Copy and label
lifetime guards. Expanded cell/header math now shares worker-prepared 4 KiB
label pages, preserving atomic formulas, exact native Copy and original source
comments through page changes and close. See [paged cell math](gnome-native.md#math-in-expanded-cells-and-header-readers).
A private multiline probe confirms that label shapes retain internal hard
breaks, while single-line mode also collapses outside breaks. It exposed and
verified a CRLF fidelity fix: worker-prepared exact-text routing preserves styles,
links, native Copy and source-line-3–4 comments instead of normalizing quotes.
See [multiline layout and source fidelity](gnome-native.md#multiline-math-layout-and-source-fidelity).
The shared paragraph widget now renders multiline formulas in at most 8 KiB of
already-parsed rich text, retaining outside breaks and original CRLF through
Copy, primary selection, dragging, source search and comments. Workers prepare
the projection and rasters; GTK owns text layout and baseline-aware child
widgets. See [bounded multiline paragraphs](gnome-native.md#bounded-multiline-math-paragraphs).
Formula-aware pages now retain complete expressions, indexed styles/links and
original source positions through long-paragraph navigation. Heavy markup also
uses the object path, with shared native style tags. Large failed expressions
stay readable without expensive wrapping. See
[math across paragraph pages](gnome-native.md#math-across-paragraph-pages).
Dense-math throughput and expanded find/bidi/accessibility acceptance remain.
See [native inline math](gnome-native.md#native-inline-math).
The paged-math fixture now also verifies equal-history ownership, repeated
Source/Preview selection restoration and foreign primary preservation. The
real-report regression passes with and without protocol logging. An intermittent
unallocated-GtkGizmo snapshot warning is now traced to the SourceView scrollbar
during GTK's normal paint. Natural-frame screenshot validation replaces forced
widget allocation and frame-clock emission; the full four-launch real-report
workflow passes. The warning persists, and its relationship to an earlier
Wayland disconnect remains unproven. See the diagnostic evidence in
[math across paragraph pages](gnome-native.md#math-across-paragraph-pages).

Native CSV/TSV/XLSX viewing now uses a shared, presentation-neutral Rust snapshot
service. GTK receives bounded pages, not whole workbooks. Data/SQL tabs, delimiter
selection, resizable wrapped cells, full-cell copy/viewing, compact SQL, worksheet
selection and native charts pass the Wayland fixture. SQL reuses the existing
read-only in-memory SQLite engine for projection, filters and sorting. CSV uses
the `csv` crate; XLSX uses Calamine's cell reader without allocating a dense range
from declared sheet dimensions. File opening also works from Files and chat links.
The private 3.26 MB and 200-column benchmarks retain main-loop responsiveness.
These formats are read-only. Native single-cell comments now preserve canonical
file/sheet/range/hash identity, headers, delimiter and the applied SQL for result
cells. Reopening checks the hash and quoted value, restores the sheet/page and
highlights only a verified cell. A changed file never silently reruns stored SQL.
Five-launch CSV and XLSX fixtures and a six-launch SQL fixture pass source,
restart, legacy, mismatch, rejected-query and session-switch checks. Remaining
comment coverage and export stay in scope. See
[native table comments](gnome-native.md#native-tabular-documents).

Native Markdown comments now capture Preview block lines or exact Source/saved
Edit line ranges, retain shared provider anchors, and support native note editing,
Save/removal, source reopening and idempotent composer insertion. A narrow shared
GET and optional revision-checked PUT use the existing SQLite tables; legacy
clients keep their original response/behavior. Atomic stale-save rejection,
local-edit retention, reload cancellation/recovery, native dirty guards and
restart restoration pass the private API/two-launch Wayland fixtures. Direct
batch submission now uses the existing Codex/OpenCode send and Router target
acceptance paths. Atomic exact-record cleanup, newer local/shared edit merging,
frozen prompts, duplicate prevention, rejected-send retention and cleanup-only
retry pass private Wayland/provider fixtures. Same-record conflicts block saves;
accepted cleanup failures retain close guards. Pending acceptance receipts are
not durable across an app crash, and sends are never replayed automatically.
Markdown source reopening now verifies quotes against a fresh immutable reader
snapshot on Rust workers. Source text uses bounded streaming; Preview reuses the
same bounded reader blocks, including table-header identity. Unique moved quotes
relocate; duplicates, missing passages and rendering limits leave an explicit
unverified Source view. Existing comments are not rewritten, and source opens
retain the dirty-editor Cancel/Save guard. Remaining selection/capture parity is
tracked below. See
[native session comments](gnome-native.md#native-session-comments).

Native chat text/code/table selections now add comments through the existing
session library, including older-history pages and Router source identities.
The two-launch GTK fixture verifies explicit Save/restart, legacy normalization,
draft-preserving insertion, live menu language and stale/recycled/oversized
selection rejection without model prompts or web-content engines. Browser
character offsets are left unset. Markdown Preview code blocks and indexed table
cells now capture comments with immutable document generations/revisions and
containing source ranges. Expanded full-cell windows support the same action in chat and
the reader; the literal quote is frozen before returning to the Comments dialog.
Synthetic excerpt ellipses stay out of selections. The private GTK fixture verifies
Save, shared SQLite records, source quote reopening, and stale file/page rejection.
Selection across independent widgets remains parity work; the full scope is intact.
An isolated GTK TextView/child-anchor experiment now passes physical cross-block
selection, exact independent clipboard checks and real Mermaid/PlantUML/D2/math
rendering. Its worker-side projection retains original alternatives/source
ranges outside GTK, with bounded cell excerpts and paged native tables. The real
4.1 MB report's first 64 KiB shows that compact anchors alone do not remove table
layout cost; bounded row pages improve repeated allocation. This is not enabled
as the default application renderer: table math, remaining styles/controls,
broader input/accessibility and cold-layout acceptance remain before promotion.
The prototype now imports the native library and reuses the actual rich/full-cell
widgets and header menus. Shared production link input uses character hit-testing
and rejects unmapped actions; real pointer/keyboard checks cover narrow glyphs,
Unicode link endings, blank space, full-cell Copy/close and owner cleanup. The
production long-rich-text regression and Rust 1.92/native/shared suites pass.
The four-launch real-report regression and optimized installed pair also pass
after the library extraction, including all five installed workflow groups.
The default reader keeps its block renderer; current package hashes and the retained
previous pair are recorded with the text-flow evidence.
The prototype now prepares syntax ranges and both code themes on Rust workers,
using Syntect with the extended two-face bundle. Common-language checks include
TypeScript, TSX and JSX. The same GTK workload favored statically linked
Oniguruma over fancy-regex for cold latency and memory; the native adapter uses
that backend. Theme replay preserves source/selection/copy/dirty state, limits
retain complete plain code, and replaced buffers reject stale styles/copies.
Core/default/native checks, actual GTK input, source lifetime and reproducible
grammar notices are covered. See [code-color measurements](gnome-native.md#native-worker-code-colors).
Image anchors now carry Unicode alt ranges and use the existing document-bound
image renderer, including table cells and header reading windows. Full copy
retains complete alt text even when the visible caption is bounded. Compact
snapshot handles retain reader/revision/page/image identity; images outside
the viewport reuse the existing admission and texture-release lane. Private
GTK acceptance uses synthetic decode replies, while the separate gateway image
suite checks actual PNG/SVG decoding and resource confinement. See
[image-anchor acceptance](gnome-native.md#native-text-flow-image-anchors).
Chat, indexed document tables and the continuous prototype now share a virtual
GTK column picker with original-index identity, one-column/selection-limit
guards and live UI language bindings. The actual 2,000-choice GTK test covers
recycling, native keyboard input, literal source headers and complete fragment
copy; the application reader retains resized widths across filtering and rejects
requests from a replaced document. Column choices stay in the open table's
memory. See [shared column controls](gnome-native.md#shared-native-table-column-picker).
The document-bound context comment adapter now extracts Copy-compatible text on
the shared bounded worker lane, freezes the destination session, and rejects
changed modes, buffers, revisions and conversation ownership. Its opt-in `flow`
quote kind streams across Rust index fragments without retaining the document.
The actual Comments Save/reopen/restart workflow preserves a Unicode/math/table
quote and containing source lines after Edit/Save changes the revision. Legacy
Source and Preview quotes retain their behavior. See
[continuous reader comments](gnome-native.md#native-continuous-reader-comments).
The experimental reader adapter now routes the existing header Comment button
through its worker-backed capture action. A bounded weak registry tracks actual
read-only text leaves, preferring the focused/recent selection; switching between
a cell and the projected document preserves quote kind and original source lines.
The real reader container fixture covers toolbar/context actions, successful
status recovery, old-revision rejection and Save/reopen/restart. Existing native
long-paragraph and expanded-cell workflows pass after the registry change.
The continuous adapter now sizes table anchors from GTK's allocated text width
instead of requesting 720 pixels. Native measurement allows the document pane
to shrink; the shared wrapping toolbar keeps its controls reachable. Resize
acceptance preserves column widths, horizontal scrolling, source/selection and
complete projected Copy while checking idle allocation. See
[responsive tables](gnome-native.md#responsive-continuous-reader-tables).
Bounded-fragment Wayland primary selection now exports the same original text as
Copy through a lazy, bounded Rust worker. Separate-client reads and native middle
paste cover stale/foreign ownership, buffer replacement and byte-budget rejection.
See [primary selection](gnome-native.md#continuous-reader-primary-selection).
Complete table math, remaining styles/controls, cross-page selection
and cold-layout/input/accessibility acceptance still
precede promotion into the application reader.
Standalone diagram preparation now has a metadata-first path. The experimental
reader mounts the shared asynchronous native diagram controls, with independent
generation routing, Source/inspection/theme handling and a retained 32-MiB
preview budget. Leaving the adapter cancels work and releases textures; the
selection fixture explicitly remounts old views to keep stale-revision coverage.
Indexed anchors now retain table identity/row offsets and mount the production
reader's bounded backend paging, column controls and cell viewers. Separate
registries, request serials and mode epochs reject stale factories/replies and
release rows. Snapshot-backed selections now resolve complete atomic tables on a
bounded independent Rust read lane for Copy, primary selection and comments;
view changes cancel pending reads and detach expired table anchors before
unrooting. The real-report four-launch check passes with GTK criticals fatal.
General cross-page selection remains open.
Diagram admission now uses
shared native viewport geometry, updates after each affected window paints,
cancellation and geometry-preserving pixel release. Table images reuse this same
scheduler; it fixes a reproduced stale-scroll-transform race in the earlier timer
approach. Native loading overlays retain layout while previews reload. Failed
engines settle until explicit retry; real-engine acceptance visits visible cards
sequentially. Snapshot-scoped worker contexts now coalesce valid adjacent indexed
table continuations, preserving exact cells/source coordinates and rejecting
inconsistent ranges. The real-report check joins two actual index-page ranges
and compares all 32 rows against the backend table response. Cross-page selection
and broader input/accessibility acceptance remain before default promotion; the
ordinary block renderer remains active. See
[deferred diagrams](gnome-native.md#deferred-native-diagrams).
See [viewport admission](gnome-native.md#diagram-viewport-admission).
See [indexed continuous tables](gnome-native.md#indexed-tables-in-continuous-text).
See [snapshot-owned continuations](gnome-native.md#snapshot-owned-table-continuations).
See [complete table selection](gnome-native.md#complete-indexed-table-selection).
See [text-flow measurements and limits](gnome-native.md#native-text-flow-experiment).
The internal `STUDIO_GNOME_CONTINUOUS_PREVIEW=1` switch now runs selectable
contexts through the real document actor and normal reader navigation. Portable
Rust preparation verifies snapshot identity; GTK retains the complete view and
expires same-revision page callbacks. A real-workflow fixture covers view/edit
transitions, Undo, column filtering and copied report tables. This is still gated
development work: dense mixed-content layout, input/accessibility,
cold-layout measurements and cross-context selection remain before promotion.
Native language/Copy headers now share the regular code viewer's controls while
keeping code in the common selectable buffer. Worker projection, quote/source
ranges, empty fences, actual keyboard Copy and stale page/buffer guards are covered.
See [native code controls](gnome-native.md#native-code-controls-in-selectable-preview).
Code-body surfaces now use worker-prepared light/dark base colors for both
highlighted and plain fallback source, plus GTK's below-text snapshot hook for
continuous backgrounds. Actual Wayland pixels cover first/empty code lines and
neighboring prose; Copy, native selection and stale-source ownership are retained.
Code-font and Markdown line-height preferences use the existing shared watchers.
The 11-case selectable-text and four-launch default application regressions pass,
alongside 274 native tests, shared parser checks, Rust 1.92 and release checking.
The wider promotion gates above remain open. See
[code-body surfaces](gnome-native.md#native-code-body-surfaces-in-selectable-preview).
The selectable adapter now has decorative native code gutters, using worker-side
Unicode line offsets without changing selectable text. Actual Wayland glyph
samples cover first/blank lines in light/dark/plain fallbacks, wrapped-line scroll
and replacement-buffer retirement. Exact Copy, source mappings and the real-report
workflow pass. See [code gutters](gnome-native.md#native-code-gutters-in-selectable-preview).
Deferred images and diagrams now share the 256-object projection budget without
consuming eager math/table preparation slots. The real actor fixture opens
64-image and 64-diagram documents, verifies viewport admission, bounded render
jobs, source cleanup and actual Home/End/Page Up navigation after deferred size
changes. GTK adjustment replacement and image-anchor width fixes are covered;
mixed math/table throughput and broader accessibility remain open. See
[dense deferred content](gnome-native.md#dense-deferred-content-in-selectable-preview).
The real reader now measures mixed inline formulas and eight indexed tables.
Context-local formula reuse cuts identical isolated renders from 16 to one while
retaining source/style identity, cancellation and existing budgets. The observed
render stage drops from 567 to 96 ms; that step left an initial GTK gap around
360 ms, addressed by the table admission work below. The default
renderer is unchanged. See
[mixed-content preparation](gnome-native.md#repeated-formulas-in-mixed-selectable-content).
The stronger mixed-table paint check also exposed offscreen labels consuming the
inline-math queue. Shared viewport/after-paint admission now renders visible table
formulas after scrolling without changing literal source or queue budgets.
That step left a measured GTK pause around 385 ms.
Selectable indexed tables now detach GTK row models outside their viewports and
restore page, column widths and scroll state on return. The reusable observer
uses the shared after-paint mechanism; idle binding and full backend Copy remain
stable. The same mixed fixture's opening gap drops from 386 to 71 ms, including
checks through first painted formulas. Backend row pages remain bounded and
cached. Broader density/input/accessibility gates still apply. See
[table viewport admission](gnome-native.md#indexed-table-model-viewport-admission).
Selectable preparation now counts distinct math source/style jobs, retaining
the 32-work, 256-object and per-occurrence raster bounds. A real 64-formula file
mounts all images with one render and 63 reuses; 33 distinct formulas produce a
persistent Preview notice with readable Source. Its original 478 ms first-opening
gap includes shell startup. Separate construction/layout counters and actual
Vulkan/OpenGL/Cairo runs now isolate a graphics-cache effect: copying only owned
GPU caches reduces the Vulkan document gap from 302 to 27 ms and OpenGL from
248 to 28 ms, with fresh application/font/document caches. GTK's default renderer
and persistent XDG cache policy remain in use; fresh-cache startup, broader
density/input/accessibility and physical-device acceptance remain open. See
[graphics startup measurements](gnome-native.md#separating-graphics-startup-from-document-layout) and
[work admission](gnome-native.md#repeated-formula-work-admission-and-preview-errors).
See [normal document requests](gnome-native.md#selectable-preview-through-normal-document-requests).
See [native chat comments](gnome-native.md#native-chat-comments) and
[reader selection coverage](gnome-native.md#native-reader-and-expanded-cell-comments).
The optimized reader-selection package also passes installed acceptance
`release-ui-6q32e1hd` with all diagram/language/backend/Git/viewer groups and no
application test hooks. Native tests, Rust 1.92 checking and the four-launch real
4.1 MB report regression pass. This checkpoint does not close the remaining
application parity and physical/provider acceptance gates.

EPUB and browser selections now use this same native library and stored provider
schemas. Pending DOM capture is scoped to the original session and frame epoch.
EPUB CFI encoding normalizes element boundaries and verifies the quote; source
reopening checks the book hash and range before highlighting. Unverified ranges
or changed books fall back to their section. Browser sources reopen with native
quote search. Four EPUB and two browser Wayland launches verify restart, Unicode,
session-switch rejection, source reopening and process cleanup with private
providers; wrong-range and changed-book cases never highlight another passage.

Native PDF controls, search-result page labels, link destinations/hover text,
status notices, password prompts and accessibility names now follow live
English/Chinese interface changes. Private Wayland checks retain selected text,
query/mode/document identity, literal URLs/snippets and typed password input;
fresh replies also use the new locale. The frozen context candidate is unchanged.

Native PDF reading now uses disposable PDFium processes through the Rust gateway,
with native page Preview/Text, contents, bounded literal search, fit/zoom, page
navigation, selected-text Copy and password entry. Files/chooser/launcher/chat
and Markdown links share the exclusive workspace pane. Checksummed PDFium and
its license notices are packaged with the native bundle; no browser or V8 is
used. Input, snapshots, raster cache, worker resources and requests are bounded,
and closing cancels an unfinished first render. Actual Wayland controls and a
real Mozilla reference PDF have been checked. Preview text selection now uses
bounded isolated PDFium hit-testing and native GTK highlight painting. PDF
Preview/Text comments share the native session library. Hash/quote verification,
restart, legacy region anchors, stale page rejection and changed-file behavior
pass the five-launch private Wayland fixture. API checks cover forward/reverse
selection on normal and rotated pages, and point/range selection on the real
Mozilla PDF. Physical pointer/touch acceptance, advanced PDF destination views, persistent annotation
objects, higher-resolution zoom and the document features listed below still
need work. See [PDF details](gnome-native.md#native-pdf).

PDFium now projects bounded page links from the existing isolated worker. Native
Preview hit testing and virtualized link rows support explicit/named destinations
within the same PDF and validated HTTP(S) URIs. URI activation uses asynchronous
GTK desktop launching; stale press/release pairs and pending launches are cleared
on page/file changes. Link clicks retain the selected Preview/Text mode, and
dragging across a link still uses native text selection. The API fixture covers
all four rotations, cropping, multiple quad regions, malformed/disabled actions
and link/region budgets. A private Wayland fixture checks Return-key activation
and a recording URI portal, including cancellation, without opening a browser.
Owned pointer events also verify actual GTK link clicks and text dragging; mode
switches reject a pending press. Existing PDF/selection/comment and full-app
regressions pass after integration.
Exact destination coordinates/zoom, cross-file PDF navigation, skewed-quad
precision and physical pointer/real desktop-handler acceptance remain gates;
PDF scripts and executable actions are never dispatched.

EPUB rendering evaluation: WebKitGTK 6 with the MIT-licensed `webkit6` 0.6.1
bindings is compatible with the existing GTK4 0.11 bindings. EPUB chapters are
XHTML/CSS; the planned boundary uses this engine for book content, with GTK
toolbars, file selection, contents, search and settings. Archive reads, package
metadata, resource validation and caches belong in Rust. CFI generation and
selection depend on the rendered DOM and should retain compatibility with the
existing EPUB reader's anchors. This follows the content-rendering approach of
[Foliate](https://github.com/johnfactotum/foliate); no Foliate application code is
being incorporated. The existing permissively licensed EPUB.js CFI implementation
is available for compatibility work.

An isolated Wayland prototype rendered Chinese/Arabic, styled tables and inline
formatting, and accepted a native GTK button action through an isolated script
world. Its private canary observed zero network requests. A response CSP blocked
book scripts, event handlers and remote image/CSS resources despite a permissive
book-supplied meta policy; the book's script world could not access the native
message handler. This checks the content boundary, not full EPUB compatibility.
EPUB is now integrated experimentally. Rust snapshot/resource delivery,
EPUB2/EPUB3 package/navigation parsing and shared revision-safe reading state pass
the private API fixture. The GTK reader has native contents, chapter/page
navigation, paper/layout/text-size controls and chapter search. Actual GTK
acceptance is incomplete: styled content and script isolation pass. Copy now
passes real pointer/keyboard input on an owned Wayland compositor, including an
independent compositor clipboard read. The earlier ownership failure used
synthetic actions with input serial zero; the existing controls/restart fixture
also passes after adding valid input. Copy rejects stale document epochs,
duplicate pending actions and selections over 2 MiB of UTF-8 without trimming.
The Copy checkpoint passes 120 native Rust tests (one ignored), the optimized
profile check and the four-launch application regression. No fresh optimized
executable or install bundle is included in this checkpoint.
Native paging/theme/font/contents, hidden-view anchors, rapid replacement and
restart controls pass, as does the complete four-launch regression. Real IDPF
GTK/font/image tests pass for Moby-Dick and the WOFF sample. Cold-start heartbeat
gaps, focused physical clipboard acceptance, external CFI links
and fixed/vertical layout parity remain. See [EPUB implementation and bounds](gnome-native.md#native-epub).

Full-book search now uses a dedicated bounded Rust lane and the existing
html5ever tokenizer, independently of chapter rendering. Native results group
literal/Unicode matches by section with counts and excerpts; they search unopened
sections, retain query/case identity, expose skipped sections and offer bounded
Previous/Continue result pages. Opening a result loads its chapter and uses the
existing rendered-section match navigation. Stop ignores late responses; replacing
a book closes its search and rejects stale generations. Private API/Wayland
coverage includes 140 sections, malformed text, literal metacharacters/trailing
spaces, case matching, paging and replacement. Real cached IDPF samples and a
24 MiB expanded-text fixture pass the API. CSS visibility can differ from extracted
text; search does not claim exact source/CFI occurrence anchors. These bounds and
the separate external-CFI/layout/physical-desktop gates remain explicit.

Native local-image viewing now decodes PNG/JPEG/WebP/GIF and rasterizes SVG on
bounded Rust gateway workers. GTK receives bounded pixels and provides fit, zoom
and clipboard controls. Markdown paragraphs, conversations, historical context
and Favorites share lazy native previews, with offscreen texture release and
source anchors preserved. Codex and OpenCode attachment history now uses flat lazy
native rows, worker-only source caching and the shared bounded Rust decoder.
Inline viewer navigation, cached restoration, text-delta preservation and source
metadata bounds are covered separately from composition. The viewer can now save
the original image through a native chooser, preserving exact PNG/JPEG/GIF/WebP/SVG
bytes instead of saving a resized preview. File access and SHA-256 identity are
rechecked; inline saves lease immutable source bytes. Rust workers verify the
digest before atomic replacement. Unconfirmed choices are cancelled on close or
replacement; admitted writes survive viewer closure and guard application exit.
The sixteen-request owned-Wayland fixture covers late replies, source changes,
cache eviction, exact outputs and disk failures with retained source/destination
context. Physical portals and remote destinations remain separate acceptance
work. At that earlier checkpoint animation and remote images were still absent; later
checkpoints add bounded animation and public HTTP(S) images. Color-management
parity and external/nested-vector SVG resources remain outside those increments.

Embedded PNG/JPEG/GIF/WebP resources inside SVG now share a GTK-independent Rust
resolver across gateway images and native diagrams/vector zoom. Bounded codec
decoding and streaming PNG normalization retain source identity and original Save;
duplicate normalization is reused while every occurrence charges the pixel/work
budget. Resource failures reject the SVG before painting. File/network hrefs and
nested SVG/SVGZ remain unresolved. Shared tests, gateway source-identity checks,
native zoom checks and actual API/GTK pixels cover this increment. See
[SVG bitmap coverage](gnome-native.md#embedded-bitmap-resources-in-svg).
This increment passes all 534 unit tests, Rust 1.92 checking, the real-report
four-launch workflow and actual image/table GTK fixtures. The matched optimized
package passes installed `release-ui-jamaitlm`, including original SVG Save from
native controls and all five existing workflow groups. The optimized API also
checks embedded pixels and rejects private absolute paths/network hrefs and
oversized/excess resource sets. Full renderer/image parity and the broader
physical/provider/fresh-machine acceptance gates remain open.

Markdown paragraph `data:` images now use shared Rust standards parsing and bounded
immutable bytes in both reader and chat, with native viewer/original Save. History
attachments reuse the same decoder and existing opaque cache, including SVG.
Canonical base64 keeps the existing block decoder after a measured comparison;
other forms use the standards streaming decoder. Compact formatting-window limits still apply to Markdown; oversized inline image
URLs now use lazy snapshot-bound references with original source coordinates. Private Wayland/portal tests verify PNG/percent-encoded
SVG, exact originals, stale-view cleanup and zero remote fetches/web engines.
The 10 MiB history regression passes on both providers.
See [embedded image bounds](gnome-native.md#embedded-image-urls).
Large data-image source spans now pass index budgeting without weakening actual
HTML/event limits. A native Rust projection keeps full Source/Edit snapshots and
validated original byte/line mappings while returning compact fragment text. Lazy
image requests retain reader/document/revision identity; viewer opening freezes
verified original bytes so Save survives reader closure. The private 13,981,772-byte
fixture returns a 536-byte fragment, verifies caption/quote line 3 and saves the
exact 10 MiB PNG after closing the reader. Ordinary fragments use a SIMD-capable
candidate scan and skip the extra Markdown parse. Vendoring reproduces both
modified MarkNoria files. See [large embedded image sources](gnome-native.md#large-embedded-image-sources).
The refreshed optimized pair passes installed `release-ui-0t_umnun` with all
existing diagram/language/backend/Git/viewer groups and no application hooks;
its image/SVG API checks also pass. The native/gateway/parser suites pass
203/226/52 tests, Rust 1.92 checking passes, and the real 4.1 MB four-launch
regression remains passing. Full migration and physical/provider gates stay open.

- Reuse MarkNoria's Rust index, source ranges, table indexes, and bounded editing.
  Expose presentation-neutral data where current APIs return only HTML.
- Virtualized Preview, exact Source, GtkSourceView paged Edit, headings, search,
  source/preview mapping, comments, reload, dirty guards, undo/redo, save conflicts.
- Paginated native tables: column selection, resizing/wrapping, stable paging,
  left alignment, accurate source anchors, and preserved context.
- Complete Mermaid/PlantUML/D2 support through bounded native/worker rendering to
  SVG/textures, validated against existing syntax fixtures. No screenshot-based
  rendering or unannounced Chromium dependency.
  The shared native inspector now provides Fit, actual size, bounded zoom,
  scrolling and Copy for reader/conversation diagrams and display math. It
  shares a rendered texture and survives source-row recycling. Bounded vector
  rerasterization now reuses retained SVG on two Rust workers, with cancellation,
  stable logical layout, display-scale-aware resolution and an 8-megapixel frame
  budget. The SVG is included in the shared cache weight. Tiled zoom and the
  recorded renderer/style/physical-desktop parity gaps remain open.
  The measured debug D2 bottleneck was in unoptimized raster kernels; targeted
  development-package optimization preserves exact pixels and lowers the observed
  400% texture-ready time from 2,875 to 281 ms. Format-specific accessible names
  support independent inspection in the all-format installed-release fixture.
- Native text/image/CSV/XLSX/PDF/EPUB viewing, reading state, selection/comments,
  SQL querying, charts, copy/export, and read-only comparison.
- Benchmark private copies of the real acceptance reports under
  `/home/rui/downloads/openresource/anything/auto-req-dsl/models/dcdc_unified/validation/latest/`
  and the >30 MiB wide-table fixture. Measure open/outline/navigation latency,
  responsiveness, memory, line correctness, transport, and cleanup.

### 3. Native workspace tools

Image animation checkpoint: bounded Rust GIF/APNG/WebP decoding and worker-side
timeline validation feed a shared native GTK player in viewer, Markdown and
conversation images. Fixed-size paintables avoid per-frame layout work;
viewport/unmap suspension, reduced motion, explicit Play/Pause/Replay, finite
loops, current-frame Copy, original Save and readable limited-animation stills
have private Wayland coverage. Physical compositor occlusion/minimization and
live-provider acceptance remain open; installed packages are unchanged. See
[image animation](gnome-native.md#bounded-image-animation).

Implemented checkpoint: native Files lazy expandable tree, paged directory reads,
hidden/ignore controls and live filesystem updates; reusable
bounded source/diff windows; native VTE connected to the shared gateway PTY;
Git status/stage/unstage/history/commit-file/diff controls; exclusive workspace
panels; persistent global window controls. Wayland tests pass for paste/ANSI/
Chinese, shell reaping, private-repository mutations/history, and source anchors.
Code/text Source selections now use the shared comment store and file identities.
Saved non-Markdown comments reopen in Source with worker-side literal verification
and byte-offset navigation for long lines; changed files relocate only a unique
quote. Two private Wayland launches verify explicit save, exact CRLF/Unicode,
restart, source lines, unchanged stored anchors and capture exclusions. This does
not complete advanced diff comments or physical
acceptance. See [Source file comments](gnome-native.md#source-comments-for-code-and-text-files).
Generic Source now also has literal full-snapshot search on bounded Rust workers,
exact byte/line navigation across pages and long lines, Unicode case controls,
native page highlighting and independent file/query generations. Native keyboard
controls and translated search labels preserve the exact query. Coverage and
remaining acceptance limits are recorded under
[Source find](gnome-native.md#native-source-find).
Identified writable code/text files now also have native Source/Edit, bounded
page edits, explicit/page-change saves, undo/redo across saves, dirty file/window
guards and conflict cancel/overwrite. A GTK-independent Rust edit composer
preserves unchanged Unicode/CRLF bytes and uses the existing atomic save API.
The shared route's body budget now matches its 5 MiB file contract, and a bounded
worker gate keeps file I/O off async HTTP lanes. A private 4.2 MB file and an
actual source copy verify these flows, shared read-only policy, and live labels.
Editing Find now searches the complete snapshot plus the unsaved page on bounded
Rust workers. It maps LF-normalized results to GTK scalar selections and original
file bytes without a per-character mapping table. Within-page navigation retains
undo; dirty cross-page navigation uses Save/Discard/Cancel. Query/mutation/file
generations, view-specific corpora and live labels are tested with a private
4.2 MB file and real-source copy. Cross-page editing operations and expanded
physical/input acceptance remain open. See [code/text editing](gnome-native.md#native-code-and-text-editing)
and [editable Find](gnome-native.md#find-in-editable-code-and-text).
Native review now also has worker-side file/rename and loaded-commit filters,
old/new diff gutters and working-tree Open File. The bounded shared Rust line
index also serves conversation diffs; GTK keeps the existing source windows.
Stable model reconciliation retains unchanged rows, and history merging stays
off the UI lane. Hidden selections and stale replies cannot retain actionable
old content. A real 103-commit private Wayland fixture verifies paging during
commit switches, model identities, stage/unstage and Markdown/source handoff back
to Review. The close lifecycle restores the parent before GtkStack hides its
child, retaining comparison's post-close behavior. Git controls have fixed
accessible names and row activation; worker/model values remain literal during
translation. Shared bounded pane navigation now extends parent return to image,
PDF, EPUB, table and Browser workspaces. Accepted open intents present the viewer;
deferred EPUB saves and internal refreshes cannot steal focus. Stale closes leave
the active pane intact; closed ancestors are skipped and cycles collapse. Existing
Markdown save guards and comparison close hooks remain in place. A dedicated
private Wayland fixture covers these transitions and draft retention; ordinary
installed controls have a separate `--viewers` variant. Advanced review/comments
and physical acceptance remain open. See [native Git Review](gnome-native.md#native-git-review)
and [viewer navigation](gnome-native.md#native-viewer-navigation).
Source files retain physical line numbers independently of syntax; only actual
review/turn diffs use origins. The real-report regression also rejects an inactive
Git panel's late close after a new document opens. The optimized installed
`--all-diagrams --language --backends --git` case passes actual keyboard/AT-SPI
selection, bilingual accessible search names, file handoff/return and explicit
staging alongside the existing workflows, without application hooks. Current
coverage is 35 shared model, 175 native and 210 gateway tests; Rust 1.92 checking
and both optimized builds pass. Full migration/manual/platform gates stay open.
Shared-directory browsing now retains the selected project context, exposes
read-only badges, and opens through the native reader with Edit disabled.
Shared scans, validated locations and live watches recheck configured roots;
revocation closes the subscription before exposing later activity. Root
deduplication and absolute shared-tree identities prevent collisions with project
rows. Native context/header file-manager actions use GtkFileLauncher; Copy path
uses the validated canonical location. The private `--shared --headless` fixture
uses actual keyboard/right-click context menus and independently reads their
copied canonical paths through its owned Mutter compositor. Native menu cleanup
now preserves the inherited action context until GTK finishes activation; the
test no longer bypasses menu activation or manually allocates its popup. It also
verifies the read-only reader and launcher configuration. It deliberately does
not invoke the external file manager.
Native Settings now configures shared directories with explicit Save/Reload and
live Files root refresh; scoped revision checks preserve unrelated settings.
Main-menu Open file… (Ctrl+O) now opens local documents or arbitrary UTF-8 code/text
through a format filter and All files option. It and Reader Open share cancellable
request ownership, reject superseded results,
and cancel on explicit replacement, unmap/close and destruction. A same-turn
cancel/reopen check verifies request identity. Private portal acceptance covers
eighteen actual GTK/D-Bus calls, seven cancelled requests, two deliberately late
successful replies, dirty source Cancel/Discard, filename-based syntax detection,
Unicode/trailing-space folder selection, explicit shared-folder Save and real file/directory descriptor
handoff. Writable project directories use readable transport descriptors because
GTK's writable-launch flag attempts an unsupported read/write directory open.
The workspace classification stays independent of the transport flag. See
[file chooser and portal handoff](gnome-native.md#native-file-chooser-and-portal-handoff).
Remaining work includes physical clipboard/file-manager/folder-chooser portal acceptance,
remaining chooser/Save/export lifecycle coverage, advanced review formats/comments
and broader terminal interaction coverage.
Files scans/filtering/ignore matching now run on bounded Rust workers using
the `ignore` library; GTK uses TreeListModel/TreeExpander. Two-request admission,
finite busy-read retries, revision-checked 200-entry pages, generation isolation,
coalesced expanded-directory watches and retained expansion cover the normal
workflow. The private Wayland fixture checks actual live create/rename/delete
events, independent hidden/ignore controls, literal whitespace and paged folders.
See [Files implementation and bounds](gnome-native.md#native-files).

Read-only Markdown comparison now uses two independent native readers, immutable
indexed snapshots and the shared Rust diff engine. Preview/Source can differ on
each side; change navigation is available when both use Source. Wayland coverage
includes opening one side first, different directories, mode switching and source
change highlighting. Advanced aligned review and non-Markdown formats remain.

- Lazy Files tree/search, watches, ignore rules, shared-directory read-only access,
  file-manager handoff, comments, and current project selection.
- GTK4 VTE Terminal using existing environment/network policy and process cleanup;
  preserve shell behavior, resizing, clipboard, and focus.
- Git Review: staged/unstaged filters, history, changed files, line-anchored
  text/Markdown/CSV diffs, stage/unstage, and no destructive discard action.
- Exclusive right workspace panels and a responsive divider that avoids costly
  document relayout on every pointer event.

### 4. Backend and session parity

Implemented OpenCode checkpoint: authenticated HTTP/SSE adapter, catalog/history,
provider/model listing, text prompts using server-owned identity, explicit Stop,
permission/question dialogs, and generation isolation. Wayland checks include a
response completing before HTTP acknowledgement and an assistant completing
while the server still reports busy. No upstream idle-status workaround is added.
Remaining backend coverage includes advanced commands/attachments, oversized
outputs, real-provider loading acceptance, event replay, lifecycle recovery and
the session features below.
Native backend transports now have bounded automatic recovery, including failed
initial handshakes before the first ready notification:
independent generation-owned timers for selected/background Codex and OpenCode,
exponential backoff, an eight-attempt flapping limit and a healthy-period reset.
Selected recovery revalidates history; background recovery preserves the current
view. Queues remain paused, uncertain drafts stay intact, and requests are never
replayed. Nonterminal queue/reply errors no longer invalidate a healthy
connection. Explicit new connections register their policy budget before
dispatch; automatic retries preserve its attempt count. Text typed before a
first successful connection survives retry even without a selected conversation.
The private Wayland fixture rejects two initial handshakes for each protocol
and then verifies recovery. Invalid settings/credentials still need correction;
gateway process exit after startup, gateway-wide recovery and
missed-event/pending-request replay remain separate work. See
[backend transport recovery](gnome-native.md#native-backend-transport-recovery).
Registry discovery now runs independently of the command/document loop, with
bounded read-only HTTP retries and generation-owned startup results. Before
discovery completes, the existing Reconnect action retries it explicitly;
credential failures do not retry automatically. Real private Wayland checks cover
temporary failures, permanent failure with independent document reading, stale
callbacks, retained drafts, selected history and closing during backoff. Native
tests, Rust 1.92/release checks, the four-launch report workflow and existing
both-provider handshake recovery pass. Gateway launch now also retains child/
parent-pipe ownership during readiness, supports explicit Reconnect after failure
or during a pending launch, and closes through an independent coalescing control
channel even with the request queue full. Owned Wayland child traces verify EOF
before replacement, no launch after close, retained drafts, rendered existing
history and stale callback isolation. Recovery after an already-running gateway
exits remains separate. See [launch recovery](gnome-native.md#native-gateway-launch-recovery)
and [startup discovery](gnome-native.md#native-startup-discovery-recovery).
Running gateway exits now retire every API clone, reject queued operations with
their original reply scopes and pause live backend activity. Dirty Markdown
buffers reject late Save acknowledgements; admitted local exports retain their
receipt/close guards until completion. Actual owned-gateway exit acceptance checks
the durable paused queue, draft, further typing, late terminal readiness and
programmatic navigation without provider prompts. Explicit Reconnect now reaps
the departed gateway and settles admitted receipts before replacing its services;
ordinary requests queued during loss or replacement launch receive scoped errors.
The persistent document actor retains bounded local edit token identities.
Revalidation checks the full-file hash and exact edit window before rebinding
new document IDs; GTK keeps its buffer, caret and undo stack. Save is enabled only
after verification and still requires the user's explicit action. Conflicts keep
the buffer editable with a persistent explanation and Save disabled. Read-only
Markdown uses the same snapshot check. Table/PDF/EPUB leases now reopen on Rust
workers after loss, verifying the original canonical path and source fingerprint.
Tables retain the last executed SELECT independently of the editor draft, page,
column widths and Wrap. PDF recovery preserves page, zoom, mode, search and sidebar
state. EPUB recovery retains its CFI and reading preferences. Changed files retain
the old display with server operations disabled. Actual private Wayland acceptance
covers unchanged and changed files with all three viewers open simultaneously.
GTK now constrains divider travel to measured pane minima, avoiding clipping with
large typography and queued messages; twenty allocation cases cover both panes.
Open EPUB search dialogs now retain their drafts/results across gateway loss,
rebind search and navigation only to a verified book snapshot, and offer the
native Reconnect action inside the modal. Real Wayland checks cover changed
books, explicit queries through the retained dialog, exact literal drafts, new
document IDs, immediate reopen and retired callback rejection. The ordinary
140-section API/dialog regression also passes. See
[EPUB search recovery](gnome-native.md#epub-search-during-gateway-replacement).
Paged/in-flight book-search recovery now has four actual owned-Wayland cases
with a 140-section fixture. A real held response verifies stale-generation
rejection. Continue finishes a partial result page instead of creating an
overlapping page; exact section identities, Previous/Next, query drafts, scan
cursors, new document IDs and changed-book controls are covered. See
[paged book-search recovery](gnome-native.md#paged-and-in-flight-book-search-recovery).
Encrypted PDF recovery now uses an owned native AlertDialog. Wrong-password,
Cancel/retry, duplicate attempts, changed files, entry clearing and stale prompt
responses are covered on real Wayland; replacing a PDF retires its active prompt.
Page/zoom/scroll/search state survives a verified unlock, and no password is
replayed or stored as a recovery preference. See
[PDF unlock recovery](gnome-native.md#native-pdf-unlock-recovery).
Multi-sheet XLSX recovery now covers distinct worksheet queries, independent SQL
drafts, exact result rows/page, new leases and retained column settings. An offline
chooser callback can no longer clear the retained page. Actual Wayland cases also
combine workbook recovery with encrypted PDF unlock and an in-flight EPUB search,
including changed-file protection and query traces that reject draft execution.
See [workbook recovery](gnome-native.md#multi-sheet-workbook-recovery).
Supplementary EPUB gateway recovery now covers a non-spine resource, successive
fragment links, actual restored passage geometry, retained main-chapter CFI and
changed-file protection. Same-section links now update their retained fragment.
Wayland cases pass alone and with XLSX, encrypted PDF and in-flight book search;
ordinary EPUB controls and restart restoration pass too. Arbitrary supplement
scroll and relaunch restoration are not covered by this transient link recovery.
See [supplement navigation](gnome-native.md#supplementary-epub-navigation-recovery).
Broader migration acceptance remains in scope. See
[gateway loss boundary](gnome-native.md#native-running-gateway-loss-boundary).
The current native/gateway pair has also been rebuilt in release mode and
staged separately as `target/gnome-native-candidate-20261102`. Its installed
workflow passes all five optional groups without application hooks, alongside
244 native tests, Rust 1.92 and the four-launch real-report regression. The
previous installed package is unchanged; this is a reviewable checkpoint of the
full migration, not final physical/provider acceptance.
Codex process exits now also retire the native connection when the gateway
WebSocket remains open. A worker-side process-generation tracker discards older
lifecycle notices before presentation, and the existing bounded recovery policy
restores verified history without replaying requests. The private Wayland fixture
exits actual owned Codex children in selected/background cases and checks
replacement PIDs, retained drafts/queues, literal native error toasts and cleanup.
See [Codex process recovery](gnome-native.md#native-codex-process-recovery).
The updated optimized frontend also passes actual installed process recovery
without application hooks, alone and with all five broader workflow groups.
Its candidate launch link is the same `target/gnome-native-candidate-20261102`;
the preceding candidate and installed package are retained unchanged. The final
checkpoint has 247 native tests passing (two ignored), Rust 1.92/release checks
and the four-launch real-report regression; full migration gates remain open.
Explicit Codex stream-gap notices now invalidate cached state and use bounded
reconnection/history verification, with queues paused and no pending request
replay. Real gateway broadcast overruns are exercised while selected and in the
background, retaining the provider process and the visible offscreen state.
This closes reported-gap revalidation parity; continuous event replay and
unresolved request-dialog recovery remain separate. See
[missed-event recovery](gnome-native.md#native-missed-event-recovery).
This checkpoint passes 248 native tests, Rust 1.92/release checks, actual selected
and background overruns, process recovery and the four-launch real-report
regression. The rebuilt optimized pair passes event-gap recovery through its
installed controls, alone and with all five broader groups plus process recovery.
The previous candidate fails the same missed-notice check as an expected negative
control. The existing candidate launch link selects the new tested pair; earlier
candidates and the installed pair remain unchanged. This does not complete the
full migration or the broader acceptance gates.
Pin ordering, rename/fork, Codex archive/restore, confirmed deletion and archived
read-only views now have native controls. Wayland tests exercise the actions
against private sessions and check stale catalog isolation. Active/Attention,
expanded lifecycle recovery, hidden-directory filtering and richer search remain.
Separate native backend connections now remain subscribed after view switches.
Wayland tests route an offscreen Codex approval while viewing OpenCode and receive
an offscreen OpenCode completion while viewing Codex. Active/Attention filters use
bounded lifecycle state; initial idle loading does not create unread work. Full
background transcript caching, reconnect replay and request-dialog recovery remain.
Bounded transcript snapshots now restore immediately while history refreshes.
Shared immutable rows avoid large message copies into GTK; the LRU budget is
16 views/64 MiB. Backend/session-scoped late replies preserve the current view
and clear acknowledged prompts from the correct in-memory draft. Continuous
background transcript replay and complete reconnect/request-dialog recovery
remain beyond this checkpoint.

Conversation Markdown now uses worker-produced blocks and a flat GTK list,
including native paged tables, column controls and reusable highlighted code
widgets. Native chat diagrams reuse the bounded render lane, invalidate on
session changes and release textures when unmapped. Actual Wayland checks cover
these widgets. Bounded rich live previews are now implemented below. Remaining
history/output workflows, remaining image
formats and
the remaining session/composer tools below still need native implementations.
Inline/mixed math and Mermaid labels now have the bounded native implementations
documented above; this does not complete the broader history/output/tool work.

Conversation find now uses Codex occurrence search and Unicode-aware literal Rust search with
case/role controls, native previous/next navigation and exact table-row scrolling.
Turn/message anchors survive worker projection, including OpenCode parent-user
IDs. Query/backend/session scoping rejects stale results and pending retrieval
is cancelled when superseded or closed. Older Codex targets are paged into a
separate read-only context window without changing the live conversation. Legacy
turn-read compatibility is tested; Codex servers without occurrence support
explicitly use loaded-window search.
OpenCode now follows cursor-paged reads for search and historical context, while
releasing raw pages between requests. Literal Unicode/trailing-space matches,
overlapping pages, opaque cursor encoding, legacy partial coverage and repeated
cursors are tested. Search returns at most 100 messages with bounded cursor/source
metadata, and explicitly labels incomplete coverage.

Native conversation history now has an inline Older/Newer/Live window. Codex
prefers the installed protocol's item-level pages, with turn-page and legacy
read compatibility; OpenCode uses cursor-based message pages. Oversized turn/
message-part lists are sliced with source anchors. Rust workers prepare bounded
immutable windows, and GTK reuses native transcript controls. Live deliveries do
not replace or scroll an older page; drafts and delivery state stay independent.
Manual ordinary Send returns to Live. Private Wayland cases cover both protocol
modes, 650 turn items, 250 message parts, source/Markdown/search, errors/retry,
cancellation, backend changes, live updates and an explicit fake Send. Page,
transport and navigation metadata remain bounded and limits are visible.
Controlled Mutter runs additionally require an actual mapped history body before
recording opening latency, with an owned virtual monitor and private desktop
services. This retains the visibility check when an unattended desktop window
does not receive layout frames. API-only security checks are included in the CI
configuration; physical
desktop/IME/accessibility/portal acceptance remains separate.
Initial Codex hydration now resumes with metadata only, then reads 120 recent
items with the matching resume cursor and a separate four-turn summary for
active-turn/shell identity. JSON and rich content preparation stay on bounded
Rust workers. Atomic immutable-row reconciliation retains observed live text and
terminal events during loading, including existing cached views and backend
reuse. Unverified reads block Send, selected-session changes, shell/review
commands and queued dispatch; errors retain drafts and require refresh.
Superseded reads are cancelled, socket/view
generations reject late results, and unsupported servers retain explicit
turn-page/full-read compatibility. Private Wayland fixtures exercise a
20,000-item provider history and the installed Codex protocol is checked against
a local fake Responses stream, without real inference or production credentials.
This does not guarantee smooth startup or recover unpersisted text from before
Studio subscribed. A separate native output viewer now retains received source
up to 16 MiB in a bounded Rust memory cache and reads it through UTF-8/CRLF-safe
text pages, global source lines, literal full-source find and explicit atomic
export. An active source lease survives cache churn; expired unopened snapshots
require reload. User/command/tool/reasoning/diff/assistant, history and saved
favorite previews share it. Controlled Wayland input exercises real source
buttons and exact exports with the real report, including full OpenCode user
history. The output Save dialog now coalesces duplicate requests, cancels on
viewer close/replacement and rejects late replies. Confirmed worker exports
survive viewer closure, while the main window waits for completion before
closing. A private Wayland SaveFile fixture checks these distinct lifetimes,
exact original bytes and unchanged cancelled destinations. Other export tools
and physical portal acceptance remain. Favorites now has the same pending-choice
cancellation and duplicate protection, retains dirty metadata during cancellation
and exports persisted library data without implicitly saving the draft. Its
Wayland checks include main-window close, immediate reopen and explicit Discard.
Reserving the close-confirmation guard before scheduling also prevents a second
save prompt from appearing after Discard has closed the library. See
[output export chooser lifetime](gnome-native.md#output-export-chooser-lifetime).
Output/Favorites failures now also enter a shared window-owned native dialog
queue, retaining the original destination after viewer close/replacement and
ignoring duplicate completions. Confirmed local Favorites writes keep their
serial/close/concurrency guards across a gateway-loss callback until their own
completion. Six private SaveFile requests exercise five real disk failures,
two queued failures, current-page/selection retention, component-level connection
fault injection, dirty metadata, a successful admitted write, temporary cleanup
and unchanged persisted data. Screenshots use actual mapped frames without
forcing GTK layout; other export/physical/provider gates remain. See
[background export failures](gnome-native.md#background-export-failures).
Conversation search and Router source jumps now cancel obsolete automatic
scrolls through a shared GTK coordinator. Reading intent survives deferred row
allocation and live updates, and returning to the end resumes following. A
deterministic private Wayland case reproduced an earlier-result search jump being
overridden by a queued follow-tail callback; the extended real-report check now
covers that ordering and two batches of new provider messages. The original
intermittent header timeout was not independently traced to this cause; physical
and longer-session navigation acceptance remains. See
[conversation navigation priority](gnome-native.md#conversation-navigation-priority).
Large plain transcript blocks now use a bounded scrollable text widget,
removing a measured multi-second wrapping-label layout stall. Rich full-output
preview, live previews above the excerpt bound,
outputs above the snapshot/transport bounds, deeper navigation, broader
lifecycle/reconnect recovery and large real-provider history acceptance remain
work; complete history parity is not yet claimed. See
[native conversation history](gnome-native.md#native-conversation-history).

Ordinary assistant deltas now receive coalesced rich previews on one dedicated
Rust CPU lane, at most one admitted request per 150 ms. Whole-snapshot parsing
shares unchanged immutable blocks; GTK updates only changed gaps. Paragraph,
table and diagram widgets retain state and current-message actions during live
updates and completion. Context/generation and source-prefix checks reject late
results while retaining newer source bytes and metadata. Completed turns cannot
be starved by a subsequent live reply. Private provider/Wayland checks cover rich
text and diagrams before completion, two delta bursts, reading/table state,
current actions, final text and a queued result after a real session switch.
Over-budget/live full-output rendering and broader provider/
desktop acceptance remain. See [native rich streaming](gnome-native.md#native-rich-streaming).

Long rich paragraphs/headings now share worker-prepared 8-KiB styled pages and
bounded GTK text views across conversations and readers. Native Unicode ranges,
safe links, source-linked selection and sparse per-page attribute indexes avoid
whole-paragraph main-thread markup parsing/layout. Ordinary/unbroken Wayland
checks verify page navigation, exact quotes and source lines, with a lower
measured memory peak than the whole-paragraph text-view experiment. Equal-refresh
reading state and search-page selection pass, as do the full application,
Markdown comment restart and complete-output export regressions. The shared model
now retains parser source spans, so repeated source occurrences resolve to their
exact page/selection. Verified Unicode/code normalization and source-only notices
prevent hidden syntax/URL hits from selecting another equal quote. Split-page
matches show a continuation notice. See
[native long rich paragraphs](gnome-native.md#native-long-rich-paragraphs).

Formatting-budget failures now use worker-prepared literal source pages across
the reader, conversation history and live/saved-message presentations. Native
read-only buffers remain at most 8 KiB per page, with original physical lines,
exact Unicode/trailing-space search projection and inert HTML/URLs. Header/body
continuations keep separate coordinates; invalid/oversized source windows still
fail. A bounded normalization map never substitutes a fuzzy search occurrence.
The private Wayland fixture verifies actual chat/reader widgets, page ends,
Source/Preview switching, unchanged file bytes and unsent message retention.
Shared parser tests cover node-limit recovery, malformed ranges and dense NUL
normalization. This does not complete rich rendering beyond snapshot/transport
bounds or physical/provider/release acceptance. See
[native formatting-limit source pages](gnome-native.md#native-formatting-limit-source-pages).

Within-page reading positions now survive equal conversation row replacement,
including changed wrapping width, and same-revision reader Source/Preview round
trips. Shared GTK text-position memory retains the visible character, signed
line inset, caret and selection. One bounded, cancelable frame callback restores validated text marks;
obsolete widgets cannot change a newer view. Explicit page/source-line/search
jumps reset the destination, and a changed document revision invalidates the
cache. Ordinary/unbroken Unicode fixtures check actual glyph/viewport positions,
top margins, outer-scroll stability, stale callbacks and existing exact search,
selection, comments and relative links. Existing-widget reflow without replacement
and physical desktop input remain separate gates.

Long Markdown table cells now use worker-prepared 256-byte/five-line excerpts in
both reader and chat, with explicit native full-cell reading, selection, Copy and
safe links. Full styled text reuses bounded 8-KiB pages and shared immutable cell
data; source lines and original Markdown remain intact. Closing/recycling the
source row closes its reading window, and repeated opens reject destroyed GTK
windows. Column labels are bounded too. The private actual-widget fixture covers
Unicode/style preservation, table/text paging, visible tail selection, reopening,
row ownership and relative links. Full-cell and column-header Find now reuses the
bounded Rust literal-search engine and original page character ranges. Native
controls navigate short, long styled and formula pages, with case/space fidelity,
visible results, live language bindings and stale/closed query rejection.
Full-cell source comments are covered by the reader/chat comment workflow above.
Further cold-layout performance and physical input/accessibility acceptance remain
in scope. See [cell Find](gnome-native.md#native-full-cell-find) and
[long table cells](gnome-native.md#native-long-markdown-table-cells).

Native Favorites now shares Studio's existing database/API. Native message and
selected-text actions, metadata editing, search, Preview/Source, copy, confirmed
removal, file-chooser Markdown export and dirty guards pass Wayland checks.
Codex source anchors open the read-only historical context without affecting the
live conversation; OpenCode sources now also load older pages, including from an
unselected session/backend. User message IDs and joined text parts agree across
history, SSE and source lookup. Broader selection/navigation parity and
streaming exports beyond the native 32 MiB response bound remain.
Recreated conversation/favorites diagram owners now use distinct generations;
generation-specific cleanup and late-result rejection preserve their isolation.

Native queued follow-ups now share the existing SQLite queue and depth preference.
Enqueue/edit/remove wait for durable writes, preserve references, and retain newer
composer text and failed edits. Explicit pause/resume, Stop, disconnect and restart
prevent unattended retries. Dispatch stays with the original backend/session while
viewing another conversation and reads ordered model/effort/Fast options. Native
Wayland checks cover Codex and OpenCode, including completion before acknowledgement,
authoritative OpenCode idle, failed database writes, and paused restart recovery.
Image attachment composition now uses a native file chooser, clipboard and file
drop target. Rust workers read regular files, sniff supported signatures and encode
inline inputs. Per-conversation attachment state and bounded in-flight accounting
preserve newer images across send acknowledgements. OpenCode converts images to
file parts and resolves image-only prompts against server-owned identity. Image
messages retain the existing UI's explicit queue restriction.
Native `@` file and `$` skill/plugin completion now uses bounded worker-normalized
catalogs and a GTK popover. Unicode scalar offsets, quoted paths, backend fuzzy
ranking, stable keyboard selection, query generations and filter tickets protect
the current draft. Per-session reference chips remain in memory; Send and durable
Queue acknowledgements clear only their captured inputs. OpenCode file parts are
confined to the original project, and one selected skill uses its command endpoint
with a separate finite five-minute deadline. Explicit completed command replies
settle activity/queues and project bounded rich rows even without SSE; failures
pause queued work. Wayland checks cover both backends and retained restart queues.
The native command palette (Ctrl+Shift+P) and inline slash completion now cover
model/Fast, permissions, status/usage, compact/review, accumulated turn diffs,
skill discovery and existing session operations. Leading `!` executes through
the backend's shell API. Codex shell acknowledgements do not imply completion;
event/history source identity, reconnect generations and terminal turn evidence
control its busy state. OpenCode requires an explicit provider/model and uses
its own shell/command APIs without changing the documented normal-message idle
limitation. Future-message Codex policies are ephemeral, scoped to the original
session/project, confirmed for full access and applied again at queue dispatch.
Scoped Rust workers prepare diff windows and usage; the metadata cache has both
entry and memory limits. MCP status now uses a native virtualized inventory with
worker-projected counts/authentication/connection state, bounded opaque-cursor
pagination and stale-dialog/session guards. Remaining reference/image variants,
remaining command variants and expanded focus/IME/accessibility coverage remain
in scope. This checkpoint does not complete the application migration.

Native subagent navigation now resolves bounded summary reads, pages the agent
catalog and validates parent relationships on Rust workers before displaying a
virtualized tree. The installed CLI 0.163.0 schema has no `ancestorThreadId` list
parameter; native discovery uses supported source-kind filters and rejects
unrelated candidates by ancestry. Bounds, cursor cycles and malformed lineage
are explicit. Wayland checks cover nested navigation, per-agent draft retention,
catalog refresh, stale/replaced dialogs and two additional launches proving
selected-agent restoration and lazy backend startup. Normal session catalogs now
share the same worker-side bounded projection instead of decoding rows in GTK.
Native Router configuration now has a GTK settings editor and scoped Rust API.
It discovers cross-backend fallback targets on workers, saves ordered conditions
and per-session responsibilities, preserves unknown/legacy fields and original
controller IDs, rejects stale/concurrent updates, and guards unsaved/newer edits.
The native editor switches only between configured controllers. Both backend
types now have explicit setup/verification: a durable SQLite intent and ID
journal prevents duplicate creation after disconnection, timeout or restart;
identity/workspace checks reject replacements, registration preserves concurrent
settings, and a failed rename cannot lose the created ID. Accepted gateway work
survives a client disconnect and is bounded to two concurrent setup jobs. The
isolated lifecycle fixture exercises creation and recovery without model prompts.
OpenCode setup uses directory-scoped authenticated metadata requests, denies tools
and confirms the final ordered permission rule, including the real 1.18.31 server's
append behavior. A failed permission update retains the known ID for recovery;
metadata is capped at 1 MiB. Real-server checks cover create/restrict/restart in a
new isolated profile. Wayland checks cover both controllers, form retention during
verification, real alert-button actions,
fallbacks, draft retention and scope changes. This is a configuration checkpoint,
not completed Router runtime support.

Native OpenCode turns now carry worker-validated instructions and output schemas;
structured JSON keeps stable message/turn anchors in live, historical and paged
views. Transport checks cover server-owned prompt identity and JSON byte budgets;
structured output never changes the authoritative busy/idle rules.

The Rust gateway now has a text-turn Router execution API. It snapshots current
configuration, projects bounded cross-backend catalogs, applies shared directory
hide/ignore rules, validates structured decisions, and dispatches to one target.
Explicit targets skip the controller model; clarification and invalid decisions
never dispatch. Target model/effort/Fast and scoped future-message permissions are
independent of controller options. Codex uses its project environment profile;
OpenCode keeps its inherited server environment, matching the native settings UI.
Environment ownership is shared with the existing apply API, bounded to 4,096
session scopes and isolated by backend process generation. Clearing a previously
applied profile explicitly clears its overrides before routed dispatch, including
profiles applied by another Studio client. The registry stores no secret values.
Unnamed Codex sessions use the sidebar's name/title/preview fallback.
Unavailable catalogs are reported and included in controller context rather than
blocking otherwise available backends. The Rust regex set compiles directory
rules once per projection, preserving ordered negation and Windows path behavior.

Durable SQLite intents/claims bound execution to four tracked requests, prevent
concurrent target deliveries, and mirror delivery identities into shared Router
history. Controller ownership is released after target acknowledgement so other
targets can run concurrently. Request UUIDs and canonical SHA-256 fingerprints
deduplicate retries. Restart recovery monitors only confirmed target identities;
unconfirmed controller/target sends become explicit failures without replay.
An inactive backend turn without a final history marker is reported as an
unconfirmed result after restart rather than remaining active indefinitely.
Readiness waits retry reads only. Stops address the owned turn; OpenCode refuses
to abort when its latest user message belongs to another turn. Real model prompts
have not been used to validate this execution API; isolated deterministic backends
exercise all four backend pairs, early completion/late acknowledgements, lost ACKs,
stop boundaries and restart recovery.

The GTK conversation now binds text Send/Stop to this durable execution API.
A worker reads controller identity before SDK history, projects only changed
jobs, and prepares rich replies with original target source anchors. The native
transcript, source-context view, relative file links and Favorites use those
anchors. A metadata-only controller read fixes restored title/project state
without loading raw decision JSON. Transport retries retain request UUIDs and
newer drafts; definitive HTTP rejections allow correction. Actual Wayland button
checks cover both target backends, stop boundaries and reopen without replay.

Native Router history now has Older/Newer/Latest cursor navigation. Polling keeps
its current range and only retains that page's rich replies. Indexed unread totals
and active identities cover all rows, including records outside the newest page.
Versioned, controller-scoped native read receipts update SQLite and its dispatch
mirror transactionally; stale or active revisions cannot clear new results.
Automatic reads require an active, uncovered conversation and a visible final
block for 500 ms. Explicit visible-read actions also work from the toolbar/menu.
Wayland checks verify hidden/offscreen guards, read persistence across reopening,
three-page navigation, historical-range retention and bounded reply retention.
Legacy schema migration preserves payloads, owners and row cursors. Physical
foreground/focus transitions still need manual validation.

Recent completed response context now comes from bounded SQLite projection.
The same controller's last 32 completed rows supply up to three eligible targets,
with current session-key/project checks. Original response tails survive display
clipping, and escaped quoted data stays inside the 512 KiB instruction budget.
No extra SDK target-history read is made for this context.

Native reply actions now open the actual target session and restore the original
Router through a header Back action. Metadata identity/project/archive checks,
source-connection guards and explicit backend restore targets protect navigation.
Drafts and model options stay with each conversation. OpenCode's known project
scope no longer depends on a catalog entry, and canonical current references
remain in refreshed catalogs. Replies beyond the live tail use the existing
read-only historical context. Wayland checks cover both backends, uncatalogued
targets, delayed results, original question widgets and old reply lookup.

The Router composer now supports explicit target selection as well as Automatic
routing. The picker reads a bounded, planner-filtered inventory from the gateway;
search runs on Rust workers and GTK constructs at most 50 rows per page. Backend,
session and project identities stay distinct. Choosing a target preserves the
controller draft/options and bypasses its model when sending. The exact target is
revalidated at delivery; rejection does not silently reroute. Configuration,
connection, session and per-picker identities guard stale choices/results, including
callbacks from closed dialogs in the same conversation. Pending delivery UUIDs
retain their original target snapshot. Wayland coverage exercises result paging,
cross-backend choices, Automatic reset, stale-dialog signals and explicit sending.
Large/incomplete discovery still reports the shared catalog limits; browsing
arbitrary targets beyond that inventory needs a separate point-lookup affordance.

Router text follow-ups now share the existing queue UI/storage/depth controls.
Acceptance atomically removes the exact queue head and reserves its durable job.
Whole-delivery status gates playback independently of the displayed history page;
controller SDK completion cannot release it. Rejected messages stay queued, and
accepted UUIDs are idempotent. Stops, failed/clarifying deliveries, disconnect and
restart pause playback; Resume is explicit. Current controller options, scoped
permissions and target choices are read at dispatch and then retained in the
submitted request identity. UUID-based native queue messages are supported;
queued images stay blocked. A dedicated two-launch Wayland fixture
covers cross-backend playback, newer drafts, Stop, paused restore and Resume.
Concurrent browser/native queue editing remains a separate reconciliation gate.

Direct Router image input is now implemented, including image-only requests and
cross-backend delivery. Bounded Rust workers validate and serialize input; GTK
holds shared snapshots. Only confirmed target turn identity consumes submitted
images/text. Rejected/clarifying/pre-dispatch failures retain them, and newer
inputs survive acknowledgement. Durable history stores counts rather than image
payloads. OpenCode uses MIME/URL echo matching and bounded read-window reduction
for large history pages, without mutation retries. A Wayland image fixture covers
large bodies, image-only input, stale-config rejection, clarification, newer drafts
and cross-backend delivery. Vision-provider compatibility needs real-model checks.

Direct reference routing is implemented for scoped files and target-validated
Codex skills/plugins. A native chooser prepares only file metadata; Router `@`
completion preserves its project root. Explicit-target completion uses the target
backend/project without changing the visible controller; both connection scopes
and target identity guard late results/actions. Canonical paths/regular-file checks and
origin containment guard file dispatch. Codex gets absolute quoted paths;
OpenCode gets local file URIs. The target's enabled inventory validates named
skills/plugins; controllers receive descriptions without typed activation inputs.
Pending-input snapshots preserve newer drafts and survive clarification or
pre-dispatch rejection. A dedicated Wayland fixture covers a 30 MiB file without
content transport, actual completion/chips, session scope, disabled target skills,
cross-backend files and incompatible Codex skill retention on OpenCode targets.

Queued Router references now use the same durable metadata format and native
controls. Atomic reservation compares both text and references; missing or
out-of-project files are rejected on workers before dequeueing. Submitted metadata
snapshots survive uncertain HTTP acknowledgements, while accepted UUID lookup
precedes filesystem checks to prevent accidental replay after a file disappears.
Queued snapshots require a matching POST acknowledgement; status polling alone
cannot consume them. Explicit Retry keeps the UUID/references and does not execute
a newer slash/shell draft. A private lost-ack UI check covers this boundary.
Accepted failures pause subsequent playback and remain in history. A separate
two-launch Wayland fixture covers 30 MiB metadata-only files, cross-backend
delivery, newer reference drafts, reference-only skill/plugin inputs, Stop,
paused restart, missing-file retention, file restoration and explicit Resume.

Equal immutable chat refreshes now rebind message callbacks while retaining
table pagination, selected columns, wrapping and resized column widths. Favorites
keep their exact-current-message guard; the actual Wayland message action is
exercised after an explicit equal-snapshot refresh.

Persisted Router history now has native literal search through the existing Find
action and a compact toolbar entry. A read-only SQLite snapshot scans stored
requests, saved reply excerpts/tails, target details, decisions and reference labels.
Rust regex handles Unicode case matching and literal punctuation/whitespace;
optional Match case and existing Older/Newer/Latest controls retain the filter.
Two worker slots and a two-second scan budget bound history work without taking
Studio's write mutex. Results are pages of 20 thin records; attention and active
delivery metadata remain unfiltered. Query/controller/serial guards reject late
results before and after the entry's debounce. Three-launch Wayland checks cover
the actual Find action, old records, live polling, controller switching, read
receipts and unsent drafts. The 5,001-job private API fixture verifies stored reply
parts/tails, controller isolation, exact spaces, paging and unchanged records/receipts.
This checkpoint passes 124 native and 199 gateway Rust tests (one ignored each),
both optimized-profile checks and the four-launch real-report regression. Native
search applied in 181 ms including debounce, with a 69 ms maximum observed
workflow heartbeat gap. These debug observations do not complete the remaining
full-application migration, desktop/provider acceptance or release packaging.

Review browser/native history-key reconciliation before
claiming full frontend interchangeability. Keep the full browser behavior in
view (`ui/thread-router-controller.mjs` and
`ui-src/{thread-router,router-coordination,router-attention,router-targets}.mts`).
Real-provider validation remains outstanding. Full native application migration
remains the objective.

Interrupted controller creation now has explicit native recovery. The GTK editor
offers existing sessions from the dedicated workspace or manual ID entry, then
requires metadata inspection before enabling Adopt. Rust verifies the exact
backend, root identity, canonical workspace and idle/unarchived state, and
re-reads metadata during adoption. Configuration and backend-scoped journal
revisions reject stale choices. Per-backend setup ownership blocks recovery while
an accepted creation is still running; recovery never creates, renames or prompts
a session. The selected identity is retained before an OpenCode permission
update; failures resume through the known ID. Registration preserves the active
backend, fallbacks and unknown settings. Native drafts stay unsaved; controller
acknowledgements that reveal concurrent configuration changes require
Reload/review before Save. Private Codex/OpenCode API checks cover genuine lost
acknowledgements, restart, invalid metadata, stale journals/settings, permission
failure and exactly one original creation. Two Wayland launches verify real
picker/inspection/adoption controls, changed IDs, closed/reopened dialogs,
in-flight close guards, drafts, concurrent-configuration protection and restart.
Physical desktop/provider and release acceptance remain part of the full goal.
The checkpoint passes 124 native and 200 gateway Rust tests (one ignored each),
both optimized-profile checks, the existing setup lifecycle fixture and the
four-launch real-report regression. The 4.1 MB report opened in 926 ms with 56
opening heartbeat ticks. Release packaging and the remaining parity gates stay
in scope.

OpenCode Router skill commands now use the target project's command inventory
and legacy command endpoint. One exact `source: skill` entry can accompany scoped
files and images; incompatible Codex paths/plugins and multiple skills fail
without a target command. Rust workers discard inventory templates and command
reply bodies after bounded decoding. OpenCode owns expansion and message IDs;
Studio sends no client message ID and never retries an uncertain command. A lost
response or interrupted dispatch remains failed after restart without replay.
The selected target model and original whitespace reach the command unchanged.
Explicit OpenCode target summaries also preserve the session title.

The legacy synchronous API returns identity after execution, so pending jobs
explain that stopping requires the target conversation. Router Stop rejects that
pending interval explicitly; it cannot safely abort an unbound message. Stop and
the last pre-send check serialize under one short lock. Once identity is known,
the existing exact-message guard and authoritative idle behavior apply. The
private API fixture covers 10 command deliveries, attachment/model fidelity,
inventory collisions/disablement, identity mismatch, failure, lost response,
restart and owned-child cleanup. Actual Wayland controls cover completion,
pending Stop rejection, viewport-checked screenshots and newer draft retention.
This checkpoint passes 124 native and 204 gateway tests (one ignored each),
both optimized-profile checks and the existing Router runtime regression. The
four-launch real-report regression also passes: its 4,097,226-byte report opened
in 1,075 ms with 65 opening heartbeat ticks. These debug workload observations do
not imply a performance improvement. The command fixture uses deterministic
private providers; real-provider verification, the documented early-Stop
limitation, full desktop acceptance and release packaging remain open.

Native agent input now has a worker-normalized, bounded typed model and GTK
forms for Codex choices/custom/secret answers and standard MCP primitive schemas.
Defaults, required fields, selection values/counts, Unicode lengths, numeric
ranges and supported text formats are validated off GTK before one reply is
queued. Cancelled questions use an explicit cancellation error; MCP Cancel and
Decline remain distinct. URL requests show the server and destination before
an explicit system-browser launch. Resolution notifications, non-ready status, backend errors and
reconnects invalidate the exact typed request ID/generation and cancel pending
URI work. Unsent conversation text remains independent. Legacy command/file
approvals expose session scope when available, and OpenCode maps that explicit
choice to `always`. Extended elicitation modes, complete ECMAScript pattern
equivalence and real-provider acceptance remain
part of the wider parity audit.

The private Wayland fixture checks actual controls and 12 provider-wire replies,
including invalid fields that never reach the provider, masked entries, enum
wire values, optional omission, URL consent/cancellation, numeric/string request
IDs, external resolution, reconnect and session-scoped grants. Screenshots of
questions, MCP forms and URL requests were inspected. Private mount cleanup now
also handles that runtime's GVfs FUSE service; boundary tests reject host paths
and unexpected filesystem types. URI tracing records only actual responses and
cancellations, including timers after a request has closed. This checkpoint
passes 131 native Rust tests (one ignored), three cleanup-boundary tests and the
optimized native build check. The full four-launch regression passes, including
the 4,097,226-byte Markdown report, paged editing/source/tables, sessions and
OpenCode workflows. That report opened in 902 ms with 56 opening heartbeat ticks.
The first input form mapped in 101 ms; workflow heartbeat maximum gap was 457 ms
including synchronous screenshot capture. Observed debug native/gateway peaks
were 322,776/43,876 KiB. These are local observations, not a performance claim.
No fresh release bundle or physical desktop/provider acceptance is implied.

OpenCode input now preserves the global-event project's directory through the
reply, including after conversation/backend changes. A connection-owned bounded
registry prevents ID rebinding and duplicate response attempts. Official
`requestID` resolution events close the exact original permission/question;
legacy Codex approval dialogs now share backend/generation/lifecycle guards with
typed forms. Native OpenCode questions support multiple/custom responses and
`custom: false`, with ordered arrays and aggregate answer-size validation.

Session reads return history before recovering that project's pending inventory
on two background tasks, coalescing up to 16 queued projects. Recovery projection
runs in the connection's event order; tombstones suppress resolved entries and
reject snapshots that outlive the bounded resolution history. Failed mutations
are not replayed. Reconnect can recover an unaccepted pending request for a new,
explicit response. A shared proxy correction preserves encoded ID segments and
directory query parameters; private fixtures now decode router parameters as
the upstream does.

The private Wayland input workflow verifies nine OpenCode POSTs and one Codex
response, initial inventory recovery, separate projects/backend views, external
resolution, held stale snapshots, lost ACK/reconnect, old-dialog/generation
guards and retained drafts. Its Unicode/space-containing project paths and
slash-containing request ID exercise the actual gateway encoding. History
remained usable in 20 ms during a held recovery read. The largest workflow
heartbeat gap was 466 ms including capture; observed debug native/gateway peaks
were 322,260/44,068 KiB. These do not claim performance improvements. The
checkpoint passes 139 native tests and 205 gateway tests (one ignored in each),
the Codex/MCP input and Router dispatch regressions, the four-launch application
regression with the actual 4,097,226-byte report, and optimized native/gateway
checks. Extended modes,
real-provider acceptance, broader parity and
release packaging remain open; the upstream OpenCode stuck-busy behavior is
unchanged.

Native command approvals now support exact server-offered command-prefix and
network allow/deny policy decisions through an explicitly selected native
picker. Ordinary once/session decisions stay independent. Command and file
approvals distinguish decline from cancel/stop-turn; permission-profile grants
retain turn/session scope. Rust workers prepare bounded choices, readable full
scope summaries and exact response values; GTK receives routing metadata and
the prepared model. Unknown request fields stay visible. Oversized/malformed
requests have no grant controls, and an empty available-decision list remains
empty. No policy is invented, preselected, written locally or replayed.

The expanded private Wayland input fixture verifies 20 provider replies, exact
prefix/host/action values, empty/malformed lists, once versus persistent choice,
stop-turn replies and resolution while a policy is selected. Nine approval
responses validate against the installed Codex 0.163.0 generated schemas.
Screenshots now reject blank frames and wait for actual GTK painting; selected
prefix/allow-host/deny-host controls and scope descriptions were inspected.
The checkpoint passes 145 native Rust tests (one ignored), the optimized native
check and the full four-launch application regression. The real 4,097,226-byte
report opened in 902 ms with 56 opening heartbeat ticks. Input first mapped in
101 ms; maximum workflow heartbeat gap was 445 ms including capture, with
322,928/43,420 KiB debug native/gateway peak RSS. These are local observations.
Physical/provider acceptance, broader parity and release packaging remain part
of the full goal. Permission selection and turn review are implemented below.

Native additional-permission approvals now select exact network/filesystem
subsets, including target-native literal, glob and special entries and legacy
read/write paths. Typed path/access identity coalesces modern/legacy aliases
without depending on JSON property ordering. Unchecked grants disappear from
all aliases; original selected values, deny entries and applicable scan limits
remain intact. Selecting all returns the exact original profile. Turn/session
scope and turn-only strict review are explicit native actions. Invalid/unknown
profiles acquire no grant controls; the worker model and GTK lists are bounded.

Final response construction now runs off GTK. A request's submission revision,
backend/generation and lifetime guard every worker result. GTK close-request
deferral handles libadwaita's close-before-response behavior: closing invalidates
an in-flight grant immediately, and a decline can supersede validation. A failed
queue admission keeps the review controls and selection. External resolution,
status loss and reconnect discard late results rather than replaying them.
Native selection counts use constant work per toggle.

The permission-subset checkpoint's private Wayland fixture checked 28 provider replies, partial turn/session
grants, strict review, aliases, constraints, empty/all selection, unsupported
profiles, all 128 bounded controls and retained composer text. Reentrant
resolution/status and actual backend reconnect verify late-result guards;
window-close-during-validation sends only a decline. Eight permission replies
validate against the installed Codex 0.163.0 response schema. Physical/provider
acceptance and the broader document, settings, platform and release gates remain
part of the whole-application goal.

The permission checkpoint passes 152 native Rust tests (one ignored), the
optimized native build check, OpenCode input recovery, private two-launch
notification transport/activation and the four-launch application regression.
The real 4,097,226-byte report opened in 1,002 ms with 62 opening heartbeat ticks.
The 28-reply workflow first mapped input in 106 ms; its maximum heartbeat gap
was 499 ms including capture. Observed debug native/gateway peaks were
324,948/43,144 KiB. Bulk clear/reselect of all 128 checkboxes took under one
millisecond of synchronous event handling, excluding layout. The selected
permission/glob review was inspected in a real painted Wayland screenshot.
These observations do not replace physical/provider acceptance, existing
platform checks or release packaging.

- OpenCode HTTP/SSE, history/status, models, permissions/input, reconnect recovery,
  and the currently documented upstream limitations.
- Session grouping, persisted selection, pins, Attention/Active/archive views,
  search, rename/fork/archive/delete, hidden-directory filtering, pagination,
  background lifecycle tracking, and stale-status recovery.
- Router setup, responsibilities/targets, dispatch/replies, attention reconciliation,
  and cross-backend navigation without bypassing runtime isolation.
- Subagents, turn navigation, session maps, annotations/review notes, favorites,
  export, and in-session search.

Session Map migration has started with shared wire types/validation and a native
worker client for the existing store. Native projection retains all 1,000 supported
topics with hierarchy, visibility, current trail and progress; mutations preserve
scope/revision identity and are not replayed after an uncertain reply. The shared
gateway moves map database work off request threads with bounded admission, and
optional map-ID/revision checks make native mutations safe against newer changes
and replacement maps while
preserving older clients. The focused native/gateway map tests, the 23 existing
browser map tests, full Rust suites and real-report Wayland regression pass.
The native Map pane/create controls and isolated AI goal/outline generation are
now implemented. The pane uses virtualized native topic rows, current selection,
human Done, Undo, Reload and confirmed Delete. Creation permits an optional goal;
AI tasks use the shared bounded structured transport, retain source drafts,
clean up their owned ephemeral threads, and freeze map ID/revision. Native unit
coverage includes input packing, role/Unicode handling, safe operations and
context budgets. Native direct/queued turns now attach bounded Map context and
support same-turn dynamic tools or private trailing metadata, including offscreen
updates. Tool retries are acknowledged without replaying mutations; conflicts
retain the frozen map ID/revision, and an uncertain tool write blocks fallback.
Queued turns wait for a pending Map write before capturing their next context.
Streaming/history/copy paths hide the suffix, and provider generation changes
clear stale contexts. Progress stays visible beside sync feedback; conflict
recovery reloads the current snapshot.

The seven-turn native Map/clipboard acceptance, controls/restart and translation
acceptance, four-launch real-report regression, 230 native unit tests, 23 browser
Map tests and Rust 1.92 checking pass. Thread Router's server-side dispatcher now
shares the Map protocol, owns target tool replies, persists captured scopes and
recovers completed updates without replaying inference. Controller Maps remain
manual so routing keeps its strict JSON decision protocol. Nine routed API cases,
two gateway restarts and actual native live-pane/record checks pass; gateway
coverage is now 237 passing tests. The optimized installed pair was refreshed
and passes all five existing optional workflow groups. Extended language,
concurrent-client and actual-provider acceptance remain. The whole application remains the
migration scope.
Installed Map controls also pass keyboard menu navigation, current/Done/Undo,
closure and persisted revision checks without model turns or application hooks.
Generated GMenu rows exposed empty accessible names on this host. The native shell
now uses public GTK action buttons with MenuItem roles, retaining the original
GMenu labels/actions and adding bounded arrow-key navigation. Debug and optimized
accessibility checks verify named actions, keyboard selection and persisted Map
changes. The optimized installed pair passes all five workflow groups, including
menu names after live English/Chinese changes; broader physical accessibility
acceptance remains open.
See [Map migration](gnome-native.md#native-session-map-in-progress).
- Attachments (picker/clipboard/drag-drop), file/skill mentions, command palette,
  shell commands, model/effort/Fast, permission controls, queues and steering.

### 5. Settings, browser workspace, and desktop integration

Native preferences now include Appearance, Sessions/visibility/notifications,
Files/shared folders, project Environment, backend registry diagnostics and an
explicit backend configuration JSON editor. Shared project variables/cache paths, multiline values,
network policy and redacted secret replacement/removal use the existing Rust
APIs. Revision checks prevent stale saves; disk I/O and locking run on workers.
Wayland acceptance verifies exact values, redaction, Save/Reload, cancellation,
and single-selection theme changes. Full advanced settings remain in scope.

The backend editor shares the gateway startup schema, supports repairing invalid
JSON, and writes the exact UTF-8 source only on Save. Scoped content/path
revisions, a recheck immediately before atomic replacement, and confirmation
after replacement detect external changes while retaining the local editor.
Reload and closing require discard confirmation for unsaved edits; pending writes
guard both editor and parent Settings closure. The active registry stays fixed
until restart. Private API and two-launch Wayland checks cover explicit saves,
invalid-file repair, undo/redo, stale conflicts, restart and lazy provider startup.
This is optimistic conflict detection, not a cross-process filesystem lock. See
[native backend configuration](gnome-native.md#native-backend-configuration).
The optimized installed pair also passes the `--backends` release acceptance
case through actual accessibility/keyboard input, without application test hooks,
alongside language/question, source editing, all four diagram engines, image
save and desktop handoff. This checkpoint passes 210 gateway tests (one ignored),
170 native tests (two ignored), the Rust 1.92 check and the four-launch Wayland
regression using the real 4,097,226-byte report. Full application parity and the
physical/real-provider/cross-platform gates remain open.

Desktop alerts now use GIO and the shared opt-in preference through a narrow,
revision-checked Rust transaction. Ordinary completion/failure and input-request
alerts replace per-session notices, reject duplicate/stale candidates, and avoid
history/idle-only alerts. Activation brings Studio forward without answering a
request. A bounded native ledger supports withdrawal after restart; native
controls handle busy states, stale conflicts, Reload and invalid-type repair.
Private-bus Wayland checks cover actual notification transport, approval replay,
click activation, two-process D-Bus reactivation, ledger bounds and cleanup.
The installer packages matching desktop/D-Bus files; real parser/activation tests
cover special-character installation paths and the file-opening fallback.
Real GNOME banners, Do Not Disturb, physical focus, provider delivery
and fresh release installation remain gates. See
[native notifications](gnome-native.md#native-desktop-notifications).

Router job completion, failure and clarification now feed the existing GIO
notification service. The bounded transition tracker uses existing per-controller
summary metadata, never historical-page loads or hidden rich responses; displaced
active jobs retain status-only watches behind queue gates. Private GIO transport
and two-launch checks verify three Router notices without duplicate/stale/history
or stopped-job alerts, retained draft/selection and unchanged hidden reply trees.
The ordinary Router delivery/navigation/history/restart workflow passes against
real gateway summaries and fake Codex/OpenCode providers. Physical GNOME banner,
focus/DND, actual-provider and installed-release acceptance remain open.

Shared-folder controls use native rows, folder chooser and discard confirmations.
Save/Reload uses a bounded Rust transaction scoped to the folder-array revision;
unrelated/future JSON values survive. Policy changes and Files root inventory
apply without restarting. Private API and two-launch Wayland acceptance pass for
add/remove, explicit persistence, literal/unavailable paths, live grant/revocation,
dirty/busy/parent close guards, cancel/discard, stale conflicts with retained edits,
and restart. The real default GNOME/Nautilus chooser now also passes on an owned
Mutter desktop: shared-folder cancellation and Unicode selection remain local
until explicit Save, with unrelated settings retained. The same host regression
covers Markdown Open and exact original-image Save/Cancel without a portal stub
or application hooks, on development and frozen optimized pairs. Physical
pointer/touch, fractional scaling, remote locations and deployment acceptance
remain open. See [real portal acceptance](gnome-native.md#native-file-chooser-and-portal-handoff).

Session visibility now has native hidden-folder/rule editors with explicit
Save/Reload, conflict/close guards and live application. The native list and Router
reuse the same Rust policy engine, including bracket classes and browser-compatible
path normalization. A 33-case fixture runs in both JavaScript and Rust. List
search, visibility, activity projection and stable pin ordering use a coalesced
worker; GTK consumes prepared rows with sequence isolation. Catalogs retain at most
10,000 entries with an explicit incomplete notice. Private two-launch Wayland
checks cover rules, selected history/draft retention, conflicts and restart.
Recovery editing now supports readable invalid lists and explicit in-memory
replacement of values that cannot safely appear in the line editor. GET preserves
the raw scoped projection with a validation error; valid replacements retain
revision checks and unrelated settings. Cold-start registry gating remains in
place and normal conversation loading resumes after repair. Parser expansion
admission and linear class-ending lookup bound extreme inputs. Private API and
two-launch recovery checks cover a one-MiB line, malformed fields, over-limit
lists, cancellation and exact saved-byte preservation until Save. Malformed JSON
documents and settings beyond the document budget remain errors; actual
folder-chooser portal acceptance remains in scope. See
[session visibility](gnome-native.md#native-session-visibility).

Shared typography and reading-layout controls now use a narrow revision-checked
Rust API that preserves unrelated and future settings. Interface/content/workspace/
code fonts, content inheritance, reading width and increased contrast apply after
explicit Save. GTK/VTE retain GNOME fonts when customization is off. Width changes
update realized Markdown rows without replacing the scrollable virtualized list.
Private API and two-launch Wayland checks cover validation, concurrent edits,
reset, close/reload cancellation, actual composer/VTE fonts and restart. Inline
Markdown code now shares code typography in short labels, reused table cells and
paged rich paragraphs. Weak bindings update existing views after Save and restore
the original markup/inherited tag sizes when customization is disabled. GTK/Pango
acceptance checks actual family/absolute size/weight, nested emphasis, exact
Unicode quotes, selection through style changes and restart. Mermaid.js themes
and layout-engine preferences, browser typography desktop acceptance, complete
interface translation and full
accessibility/IME/portal acceptance remain migration gates. See
[native appearance](gnome-native.md#current-feature-coverage).
The inline-code checkpoint passes 121 native Rust tests (one ignored), the
optimized-profile check, long-rich-text/source-projection acceptance and the
four-launch application regression with the real 4.1 MB report.

Markdown layout now exposes the shared Reading/Technical/Compact modes in this
same native editor. The scoped appearance API projects `markdownMode` onto the
existing `markdown.mode` field while preserving unknown Markdown fields and
unrelated preferences. Saving changes font size, line spacing and block spacing
in existing Markdown widgets; bounded text views update a line-height tag on
every page. Source/Edit, the composer and standalone source panes keep their
own typography. Private API and two-launch Wayland checks cover all modes with
custom/system fonts, actual wrapped-line advances, retained paragraph text/page,
explicit persistence and restart. The original Browser typography setting is
for its toolbar; website content retains its own styles.
This checkpoint passes 122 native and 197 gateway Rust tests (one ignored in
each), both optimized-profile checks, long-rich-text/source-projection acceptance
and the four-launch application regression. The real 4,097,226-byte report opened
in 1,064 ms with 64 opening heartbeat ticks. No fresh release executable or
installation bundle is produced for this checkpoint.

Native Mermaid preferences now cover text size, reading/code font families,
flowchart spacing and curves after explicit Save. Full presentation profiles are
part of the bounded raster-cache key. Existing chat/reader diagrams refresh with
cancellation and stale-result isolation; remapping Preview after Edit retains
unsaved text. The narrow Rust appearance API merges only these three Mermaid
fields and preserves other-client style/layout choices and unknown metadata.
Generated SVG normalization resolves/removes CSS custom properties so the Rust
SVG parser reaches inherited font declarations. Real raster tests verify
font-only and curve-only changes, and private two-launch Wayland acceptance
checks reader/chat consistency, dirty-buffer/file preservation and restart.
Screenshots of both sizes were inspected. The suites pass 124 native and 198
gateway tests (one ignored each). Pele's own appearance/layout does not implement
Mermaid.js themes, hand-drawn rendering or ELK selection; these remain parity
gates alongside the broader document, provider, desktop and release gates.
Both optimized-profile checks, long-rich-text/source-selection acceptance and
the four-launch regression pass. The real 4.1 MB report opened in 1,068 ms with
65 opening heartbeat ticks. These debug checks do not produce a release bundle.

The observed narrow-split conversation/composer clipping is now fixed with shared
native wrapping toolbars and bounded selector labels. Conversation search, chat/
reader table controls and the reader header use the same libadwaita mechanism.
Sixteen Wayland layout cases verify visible pane bounds, long names, contextual
controls and maximum fonts; popup text/selection and actual heading typography
are also checked. Wider workspace-tool coverage, short windows, physical splitter
dragging, assistive technology and fractional-scaling acceptance remain gates.

The browser workspace now uses GTK tabs, address/navigation/search/zoom,
download controls and shared source-linked session comments. The content engine
starts lazily, uses an isolated ephemeral session and has no Studio IPC capability.
Both desktop shells share the same Rust URL validation/preferences module.
Private Wayland acceptance passes navigation/history/tabs/search/zoom, comments,
crash/reload and Exit/reopen storage isolation. Native upload/download acceptance
now covers sixteen real chooser calls, exact HTTP bytes, six cancellations and
late replies, same-URL/tab ownership, existing-file protection and policy changes.
Downloads use Rust-owned staging and atomic no-overwrite publication; WebKit never
receives the final destination. Four file-worker lanes, preparation admission and
an actual window-close/cleanup check keep file work off GTK and retain owned
cleanup during shutdown. The corrected private portal text scale removes a false
zero-viewport failure; no speculative tab-layout change remains. Native tests pass
195 cases (two ignored). The final mapped Downloads screenshot and real pending
file-worker window-close check pass. The refreshed optimized pair also passes
installed editor/Find/diagram/language/backend/Git/viewer acceptance and the
four-launch real-report regression. Focused transfer checks use debug controls;
real desktop portal presentation, persistent profiles, advanced
browser preferences and full page annotations remain. See
[browser file transfers](gnome-native.md#native-browser-file-transfers). See [native browser scope](gnome-native.md#native-browser-workspace).

Opt-in browser address restoration now uses a bounded native-only metadata file
and revision-checked worker API. Settings alone and ordinary startup keep the
engine idle. Browser Open restores permitted addresses and the active tab; a
specific URL opens alone. Individual/last tab closure is saved, while Exit clears
private page storage and retains opted-in addresses. Restore off/on rotates the
metadata generation to reject stale saves and forget old addresses. One in-flight
save plus one latest snapshot bounds synchronization; window closure waits for
confirmed metadata persistence. Real revision-failure Cancel disarms deferred
close callbacks, and explicit reopen retries against a new revision. The private
Wayland check passes across two actual process launches, including final queued
selection, empty page storage, cleanup and zero model turns. Metadata reads are
skipped when restoration is off, and a per-transfer cancellation gate prevents
repeated engine cancellation across chooser, button, policy and Exit paths. The
sixteen-call file fixture passes repeated Cancel in the same GTK turn; full
four-launch application acceptance also passes. See
[address restoration](gnome-native.md#native-browser-tab-restoration).

Settings → Browser now provides native explicit Save/Reload controls for enabling
Browser, HTTP navigation, private IP addresses, localhost, page JavaScript and
opt-in tab restoration.
A scoped revision transaction preserves unrelated browser/agent settings and
unknown fields. Conflicts retain local edits; closing/reloading protects unsaved
changes. Successful reads/saves update current tabs: disallowed addresses close,
JavaScript changes reload, and disabling Browser ends its private session. A
rejected Open cannot be replayed by re-enabling Browser. Private Wayland acceptance
uses the real Settings shortcut, native discard buttons and local pages to verify
these behaviors without model turns. This does not complete browser profiles,
cross-client live policy synchronization or physical file portal acceptance.

Native interface language now shares the existing `settings.json` field through
a narrow Rust worker API, with scoped revision conflicts and explicit Apply/Reload.
The embedded catalog reuses Studio's translations plus native additions; it adds
no runtime Node dependency. Explicit weak bindings update existing controls and
translate UI choice labels without changing their keys. Core shell/menu/reader
controls and the settings panels have coverage; unmarked content and model/user
values remain literal. Conflict, reload, restart and unsaved editor/draft
preservation are covered by private API and Wayland fixtures. Remaining workspace
dialogs and dynamic explanations still need translation; full language parity
remains a migration gate. See [native interface language](gnome-native.md#native-interface-language).
Files now has live control/menu/status/row language bindings, with literal model
names, paths, diagnostics and action targets preserved. Shared row presentation
retires stale bindings on recycling, including value-equal UI words. The focused
Wayland runs cover fixed notices, badges, repeated locale changes, real shared
folder Copy and unchanged navigation; the existing language save/restart workflow
also passes. Common workspace icon tooltips now register explicit bindings, and
Source headings retain the code context. See
[Files language](gnome-native.md#native-files-language). Other workspace dialogs
and diagnostic language still remain before complete language parity.
This checkpoint passes 170 native and 206 gateway tests (two and one normally
ignored checks respectively), Rust 1.92 checking and the four-launch real-report
Wayland regression. The optimized installed pair also passes bidirectional
language selection through real GTK controls alongside editing, four-format
diagram inspection and original-image saving, without application test hooks.
This is local private-desktop validation; physical GNOME and remote CI gates
remain open.

- Native agent-input/approval and queue controls now share live translation
  bindings for ordinary, formatted and response labels. Exact questions, choices,
  policy scope, paths, typed text, queue identities and reference data remain
  literal. English/Chinese fixtures each verify 29 provider replies, live locale
  changes with open forms/policies/subsets, cancellation and late-result guards.
  The queue fixture verifies explicit editing/save and restart against the shared
  SQLite store. A screenshot review also exposed a stale custom-answer override;
  single-choice custom answers now have an exclusive Other radio option, so a
  selected provided choice determines the reply while its custom draft is kept.
  A provider-wire test checks this selection, and existing OpenCode input recovery
  plus the four-launch real-report regression still pass. The refreshed optimized
  pair also verifies both preset/custom cases through actual editable text and
  native radio-group keyboard navigation in the installed `--language` workflow,
  without application test hooks. Rust 1.92 checking and all 170 native unit tests
  pass. Translation of remaining
  workspace dialogs and worker summaries/errors, physical input/accessibility and
  real-provider acceptance remain open.

Native comparison actions, mode-dependent tooltips, empty/loading statuses and
change/count/timing summaries now have live language bindings. Markdown footers,
outline line tooltips and Source/diff line-range/limitation summaries share those
bindings. Explicit literal paths prevent a filename equal to a UI key from being
translated. The private fixtures check selection, source ranges, file identity,
all status-flag combinations, dirty editor state and return-to-reader behavior.
See [document language coverage](gnome-native.md#native-document-and-comparison-language).
The final native suite passes 204 tests (two normally ignored), Rust 1.92 checking
passes, and the four-launch real-report, language/save/restart and 4.2 MB code-edit
regressions pass. The matched optimized package passes installed
`release-ui-geou0_w1`, including exact bilingual page statistics and native
comparison navigation alongside all five existing workflow groups, without
application test hooks.
Other workspace dialogs/diagnostics and the broader acceptance gates remain.

Favorites controls, counters, accessibility labels, view tabs and confirmations
now follow the interface language without translating editable metadata or source
content. Shared export failure labels/acknowledgments and explicit interface
notices also use weak bindings; destination/diagnostic values remain literal.
The existing live settings fixture preserves an unsaved Favorite and open prompt
across Apply/Reload, then verifies an explicit save after restart. A separate
installed-pair accessibility workflow exercises both languages, dirty guards,
Source/Preview, Copy and native export cancellation/failure/success with no model
turns. Remaining workspace/diagnostic language and physical acceptance stay in
scope. See [Favorites language](gnome-native.md#native-favorites-language).
The optimized pair has been refreshed and passes that installed Favorites
workflow, all five existing installed workflow groups and the installed Map
controls check. Native unit tests, Rust 1.92, the shared export-failure regression
and the four-launch real-report application regression also pass.

Comments controls, status/cleanup messages, editor accessibility and native
confirmations now follow the interface language. Composed source descriptions
translate only explicit interface fragments, retaining source identifiers,
literal excerpts, anchor extensions and prompt payloads. Live Apply/Reload and
save/restart checks preserve an open dirty prompt. Both locales pass editing,
conflict/reload/source checks and all seven fake-provider submission/cleanup
cases. A separate installed-pair workflow verifies actual controls, insertion
without sending, source reopening and removal. Native tests, Rust 1.92 and the
four-launch real-report regression pass. Other language/diagnostic, selection
and physical/provider/platform gates remain. See
[Comments language](gnome-native.md#native-comments-language).
The refreshed optimized pair also passes the bilingual installed Comments and
Favorites workflows, all five broader installed groups, and Map controls/Undo.

- Complete language/translation coverage, themes, fonts, contrast/density/content width, default and
  per-session models/effort, environment profiles, redacted secrets, network/cache
  settings, backend configuration and diagnostics.
- Physical notification acceptance, external links, clipboard/file portals, application identity,
  high DPI/fractional scaling, system theme changes, accessibility, complete
  Chinese/English UI text, and ordinary GNOME launcher startup.
- Embedded Browser: native GTK4 controls/tabs/menu/downloads, URL policy,
  isolated storage, explicit lazy engine startup, crash recovery, page comments,
  and cleanup. Remote content must retain its capability/origin isolation.
- Reproducible packaging and fresh-machine installation, with no knowledge of
  the separate MarkNoria repository required.

Native selected-text translation now has a GTK dialog, context-menu action and
Ctrl+Shift+T shortcut, literal source/result text, English IPA, Chinese pinyin,
Copy, and an explicit shared Translation settings page. Rust workers run isolated
Codex/OpenCode tasks or the existing local Ollama endpoint, with bounded task
admission and a byte-accounted memory cache. Codex completion before its RPC
acknowledgment, cancellation before creation acknowledgment, invalid output,
temporary-task cleanup, browser selection, stale backend context, local model
listing, and preservation of both providers' unsent drafts pass the private
Wayland fixture. PDF Preview/Text and retained EPUB selection also pass their
translation checks. Native English, pinyin and Mandarin playback uses a local
Speech Dispatcher connection with scoped Stop and explicit dialog-close cleanup.
A silent server verifies literal multiline data, fragmented replies, early
completion, foreign-client event rejection and an unconfirmed-stop timeout.
Gateway-owned OpenCode utility receipts now filter live and late catalog rows
across the sidebar, agent catalog, Router picker and automatic Router discovery.
Explicit Router targets revalidate ownership before dispatch. The real GTK
fixture checks filtering while translation is still running and retains a
similar-named user session. Private API checks cover pending creation, original
page cursors, changed ownership and single-attempt cleanup. A managed native SSE
endpoint now removes owned OpenCode events before background activity and
notifications; actual GTK acceptance retains an ordinary session's busy and
Attention transitions. Both Rust consumers use the same incremental,
SIMD-capable SSE parser. Private API checks cover fragmented Unicode/BOM,
metadata-only blocks, late/adopted events, unchanged legacy output, stream limits
and 13 MiB delivery without the previous long-line stall. Actual voice quality
and broader physical desktop/accessibility acceptance remain work. The full
application migration remains the goal.
See [native selection translation](gnome-native.md#native-selection-translation).

The large data-image source checkpoint now passes the real 4.1 MB report's
four-launch regression, native comment relocation, Rust 1.92 checking and all
204 native/226 gateway/56 shared parser/35 MarkNoria tests. Its matching optimized
package passes installed run `release-ui-yll2vdt8` with all five optional workflow
groups and no application test hooks. The large-source fixture independently
verifies 14 MB original text, compact transport, original source lines and saving
the original image after reader closure. See the current
[large-image evidence](gnome-native.md#large-embedded-image-sources) for bounds,
measurements and package hashes. This completes that increment; the wider native
migration and remaining acceptance gates continue.

File-wide reader reference definitions now have a private disk stream and bounded
backend metadata/context resolution. Source/line anchors remain on the original
file; full/collapsed/shortcut references, Unicode folding, first-definition
precedence and continued headers have regression coverage. Large reference-image
URLs use the existing lazy snapshot protocol. Actual Wayland checks cover native
reference-link navigation and returning to images, a 10 MiB original, exact Save
after reader closure, continued table headers and cross-reader label isolation.
See [file-wide references](gnome-native.md#file-wide-markdown-references).
This increment passes 204 native, 226 gateway, 60 shared parser and 36 embedded
indexer tests, Rust 1.92 checking, the four-launch real-report workflow and comment
relocation. The matching optimized package passes installed run
`release-ui-u6clte9k` with all five optional workflow groups and no application
hooks. The optimized compositor-free reference API is also included in CI; its
local run verifies the new protocol without requiring GTK automation.
Native reader/chat tables now use shared viewport-admitted image previews, fixed
thumbnail areas, full-cell viewing and native header reading menus. Original
lookup skips nested image-alt descendants consistently with source projection;
inner images stay caption text. Actual Wayland checks verify PNG/SVG originals,
header and modal viewer controls, paging/scrolling, exact source lines, cleanup and
stale actions. Allocation-driven visibility handles page changes with unchanged
scroll ranges, without idle polling. The shared long-cell/comment/lifetime
regression remains passing. This increment passes 527 unit tests and Rust 1.92
checking, the four-launch real-report workflow, both-provider large-image history
and the large-reference-source regression. Its matching optimized package passes
installed `release-ui-xzd0wkj8`, including actual full-cell image handoff alongside
all five existing optional workflow groups, without application hooks. The
optimized table-image API fixture also passes and is included in CI.
See [table-image coverage](gnome-native.md#native-markdown-table-images).
Wider rendering/style/resource and physical-desktop acceptance gates still apply;
this remains part of the full application migration.

## Completion and validation

Do not complete this goal because a binary builds or a conversation works.
The entire application's workflows above need native implementations or an
explicitly user-approved scope change; no reduced scope has been approved.

Record implemented/verified coverage in `gnome-native.md`. Maintain deterministic
fake-backend tests for approvals, cancellation, failures, reconnects, lifecycle,
and cleanup. Exercise actual GTK widgets on Ubuntu 26.04 Wayland and inspect
screenshots. Mutating tests use private document/repository copies. Do not send
real messages or change real sessions in automated acceptance tests.

Keep the existing desktop/server/security/platform checks passing. Final native
acceptance includes the GNOME launcher, all application flows, large-document
performance, and checks that unused services/engines do not start eagerly.
