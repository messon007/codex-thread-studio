# Temporary MarkNoria #38 compatibility asset

This narrowly scoped shared-reader correction uses the urgent-fix exception in
AGENTS.md. Its upstream issue is
[MarkNoria #38](https://github.com/messon007/marknoria/issues/38). Remove the entire
layer when a fixed public SDK passes the same geometry/typography/anchor checks.
Standalone Compare remains excluded and is not affected.

## Exact provenance

The imported SDK directory and all 856 manifest entries remain unchanged.
The normal upstream asset getter and independently built SDK server still serve
the original reader. Studio derives **one** reader asset at build time:

| Item | Value |
| --- | --- |
| Fixed SDK revision | `0a60f6951250e4ee50036c20e10e4283c4cd9079` |
| Original `web/reader.mjs` SHA-256 | `5a3ce895a3d7e0b01ded2d0883b686b602c39bc33ce742b3b20de4978c099d86` |
| Derived `reader.mjs` SHA-256 | `4f35a6ea75c7d0bf13c8805d1f42ac75cc872edc0638a5f91504f27d03b3392a` |

`src-tauri/build/reader_compat.rs` checks the fixed pin, original hash, unique
replacement locations and exact derived hash. Any future SDK or helper change
fails the build until deliberately reviewed. The output and provenance JSON live
in Cargo's `OUT_DIR`; there is no generated source in the SDK inventory, runtime
rewrite, network fetch or general patch loading pipeline. Existing `sha2` is
reused, with no dependency/lock changes. The helper normalizes its own checkout
line endings; the immutable SDK bytes are verified without normalization.

The [reviewable diff](marknoria38-reader-compat.patch) applies forward to the exact
original and reverses to its original bytes. Rust tests exercise the actual
build derivation, exact inverse, source/revision rejection, and stateless routes.
Both `/reader-assets/reader.mjs` and the existing authorized
`/markdown-reader/{id}/reader.mjs` path serve the same derived asset. Lease,
origin, path and method checks occur before document-scoped asset delivery.
Every other asset falls through to the unmodified public SDK router. Independent
SDK tests and standalone results refer to the original, not the derived asset.

## Why the correction is needed

The upstream typography and wrap/column-layout paths call `virtualizer.measure()`.
This clears its measured size cache without resizing already-mounted DOM. A
ResizeObserver need not fire when a code-only setting leaves prose/table geometry
unchanged. Moreover, immediately calling `measureElement` after clearing is
insufficient: virtualizer measurements can still contain the old actual sizes,
so `resizeItem` observes a zero delta and leaves the size cache empty; later layout
uses estimates instead. Normal measuring may also be suppressed while scrolling.

The helper clears the cache, forces the virtualizer to recompute its current
items, then measures only connected, ready, non-placeholder entries in the
already-mounted `nodes` map. It uses the virtualizer's existing configured
measurement function and `resizeItem`, and commits transforms/sizer together.
It does not traverse the full document, expand the mount window, alter parser
budgets, replace source anchors, or access host/AI state. The two call sites are
`refreshTypography` and `changeLayout`. New or loading pages retain their ordinary
measurement lifecycle.

The production Studio host remains a public-protocol adapter. Its separate
presentation delta/serialization correction prevents unnecessary invalidations;
this shared-reader compatibility asset handles actual layout changes. The host
does not access the child's private globals or DOM.

## Removal gate and regression

`scripts/smoke-reader-layout.mjs` uses synthetic prose/tables and the unmodified
Studio app, checks physical adjacency within one pixel and bounded DOM through
initial load, scrolling, theme/locale, Auto and code-only changes, actual font
changes, window resizing and document reopen. It also verifies identical assets
at both paths, exact source preservation and owned-cache cleanup. Tests inspect
rendered DOM only for QA; they are not production coupling.

Before removing the layer, run this smoke with the newly fixed **unmodified** SDK,
plus source/dirty editor navigation and existing table/comment/selection fixtures
in Chromium and native WebKit as applicable. Remove the build generator, helper,
asset override, associated provenance/diff and fixed-hash tests together. Do not
silently repin this workaround onto another upstream version. The adopting PR
records executed geometry and independent actual-document results; private
source text/screenshots are not published.

The final local standard build passes 60 strict samples per engine in Chromium
and native Wayland WebKit, plus independent actual-document checks. The original
upstream asset remains a failing negative control for the code-only typography
case; these results validate Studio’s derived asset, not an upstream fix release.
