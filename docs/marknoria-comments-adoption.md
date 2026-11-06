# Case-aware search and persistent comment marks — 2026-11-06

Historical acceptance for the `fbb5ff8` delivery. The [current syntax-profile adoption](marknoria-syntax-adoption.md) supersedes this pin and removes the temporary message-catalog supplement described below.

This adoption starts at Studio main `049a0888052cf907ef752a6ab5463bd39a904edc`.
It follows the [public search adoption](marknoria-search-adoption.md), whose
`8552f5c` delivery and acceptance results remain historical.

The new unmodified delivery is `fbb5ff855e16ff55b349bc345d9adf6570a8d615`.
Upstream main `323655d252f180897906d0ac54463b968e7be1cb` adds acceptance evidence
without changing that delivery. The intervening runtime commits provide explicit
case matching and full-cell actions (`8716dc5`), exact comment underlines
(`ae68c15`), and bounded SQLite initialization/contention handling (`fbb5ff8`).
The intermediate comment package is not used.

Two clean offline exports match the official 841-file delivery:

- Archive SHA-256: `65750e0025f8c3ea3e65226b39c6254c4398badcb2a0dc23991aa418fdc98a3c`
- Tree SHA-256: `49f72497dc9c1967f3f837fcb767a63608d6e53757fff736a2c92b27f994e64d`

All eight reproducibility/inventory/tamper checks pass. No SDK files are patched.
The public message catalog omits several new labels; Studio temporarily supplies
them through its presentation adapter. [MarkNoria #28](https://github.com/messon007/marknoria/issues/28) records the
removal condition: remove supplemental labels when a fixed imported catalog
includes them.

The outer Cargo lock changes only the newly required regex dependency and its
regex-automata resolution; the package's own lock is unchanged. Ordinary builds
and public CI use the vendored public delivery without private credentials or
another checkout. Platform-native UI/FFI and the separate native branch remain
excluded. The session write-access menu removed in PR #61 remains absent.

## Studio integration

The host Match case checkbox initializes from the format's declared default
(source/EPUB true, PDF false), keeps its choice through clear and search-panel
reopen within a pane, and repeats the explicit boolean in every host search.
Toggling it cancels old search work and starts at the first result. Search
identity, case folding, original UTF-8 ranges, renderer CFIs, bounded windows,
activation and highlights stay in the shared SDK. Exact totals remain distinct
from incomplete windows and honest PDF/EPUB result caps.

Studio remains the owner of selection actions, comments and AI/workspace state.
It supplies only matching document/revision records through `setCommentMarks`;
`commentRequested` opens the matching existing Studio comment editor or list.
Reader Notes and Favorite entries remain absent, and no implicit reader notebook
reads occur. Studio's separate Favorite data is not part of this mark set.

New selections retain the public version-1 `textRange`, with at most 32 bounded
projection/cell/queryCell scopes, immutable text hashes and UTF-16 endpoints.
Normalization rejects malformed/duplicate scopes and excessive data. The original
bounded quote is retained independently of the reply excerpt's trimming. Existing
source byte locations, PDF geometry, EPUB CFIs and SQL snapshot provenance remain
intact. SQL marks belong only to their live query identity; reopening a historical
snapshot never reruns a query or maps it to source cells.

Comments remain session-owned Studio drafts (up to 32 per session), persisted in
Studio's SQLite store. Saving, editing, deleting, clearing, switching sessions,
reopening the reader and revision changes synchronize the bounded mark set.
The SDK repaints exact mounted ranges across view/theme/window changes. Old coarse
or stale references remain saved and get an explicit unavailable-exact-mark hint;
there is no full-document quote search or guessed highlight. The host serializes
replacements, cancels obsolete work, and checks owner/revision/ID before opening
an editor. Marker painting, table word selection and the explicit View full cell
button/Enter behavior use the shared implementation.

## Validation

Executed before opening the integration PR: all 801 UI tests, generated/type
checks, 14 shared search-controller contracts, and comment component checks in
Chromium (40), forced rectangle fallback (38) and native Wayland WebKit 2.52.6
(40). The UI tests include original selector/quote round trips, malformed bounds,
case identity/paging/cancellation and host mark replacement/event isolation.

Application Rust validation passes 151 tests (one existing helper ignored) and
14 public contracts; the independent SDK passes 171 tests. Public embedded search
passes 150 Chromium and 69 native WebKit assertions, including configurable-case
identity and renderer caps. Embedded source/SQL/ownership checks pass; PDF and EPUB
checks pass 44 and 63 assertions respectively. The latter two upstream wrappers
are not in the archive: their immutable fbb5ff8 test-only scripts were copied into
a disposable extraction, without modifying the installed SDK or replacing assets.

The retained Studio selection smoke passes 20 checks, including insensitive
cross-window activation, case-toggle cancellation, pane retention and original
Kelvin UTF-8 bytes. Existing actual-app issue regressions pass real Markdown/CSV/SQL
double-click selection, explicit full-cell controls, SQL correction/chart/source
checks and large structured documents; the native Wayland structured-text checks
also pass. The four-format actual Studio shell passes in Chromium and WebKit.

Independent actual Studio acceptance passes nine groups: a selected offline
session, native double-click on the second repeated word, exact selector storage
through the real gateway/SQLite, dashed marks surviving cleared selection, owned
hover actions, page reload/document reopen, case toggles, UI deletion, original
Source byte marks across views, overlapping records, and Clear Comments. The
700-pixel viewport keeps the search panel within its bounds. These checks are
retained in `scripts/smoke-reader-comments.mjs` with a non-inference fixture;
`STUDIO_REMOTE_BINARY`, `STUDIO_PLAYWRIGHT_MODULE` and `STUDIO_CHROMIUM_PATH`
select the local binary/browser. Public-state waits use bounded async polling,
not asynchronous Playwright waitForFunction predicates.

The adopting PR records final source/binary provenance. Three-platform CI gates
merging.
QA harness timings are not isolated product latency benchmarks.
