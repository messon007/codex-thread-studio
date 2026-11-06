# MarkNoria Web reader synchronization — 2026-11-06

Studio adopts the unmodified, fixed public Web reader delivery at
`89a3210ae0a37dded80401babd4253bd59af692c`, from the actual remote Studio main
`b718236` and its `5e5d4de6e2ffec368494bc52d0ecffe5068b765a` reader pin.
The local `c73fdf4` checkout was older than remote main; the public SDK migration
and removal of the old source-replacement/query patches had already landed in
PR #55. This update does not repeat that migration or restore those patches.

Only the public distribution allowlist is imported: Web/browser/WebView reader,
its actual Rust core/runtime and resources. No platform-native MarkNoria UI/FFI,
Studio GNOME branch, sibling working files, private documents or credentials enter
this delivery. AI/workspace authorization stays in Studio.

## Reviewed upstream commits

| Commit | Adopted changes |
| --- | --- |
| `35c84a8` | Functional-followup evidence and explicit deferred binary-size work |
| `4c20b45` | Web acceptance fixes, source-preserving structured projections/query help and queued PDF range bursts; unrelated native changes excluded by allowlist |
| `5990e7f` | Public Notes/text-selection ownership, startup URL policy, text-only events and selection cleanup |
| `8a29ba6` | Preview search restoration and bounded streaming JSON formatting |
| `8dda44b` | Standalone current-match/exact-count search, bounded backend result windows; public host gap described below |
| `89a3210` | Complete EPUB paginated code listings and cleared-selection cleanup |

The exporter and templates are read from that exact commit, not an active
checkout's generated assets. Two isolated exports (second offline) match:

- Archive SHA-256: `f79ec49b79a8eacc74eafa17e64017745adb1d573100a7695e8fb3e1991c4460`
- Tree SHA-256: `b5ce0850c2f403c1af8e7d7cc88ed96871fa683d78586505a8c03c1af7a75ae8`
- 827 inventoried files; API/bridge version 1; eight archive/Git-byte checks pass.

`scripts/marknoria-release.json` is the reviewed external pin;
`crates/marknoria/reader-manifest.json` includes source mappings, licenses and
resource hashes. SDK sources are neither patched nor reformatted. Normal builds
and public CI use this checked-in delivery and public locked dependencies only.
Studio's Cargo/npm locks remain unchanged; no audit ignore is added. The earlier
syntect/bincode replacement remains the upstream measured decision already adopted
by PR #55, not a new optimization claim in this update.

The still-open PR #58 (`794a322`, older `4c20b45` pin) is superseded by this
delivery. Its four Studio translations and explicit SQL-correction, numeric-axis
and cell-substring regression checks are retained. Its original acceptance
record is preserved as a clearly historical [checkpoint](reader-acceptance-update.md);
its older package hashes/results are not evidence for this new pin.

## Studio selection policy

Before navigation, Studio adds `notes=host&selectionMenu=host` to the existing
reader URL while retaining its token and other parameters. Public settings keep
both owners at `host`. The child never mounts its default popup or Notes entry.
Studio no longer mounts the shared Notes/Favorite toolbar in its outer pane.
No database or existing note/favorite record is deleted.

Text selections use the Studio overlay: Copy writes the original bounded text,
Comment opens the existing Studio annotation workflow, and Translate remains
available. Favorite stays hidden for document selections; the independent chat/
response Favorites UI remains. The overlay is outside the hidden chat workspace,
so standalone document reading works without selecting an AI session.

The adapter retains document/revision, precision, original source bytes, cell
coordinates, PDF locations and EPUB CFIs. Comments use the existing Unicode-safe
16 KiB UTF-8 excerpt budget. SQL selections obtain verified `querySnapshot`
provenance before exposing actions and never masquerade as source cells. Null
selection, view/revision/owner changes and disposal clear the popup and cancel
pending provenance; late results cannot reopen it. No child DOM, diagnostic
global, injected stylesheet or relaxed origin/token check implements this policy.

## Explicit search limitation

The standalone search improvements are present in the package, but the v1 public
`search` command still calls the legacy HTTP route without `offset`, so it cannot
return the accurate total or a second source-search window. It also lacks a
public current-occurrence/highlight operation. This is reproduced and tracked in
[MarkNoria #24](https://github.com/messon007/marknoria/issues/24).

Studio retains its first-window navigation and uses exact UTF-8 ranges in Source
when supplied. It displays the actual returned result limit (`100+` for a capped
legacy window) and explicitly labels navigation as limited to the loaded results.
Renderer `complete:false` is likewise visible. It honors an explicit total when a
future compatible response supplies one, but never invents an exact total from
the buffer length. Full standalone/host search parity is not claimed or waived.

## Verification

Local Linux validation uses isolated targets, config/data/cache directories,
a dedicated Codex config directory only in the tested child process, and synthetic
documents, with real AI executables explicitly disabled. The agent/shell environment
and user configuration directories are unchanged. No real AI
request is part of acceptance.

- TypeScript check and full UI suite: 795 passed.
- Studio Rust workspace: 152 passed, one existing ignored helper; 14 public reader
  integration contracts passed.
- Independent fixed SDK `--workspace --all-targets --locked`: 148 library, 13 API,
  one Markdown-profile and one syntax test passed; other targets have zero tests.
- Unmodified selection-policy browser contract: 20 checks each in Chromium
  149.0.7827.55 and native WebKitGTK 2.52.6.
- Unmodified EPUB-layout browser contract: 103/103 paginated lines and 824/824
  continuous lines visible; at most three frames and 19 tracked elements;
  no missing lines or page errors.
- Independent actual Studio selection review: real pointer selection without an
  AI session, Copy/Comment/Translate visibility, hidden Favorite, Unicode clipboard,
  Studio comment dialog, selection clearing and owner replacement passed. Gateway
  checks rejected missing credentials, foreign origins and outside-root paths;
  a reader token could not authenticate the Studio gateway. The
  retained regression is `scripts/smoke-reader-selection.mjs`.
- Public SDK backend search contract: 250 occurrences, 100-result windows, original
  Unicode byte ranges, stale-revision/auth rejection and pane isolation passed.
- Actual release structured-file regression in Chromium and native WebKit: 7.8 MB
  XML, 2.3 MB JSON, 50,000-record JSONL and a 900 KB JSONL record retain Preview,
  original Source and whole-input jq. Chromium additionally passes PR #58's SQL
  correction without automatic execution, explicit numeric chart axis and exact
  cell-substring identity checks. Original files remain unchanged and reader
  caches are removed.
- Actual release Studio shell in Chromium and native WebKit: Markdown/table/PDF/EPUB
  first-open and reopen, public
  settings ownership, hidden reader Notes entry, live locale, themes, outline,
  search shortcut, accessible controls and contained SQL dialogs passed.

The release shell's small synthetic first-open samples took 287–335 ms in Chromium
and 327–553 ms in WebKit, with 31–731 mounted reader elements and SDK indexing RSS
of roughly 40–43 MiB. Maximum observed first-open heartbeat gaps were 25–63 ms in
Chromium and 31–113 ms in WebKit. Earlier debug runs included a 3.23-second Markdown
reopen and 1.24-second EPUB reopen. These results are not a speedup claim or a
cross-platform performance guarantee. The standalone
EPUB continuous fixture's maximum observed step was 88 ms. Serialization/transport
is included in the real host opening observations, but was not isolated into its
own benchmark. Required Linux/macOS/Windows PR CI remains the platform gate;
local Linux results are not a substitute for it.
