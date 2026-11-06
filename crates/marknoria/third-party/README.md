# Reused source

The user authorized reuse from Codex Thread Studio (MIT), revision `37028200f111d210c4b1d681087c6f4b9baaa2c9`.

- `src/core/documents/structured.rs`: bounded structured-text formatting and exact token mappings; the text snapshot adapter uses MarkNoria storage.
- `src/core/documents/svg_resources.rs`: bounded SVG embedded bitmap resource resolution.
- `src/core/documents/image_decode.rs` and `image_animation.rs`: bounded oriented image/animation decoding, adapted to MarkNoria’s private PNG cache.
- `web/styles.css` selection-action styling and `native/linux/src/annotations.css`: compact selection-popover treatment adapted from the same MIT UI. The Notes list/detail arrangement follows its comment library, implemented locally with MarkNoria's note records and copy-only actions.

The license is retained in `codex-thread-studio-MIT.txt`. These are local source copies; builds and runtime do not access the upstream checkout.

`tree-sitter-highlight` and the pinned static grammar/query crates now supply
the production highlighter; see [highlighting.md](highlighting.md) for notices
and exact upstream revisions. The historical performance baseline uses
`two-face` 0.5.2+bat-0.26.1 and `syntect`, including TOML. Embedded grammar
licenses and credits are retained in [syntax-acknowledgements.md](syntax-acknowledgements.md);
the bundle’s own MIT notice is retained in [two-face-MIT.txt](two-face-MIT.txt).
The web editor uses MIT-licensed CodeMirror packages, including its TOML mode;
its license is copied into `web/vendor/` during builds.

Additional Web table measurement, cleanup and fill-width fixes, and Windows
save-handle release/error context, are adapted from the committed Studio snapshot
`a2f87a37d5a3d0de7c0dbe0f0f9666e0b13c3a4a` (`crates/marknoria/web/table-layout.mjs`
and `scripts/marknoria-embedding.mjs`), under the retained Studio MIT license.
MarkNoria additionally bounds measurement traversal before copying cell content.

The pinned `@d2lang/d2` 0.1.34 browser resource (MPL-2.0) is modified during
`scripts/build.mjs` by the checked adapter in `scripts/d2-resource.mjs`.
It accepts an owned worker factory and terminates immediately on disposal,
including cancellation before WASM initialization completes. The original MPL
license and third-party notices are copied beside the resource. The adapter
fails closed when the pinned source changes; it does not edit installed packages.

The Web PDF reader uses `pdfjs-dist` 6.4.299 (Apache-2.0), including its official
text layer and viewer CSS. `scripts/pdf-assets.mjs` copies the locked worker,
CMaps, standard fonts and image/color WASM resources with their bundled licenses
under `web/vendor/pdf/`. The generated Rust resource map serves only those exact
files. PDF scripting/QuickJS resources are excluded. The local layout/controller
adds bounded continuous/paged rendering and the shared selection/position API;
no Studio checkout, generated bundle or runtime is needed.

The shared `web/epub-reader.mjs` and `web/epub-reader.css` adapt the committed
Studio `ui/epub-reader.mjs` and EPUB stylesheet section at
`a2f87a37d5a3d0de7c0dbe0f0f9666e0b13c3a4a`, under the retained Studio MIT
license. MarkNoria adds scoped lazy archive resources, exact CFI range handling,
public lifecycle/selection/search APIs, bounded search and sandbox restrictions.
The build bundles pinned `epubjs` 0.3.93 locally and copies its BSD-2-Clause
license beside `web/vendor/epub.mjs`. It never reads Studio build artifacts.

`web/table-cells.mjs` adapts the cell-selection, full-cell dialog, copy and
keyboard interaction behavior from the same frozen Studio `ui/table-reader.mjs`
(MIT). It shares the upstream table layout and does not copy Studio's JavaScript
CSV/XLSX parsers or its truncated first-100-row chart data path. Rust retains
complete dataset parsing/query ownership.

`web/table-plot.mjs` adapts the SVG bar-chart layout from Studio's
`ui/table-reader.mjs::toggleChart` at the same frozen revision (MIT). Its input
comes from a new local Rust complete-input projection, replacing the old
first-100-row path. Signed/null handling, budgets, original coordinates,
cancellation and scoped component lifecycle are maintained locally.

The shared Rust Markdown extension uses `linkify` 0.11.0 under MIT OR
Apache-2.0, pinned by Cargo.lock. Its source and license are supplied by the
public Cargo package; no code or assets are read from another checkout. See
[the shared Markdown profile](../docs/markdown-profile.md) for scanner scope,
source-anchor behavior and the frozen Studio comparison.


The fixed Web export retains installed locked npm LICENSE/COPYING/NOTICE and
README files plus declared license/author/repository/integrity metadata under
`licenses/`. This includes packages whose complete notice is only in README
(for example isarray 1.0.0), and khroma 2.1.0's original `license` even though its
package metadata omits a license field. No license is inferred from a missing
metadata field. Some platform build-tool packages provide only a README pointing
to their parent package's retained license; their binaries are not exported.

`marks-pane` 1.0.9 and `path-webpack` 0.0.3 omit a standalone license file in their
published npm tarballs. Their original MIT declarations and author metadata are
retained in `marks-pane-1.0.9-package.json` and `path-webpack-0.0.3-package.json`,
from the exact published source commits:

- https://github.com/fchasen/marks/blob/2f5bb3ccfb237a2575e12c4db9188651a0b21d9a/package.json
- https://github.com/fchasen/path-webpack/blob/a8e7bf91e3646e3265c69fe4e8d414aad41b3e30/package.json

path-webpack's own README identifies Node v7.0.0's path module as its source. The
original Node/Node-contributor/Joyent notices are retained verbatim in
`path-webpack-Node-v7.0.0-LICENSE.txt`, from
https://github.com/nodejs/node/blob/v7.0.0/LICENSE
(SHA-256 `1914bfb950be8d576ce9e49c8a0e51c9f2402560fe3c19093e69bc1306a56e9e`).
These third-party declarations/notices are not replaced by MarkNoria's first-party
MIT delivery license.


The bounded Markdown raw-HTML projector directly uses the already locked
`html5ever` 0.26.0 tokenizer (MIT OR Apache-2.0); its registry source/license was
already part of kuchikiki's dependency graph. The added direct declaration does
not copy another checkout or add a new package version. The existing source
allowlist includes the local adapter, and the Web-only Cargo template retains the
same exact dependency/options.

`web/outline.mjs` adapts the hierarchy and ancestor-preserving filter semantics
from Studio `ui-src/document-outline.mts` at frozen revision
`a2f87a37d5a3d0de7c0dbe0f0f9666e0b13c3a4a` (MIT, copyright 2026 messon007).
It consumes existing indexed metadata, adds cancellable linear filtering, and
uses viewport virtualization instead of copying the old 2,000-heading limit.
The original MIT notice remains in `codex-thread-studio-MIT.txt`.

The shared Source/Edit find panel reuses MIT-licensed `@codemirror/search`
**6.7.2**, matching the fixed Studio dependency. The lockfile records its exact
package/integrity; public exports retain its installed npm license and notice.
No Studio editor wrapper or second search implementation is required.

`web/comment-marks.mjs` follows Studio’s MIT dashed comment-anchor visual
treatment from the same pinned source above. Its scoped selectors, non-mutating
CSS Highlight/geometry painter and bounded persistence integration are local
MarkNoria implementations; there is no Studio runtime or build dependency.
