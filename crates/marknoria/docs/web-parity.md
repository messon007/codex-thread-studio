# Shared Web reader acceptance

Status: **Astra independent technical acceptance passed**. Studio adoption is merged into main in PR #55. This concerns
browser/WebView reading and its actual Rust dependencies. GTK/AppKit/WinUI,
native controllers and FFI are outside synchronization. The original Studio
checkout is untouched; adoption uses `sync/marknoria-public-reader`. Historical
checkpoints do not override this matrix.

The formal interface is `marknoria::reader`, `marknoria::web::Service`,
`web::reader_assets()`, `createReaderClient` v1 and documented ordinary-DOM
components. Immutable source/resource hashes are in `reader-manifest.json`;
Studio separately pins the reviewed archive in `scripts/marknoria-release.json`.
Ordinary builds/CI do not read a private checkout, its assets or credentials.

| Acceptance area | Current implementation / evidence | Acceptance / limits |
| --- | --- | --- |
| Functional union / public API | Shared parsing, queries, PDF/EPUB, cells/charts, Notes, original resources, typed references; original public Rust fixtures and real two-product tests below | Accepted and merged in Studio PR #55 |
| Ordinary entry / defaults | Every ordinary Studio file uses the SDK; fresh Preview, capability-gated Source/Edit, explicit unsupported errors; 15-format gateway corpus in both engines | Documented Windows/WSL2 entry passed; WSL1 is outside the supported requirement |
| Locale / shortcuts / visual / accessibility | Finite scoped host dictionary, live labels with literal data preserved; actual full Studio shell and standalone four-format scenes, screenshots, named controls, overlays and search | Control-name checks are not a full screen-reader audit |
| Release performance | Separate index time/RSS, content timing, heartbeat and DOM; first-open and browser-cache reopen measurements below | Known cold-start and first-locale risks; no pre-agreed absolute latency threshold. Existing resource budgets remain enforced |
| Fixed delivery | MIT allowlist, provenance, locked isolated resource build, unchanged import; two clean offline exports and eight archive/inventory/Git-byte checks | Fixed pin, six platform checks and Astra acceptance passed; merged in PR #55 |

## Formats and actions

All fresh documents open in Preview. Explicit/restored Source/Edit is retained
when supported and granted. Queries do not grant writes. All listed checks are
local evidence, distinct from remote platform acceptance.

| Format / actions | Shared behavior / defaults | Retained state / references | Executed checks |
| --- | --- | --- | --- |
| Small/large Markdown | Rust GFM, safe HTML and bounded virtual fragments; ordinary/virtual tables share layout/paging/cells/SQL/charts; authorized local/inline images and links | Original BOM/CRLF UTF-8; exact source/cell anchors; prose precision explicitly approximate; scoped resources | Markdown profile/projection/references, two-pane embedded; actual common-entry and large-document Studio corpus |
| Text / Python / C / C++ / Rust | Rust language/highlighting and shared paged CodeMirror Source/Edit; complete Preview search, current-page editor search | Editor LF/UTF-16 maps to original UTF-8; selection, draft/undo/redo, atomic save/conflict | Syntax/structured/source/CodeMirror contracts; actual entries and edit/save/conflict |
| JSON / JSONL / XML / TOML / YAML | Bounded structured Preview/highlighting; JSONL formatting in Rust; complete-input jq with paged text | Original/formatted coordinates distinct; exact numeric text; visible input/output/cancel budgets | Structured/jq/embedded-jq and actual ordinary corpus |
| CSV / TSV / XLSX | Shared independent paging, compact widths, drag/keyboard resize/fit/reset, columns/edge scroll, complete-input SQL/charts | Source row/column/sheet; session widths/selection; text Source/Edit, no workbook Source/Edit | Table/layout/cell/query/plot contracts; actual three-format corpus against locked ExcelJS oracle |
| Cells / SQL snapshots | Complete-cell dialog/copy; source/result charts; SQL Notes and Ask AI historical values survive query close | Typed source cells; revision/result ownership; immutable bounded query snapshot/provenance; inspector never reruns SQL | Public Rust API, two-pane/query/Notes; actual AI draft database reload and 480×480 inspector |
| PDF | Original-container PDF.js text layer, cross-page search, outline, text/Shift-region selection, global zoom/flow | One-based original pages and normalized rectangles; saved page/zoom/flow; bounded canvas/text; lazy outline destinations | Real PDF component/public embedded; actual restore and complete shell |
| EPUB | Original archive/epub.js, CFI, chapter search/outline, flow/paper/font; whole-book progress opt-in | Exact CFI/content-keyed state; autosave verified before close; revocable archive-only resources | Component/progress/public embedded; actual autosave/reopen/selection/search/full shell |
| Static HTML | Rust safe projection; raw images/links use authorized providers; scripts/forms/styles/media inert | Granted original Source/Edit; prose precision explicit | HTML/Markdown profiles, reference policy and actual resource corpus |
| Images | Shared bounded raster/SVG/animation decode and compact fit/zoom/animation UI | Logical identity/revision; authorization before cached lookups; no repeated original-byte transfer on valid cache hits | Image/frame/host-image; actual project/shared/inline/revocation/replacement/image entries |
| Mermaid / PlantUML / D2 | Owned browser renderers; no backend Chromium or installed upstream app needed by Studio | Bounded revision-scoped input; cancelled workers/observers disposed | Diagram lifecycle/engine; actual three-engine large corpus |
| Notes / Comment / Favorite | One UI, category Clear/copy, durable storage provider, bounded Ask AI extension | Typed source/cell/query/PDF/EPUB anchors; stable file/WSL identity; owner/revision; literal comments preserved | Notes/translation/two-pane, durable SQL Notes/AI drafts, full-shell en/zh/en |
| Outline / lifecycle / settings | Virtual ancestor filter/collapse, keep-open chapters, cross-window keyboard traversal; scoped locale/palette/fonts; exact-origin v1 protocol | Generation/cancel guards, separate document IDs/revisions, bounded queues/disposal | 35-assertion outline, presentation/translation/search, two-pane lifecycle, delayed-handshake regression |

Provider profiles, limits, precision and migration are in
[embedding.md](embedding.md), [reader-api.md](reader-api.md),
[table-components.md](table-components.md), [jq-components.md](jq-components.md),
[markdown-references.md](markdown-references.md), [translation.md](translation.md).
Memory and whole-document query profiles declare their actual complete dataset;
no truncated prefix is represented as a whole-input query.

Ctrl/Command+F opens Preview search, including EPUB chapter focus; Source/Edit
use current-page CodeMirror search, with replacement only in Edit. Ctrl/Command+S
saves; SQL/jq Run uses Ctrl/Command+Enter. Escape closes the owning overlay;
active modals retain shortcut ownership. Embedded Ctrl/Command+O delegates to
the host. Shared contracts cover focus return, handled events, retained drafts/
queries and disposed subscriptions.

## Latest local validation

Ubuntu 26.04.1, Chromium 149 and mapped **native Wayland** WebKitGTK 2.52.6.
Studio passes 792 UI tests, generated/type checks, 152 application Rust tests
(one pre-existing ignored) and 14 original public SDK tests. Required upstream
desktop-feature Rust tests and the 41-module Web build pass. The new delayed-child
contract deliberately loses the first hello: original tail-call sender times
out; fixed sender passes eight assertions in both engines. Source/origin checks
are retained.

`smoke-reader-shell.mjs --webkit` runs the actual unmodified Studio app.
`smoke-standalone-reader.mjs CORPUS --webkit` reuses its generated Markdown,
123-row wide CSV, 12-page PDF and EPUB. Both test theme/control/overlay states;
Markdown tests Notes/Contents/search, SQL tests dialog bounds. Chromium keyboard
checks use physical events; native helper keyboard checks use synthetic DOM
events. Screenshots were inspected and remain private. Product shell pixels,
independent user font/paper preferences and a screen-reader audit are not claimed
to be identical or covered by these checks.

| Release corpus / engine | First-open host→content | Browser-cache reopen | Opening heartbeat maximum | Mounted nodes |
| --- | --- | --- | --- | --- |
| Studio / Chromium | 284–324 ms | 295–384 ms | 40 ms | 31–731 |
| Studio / Wayland WebKit | 330–539 ms | 331–429 ms | 98 ms | 31–731 |
| Standalone / Chromium | 98–259 ms | 87–245 ms | 55 ms | 32–731 |
| Standalone / Wayland WebKit | 231–1179 ms | 226–394 ms | 81 ms | 32–731 |

These are new application/browser contexts and owned backend caches, **not
cleared OS/GStreamer/font caches**. Studio worker RSS was about 41–43 MiB versus
standalone 8–10 MiB; different executable footprints matter. These are index-worker
peaks, excluding browser processes, not whole-application memory guarantees.

Release 1,941,749-byte Markdown and 1,155,181-byte three-diagram fixtures pass
reading, complete SQL, references, paged edit/save/undo/redo, conflicts, read-only
and symlink/token/origin enforcement, and cache cleanup. Indexing took 49/35 ms
at 53,092/49,944 KiB RSS; Chromium content took 201/194 ms, host content 324/289 ms,
heartbeat gaps 37/19 ms, with 741/21 initial nodes. Source/editor and completed
figures are separate later checks.

Dividing source size by these index times gives about 40/33 MB/s for these two
fixtures; this is a derived sample, not a sustained or cold-I/O throughput
benchmark. End-to-end wire byte counts, serialization CPU, browser/process-wide
peak memory and large-corpus sustained throughput are **unmeasured**. Source-byte
sizes, bounded transport windows and image-loader reuse counts do not substitute
for measured transfer cost or a whole-application memory bound.

With a **fresh private platform `XDG_CACHE_HOME`**, Wayland Markdown still took
1952 ms to content with an 1839 ms heartbeat gap. Same-cache editing took 198 ms /
90 ms gap and diagrams 47 ms / 34 ms gap. Earlier isolated experiments identified
a GStreamer registry cache dependency, not the exact blocking call. Font/GPU/media
toggles did not fix it. No prewarming/copied registry is presented as a product
fix. This remains a measured cold-start limitation; no absolute latency ceiling was agreed for this scene.

A separate bounded interaction follow-up first opens English Notes and drains
both realms' paint/timer work before switching locale. Its heartbeat maximum
remains 101 ms after opening, then rises to **433 ms during the first locale
update to Chinese through painting**, with Notes already open. It is unchanged
before/after the QA screenshot. `setLanguage` itself took 12 ms; temporary nested
focus/style/rectangle probes do not explain the complete pause and can perturb
timings. An unwrapped paint-drained sample also observed 398 ms. The cause remains
unattributed; it is not assigned to Contents, screenshot capture or a particular
synchronous call. Subsequent cumulative maxima cannot establish repeat-switch
latency. This is a separate first-use performance risk; regression against the frozen Studio baseline has not been established.

WSL's exact portable read-only adapter passes seven formats on Linux, including
reauthorization, stable identities and cleanup. This is **not actual Windows/WSL
execution**. WSL EPUB remains explicitly unsupported as in its previous transport.

Remote results identify the tested consuming commit: Studio `c058956` passed
Windows, macOS and Ubuntu in both push run `42029624553` and PR run `42029636607`.
These include the independent fixed SDK and Windows atomic replacement checks.
Astra accepted public core and embedding/components; #4 and #5 are closed. Final
technical candidate `f2ec4f8204a04fd4fc2598cab867e402bce0d666` passed all three
operating systems in push run `42037000119` and PR run `42037012109`. Windows ran
the production gateway on Ubuntu 24.04/WSL2, kernel
`6.18.54.1-microsoft-standard-WSL2`, user `reader-test`, UID 1000. Original
BOM/Unicode/CRLF bytes (88 bytes/SHA-256), exact Source, explicit identity,
read-only capabilities, cached image revocation, stable reopen identity, expired
lease and cache cleanup all passed. This real WSL2 case covers one Markdown with
project/shared images; it does not replace Linux's seven-format adapter evidence.

An earlier WSL1 probe failed a byte-count check. Studio requires WSL2; the extra
WSL1 observation is not evidence that the supported backend fails, nor a new WSL1
support obligation. No validation was weakened or SDK/native code changed for
this check. Private upstream CI still reports a GitHub billing/spending block;
public consumer CI did execute.

A bounded fresh-cache QA comparison measured native window launch to content at
3753 ms normally and 1936 ms with system GStreamer plugins disabled only in the
QA subprocess. Reader first-content/heartbeat were about 98 ms in both runs;
the changed startup cost occurred outside that reader timing window. First
Chinese Notes update still produced 382/383 ms gaps. JSC sampling found Fontconfig
work under a forced reading-position layout, but suppressing redundant theme
requests did not improve that gap. The speculative production change was removed.
Neither diagnostic is a product fix or a performance waiver.

Astra rechecked the original acceptance criteria: they require performance
measurement and explicit unknowns, but specify no absolute cold-start/locale
latency ceiling. The two observed delays remain risks, not newly invented merge
gates; no resource budget is relaxed. After the actual WSL2 pass, Astra independently
accepted #7's functional union/actual adoption and #6's fixed delivery/platform
compatibility. No blocking code/test defect remains. Studio [PR #55](https://github.com/messon007/codex-thread-studio/pull/55) was merged
as `eeb397ae7c6adcc2f713904f2be9d1b9cd6f0d1d`. Remote main contains the final
adoption commit `e5ce8ba301ee9fe323ebf1c90ca6e29ea435ffce`, with an identical source
tree. That documentation-only final commit also passed all six platform checks
in runs `42039044877` and `42039056520`. Parent closure records are in #2 and
Studio #48. The original Studio checkout remains untouched.
