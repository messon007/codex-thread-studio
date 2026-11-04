# Native GNOME migration status

The target remains the entire application on Ubuntu 26.04 GNOME/Wayland.
Implementation order belongs to the migration; individual checkpoints do not
require the user to choose the next area. This is the current parity inventory.
The [plan](gnome-native-plan.md) and [validation record](gnome-native.md) also
contain chronological notes; an old “remaining” paragraph is not evidence that
its feature is still absent.

## Implemented application areas

| Area | Native implementation and existing evidence |
| --- | --- |
| Shell and conversations | GTK4/libadwaita navigation, virtualized transcripts, composer, search, history, lifecycle/reconnect recovery, background status and notifications. Private Wayland fixtures and installed workflow/recovery checks cover these paths. |
| Providers and session tools | Codex-compatible instances and OpenCode, models/effort/Fast, permissions/agent input, commands/skills/MCP/subagents, attachments, queues, Router, Session Map and latest-turn Resources with filters, source occurrences and shared favorites. Fake providers verify request identities and mutation/cleanup behavior without touching real sessions. |
| Markdown | Default continuous native text Preview, Rust indexing, bounded Preview/Source/Edit, outline/search, tables with column controls, paging and read-only SQL queries, explicit/page-change saves, conflicts and undo/redo; independent read-only comparison. Real report copies and generated 32 MiB tables exercise source/context preservation. |
| Rich content | Native images and bounded animation, Mermaid/PlantUML/D2 and math, original-image Save, PDF, tabular SQL/charts. EPUB XHTML, passive local HTML and website content use isolated, lazily created GTK4 WebKit surfaces with native application controls. Rendering limits remain documented. |
| Structured files | Rust token formatting for JSON variants, XML and assignment files; YAML indentation is preserved. Native paged Preview/Source/Edit, display search, original-source comment mapping, explicit/page-change save and native syntax admission. JSON jq filters, slurp and paged read-only output reuse the shared Rust query service. |
| Workspace | Native Files, watches/shared roots, VTE terminal, Git status/staging/history and line-numbered diffs, exclusive panels and parent-view navigation. Private repositories/files/PTYs and installed controls have acceptance coverage. |
| Saved user data | Existing Favorites, comments, pins, session options, queues and settings retain their shared stores. Native capture/source verification, conflict guards, explicit save, exports and restart behavior have focused fixtures. |
| Settings | Native appearance, language, environment/secrets/network/cache, connections/backend configuration, Router, shared folders, session visibility, notifications, browser and translation settings. Exact editable/model values remain literal. |
| Desktop and distribution | Wayland launcher/D-Bus activation, file-open/save portal lifecycles, packaged sibling gateway, verified Mermaid worker and optional diagram/PDF runtimes with notices. Existing Tauri and server entry points remain available. |

These rows summarize implemented areas, not a claim that every interaction or
format has reached complete parity. Exact test scope and preserved failures are
in the validation record.

## Confirmed implementation gaps and next order

The current main integration baseline is `38e27e0`. Markdown-table SQL, JSON jq
and explicit Codex session release now have native controls and safeguards.
SQL and jq reuse the bounded shared Rust services. Existing native CSV/XLSX SQL
retains its separate source importer and shares formatting and table controls.

1. **Finish the control/translation and compatibility audit.** Session Resources,
   information, connections and activity inspection now have native controls and
   focused acceptance. History paging and shared transcript actions now follow
   live language changes, with retained data/selection/draft checks. The composer's
   accessible instruction and Router target picker/control now have matching
   language and identity-preservation coverage. Router history captions and
   sidebar activity/pin prefixes now have live bindings too, preserving literal
   names, queries and state. Named HTML menu actions now pass actual AT-SPI
   Copy, Select all and Translate activation, plus keyboard and pointer checks.
   New session and image controls now pass installed language and portal checks,
   preserving literal paths/names and zoom. The audited fixed
   controls in Tables, EPUB, conversation search and Terminal now have bindings
   and complete installed acceptance. Shared adaptive choices retain their popup
   style and raw model values; EPUB section input survives asynchronous relayout.
   Command palette, session status, permission choices/confirmation and diagram
   inspectors now have bindings and installed English/Chinese debug and optimized
   acceptance. The migration is committed locally as `5342ff9`. Review remaining
   diagnostics against the feature inventory; string matches alone do not
   establish complete control coverage.
   Assess renderer limits against actual supported documents. Historical missing
   Router execution, images, animation, recovery, EPUB search and output-paging
   notes are superseded by later implementations.
2. **Release acceptance.** Keep installed workflows and existing clients' checks
   passing as gaps close; validate fresh-machine packaging and record physical
   GNOME/input/assistive-technology and real-provider checks separately. Tests
   continue using private profiles and document/repository copies. The matched
   optimized pair now passes the installed checks below. Four new private
   profiles reached conversation readiness in 2.6–4.1 seconds without the
   earlier debug startup timeout; its original cause is still not established.
   Fresh-machine/remote CI and physical desktop checks remain separate gates.

Image-region comments and advanced review comments are not assumed parity gaps:
the current image workspace explicitly does not support comments, and the
existing Git Review renders read-only unified diffs. Adding these capabilities
would be separate feature development. Existing PDF region comments remain part
of native PDF coverage.

## Release a Codex session to the CLI

The native Main menu has **Release session to CLI** for a ready, idle Codex
conversation. Active turns, local commands, sending/queued delivery, supervision
and Router controllers prevent release. The action is disabled for OpenCode.
Studio pauses the source queue and sends `thread/unsubscribe`; it never sends
`turn/interrupt`. Conversation content and the in-memory draft remain readable.

The source stays blocked for writing after release, including when selected
again or when its backend reconnects. Errors, unexpected acknowledgments and
connection loss leave release unconfirmed rather than implicitly resuming it.
The normal 35-second RPC timeout also leaves the guard in place. Replies belong
to their original backend, socket generation and release ticket; a late reply
cannot revive a released source or affect another selected conversation.

Use **Refresh sessions** explicitly to reconnect the selected released source
with `thread/resume`. Queued follow-ups remain paused until separately resumed.
Release status, its action and the queue explanation have native language
bindings. The guard is local to this running Studio instance; starting a new
instance follows the normal session-loading workflow.

Run the installed acceptance without touching real profiles or the user's
desktop:

```sh
python3 scripts/smoke-gnome-session-release.py /path/to/native-bundle
python3 scripts/smoke-gnome-session-release.py /path/to/native-bundle --language zh-CN
```

The script uses an owned headless Wayland compositor, ordinary AT-SPI controls
and deterministic provider processes. It checks delayed replies across session
switches, release errors and unknown statuses, actual acknowledgment expiry,
late notifications, explicit reload, durable queues and exact draft/source
retention. It asserts that no model turn or interrupt was sent. Application test
hooks and a gateway override are disabled; the bundle installs to a private
prefix containing spaces and Unicode.

The final debug package passes the installed workflow in English
(`session-release-0cm39mdb`) and Chinese (`session-release-ag_hmerd`), including
the real timeout and late reply. All 397 native tests pass (four opt-in ignored),
and Rust 1.92 checks all release targets. The local candidate is
`target/gnome-native-candidate-20261104-session-release`; its
`SESSION-RELEASE-EVALUATION.json` lives beside the package in the external QA
directory. These are native checkpoint checks; the final optimized distribution
and complete migration acceptance remain outstanding.

## Native JSON queries

The existing structured-file viewer exposes **Query JSON with jq** for JSON,
JSONL and NDJSON. Its formatted Preview, Source and Edit behavior stays in that
viewer. Queries open a read-only saved-file snapshot using the existing native
document actor's open/index/close operations. The jq filter, slurp checkbox,
read-only output, Copy and result pager are GTK controls; each output page is
bounded to 48 KiB. Unsaved edits disable the entry and retire an existing query.
Changing the source retires the old query context. A disconnected query can be
closed and reopened; queries are not silently replayed.

The evaluator, JSON parsing, supervised worker, limits, revision validation and
result cleanup come from the shared MarkNoria Rust service. Its native API alias
preserves authentication, origin and document checks. No system jq executable,
Python process or embedded browser reader is used by this feature. The native
query shell reuses the existing source widgets, highlight admission, theme,
adaptive controls and language bindings, and is reusable for Markdown-table SQL.

Actual JSONL testing also found a shared indexer failure: continuous JSONL or a
large minified value was treated as one indivisible Markdown block. The shared
source-index path now pages JSON without Markdown parsing while preserving exact
bytes, UTF-8 boundaries and line metadata. Markdown continues using its existing
parser and page writer. Both clients benefit, and all 43 vendored reader files
reproduce from the pinned source and combined adapter patch.

Validation commands:

```sh
python3 scripts/smoke-gnome-reader-queries.py /path/to/gateway
python3 scripts/smoke-gnome-reader-query-ui.py /path/to/native-bundle
python3 scripts/smoke-gnome-reader-query-ui.py /path/to/native-bundle --language zh-CN
```

The final debug pair passes installed English (`reader-query-ui-ct_4n_ua`) and
Chinese (`reader-query-ui-7cqbefi9`) workflows, using real controls and an
independent clipboard reader in private Wayland sessions. These cover 1,200-row
JSON projection and paging, query replacement, error retention, source changes,
JSONL slurp, unchanged original files and the exact chat draft. No model turn
is started. GTK 4.22 check nodes without AT-SPI Action use ordinary keyboard
focus traversal; Ctrl+Tab exits the source editor without inserting indentation.

The API check covers permissions, method/origin rejection, stale/cross-document
identities, Unicode result pages, close, slurp, disabled environment access and
termination of an infinite filter. During that filter, an independent source
read completed in about 5 ms in the first successful local run. This is a
focused local service observation, not a UI latency or hardware guarantee.
All 399 native tests, 39 shared reader tests, 272 gateway tests and Rust 1.92
release-target checks pass. The local debug candidate is
`target/gnome-native-candidate-20261104-reader-jq`; its
`READER-JQ-EVALUATION.json` is beside the package in the external QA directory
and retains the startup-timeout observation. The final optimized distribution, fresh-profile
startup qualification and the broader migration audit remain outstanding.

## Native Markdown-table SQL

Both continuous and paged Markdown table toolbars expose **Query table with
SQL**. The saved document's full table, including physical `_line` metadata,
uses the same bounded, read-only Rust SQLite service as the browser reader.
The initial query names every field; users can remove fields or add `WHERE`,
`ORDER BY` and `CASE`. Compact formatting uses the existing Rust SQL formatter.
The GTK query window shares its source editor, adjustable input/result split,
asynchronous transport, result cleanup, theme and language bindings with jq.
The existing native column picker and CSV cell styling provide result-column
visibility, resize and Wrap. Columns, Copy and Wrap stay together on the right.

Result paging does not rebuild the source table or write Markdown. Invalid or
unauthorized statements retain the last successful result. Query windows retire
when that reader replaces its source, becomes dirty or loses its valid gateway
context. Navigation to a separate retained workspace still follows the existing
Back-navigation lifetime. A reproduced GTK crash when resetting from three
result columns to five is fixed by removing old model rows before installing
the incoming schema; GTK binds columns synchronously.

Installed English (`reader-query-ui-fa0tlhyr`) and Chinese
(`reader-query-ui-ov1spvtc`) checks pass on the matched debug package, without
application hooks. They cover 1,200 source rows, numeric sorting/filtering and
`CASE`, result paging, the shared picker, actual row-height changes with Wrap,
independent Wayland clipboard reads, error retention, reset/schema replacement
and source retirement. Original files and the chat draft remain unchanged;
no inference is started. The native API check also covers all 600 filtered
rows, exact source lines, result close, stale/cross-document identities,
authentication/origin/method checks, write/file-access rejection and bounded
recursive SQL execution.

```sh
python3 scripts/smoke-gnome-reader-query-ui.py /path/to/native-bundle --sql
python3 scripts/smoke-gnome-reader-query-ui.py /path/to/native-bundle --sql --language zh-CN
```

The same pair also passes English (`reader-query-ui-hsv4cn1l`) and Chinese
(`reader-query-ui-jn9k_660`) jq regression checks after the shared window changes.
All 400 native tests, 40 shared reader tests, 272 gateway tests and Rust 1.92
release-target checks pass. The debug candidate is
`target/gnome-native-candidate-20261104-reader-sql`, with source/package hashes
and retained failure records in `READER-SQL-EVALUATION.json` beside the package.
The following optimized checkpoint now covers the integrated pair as well.

## Optimized integrated release checkpoint

The local candidate is `target/gnome-native-candidate-20261104-integrated-release`.
Its native client and gateway are both optimized builds of the integrated
sources at `3cf1427`, including session release, jq and Markdown-table SQL.
Application UI remains GTK4/libadwaita; both query evaluators and result stores
are the existing shared Rust services. No separate MarkNoria checkout is needed.

Ten sequential checks pass on Ubuntu 26.04.1, GTK 4.22.4, libadwaita 1.9.1 and
Mutter 50.1, using owned headless Wayland desktops without X11:

- Standalone installation, immutable update/reinstall, executable/notices and
  checksum validation, corruption rejection and unrelated-tool preservation.
- Authenticated jq/SQL APIs and source responsiveness; installed SQL and jq in
  English and Chinese, retaining original files and the exact chat draft.
- Installed native core workflows with live language/backend settings, Git
  Review, file portals, Preview/Source/Edit, undo/redo/save, viewer returns,
  images, PDF, EPUB, passive HTML and all four diagram/math inspectors at 400%.
- The actual 8.4 MiB report plus generated 32 MiB LF/CRLF tables: bounded source
  windows, exact outline lines, distant context, table paging/column filtering,
  explicit/page-change editing saves and original-report preservation.
- English and Chinese session release, including real acknowledgment expiry,
  late acknowledgments/notifications and explicit reload without model turns
  or interrupts.

These installed checks use fake providers and no application test hooks or
gateway override. Earlier real-CLI/mock-provider transport acceptance remains
separately recorded. The four fresh-profile query launches reach conversation
readiness in 4.07, 2.88, 2.62 and 2.87 seconds. OS disk caches were not flushed;
these observations do not explain the preceding debug timeout or guarantee
startup latency on another machine.

The large-reader run observes no WebKit content process. Its peak native RSS is
about 370 MiB and gateway RSS about 31 MiB over all three documents. Source
windows stay below 128 KiB. The report opens in about 3.5 seconds and generated
files in 0.6–0.7 seconds in this fixture. Outline measurements include keyboard
focus/traversal, AT-SPI polls and opening/closing the pane; they are not isolated
single-click or parser timings.

`NATIVE-INTEGRATED-RELEASE-EVALUATION.json` beside the external package records
the exact installed executable/source hashes, runtime versions, ten check
results and retained logs. This checkpoint also retains 400 passing native
tests, 272 gateway tests, 40 shared reader tests, Rust 1.92 release-target
checking and the reproducible 43-file vendoring check. It does not claim a
fresh-machine install, physical IME/Orca/fractional-scaling acceptance or remote
CI. GitHub credentials still lack `workflow` permission, so the new workflow
and feature branch have not been published. The default user installation is
unchanged, and the full migration goal remains active for the final parity and
compatibility review.

## Current main integration

The native branch integrates main through `38e27e0`, retaining shared
Codex-daemon transport, reader query services and the upstream dependency fixes.
Native text reads now use an authenticated raw-source endpoint, so the browser's
new JSON reader selection cannot replace the native Preview/Source/Edit reply
with an embedded-reader URL. Native EPUB decoding handles fallible ZIP names;
Calamine 0.38 supplies ZIP 9 support for XLSX without the older ZIP dependency.
The native parser minimum is 0.13.6 and its newer Tokio remains retained.

Fresh checks pass: 272 normal gateway tests (two opt-in tests now ignored by
default), 38 MarkNoria tests, 100 shared Markdown model tests, 110 tests with
syntax/structured features, five SVG-resource tests, 395 native tests and 753
web tests. The separate real CLI opt-in also passes against Codex 0.166.0 with
two clients, native private history reads, local mock model replies, CLI turns
and private-daemon restart. Its harness now compiles inside the actual gateway
test crate rather than omitting native routing dependencies. No real user
session or external model is used.

Rust 1.92 checks, the default Tauri Linux check and installed debug language,
viewer/backend and both-provider navigation workflows pass. All 43 pinned
MarkNoria library/reader/provenance files reproduce from the combined embedding
and query patches without modifying the separate MarkNoria checkout. Exact
current JavaScript dependencies were installed under `/media`; the previous
dependency tree is preserved.

The optimized integrated checkpoint above now refreshes acceptance for this
integration, including native query and session-release controls. Older
status/inspector packages predate this merge. Remote publishing still requires
suitable workflow permission; no remote branch or CI is claimed.

## Navigation timing follow-up

Actual private Wayland wheel input now has a component-level regression fixture
that imports the production coordinator. Three gestures stop its pending work
after one probe and do not snap back when a delayed target becomes available.
The positive control exposed a separate stale-allocation bug: tick callbacks
run before layout, so a changed adjustment could be combined with old bounds.
Waiting one layout cycle after an offset change fixes the reproduced 457 px
misalignment; the target now settles at 16 px. The twelve-frame limit, reset,
source-generation and wheel cancellation guards are preserved.

All 395 native tests, Rust 1.92 release-target checks and the debug build pass.
Installed `turn-navigation-n4ft2ro2` verifies real turn controls, keyboard jumps,
literal prompts and source/draft retention. Installed Codex and OpenCode scope
checks each cover three history pages, context switches and retained data;
Codex additionally covers live/historical streaming and late-reply rejection.
These tests use fake providers and no application hooks. The wheel fixture is
component scope, not a replacement for installed application acceptance.

The previous optimized status/inspector candidate predates this small fix.
Optimized acceptance must be refreshed after integrating the thirteen newer
commits found on remote `main` (`38e27e0`), including shared Codex-daemon support
and reader/dependency fixes. The remote fetch changed no worktree files.

## Status, permissions and inspector follow-up

The latest native tests pass: **395 passed, four opt-in ignored**. The debug
package passes installed `release-ui-rwaml606` without application hooks or a
gateway override. It covers both-language command filtering, translated status
fields with literal backend/session/project identities, Full access cancellation
without changing inherited permissions, and diagram zoom/fit. Existing history,
comparison, images, file portals, source saves and draft retention pass in the
same run. No web-content process is started for these native operations.

Two earlier fixture attempts are retained: `status` also matches the MCP
description, so the final check uses `/status` and waits for its filtered result
before Enter. These failures do not establish a product command-dispatch defect.
Rust 1.92 release-target checks and the optimized build pass. Optimized installed
`release-ui-btd85gzz` passes the expanded language/viewer/backend workflow,
including PDF, EPUB, structured and passive HTML saves, animation, original
image handoff, source search/edit/reload and connection diagnostics. The local
candidate is `target/gnome-native-candidate-20261104-status-inspector`; private
`native-status-inspector-20261104/STATUS-INSPECTOR-EVALUATION.json` retains exact
source, binary and installed-package identities. The previous optimized
reader-controls candidate and default installation are preserved.

The shared-gateway review covers preserved default Tauri builds and Linux-only
native dependencies, authenticated routes, bounded workspace/Map work, optional
revision checks for legacy clients, private history-reply correlation and shared
annotation/source preservation. Commit `5342ff9` records the migration on
`feat/ubuntu-26-04-gnome-native`. Publishing that branch is blocked: SSH key
authentication failed, the GitHub CLI OAuth credential lacks `workflow` scope,
and the connector returned 403 when creating the branch reference. No remote
branch or CI run is claimed. This is not physical GNOME/IME/Orca/real-provider
acceptance. Remaining
work includes the renderer/release audit and explicit wheel interruption while a
navigation alignment is pending. The whole-application goal remains active.

## Latest qualified reader/search/terminal checkpoint

Tables, EPUB, conversation search and Terminal now have explicit interface
bindings, preserving literal queries/results, source names/snippets and shell
text. Adaptive UI choices retain wrapped popups/selected markers and raw values;
context distinguishes delimiter Tab and EPUB Pages. SQL/search controls have
accessible names and the reading-menu name stays stable while open. A reproduced
EPUB relayout bug is fixed: background controls no longer overwrite an unsubmitted
section number.

All 395 native tests pass (four opt-in ignored), with Rust 1.92 checks. Matched
packages pass 20 layout cases and full installed `control-language-khtxuxvz` without application
hooks or gateway overrides: SQL/results and trailing-space filtered search survive
language changes, real terminal input keeps its shell PID, and EPUB section 2,
paged layout, edited section input and both-language book search are preserved.
Expanded viewer/backend/connection/language and Router unread checks also pass.

The local candidate is `target/gnome-native-candidate-20261104-reader-controls`,
with private `native-reader-controls-language-20261104/READER-CONTROLS-EVALUATION.json`.
Gateway/web/Tauri checks are inherited after unchanged input and gateway-binary
verification. Previous candidates and the default installation remain. Remaining
work is the status/inspector and final control audit, source review/commit, explicit
wheel interruption, renderer/release audit and separately documented physical
GNOME/IME/Orca/real-provider acceptance. The whole-application goal remains active.

See [reader/search/terminal language](gnome-native.md#native-reader-search-and-terminal-language-controls).

## Previous qualified session/image language checkpoint

New session and image controls now follow language changes, including actual
file-portal titles/filter and accessible project-entry naming. User paths, image
names and alt text remain literal; zoom, source identity and draft state survive.
All 395 native tests pass (four opt-in ignored), with Rust 1.92 release-target
checks. Matched packages pass 20 layout cases, installed expanded language/viewer
`release-ui-1x2blan9` and installed Router unread acceptance. The six localized portal
cancellations preserve the existing image, proposed save name and project path.
Existing real image export, native source picker, viewer returns and editing
checks pass in the same optimized workflow, without application hooks or a
gateway override.

The local optimized candidate is
`target/gnome-native-candidate-20261104-session-image-language`, with private
`native-session-image-language-20261104/SESSION-IMAGE-EVALUATION.json`. Gateway,
existing frontend and Tauri checks are inherited only after their unchanged input
and gateway binary verification. Previous candidates/default installation remain.
Next are the specific language gaps above and the remaining compatibility/release
and wheel-interruption audit; physical GNOME/IME/Orca/real-provider checks remain
separately documented. The whole-application goal stays active.

See [session and image language](gnome-native.md#native-session-and-image-language-controls).

## Previous qualified Router unread checkpoint

Router prompt rows now show owned unread-response indicators for the actual page.
Updates preserve bound buttons, geometry and focus; accessible names/tooltips
follow live language changes while source questions stay literal. Native read
receipts still require the actual response end and existing ownership/dwell guards.
Ordinary sources and read-only browser history do not inherit native unread cues.

All **395 native tests** pass (four opt-in ignored), plus Rust 1.92 all-release-target
checks. Matched debug/release packages pass 20 layout cases, installed unread
controls, prompt navigation, both providers' paged/source workflows and core/live
language controls. Installed `router-unread-zt1qx5at` uses a real private-prefix installation,
sibling gateway and no application hooks, verifies actual question overlap versus
response-end read receipts, both Router pages, source restoration, untouched browser
records/foreign-controller state and preserved drafts without inference.

The local optimized candidate is
`target/gnome-native-candidate-20261104-router-unread`, with private
`native-router-unread-qualified-20261104/UNREAD-EVALUATION.json`. Gateway/web and
existing Tauri checks are inherited only after unchanged source/input and gateway
binary verification. Earlier candidates and the default installation remain.
Remaining work is the specific language gaps above, the control/renderer and
release audit, explicit wheel-interruption timing and separately documented
physical GNOME/IME/Orca/real-provider acceptance. The full goal is active.

See [Router unread cues](gnome-native.md#native-router-unread-turn-cues).

## Previous qualified turn-navigation checkpoint

The compact Turn navigation control now has named virtualized prompt buttons,
Rust-coalesced previews and mapped-row current feedback over the displayed page.
First/middle/last jumps align the prompt into view, preserve literal source text
and drafts, and retire stale actions across source/page changes. Both Codex and
OpenCode pass three history pages, Newer/Live and source/draft restoration.
Codex additionally passes live updates while reading older history, actual reading
position during a live delta and a delayed old-source reply after switching away.

All **395 native tests** pass (four opt-in ignored), with Rust 1.92 release-target
checking and the existing Tauri Linux check. Gateway source and both binaries
match the Continue checkpoint; its **264 gateway tests** and **749 web tests**
are inherited rather than rerun. Matched debug/release packages pass 20 layout
cases and installed prompt navigation, both provider scopes, Continue, core/live
language controls and Session Map. Navigation/scope/Map use actual private-prefix
installations without application hooks or a gateway override. Continue deliberately
uses the private Ollama proxy. No real provider/default installation is touched.

At 640 px, normal and large-type popup captures were inspected independently of
the parent window. This exposed popovers retaining the default font while the
window used the configured interface size. The shared typography selector now
includes GTK popovers; heading height grows from 18 to 24 px and prompt labels
from 22 to 29 px. Both layouts pass the same navigation/draft checks. Passive
capture hooks are used only in those separate debug runs.

The local optimized candidate is
`target/gnome-native-candidate-20261104-turn-navigation`. Private
`native-turn-navigation-qualified-20261104/NAVIGATION-EVALUATION.json` retains
source/package/evidence identities and earlier failures. Previous candidates stay
available. Remaining work is Router unread presentation, the control/compatibility
and release audit, and separately recorded physical GNOME/IME/Orca and real-provider
acceptance. Explicit wheel-interruption timing has not been claimed by these tests.
The full goal remains active.

See [native turn navigation](gnome-native.md#native-turn-navigation).

## Previous qualified composer Continue and message-flow checkpoint

Settings → Sessions now offers the shared one/two/three-message queue depth and
Continue behavior. Continue generates a session-model or local Ollama draft, or
explicitly sends a short continuation when Quick send is selected. Drafts require
an empty composer and a verified idle source. Cancellation, typing, source/model
changes and stale results cannot overwrite or send a user's draft. Lowering the
queue limit preserves messages already queued; conflicting settings remain
unsaved until reloaded. Ctrl+Shift+Enter is scoped to the composer.

All **394 native tests**, **264 gateway tests** and **749 existing frontend tests**
pass (four native and one gateway opt-in tests ignored). Rust 1.92 all-release-target
checking and the existing Tauri Linux check pass. Matched debug/release packages
pass translation regression, 20 layout cases and installed core/language controls.
Installed `continuation-xrrrdwpo` exercises actual GTK controls with private
Codex/OpenCode providers and a local Ollama proxy, without application hooks.
It deliberately uses a gateway override for that proxy. Utility cleanup,
model/effort/Fast preservation, cancelled creation, held HTTP replies, settings
conflicts, shortcut scope and unchanged source conversations are verified.

The layout regression also exposed continuous Markdown inheriting the interface
font. Restoring the shared content-style class gives the configured 24 px body
and scaled bold heading; the pre-fix measurement was 20 px. Mapped normal and
large-type captures were inspected. Optimized binaries are available at
`target/gnome-native-candidate-20261104-composer-continue`; private
`native-composer-continue-font-20261104/CONTINUATION-EVALUATION.json` retains source,
package and validation identities. Earlier candidates/default installation remain
unchanged. Turn navigation, the final compatibility/release audit and separately
recorded physical GNOME/IME/Orca and real-provider checks remain. The goal is active.

See [composer Continue and message flow](gnome-native.md#native-composer-continue-and-message-flow).

## Previous native language and HTML-menu checkpoint

Router captions, search/count/unread phases, toolbar hints and sidebar
activity/pin prefixes now change language live. Session titles such as
`Search results {count}`, source text, query spaces, selections and composer
drafts remain literal. Installed `router-archive-ok524weu` verifies the controls,
Chinese/English switching, unchanged browser records and draft restoration.

Passive HTML uses the shared named GTK action-row popup for Copy, Select all and
Translate. Menu and Shift+F10 work; Copy uses WebKit's original selection and an
independent Wayland clipboard reader verifies the result. Actual assistive-client
activation runs one private translation utility and verifies exact cleanup.
Retiring/hiding a reader closes its popup and rejects stale actions. A real
right-click regression exposed surface coordinates being used as a widget-local
anchor; the shared coordinate adapter now applies the native surface transform
and widget conversion. `translation-mpgf7nc6` verifies the exact local anchor,
translation/comments, source bytes, scoped selection and utility cleanup.

All **393 native tests** pass (four opt-in ignored), along with Rust 1.92 release
target checks. Gateway source and both binary hashes match the preceding archive
checkpoint, whose **260 passing gateway tests** are inherited rather than rerun.
Matched native debug/release packages pass passive HTML and Map workflows.
Optimized `html-menu-0x4ed2sp`, `release-map-af5hp8ia` and `release-ui-jq9v9uie`
verify installed menu actions, keyboard navigation/dialog focus and language/core
reader controls without application hooks or a gateway override. Installer
`bundle-install-py6rhyma` passes independent installation, unchanged unrelated
tools, repeat/update retention and checksum rejection. CLI startup also passes
in the read-only extracted Ubuntu 26.04 runtime without host system files,
checkout, display, network or backend; this is not a configured fresh desktop.

The local optimized candidate is
`target/gnome-native-candidate-20261104-native-menus`. Private
`native-language-html-menu-pointer-20261104/MENU-EVALUATION.json` retains source,
package and evidence hashes, inspected earlier language/translation captures and
the invalid-setting and pre-fix pointer probes. Previous candidates and the
default installation remain unchanged. The full goal stays active: message-flow
settings/Continue, turn navigation and the final compatibility/release audit
remain; physical GNOME/IME/Orca and real-provider checks are separately recorded.

See [native language and HTML menus](gnome-native.md#native-router-language-and-html-menus).

## Previous browser Router history checkpoint

The archive reader uses the original `router_dispatches` rows and never imports
old entries as native delivery intents. Indexed 20-record pages return metadata
only, without provider startup or recovery. Stable identities include the row and
controller/turn key; receipt checks reject records changed during source reads.
Native mirror filtering requires both the native ID and its actual mirror key,
so unrelated records and orphans stay visible. Search uses decoded metadata
strings, with significant spaces and a two-second read-only snapshot budget.

Opening a page loads source previews through two bounded readers. Original
controller questions and accepted target questions remain distinct; reply parts
retain actual backend/thread/project/turn/item identities. Codex uses older turn
pages; OpenCode retains its directory and opaque cursor across message pages,
including replies split across pages and structured output. Foreign source parts,
repeated cursors and malformed metadata cannot manufacture response anchors.
Previews walk at most 64 pages and allow 15 seconds each for original/source
reads; clipped or unavailable previews direct the user to the source conversation.
Search covers saved routing metadata, not provider replies fetched for display.
Saved delivery status remains historical and opening the archive does not claim,
monitor, resume or send an old delivery. Archive rows do not enter supervision
admission, automatic queue gates or read-acknowledgement writes.

All **393 native tests** (four opt-in ignored) and **260 headless gateway tests**
(one opt-in ignored) pass. Private `router-archive-ad2vc19m` exercises 55 browser
records plus a native mirror, metadata-only paging/search, Codex/OpenCode source
reads, actual GTK selection/search/return, preserved drafts and unchanged records.
It uses AT-SPI/owned Wayland input without application hooks. Source previews
retain the actual accepted question even when the old recorded forwarded prompt
differs. Rust 1.92 and the existing Tauri Linux check pass.

Matched optimized binaries are packaged. Installed `router-archive-s7zop9eg`
passes the same source/selector/search workflow without application hooks or a
gateway override. Packaged `router-archive-kqq25z3m` adds passive mapped screenshots,
which were inspected separately. Router execution `router-runtime-9vxlhwj9`,
supervision restart `supervision-ui-8fj2ptmd` and all requested existing installed
workflow groups (`release-ui-2wztnlcj`) pass against the same matched package.
The local optimized candidate is `target/gnome-native-candidate-20261104-router-archive`;
private `native-router-archive-20261104/ARCHIVE-EVALUATION.json` retains source,
package, evidence and failed-probe hashes. The previous candidate/default
installation remain unchanged.

See [browser Router history](gnome-native.md#native-browser-router-history).
The newer language/menu checkpoint above supersedes this checkpoint's owned
caption/sidebar and HTML-menu gaps. The full migration remains active.

## Previous supervision restart and installed-controls checkpoint

An indexed metadata-only lookup now resolves a source's Router/controller before
supervision starts, including when opening the target directly after restart.
Pending discovery briefly gates Router queues; the resolved source/controller
remain held through evaluation and handoff. Stop during discovery retains the
gate until the actual parent is known. The lookup does not start providers,
recover jobs, load full replies or wait for a Map update. Unknown/mismatched
ownership fails rather than guessing. Terminal menu status is scoped to its
actual turn and connection generation.

All **392 native tests** (four opt-in ignored) and **257 headless gateway tests**
(one opt-in ignored) pass. Rust 1.92 and the existing Linux Tauri build check pass.
Matched optimized native/gateway binaries are built. The self-contained debug
bundle passes ordinary supervision, cold target-side opt-in, broad controls and
both Map GUI regressions. The cold fixture proves that no hidden Router reply
projection is loaded while resolving its controller queue.

Installed `release-supervision-t18zqm1l` drives the optimized application through
AT-SPI and owned Wayland input, without application hooks or a gateway override.
It verifies explicit opt-in, visible progress, authoritative evaluator output,
exact continuation, held/released queues, preserved draft, durable Router handoff,
current-turn status and Stop without restarting evaluation or the paused queue.
Four source turns and two owned evaluator cleanups are checked against the
private provider ledger. Installed `release-ui-cdq9n57e` also passes the existing
diagram, language, backend, Git, viewer, process-recovery and event-gap groups
against this same package. All providers and profiles remain private fixtures.

The local optimized candidate is
`target/gnome-native-candidate-20261104-supervision`. Its private
`native-supervision-scope-20261104/SCOPE-EVALUATION.json` record retains source,
package and evidence hashes. The normal installation remains unchanged.

See [restart and installed supervision](gnome-native.md#native-supervision-restart-and-installed-controls).
The full goal remains active: old browser Router history, the remaining parity
and control audit, and final distribution/desktop acceptance still need work.

## Previous composer/Router language checkpoint

The composer's tooltip and accessible instruction now follow language settings.
The Router target dialog, automatic-routing choice, search hint, result counts,
paging/refresh controls and error wrappers do too. Explicit target names/keys/paths,
search text, model options and drafts retain their literal values. Switching
between automatic and explicit targets retires the old interface binding.

Packaged `router-ui-r2umt2ul` passes three launches with routing, Stop, both-provider
sources, favorites, restart without replay, history and live locale changes on a
50-result picker page and explicit/automatic target transitions. History
`history-pages-ui-tv80y6fd` and broad controls `run-24l2katt` pass. Installed
`release-ui-gxvr3dbu` checks the actual composer accessible name in Chinese and
English, preserved draft contents and every existing optional workflow group,
without application hooks. All 330 native tests, 749 web tests and Rust 1.92 checks
pass; the gateway and existing clients are unchanged.

The candidate is `target/gnome-native-candidate-20261104-router-language`; private
`native-router-language-20261104/ROUTER-LANGUAGE-EVALUATION.json` and `BUILD.json`
retain source, package and evidence hashes. At that checkpoint the fuller parity
audit found no native turn-supervision runtime or controls. Backend integration
has since advanced as described above; completing the native workflow remains
the next feature priority. HTML-menu/owned-control and final
acceptance work remain. See
[composer/Router language](gnome-native.md#native-composer-and-router-language).

## Previous history/transcript language checkpoint

Older/Newer/Live, history counts/status/details and tooltips now follow the chosen
language. Copy/excerpt/full-output/favorite actions, routed-session tooltips, table
wrapping, the Sessions heading and several composer tooltips use the shared
explicit bindings. Provider diagnostics and identifiers remain literal values;
changing language changes no action identity, snapshot, provider request or cache.

Packaged history runs `history-pages-ui-3rm0b8bu` and `history-pages-ui-whp57j1i`
cover preferred/compatibility Codex pages, legacy fallback, OpenCode parts/cursors,
live Chinese/English changes, selected text, reading anchors, preserved drafts,
loading/error notices and stale-reply/backend isolation. Activity and 6.1 MB output
regressions and broad controls `run-r5q3iv2h` pass. Installed `release-ui-l1zu35n_`
opens history by its real shortcut, switches language in Settings, verifies counts
against the private provider ledger, checks translated Copy/favorite controls and
returns to Live. Every existing installed optional workflow group passes without
application hooks. All 330 native tests, 749 web tests and Rust 1.92 checks pass.

The candidate is `target/gnome-native-candidate-20261104-history-language`; its private
`native-history-language-20261104/HISTORY-LANGUAGE-EVALUATION.json` record retains
source/package/evidence hashes and failed fixture attempts. The gateway and existing
clients remain unchanged. This closes the identified history/header-label gap;
remaining owned controls, HTML menu accessibility, compatibility and release
acceptance keep the full migration active. See
[history/transcript language](gnome-native.md#native-history-and-transcript-language).

## Previous activity-inspection checkpoint

Activity headers expose **Inspect activity snapshot**, using the shared paged
native output viewer. Rust preserves tool arguments/errors/status, command arrays,
file-change kinds and future metadata; visible reasoning falls back to content
when the summary is empty. OpenCode retains its original tool part and server IDs.
Plan notifications replace one stable row per turn without deciding session idle.
Snapshots remain immutable; reinspection after a complete provider update opens
its newer revision. Output-viewer controls and summaries follow live language changes.

Packaged `activity-mmquojza` (Codex) and `activity-hxm2x976` (OpenCode) pass actual
inspect controls, metadata-only updates, paging/search/export, exact Copy and
preserved drafts/lifecycle. No WebKit content process starts. The 6.1 MB real-report
output regression `outputs-ui-jhqq1q_6`, real failed exports `write-failures-ucznyigt`
and broad controls `run-ludyucbr` pass. Installed `release-ui-ndnqt5wt` verifies the
inspector and exact clipboard content through AT-SPI, without application hooks,
alongside every existing diagram/language/backend/Git/viewer/recovery group.
All 330 native tests (four opt-in ignored), 749 web tests and Rust 1.92 checking pass.

The local candidate is `target/gnome-native-candidate-20261104-activity`; its private
`native-activity-final-20261104/ACTIVITY-EVALUATION.json` record and `BUILD.json`
retain source, package and evidence hashes. Gateway/runtimes and 853 existing-client
source files are unchanged from the preceding checkpoint. Full details and limits
are in [activity inspection](gnome-native.md#native-activity-inspection).
The goal remains active; the control/compatibility audit and final acceptance remain.

## Previous connection-diagnostics checkpoint

Connections & accounts is available from the application menu and the Backends
settings page. Backend rows show actual session-event connection state; expandable
details show executable, protocol, transport, execution environment, host platform
and any reported backend version or health result. Health/metadata does not imply
that a session event stream is connected. Configuration location/errors and the
native/gateway versions are included. Live status updates need no metadata polling;
Open/Refresh performs bounded Rust reads over existing authenticated endpoints.

Packaged private Wayland `connections-t9ibj44b` passes real controls, literal markup-like
names/arguments, two independently connected event streams, partial health failure
and recovery, live Chinese labels, closed-dialog replies and an actual gateway
parent-pipe stop/reconnect. The executable canary remains unstarted; original
files and unsent drafts are unchanged, all owned processes exit, and no WebKit
content process starts. Screenshots were inspected and the layout puts backend
status first. Rust tests verify four-request admission, stable registry order,
known endpoint paths, cancellation, rejected health metadata and Unicode limits.
All 323 native tests, 749 web tests and Rust 1.92 release-target checking pass.
Complete native control run `run-g2rxfdae` and both-provider session-information
regressions pass. Installed release `release-ui-4soueiio` opens diagnostics from
the actual Settings row without application hooks and passes every existing
diagram, language, backend, Git, viewer, HTTPS image and recovery group.
The candidate is `target/gnome-native-candidate-20261104-connections`; its private
`native-connections-final-20261104/CONNECTIONS-EVALUATION.json` record retains
source, evidence and package hashes. Gateway, runtimes and existing-client sources
remain unchanged. The migration remains active; activity-detail preservation and
the control/compatibility audits remain next.

## Previous session-information checkpoint

The application menu's Session information dialog displays provider metadata and
the original opening question, including questions outside the loaded history
tail. Opening questions and Router responsibilities support explicit conflict-checked
Save, Copy, Reload and dirty-close choices. The editor retains fields after a
conflict and requires Reload; HTTP status, rather than diagnostic text, determines
conflict handling. Unknown opening-capture fields survive narrow updates.

Generate with AI adds an ordinary turn to the selected session using its current
model, effort, Fast and access options. It preserves the chat draft and only fills
the responsibility field; saving remains explicit. Stop targets the identified
generation. OpenCode uses its session-level activity marker, checks the latest
server-owned user message before aborting, and rejects a newer message. Its API
cannot make that read and session-level abort atomic. Source/session/connection
changes freeze the editor without discarding edits. Router controllers do not
automatically capture their internal coordination prompt as a user's opening question.

Packaged private Wayland runs `session-info-hjunnzcs` (Codex) and `session-info-t08z2_ci`
(OpenCode) pass actual dialog/button, clipboard, save/reopen, two-editor conflict,
Reload Cancel/Discard, generation/Stop, stale-editor and live Chinese-label checks.
Screenshots were inspected. Original files and unsent drafts remain unchanged;
owned processes exit, and no WebKit content process starts. All 320 native tests,
250 gateway tests, 749 web tests, Rust 1.92 release-target checking and the original
Linux Tauri check pass. The complete native control run `run-c4limv0f` also passes,
including Router role metadata/conflicts. Matched installed release run
`release-ui-ynznn4wn` passes all existing diagram, language, backend, Git, viewer,
remote-image and recovery groups without application hooks. The source/package
record is `native-session-info-final-20261103/SESSION-INFO-EVALUATION.json` in the
private QA directory. The candidate is available at
`target/gnome-native-candidate-20261103-session-info-final`; existing installations
were not replaced. The wider migration remains active.
See [session-information details](gnome-native.md#native-session-information).

## Previous session-resource checkpoint

The native Resources workspace indexes the latest turn's user, assistant and plan
text on the bounded Rust worker pool. The shared SIMD-enabled Markdown parser
handles explicit/reference links and images; plain URLs and path references are
also indexed. Fenced commands/tool output and older turns are excluded. Search,
All/Web/Files/Code filters, occurrence selection, Copy, read-only source-message
navigation and the existing shared favorite editor are connected to real controls.
File/media opening retains project/shared-folder authorization. Folder opening
expands the existing Files tree rather than granting a new root. Viewer Close
returns to Resources; session changes restore that session's search/filter state.

All 316 native tests and Rust 1.92 release-target checks pass. Packaged private
Wayland `session-resources-o43uqukw` verifies the actions above, explicit refresh,
decoded filename navigation, automatic refresh after shared-folder Save, blocked
paths, stale-response rejection, live Chinese labels, unchanged chat/draft
and original-file hashes, and owned-process cleanup. Painted screenshots were
inspected. Limits are explicit: 1 MiB per narrative item, 4 MiB per turn, 100
references per item and 2,000 per index. Truncation is shown in the footer.
The matching optimized installed `release-ui-s06679tl` passes every existing
diagram, language, backend, Git, viewer, portal, remote-image and recovery group,
without application hooks or a continuous-reader override. Gateway, renderer and
runtime bytes remain unchanged from the distribution candidate.
See [session-resource validation](gnome-native.md#native-session-resources).
The wider migration remains active; connection/activity audits and final release
acceptance still require work.

## Previous distribution checkpoint

The native bundle now carries its runtime dependency installer, standalone
application installer, README and complete checksums. Installation needs neither
a source checkout nor Python. Versioned private siblings prevent updates from
mixing a running frontend with new engines, and avoid overwriting unrelated
`d2`/Java/PDF tools. Private-prefix tests verify reinstall/update, original bundle
integrity, corrupt-bundle rejection and desktop-entry validity.

Installed `release-ui-z2x7d1fa` passes all existing workflow groups from the new
layout. Current compatibility checks pass 725 web tests, 247 gateway tests,
36 indexer tests, 110 native Markdown model tests and five SVG tests; the original
Tauri desktop path also type-checks with the isolated GTK3 SDK. The optimized
native/gateway/renderer executables are unchanged from the preceding candidate.
WebKitGTK's library is now explicitly listed as a startup dependency; its content
processes remain lazy. CI gains a separate runtime-only Ubuntu 26.04 install/link
job; remote CI has not yet run. Full migration remains active because the session
tools listed above are still required.

## Previous default-reader checkpoint

Normal launches now use continuous native Markdown Preview, including both
comparison panes. Cross-block selection, projected Copy/primary/drag, comments,
translation, paging and embedded table/diagram/image controls retain the shared
Rust preparation and bounded snapshot APIs. No renderer environment setting is
needed. The older block adapter remains an explicit diagnostic option.

Installed `release-ui-75buj_b3` verifies the default text surface and exact full
cross-block/formula Copy without a renderer override or application hooks. All
existing language, comparison, Git, viewer, diagram, portal, artwork and recovery
groups pass. Installed `release-large-s7w4g8jh` verifies the real 6,065,514-byte
report and generated 32 MiB LF/CRLF files through normal controls: exact outline
lines, bounded Source/Edit, explicit saves and undo, table paging and columns.
Its outline check uses native keyboard model navigation, allowing GTK to realize
offscreen rows; visible accessibility-child indexes are not treated as model IDs.

All 312 native tests, Rust 1.92 release targets, input/accessibility, rich-paragraph,
translation, retained-workspace and actual-report checks pass. Existing label
geometry and block-model A/B probes explicitly use their diagnostic adapter;
normal-reader acceptance uses installed controls and the continuous pipeline.
See [default-reader validation](gnome-native.md#default-native-markdown-reader).
The full-application compatibility/release audit remains active.

## Previous retained-reader transfer checkpoint

Returning from another workspace pane no longer permanently cancels the retained
document's indexed-table selection. Immutable document validity and each visible
period now have separate cancellation scopes. Hiding cancels old reads/offers and
releases busy state; returning creates a fresh scope without rebuilding the view.
Old completions cannot publish or clear a new request, and old drag offers remain
invalid even when the same view and selection return.

Packaged `workspace-navigation-wdf9ipu7` exercises real image/table/source/PDF/EPUB
dispatch and Close controls, verifies the same TextView and complete 70-row Copy
and primary text after returns, and deliberately holds Copy/comment table replies
across remapping. The ordinary-reader counterpart `workspace-navigation-o9fqpver`
also passes. Private source files retain their hashes and owned processes exit.

All 312 native tests, Rust 1.92 release-target checks, packaged input/accessibility,
rich text, translation including the 32 MiB table, the real 6 MB report and optimized
installed `release-ui-ks9xznsx` pass. The separate
`native-workspace-selection-20261103` candidate changes only the native executable
in the 492-file package; dependency inputs, gateway and renderer retain their
preceding identities. See [retained-reader validation](gnome-native.md#continuous-reader-workspace-return-and-transfer-lifetime).
Continuous Preview remains opt-in pending the remaining compatibility/default
promotion audit; physical/provider release acceptance remains separately recorded.

## Previous native accessibility and selection checkpoint

Full-document formula pictures now supply their source alternative as GTK's
accessible description. The anchored text view preserves embedded child slots in
accessible contents, keeping its count, caret, selection and ranged/line queries
in the same character coordinates. The target-host baseline omitted those slots,
reported 67 characters for a 69-character buffer and cut off the code-line tail
in a counted read. Its executable/source/tree evidence is preserved privately.
GTK continues to own text attributes, geometry and boundary calculation.

Selected opaque objects now show a native accent outline. It clears on selection
clear, buffer replacement and view retirement; light/dark pixel checks verify
painting and unchanged object size. Inline formulas retain the text-selection
background without an inset border over their glyphs. Updates coalesce through
the existing frame mechanism and never poll inactive views or change source text.

The final packaged input/accessibility fixture `flow-input-bfxdloba`, rich-text
`inline-labels-gc9gf738`, translation `translation-n53uwnin`, actual 6 MB report
`flow-pipeline-b9o5ljge` and optimized installed pair `release-ui-w4zwz6ow` pass.
All 310 native tests and Rust 1.92 release-target checks pass. The separate
`native-flow-accessibility-20261103` candidate changes only the native executable
in its 492-file inventory; dependencies/gateway/worker identities are unchanged.
See [native accessible text and selection validation](gnome-native.md#continuous-reader-accessible-text-and-object-selection).

This is private native AT-SPI and rendered-pixel acceptance, not physical Orca,
IME, fractional-scaling or complete compatibility acceptance. Continuous Preview
remains opt-in while the remaining interaction and release gates are verified.

## Latest continuous-reader input checkpoint

Full-document drag export now shares the existing bounded Rust selection exporter.
The prior native GTK drag omitted the inline formula even though Copy and primary
selection preserved it. A frozen failing executable/source overlay and private
Wayland evidence retain that baseline. The new drag offer uses the same projected
plaintext as Copy, preserving formula alternatives, Unicode and code. Indexed
tables resolve through the existing snapshot-scoped worker/API path, independently
of the visible row page. Embedded controls retain their own selection/drag input.

Private `flow-input-rpw6djx0` uses the actual document actor, keyboard/pointer and
a native text-field drop target. It verifies Ctrl+Shift+End selection, Copy,
primary selection, exact drop text, click-to-clear, word/line selection, mouse
selection across blocks, foreign-primary retention and rejection after changed
selection, buffer replacement or Source retirement. A 50-row visible table page
exports all 70 rows. Generated files remain unchanged; no provider prompt or web
content process starts. The text/table drop screenshots were inspected.

The matching packaged `flow-input-pnu02ccg`, existing rich-paragraph regression
`inline-labels-x9xgtje0`, translation/cancellation `translation-9jor4eje`, actual
6 MB report workflow `flow-pipeline-1abjkivv` and optimized installed pair
`release-ui-7xio72uw` all pass. All 310 native tests and Rust 1.92 release-target
checks pass. The separate `native-flow-input-20261103` package changes only the
native executable in its 492-file inventory; dependencies, gateway and renderer
worker retain their prior identities. See [projected drag validation](gnome-native.md#continuous-reader-projected-drag-and-input).

The continuous renderer remains opt-in. This closes a concrete input defect;
it does not establish full assistive-technology, embedded-selection presentation
or format compatibility, or complete the migration.

## Latest HTML comment and translation checkpoint

HTML comments now have shared-store workflow coverage: a real rendered selection
opens the native Comment control, saves the quote/file identity, reloads a fresh
library panel and reopens Source twice. Literal text verifies; entity/inline-markup
and duplicate quotes retain an explicit unverified-location warning. This matches
the existing client's Source/literal reopen model; no original-source positions
are fabricated for rendered HTML. A read-only SQLite check verifies that reopen
has not changed the persisted records, and file bytes remain unchanged.

The workflow exposed a real shortcut defect: the application accelerator consumed
Ctrl+Shift+T before HTML's local key controller, then searched only remembered GTK
text selections. The accelerator now tries the focused widget's scoped native
action first. HTML's menu and shortcut share an action, disabled on selection
clear, document retirement or renderer failure. An empty focused HTML selection
cannot translate selected composer text. Existing native/projected translation and
fake Codex/OpenCode/local-provider checks remain part of the same workflow.

A real private right-click constructs the existing content menu; acceptance
checks and activates its enabled Translate GAction. This does **not** verify
assistive-client or keyboard activation of the menu item. Provisional popup
replacements were removed after probes observed unnamed accessible items in both
implementations. The normal menu implementation remains in place; the wider
accessibility audit must resolve this separately.

The focused private workflow `translation-7p639qww` passes, and its translation
and unverified-location screenshots were inspected. The final packaged workflow
`translation-f6nrfzz2`, passive HTML `html-_k066tuv` and optimized installed pair
`release-ui-fcnzzm2y` also pass. The installed run exercises all existing optional
workflow/recovery groups through real native controls and its launcher. All 310
native tests pass with four opt-in fixtures ignored; Rust 1.92 checks every native
release target. The separate `native-html-actions-20261103` candidate preserves
all 492 package files; only its native executable differs from the prior bundle.
Dependency inputs, gateway and Mermaid worker are unchanged. The default
installation remains untouched and remote CI was not run. See
[HTML action validation](gnome-native.md#native-html-comments-and-focused-translation).
This checkpoint is part of the full migration, which remains active.

## Latest ordinary-file Reload checkpoint

The native file header now offers Reload for identified files, including read-only
shared roots. It rereads the file service and preserves Source/Preview/Edit when
supported. An unchanged snapshot keeps its valid byte page; changed snapshots use
clamped physical-line context, avoiding invalid UTF-8 offsets or an empty terminal
line after shortening. Search resets, and a successful Reload starts a fresh
editor snapshot/undo history. It is not a session catalog/history Refresh.

Dirty pages use Save/Discard/Cancel. Save still performs conflict checks before
rereading; conflict cancellation retains edits. Missing-file/read errors leave
Reload available, and old responses cannot restore their view over another file.
Synthetic/diff content does not receive a working-file target. The Chinese tooltip
is checked against the actual expected text rather than its translation helper.

All 310 native tests and Rust 1.92 release-target checks pass. Private Wayland
`source-reload-4alw39_k` verifies page context, shortened files, save/discard/cancel,
conflict cancellation, delete/recreate retry, read-only/HTML Preview, stale replies
and live language. Source/Edit, editable Find, structured Preview and HTML fixtures
also pass. The optimized installed pair verifies actual external-file Reload
through native AT-SPI alongside every existing workflow/recovery group. Exact
scope and package identity are in [file Reload validation](gnome-native.md#native-ordinary-file-reload).

## Previous HTML checkpoint

Local HTM/HTML/XHTML now has bounded Rust sanitization and a lazy isolated HTML
content surface. Its Preview/Source/Edit controls, outline, Find, comments and
save guards remain native. Project scripts, resource URLs, active attributes,
forms/embedded content and author styles are removed. Independent CSP, disabled
markup JavaScript, ephemeral sessions and denied navigation/permissions/downloads
keep opening a project file separate from website browsing. The original source
and its save operations remain immutable inputs, separate from sanitized HTML.

The focused private Wayland test covers real keyboard Copy, native outline
activation and Find, exact CRLF saves, dirty view switches, a save that inserts an
earlier heading, read-only shared roots, large heading limits, explicit retry
following owned-renderer termination, live language/theme, oversized Source and
stale replies. A live canary with a positive control receives no resource
requests; an application-owned default-world probe independently verifies CSP.
Screenshots are inspected as part of acceptance. The HTML content process owns
DOM layout; the application UI remains GTK4/libadwaita.

All 308 native tests pass with four opt-in fixtures ignored, and Rust 1.92 checks
all native release targets. Root desktop/server manifests and lockfile match the
preceding checkpoint. See [HTML validation](gnome-native.md#native-passive-html-preview)
for exact package and installed-workflow scope. The full migration remains active.

## Previous structured-file checkpoint

Structured Preview keeps immutable original bytes separate from formatted text.
It formats only bounded inputs and preserves literal comments, numbers, duplicate
keys and quoted values. Search addresses display text; comments map back to
original quotes/physical lines. Preview line numbers are explicitly labeled as
Preview coordinates. Dirty view switches keep edits in memory until explicit
save or page-change save.

Actual Wayland acceptance exposed a 7.7-second GTK stall on a 4 MiB single-line
JSON file. Merely disabling syntax paint did not stop the language context
engine, and detaching the engine alone did not remove the text-layout stall.
Ordinary-file windows now admit at most 16 KiB / 200 lines, plus up to two bytes
to keep CRLF intact; unsafe syntax pages detach their language until safe again.
The separate indexed Markdown reader keeps its own bounds. Repeated private
Wayland runs paint the long-line context in 489–538 ms with maximum sampled
main-loop gaps of 129–134 ms. These are host samples, not latency guarantees.

Those runs and the final package check verify actual-frame viewport visibility,
cross-page Find,
original-source comments, exact Copy, dirty switches, explicit/page-change save,
Undo/Redo, read-only shared roots, stale replies, narrow panes and live language/
dark changes. Syntax colors use native GtkSourceView; read-only Preview does
not mark missing page context as a syntax error. No web-content process or real
provider prompt is started by these structured-file checks.

The shared reveal correction also covers Source/Edit returns and search: two
consecutive editor runs and the final packaged editor/search checks pass. All
302 native tests and Rust 1.92 release-target checks pass. The separately packaged
optimized pair passes every installed workflow group, including actual launcher
Preview/Source/Edit and exact original-file saves. The previous bundle and default
installation are preserved.

See the [structured Preview validation](gnome-native.md#native-structured-file-preview)
for package and broader-regression scope. Static HTML and continuous-reader
promotion remain separate work; this checkpoint does not complete the migration.

## Previous selection-lifecycle fix

Continuous-reader Translate now owns a cancellable request scoped to its actual
selection. Changing/clearing that selection, modifying/replacing its buffer or
unmapping its view cancels the read and releases the action immediately. An old
future finishing cannot clear a newer request. This does not cancel the shared
document snapshot or another selection operation.

The two request-lifecycle tests and private Wayland
`translation-zzkt7mmd` pass. The latter uses a 33,554,550-byte table, actual
Ctrl+Shift+T and cursor input, paused table delegates, and the fake-provider
translation/cache workflow. It proves that cancellation drops the paused
receiver before release, and that a replacement selection succeeds. Existing
Codex/OpenCode/local-provider, PDF/EPUB/browser, speech and cleanup checks pass
in the same run. This fix does not promote the experimental renderer or finish
the migration.

All 301 native tests pass (four opt-in fixtures ignored), all native release
targets check on Rust 1.92, and debug/release builds are packaged separately as
`native-selection-cancel-20261103`. Its gateway, worker, dependencies and other
packaged files match the previous candidate. The broader installed workflow
results still refer to `native-flow-prefix-20261103`; they were not rerun for
this focused opt-in adapter fix. Both bundles and the default installation are
preserved. See the [selection-lifecycle follow-up](gnome-native.md#selection-lifecycle-follow-up).
