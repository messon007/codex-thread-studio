# Bounded Markdown table delivery (2026-11-12)

Studio's fixed import advances from `64765f26` to
`0a60f6951250e4ee50036c20e10e4283c4cd9079`, on Studio main
`6a354725b3dea704abf6b5be302837f5282b42ad`. This is an Unreleased main update;
version stays 0.4.0. It addresses [MarkNoria #37](https://github.com/messon007/marknoria/issues/37).

## Provenance and boundaries

The upstream range contains one runtime commit: bounded Markdown table indexing
and complete-source access. Two clean offline exports match the published archive
and pass all nine delivery checks. The unchanged SDK inventory has 856 files:

- Archive SHA-256: `08ed97c8a6b81266f4fcdbb9f624ca4882daceb05d9451ac78e3f9e3b8bf7495`.
- Tree SHA-256: `4b0d89689d46b71f6dcbb7b9c0b2b846e181fb73e41e7b2a6292c8c920a9d456`.
- API and bridge versions remain 1; original MIT/third-party notices are retained.

No dependency or lock updates, private build credentials, platform-native UI/FFI,
or SDK source modifications are introduced. A single explicit build-derived
compatibility asset for #38 is described below; the fixed inventory stays original. The reduced 97-grammar profile and 13 excluded
languages remain unchanged. Studio keeps Copy/Comment/Translate, persistent comment
marks, original-byte references, workspace grants and the PR #65 lease/dirty-refresh
protections. Reader Favorite/Notes and the obsolete release-session action remain
absent. Standalone Compare is explicitly excluded at the user's request; Git
Review/diff remains in scope. Compare is not a parity gap or follow-up API request.

## Adopted table behavior

Rust now streams table rows and separates complete input from HTML projections.
Pages flush before cumulative event/serialized-HTML budgets are exceeded. Dense
inline markup may become complete plain text; oversized values have explicitly
abbreviated previews. SQL/charts consume full data and retain the existing 128-KiB
record limit: over-budget complete records produce an error rather than querying
a shortened prefix. No source/page/event/cache budget was increased.

“Copy preview” and “View original row” distinguish abbreviated values. The latter
uses the existing revision-bound `sourceWindow` command, displaying at most
128 KiB at a time with replacement paging. Its raw syntax never publishes a
rendered-cell comment anchor. Ordinary Source view remains the path for exact
original-text comments. Source search covers omitted text and navigates such a hit
to Source, including hidden columns. BOM/CRLF and UTF-8 positions are retained.
Oversized tables nested in a list/quote use an explicit complete source-only
preview without inventing table identities or SQL input. Failed rich attempts
remove only their owned cache before fallback.

Studio supplies Chinese translations for all 17 new public catalog labels and
retains live locale changes in an already-open original-row dialog. No new file
route, bridge method, authorization or private child-DOM adapter is needed.

## Layout correction and remaining upstream defect

Studio formerly resent all presentation fields after initialization and on
locale/theme changes. The shared reader treats the presence of a typography field
as a request to clear measured page heights, even if that field is unchanged.
This can replace measured heights with estimates without a corresponding DOM
resize, leaving blank space or overlapping the next projection.

The host now compares initialization with public presentation state and sends
only changed settings thereafter. Updates are serialized per iframe; only a
successful acknowledgement advances the comparison state, failed values can be
retried, and disposal cancels pending work. EPUB settings retain partial-field
semantics. The adapter uses the public protocol, not private virtualizer state.
An independent real-document A/B isolated the repeated-setting trigger and
verified correct adjacency after removing it. Private documents and screenshots
are not included in the repository or public PR.

The initial host-only candidate still failed the synthetic layout regression:
about 59 px of blank space and 207 px of overlap when an actual initial font
setting reset unchanged prose/table measurements. This blocked its delivery.
The failure is retained as the before-case for
[MarkNoria #38](https://github.com/messon007/marknoria/issues/38).

Under the documented urgent shared-fix exception, Studio now derives one
strictly pinned reader asset during its build, restoring the measurements of
already-mounted pages after layout invalidation. The original SDK remains
byte-identical and independently buildable. Both Studio asset routes use the
same derived asset. See [the compatibility record](reader-layout-compatibility.md)
for exact input/output hashes, the reversible diff, bounded work, public router
boundary and mandatory removal gate when the upstream fix is adopted.

## Executed verification

- Two fixed offline exports and nine delivery checks; exact installed inventory.
- Generated/type checks and 810 UI tests, including presentation acknowledgement,
  retry, disposal, unchanged-font suppression and source-window label interpolation.
- Studio Rust: 154 passed, one existing ignored; 15 integration tests passed.
  Independent fixed SDK locked workspace/all-targets passed.
- Updated table cells: 52 checks in Chromium and 52 in native Wayland WebKit;
  table view: 41 in each engine, including source-window bounds and lifecycle.
- Packaged 13,847,184-byte synthetic report: 34 real HTTP/browser checks passed,
  including complete source rows, SQL complete input/budget rejection, paging,
  omitted-text search and nested fallback.
- Real Studio shell: five Chromium formats including a large Markdown table;
  four established formats in native WebKit. The new Chromium case reconstructs
  the original UTF-8 row byte-for-byte across bounded windows, checks raw-selection
  isolation and live zh/en labels. Existing host reading controls and syntax
  profile checks remain covered.

- Final standard Studio binary: strict synthetic layout passes 60 samples in
  Chromium and 60 in mapped native Wayland WebKit, including first load, theme,
  locale, code-only/actual font changes, scroll, window resize and reopen. Both
  asset paths serve the same derived hash. Source bytes and owned cache are clean.
- Independent actual-document review verifies IR page adjacency and the original
  13,394,552-byte report opening as Markdown. Its 215,607-byte source row is
  reconstructed exactly from two bounded windows, with Source navigation and
  no changes to the original file. Private content/screenshots remain local.
- Final lease/dirty-refresh smoke passes four checks; actual selection/search
  smoke passes 20, including Unicode source anchors, typed comments, case mode,
  cross-window navigation, cancellation and gateway permission boundaries.

The adopting PR and local build metadata record exact final binary provenance
and required three-platform CI.
The pre-fix #38 failure and final derived-asset geometry verification are
reported separately from original upstream table/input contracts.
All local runtime checks use isolated configuration/cache and disabled or fake
AI backends. Synthetic timings are diagnostics, not a benchmark or speedup claim.
The previous reading-controls adoption remains historical in
[marknoria-reading-adoption.md](marknoria-reading-adoption.md).
