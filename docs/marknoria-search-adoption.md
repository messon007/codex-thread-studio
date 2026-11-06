# Public reader search adoption — 2026-11-06

This update follows [MarkNoria #24](https://github.com/messon007/marknoria/issues/24)
and supersedes the public-search limitation recorded in
[the earlier synchronization](marknoria-sync-20261106.md). It starts from Studio
main `95f729d5d1128e9c51987118537d2711d2a7b978`.

The adopted source is the official delivery revision
`8552f5ca8f296ea56cc681ef2a19c9ea9f410f9a`. Upstream main `244fb5508d7bb4ca2b410d9b61512703e9a86ffc`
adds acceptance documentation and a standalone Wayland test-host adjustment;
it does not change the pinned delivery's runtime sources.

Two clean offline exports match the official 833-file archive:

- Archive SHA-256: `ecf480cf58e7962f450b86f1c7b6113dbdda25c086722df6aab9deab6952c366`
- Tree SHA-256: `e07ac792d585f7e6ed138b50b2e008802ae958200fcaa2cb6185587fd47ee472`

All eight archive/inventory/tamper/Git-byte checks pass. Imported SDK files are
unmodified; its own lock and Studio's lock remain unchanged. Ordinary builds
continue to need neither private credentials nor a MarkNoria checkout. Native
platform UI/FFI and the separate Studio native branch are excluded.

## Host behavior

Studio checks `capabilities.search.version:1` and uses only public `search`,
`searchActivate` and `searchClear` commands. It renders the authoritative
`state.search` fields rather than computing totals or retaining a growing array
of matches. The SDK owns a source window of at most 100 results plus the active
occurrence; the host activates `{searchId,index}` with captured document/revision,
without supplying or reconstructing navigation coordinates.

Exact totals and result-window completion are separate: the first window of a
250-match source search displays `1 / 250`, despite `complete:false`. PDF/EPUB
renderer limits remain explicit (`500+` and a limit message when capped), and
navigation wraps only within `available`. Match locations retain their source,
row, PDF-page or EPUB-CFI precision; the host does not label generated container
projections as exact original bytes.

Rapid next/previous input keeps only a desired ordinal while prior operations are
cancelled. The displayed current ordinal changes only after verified activation.
Late cancelled work cannot overwrite new state or restore highlights. New queries
clear the previous search immediately, including during debounce and invalid
input. Empty strings clear; whitespace is searched literally. Queries exceeding
the declared 256 UTF-8-byte limit display an explicit error. SDK `CANCELLED` and
local aborts are treated as cancellation, not user-visible failures. New activation
clears a prior local error; superseded callbacks cannot reset newer loading state.

Preview/Source retains the shared active occurrence and highlight. Edit, changed
revision, owner replacement and disposal invalidate old saved-view work. The
existing Studio Copy/Comment/Translate menu ownership, absent reader Favorite/
Notes entry, gateway permissions and SQL-reference validation are preserved.

## Executed validation

- Type/generated checks and all 796 UI tests pass. Host-specific tests use the
  actual shipped search controller to cover source windows, renderer caps,
  owner/revision expectations and late cancellation.
- Release Studio workspace: 149 tests pass, one existing helper ignored; all 14
  public reader integration contracts pass.
- Independent SDK release workspace/all-targets: 148 library, 13 API, one Markdown
  and one syntax test pass. The unchanged controller's 12 contracts also pass.
- Independent actual Studio Chromium review passes 19 checks with both plain and
  inline-formatted Markdown prefixes: exact 250 count, >100 navigation and reverse
  wrap, actual Preview highlight for occurrence 101, same-line Unicode byte ranges,
  precise Source selection, same-tick 11-next/5-previous input, query replacement,
  clear/expired identities, whitespace and oversized queries, malformed/range/stale
  requests, document replacement, no-session menus and authorization boundaries.

The independent review's step timings include Playwright input waits and repeated
public-state polling. They are QA harness durations, not isolated search latency,
a benchmark or a speedup claim. Required Linux/macOS/Windows CI is the platform
merge gate. Additional native WebKit and shared multiformat browser checks are
recorded in the final adopting PR; no unexecuted result is implied here.
