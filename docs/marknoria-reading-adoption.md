# Reading controls delivery (2026-11-09)

Studio adopts MarkNoria `64765f26d9c56d16b7970e632cde0b56d34e66fd` from
Studio `a73296618252138b8b3a91a86ba8e012e050d873`. Version remains 0.4.0;
this is an Unreleased integration, not another release.

## Fixed delivery and review

Two clean offline exports of this immutable revision pass all nine delivery
checks and produce 853 files with identical archives and trees:

- Archive SHA-256: `9b1b7548af630ba8e210be3848759f1cd32b36cf2f9599fdb473ee4ab71b8c03`.
- Tree SHA-256: `ee50831c39bba63a10c38141343f3184496a7b1397dabd567631de282b9f724a`.

The upstream log since `0664d06` contains the table-navigation acceptance,
removal of machine-specific documentation, a documentation reorganization,
`d271370` (absolute line navigation, nearby word matches and standalone comparison),
`f6d02cd` (remove duplicate line navigation from reading settings), and `64765f2`
(text/code sizes with stable layout, anchors and drafts). The SDK is imported
unchanged, including MIT and third-party licensing. No Rust/npm dependency or
lockfile changes are needed. Public API/bridge version remains 1. Ordinary builds
and public CI need neither private credentials nor an upstream checkout.

## Adopted behavior and host settings

- The shared reader handles Ctrl+G while its Preview, Source or Edit surface has
  focus. It navigates original source lines, validates document/view ownership,
  and preserves dirty editor buffers. Studio does not inspect private reader DOM
  or add a second navigation implementation.
- Studio Settings → Typography → Document reading exposes the public
  `highlightSelectionMatches` setting. It defaults off, is persisted by Studio,
  and overrides any standalone reader preference. Matching remains the upstream
  bounded, case-sensitive whole-word operation; PDF, images, EPUB and editing
  do not gain unsupported matching modes.
- Reading-text and technical-code sizes accept 11–32 px. Existing Studio defaults
  and saved values remain unchanged (content 15, code 14). “Use automatic code
  size in documents” sends public `codeFontSize: null`, without changing the size
  of other technical output. Both reading toggles persist across restart/reopen;
  resetting settings turns them off.
- Presentation updates are explicit on settings changes, as well as theme and
  locale changes. The shared implementation preserves reading anchors and dirty
  source/editor state while remeasuring text, code and table layouts. Host and
  child controls retain live English/Chinese translation.
- The 97-grammar reduced profile and its 13 exclusions, original-byte anchors,
  persistent Studio comment marks, bounded search, grants, query provenance,
  and PR #65 lease/foreground/dirty-refresh protections remain in place.
  Copy/Comment/Translate remain Studio-owned; no reader Favorite/Notes or obsolete
  Release session write access menu is restored.

## Remaining shared capability

This source includes improved standalone comparison split sizing and complete
truncated-source-line actions, but the packaged single-document `web::Service`
explicitly has no comparison routes. `compare_runtime::Comparisons` is private to
the standalone application. These improvements are **not exposed in Studio**.
[MarkNoria #35](https://github.com/messon007/marknoria/issues/35) tracks a public
comparison delivery with two independently authorized inputs, lifecycle,
host toolbar/actions, resource budgets and Chromium/WebKit fixtures. Studio will
adopt that verified interface; it does not open arbitrary paths or depend on the
standalone application's private DOM/runtime.

## Verification and limits

The fixed SDK inventory, generated UI and TypeScript checks pass. UI tests cover
legacy preference migration, persistence, resetting and the expanded size bounds;
the Rust preference test performs a file roundtrip and rejects invalid bounds.
Both Chromium and mapped native Wayland WebKit pass the shared word-highlighting
(73 checks each) and public-presentation (28 each) contracts. The SDK retains its
independent locked workspace/all-target checks and the outer Studio integration
suite checks the actual reduced-profile dependency.

The shared Ctrl+G/typography contract passes 51 Markdown checks in both engines
and seven compact-JSON checks in Chromium. Four-format Studio shell checks pass
in Chromium and native WebKit; Chromium additionally verifies the host settings,
11/32 px text/code roles, source Ctrl+G with unchanged authority, boolean-only
updates and reopen persistence. The actual lease-expiry smoke passes all four
checks. Independent Studio review verifies default-off overriding old standalone
localStorage, dirty Edit Ctrl+G, 32 px/Auto/highlights, rejected refresh, unchanged
source bytes and owned cache cleanup. Public search contracts pass 69 checks per
engine, including Unicode/case, bounded windows and renderer limits.

Local Rust results: Studio 152 tests (one pre-existing ignored) plus 15 integration
tests; the independent fixed SDK workspace/all-targets passes. All 807 UI tests,
generated checks, type checking and application Rust formatting pass. The adopting
PR records the final six Linux/macOS/Windows push and PR CI checks and release
binary provenance. Runtime fixtures use isolated configuration/data/cache paths
and disabled or explicitly fake AI backends. Component checks are distinguished
from actual Studio shell checks; synthetic fixtures are not a performance
benchmark or a complete accessibility audit. The previous `0664d06` navigation
report remains historical in [marknoria-navigation-adoption.md](marknoria-navigation-adoption.md).
