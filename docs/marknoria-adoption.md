# Fixed public reader adoption — local validation stage

The reviewed archive pin is `scripts/marknoria-release.json`; the full unchanged
source/resource inventory is `crates/marknoria/reader-manifest.json`. This update
uses public `marknoria::reader` / `marknoria::web` APIs, the official parent client
and the stateless SDK asset router. It removes embedding/query source patches
and orphaned reader bundle copies. Ordinary Markdown table layout now imports
from the complete SDK namespace, preserving its relative module dependencies.
No MarkNoria checkout, private token, active upstream assets or installed reader
binary is used to build or run this source snapshot.

`node scripts/vendor-marknoria.mjs --verify` checks the installed inventory.
To update, review a fixed source archive, its source/version/license inventory
and external SHA-256 pin, update the separate pin, and pass that local archive to
`node scripts/vendor-marknoria.mjs archive.tar.gz`. `--check` performs verification
without replacing the current source. The importer rejects altered or extra
files, wrong revision/tree/archive hashes, links, duplicate/traversal paths and
oversized inputs. Failure preserves the previous source. Imported code is never
patched or formatted. CI formats the application package explicitly; Cargo's
`fmt --all` also traverses path dependencies outside the workspace.

The ordinary Cargo test matrix directly compiles the SDK's original public
reader and Markdown contract fixtures. `src-tauri/tests/marknoria_contract.rs`
refers to these unchanged files instead of maintaining a second test copy.
The consuming workspace lock records serde_json 1.0.154 and sha2 0.11.1; these are
within the SDK's public compatible patch ranges. The SDK's standalone lock stays
unchanged. Third-party notices and first-party export MIT terms remain intact.

## Earlier complete-suite baseline on Ubuntu 26.04.1

- Complete Studio UI suite: 780 passed; TypeScript type checks passed.
- Complete Rust workspace: 149 application tests passed, one pre-existing ignored
  test, and 12 exact shared SDK contract tests passed.
- Strict import synthetic regression: 13 tests passed, including replacement and
  rollback failures; installed 791-file inventory matched the external pin. A
  failed rollback retains the original source in the reported staging directory.
- Actual Studio gateway + document workspace in Chromium, plus gateway-served
  reader frames in native Wayland WebKitGTK: generated 1,941,749-byte Markdown (24 columns, 120 rows) and
  1,155,181-byte Mermaid/PlantUML/D2 document passed reading, exact Source bytes,
  table paging/column selection with stable viewport, complete-table SQL count,
  outer typed source-reference capture with original byte units,
  Preview/Source/Edit draft preservation, save/undo/redo, external-change conflict,
  close/discard protection, retained frame, shared-directory read-only enforcement,
  symlink retarget rejection, authentication/origin checks and cache cleanup.
  Backend Chromium was deliberately unavailable. Wayland ran on an owned Weston
  socket with `GDK_BACKEND=wayland`; no X11/XWayland was used. No GTK critical or
  error appeared in this successful run.
- In that single warm local run, Rust indexing took 298 / 233 ms, with worker peak
  RSS 65,624 / 62,504 KiB. Host opening to first useful text took 540 / 367 ms;
  reader-internal content times were 92 / 192 ms. Measured main-thread heartbeat
  gaps were 21 / 20 ms at opening. Chromium mounted 739 / 15 nodes then; WebKit
  mounted 1345 / 1574 nodes at its later source-check point. These are distinct
  measurement points, not matched snapshots, cold-start guarantees or total
  application memory. The diagram document's first text is not SVG readiness;
  the test separately waited for all three rendered figures.
- The earlier source revision was exported twice offline: identical archive
  SHA-256 `136434e10b88314af6aad4fe407966a539aeb961b906c8c27f6a3c5aef58973f`.
  Export/inventory/tamper rejection contract: 8 checks passed, including byte-preserving Git staging with
  `core.autocrlf=true`. The package scopes `* -text` to its own `.gitattributes`.

## Public interface and latest local package

The current pin is MarkNoria `160b934e257a987e60530e68b5cdd82ebd0649ab`.
Two clean offline exports contain the same 798-file inventory and archive SHA-256
`f0a352c52059bbf65524312826f4064936ecbe248ec93e466deceb01962a3160`.
The eight export contracts include staged Git byte preservation. Studio imports
that exact archive without modifying any SDK source or generated resource.

The supported host surface is `marknoria::reader` / `marknoria::web::Service`,
`createReaderClient` v1, and the stateless `web::reader_assets()` component router.
The document proxy delegates static resource inventory to that router; document
API paths/methods, origins, tokens, project/shared grants and edit revalidation
remain explicitly constrained. New modules no longer require a second static
file list in Studio. No native UI or FFI participates in this integration.

Studio comments preserve typed public source/cell/projection/PDF/EPUB references,
revision and precision through its existing provider/storage model. Source UTF-8
bytes are never substituted into legacy DOM UTF-16 offsets. Reopening rejects
stale revisions; unsaved drafts and transient query IDs are not silently treated
as durable positions. A durable SQL snapshot/storage adapter remains open.

The former duplicate PDF renderer is now a thin adapter to the shared PDF
component. Studio still authorizes and transports original bytes. A synthetic
12-page fixture passes 12 adapter assertions in both Chromium 149 and native
Wayland WebKitGTK 2.52.6, with the adapter and assets served by the real Studio
gateway: outline, complete-document search, global zoom, continuous/paged and
rotated-page navigation, bounded canvases, typed text regions, stale-reference
rejection, abort and disposal. No gateway or AI credential enters that component.
This ordinary-host test is distinct from full application visual acceptance.

## Shared CSV / TSV / XLSX workspace stage

Ordinary CSV, TSV and XLSX opens now use the public document service and parent
client, including shared paging, resizing, SQL, complete-input charts and typed
cell selections. A declared binary source capability prevents offering Source or
Edit for workbooks. Restored Source views can return to Preview after readiness;
text tables support the same edit/save/undo flow. Ctrl/Command+O delegates to the
host. Denied access does not fall back to an alternate loader. The only retained
fallback is explicit HTTP 501 for unsupported WSL transport.

`scripts/smoke-shared-tables.mjs` generates its own fixtures and compares workbook
scalars against Studio's locked ExcelJS oracle. The real gateway/workspace passes
in Chromium 149; the same gateway-served reader frames pass in native Wayland
WebKitGTK 2.52.6. Each format has 123 records, leading zeros, quoted delimiters,
multiline values and literal HTML. Workbooks also cover two sheets, dates, boolean
values and cached/uncached formulas. Complete-input SQL and charts reach the final
record; cell references retain original row/column identity. Text edit/save/undo
round trips restore original bytes, and session close removes owned caches.
Authentication, foreign origin and outside-project access are checked separately.

One warm local debug run measured CSV / TSV / XLSX indexing at 2.0 / 2.5 / 17.2 ms
and index-worker peak RSS at 55,216 / 55,520 / 60,872 KiB. Workspace opening to
first actual table rows took 564 / 305 / 325 ms; reader first useful content took
42 / 31 / 189 ms. Opening heartbeat gaps were 29 / 20 / 24 ms, measured before
queries/editing. These small synthetic runs are not release/cold-start or large
input guarantees. Direct Wayland frame coverage is not a full Studio shell test.

The registry owns one reusable maintenance task rather than spawning a timer for
each rapid reopen. Disposal during an outstanding settings/mode request cannot
start a late heartbeat. Lifecycle regression tests cover both conditions.
The public archive's normalized tree SHA-256 is
`45855b3ed4125ac751ac4329f3d5ebdc9a892e1b7ce0e60cd88aaea5682e257b`.

## Shared EPUB workspace stage

Ordinary EPUB opens now use the same public service/client as shared tables.
The previous `fnv1a64` content key and Studio's SQLite reading-state records remain
compatible. Rust computes that key with a fixed 64 KiB buffer under the existing
single-open permit; the UI no longer receives the whole original ZIP. Shared Rust
archive/resource validation and bounded chapter loading remain authoritative.
Source/Edit are unavailable, and no parent-directory grant is inferred.

The host restores paper/flow/font settings and saved CFI through public commands.
It reveals the frame after initial navigation, preserving layout dimensions while
avoiding a first-chapter flash. Typed initial references undergo the same revision,
precision and location checks as subsequent navigation. Disposal during settings,
mode or location restoration cannot install a late heartbeat.

Existing EPUB comments keep their provider, e-book-specific AI instruction,
legacy book key/CFI and reopening workflow. They additionally preserve exact
public revision/location metadata; legacy references still reopen under their
original content key. Failed or superseded reading-state loads release their new
reader lease. Closing flushes the current reading state through Studio's existing
storage route before releasing the service.

`scripts/smoke-shared-epub.mjs` runs the real gateway/document workspace against
the fixed package's generated book. Chromium verifies opening without the legacy
original-byte endpoint, prior content-key compatibility, chapter outline, complete
book search, paginated/scrolled settings, a physical text selection with exact CFI,
Studio comment-provider capture/reopening, progress index/seek, persistence across
new sessions, delegated Open, resource sandboxing, original-byte preservation and
cache cleanup. Native Wayland WebKitGTK reader frames also pass outline/search,
CFI/settings, progress/seek and cleanup. This direct frame coverage does not claim
full Studio-shell accessibility or visual acceptance in WebKit.

One warm debug run measured indexing at 0.9 ms, index-worker peak RSS 57,896 KiB,
reader first useful content at 230 ms and host first book content at 724 ms. The
opening heartbeat gap was 67 ms, so this measurement does not establish a 50 ms
responsiveness target. It is a small synthetic run, not release/cold-start or a
large-book performance conclusion. The exact 791-file SDK inventory remains
unchanged; this stage needs no upstream source/resource patch.

The common-host CSV/TSV/XLSX and large Markdown/diagram workflows also pass after
the initialization change, including original Source/edit/save, typed references,
grants and cleanup. One concurrently launched Wayland WebKit process exited with
SIGSEGV before producing its Markdown check result; the isolated sequential rerun
completed successfully. This records the observed failure and rerun, rather than
claiming that the environmental/process instability has been diagnosed or fixed.

## Host-owned shared document Notes stage

Studio explicitly configures the existing public Rust Notes store under its own
application state directory. File read-only permission is preserved; Notes storage
is separate authority. The default library/service profile remains Notes-disabled.
No data, gateway credential or AI account is added to the SDK package.

The public `settings({notes:'host'})` presentation capability disposes the child
Notes UI without revoking granted storage. The parent mounts the unchanged shared
`mountNotes` component in an explicit document-toolbar slot with Studio's palette
and typography. Each binding captures its document ID/revision and transport;
remounting, pending module imports, changed revisions and disposal cannot redirect
an old request/action to a new active file. Selection bounds are mapped from the
frame to the host viewport. Copy/Comment/Favorite use the shared component; its
additional Ask AI action opens Studio's existing comment workflow. Chat/session
comments and favorites remain Studio-owned; this new store holds document Notes.

The real Chromium gateway/workspace checks a selected CSV cell's comment/favorite,
a complete-input SQL aggregate favorite, SQL-result closure with durable query
provenance, copying its snapshot label, comment-only clearing that retains both
favorites, and persistence across edit/save/undo and a new reader session. TSV
and XLSX demonstrate separate document stores; their ordinary table/AI actions
still run through the shared host. Native Wayland WebKitGTK reader frames verify
explicit Notes capabilities, host ownership and durable SQL-result Notes through
the actual scoped Rust service. This is not full Studio-shell WebKit acceptance.

The Notes library's bounds fit the actual Studio viewport; a generated screenshot
was inspected locally and remains outside version control. The toolbar entrance
uses a dedicated host slot instead of inheriting browser-default button styling.
The consuming suite passes 780 UI tests, type/format checks, 149 application Rust
tests (one existing ignored) and 12 unchanged public SDK contract tests. The
imported 791-file inventory remains unchanged after building. Two clean offline
exports match; eight archive/inventory/Git-byte checks pass.

The cold-start workflow exposed a shared SQL input race: edits were possible
before schema/default SQL arrived and could then be overwritten. The upstream
input now remains disabled until schema initialization, with 27 component checks
passing in each engine. Host controls likewise remain unready until initial
settings/location restoration finishes; a delayed-location regression covers this
ordering. EPUB and large Markdown/diagram workflows pass after these changes.
Ordinary PDF/image/small-Markdown Notes entry points, durable SQL references for
AI drafts, remaining locale/defaults and full common acceptance remain open.

## Remaining acceptance

This is not a completed #48 adoption or #7 feature-union acceptance. Small
Markdown still uses the existing ordinary document renderer; image entry points
have not migrated. WSL tables retain an explicit unsupported-transport fallback
and still need shared byte transport. The shared PDF adapter still needs full
application visual/accessibility acceptance. Remaining work includes authorized Markdown
image/link resources, remaining typed selection/Notes/AI adapter integration across ordinary entry points, remaining
locale/default-state behavior, common corpus/visual/accessibility/performance
comparison, and actual downstream three-platform CI and review/merge. Native
Wayland WebKitGTK also passed original-source display, Mermaid/PlantUML/D2,
draft-preserving view changes, save and external-byte verification. It ran with
`GDK_BACKEND=wayland` on an owned Weston socket; no X11/XWayland was used. No current
remote CI or macOS/Windows execution is claimed from local compilation.

The original Studio checkout is untouched. These integration changes are being
prepared and committed in an isolated checkout. Remote pushes are deferred until
the complete change set and local validation are ready, at the owner's request.


## Current resource adoption batch

The fixed SDK above and the application resource wiring form one adoption batch.
The reviewed tree SHA-256 is
`1bcb291c74e370b33ced8bae160924914337a94bfe710519d7caafe0b7f0f7ff`.
The unchanged inventory verifies after the application build. No SDK patch or
private checkout/runtime is used. The upstream duplicate-export contract passes
eight checks; it is not regenerated for host-only edits.

Each service receives its already authorized project root and a narrow trusted
provider over Studio's current shared-directory configuration. Both old loaders
and the provider use the same canonical-directory resolver. Resource authority
is checked before image-cache hits, so new grants/revocations apply to new reads
without closing the document or discarding a draft. A shared-root change does not
remove the project's independent edit grant. The 257-root profile accommodates
one project plus the existing 256 shared-directory setting, after deduplication.
The gateway adds only the two declared reference GET routes; tokens, origins,
method restrictions and ordinary file-open authorization remain intact.

`linkRequested` is checked against the bound document/revision and bounded target.
The workspace delegates local targets to its existing authorized open flow and
external targets to its existing browser action. Stale, malformed, oversized and
after-dispose events do not enter host actions; callback failures are reported.
No child DOM or experimental global is used by this production adapter.

Executed against the actual application binary on Ubuntu 26.04.1:

- Six affected Rust gateway tests and 44 host/workspace UI tests pass, as do UI
  build/type checks and the unmodified 798-file inventory verification.
- `scripts/smoke-reader-resources.mjs --webkit` runs the actual gateway and
  document workspace. Chromium 149 displays project/inline/shared images,
  rejects missing token/foreign Origin/wrong method, adds and revokes a shared
  grant, rejects cached metadata/frames after revocation, preserves/saves the
  dirty draft, opens a relative CSV target and delegates an external link.
- Gateway-served reader frames in native Wayland WebKitGTK 2.52.6 verify all three
  resource images, PNG Blob transport, exact link identity and disposal. This is
  reader-frame coverage, not full Studio-shell visual/accessibility acceptance.
  No X11/XWayland is forced. Backend Chromium is intentionally unavailable.
- App-requested closure removes all owned reader caches. Only the explicit editor
  fixture changes; generated documents/caches/screenshots stay untracked.

Before this wiring, the preceding fixed package passed 780 UI tests, type checks,
149 application Rust tests (one pre-existing ignored) and 13 exact SDK contracts.
This follow-up uses the targeted tests above rather than claiming those complete
suites were rerun after every host edit. The public core/runtime and their shared
fixtures did not change in the dynamic-resource follow-up.

Small Markdown/raw-HTML compatibility, original-image profiles, remaining common
entry points/Notes, durable AI draft references, WSL transport and the final
cross-product/remote gates remain open. New commits remain local until the whole
task is ready for the owner's requested combined push.
