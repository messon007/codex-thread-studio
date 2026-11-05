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

## Executed locally on Ubuntu 26.04.1

- Complete Studio UI suite: 769 passed; TypeScript type checks passed.
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

The current pin is MarkNoria `a9cf902ded351b518222aaf74c1885864f703d78`.
Two clean offline exports contain the same 791-file inventory and archive SHA-256
`8cc605ddcd795e6be8aa0f9fceb0d38d2719a69fca9f1b8dc4a94910009e6cac`.
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
`210f8f4595ace9c9abec8c715bf5e99e2f32217af7f24f16df6870f486bb3452`.

## Remaining acceptance

This is not a completed #48 adoption or #7 feature-union acceptance. Small
Markdown still uses the existing ordinary document renderer; EPUB/image entry
points have not migrated. WSL tables retain an explicit unsupported-transport
fallback and still need shared byte transport. The shared PDF adapter still needs full
application visual/accessibility acceptance. Remaining work includes authorized Markdown
image/link resources, full typed selection/Notes/AI adapter integration, remaining
locale/default-state behavior, common corpus/visual/accessibility/performance
comparison, and actual downstream three-platform CI and review/merge. Native
Wayland WebKitGTK also passed original-source display, Mermaid/PlantUML/D2,
draft-preserving view changes, save and external-byte verification. It ran with
`GDK_BACKEND=wayland` on an owned Weston socket; no X11/XWayland was used. No current
remote CI or macOS/Windows execution is claimed from local compilation.

The original Studio checkout is untouched. These integration changes are being
prepared and committed in an isolated checkout. Remote pushes are deferred until
the complete change set and local validation are ready, at the owner's request.
