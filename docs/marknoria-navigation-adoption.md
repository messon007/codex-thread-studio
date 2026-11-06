# Settled public table navigation — 2026-11-06

Studio adopts fixed source `0664d06853d9a3f1a0768b15794cd274cb97e9d1` from
main baseline `44fd6bf78b905cc80bd4497f96b5495bf9af9371`. Upstream main
`1e840b8f66f4e60ea83c94b1c63898173fdb6680` only adds acceptance evidence.
The runtime commit addresses [MarkNoria #30](https://github.com/messon007/marknoria/issues/30),
which Studio had reproduced in both `fbb5ff8` and `c8947e7`: navigation selected
the correct table cell, then a later layout frame moved it below the viewport.
The [syntax-profile adoption](marknoria-syntax-adoption.md) remains historical.

Two clean offline exports reproduce the official 850-file package:

- Archive SHA-256: `3da8ed777a6cf23e12bbb5268a350ef866f0b23e9d463b4c9e7b973c347d505d`
- Tree SHA-256: `dd899148cd2278c7c444d54aa3eea17d975edbc6b4268d7aa9cba189b2a638ef`

All nine delivery/inventory/tamper/profile checks pass. SDK sources and assets
are imported unchanged, with no private checkout, credentials or runtime needed
for ordinary builds/CI. No lock versions, host adapters, platform-native widgets,
FFI, native branch or CI workflow changes are required.

## Adopted behavior

Explicit Source/Edit-to-cell navigation suppresses the implicit Preview restore
jump and keeps the target virtual table mounted until the requested column is
restored and revealed. Synchronous virtualizer size notifications update mounted
transforms and the scroll sizer together. Table-height publication happens after
final retained-height calculation, before the host measures it.

Reveal waits for stable geometry with an eight-frame/250ms settle bound and two
commit frames; frame waits have a50ms fallback. It checks the actual scroll
viewport and table-content client bounds. Oversized cells align top/left. A
continuously unstable layout returns retryable `BUSY` instead of claiming a
clipped target succeeded. There is no persistent scroll-correction observer or
increase to document/row/node budgets.

The existing public navigation request now carries its AbortSignal. Revision,
owner, intent and navigation tickets remain checked across asynchronous work.
New navigation, explicit abort, user input and disposal cannot commit a stale
selection/reveal. Cancellation retains the public `CANCELLED` code. Temporary
listeners, timers and mounted-page pins are released. Ordinary Source→Preview
continues to restore reading position; only explicit cell targets suppress it.

Studio continues using the public protocol and existing host adapter. Reduced97
remains selected and exactly the same13 grammars stay excluded. Copy/Comment/
Translate, session-owned persistent marks, SQL provenance and bounded case-aware
search keep their existing ownership and authorization. Reader Favorite/Notes
remain absent; the removed session write-access menu does not return.

## Acceptance and delivery evidence

The package retains the strengthened embedded regression and adds a dedicated
public table-navigation contract: readonly/editable Source, hidden columns,
reader/host toolbar policy, actual client geometry at return and eight frames
plus200ms, newer requests, abort, stale revision, user input and disposal. An
ordinary-table contract covers late layout, oversized cells, continuously
unstable layout, ignored aborts and lifecycle. Browser probes exist only in test
files; production Studio never reads private reader DOM for navigation.

Studio validation uses the fixed package's Chromium/native Wayland WebKit
contracts plus its actual shell/search/comment regressions and an independent
original-failure reproduction with a500ms checkpoint. Existing UI/generated/type,
Rust integration and three-platform CI entry points remain in place. The final
adopting PR records executed counts/results and any limitations; upstream issue
closure alone is not accepted as downstream verification.

Final release is built after Cargo tests, because integration tests can regenerate
the same binary path. The exact frozen commit, binary SHA/size, independent
runtime results and reports are retained in `marknoria-navigation-build.json`
and `marknoria-navigation-validation/` beside the dedicated caches. The prior
production binary is preserved until the reviewed PR passes all six required CI
checks and the main-worktree owner activates the new version. Harness timings
are not isolated product latency or speedup measurements.
