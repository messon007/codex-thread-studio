# Temporary comment-click compatibility asset

Tracks [Studio #78](https://github.com/messon007/codex-thread-studio/issues/78) and
[MarkNoria #39](https://github.com/messon007/marknoria/issues/39). The pinned reader
paints comment anchors and supports a hover popup, but has no direct click handler.
Studio's existing revision/ID-checked `commentRequested` adapter already opens the
correct saved comment editor.

This urgent generic fix derives only `web/comment-marks.mjs`, in addition to the
separate [#38 layout correction](reader-layout-compatibility.md). It does not edit
the SDK inventory, reach into reader DOM from the Studio host, change comment
storage, or introduce a standalone Compare capability.

| Provenance | SHA / revision |
| --- | --- |
| SDK | `0a60f6951250e4ee50036c20e10e4283c4cd9079` |
| Original comment module | `69afa7035c008fbcb865f8712ae47a7f91c0ae8e2100eb5d210701e6249e9b8b` |
| Derived comment module | `5c6915f1e0c7da9b4622a3e5d3eb482d219c06cac899d9926fef45949a95ee83` |

`src-tauri/build/comment_compat.rs` checks the fixed source/revision, unique
replacement sites and derived hash. Build output stays in Cargo OUT_DIR. Both
existing authorized document assets and `/reader-assets` use the same exact
module; all other assets retain their existing routing and authorization. The
independent SDK remains unmodified. [The upstream diff](marknoria39-comment-click.patch)
is reversible; Rust tests check inverse/source rejection and actual asset routing.

The bounded existing painted hitboxes identify comments. An unmodified primary
click that starts and ends on the same mark opens the one matching saved ID.
Overlaps retain the existing bounded chooser. A 400 ms delay permits double-click
word selection; pointer movement while pressed, selection changes, scroll, blur,
cancellation, new pointer presses, mark replacement and disposal cancel pending
activation. Links, buttons, editable content, modified clicks and prevented events
retain their own behavior. Existing hover access remains available, including on
marked links. No text nodes are wrapped and no source references are recomputed.

Removal gate: adopt a verified upstream SDK implementing direct activation and
passing the retained pointer/persistence regressions. Remove the #39 generator,
helper, exact asset override, tests of its derivation, and this provenance/diff
together. Keep #38 independently until its own upstream adoption gate passes.
Do not silently carry this derivation to another SDK revision.

Regression command: `node scripts/smoke-reader-comments.mjs --webkit` with the
usual local Playwright/Chromium paths. It uses an isolated offline Codex fixture,
synthetic Markdown and real Studio storage/editor. Chromium uses actual browser
mouse input; native GTK WebKit uses DOM-dispatched pointer events, and verifies
edit/save/reopen through the full app. User documents and sessions are not used.

Local validation (2026-11-17): the original binary fails the direct-click editor
assertion; the normal derived build passes real Chromium click/edit/save/reopen,
double-click, drag, overlap, deletion, clear, Source and search checks. Native
Wayland WebKit 2.52.6 passes full-app click/edit/save/reopen and selection checks
with DOM-dispatched events. All 817 UI tests pass. Source bytes remain unchanged
and fixture-owned reader caches are cleaned.
