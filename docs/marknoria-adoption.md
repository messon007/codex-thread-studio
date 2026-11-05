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

- Complete Studio UI suite: 750 passed; TypeScript type checks passed.
- Complete Rust workspace: 146 application tests passed, one pre-existing ignored
  test, and 10 exact shared SDK contract tests passed.
- Strict import synthetic regression: 13 tests passed, including replacement and
  rollback failures; installed 780-file inventory matched the external pin. A
  failed rollback retains the original source in the reported staging directory.
- Actual Studio gateway + document workspace in Chromium and native Wayland
  WebKitGTK: generated 1,941,749-byte Markdown (24 columns, 120 rows) and
  1,155,181-byte Mermaid/PlantUML/D2 document passed reading, exact Source bytes,
  table paging/column selection with stable viewport, complete-table SQL count,
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
- The current source revision was exported twice offline: identical archive
  SHA-256 `136434e10b88314af6aad4fe407966a539aeb961b906c8c27f6a3c5aef58973f`.
  Export/inventory/tamper rejection contract: 8 checks passed, including byte-preserving Git staging with
  `core.autocrlf=true`. The package scopes `* -text` to its own `.gitattributes`.

## Remaining acceptance

This is not a completed #48 adoption or #7 feature-union acceptance. Small
Markdown still uses the existing ordinary document renderer; PDF/EPUB/table/image
entry points have not all migrated. Remaining work includes authorized Markdown
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
