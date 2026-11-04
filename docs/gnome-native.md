# Native GNOME frontend

This branch introduces a separate Rust GTK4/libadwaita application for Ubuntu
26.04 GNOME/Wayland. Application controls, conversations and Markdown use native
widgets. EPUB XHTML/CSS and browser pages use isolated GTK4 WebKit content
surfaces; their toolbars, tabs, contents, search and settings use GTK widgets.
The existing Tauri application remains the default build on all platforms.

The goal is full application migration. Current work follows the
[native migration plan](gnome-native-plan.md). The concise
[current parity inventory](gnome-native-status.md) distinguishes present gaps
from the historical checkpoints below.

## Native status, permissions and inspector controls

The native command palette, session status, message-permission choices and
confirmation, and diagram inspector now bind their interface text explicitly.
Command names, backend/session/project IDs, model/effort/tier values, gateway
version, usage JSON and diagram kinds remain literal. Rendered dimensions use
named numeric parameters. The existing source/generation guards, permission
confirmation, bounded raster worker and zoom lifecycle remain unchanged.

All **395 native tests** pass (four opt-in ignored), and the debug binary builds.
Installed private Wayland `release-ui-rwaml606` passes the expanded
`scripts/smoke-gnome-release.py PACKAGE --language` check without application
hooks or a gateway override. It verifies English/Chinese command search and
status fields, selected inheritance, translated destructive confirmation and
Cancel without changing permissions, plus actual diagram zoom/fit in both
languages. Chat draft/source bytes, native history/comparison/image controls,
file portals and save behavior remain covered. Owned providers exit normally;
no web-content process is started.

Earlier attempts selected MCP or waited for an impossible single result because
the query `status` also matches its English description. The fixture now uses
the literal slash command and waits for the asynchronous filter before Enter.
Those failed artifacts remain alongside the passing run. License snapshots,
native/shared formatting, interface catalog and shell syntax checks pass.

The checkpoint resides in private `native-status-inspector-20261104`. Its
optimized build, Rust 1.92 check and expanded installed release validation are
still in progress. The previous optimized reader-controls candidate and default
installation remain unchanged. This does not finish the full migration; see
the current inventory for release, renderer and physical-desktop validation.

## Native reader, search and terminal language controls

Tables, EPUB, conversation search and Terminal now bind their owned labels,
menus, tooltips and status messages explicitly. SQL, columns/cells, snippets,
chapter/book metadata, paths, shell output and backend error details remain
literal. Native search-role markup keeps the existing escaped snippet fragments.
Table/terminal error toasts also treat their messages as plain text. Computation,
indexing, search, SQL and document decoding remain on the existing Rust workers.

The shared adaptive string selector now has an explicit UI-choice variant with
optional translation context. It keeps the existing bounded label, wrapped popup
and selected marker; ordinary model/name selectors stay literal. Delimiter Tab is
“制表符” and EPUB layout Pages is “分页”, with unchanged underlying model values
and selection indexes. SQL and EPUB search fields have explicit accessible names.
Reading settings retains its own accessible name when its popover is open, rather
than taking the concatenated option text.

An actual resize-during-entry check reproduced the EPUB section field losing an
unsubmitted number: asynchronous layout/save refreshes unconditionally reset it.
The field now refreshes only the value last displayed by the controller. Enter
commits navigation, leaving the field restores the current section number, and a
new document resets the input. The existing document generation, CFI and read-only
save/ownership guards remain unchanged. The pre-fix assertion and real mapped
before/after input captures remain in the private evidence directory.

`scripts/smoke-gnome-control-language.py PACKAGE` performs a real private-prefix
installation and uses its sibling gateway without application hooks. Optimized
`control-language-khtxuxvz` passes all four areas: SQL buffer/result/header preservation, localized
row counts and contextual delimiter options; literal trailing-space conversation
queries and a retained User filter/result snippet; real PTY keyboard input with
the same shell PID across language changes and cleanup after Close; and chapter 2,
paged EPUB layout, resize-safe section entry and book search in both languages.
Source bytes and the Chat draft remain unchanged; no inference is sent.

Book search is modal, so its normal parent-window Settings shortcut is blocked.
The fixture closes that dialog to change the reader language, then reopens and
explicitly searches the retained query. It verifies both rendered language forms
without claiming a live modal-row identity check. `--epub-only` is a diagnostic
subset and reports `complete: false`; it cannot replace the default full pass.
Separate debug `--capture` uses only the passive window observer. Normal installed
acceptance uses desktop activation and no debug observer. Pointer input is confined
to the verified full private monitor after the maximize transition, and actual
entry focus is checked before typing. Earlier CSV-schema, unmapped controls,
unsupported AT-SPI focus, nested popover and keyboard-close probes are retained.

All **395 native tests** pass (four opt-in ignored), with Rust 1.92 all-release-target
checks. Matched debug/release packages pass 20 layout cases, the full installed
control workflow, existing expanded language/viewer/backend/connection checks and
Router unread acceptance. The gateway and existing frontend source/test inputs
plus both gateway binaries are unchanged from the previous checkpoint; its 264
gateway tests (one ignored), 749 web tests and Tauri Linux check are inherited,
not rerun. This batch does not claim every real-provider or physical-desktop path.

```bash
GDK_BACKEND=wayland ./target/gnome-native-candidate-20261104-reader-controls/codex-thread-studio-gnome
```

Private `native-reader-controls-language-20261104/READER-CONTROLS-EVALUATION.json`
retains source, package and evidence identities. Previous candidates/default
installation remain unchanged. Next complete the status/inspector and remaining
control audit, review and commit the migration sources, then finish the renderer
and release audit. Explicit pending-alignment wheel interruption and separately
documented physical GNOME/IME/Orca/real-provider checks remain. The full goal stays
active.

## Native session and image language controls

New session now binds its heading, directory instruction, Create/Cancel responses,
project-entry accessible name and folder-picker title. The image workspace and
inline preview bind their owned toolbar, loading/save status, dimensions, fallback
captions and file-dialog titles/filter. Filename/path/alt-text values and backend
error details remain literal. Explicit title keys distinguish generated Attached
image/Embedded image captions from actual user filenames. Zoom and decoded source
identity are independent of language. No parsing, decode, file-save or ownership
logic moves into GTK.

Expanded `scripts/smoke-gnome-release.py PACKAGE --language` passes from the private
installed debug package in `release-ui-990xhq9q`, without application hooks or a
gateway override. The existing history/composer and nested agent-input language
workflow is preserved. A separate image-only workflow changes language while an
image is open, verifies the retained 125% zoom, localized dimensions/tooltips,
literal Unicode/template-like filename and unchanged original bytes, exercises the
translated zoom action and returns to the existing reader. New session is opened
and cancelled in both languages with an unchanged literal project path and Chat
draft. All six Open/Save/folder portal cancellations verify actual localized
Wayland-parented requests, image filter name and original suggested filename.
The real English image export and source-file picker still pass afterward.

Earlier probes wrongly sought conversation controls in the intentional image-only
workspace. GTK removes those unmapped controls from the accessible tree. The test
now keeps the original modal/history/composer workflow and tests image language
changes separately, then checks restored reading state. The failed probes remain
in the private evidence directory; no product navigation or visibility behavior
was changed for them.

All **395 native tests** pass (four opt-in ignored), with Rust 1.92 all-release-target
checks. Matched debug/release packages pass 20 layout cases. Optimized installed
`release-ui-1x2blan9` passes the new language/portal checks together with the existing
viewer group: image/SVG/animation, full-cell image handoff, PDF language/search/copy,
EPUB and passive HTML, structured/source editing and returns. The installed Router
unread workflow also passes with source/paging/read-receipt invariants intact.
Gateway/web/Tauri evidence is inherited after unchanged gateway/frontend input
and gateway binary verification; this batch does not rerun every provider/Map or
real-desktop workflow.

```bash
GDK_BACKEND=wayland ./target/gnome-native-candidate-20261104-session-image-language/codex-thread-studio-gnome
```

Private `native-session-image-language-20261104/SESSION-IMAGE-EVALUATION.json`
retains source/package hashes and exact workflow scope. Earlier candidates and
the default installation are unchanged. Further source audit confirms fixed
controls still need bindings in Tables, EPUB, conversation search and Terminal;
status/inspector controls also need review. The audit's string matches are leads,
not a proof of all remaining gaps. Explicit wheel-interruption acceptance and the
full renderer/release audit remain. Physical GNOME/IME/Orca and real-provider
checks are recorded separately. The full migration goal stays active.

## Native Router unread turn cues

The Turn navigation popup now marks unread native Router responses with a symbolic
mail indicator. It uses only the existing owned terminal-job metadata for the
actual displayed controller/page. Read-only browser history and ordinary sources
show no native unread cues. The complete accessible button name and tooltip add
“Unread response”; live language changes translate interface fragments while
preserving literal question text, Unicode and template-like braces.

Acknowledgements update bound buttons in place. A reserved indicator cell retains
row geometry, keyboard focus and the open popup. No provider read, inference or
read-receipt call is added by the cue code. Existing actual-visible-response-end,
active-window, dwell, source ownership and receipt checks remain authoritative:
jumping to a question alone cannot mark its unseen response read.

`scripts/smoke-gnome-router-unread.py NATIVE GATEWAY --installed` passes in
`router-unread-zt1qx5at` against the optimized package, with an actual Unicode/space/percent
private-prefix installation, sibling gateway, no application hooks and no real
provider. Twenty-five private terminal jobs force two actual Router pages. The
fixture verifies initial cues, metadata updates without replacing buttons or
moving geometry/focus, Chinese/English switching with literal prompts, actual
question viewport overlap without an acknowledgement, and a receipt only after
search exposes the response end. Earlier-page cues, ordinary-source switching,
restored owned cues, untouched foreign-controller unread state, unchanged browser
archive rows and preserved composer drafts also pass. One explicit private SQL
update simulates another client's metadata change; that step tests presentation,
not native read semantics. Actual response-end receipt is tested separately.

Separate debug `router-unread-25mhvn5n` passes without application hooks; passive
capture `router-unread-v2tv1tkv` records before/after acknowledgement and Chinese
popups. These images were inspected: the indicator disappears without shifting
rows or the focus outline. Capture uses the existing passive debug observer;
installed acceptance does not enable it. Earlier failed probes are retained:
Router search uses a different control from ordinary conversation find, its
AT-SPI entry name is the literal-search hint, and GtkDropDown exposes the selected
source value rather than its assigned history-picker label. Test selectors were
corrected; no application visibility/read guards were weakened.

All **395 native tests** pass (four opt-in ignored), and Rust 1.92 all-release-target
checks pass. Matched debug/release packages pass all 20 layout cases, installed
unread controls, ordinary prompt navigation, Codex/OpenCode three-page history and
source restoration, and core/live-language controls. The 168 gateway source and
380 existing frontend/test inputs, plus both gateway binary hashes, match the
previous qualified checkpoint: its 264 gateway tests (one ignored), 749 web tests
and existing Tauri Linux check are inherited, not rerun. Earlier Continue, Map,
viewer and fresh-runtime evidence remains recorded separately; this batch does
not claim those entire workflows were rerun.

```bash
GDK_BACKEND=wayland ./target/gnome-native-candidate-20261104-router-unread/codex-thread-studio-gnome
```

Private `native-router-unread-qualified-20261104/UNREAD-EVALUATION.json` retains
source/package identities, installed results and inherited-check provenance.
Previous candidates/default installation remain unchanged. The next control
audit has identified fixed-English New session dialog and image-viewer controls;
these need explicit live bindings without translating user paths or image names.
Pending-alignment wheel interruption, the broader renderer/release audit and
separately documented physical GNOME/IME/Orca/real-provider acceptance remain.
The entire-application goal is active.

## Native turn navigation

The conversation status line now has a compact **Turn navigation** control. Its
current/total count follows actual viewport geometry; the popup lists user-input
previews for the displayed page. The list is virtualized, width-bounded and
independently scrollable. Prompts are named native buttons with complete preview
tooltips, including literal Unicode and braces. Attachment-only inputs retain the
numbered fallback. The control is hidden when fewer than two prompts are available.

The shared bounded Rust pool prepares 72-character whitespace-compacted previews.
A latest-input watch channel finishes one CPU job before taking newer input;
streaming assistant updates reuse unchanged user handles. There is no provider
read, inference, source mutation or full-message JSON transfer for this navigation.
GTK measures only bound/mapped rows for the current marker. Button actions verify
the source backend/thread/generation, page serial and actual user turn/item before
jumping through the existing transcript navigation coordinator. Search and history
remain separate existing controls.

An actual middle-turn test exposed GTK's default ensure-visible behavior: it could
place just the header at the bottom and leave the prompt outside the viewport.
The shared coordinator now follows realization with a bounded, generation-scoped
alignment using the real header allocation. New navigation, reset and user wheel
input retire pending alignment. This preserves the existing reading/follow-tail
coordination. Earlier bare GtkListView rows exposed labels but no activation
action; each virtualized prompt now has a named native Button instead.

All **395 native tests** pass (four opt-in ignored), with Rust 1.92 all-release-target
checking. `scripts/smoke-gnome-turn-navigation.py NATIVE GATEWAY` uses actual
AT-SPI and owned Wayland input without application hooks. It deliberately supplies
the explicit debug gateway path. Private `turn-navigation-0obj0c9a` verifies
first/middle/last prompt viewport overlap, first/last current highlighting, named
activation, keyboard activation, literal preview text, preserved draft/source
history, no inference and owned-provider cleanup. GTK's plain user Labels and
rich TextViews are both valid surfaces; geometry rejects merely preloaded rows.
The preceding unnamed-row, keyboard/focus and text-role probes are retained with
the actual pre-alignment failure. Existing debug-hook streaming regression
`streaming-ui-inz5t_7s` passes independently: 80 deltas, stable paragraph/table/diagram
widgets, retained draft and stale-session rejection; first rich output 105 ms,
maximum event work 2 ms, reconciliation 34 ms and heartbeat gap 312 ms. Native
peak RSS is about 368 MiB and gateway about 44 MiB in this fixture. These are
fixture observations, not a large-history performance guarantee or turn-navigation-
during-streaming acceptance.

The scope fixture `scripts/smoke-gnome-turn-navigation-scope.py NATIVE GATEWAY
--backend codex|opencode` adds three actual history pages, Newer/Live, source
switching and draft restoration. Codex also exercises live input while reading an
older page, verifies actual prompt viewport overlap during a live delta, and holds
an old history reply until after selecting another source. The new source retains
its own prompt list when that reply arrives. Event handoffs are atomic and record
publisher failures; assertions wait for actual enabled native actions, not an
assumed latest-answer visibility or a draft-restoration timing shortcut.

Both fixtures support `--installed`, which performs a private-prefix installation
with Unicode/space/percent characters in the path and uses the sibling gateway
without `CODEX_THREAD_STUDIO_GATEWAY_BIN`. Installed `turn-navigation-d6iru99e`,
`navigation-scope-codex-gtoin7j6` and `navigation-scope-opencode-kzm3jkqa` pass against
the optimized pair, without application hooks or real providers. Original source
bytes and drafts remain unchanged and navigation starts no inference.

Separate debug `--capture --narrow` and `--capture --narrow --large-type` runs use
actual 640 × 900 windows. A reusable passive capture observer records the mapped
popup content separately because a parent-window snapshot omits a Wayland popup
surface. No widget allocation or synthetic frame signal is used. The first
captures exposed the popup ignoring the configured interface font. Shared
appearance CSS now applies the same interface font to `window, popover`, while
content/code typography retains its own roles. Normal/large heading heights are
18/24 px and prompt-label heights 22/29 px; popup widths 342/534 px fit the window.
Both `turn-navigation-jw5k9xji` and `turn-navigation-_8q760xu` pass actions/keyboard/
draft checks, and their popup and window captures were inspected. These capture
runs explicitly use passive debug hooks; installed acceptance does not.

Matched packages pass `layout-ui-3oel4_ek` (all 20 cases), installed Continue
`continuation-ygz7ra5i` (private Ollama proxy/explicit gateway override), installed
core/language `release-ui-ps29boc_` and actual private-prefix Map
`release-map-7nmw5rr3`. The Map check now installs first rather than calling a
package path an installation. Its initial immediate-PID assertion raced normal
asynchronous shutdown; authoritative follow-up found the process gone. The test
now observes bounded cleanup. Earlier cursor/header, action-readiness, source-title,
event-handshake and narrow document-only fixture probes are retained. The HTTP
cursor fixture uses an ASCII percent-encoded opaque header. A 640 px document
window intentionally hides the conversation, so narrow navigation is tested in
conversation mode. No product loading/read guards were weakened for those probes.

All 395 native tests pass (four opt-in ignored); Rust 1.92 all-release-target and
existing Tauri Linux checks pass. Gateway source plus debug/release binary hashes
match the preceding Continue checkpoint, whose 264 gateway tests (one ignored)
are inherited. The unchanged existing frontend's 749 tests are also inherited.
This focused batch does not rerun every optional viewer or real-provider workflow.
The latest optimized candidate is:

```bash
GDK_BACKEND=wayland ./target/gnome-native-candidate-20261104-turn-navigation/codex-thread-studio-gnome
```

Private `native-turn-navigation-qualified-20261104/NAVIGATION-EVALUATION.json`
retains source/package/evidence identities and failed probes; the earlier debug
record remains available. Previous candidates and the default installation are
unchanged. The next confirmed parity gap is Router unread cues in prompt rows:
the web navigator has them, while native metadata/counts/read receipts are present
but the new list does not yet display them. Preserve ownership, visible-end read
semantics and the read-only browser archive when adding those cues. Explicit
pending-alignment wheel interruption, remaining compatibility/release acceptance
and physical GNOME/IME/Orca/real-provider checks remain separately recorded. The
entire-application goal stays active.

## Native composer Continue and message flow

Settings → Sessions → Message flow edits the existing shared `queueDepth` (one,
two or three) and `continueBehavior` values. The authenticated native endpoint
reads defaults without writing and saves explicitly with a scoped revision.
Unrelated/future settings survive; invalid values and conflicting writes leave
bytes unchanged. A final full-document check rejects external edits that arrive
while a save is staged. Dirty settings retain their close/discard guard.
Lowering the limit preserves existing queued messages and does not replay them.

The native composer now has **Continue**, with Ctrl+Shift+Enter active only while
the composer has keyboard focus. The saved behavior selects:

- **Session-model draft:** generate a structured next-message prompt in an
  isolated utility, preserving the source's effective model, effort and Fast
  tier. Codex uses an ephemeral read-only, no-approval session; OpenCode uses
  its tool-denied utility and exact ownership lease. Only the utility is deleted.
- **Ollama draft:** use the saved local Ollama model independently of the
  translation engine and place the validated result in the composer.
- **Quick send:** explicitly send one of the existing short continuation phrases
  through normal source submission; the two draft behaviors never auto-send.

Draft generation requires a verified idle normal session, loaded options/queue
and an empty composer without pending IME text, attachments, references or queued
messages. It captures source/backend/generation, model options, response identity
and composer revision. Typing or changing context cancels; stale results cannot
replace or send text. **Cancel draft** remains **Stopping draft…** until utility
cleanup settles. Local cancellation stops waiting and rejects the reply; it does
not claim that upstream Ollama computation has been aborted. An external behavior
change cannot silently switch an admitted draft action into Quick send.

Rust workers prepare bounded source text (32,000 Unicode characters) and validate
the structured result. Shared continuation instructions/schema retain the web
contract and treat assistant content as quoted input. Shared translation and
OpenCode utility paths retain their prior cleanup semantics.

`scripts/smoke-gnome-continuation.py NATIVE GATEWAY --installed` installs a private
matched package and exercises actual AT-SPI/Wayland controls. It checks saved
settings/conflicts, dirty-close protection, retained over-limit queues,
session-model and local drafts, held HTTP cancellation, typing during inference,
cancellation before creation acknowledgement, effective model/effort/Fast,
explicit quick-send and shortcut scope. Original Codex/OpenCode source records
remain unchanged except for the one explicit Quick send. Exact utility deletion
and owned-process cleanup are verified. The fixture uses no application hooks;
it deliberately supplies a gateway override/private proxy for local Ollama.
`--api-only` checks authentication, origin, defaults, scoped preservation,
conflicting revisions and invalid values. No real provider is contacted.

All **394 native tests**, **264 gateway tests** and **749 web tests** pass (four
native and one gateway opt-in tests ignored). Rust 1.92 all-release-target checks
and the existing Tauri Linux check pass. The first parallel gateway run hit an
existing three-second Router test deadline during filesystem contention; the
focused rerun and complete two-thread run pass without weakening the deadline.
Private fixture mistakes (missing items-page answers and premature creation
release) are retained alongside the corrected authoritative checks. The installed core
fixture also raced asynchronous provider cleanup immediately after native exit.
It now observes PID disappearance within ten seconds and records the duration;
the original failure and incorrect live-application wait-helper probe are retained.
No application shutdown behavior was changed to satisfy this test.

The 20-case layout regression now measures continuous TextView body/tag typography
rather than requiring the previous Label renderer. That independently exposed
continuous Markdown missing the shared content-style class: a configured 24 px
body inherited the 20 px interface font. The class is restored; all large-type
cases measure 24 px body and a 40.8 px bold heading. Normal and large-type mapped
captures were inspected. This fixes content typography without moving rendering
back to the legacy adapter.

Matched debug/release packages pass `translation-zaaeqana`, `layout-ui-s_o40q76`,
installed `continuation-xrrrdwpo` and core/language `release-ui-hy_p9sv1`. The
core run observes provider cleanup 51 ms after native exit. Package
checksums and source/evidence identities are recorded in private
`native-composer-continue-font-20261104/CONTINUATION-EVALUATION.json`. This focused
batch does not rerun every optional viewer, Git, recovery or diagram group from
preceding checkpoints. New test artifacts use the external dependency disk via
optional `STUDIO_NATIVE_QA_DIR`; application defaults are unaffected.

The optimized local candidate is:

```bash
GDK_BACKEND=wayland ./target/gnome-native-candidate-20261104-composer-continue/codex-thread-studio-gnome
```

Previous candidates and the default installation remain unchanged. Native turn
navigation, remaining compatibility/release acceptance and separately recorded
physical GNOME/IME/Orca and real-provider checks remain. The full goal stays active.

## Native Router language and HTML menus

Router history captions and sidebar activity/pin prefixes now use explicit live
language bindings. History/search/count/unread phases and toolbar hints switch
between English and Chinese while session names, source text, query spaces,
selections and drafts remain literal. The private language fixture deliberately
names a session `Search results {count}` to catch accidental translation or
interpolation of user data. It checks zero/one/many results, pending search,
source selection and returning to the existing archive/current view.

Passive HTML now uses the shared GTK popup containing named MenuItem buttons,
retaining its GMenu labels, GAction identities and enabled state. Copy and Select
all invoke WebKit's editing commands; Translate uses the existing scoped native
action. The Menu key and Shift+F10 open it. Selection updates refresh Copy's
enabled state, and retiring/unmapping a reader closes/unparents the popup.
Action callbacks verify the original reader epoch. This changes application
controls, not HTML sanitization, the CSP or content-process isolation.

The actual right-click acceptance exposed another error: the event's `(783,255)`
surface position was passed directly to a popover expecting the WebView-local
point near `(32,146)`. The reusable adapter now obtains the event's native surface,
applies its surface-to-widget translation and computes the attached-widget point.
This follows [GDK event coordinates](https://docs.gtk.org/gdk4/method.Event.get_position.html),
[the native surface transform](https://docs.gtk.org/gtk4/method.Native.get_surface_transform.html)
and [GTK widget conversion](https://docs.gtk.org/gtk4/method.Widget.compute_point.html).
The regression compares the actual popup anchor with an independently chosen
DOM point after a real owned Wayland right-click. Keyboard-only menu tests would
not detect this mistake.

`scripts/smoke-gnome-html-menu.py NATIVE GATEWAY --installed --translate` uses
AT-SPI and owned Wayland input, without application hooks or a gateway override.
It activates named Select all, Copy and Translate controls, reads the independent
clipboard, checks empty-selection admission, verifies the exact private utility
input/delete and retains original HTML/draft bytes across Source/Preview switches.
The separate `--capture` debug run is mutually exclusive with `--installed`.
`scripts/smoke-gnome-router-archive.py NATIVE GATEWAY --installed --language`
exercises the bilingual history/sidebar paths and unchanged saved records.

All 393 native tests pass (four opt-in ignored), with Rust 1.92 all-release-target
checking and fresh matched native debug/release packages. Gateway source and both
binary hashes are unchanged from the qualified archive checkpoint; its 260
gateway tests (one ignored) are inherited. Private `translation-mpgf7nc6` passes
the new pointer-anchor guard and existing translation/speech/projected-selection/
HTML-comment workflow. `html-xkad6wxc` passes passive HTML safety/editing, and
`session-map-z8e5djc1` passes Map controls/isolated utilities/restart.
Installed `html-menu-0x4ed2sp`, `router-archive-ok524weu`,
`release-map-af5hp8ia` and `release-ui-jq9v9uie` pass the menu/language, named shell
keyboard/dialog-focus and language/core-reader groups. These installed checks
use private providers and actual native controls, without application hooks or
a gateway override. This focused batch does not rerun every optional viewer,
Git, recovery or public-image group from the preceding unchanged implementations.

`bundle-install-py6rhyma` passes standalone installation/update retention,
unrelated-tool canaries, no-Python installer and corruption rejection. The new
release also runs `--help` against the read-only extracted official Ubuntu 26.04
runtime, without host system files, source checkout, display, network or backend.
That proves library loading/CLI startup, not a configured fresh GNOME session.
The initial invalid-empty-model translation fixture and reproduced pointer
failure are retained alongside the successful checks. Earlier painted language
and translation captures were inspected; window-widget captures do not establish
popup-surface pixels, so menu evidence uses actual AT-SPI actions and geometry.

The optimized local candidate is:

```bash
GDK_BACKEND=wayland ./target/gnome-native-candidate-20261104-native-menus/codex-thread-studio-gnome
```

Private `native-language-html-menu-pointer-20261104/MENU-EVALUATION.json` retains
source/package/evidence identities. Previous candidates/default installation
remain unchanged. The full migration remains active. The next concrete gaps are
native message-flow settings and Continue (session-model/Ollama draft and explicit
quick-send), followed by the compact turn navigator; native supervision and
transcript search do not replace those user actions. Remaining compatibility and
release acceptance, and separately recorded physical/real-provider checks, still
apply.

## Native browser Router history

The Router toolbar now offers **Current requests** and **Earlier browser
requests**. The first view retains native execution/queues/supervision; the second
reads existing browser-created records from `router_dispatches`. Switching the
view preserves the composer draft, and both views reuse native paging/search.
Sending a new Router request returns to Current requests. Earlier browser
requests show their saved delivery status and bounded source previews; they do
not become native delivery intents or acquire monitoring/Stop/supervision rights.

`GET /studio/native/router/browser-history` is authenticated and read-only. It
returns indexed pages of at most 20 metadata rows, with older/newer cursors and
decoded literal Unicode search over the saved metadata. Spaces remain significant.
Listing does not start a provider, recover a job or load hidden reply trees.
Search uses a read-only WAL snapshot and a two-second budget. Native mirror
records are excluded only when their native ID, controller and mirror key agree;
orphans and unrelated records remain readable. A bounded row/key digest prevents
an old identity from aliasing a subsequently reused SQLite row. Original records,
unknown properties and unread flags remain untouched.

Visible-page detail reads use two bounded readers. An explicit browser request
retains its saved original question; routed questions are read from the exact
controller turn. Accepted target questions come from their actual source turn,
so comments/favorites do not inherit a stale forwarded prompt after a continuation.
Codex history uses older four-turn pages; OpenCode keeps its verified directory
and `before`/`x-next-cursor` contract across message pages, including response parts
on a newer page than their user message. Structured OpenCode output uses the
existing message-derived identity. The shared preview projector retains actual
part IDs and UTF-8 boundaries under its 32 KiB text budget. Foreign conversation/
message IDs and repeated cursors fail explicitly. Metadata is checked again after
hydration so a changed record cannot silently attach another source reply.

Previews walk at most 64 pages, with 15-second budgets for the original question
and target read. Unavailable/long previews show an explicit notice and a source
conversation action when its identity is available. They do not fall back to
starting/resuming a turn. The search field searches saved routing metadata;
provider replies fetched for display are not indexed by this archive search.
Refresh reloads the current source previews; background metadata polling does
not repeatedly hydrate unchanged replies. Archive rows are excluded from native
supervision admission, queue gates and automatic read-receipt writes.

`scripts/smoke-gnome-router-archive.py NATIVE GATEWAY` seeds 55 private browser
records and a native mirror, then exercises the API and actual GTK controls through
AT-SPI/owned Wayland input. `--api-only` needs no compositor; `--installed` checks
the installed optimized pair without application hooks or a gateway override.
`--capture` is a separate debug-only visual run: its passive capture observer
does not alter widgets, focus, controls or model state. These two flags are
mutually exclusive. All providers, directories and database contents are private
fixtures, and every read is checked against the unchanged shared record snapshot.

Private `router-archive-ad2vc19m` passes original/direct questions, older controller
and target pages, Codex/OpenCode sources, responses split across pages, structured
output, exact accepted questions, malformed/repeated cursor and foreign-part
guards, literal search, native selector/return and preserved drafts. Its metadata
pages measured about 3–5 ms and deeper source reads about 63–141 ms on this host;
these are fixture samples, not general latency guarantees. The GTK probe exposed
several fixture assumptions: the selector exposes its selected value as its name,
its popup is inside its accessible subtree, and list items require keyboard
selection rather than an AT-SPI activation action. The final driver focuses the
selector, waits for the mapped popup, then uses Home/End and Enter. Failed probes
are retained. There are no application hooks in the ordinary acceptance run.

All **393 native tests** (four opt-in ignored) and **260 gateway tests** (one
opt-in ignored) pass. Rust 1.92 and the existing GTK3/Tauri Linux check pass.
Matched optimized binaries are now packaged. Installed
`router-archive-s7zop9eg` passes the archive workflow through AT-SPI and owned
Wayland input without application hooks or a gateway override. Packaged
`router-archive-kqq25z3m` separately adds passive screenshots of actual mapped
widgets; these were inspected. Router execution `router-runtime-9vxlhwj9`,
supervision restart `supervision-ui-8fj2ptmd` and the existing installed groups
(`release-ui-2wztnlcj`) all pass against the matched package. Those installed groups
include diagrams, language, backend configuration, Git, viewers, process recovery
and event-gap recovery; this run does not exercise remote HTTP image fetching.

The local optimized candidate is
`target/gnome-native-candidate-20261104-router-archive`:

```bash
GDK_BACKEND=wayland ./target/gnome-native-candidate-20261104-router-archive/codex-thread-studio-gnome
```

The private `native-router-archive-20261104/ARCHIVE-EVALUATION.json` record retains
source/package/evidence hashes, screenshots and failed probes. Previous testing
candidates and the default installation remain unchanged. The full migration
remained active at this checkpoint. The newer language/menu checkpoint above
closes its caption/sidebar and HTML-menu gaps; the current parity inventory lists
the remaining user-facing work.

## Native supervision restart and installed controls

Target-side opt-in now resolves Router ownership through the authenticated
`POST /studio/native/router/supervision-scope` endpoint before loading options
or starting the supervisor. It echoes the exact backend/thread/turn/project and
returns either an explicit unrouted result or the owning job/controller. SQLite
uses its source index and a read transaction; only metadata for at most two rows
is projected. Missing claims, ambiguous ownership, wrong projects and sticky Stop
fail explicitly. The lookup neither recovers jobs nor calls a provider, and does
not deserialize or transport complete replies. Map readiness remains a separate
preparation gate.

While discovery is pending, the native UI holds the source and provisionally
holds configured Router automatic queues. Once resolved, only the actual
source/controller pair stays held. Stopping during discovery waits for that
identity, then pauses the pair without starting an evaluator. A failed lookup
does not persistently pause unrelated Router queues. Terminal status applies
only to the matching current turn, project and connection generation; an earlier
completion notification may still appear as its own toast.

The native suite passes **392 tests** (four opt-in ignored), including eight UI
ownership tests. The headless gateway passes **257 tests** (one opt-in ignored).
Rust 1.92 all-release-target checking and the existing GTK3/Tauri Linux check pass.
Both optimized executables have been built and packaged with the existing
verified optional runtimes. Ordinary supervision, broad native controls and both
Map workflows pass against the matched debug package.

`scripts/smoke-gnome-supervision-scope.py GATEWAY` proves after gateway restart
that ownership lookup preserves the stored record and provider ledger, makes no
provider/recovery call and returns only small authenticated metadata. The native
`scripts/smoke-gnome-supervision.py NATIVE GATEWAY --restore` fixture opens the
target first, asserts that full Router projections remain unloaded, starts
supervision through the real menu, and verifies provisional/resolved parent
queue holds, continuation, durable handoff and queued release. The ordinary
eight-source/five-evaluator workflow also passes, including evaluator cancellation
and current-turn menu state.

`scripts/smoke-gnome-release-supervision.py RELEASE_PACKAGE_DIRECTORY` installs
the optimized pair into a disposable prefix and uses only AT-SPI/owned Wayland
input. Installed `release-supervision-t18zqm1l` verifies cold target-side opt-in,
visible progress, authoritative utility output, exact continuation, preserved
draft, Fast, held/released queues, durable Router handoff, absence of stale status
in the new turn's menu and Stop without queue/evaluator restart. Its ledger
contains four source turns, two evaluations and two owned utility cleanups.
There are no application test hooks or gateway path overrides. Two earlier
fixture failures are retained: one matched a legitimate completion toast outside
the current menu, and one expected the generic Paused label instead of the
specific stop reason beside Resume. Their assertions now inspect the actual
current row and paused-queue controls; production code did not change for them.

Installed `release-ui-cdq9n57e` also passes all requested existing groups against
the same optimized package: four diagram engines at 4× zoom, live language and
history controls, backend configuration, Git, document viewers, process recovery
and recovery from 13,699 missed events. Remote HTTP image fetching was not part
of this run. Both installed tests use private profiles, provider scripts and file
copies. The private `native-supervision-scope-20261104/SCOPE-EVALUATION.json`
record retains source/package/evidence hashes and earlier fixture failures.

The local optimized testing candidate is
`target/gnome-native-candidate-20261104-supervision`:

```bash
GDK_BACKEND=wayland ./target/gnome-native-candidate-20261104-supervision/codex-thread-studio-gnome
```

It uses the normal Studio profile; tests used disposable profiles instead. The
default installation and older testing candidates have not been replaced.

At this checkpoint a compatibility probe found that browser-created Router
records were intact but absent from native history. The browser-history reading
view described above closes that gap.
Remaining control/accessibility and final distribution/desktop checks also remain.
The historical controls/backend checkpoints below retain their original evidence.

## Native turn-supervision controls

This earlier debug checkpoint precedes restart and installed-control qualification
above.

The composer now has a native **Turn supervision** menu. It lists the current
running Codex turn, running Codex targets in the selected Router, and owned
supervisors that may need Stop while another conversation is selected. Opt-in
is explicit; loading a conversation never starts evaluation. A highlighted menu
button and a translated progress caption beside connection status distinguish
provider idle from active evaluation/preparation. Menu rows retain their buttons
and focus when status changes; rearming uses the current running turn rather than
an earlier terminal task's source. **Stop supervision** stops automation and its
owned evaluator, independently of the ordinary provider-turn Stop action.

`supervision_state.rs` retains bounded UI ownership and queue gates; GTK only
renders its state and supplies explicit actions. A unique request identity on
Start, state and preparation updates prevents stale callbacks, rejected starts or
previous connection generations from releasing a newer owner. Source and Router
controller queues are held immediately, including while initial options load,
and at queue preparation and final dispatch. Stop keeps the gate until issued
work/receipts settle and pauses both queues; completion permits ordinary queued
follow-ups. Cancellation before Start retires its settings callback without
starting a worker. Reconnect/process loss retires UI ownership and never resumes
automation implicitly.

Source options use the ordered shared-settings worker and their actual session
key. Offscreen routed targets retain their own model/effort/Fast and permissions;
the selected controller's options do not replace them. Model/Fast/access changes
invalidate pending preparation before an asynchronous preference save. Human
session actions stop supervision before submission work; Codex image preparation
now runs after its socket's human-action guard, so Steer invalidates an evaluator
before attachment work. The socket retains whether preparation identified a
routed job and treats a missing subsequent Router binding as a failed handoff.
An accepted source identity remains real even when this fails; no retry occurs.

The native suite passes **389 tests** (four opt-in ignored), including five
UI-ownership tests and the extended actual-socket missing-route failure scenario.
`scripts/smoke-gnome-supervision.py NATIVE GATEWAY` drives actual GTK menu, Send,
Fast and Queue controls under private Mutter/Wayland, through the real gateway,
authoritative source reader, evaluator transport and Router persistence. Eight
source turns include two automatic continuations and two ordinary queued tasks;
five ephemeral evaluators are cleaned up, including one cancelled in progress.
Two Router decisions are the ordinary initial/queued requests. The fixture checks
exact continuation text, held/released source/controller queues, distinct
controller/source model options, source Fast, preserved drafts, Stop without
resume and the queued task retained after evaluator cancellation. It deliberately
omits utility event output so the authoritative completed-thread read is required.
The actual mapped popup and visible progress were captured and inspected.

All providers/profiles are private deterministic fixtures; no real inference or
user conversation is used. An initial fake-provider routing-function name
collision and a test gesture that changed Fast's appearance without clicking it
were corrected; their failed runs are retained. Rust 1.92 all-release-target
checking passes. The separate 496-file QA debug bundle passes this workflow,
the broad native control fixture, ordinary seven-turn Map synchronization and
routed Map/recovery acceptance. Its existing optional diagram/PDF runtimes are
retained; native/gateway checksums are regenerated. The private
`native-supervision-controls-20261104/CONTROLS-EVALUATION.json` record retains
source/binary/evidence hashes, screenshots and the earlier failed fixtures.
Release/installed-control acceptance remains; the previous composer/Router
release candidate does not expose this working-tree feature. A subsequent audit
identified two remaining edges before release: target-side opt-in needs a reliable
Router/controller association, including after restart when background history
intentionally has no full job projections; and a newer running turn must not
display its previous terminal supervisor's status. The qualified routed fixture
starts from the selected Router. Complete these paths before treating supervision
as fully qualified. The full migration remains active.

## Turn supervision: backend integrated, controls pending

This earlier backend checkpoint preceded the native controls described above.

The existing web feature (`ui/turn-supervision.mjs`, `ui/app.js`) still requires
native controls and queue coordination. The Rust core in `turn_supervision.rs`
models Waiting, Evaluating, Preparing, Sending, Complete and Stopped without I/O,
GTK, persistence or retries. A private monotonic owner, revision and source-text
fingerprint qualify each evaluation and send. Human intervention invalidates late
work; a pending evaluation keeps its work lease until it settles. A source turn
announced before its send acknowledgement does not prematurely cancel ownership.
An accepted late acknowledgement after Stop still returns its Router handoff
receipt without restarting supervision.

The core retains the logical root task, steering additions and previous prompts/
replies, limits compact evaluator context to 100,000 UTF-16 units without silently
truncating evidence, rejects malformed decisions and bounds continuation prompts
at 4000 UTF-16 units. Five unique continuations are the hard send limit; completion
can still be evaluated at that limit. Before dispatch, the caller must supply a
fresh authoritative completed-turn snapshot with matching connection, source and
text fingerprint. An uncertain, duplicate, empty or invalid acknowledgement never
causes a retry. Callers receive read-only state and guarded action tickets rather
than writable job fields. Evaluator instructions/schema preserve the existing
read-only/never-approval utility workflow and no-tools instruction.

The 16 focused state-machine tests cover stale owners/backends, manual mutations, steering, approvals,
connection changes, changed source text, early notifications, late receipts,
ambiguous acknowledgements, exact context history, Unicode budgets and terminal/
repeat/limit decisions.

`turn_supervision_io.rs` now provides authoritative source reads, independent of
GTK display excerpts, and an isolated evaluator adapter. Source reads validate the
Codex session/project and visibility, fetch latest-turn states and up to 32 item
pages of 120 entries, and revalidate states after paging. Older providers fall
back to full turn pages or identity-checked legacy reads only for unsupported
methods. The adapter keeps raw user text and the latest nonempty assistant message,
preserving whitespace, Unicode and content beyond the 128 KiB display excerpt.
It never fabricates image/skill text or silently truncates evidence. Repeated or
overlapping pages, changed source identities/states, malformed data and exhausted
text/page/time budgets fail without yielding partial evidence. Each source text
has a 400,000-byte allocation bound; the complete evaluator input retains the
core's 100,000 UTF-16-unit bound. Reads have a 45-second deadline and cancellation
checks. Backend activity/attention flags guard dispatch; the owning runtime must
also qualify connection epochs, human actions, approvals and queue state.

The existing structured utility runner now has an explicit completed-thread read
mode, enabled for supervision while translation and Session Map keep their existing
event-output mode. It reads only its owned ephemeral thread and validates that the
latest turn is the acknowledged completed turn, with no error, before returning
the full last assistant message. Completion before acknowledgement records its
identity, and explicit other-turn events cannot contaminate an owned result.
Decision parsing runs on a Rust worker. Cancellation and failed/ambiguous reads
still perform single-attempt owned cleanup; cleanup uncertainty is retained as a
warning. A returned source identity is rejected before any turn or cleanup mutation.
Read-only sandbox, never-approval policy and no-tools instructions match the web
workflow; the instruction is not a claim of hard tool denial.

Thirteen additional tests exercise source pagination/fallbacks, text and Unicode
limits, attention/lifecycle guards, stale identities, cancellation and actual
isolated WebSocket evaluation. Eight fake-provider scenarios cover missing event
text, completion before acknowledgement, changed utility turn, read failure,
cancellation, malformed/oversized decisions, a returned source identity and
unconfirmed deletion. The full native suite passes **359 tests** (four opt-in
ignored), with Rust 1.92 all-release-target and format checks passing. The initial
WebSocket fixture omitted the required authentication subprotocol; the fixture
now acknowledges it and the passing runs use the normal authenticated client.
An initial reader future-lifetime compilation error was corrected before testing.

`turn_supervision_runtime.rs` now coordinates these adapters as bounded
connection-owned work. It admits at most 16 sessions and two workers, rotates
source polling, and retains stopped owners while their evaluator, final read,
issued source request or handoff remains unsettled. It exposes ordinary
environment/Session Map preparation, then performs a final authoritative source
read and releases send parameters once through `dispatch`. The host must not
bypass that single-use admission. Preparation and acknowledgement have finite
deadlines; uncertainty never retries a source request. Exact source model/effort,
Fast and access parameters are retained, with invalidation hooks for source
options/permissions changed during preparation. They must be connected to the
actual host settings actions.

Accepted source receipts carry the exact submitted continuation text for Router
question/source provenance, including when Stop precedes the acknowledgement.
Another evaluation or owner waits until the host settles that handoff. A failed
handoff stops automation while preserving the accepted current-turn identity.
Steer invalidates the evaluator but retains its cleanup lease; late results
cannot authorize the new revision. Shutdown cancels workers and drains their
owned cleanup paths; an early coordinator drop allows cancellation to finish
without persisting or replaying jobs. The actual host must also process accepted
handoffs during its shutdown sequence. Cleanup uncertainty remains visible when
the subsequent source read fails.

Fifteen asynchronous coordinator tests verify these preparation, source-change,
identity, single-use dispatch, running/completed observation, terminal decision,
Steer, option/permission, lost/late acknowledgement, admission, handoff and shutdown
contracts with a controlled service. Together with the adapter/state-machine
tests, the full native suite passes **374 tests** (four opt-in ignored). These
coordinator tests do not execute an actual application socket, queue or GTK host.
The earlier eight WebSocket scenarios separately exercise the real utility adapter.
Rust 1.92 `cargo check --release --all-targets --locked`, format and diff checks
also pass for this coordinator checkpoint.

The socket host in `turn_supervision_host.rs` now connects the coordinator to
`codex_socket`. Typed requests retain their backend/connection generation; a Start
also requires the canonical backend, including its evaluator socket path. Foreign
or stale requests are rejected before reading source evidence or starting a utility.
OpenCode rejects supervision without starting provider work. Host preparation shares
the ordinary environment cache and mechanism, then uses the existing Map context
loader/resume path. Guarded Map preparation has no turn tracker or send until its
single-use release after the final authoritative read. Cancellation discards it;
late Map replies cannot become ordinary sends. A possibly applied Map configuration
is remembered even when its resume reply is cancelled, so a later no-Map turn clears
it. At socket admission, human sends, Stop, review, shell and session mutations
stop automation; Steer invalidates its pending decision and records additional
instructions while retaining cleanup ownership. Approval/input requests and
backend lifecycle changes guard it too. During shutdown, supervised wire admission
also checks a separate closing flag while bounded utility cleanup drains.
Accepted routed receipts during shutdown are handled by the subsequent host
handoff checkpoint below.

Assistant evidence now uses the same private-Map suffix filter as the displayed
transcript. It skips private-only assistant items and retains the latest nonempty
visible reply, without shortening ordinary text or rewriting user instructions
that contain a literal marker. This is semantic private-metadata filtering, not
context truncation.

Three guarded-Map tests and three actual socket-loop tests cover mapped/unmapped
dispatch through real private HTTP/WebSocket connections, a source notification
before its acknowledgement, human interruption and approval while a Map reply is
held, and rejected backend/generation/evaluator socket identities. The evidence
and evaluator services are controlled in these socket tests; the earlier utility
WebSocket tests exercise real utility transport separately. The initial fixtures
incorrectly used HTTP 200/null for an absent Map and blocked notifications while
holding a reply. They now use the gateway's HTTP 404 contract and concurrent
notification delivery with an ordered barrier; preserved failure logs distinguish
those fixture corrections from product changes.

All **381 native tests** pass (four opt-in ignored), with Rust 1.92 all-release-target,
format and diff checks passing. A rebuilt debug executable also passes
`scripts/smoke-gnome-map-sync.py` under private Mutter/Wayland, using private
profiles/providers and the unchanged packaged gateway. `map-sync-ihnx3jsv` verifies
seven source turns, revisions `[1,2,3,4,5,6,6]`, same-turn tools, unsupported-tool
fallback, private-marker Copy/stream filtering, offscreen updates, queued latest
revision, stale human edits and empty/malformed no-ops; extra inference is zero
and owned providers exit. This is existing-Map GUI regression coverage, not GTK
supervision acceptance.

The socket host now performs accepted-receipt Router handoff itself. Before ordinary
environment/Map preparation it calls the authenticated, bounded
`POST /studio/native/router/prepare-supervised` endpoint. A matching routed job's
preceding Map scope must settle before the native source client configures the
next one; identity, ownership and retained Stop intent also guard preparation.
This uses an indexed SQLite projection on a Rust storage worker, not a full reply
reload on every poll. Preparation has a finite deadline and never sends a turn.

After a real source acknowledgement, the host calls
`POST /studio/native/router/follow` with the exact accepted continuation and
previous/next turn identities. The gateway verifies the actual provider's adjacent
completed predecessor and accepted user input, plus session/project visibility and
the existing Router target. No inference, routing decision or source send is
performed by this endpoint. A transaction adopts monitoring of the acknowledged
turn, clears old reply/items/context/Map metadata before changing source anchors,
preserves unrelated metadata and keeps Stop sticky. Duplicate receipts do not
rewrite ownership or restart monitoring. A foreign claim or stale worker write is
rejected. Terminal-save/monitor removal, recovery admission and handoff share a
short transition gate; provider RPC is outside that gate. Recovery is published
only after its monitors have been registered. Existing monitors adopt a changed
accepted turn without rerouting. Shared delivery history and native source
provenance use the actual supervisor prompt while retaining the original Router
question as the display group.

Window closure stops future automation, drains an already issued source
acknowledgement within its deadline and settles its handoff without a GTK event
recipient. Preparation/handoff failures retain the accepted identity, stop
automation and never retry a source mutation. Another evaluation waits for that
handoff to settle. These guarantees cover a known acknowledgement; an uncertain
network outcome is still not evidence of an accepted turn.

The native suite now passes **384 tests** (four opt-in ignored). Two more actual
socket tests cover mapped/unmapped late acknowledgements after closing the window
and readiness/handoff failure without a source retry; a projection test checks new
reply anchors and unchanged old snapshots. The headless gateway suite passes
**255 tests** (one child-process helper ignored). Storage tests exercise stale
writes, claim conflicts/rollback, unknown metadata, idempotence, sticky Stop and
indexed readiness. `scripts/smoke-gnome-supervision-handoff.py GATEWAY` uses private
providers to verify completed/active adoption, accepted receipts after Stop,
concurrent recovery/handoff after restart, exact shared source provenance and
waiting for a real prior Map update. Existing Router and native Wayland Map
workflows are rechecked separately; those GUI tests are regression coverage,
not supervision control acceptance. Rust 1.92 all-release-target checking and the
existing Linux Tauri desktop build check pass. The private
`native-supervision-handoff-20261104/HANDOFF-EVALUATION.json` record retains source,
binary and evidence hashes. This checkpoint uses debug executables and does not
promote a release package or claim macOS/Windows runtime validation.

At this earlier checkpoint queue coordination, current-option/mutation UI hooks
and native controls were not yet integrated; the socket owned receipt handoff
independently. The subsequent native-control work is described above. No real
provider inference was started. No new release candidate was promoted; the
separate qualified composer/Router GUI candidate below predates this work and does
not expose supervision.

## Native composer and Router language

The composer now explicitly binds its tooltip and accessible instruction to the
interface language. This owns the accessible name, not the TextBuffer's text.
Router reference-file and target controls use the same mechanism. The target
picker binds its dialog title, automatic choice, search hint/name, result count,
paging/refresh tooltips and fixed notices. Provider errors/warnings remain literal;
only their interface wrapper translates. The selected target's marker has a named
native accessible image.

Automatic routing is interface-owned text. Choosing an explicit target replaces
that binding with a literal session/backend label and a translated tooltip whose
title/key/path variables stay literal. Clearing the target restores the automatic
binding. Locale changes never change the chosen target, query, catalog, revision,
callback, model/effort options or draft. The catalog reuses the existing shared
Automatic routing and No matching sessions translations; no duplicate keys or
JavaScript runtime are introduced.

`scripts/smoke-gnome-router-ui.py NATIVE GATEWAY --headless` now verifies live
English/Chinese changes on the final 10 rows of a 60-result search, then on the
explicit OpenCode and restored automatic target controls. It checks actual dialog,
button, hint/count and tooltip text; the same search text and row objects remain,
as do target identity, source scope, options and draft. The inspected Chinese
screenshot shows translated controls and unchanged session/path data. Packaged
`router-ui-r2umt2ul` passes all three launches, including real routed Send/Stop,
both-provider source navigation, shared favorites, restart without replay and
history/search. It uses private fake providers only.

Installed `release-ui-gxvr3dbu` switches languages through the actual Settings UI
and reads the composer's accessible name through AT-SPI. It verifies the Chinese
instruction, restores its English name and checks the retained draft contents.
Application hooks and reader overrides are absent. All existing installed
language/backend/Git/viewer/HTTPS-image and process/event-gap recovery groups pass,
including math, Mermaid, PlantUML and D2. History `history-pages-ui-tv80y6fd`, broad
controls `run-24l2katt`, all 330 native tests (four opt-in ignored), 749 web tests,
format/catalog checks and Rust 1.92 all-release-target checking pass.

The matched candidate is `target/gnome-native-candidate-20261104-router-language`.
Its `BUILD.json` and private
`native-router-language-20261104/ROUTER-LANGUAGE-EVALUATION.json` retain source,
package and evidence hashes. The 496-file release changes only its native
executable and checksums; gateway/runtimes and existing-client code/manifests stay
byte-identical to the preceding history-language checkpoint. Previous gateway,
original Linux Tauri and large-output/write-failure evidence remains applicable;
no new macOS/Windows or remote-CI run is claimed. Default installations and earlier
qualified candidates remain unchanged.

At this composer-language checkpoint, the renewed feature audit found no native
runtime or controls for running-turn supervision (`ui/turn-supervision.mjs`,
`ui/app.js`, release `e114d43`). Backend integration has since advanced as recorded
above; controls and queue coordination remain required native work. An unused
next-message translation key is not evidence of
an existing next-message UI. This feature gap, remaining owned-control/HTML-menu
accessibility, compatibility and physical/provider/fresh-machine acceptance keep
the full migration active.

## Native history and transcript language

The history pager's Older/Newer/Live controls, tooltips, page/item counts,
loading/unavailable text and legacy/partial/excerpt details now use the shared
explicit language bindings. Error summaries translate their interface wrapper
while retaining the provider diagnostic as a bounded literal value. Visible labels
and their full tooltips update together; there is no re-fetch or translated model.

Shared transcript headers now bind Copy, excerpt/full-output and favorite actions,
routed-session/response tooltips and the Favorite role. Session/backend names remain
literal template variables. The output viewer's Favorite caption and chat-table
wrap tooltip follow the locale too. The Sessions heading, pin/image/queue/Fast
service tooltips and shell permission hints use the same interface-only mechanism.
Model IDs, effort values, source messages and action/selection identities do not
change. This is an increment in the owned-control audit, not a claim that every
remaining diagnostic or accessibility label has been translated.

`scripts/smoke-gnome-conversation-pages.py NATIVE GATEWAY --headless [--items]`
exercises real gateway reads and private fake providers. Packaged preferred-item
`history-pages-ui-3rm0b8bu` and compatibility-turn `history-pages-ui-whp57j1i` pass
both Codex and OpenCode paging, legacy fallback, large-turn/part windows, cursor
seams, loaded-page search, cancellation, backend switches, one explicitly fake
manual Send and owned process cleanup. The added checks change English/Chinese
while reading normal, legacy and OpenCode pages. They verify actual button names,
summary/detail text, selected GTK-label text, identical history/live item handles,
unchanged source text/drafts and retained reading anchors. Loading and failed-read
wrappers update without changing the literal diagnostic. Screenshots were inspected.

Fixture failures are retained. The first selection probe incorrectly expected the
short-message renderer to be a TextView; these messages use selectable GTK labels.
The subsequent raw-scroll comparison started before the legacy page finished
settling. Its detailed evidence showed identical data, selection and draft, with
only scroll geometry changing. The fixture now waits for four stable normal GTK
frames before taking its baseline, records geometry, and checks the bottom anchor
when reading at the end or the top content offset otherwise. It retains the
original two-pixel tolerance; it does not force widget allocation or scroll values.
The settled record shows the unchanged 34-pixel toolbar and a one-pixel viewport
change when localized status text is laid out.

Installed `release-ui-l1zu35n_` uses Ctrl+Shift+H and the normal Settings language
controls through AT-SPI. It checks Chinese and English history counts, shared Copy
and favorite actions, preserved draft/document contents and returning to Live.
Expected counts come from the private fixture ledger, including the independent
event-gap recovery item. An earlier attempt `release-ui-_0n0uj5m` incorrectly
expected only the two original items despite that third recovered item; its
accessible tree already showed the correct translated count. The final run retains
that count assertion and all existing diagram/language/backend/Git/viewer/image and
process/event-gap recovery groups. Application hooks and reader overrides are absent.

Both activity inspectors and the nine-case 6.1 MB real-report output regression
pass on the matched package, as do broad controls `run-r5q3iv2h`, 330 native tests
(four opt-in ignored), 749 web tests, formatting/catalog checks and Rust 1.92
all-release-target checking. The package candidate is
`target/gnome-native-candidate-20261104-history-language`; `BUILD.json` and private
`native-history-language-20261104/HISTORY-LANGUAGE-EVALUATION.json` retain source,
package and evidence hashes. Only the native executable and checksum manifest
change in the 496-file release package. Gateway/runtimes and existing-client
sources/manifests remain byte-identical to the preceding activity checkpoint, so
its gateway/original-Linux-Tauri and failed-write evidence remains applicable.
No new macOS/Windows or remote-CI validation is claimed. Previous candidates and
the default installation remain unchanged. Remaining owned-control/accessibility,
HTML-menu, compatibility and physical/provider/fresh-machine acceptance stay in
the full active migration.

## Native activity inspection

Use **Inspect activity snapshot** in an activity item's header. The shared GTK
output viewer provides bounded pages, source line numbers, literal case-sensitive
search, exact displayed-page Copy, wrapping and complete snapshot export. It is
read-only. Reopening the header after a complete provider update inspects the new
revision; an already open snapshot does not change under selection or Copy.

Rust projection now retains command arrays, exit codes, file-change kinds, tool
arguments/errors/status and unknown future metadata. Visible reasoning uses its
content when no summary is supplied; opaque encrypted reasoning envelopes are
omitted. Commentary retains the same private Session Map suffix filtering as the
transcript. OpenCode tool snapshots include the original part, input, error,
metadata, timing and server-owned IDs alongside its normalized command view.
Codex plan notifications replace one stable row per turn. Reported status badges
are display-only; they never infer that a session became idle or interrupt a turn.

JSON serialization, cache lookup, paging, search and export run on Rust workers.
Pretty JSON preserves field values rather than original provider wire formatting.
Serialization rejects snapshots over 16 MiB before retaining partial JSON. The
shared cache has a 64 MiB accounting budget and at most 256 entries, plus its
existing active-reader pin and in-flight worker leases; this is not a total-process
memory limit. Only 64 KiB/256-line pages and references cross into GTK. Partial,
oversized and expired snapshots report their limits. Streaming text deltas do not
rewrite the last captured activity object; complete item updates supply a newer
snapshot. User/final assistant messages keep their existing complete-output action.

The viewer's fixed labels, page/line/byte summaries, notices, search and export
controls follow live language changes. Provider JSON and unknown status values
remain literal. This increment does not finish the broader interface-language audit.

`scripts/smoke-gnome-activity.py NATIVE GATEWAY --backend codex|opencode` uses private
profiles, denied-tool fake providers and an owned Wayland compositor. Final packaged
runs `activity-mmquojza` and `activity-hxm2x976` verify actual header controls,
metadata-only rebinding, immutable open revisions, Unicode/NUL JSON, reasoning,
command arrays, file kinds, unknown activity fields, large paging/search/export,
exact clipboard content and preserved drafts/selection/idle state. OpenCode uses
HTTP history and SDK events through the real adapter. Codex additionally verifies
plan-row replacement and live Chinese controls. Screenshots were inspected.
Neither focused run starts a WebKit content process or sends a provider mutation.

The shared output regression `outputs-ui-jhqq1q_6` reads a 6,065,514-byte real report
and verifies all nine live/history/provider output cases, exact complete exports,
source line offsets, failed literal search/page retention, expired identities,
stale-dialog isolation and priority of user navigation over background updates.
Its GTK heartbeat's maximum observed gap, including screenshots, was 312 ms; this
is fixture evidence rather than a physical-desktop latency guarantee. The failed
export run `write-failures-ucznyigt` exercises five actual filesystem failures,
closed/replaced viewers, queued contextual errors, pending-write close guards,
unchanged destination sentinels and temporary-file cleanup. Complete native
controls `run-ludyucbr` also pass.

Installed `release-ui-ndnqt5wt` opens the inspector through AT-SPI and checks
read-only original metadata, exact independent Wayland clipboard content and
returning to the retained reader/draft. Application hooks and reader overrides
are absent. Every existing installed language/backend/Git/viewer/HTTPS-image and
process/event-gap recovery group passes, including math, Mermaid, PlantUML and D2.
All 330 native tests pass (four opt-in ignored), as do 749 web tests, formatting,
catalog checks and Rust 1.92 all-release-target checking.

Earlier fixture failures are retained: the first OpenCode probe assumed a GTK row
existed in the same tick as history completion, and its follow-up assumed an unused
Codex request log existed. The first shared-output run used a synchronous capture
before the dialog painted and quit without a result. The fixture now waits for
normal coalesced rows and the shared GTK frame-aware capture; screenshot and export
assertions remain in place. Failed fixtures' owned providers have exited.

The matching candidate is `target/gnome-native-candidate-20261104-activity`.
Its `BUILD.json` and private `native-activity-final-20261104/ACTIVITY-EVALUATION.json`
retain source/package/evidence hashes and the failed attempts. Only the native
executable and checksum manifest change in the 496-file release package; gateway,
runtimes and notices remain byte-identical. All 853 existing-client source files
outside native/QA/documentation match the preceding connection checkpoint. Its
250 gateway tests and original Linux Tauri validation are retained from unchanged
sources; this is not a new macOS/Windows or remote-CI run. The default installation
and previous qualified candidates remain untouched. Control/translation and
compatibility audits, physical GNOME/input/assistive-technology checks, real
providers and fresh-machine acceptance remain part of the active goal.

## Native connection diagnostics

Open **Connections & accounts** from the application menu, or **Settings → Backends
→ Connection diagnostics**. The GTK/libadwaita view lists backend event-stream
status first. Expand a backend to inspect its executable, protocol, transport,
execution environment, host platform and reported version/health/error. Native
and gateway versions and the loaded configuration path/errors remain available
below the backend list. Names, commands, paths and diagnostics are literal data;
markup-like arguments do not become Pango markup. Fixed labels follow live language
changes. Editing backend configuration still uses the existing settings editor.

The Rust diagnostics job reads the existing registry/application/backend information
endpoints, with at most four concurrent metadata requests and stable display order.
Only registry-approved IDs are used to construct endpoint paths; upstream path
fields are not followed. Responses are normalized on bounded Rust workers and
backend-information strings are capped at 8 KiB plus a truncation marker. A failed
provider health check retains successful metadata from the other backends. Closed
dialogs cancel their request identity; gateway stops invalidate pending reads.

Codex information endpoints describe configuration without proving process health
or starting an unused Codex process. OpenCode's existing information endpoint may
start its managed server to perform a health check, on this explicit user action.
Session-event status is presented separately and comes from actual native transport
state. Status changes do not trigger metadata polling, session selection, model
inference or persisted preference changes. Refresh is explicit; Reconnect retains
its existing application action.

`scripts/smoke-gnome-connections.py NATIVE GATEWAY` exercises private profiles,
fake Codex/OpenCode providers, an executable canary and an owned Wayland compositor.
Packaged run `connections-t9ibj44b` verifies menu and settings entry points, metadata,
literal markup-like names/arguments, one outstanding Refresh per dialog, partial
health failure/recovery, live Chinese labels, independent foreground/background
connections, closed-dialog stale reads and actual gateway stop/reconnect. The
fixture wrapper closes only its own gateway's parent pipe and records all owned
gateway/provider PIDs for cleanup checks. No provider inference/mutation request
is sent. Original project/shared files and the unsent chat draft remain unchanged;
no WebKit content process starts. English, Chinese and unavailable-provider
screenshots were inspected. A first run completed its assertions but caught GTK
markup warnings during subtitle construction; literals are now assigned only after
markup is disabled, and the clean rerun passes the log guard.

The native unit suite passes 323 tests, with four opt-in tests ignored. A real
private HTTP regression checks authentication, approved GET paths, four-request
admission, stable ordering despite out-of-order replies and retained failed-health
metadata; additional tests cover cancellation and Unicode/NUL display limits.
All 749 existing web tests and Rust 1.92 all-release-target checking pass.

Matching installed `release-ui-4soueiio` also opens diagnostics through the actual
Backends settings row and verifies the connected event stream, populated metadata,
Refresh admission and returning to Settings. It passes every existing diagram,
language, backend editing, Git, viewer, HTTPS original-Save and recovery group.
Application hooks and continuous-reader overrides are absent. Complete debug
control run `run-g2rxfdae` and both-provider session-information regressions pass.

The first broad run `run-zaat7s5c` exposed a fixture race: its background OpenCode
turn completed after a fixed 0.8-second delay, before the native workflow had
reliably switched backends. The fake task now waits for the actual offscreen
transition, while the separate idle-before-acknowledgment case remains unchanged.
The follow-up `run-ipth2aaj` passed OpenCode but used the bare debug executable
without bundled diagram runtimes and consequently failed its document probe.
Both failures are retained. The final rerun uses the complete matched package;
no assertions or per-operation deadlines were relaxed.

`native-connections-final-20261104/CONNECTIONS-EVALUATION.json` in the private QA
directory and the candidate's `BUILD.json` record source/package/evidence hashes.
Only the frontend executable and checksum manifest differ from the preceding
496-file session-information package. The gateway, diagram/PDF runtimes and
notices are byte-identical; existing-client sources/manifests/lockfiles are
unchanged. The 250 gateway tests and original Linux Tauri check are retained from
that unchanged source checkpoint; web tests were rerun. The local alias
`target/gnome-native-candidate-20261104-connections` points to the optimized package.
Default installations and previous qualified candidates remain untouched.
Physical desktop and real-provider acceptance remain separately documented;
activity details and the broader audits still belong to the full migration.

## Native session information

Open **Session information** from the native application menu. The GTK dialog
shows backend/session identity, available provider metadata, and an editable
opening question. Identification pages through provider history on bounded Rust
workers instead of treating the first message in the current chat tail as the
original question. It never labels an incomplete opening-history scan as complete.
Automatic identification does not replace an existing opening question. Router
controllers skip automatic capture of their internal coordination prompts.

Ordinary sessions also expose the shared Router responsibility editor. **Generate
with AI** explicitly adds a normal session turn, retains model/effort/Fast/access
settings and leaves the unsent composer draft and attachments alone. The returned
summary fills the field for review; **Save** writes it. Closing the dialog does
not implicitly save or interrupt the ordinary conversation turn. **Stop generation**
uses the acknowledged turn. OpenCode's activity marker is session-scoped, so its
adapter verifies that the latest server-owned user message still belongs to this
generation before calling abort. A later message rejects the abort; the upstream
API does not provide an atomic conditional abort.

`GET/PATCH /studio/native/session-info` use the existing authenticated gateway and
shared opening-message SQLite row. Field-specific patches retain unknown capture
metadata and compare revisions before saving. Router's existing responsibility
endpoint uses the same update implementation and legacy-preference synchronization.
Opening text retains the existing 16 KiB UTF-8 limit; responsibilities retain the
4,096-character limit. A conflicting HTTP 409 or uncertain write outcome requires
Reload while keeping the user's fields. Session/backend/connection changes block
stale editor actions. No schema migration or new model credential is introduced.

`scripts/smoke-gnome-session-info.py NATIVE GATEWAY --backend codex|opencode`
uses private profiles, document copies, fake providers and an owned headless
Wayland compositor. Packaged runs `session-info-hjunnzcs` and `session-info-t08z2_ci` verify:

- Original questions outside the loaded history tail, literal provider metadata,
  actual edited-question clipboard Copy, Save and reopening.
- Normal session generation, unchanged unsent draft, an independent editor proving
  generation did not save, and explicit generated-result persistence.
- Two-editor save conflicts, retained text, disabled Save, Reload Cancel/Discard,
  stale-session blocking, actual Stop/abort and unchanged saved responsibility.
- Live Chinese labels with literal model text, no WebKit content process,
  unchanged project/shared files and cleanup of owned providers.

Rendered English/Chinese dialogs were inspected. The native suite passes 320
tests (four opt-in tests remain ignored); 250 gateway tests and 749 existing web
tests pass. Rust 1.92 all-release-target checking and the original Linux Tauri
check pass. The OpenCode adapter's
HTTP regression rejects a newer user message without sending abort and then
accepts the owned identity with project-scoped requests. This is fake-provider
acceptance, not a claim of physical input, real inference or whole-migration
completion.

The matching optimized package passes installed `release-ui-ynznn4wn`: default
continuous native Preview and cross-block Copy, all four diagram engines,
language/backend settings, Git, viewer returns, source editing/Reload, file
portals, HTTPS original Save, process replacement and event-gap recovery.
Application hooks and continuous-reader overrides are absent in this installed
run. Complete debug control run `run-c4limv0f` passes the existing conversation,
workspace, settings, command, agent, queue, Router and OpenCode suites, including
responsibility metadata/conflicts and restart retention.

The first broad run `run-78779bre` reached the final OpenCode workflow but hit the
old 60-second total integration guard. Its completed Router results and failure
are retained. That guard now allows 180 seconds for the accumulated suites; each
operation retains its shorter deadline and all assertions. The rerun finished
its primary GTK workflow in 55.3 seconds. This corrects a test budget, not a
claim of faster application performance.

The private `native-session-info-final-20261103/SESSION-INFO-EVALUATION.json` record
and sibling candidate `BUILD.json` retain source, evidence and package hashes.
Both debug/release packages include the matching new gateway. Only the frontend,
gateway and checksum manifest differ from the preceding 496-file Resources
package; diagram/PDF runtimes and notices remain unchanged. The repository alias
`target/gnome-native-candidate-20261103-session-info-final` points to the optimized
package. The default installed application and earlier qualified candidate remain
untouched. Connection/activity audits, final compatibility and physical/provider
acceptance remain part of the full migration.

## Architecture

`native/gnome` is an independent Cargo workspace. GTK4 and the existing GTK3-based
Tauri runtime must never be linked into the same process. The native application
starts an owned, gateway-only build of Studio over a private authenticated
loopback connection. The gateway is built with `--no-default-features` and has no
Tauri, GTK3, or WebKit dependency. It reuses the existing backend registry, Codex
handshake, process management, SQLite stores, environment settings, API security,
and profile locking. No additional model credential is stored by the frontend.

The native executable finds `codex-thread-studio-gateway` beside itself. Developers
can override that location with `CODEX_THREAD_STUDIO_GATEWAY_BIN`. The gateway
chooses an ephemeral loopback port; its token stays in memory and is never logged
by the native client. Closing the UI closes its parent pipe so the gateway exits
and cleans up its managed backends. Remote `--serve` behavior is unchanged unless
the explicit `--parent-pipe` option is supplied.

The two applications share the normal Studio profile. Close the existing Studio
or server before launching the GNOME application. To test separately, set
`XDG_CONFIG_HOME`, `XDG_DATA_HOME`, and `XDG_CACHE_HOME` to private directories;
native development does not silently bypass the profile lock.

## Build and run

Ubuntu 26.04 needs GTK4, libadwaita 1.7 or newer, and GtkSourceView 5. The complete
build needs Rust 1.95 or newer for the independent Mermaid worker; the native
frontend and gateway retain their Rust 1.92 requirement:

```bash
bash scripts/install-gnome-dev-deps.sh
bash scripts/build-gnome.sh
./target/gnome-native/package/codex-thread-studio-gnome
```

Markdown Preview uses the continuous native text view by default, including
both read-only comparison panes. Source/Edit and the document toolbar remain
native and paged. For renderer comparisons, `STUDIO_GNOME_CONTINUOUS_PREVIEW=0`
selects the retained block adapter; normal launches need no renderer variable.

The executable dynamically links GTK4, libadwaita, GtkSourceView, VTE and WebKitGTK
6. Those system libraries are required even before a content viewer is opened;
WebKit's content processes start only for actual HTML, EPUB or browser content.
Node.js, Python and an external browser are not Studio runtime requirements.
Configured AI backends retain their own installation requirements. The development
installer adds headers and build tools to the same runtime package list.
The Rust gateway still embeds the existing browser assets because it shares the normal
HTTP routes, but the native frontend never loads or executes them.

The build script also packages checksummed PlantUML MIT 1.2026.9 and D2 0.9.0
runtimes. PlantUML needs Java/Graphviz; both external engines use Bubblewrap user
namespaces, with no home/project mounts or network. The development dependency
installer includes these system packages. Mermaid uses bundled Merman 0.9.3 in
`codex-thread-studio-mermaid`, with Rust font measurement, RaTeX and path-only SVG
rasterization. Its isolated process receives only explicitly selected font files,
has no home/project/network access, and starts only for a diagram request.
Style, layout, text size, spacing and curves use the shared appearance settings.
For offline development builds, `STUDIO_GNOME_SKIP_DIAGRAM_DOWNLOAD=1` skips the
runtime download; `bash scripts/install-gnome-diagrams.sh` packages them later.

The bundle also includes checksummed PDFium 158.0.8111.0 without V8/XFA, its
third-party notices, and a Rust worker interface. PDFium runs in a separate
Bubblewrap process, never in GTK or the gateway process. It uses the same
installed Bubblewrap dependency as diagrams. `STUDIO_GNOME_SKIP_PDF_DOWNLOAD=1`
skips this build-time download; `bash scripts/install-gnome-pdf.sh` packages it
later. Developers using separate executables can set
`STUDIO_GNOME_PDFIUM_LIBRARY` to the absolute `libpdfium.so` path. There are no
runtime downloads or browser requirements for PDF reading.

To keep build output and Cargo downloads on another disk:

```bash
export CARGO_TARGET_DIR=/media/rui/studio-gnome-deps/build
export CARGO_HOME=/media/rui/studio-gnome-deps/cargo-home
bash scripts/build-gnome.sh
"$CARGO_TARGET_DIR/package/codex-thread-studio-gnome"
```

Build systems that already produced both executables can assemble the same
bundle without separate Cargo target directories. Build the Mermaid component
once with Rust 1.95+, or reuse a component verified against this checkout:

```bash
bash scripts/build-gnome-mermaid.sh /path/to/new-mermaid-component
bash scripts/package-gnome.sh /path/to/codex-thread-studio-gnome \
  /path/to/gateway-build/codex-thread-studio /path/to/package /path/to/new-mermaid-component
```

The ordinary build script delegates to this packager. It includes the icon,
launcher, activation service and embedded-component notices even when optional
external-engine downloads are skipped.
`STUDIO_GNOME_MERMAID_COMPONENT` can supply that verified component to either
script, including when compiling the parent with Rust 1.92. Component protocol,
source/patch hashes and every declared binary/notice file are checked before
packaging or installation. No runtime source checkout or downloads are needed.

Install the frontend, gateway, Mermaid worker, application icon, GNOME launcher and D-Bus
activation service locally:

```bash
bash scripts/install-gnome.sh
```

For an extracted bundle, a source checkout and build toolchain are unnecessary:

```bash
bash ./install-runtime-deps.sh       # Ubuntu 26.04; --print / --simulate also work
bash ./install.sh
~/.local/bin/codex-thread-studio-gnome
```

The installer uses `~/.local`; optional arguments select a different bundle
directory and installation prefix. `SHA256SUMS` is checked before installing and
again after staging. It detects incomplete/modified files, not an untrusted
release publisher. Versioned files, gateway and engines live under
`PREFIX/libexec/codex-thread-studio-gnome/`; only Studio's executable links enter
`PREFIX/bin`. Existing `d2`, Java, PDF libraries and the Tauri application/launcher
are left intact. Reinstalling the same checked bundle reuses its version; updates
retain old versions so an existing process still has matching sibling engines.
The matching desktop entry enables D-Bus activation and declares notification
support. The activation service launches the installed native executable with
`--gapplication-service`; keep it with the desktop entry when packaging. The
installer escapes both the string-value and Exec argument layers. The desktop
fallback uses `/usr/bin/env` to avoid GLib's executable lookup preceding percent
expansion in paths. Private-bus tests cover spaces, percent, dollar, backtick,
quote and backslash characters, plus a file argument.

## Native release validation

A current optimized frontend and gateway have now been built and packaged on
Ubuntu 26.04, including the pinned PlantUML/D2/PDFium runtimes and notices. The
packager is shared with the ordinary build script; a staged installation uses
the same installer and sibling-gateway discovery as a normal install.

### Earlier flow-prefix candidate

The matched optimized bundle with projected continuous-reader translation is
`/media/rui/studio-gnome-deps/candidates/native-flow-prefix-20261103/package`.
It includes the earlier Merman/appearance/HTTP-image work and adds the portable
Rust prefix cursor, native table-prefix API and asynchronous Translate capture.
The normal reader keeps its current renderer; the continuous adapter remains
opt-in for its remaining input/accessibility/promotion checks.

```bash
GDK_BACKEND=wayland ./target/gnome-native-candidate-20261103-flow-prefix/codex-thread-studio-gnome
# To exercise continuous Preview and its projected selection capture:
STUDIO_GNOME_CONTINUOUS_PREVIEW=1 GDK_BACKEND=wayland ./target/gnome-native-candidate-20261103-flow-prefix/codex-thread-studio-gnome
```

Frontend SHA-256: `7a0d0ff6da6452987bde9cd28ad8d893cebc7782be1e98397b627824cf906b2a`.
Gateway SHA-256: `48660f72837cef7447dedebe50f42354d274beb18c96cc0c990895b7ec77f8af`.
The bundled Mermaid worker remains
`39e3c2ece5a8b80e79bf88c0d47650185e3b37bf3b40888c30094e0bc9cbe4fa`.
`BUILD.json` fingerprints the source and all packaged files. The preceding
488-file HTTP-image bundle is preserved; no test touched the default install.

Installed `release-ui-_gs4m4bw` passes all existing groups, HTTP-image original
Save, process replacement and event-gap recovery without application hooks.
`translation-1mbvy8c6` adds actual Wayland Ctrl+Shift+T and native context capture
on a 33.5 MB table, exact provider text/cache and held-read retirement. The real
current 4,193,003-byte report passes `flow-pipeline-aagvky45`, including mixed
math/table selection, code controls and source/dirty/paging guards. Its observed
open is 936 ms after the first shell frame, with a 25 ms maximum opening loop gap;
these host samples do not include the initial graphics-cache startup cost.

See [continuous-selection translation](#native-continuous-selection-translation)
for API/work bounds and tests. The private acceptance record is
`native-flow-prefix-20261103/FLOW-PREFIX-EVALUATION.json`. Core/gateway/native
suites pass 104/247/299 tests, Rust 1.92 checks pass, and the existing GTK3/Tauri
Linux build still checks with its separate SDK. The wider migration, renderer
promotion and remaining platform/desktop acceptance gates remain active.

### Previous HTTP-image candidate

The preceding optimized bundle with public HTTP(S) image support is
`/media/rui/studio-gnome-deps/candidates/native-remote-images-20261103/package`.
It includes the preceding Merman/appearance integration and adds Rust HTTP(S)
image fetches, native chat/attachment/reader previews, original Save, cancellation
on replacement/unmap, and packaged Linux TLS notices. The shared gateway and
Mermaid worker are unchanged. Launch the exact candidate with:

```bash
GDK_BACKEND=wayland ./target/gnome-native-candidate-20261103-remote-images/codex-thread-studio-gnome
```

Frontend SHA-256: `3b805a666935dc080ccd5183fb23834d1cc3a0319cb3ee02badb3c83514f743b`.
Gateway SHA-256: `203dc9e0b0eccd50ac783f5ef6d8e483c1beffdd1c0cc50b26a3cbe6bd004250`.
Mermaid worker SHA-256: `39e3c2ece5a8b80e79bf88c0d47650185e3b37bf3b40888c30094e0bc9cbe4fa`.
`BUILD.json` records exact source and package fingerprints. All 486 files in the
preceding Mermaid candidate remain unchanged, and tests use private installation
prefixes. The default installation was not touched by this work.

Installed run `release-ui-0444cnie` passes all existing optional workflow groups,
process/event-gap recovery, and the new HTTPS image/viewer/exact-original Save
case through ordinary installed controls, with no application hooks. Public
artwork `remote-images-5lat77jj` also checks chat attachments, SVG, a blocked local
canary, retained source/draft, and zero web engines/model turns. Existing
`embedded-images-dada0hbw` passes the local/data-image and source/Save regression.
All 298 native unit tests and Rust 1.92 release-target checks pass; four opt-in
fixtures remain normally ignored, with the public HTTPS fixture separately run.
TLS notice reproduction, formatting, shared translation snapshots and library
resolution pass. CI is wired but has not run remotely for this worktree.

See [remote image bounds and reproducible checks](#native-remote-https-images).
The acceptance record is
`native-remote-images-20261103/REMOTE-IMAGES-EVALUATION.json` under the private QA
root. This checkpoint leaves the full native migration and its remaining
rendering, input/accessibility, provider and platform acceptance work active.

### Previous Mermaid adoption candidate

The preceding optimized native bundle is
`/media/rui/studio-gnome-deps/candidates/native-mermaid-gtk-math-20261103/package`.
It adds the qualified Merman worker, prepared font/image cache, actual math-label
metadata, mixed supported/unsupported formula handling, shared style/layout
controls and verified worker build/install integration to the earlier candidate.
The local launch alias is `target/gnome-native-candidate-20261103-mermaid`:

```bash
GDK_BACKEND=wayland ./target/gnome-native-candidate-20261103-mermaid/codex-thread-studio-gnome
```

Frontend SHA-256: `b90312e35b6b4fc86ef8f93a9e34e792bc2d51e7d46636b6fcbbc7f172964bc3`.
Gateway SHA-256: `203dc9e0b0eccd50ac783f5ef6d8e483c1beffdd1c0cc50b26a3cbe6bd004250`.
Mermaid worker SHA-256: `39e3c2ece5a8b80e79bf88c0d47650185e3b37bf3b40888c30094e0bc9cbe4fa`.
`BUILD.json` records exact source and packaged-file fingerprints. The pinned
D2/PlantUML/PDFium runtimes and notices are reused from the earlier package.
All 64 files of that earlier integration package remain unchanged; the default
installation is unchanged too.

Current installed run `release-ui-qx568plm` passes every existing optional
workflow group, real process replacement and event-gap recovery, without
application test hooks. Full native `run-tj7qaak0` also verifies actual Mermaid
math counts 3/7 and the 4,119,318-byte report, while two-launch appearance
`appearance-ui-_se9ptct` verifies saved style/layout, fonts and retained edits.
All 20 worker tests, 1,029 selected-feature renderer tests, 59 valid corpus cases,
13 font profiles, five pixel-identical font controls, nine package checks and
real resource/cache invalidation pass. Dependency test fixture/lock provenance
and the original mixed-formula failure are preserved. The parent/gateway suites
pass 296/247 tests; their 3/1 normally ignored tests are separate opt-in fixtures.
Rust 1.92 and current-compiler native release checks pass. GNOME CI wiring has
been updated; remote CI has not run for this worktree.

See the [worker adoption and limits](../native/mermaid-worker/README.md#gtk-adoption-2026-11-03)
for reproducible checks and source/evidence identity. This checkpoint does not
promise all Mermaid HTML/class-member/dynamic-font behavior or universal speedups:
the measured 80-node case still regresses against Pele. Continuous Preview
remains an opt-in experiment. The full migration, physical desktop/input,
real-provider and fresh-machine/platform acceptance gates remain open.

### Previous integrated candidate

The optimized pair in
`/media/rui/studio-gnome-deps/candidates/native-integration-20261103/package`
includes the large-context fixes, live PDF English/Chinese controls and Router
result/clarification notifications. The local alias is
`target/gnome-native-candidate-20261103-integration`; the earlier candidates and
default installation remain unchanged.

```bash
GDK_BACKEND=wayland ./target/gnome-native-candidate-20261103-integration/codex-thread-studio-gnome
```

Frontend SHA-256: `9c9fced483578ae23bde50f0179ced9b73e2a3b03e4feb7d6e6a92b884c9ba01`.
Gateway SHA-256: `26d71431b1d048de5d217a5afb27e391dcd98982c04d129e51a10fdc09a3c791`.
The gateway and shared sources/binary match the previous large-context candidate.
`BUILD.json` records 856 source inputs, all 64 packaged files, compiler/branch
identity, and preserved earlier-package hashes. Existing checked D2/PlantUML/
PDFium runtimes and notices are reused without downloads or version changes.
GTK4/WebKit6 remain in the native frontend; `ldd` confirms the gateway links no
GTK/WebKit/JavaScriptCore, and neither binary has unresolved libraries.

Installed run `release-ui-icze2wav` passes every existing diagram/language/backend/
Git/viewer group, process replacement and event-gap recovery through the ordinary
installer, desktop entry and sibling-gateway discovery. It uses owned GNOME
accessibility/keyboard input, private files/providers, and no application hooks.
The additional PDF case selects page 2 and Text mode, runs a query, and changes
English → Chinese → English through Settings. It verifies retained page/query,
raw PDF text, literal filename, composer draft and exact clipboard bytes, plus
translated result labels/status/accessibility names. Copy waits for asynchronous
GTK/Wayland ownership, like the existing independent-clipboard workflow; the
initial immediate-read assertion is retained as a fixture diagnostic.

Installed `release-large-onel0se0` passes the actual 8,340,599-byte report and
32 MiB LF/CRLF tables: exact outline targets, bounded Source/Edit, explicit saves,
EOF context/Undo, stable table paging/column filtering and retained draft. Opening
observations were 2,132 / 587 / 601 ms, and Source windows were at most 130,775
bytes. These include accessibility/startup observation costs; the generated files
reuse the same process and warm graphics/fonts. Native/gateway RSS peaks were
373,548/35,428 KiB, with no application web-content processes or model turns.

`desktop-portal-t1y6nian` also passes the seven real GNOME/Nautilus chooser calls
without a portal stub or app hooks. The existing Linux Tauri desktop target still
passes `cargo check` against the isolated genuine GTK3/WebKit SDK. Native/shared/
gateway production sources remain at their previously validated unit/MSRV
checkpoints; the new package changes no persisted-data format or runtime pin.
The `native-integration-validation/VALIDATED.json` QA record contains completed
run artifacts, source/test/binary hashes and the first clipboard timing failure.
`READY.json` identifies this candidate for testing. This does not mark full native
feature parity, physical/provider/platform or fresh-machine acceptance complete.

### Current large-context candidate

This earlier candidate remains frozen as a comparison baseline; the integrated
candidate above also includes the later PDF and Router changes.

The matched optimized pair in
`/media/rui/studio-gnome-deps/candidates/native-large-context-20261103/package`
includes the EOF context and wide-table position fixes. It is available as
`target/gnome-native-candidate-20261103-context`; earlier packages/links and the
installed default remain unchanged.

```bash
./target/gnome-native-candidate-20261103-context/codex-thread-studio-gnome
python3 scripts/smoke-gnome-release-large.py /path/to/package --reference /path/to/actual-large.md
```

Switching from Edit to Source now retains a containing raw source window or asks
the Rust worker for saved context using the editor's original bounds. It never
substitutes the editor's CRLF-normalized text for raw Source. The shared
`SourceWindow::contains_caret_line` policy includes a trailing empty caret line
only when the window reaches document EOF. A different page's first line is not
owned by the previous window. Cached raw context must also overlap the editor's
byte range, since one very long physical line can span multiple windows.
Post-save index refresh retains a matching editor context instead of reopening
an empty EOF page and clearing GTK undo history. Dirty text stays in memory
across view switches; explicit/page-change saves keep their existing behavior.

The native table API now treats `row` as the exact source-row cursor. Reducing
12 columns to 11 raises the cell-bounded page size from 42 to 46; it keeps the
same top row instead of rounding row 42 back to zero. The existing 50-row,
512-cell, source-byte and fragment limits still apply. The compositor-free
document API regression checks 12/11/4-column transitions and is included in CI.

Installed run `release-large-e8sg78_2` uses the ordinary installer/desktop entry,
private providers, native GNOME accessibility/keyboard controls and an independent
Wayland clipboard reader, with no application hooks or gateway override. It
checks the actual report, two generated 32 MiB tables, distant outline/source
lines, raw CRLF, EOF mode switches, undo/redo after explicit save, unchanged bytes
outside the edited page, table paging, keyboard column selection, retained top
row/draft, no model turns and cleanup. The original user report remains unchanged.
The same final fixture rejects the frozen previous candidate at the dirty EOF
Source switch (`release-large-myskyhev`), capturing an empty Source buffer. An
independent old-API probe also reproduces row 42 becoming row zero after column
selection. These are expected negative controls, not failures of the new pair.

| Document | Bytes after workflow | Observed open | Initial Source bytes |
|---|---:|---:|---:|
| Actual report | 8,340,599 | 2,174 ms | 46,220 |
| Generated LF table | 33,555,074 | 593 ms | 130,690 |
| Generated CRLF table | 33,574,894 | 642 ms | 130,775 |

The observations include startup/control-query overhead; the generated documents
open later in the same process with warm graphics/fonts. They are not an isolated
parser benchmark or a general speed guarantee. Distant outline observations
are 1,074/1,278/1,127 ms and include opening/querying the native contents controls.
Sampled native/gateway peaks are 373,984/35,724 KiB across 612 samples, excluding
other processes; no web-content process starts for these Markdown files.

The matched optimized pair also passes all optional installed core groups and
process/event-gap recovery (`release-ui-yo4oo56y`), and the exact document API
command used by CI (`documents-amzbwdwc`). The four-launch real-report regression
(`run-mqayvi_f`), 288 native/246 gateway/97 shared parser tests, Rust 1.92 and
release-profile checks, the Linux Tauri compile check, all 749 web/shared tests,
TypeScript and Rust formatting pass. Three entries are marked ignored in the
ordinary suites, including a subprocess test helper and an explicit D2 benchmark.
The optional packaged-engine rendering/file-import confinement test was run
explicitly on this host and passes.
Source/binary/package hashes, successful and failed development runs, API negative
control, logs and preserved package hashes are recorded in
`/media/rui/studio-gnome-deps/qa-artifacts/native-smoke-jwfgtzto/large-context-validation/VALIDATED.json`.
Physical Wayland/IME/accessibility, live-provider, fresh-machine and other-platform
acceptance remain open; this checkpoint does not complete the full migration.

- Native SHA-256: `19ec29e8eeed999c157003d30694877b17987f736fbf93930afeee613d75f4dd`.
- Gateway SHA-256: `26d71431b1d048de5d217a5afb27e391dcd98982c04d129e51a10fdc09a3c791`.

### Earlier refreshed candidate

The earlier refreshed staged candidate is
`/media/rui/studio-gnome-deps/candidates/native-full-refresh-20261103/package`,
also available from the checkout as `target/gnome-native-candidate-20261103`.
Both frontend and gateway were rebuilt in release mode from the current branch
on Ubuntu 26.04.1. The ordinary packager assembles the pair and its notices;
the pinned optional D2/PlantUML/PDFium files are byte-identical to the preceding
package. Runtime linking verifies GTK4/WebKitGTK6 for native content and no
GTK/WebKit/JavaScriptCore in the gateway. Build warnings remain.

Installed workflow `release-ui-h0vzv48i` passes all five optional groups below,
owned Codex process-exit/event-gap recovery, and real GIF/APNG/WebP
Play/Pause/Replay controls. It installs into a private prefix through the normal
installer and launches via the desktop entry with no application hooks or gateway
override. Source/Edit/find, dirty view switches, explicit saves, diagram inspection,
bilingual controls/questions, backend JSON save, Git, image/table/PDF/EPUB handoff
and owned-process cleanup pass. The application log has no GTK warning, critical
or panic; the deliberately limited recording portal reports two GDK interface
warnings. Sampled native/gateway peaks are 628,680/98,644 KiB; EPUB web content
peaks at 241,504 KiB and starts only after an intentional EPUB open. These are
debugging observations of an optimized workflow, not process-RAM limits or a
controlled performance comparison.

The same optimized package also passes real native Map controls/Undo
(`release-map-zcps1_ja`), bilingual Favorites (`release-favorites-i2dcyp6r`),
bilingual Comments (`release-comments-uzcjy6ve`) and the compositor-free image API
(`images-ui-yw4w73rz`). Those extra UI fixtures launch the packaged pair directly
with private profiles, sibling-gateway discovery and no application hooks. The
image API checks GIF/APNG/WebP file/inline/snapshot transport, still fallback,
original identities/bytes and confinement. Sources, binary/runtime/package hashes,
logs and the five owned runs are recorded in
`/media/rui/studio-gnome-deps/qa-artifacts/native-smoke-jwfgtzto/native-release-refresh-validation/VALIDATED.json`.
This candidate includes the current recovery, rendering and animation checkpoints;
the continuous text-flow experiment retains its existing opt-in status. It does
not complete the full migration or physical/provider/fresh-machine/platform acceptance.

```bash
./target/gnome-native-candidate-20261103/codex-thread-studio-gnome
```

Candidate SHA-256:

- Native: `ae32507596a8d0e7b1ee2a8b052158937a381770a744a5529bc32a0849683d0c`.
- Gateway: `3757786c4b51dd60cbd735a518c650c6d636b59eb98d712bf991a6c404f5b45b`.

The previous `target/gnome-native-candidate-20261102` launch link and both of its
binaries remain unchanged. The existing installed package is unchanged too;
this refresh creates an additional candidate rather than replacing that install.

The existing web/shared suite passes all 749 tests, generated-UI verification and
TypeScript checking. Its language audit now excludes JavaScript test fixtures and
recognizes an explicit Rust file-level `#![cfg(test)]` boundary; Unicode fixtures
are retained. All seven Router tests still run and pass. The ordinary default
Tauri desktop `cargo check -p codex-thread-studio --locked --offline` also passes
on this host after supplying real GTK3/WebKit4.1/accessibility development metadata
in `/media/rui/studio-gnome-deps/legacy-desktop-sdk/root`. Ten packages were checked
against APT's SHA-256 metadata and extracted into a separate SDK; no system
packages were installed or upgraded. This supersedes the earlier missing-GTK3
compile-check limitation for this host. It is a Linux compile check, not a new
legacy-desktop runtime test or macOS/Windows acceptance.

The existing installed checkpoint is `release-ui-0y2jeua7`, passing all five optional
workflow groups shown below without application test hooks. Its matched package
is `/media/rui/studio-gnome-deps/current-release/package`; large embedded-image
transport, file-wide reference resolution and exact source-line coverage are
recorded under [file-wide Markdown references](#file-wide-markdown-references).
The latest package also covers native table images and modal cell-to-viewer
handoff, live document/comparison language changes, and embedded SVG bitmaps.
It also includes routed Session Map synchronization/recovery, named native
shell menu actions and Favorites/Comments language coverage. The native
implementation now shares its widget library with document probes and fixes
character hit-testing for read-only TextView links. Current hashes:

- Native: `0dcce66370f4de0f2e91bc47ae44700851a90bf031e7fbde717ab400ec932a47`.
- Gateway: `04653357c732003ca21556f6580f3023641e2856fc760299cfd7e2581ee6f3d5`.

Earlier run names
and measurements in this document describe implementation checkpoints.

```bash
python3 scripts/smoke-gnome-release.py /path/to/package
# With packaged D2/PlantUML, Java and working user namespaces:
python3 scripts/smoke-gnome-release.py /path/to/package --all-diagrams
# Include real GTK language selection in both directions:
python3 scripts/smoke-gnome-release.py /path/to/package --all-diagrams --language
# Also validate and explicitly save backend JSON through installed controls:
python3 scripts/smoke-gnome-release.py /path/to/package --all-diagrams --language --backends
# Include native Git filtering, file handoff, keyboard selection and staging:
python3 scripts/smoke-gnome-release.py /path/to/package --all-diagrams --language --backends --git
# Also open/close image, source, table, PDF and EPUB through the installed launcher:
python3 scripts/smoke-gnome-release.py /path/to/package --all-diagrams --language --backends --git --viewers
# Also verify real owned Codex process recovery through installed controls:
python3 scripts/smoke-gnome-release.py /path/to/package --all-diagrams --language --backends --git --viewers --recovery
# Include an actual gateway event overrun, without application hooks:
python3 scripts/smoke-gnome-release.py /path/to/package --all-diagrams --language --backends --git --viewers --recovery --event-gap
# Also exercise installed Session Map controls and persisted Undo:
python3 scripts/smoke-gnome-release-map.py /path/to/package
# Include bilingual Favorites editing, Copy and native export portals:
python3 scripts/smoke-gnome-release-favorites.py /path/to/package
# Include bilingual Comments editing, insertion and native source opening:
python3 scripts/smoke-gnome-release-comments.py /path/to/package
```

This fixture requires test-only `python3-gi`, `gir1.2-atspi-2.0` and Mutter. It
installs into a private prefix containing spaces, Unicode and a percent sign,
then starts both actual optimized executables without application test hooks or
a gateway-location override. It uses the private GNOME accessibility bus and
fixture-owned Wayland input, checks the application PID before accessible
actions, and uses only fake providers and private files. GTK 4.22.4's
[Component.GrabFocus handler](https://github.com/GNOME/gtk/blob/4.22.4/gtk/a11y/gtkatspicomponent.c#L174)
returns unsupported, so the fixture uses supported text/widget actions and
ordinary Tab/arrow/Space navigation for the language picker and radio groups
rather than depending on that method. Polling tolerates accessibility nodes disposed during dialog dismissal,
while checking that the owned application remains alive.

The installed pair passes conversation loading, exact selected-TeX Copy through
an independent compositor reader, native document-diagram inspection with
zoom/actual size/Fit and normal Escape dismissal, Preview/Source/Edit, Undo/Redo, retaining
unsaved edits across view switches, explicit file save, composer retention and
file handoff through the installed desktop entry. It also opens a 3,000 × 1,000
PNG through that entry and saves its exact original bytes through the optimized
viewer's actual GTK button and a private SaveFile portal. This verifies the
matched gateway source-identity headers as well as destination writing without
application test hooks. Closing the real window reaps the owned providers;
unused browser/EPUB processes remain absent in workflows that do not open EPUB.
The `--viewers` case checks that absence before its intentional EPUB open and
verifies content-process cleanup after normal window closure. The all-format variant also opens
four independent Markdown files through that entry, checks each format-specific
accessible inspection action and verifies 400% raster dimensions, actual size,
Fit and Escape dismissal. With `--language`, it opens Settings through `Ctrl+,`,
selects Chinese and then English through the real GTK picker, explicitly applies
each and verifies live labels and unrelated preference/document/draft preservation.
In both locales a private provider sends a real question request. The fixture
types into its native entry and uses native radio-group keyboard navigation:
Chinese selects a preset after typing a custom draft, while English returns to
Other and submits the retained exact text. It checks both replies on the provider
wire, then closes Settings through its own native button before the other checks.
With `--backends`, the fixture reopens Settings, activates Backends and its native
editor, replaces its GtkSourceView text, validates without creating a file, then
explicitly saves and verifies exact Unicode/CRLF bytes. It closes both native
dialogs and checks that the document and composer draft were retained.
With `--git`, it also selects a file through actual keyboard/accessibility input,
filters its diff, opens the working file, returns to Review and explicitly stages
it. The index is checked independently. The combined language/Git case checks the
search field's accessible name in Chinese and English. The earlier expanded-workflow
checkpoint (`release-ui-sl_dsd6r`, with all four options) sampled native/gateway peak RSS
of 563,408/76,108 KiB, excluding worker peaks. This is not
a full memory bound, a startup benchmark, a fresh machine installation, or
physical assistive-technology/portal acceptance.

This external check found a reader navigation bug hidden by earlier internal
fixtures: replacing a Source/Edit buffer leaves its insertion mark at EOF.
Switching Source to Edit could consequently open an empty last page. Newly
loaded pages now place and scroll their cursor at the page start; reused dirty
Edit pages retain their buffer and selection. The installed check verifies the
actual accessible text/caret and subsequent save behavior.

At this optimized-package checkpoint, the native suite had 230 passing tests and two normally ignored
runtime-dependent checks (engine confinement and an explicit D2 stage benchmark);
the gateway suite passes 237 tests with one normally ignored check. The shared
Markdown parser and embedded MarkNoria suites pass 65 and 36 tests respectively.
The reusable SVG bitmap resource suite passes five tests.
Rust 1.92 checking passes. The actual
optimized math worker passes its resource limits and invalid-input checks.
Dynamic-library inspection confirms GTK4/WebKitGTK 6 in the frontend, no GTK3,
and no GTK/WebKit/JavaScriptCore in the gateway. The four-launch debug application
regression still passes after native Git Review integration: the real 4,097,226-byte report
opened in 939 ms with 57 GTK loading ticks, preserving source lines, paged
editing, Unicode/CRLF saves, conflict detection and backend cleanup. Its broader
workload sampled native/gateway peak RSS of 585,720/189,772 KiB; these observations
exclude worker peaks and are not comparable to the small release fixture.

The native migration remains in progress. Expanded installed workflow coverage,
physical Wayland/IME/accessibility/portal acceptance, real-provider checks and
remaining feature parity still apply. Earlier sections below record the narrower
checks available at each implementation checkpoint; they do not replace this
installed-pair acceptance or claim a remote CI run.

## Current feature coverage

Implemented native workflows:

- Responsive conversation/composer, conversation-search and Markdown toolbars
  use the native [libadwaita WrapBox](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/main/class.WrapBox.html).
  Controls wrap before selected labels become unreadably narrow; short action
  lines keep their normal button widths. Long model names stay intact in the
  model and accessibility text, with compact selected labels and wrapped popup
  choices/selection markers. Linked Preview/Source/Edit labels can shorten with
  tooltips at large fonts. This fixes GTK allocating a 708-pixel conversation
  partly offscreen when its visible split pane was smaller. Native Markdown
  headings retain their size/weight hierarchy with custom reading typography.

- Configured Codex-compatible backend selection, explicit reconnection and
  bounded recovery of established transports. See
  [backend transport recovery](#native-backend-transport-recovery).
- Separate bounded connections keep previously opened backends subscribed when
  switching views. Approval replies carry their originating backend/generation;
  an offscreen backend cannot receive another backend's approval. Background
  completion updates Active/Attention without treating initial idle loading as
  unread work. Duplicate completions and late starts do not revive finished turns.
- OpenCode through the shared authenticated HTTP/SSE gateway: session catalog,
  history, provider/model selection, text/image/file/skill sends, Stop, and native permission/
  question dialogs. OpenCode owns message identity; the client does not submit
  generated message IDs. Activity follows server status, including when idle
  arrives before the HTTP prompt acknowledgement. Assistant completion never
  fabricates idle. The known upstream stuck-busy behavior is left to OpenCode.
- Paginated session catalog, local title/path filtering, project-folder chooser,
  new sessions, and history/resume.
- Native conversation find (`Ctrl+Shift+F`) uses Codex occurrence search or
  cursor-paged OpenCode reads on bounded Rust workers. Literal whitespace, Unicode-safe source ranges,
  case sensitivity, user/assistant/activity filters and previous/next navigation
  are supported. Matches navigate to the corresponding block and table cell.
  Query, backend and session generations reject stale results; closing search
  cancels pending retrieval. Older Codex targets open in a separate read-only
  context window, retrieved through turn pagination with legacy read support,
  without resuming a thread or changing the live composer. Surrounding context
  is bounded; oversized messages show an excerpt around the match. Codex servers
  without occurrence search explicitly search only loaded messages. OpenCode
  follows the shared proxy's `x-next-cursor`/`before` protocol in 80-message pages,
  searches full source before presentation truncation, and releases raw pages as
  it proceeds. Search returns at most 100 matching messages; cursor and source
  identity accounting each have an 8 MiB scan budget. Missing pagination on a
  full first page or exhausting a scan budget is labeled as retrieved-history
  coverage. Cursor cycles and malformed pages fail explicitly. These reads do
  not change the selected backend, session, turn state or composer.
- Conversation presentations retain backend turn/message anchors, including
  OpenCode assistant `parentID` mappings and aggregated user-message IDs, for
  navigation and shared saved data. User text-part updates retain one item with
  the server's message ID; their projection cache is capped at 8 MiB/64 messages.
- Native Favorites uses Studio's existing SQLite/API: save a displayed response,
  or select response text/code/table-cell text and use its context menu. Edit
  titles, notes and tags, search the shared library, copy, switch Preview/Source,
  confirm removal and export Markdown through the native file chooser. Unsaved
  metadata is guarded and mutations wait for their result. Truncated responses
  are saved as labeled excerpts. Codex and OpenCode source navigation open
  read-only paged context, including sources from another configured backend or
  session, without changing the live draft. OpenCode retains up to 40 surrounding
  items on each side, including context crossing a page boundary. Exports retain
  the native transport's 32 MiB response bound.
- Pin/unpin controls and stable pinned ordering reuse Studio's existing SQLite
  pins, including its ten-session limit. Pins survive restarting the native app
  and are visible to the existing UI; no second pin database is introduced.
- Native session rename, fork, Codex archive/restore and confirmed deletion.
  All/Pinned/Archived views use native controls; archived chat is read-only and
  history reads include turns without resuming the thread. Catalog/history
  generations discard obsolete responses. Backend changes and window close wait
  for session mutations. OpenCode exposes rename/fork/delete and disables archive.
- Worker-parsed conversation Markdown flattened into virtualized block rows,
  with selectable text, headings/lists/quotes, escaped inline markup and Unicode.
  Native ColumnView tables support stable paging, column selection, resizing
  and wrapping; table pages contain at most 50 rows and 512 selected cells.
  Reusable GtkSourceView code blocks provide highlighting, theme changes and
  Copy in both conversations and the document reader.
- Conversation Mermaid/PlantUML/D2 and standalone display math use the shared bounded diagram lane and
  raster cache. Jobs start when rows map; unmapping cancels work and releases
  textures. Source views are created on demand. Session changes invalidate old
  render results, and chat admits at most two jobs into the shared render lane.
  Recreated history/favorites windows receive distinct render generations;
  cancellation targets that generation so late results or old-window cleanup
  cannot affect a replacement window.
- Immutable shared transcript rows avoid copying large text into the GTK list
  model on each render. An LRU memory cache holds at most 16 conversations and
  64 MiB of transcript data; this is a cache budget, not a limit on total app RAM.
  Previously viewed text appears immediately while authoritative history and
  options refresh. Late replies stay scoped to their backend/session, including
  clearing a successfully submitted draft after switching to another view.
- Streaming deltas, Send, Steer, Stop, model/effort selection, and turn status.
  Failed Stop/Steer requests re-read the session before another send; they do not
  automatically resend the user's input.
- Commands opens from the menu or Ctrl+Shift+P without replacing the composer
  draft. Inline `/` completion runs model, Fast, permissions, status, compact,
  review, diff, skills and session commands. A complete typed command executes
  locally; paths and commands with arguments remain ordinary message text.
  `/subagents` (also `/agent`) opens the native agent-family navigator for Codex.
  `/diff` shows the session's accumulated turn diff in a paged GtkSourceView,
  and `/status` includes the backend's reported token usage. Rust workers index
  and format this data before GTK sees it; a 128-entry/32 MiB cache isolates
  sessions/backends. Diff viewing is capped at 8 MiB/500,000 lines, with an
  explicit error referring larger changes to Git Review.
- Leading `!` uses the backend's shell command API and displays Run in the
  composer. Codex's shell API runs with full local access, independently of
  model-message sandbox choices; the native UI describes this before running.
  Its immediate acknowledgement does not release the composer or queued work.
  Started/completed turn events and user-shell history track it across session
  switches. OpenCode requires an explicitly selected provider/model and uses
  its own shell API. Shell commands reject attachments and cannot be queued.
  `/compact` and `/review` use backend commands; review targets uncommitted
  changes inline. These operations pause queued dispatch until Resume.
- `/permissions` selects inherited, read-only, workspace-write or full access for
  future Codex messages in the current Studio run. Full access requires an
  explicit confirmation; choices are scoped to the session's original project
  and a stale dialog cannot change another session. At most 128 overrides are
  retained in memory. New sends and queued dispatch apply the current choice;
  existing turns and shell commands use their backend's own policy. OpenCode
  explains its project/runtime permission model rather than exposing Codex modes.
- `/mcp` opens Codex MCP status in a native virtualized list. Connection/authentication
  state and tool/resource/template counts are projected on a Rust worker; full
  tool schemas and resource payloads are dropped before GTK receives the reply.
  Opaque-cursor pagination is bounded to 100 servers per page, 10 pages/1,000
  servers total. Repeated cursors and limits report partial coverage explicitly.
  Reload is scoped to the original backend/session/connection; old dialog replies
  cannot replace a reopened inventory. This is status viewing, matching the
  existing command; server configuration and OAuth actions are not added here.
- Agent navigation resolves parent summaries with `thread/read` and
  `includeTurns: false`, then scans the paginated agent catalog. Membership,
  ordering and tree data are computed in Rust; unrelated families, orphans and
  cycles cannot enter the displayed tree. The installed CLI 0.163.0 schema has
  source-kind filtering but no `ancestorThreadId` parameter, so discovery does
  not rely on that hint. Summary reads override older catalog metadata.
  A virtualized GTK list shows the main agent, nested agents, names, roles and
  status; the current agent's Open button is disabled. Opening another agent
  uses normal session selection and preserves each in-memory draft. Refresh
  retains the selected agent even when the ordinary catalog omits subagents;
  its backend/session selection and parent metadata also survive app restart.
  Discovery has bounds of 32 ancestry reads, 100 catalog pages/10,000 sessions
  and 32 MiB of projected catalog metadata. A scan/depth limit explicitly marks
  the tree as partial; malformed ancestry and repeated cursors show errors.
  Late replies stay scoped to the dialog, request revision, backend, session and
  connection generation. OpenCode does not expose this Codex-specific command.
- Native Thread Router settings open from the menu or `/router`. They edit the
  controller backend, up to three ordered fallback sessions/conditions and the
  selected session's responsibility. Target discovery reads Codex and OpenCode
  catalogs on workers, excludes child/ephemeral/controller sessions, follows
  opaque cursors and reports partial/error coverage separately from save errors.
  Discovery is bounded to two concurrent backends, ten pages/1,000 sessions per
  backend and 2,048 target choices overall. Closing/reloading cancels old jobs.
  Preferences updates change only Router fields, preserve unknown fields from
  newer clients and retain every existing controller ID. Switching to another
  backend requires an already configured controller. Both Codex-compatible and
  OpenCode backends have explicit Set up/Verify controller actions. Codex creation
  uses the dedicated Router workspace, read-only sandbox and no approval requests.
  OpenCode creation sends the dedicated directory, title and a catch-all tool
  denial through its authenticated session API. Recovery checks the saved ID,
  workspace and root-session identity without loading conversation history.
  Verifying an older OpenCode controller adds a final tool denial if necessary
  and confirms it before registering the controller. OpenCode 1.18.31 appends
  permission updates; validation follows its documented
  [last matching rule semantics](https://docs.opencode.ai/docs/permissions/),
  preserving existing session metadata and avoiding repeated rule accumulation.
  Metadata reads have a 1 MiB limit and run off the GTK thread.
  A shared SQLite setup journal records creation intent before sending the
  request and saves the returned ID before naming or preference registration.
  The gateway completes an accepted setup even if its HTTP client disconnects;
  two setup operations can run at once. Unknown creation results stay explicit
  across restarts and are never replayed. Known IDs can be recovered after a
  failed registration; a conflicting ID is never overwritten. Setup preserves
  the active provider and all fallback conditions, including concurrent edits.
  **Recover existing controller…** handles creation whose acknowledgement was
  lost before its ID reached the journal. Its searchable native selector offers
  up to 100 matching sessions from the loaded, bounded catalog; manual ID entry
  supports other sessions. Inspect displays the exact ID, title, dedicated
  workspace and state. Adopt requires a current successful inspection; changing
  the ID invalidates it. Recovery reads metadata without creating, renaming or
  prompting a session. Adoption re-reads the candidate and checks both the Router
  configuration revision and a backend/workspace/retained-ID journal revision.
  Only idle or not-loaded, unarchived root sessions in the current canonical
  Router workspace qualify. Per-backend ownership rejects recovery while an
  accepted setup is still running, even after its HTTP client disconnects.
  Adoption is also owned work and retains the ID before OpenCode's final-denial
  permission update. A refused/interrupted update can resume from the known
  identity. Ordinary verification also rejects archived saved controllers.
  Recovery never clears an uncertain intent or creates a replacement.
  The authenticated structured POST endpoints are
  `/studio/native/router/controller/recovery` for inspection and
  `/studio/native/router/controller/adopt` for explicit registration. Reads and
  adoption have separate bounded admission; metadata retains its existing
  transport limits. Disk/backend work stays off GTK. Native request/dialog/ID
  guards reject late inspections, including closed/reopened dialogs. Adoption
  locks its ID and closing controls until the result is known. Local fallback
  and responsibility drafts survive; a controller response cannot silently
  rebase them over concurrent active-backend/fallback edits. Reload/review is
  required before saving a conflicted draft. Other preferences retain their values.
  `scripts/smoke-gnome-router-recovery.py "$GATEWAY"` covers genuine lost
  acknowledgements and restart with private Codex/OpenCode servers, read-only
  inspection, invalid/busy/archived/child/workspace guards, stale state,
  permission-failure retention and adoption with exactly one original creation.
  It is included in GNOME CI; remote CI has not run for these local changes.
  `scripts/smoke-gnome-router-recovery-ui.py "$NATIVE" "$GATEWAY" --headless`
  runs two private Wayland launches with actual picker/Inspect/Adopt controls,
  stale ID/dialog checks, closing protection, drafts, concurrent-configuration
  guards, restart and backend cleanup. Its journal is seeded for GUI isolation;
  the API fixture separately covers acknowledgement loss. The inspected dialog
  screenshot was reviewed. Observed debug peak RSS was 335,044/42,672 KiB
  native/gateway initially and 285,056/43,100 KiB after restart; these are local
  workload observations. The wider migration and release gates remain open.
  This checkpoint passes 124 native and 200 gateway Rust tests (one ignored in
  each), both optimized-profile checks, the previous setup lifecycle fixture and
  the four-launch application regression. The real 4,097,226-byte report opened
  in 926 ms with 56 opening heartbeat ticks; paged editing, tables, source
  anchors, comparisons and session recovery remained covered. No fresh release
  executable or installation bundle is produced for this checkpoint.
  Gateway startup reads existing settings without rewriting them; only missing
  settings files receive defaults, preserving extensions written by newer clients.
  Responsibility saves update only that field in the shared SQLite opening
  message, preserving its text, capture metadata and unknown extension fields.
  Revision checks and a conditional SQLite update reject stale/concurrent saves.
  A late acknowledgement cannot replace newer form text. Responsibility scope
  stays with its original backend/session/connection, and a stale editor cannot
  write to a different selection. Save, Reload and adaptive discard confirmations
  keep unsaved changes in memory and guard application close.
- Native Router text conversations use the same composer and virtualized transcript.
  Controller identity is loaded before SDK history, and only a metadata read
  supplies its title/project directory. Send creates a durable gateway job; Stop
  addresses the latest unfinished delivery rather than interrupting an unrelated
  controller turn. Worker polling fetches summary pages and only changed jobs.
  Older/Newer/Latest controls read indexed SQLite cursor pages of 20 requests.
  Polling retains the displayed historical range while new requests arrive.
  Rich reply trees are kept only for that page; other controllers contribute
  activity/unread summaries rather than hidden transcript trees. Retrying an
  uncertain delivery keeps its original UUID even after another controller
  becomes active.
  **Find in conversation** (header or Ctrl+Shift+F) opens persisted Router search
  when viewing a Router; its history toolbar also has a search button. The entry
  searches saved requests, reply excerpts, response-item text and saved response
  tails, errors,
  statuses, target names/paths/session keys, decision text and reference labels.
  Matching is literal, with Unicode case-insensitive matching by default and an
  optional **Aa** Match case control. Punctuation has no regex/SQL wildcard
  meaning, and trailing spaces stay significant. Older/Newer/Latest navigate
  matching jobs, not unfiltered history. Escape/closing search restores the
  ordinary latest page. Changing Router controllers clears the filter while
  preserving each conversation's unsent draft.
  The authenticated jobs API accepts `query` and `caseSensitive`. Rust workers
  use the existing `regex` and SQLite libraries over one read-only WAL snapshot;
  scans do not hold Studio's write mutex or contact a model. At most two scans
  run together, with 512-character queries, a two-second scan deadline and the
  existing two-MiB record budget. Budget failures return an explicit error
  instead of partial/empty success. Each response contains at most 20 thin job
  records; rich content is fetched only for that displayed page. Summary counts,
  active delivery IDs, Stop and queue gates retain their full-controller scope.
  Search does not alter records or read receipts; the existing actual-visible
  reply confirmation policy still applies when viewing results.
  Native serial, current-entry and controller checks reject superseded results,
  including the interval before the entry's delayed callback fires. Live polling
  retains filtered historical ranges. Errors retain the previous view/draft and
  identify an unapplied search; Enter can retry an unapplied filter.
  `scripts/smoke-gnome-router-search.py "$GATEWAY"` verifies 5,001 private stored
  jobs, stored source excerpts and response tails, Unicode/case/space matching,
  scoped cursor paging, invalid input and unchanged payloads/receipts. In the
  measured debug run, common-result pages took about 12 ms and full scans
  83–221 ms; responses were at most 4,645 bytes. Gateway RSS/high-water memory
  grew from 35,924 to 40,896 KiB across the workload. A separately issued WAL
  write committed in 2.47 ms while a search request was in flight. These are
  local debug observations. Searches cover stored text; the existing Source and
  Open session actions provide access to the original target conversation.
  The three-launch `scripts/smoke-gnome-router-ui.py "$NATIVE" "$GATEWAY"
  --headless` fixture exercises actual Find/search/case/paging/close controls,
  stale query callbacks, controller switching and draft retention. The history
  phase applied its first search in 181 ms including debounce, with 226 heartbeat
  ticks and a 69 ms maximum observed main-loop gap across the workflow. Sampled
  peaks were 303,668 KiB native and 47,756 KiB gateway. Search screenshots were
  inspected. The private provider now implements bounded item history rather
  than returning an unknown-method error for the current hydration protocol.
  The API fixture is included in the GNOME CI workflow; that workflow has not
  been run remotely for these local changes.
  This checkpoint passes 124 native and 199 gateway Rust tests (one ignored in
  each), optimized-profile checks and the four-launch application regression.
  The real 4,097,226-byte report opened in 1,076 ms with 65 opening heartbeat
  ticks; document editing, source anchors, table paging, comparisons and normal
  conversation Find remain covered. No fresh release package is produced here;
  the remaining migration/desktop/provider gates still apply.
  Replies retain original backend/session/turn/message anchors, including
  OpenCode text-part and structured-output identities. Source actions open a
  read-only history window; favorites and relative file links use the target's
  source directory and actual question. Excerpts without original message IDs
  cannot invent persistent favorite anchors. Preview text is bounded to 32 KiB
  and 128 message anchors, with shared question storage between reply items.
  HTTP transport failures retain the delivery UUID for explicit retry; definitive
  HTTP rejections retain draft text and permit correction. Newer composer input
  survives acknowledgements. Reopening reads existing jobs without replaying
  prompts. Controller model/effort/Fast and target options remain separate.
  Native read receipts persist in shared SQLite and update the delivery-history
  mirror. Each receipt names an exact terminal revision and controller; stale,
  active and foreign records stay unread. Automatic confirmation requires the
  reply's final rendered block to remain visible for 500 ms in an active window,
  without a dialog covering the conversation. The toolbar/menu can explicitly
  mark visible replies read. Hidden/offscreen replies remain unread, including
  replies outside the latest page. Indexed totals and active IDs cover the full
  controller history. Migration preserves old payloads, row cursors and owners.
  Recent follow-up context is read from SQLite on workers: a bounded lookback of
  32 completed deliveries supplies up to three currently eligible target replies
  for the same controller. Session keys and project directories must still match
  the current catalog. Each entry quotes at most 1,000 Unicode scalars of the
  request and 4,000 of the response; the full response tail is saved before the
  32 KiB display preview is clipped. Legacy records use their saved preview.
  The controller receives quoted conversation data, not new routing rules.
  Escaped JSON counts toward the 512 KiB context budget; optional older context
  is reduced/omitted when necessary. No additional SDK history load is required.
  Replies also have an Open session action alongside read-only source lookup.
  Metadata-only lookup validates backend identity, session, project and archive
  state before selection. The exact target is restored across backend changes,
  including sessions outside the loaded catalog page. Canonical selected sessions
  survive catalog refreshes. OpenCode receives the verified project directory
  independently from its directory cache. Navigation retains per-session drafts
  and model choices; Return to Router restores the original conversation. Older
  replies outside the live tail open in read-only historical context. Late
  lookups/errors cannot change a newer selection; navigation never sends a prompt.
  The composer also has a Router target chooser. Automatic routing remains the
  default; selecting an exact backend/session/project bypasses the controller
  model and sends the original request to that target. The authenticated target
  endpoint reuses planner eligibility (hidden paths, archived/ephemeral/child
  sessions and controller exclusions). It returns only title and identity/path
  metadata, with unavailable-backend warnings. Discovery retains the existing
  bounded catalog limits (100 rows/page, 10 pages/backend, 2,048 projected
  targets); an incomplete backend is reported rather than treated as complete.
  Unicode title/backend/path search runs on Rust workers over the loaded
  inventory. GTK renders at most 50 rows, with previous/next result pages.
  Each controller retains its selected target in memory; Automatic routing
  explicitly clears it. Target choice preserves composer text and model options.
  Unresolved delivery UUIDs lock the chooser to the original request snapshot.
  Session/connection and picker-instance checks reject late discovery/search
  callbacks, including a closed picker reopened in the same conversation.
  Configuration revision checks reject stale choices. Send re-reads the target
  and checks its current directory, eligibility and busy state; an unavailable
  explicit target fails without silently switching to automatic routing.
  Router follow-ups now use the same native queue controls and shared depth
  setting. A whole routed delivery must complete before the next message starts;
  a controller turn completing alone never releases the queue. The exact queue
  head (text and reference list) and durable routing intent commit in one SQLite
  transaction. Rejected requests preserve the saved entry, and repeated accepted
  UUIDs return the original job without sending again. This transaction preserves
  opaque extension metadata on remaining queue entries. Thin, controller-scoped
  job status reads run on Rust workers with at most two concurrent requests,
  independently of the visible history page. Queued sends preserve newer composer
  text and reference chips. Stop, clarification, failure and disconnect pause
  playback; application restart restores the remaining queue
  paused until Resume. Known Router flights survive SDK reconnects for durable
  status reconciliation. Queued messages use the controller's current model/options,
  permission choices and per-controller explicit/automatic target at dispatch;
  those choices are frozen once its durable delivery UUID is submitted. Target
  selection itself is kept in memory, so after restart inspect the target control
  before resuming. The native queue uses UUID message identities. Scoped files,
  Codex skills/plugins and reference-only requests are supported; only metadata is
  persisted. File existence, regular-file status and originating project scope are
  checked on Rust workers before a new queue head is consumed, then checked again
  by routing before target delivery. Missing files leave the saved entry paused.
  The same submitted reference snapshot survives uncertain HTTP acknowledgements.
  Queued delivery requires its POST acknowledgement before the local snapshot
  is consumed; a UUID from status polling alone cannot confirm matching inputs.
  After a transport failure, Retry delivery checks the frozen UUID/references,
  even when a newer composer draft contains a slash or shell command.
  Already accepted UUIDs are resolved before filesystem checks, so retries cannot
  replay a request merely because an attached file has since disappeared.
  Send and Queue wait for file metadata preparation. Images remain unsupported in
  queues. Accepted routing failures/clarifications remain in history and pause
  subsequent playback; accepted entries are not automatically requeued.
  Simultaneous browser/native queue writers still need reconciliation before
  claiming full frontend interchangeability.
  Router image sends now accept inline PNG/JPEG/WebP/GIF, including image-only
  requests, automatic routing and explicit targets across Codex/OpenCode. The same
  image inputs reach the controller and target. Limits match ordinary native image
  input: four files, 10 MiB each and 20 MiB combined, inside a 32 MiB API body.
  Remote URLs and filesystem-image paths are rejected. Two bounded Rust input
  workers validate base64/signatures and hash canonical JSON; hashing streams
  directly into SHA-256 instead of allocating another full encoded request buffer.
  GTK retains shared image references; request assembly runs on native workers.
  After target acknowledgement the gateway releases submitted image buffers
  before monitoring a potentially long-running turn.
  A routing-intent ACK alone does not consume images. Controller-scoped status
  reads confirm the target turn ID before clearing only the submitted image IDs
  and matching text. Newer text/images survive. Rejection, clarification, stop or
  failure before confirmed target delivery preserve the draft. Pending image
  delivery blocks accidental resubmission while status is reconciled outside the
  visible history page. The Studio job record stores an image count, not image
  URLs/base64; provider/SDK conversation history still owns the sent message.
  Pending image drafts remain memory-only, as for ordinary image sends. Queued
  images are still unsupported and stay in the draft.
  OpenCode compares echoed file MIME/URL with server-owned message IDs, ignoring
  added server metadata. History reads have a 32 MiB byte budget, checked against
  Content-Length before buffering and against streamed bytes. An oversized page
  reduces only its read window (100/16/4/2 messages); sends are never retried.
  If even the bounded window is unavailable or no longer contains the owned
  message, routing reports that limitation and requires checking the source.
  Direct Router references now share the same pending-input snapshots and target
  acknowledgement rules as images. The composer has a native reference-file
  chooser, and Codex Router `@` completion retains structured file/project inputs.
  With an explicit target, completion queries that target's backend and project;
  OpenCode file and skill queries carry its directory explicitly. Metadata replies
  from another backend do not switch the displayed conversation. Owner/target
  session and connection identities guard results, and target changes invalidate
  the old popup. Selecting a target that has not been opened establishes its
  backend connection in the background without selecting/resuming a target
  conversation or sending a model turn. Command discovery continues to use the
  controller's backend.
  File selection prepares only
  canonical path metadata on workers, including for very large files. Source file
  roots remain attached to each reference. The gateway checks regular-file status,
  origin containment and canonical paths again before dispatch. Codex receives
  quoted absolute file paths; OpenCode receives scoped local file URIs with
  `text/plain` MIME, matching its ordinary native file-reference semantics.
  The controller receives quoted reference descriptions rather than typed target
  skill/plugin inputs. Target Codex skills and plugins must match an enabled
  inventory entry in that target project, including exact name/path or plugin ID;
  disabled/uninstalled/missing entries fail before a target turn is sent. Inventory
  queries are read-only and bounded to 64 project/marketplace rows and 512 entries.
  An OpenCode target accepts one skill from its own project inventory, with
  optional file and image attachments, through `/session/{id}/command`.
  The gateway checks the exact skill name/path (`path` is OpenCode's command
  name) and `source: skill`; a custom command or MCP prompt with the same name
  cannot silently replace it. Multiple skills, Codex skill paths and Codex plugin
  references fail before a command is sent and retain the submitted draft.
  Command inventory decoding discards templates on a Rust worker, limits the
  response to 16 MiB and rejects more than 512 entries or duplicate names.
  OpenCode expands the command; Studio forwards the exact arguments, target's
  selected provider/model and typed attachments without expanding templates.
  A command has a finite five-minute HTTP deadline and is sent once. Its bounded
  acknowledgement retains only server identity; the normal bounded history/status
  reader obtains the result. Lost, malformed, old or foreign identities require
  inspection rather than retry; restart never replays uncertain delivery.
  The legacy API acknowledges identity after execution. During that wait the job
  shows a notice and Router Stop returns an explicit conflict directing the user
  to the target conversation. It does not guess an ID from concurrent messages.
  Stop and the final pre-send check share one short lock; a Stop accepted first
  prevents sending. After identity is confirmed, existing owned-message Stop and
  authoritative server-idle checks apply. GTK remains responsive during the wait.
  This limitation follows OpenCode's [legacy command implementation](https://github.com/anomalyco/opencode/blob/v1.18.31/packages/opencode/src/session/prompt.ts#L1381)
  and [command input schema](https://github.com/anomalyco/opencode/blob/v1.18.31/packages/opencode/src/session/prompt.ts#L1447).
  `scripts/smoke-gnome-router-command.py "$GATEWAY"` verifies 10 isolated
  deliveries, large inventory projection, file/image/model fidelity, disabled or
  shadowed skills, duplicate requests, foreign/old/missing identity, backend
  failure, lost response, interrupted dispatch without replay and owned-child
  cleanup. GNOME CI includes this fixture; remote CI has not run these local
  changes. The actual `--refs --headless` Wayland fixture also verifies pending
  Stop rejection, visible notice/completion, exact command arguments and newer
  unsent text. Inspected screenshots use the real transcript viewport. Observed
  debug peak RSS was 335,220/46,748 KiB native/gateway during the reference/command
  workflow. Real-provider command acceptance remains a manual gate.
  Limits are 32 references, 4 KiB paths, 32 KiB saved reference labels, and 64 KiB
  final routed text including Codex file paths. Reference-only requests are
  supported. Newer text and reference chips survive target acknowledgement;
  clarification and pre-dispatch failure retain submitted references. File chooser
  and preparation results validate the originating session/connection. The routing
  database retains reference labels, never file contents. Local files are not
  translated into WSL paths. Cross-frontend queue editing still needs separate
  reconciliation work.
- Native queued follow-ups use the existing SQLite queue and shared depth setting
  (one to three messages). Enqueue, edit and remove wait for persistence; failed
  writes retain editor text, and acknowledgements preserve newer composer text.
  Pause/Resume is explicit; Stop, disconnect, errors and application restart pause
  dispatch. Queues stay with their originating backend/session across view switches
  and use ordered per-session model/effort/Fast options at dispatch time. Codex and
  OpenCode wait for authoritative completion, including completion arriving before
  the send acknowledgement. Existing non-text references are preserved; OpenCode
  pauses rather than dropping unsupported references. File/skill inputs survive
  editing and play through their originating backend. Image messages cannot be
  queued; an explicit error preserves their attachments and composer text.
- Native `@` file completion uses backend fuzzy search; `$` completion combines
  enabled skills and installed/enabled Codex plugins. A native popover shows up
  to 30 results, with arrow-key selection, Tab/Enter insertion and Escape to close.
  GTK Unicode scalar offsets preserve Chinese/emoji text, and paths containing
  spaces/quotes are quoted. File results retain the backend's fuzzy ranking;
  merged skill/plugin results preserve the highlighted reference across updates.
  Search generations and worker filter tickets reject late results after typing,
  moving the cursor, changing conversations or reconnecting. Discovery projects
  at most 200 files or 512 skills/plugins per source. Physical IME, focus and
  accessibility coverage remain part of final native acceptance.
- Removable per-session reference chips preserve skill/plugin inputs and OpenCode
  file parts without persistent drafts. At most 32 references per session share
  a 2 MiB/128-session memory budget. Send/Queue capture reference identities;
  failures retain inputs, and late acknowledgements preserve newer references.
  Codex files use text paths, matching the existing UI. OpenCode canonicalizes
  file URLs on a worker and rejects missing files, directories and symlink escapes
  outside their originating project. One OpenCode skill invokes its command API;
  combinations with files/images/plugins or multiple skills fail explicitly.
  Command requests have a separate five-minute deadline; ordinary API deadlines
  remain unchanged. Returned server-owned user IDs, explicit completion/failure
  and bounded worker-projected response rows work even without SSE completion.
- Native image attachments through the file chooser, clipboard image paste and
  file drag-and-drop. The composer displays removable filename/size chips that
  wrap with its available width. Supported inputs match the existing UI: PNG,
  JPEG, WebP and GIF, at most four images, 10 MiB each and 20 MiB per message.
  Rust workers read only regular files, check signatures and encode inline data;
  filesystem paths and base64 payloads never become widget text. No upload occurs
  before Send/Steer. Attachments are scoped to their conversation and remain in
  memory without draft files. Failed sends preserve them, while acknowledgements
  clear only the images included in that send, even after switching sessions.
  Clipboard reads start in the paste signal's dispatch, capturing its current
  offer before a later copy/focus change. Byte reads remain asynchronous, bounded
  and cancellable; this does not depend on keeping an unattended Wayland window
  active.
  Unique pending/in-flight image inputs have a 64 MiB cache budget; temporary
  worker and transport buffers are separate. Only two image preparation requests
  are admitted at once. OpenCode uses inline file parts and
  confirms server-owned identity for image-only messages. History attachments now
  receive lazy native preview rows and can open in the image viewer; worker
  projection keeps encoded URLs out of GTK. Local image files and Markdown image
  references also have native viewing. See [history image bounds](#native-images).
- Per-session model/effort/Fast options share the existing SQLite records.
  Model capabilities determine effort choices and Fast availability. Inherited
  Fast is displayed; turning it off sends explicit `default`, and enabling it
  sends `fast`. OpenCode does not receive Codex-only effort/service-tier fields.
  Settings reads/writes are ordered and scoped to the selected session; forks
  copy the source options, and missing saved model IDs are retained.
- Native per-request approval and structured user-input dialogs, including
  question choices/custom/secret answers, MCP flat primitive forms and explicit
  URL consent. Approval dialogs offer session scope when available.
  Unsupported interactions expose decline rather than silently granting permission.
  See [native agent input](#native-agent-input).
- Adaptive sidebar, system/light/dark appearance, keyboard controls, and GNOME
  application identity. Composer text is retained in memory across selection
  changes; it is not written as a draft.
- Native shell appearance, window geometry, sidebar preference and last selected
  backend/session persist in `gnome.json` beside the normal profile settings.
  The Rust gateway validates and atomically patches this file on a worker;
  serialized writes are flushed before gateway shutdown. This file contains no
  message/editor drafts and does not replace the web application's settings.
- Native Settings dialog with searchable Appearance, Environment and Backends
  pages. Project profiles share Studio's variables, multiline values, cache paths,
  network policy and redacted secret store. Explicit Save, Reload and discard
  confirmation preserve edits; revision checks reject concurrent overwrites.
  Profile reads/writes run on Rust workers and secret files are atomically
  replaced with private permissions. The backend configuration JSON editor uses
  the startup validator, explicit file saves and conflict detection; the separate
  active-registry diagnostics reflect the current process. Restart applies saved
  backend changes; editing or saving does not start unused backends. See
  [native backend configuration](#native-backend-configuration).
  Fonts and reading layout now share the normal Studio settings;
  interface language now shares the normal Studio preference too. Remaining
  workspace/dialog translation coverage and advanced settings are still pending.
  See [native interface language](#native-interface-language).
- Settings → Appearance → Fonts and reading layout provides interface, reading/
  conversation, workspace and code/terminal typography, content inheritance,
  comfortable/wide/full reading width and increased contrast. Without saved
  typography, GTK and VTE keep GNOME defaults. System high contrast remains
  independent. Changes apply after explicit Save; Reload and closing retain dirty
  edits until the user chooses to discard them. Existing widgets and subsequently
  opened views use the same saved plan. Markdown clamps individual realized rows,
  preserving its scrollable virtualized ListView.
  The authenticated `/studio/native/appearance` API validates font fallback lists
  with Rust's unmodified `cssparser` library on a worker. It reads at most 2 MiB
  of settings and accepts at most 64 KiB per patch. A scoped revision rejects
  concurrent appearance edits while preserving unrelated and unknown settings;
  atomic writes update the existing `settings.json`. Native window/theme state
  remains in `gnome.json`. Font changes cover GTK rich text, source editors,
  tables, file lists and VTE. Inline Markdown code now uses the saved code family,
  absolute size and weight in short labels, reused table cells and paged rich
  paragraphs. Nested strong/italic code retains its emphasis. EPUB/browser
  content keeps its own rendering preferences. Native Mermaid text/flowchart
  defaults are supported below; Mermaid.js themes/layout engines, browser
  typography desktop acceptance and accessibility remain in scope.
  Short labels retain the original worker-escaped markup and update only its code
  presentation using complete Pango font descriptions. This avoids combining
  mutable manual attributes with markup, a combination discouraged by
  [GTK's label API](https://docs.gtk.org/gtk4/method.Label.set_attributes.html).
  Long paragraphs update their existing code tag; no paragraph text, source
  offsets or reading pages are replaced. Weak bindings follow label reuse and
  reject deferred notifications from their own style update. Disabling custom
  typography restores the original markup and inherited tag sizing/weight.
  The two-launch `scripts/smoke-gnome-appearance.py "$NATIVE" "$GATEWAY" --headless`
  fixture verifies native Save/Reload/conflicts, actual Pango fonts, existing and
  new widgets, reset, reused cells, selected Unicode quotes and restart with
  private profiles/providers. A parser test additionally verifies that quoted
  and markup-like font names cannot alter displayed text or markup boundaries.
  The inline-code checkpoint passes 121 native Rust tests (one ignored) and the
  optimized-profile check, plus long-rich-text and four-launch application
  regressions. The real 4,097,226-byte report opened in 1,058 ms with 64 opening
  heartbeat ticks. The rich reader mapped in 201 ms with a 24 ms maximum observed
  heartbeat gap; cold conversation layout still showed a 445 ms gap, close to
  earlier debug observations of 438–441 ms. These are local observations, not
  release performance guarantees. No fresh release executable/install bundle is
  produced for this checkpoint.
  Markdown layout now provides **Technical**, **Reading** and **Compact** in the
  same dialog. The shared setting remains `markdown.mode`; the native API exposes
  `values.markdownMode` and merges only that field, preserving future Markdown
  metadata. Reading increases body text by one pixel and adds block spacing;
  Compact reduces text by one pixel with an 11 px floor and tightens spacing.
  Technical uses the saved reading size. The native line-height factors match
  the existing layout modes (1.86/1.72/1.55); actual pixel advances depend on font
  metrics. GtkLabel uses CSS, while paged rich paragraphs use a persistent GTK
  text tag reapplied to each page. Markdown code blocks use 1.65, or 1.48 in
  Compact. Source/Edit, standalone source windows and the composer are separate
  roles and do not receive the Markdown layout adjustment.
  Mode changes update live widgets after explicit Save without replacing document
  indexes, text buffers or selected paragraph pages. Two private Wayland launches
  verify custom/GNOME-default fonts, reset and Reading-mode restart. With 19 px
  reading typography, measured wrapped-line advances were 52/45/38 px for
  Reading/Technical/Compact, with 12/8/4 px block margins. Actual screenshots of
  the Reading and Compact samples are inspected. These are local debug
  observations, not release performance guarantees. Browser typography in the
  existing desktop client controls its toolbar; this migration retains website
  styling rather than adding a new page-font override.
  The layout checkpoint passes 122 native and 197 gateway Rust tests (one ignored
  in each), optimized-profile checks for both, long-rich-text/source-projection
  acceptance and the four-launch application regression. The real 4.1 MB report
  opened in 1,064 ms with 64 opening heartbeat ticks. The rich reader mapped in
  202 ms with a 26 ms maximum observed heartbeat gap. Release packaging and the
  wider desktop/provider acceptance gates remain outstanding.
  **Mermaid diagrams** offers Auto/Classic/Neo/Hand drawn/Document styles,
  Auto/Dagre/ELK layouts, text size (12–20 px), Compact/Standard/Loose spacing,
  and Rounded/Straight/Step/Basis curves. Text follows reading/code font roles.
  Settings apply after Save to conversation and document diagrams, including
  views remapped after Edit. Superseded work is cancelled; late replies cannot
  replace current output or write unsaved buffers. The shared appearance API
  exposes `mermaidStyle`, `mermaidLayout`, `mermaidFontSize`, `mermaidDensity` and
  `mermaidCurve`; revisions detect concurrent changes while preserving unknown
  stored metadata. Source directives retain upstream precedence.
  The default GTK queue uses the [bundled Merman worker](../native/mermaid-worker/README.md).
  Font preparation, environment-aware image lookup and validation run on its
  single bounded Rust CPU lane. Glyph outlining uses the same selected fonts as
  measurement; GTK's rasterizer does not enumerate fonts again for preview/zoom.
  Parent and child share a versioned, bounded protocol; the independent worker
  uses Rust 1.95 while the parent remains Rust 1.92. Normal packaging/install/CI
  include the worker and its 204 dependency entries with EPL/MIT/OFL notices.
  The 59 valid corpus cases pass through the real sandbox, alongside invalid
  input, file/home/network/environment denial, OS/output limits and descendant
  cleanup. Actual generated-formula counts survive outlining in SVG metadata;
  stripping only this attribute restores all 59 previous SVGs byte for byte.
  Static inline family layout, mixed scripts and role fonts retain the earlier
  qualification. Dynamic families, formula/object family overrides and unsupported
  geometry remain guarded; unsupported formulas/class-member math remain literal.
  The 80-node outlined benchmark still regresses against the former Pele path;
  this is a compatibility adoption, not a universal performance improvement.
  The two-launch Wayland appearance regression checks all style choices and
  layout controls, pixel parity between reader/chat, font/spacing/curve changes,
  restart, stale replies and exact dirty-buffer/original-file retention.
  Frame-aware captures use the existing GTK paint observer. Earlier capture
  failures and an unmapped test-page timeout remain in the private evidence.
- Native Markdown Preview/Source/Edit, file picker, outline, exact-text search,
  source line gutters, paged navigation, and native syntax highlighting.
- Native PNG/JPEG/WebP/GIF still previews and SVG viewing, with fit, zoom, original/
  preview dimensions, clipboard copy, reload and native file selection. Files,
  launcher paths and file links share the viewer. Local Markdown image paragraphs
  also render lazily in the reader/comparison, conversations, historical context
  and Favorites. See [native image bounds and verification](#native-images).
- Native shared session comments with Markdown, tables, PDF, EPUB and browser selection/source
  anchors, explicit editing/saving, stale-save protection and composer insertion. See
  [native session comments](#native-session-comments).
- Native read-only PDF Preview/Text, fit/zoom, page entry and previous/next,
  contents, literal case-sensitive search, Preview/Text selection, Copy and comments,
  password entry, reload and file selection. Files, launcher paths and chat or
  Markdown file links share the viewer, including `#page=N` references. See
  [native PDF bounds and verification](#native-pdf).
- Experimental native EPUB controls with Rust archive/resource delivery,
  EPUB2/EPUB3 contents, chapter navigation, scroll/paged layout, text size,
  paper themes, current-chapter search, source-linked comments, selection copy and
  shared CFI reading state. Its actual-widget acceptance is still in progress; see
  [native EPUB](#native-epub).
- Rust worker parsing and bounded Preview context: small files include their
  logical fragments together; large files include the previous/current/next
  logical fragments. Continued tables form one paginated native table.
- Diagram generation has its own bounded queue and one CPU render at a time;
  chart work does not occupy the document paging/editing request lane. Replacing
  or closing a document cancels pending work, including sandboxed external
  engines. A shared 16 MiB memory-weighted cache reuses identical diagrams,
  accounting for preview pixels, retained vector output and source/style keys.
  This cache budget does not represent the whole application's memory use.
- Native table column selection, resizing, wrapping, and stable paging. A table
  page contains at most 50 rows and 512 selected cells. Long Markdown cells use
  bounded excerpts and an explicit full-cell reading window; see
  [native long table cells](#native-long-markdown-table-cells).
- Native CSV/TSV/XLSX viewer with Data/SQL tabs, worksheet selection, automatic or
  explicit delimiter selection, resizable wrapped columns, full-cell viewing and
  Copy, and native charts. SQL has syntax highlighting, Compact/Expand, and Query
  inside its editor; Chart, Copy and Wrap are grouped on the right. Files, document
  pickers, launcher paths and chat links open this viewer. Explicit line references
  continue to open Source so physical line anchors remain meaningful. See
  [native table bounds and verification](#native-tabular-documents).
- Explicit save and save before crossing edit pages; view switches retain edits
  in memory. Atomic saves preserve Unicode/CRLF, validate the current revision,
  retain GTK undo/redo across saves, and protect externally modified files.
  Window/document close and opening a replacement document offer Save/Discard/
  Cancel for dirty edits.
- Read-only Markdown comparison with independent file pickers and Preview/Source
  views, including files in different directories. Either side displays before
  the other is opened. Rust compares immutable indexed snapshots; only bounded
  line coordinates reach GTK. Source change navigation highlights changed lines;
  it is disabled when either side uses Preview. Returning to Reader retains the
  left file and protects an existing dirty editor.
- Native VTE terminal using the existing gateway PTY/environment. Paste/keyboard,
  ANSI/Unicode, clipboard, resizing, stale-connection isolation, and owned-shell
  cleanup use native widgets and the authenticated terminal stream.
- Native Files uses a lazy expandable GTK tree, paged Rust directory scans,
  hidden/ignore controls and live filesystem updates. Native project selection
  and bounded source/code windows preserve exact line gutters. Out-of-project
  symlinks remain confined by the shared Rust file service. See [Files bounds
  and behavior](#native-files).
- Native Git Review for unstaged/staged files, explicit stage/unstage, paginated
  commit history, commit files, file/commit filters, old/new diff gutters and
  syntax-highlighted historical diffs. Open File shares the native Files dispatch;
  closing Markdown/source returns to Review. See [native Git Review](#native-git-review). File,
  document, terminal, source and Git panels share one exclusive workspace pane.
- Experimental native browser workspace with GTK tabs/navigation/search/zoom,
  isolated page content, private sessions and shared source-linked comments.
  See [native browser scope and gates](#native-browser-workspace).

Ctrl+Enter sends a message; ordinary Enter preserves native multiline/IME input.

The full migration remains in progress. Use the
[current parity inventory](gnome-native-status.md) for present gaps and next work.
The records below document Router execution, commands, settings, EPUB/browser,
history/output and recovery increments; their older “remaining” lists are not
current feature inventories.
Mermaid/PlantUML/D2 now render in native GTK images, with shared Fit/zoom/Copy
inspection and bounded vector rerasterization. Tiled zoom, advanced SVG features, and full
rendering parity remain in progress.
The native frontend is experimental, not yet a feature-complete replacement for
the Tauri app.
The transcript shows a bounded tail of 400 items/32 MiB of presentation data and
truncates individual large outputs at 128 KiB. Assistant replies use native
structured Markdown, including coalesced live previews while deltas arrive; see
[native rich streaming](#native-rich-streaming). Bounded history navigation and
complete-output text paging are available separately from the live tail. Remote
images and bounded animation are implemented. Cross-block selection is still
limited to the opt-in continuous reader, whose promotion checks remain open.
Paged inline math and Mermaid formula labels now use native Rust rendering;
see [Mermaid label math](#math-inside-mermaid-labels) for its bounds and coverage.
The document reader retains its separate bounded file/index/edit pipeline.

## Native Session Map: in progress

The native application now exposes **Session Map** from the main menu. Its
virtualized topic list retains the complete supported map, with goal, completion
definition, current ancestor trail, progress and revision. Topic buttons set the
current subject; Done, Undo, Reload and confirmed Delete use the shared store.
Imported labels remain literal, with NUL replacement confined to GTK display.

Create Map accepts an optional goal, completion definition and structure. With
no goal, an isolated Codex-compatible utility infers it from the first two and
last eight captured interactions; generation then creates the initial outline.
An explicit OpenCode goal can create a map, while AI generation retains the
existing Codex-compatible provider restriction. Generate/Update with AI uses the
selected conversation model/effort and safe incremental operations. It cannot
claim human completion, change the goal, archive, move, reorder or delete topics.

The structured utility transport is shared with native translation, with separate
output schemas and byte budgets. Each Map utility owns an ephemeral thread and
cleans it up without sending a prompt to the source conversation. The UI captures
at most the first 32 and last 256 item references; workers select/pack text. Goal
inference includes at most ten interactions (4,000/6,000 bytes per user/assistant),
while outline updates use the latest twelve within a 48,000-byte text budget.
AI map context includes at most 80 items with their ancestor chains. Assistant
operations are validated as a whole, capped at 40, and applied against the frozen
map ID and revision. Generation is cancellable; changing sessions invalidates
presentation and cancels its task. An uncertain/conflicting mutation requires a
reload. A created map survives a later generation failure.

The packaged release above remains the SVG checkpoint. These Map changes are
validated in the debug worktree and do not claim a newer installed release.

`src-tauri/src/session_map_types.rs` shares the exact camel-case map, item,
relation and operation contracts between the existing SQLite service and native
client. Validation remains shared; parent lookups now use one ID index rather
than repeated linear scans. Native workers prepare ordered visible rows, ancestor
trails and completed-topic counts for the native pane's immutable snapshot.
The complete supported 1,000-item map is retained; malformed, cyclic, oversized or
foreign-session snapshots are rejected. Archived-parent orphan handling matches
the existing frontend. Titles/goals/summaries remain literal data.

All five existing map endpoints now parse, lock, access SQLite and serialize on
blocking workers, with eight admitted jobs per gateway. A single-thread request
runtime test holds the database mutex while eight requests wait, verifies that
another request receives 429, then releases the lock through that same async
runtime and checks all permits return. A database wait cannot block notification
and HTTP delivery on the request thread.

Undo and delete accept optional `revision` and `mapId` query parameters, checked
inside the same transaction as the mutation. Operations also accept an optional
`mapId` alongside their existing `baseRevision`. A stale revision or map identity
returns 409 without consuming history or deleting/changing newer content. The
identity check also protects a replacement map whose revision has restarted at
one. Clients omitting these new fields retain their existing behavior and the
user/assistant operation rules. The native client always supplies both map ID and
revision, verifies response identity/revision, distinguishes a missing map from
conflicts, and marks a mutation's lost/malformed reply as uncertain. It never
replays an uncertain mutation. Request serialization, snapshot validation and
tree preparation run on Rust workers; no provider task is started by these data
operations.

Seven native tests cover the 1,000-item projection, scope/cycle/field guards,
Unicode/path encoding, exact request payloads, successful create/update/undo/
delete, revision/identity conflicts and lost-reply behavior. Eleven gateway map tests cover
the shared store, existing HTTP flow, conditional mutations and bounded async
admission. The existing 23 browser Map tests pass with the project's
`--experimental-test-isolation=none` runner. At that data-layer checkpoint, the native/gateway suites passed
212/230 tests (two/one normally ignored), with Rust 1.92 checking.

Final four-launch regression `run-bg7728vh` passes after the identity guards. Its
real 4,097,226-byte document opened in 902 ms with 55 opening GTK ticks. Source
lines, paging/editing/conflicts, comparison, Files/Git, providers and restart
remain covered. Sampled native/gateway peaks over the full debug workflow were
595,592/193,596 KiB. All 544 Rust tests pass, alongside the 23 browser Map tests.
These are existing-workflow regression observations, not native Map UI acceptance.

The new Map unit coverage also checks actual conversation roles, UTF-8 packing,
first/recent goal context, current-ancestor retention in a 1,000-item map,
all-or-nothing safe operations, cancellation before dispatch, and independent
structured utility output handling. At that controls checkpoint, the native
suite passed 218 tests with two normally ignored checks.

Native Map acceptance `session-map-b0qb_kgc` passes two actual Wayland launches:
optional-goal inference, initial outline, Set current, human Done, Undo, explicit
AI update, cancellation, late results after a session switch, draft retention,
restart persistence and confirmed Delete. All five fake model turns run in owned
ephemeral tasks, all five tasks are deleted, and source history remains byte-for-
byte equivalent. Owned provider processes exit after each launch. Sampled edit/
restart native peaks were 338,880/299,720 KiB; gateway peaks were 47,024/45,960 KiB.
The screenshot was inspected and the current-topic label uses readable bold
accent text. Shared translation acceptance `translation-9ooj53og` also passes
after transport reuse, including early completion, cancellation, invalid output,
Codex/OpenCode/local execution and utility-event/catalog isolation.

Four-launch regression `run-cz6m6ea0` passes with the native Map controls present.
The real 4,097,226-byte report opened in 939 ms with 57 opening GTK ticks; source
anchors, edit/conflict handling, comparison, Files/Git, providers and restart pass.
Sampled native/gateway peaks for that complete debug workflow were
599,992/198,984 KiB. The 23 browser Map tests, Rust 1.92 check, formatting and
generated-language verification pass. These observations do not claim Map
automatic-turn synchronization or a newer optimized release bundle.

Native direct and queued conversation turns now prepare Map context in the Rust
socket worker. An asynchronous store read freezes map identity/revision before
`thread/resume` supplies bounded developer context and `update_session_map`.
Unsupported dynamic tools retry only that configuration step with developer
context; the original `turn/start` is sent once with its input/options intact.
No extra inference is started for automatic synchronization.

The socket owns live Map contexts independently of GTK selection. Safe tool
arguments are parsed on CPU workers, applied as assistant operations against the
captured version, and acknowledged with the app-server tool result. Duplicate
call identities receive their existing result; changing their arguments is
rejected. Own successful writes advance the captured version for subsequent
sequential tool calls; external edits never rebase it. Successful tools suppress
the final fallback. Uncertain/conflicting tool writes also block that fallback,
so a lost reply cannot cause another mutation attempt. An accepted write whose
reply was lost is never replayed.

When tools are unavailable, the same turn's trailing metadata is parsed once at
completion. Missing, malformed, oversized, unsafe or stale blocks leave the Map
unchanged; an empty update is a no-op. The next queued turn waits for any pending
Map write before reading its own context. Completed-turn and duplicate-call
state is bounded: eight preparation/update jobs, 32 retained turn contexts and
128 completed tool replies. The streaming filter retains only undecided marker
prefixes and bounded private suffixes (128 items / 4 MiB total), rather than full
visible replies. Restart/stopped/error generations discard old contexts and
partial stream state. Deleting a map clears remembered configuration before its
following normal turn.

Private markers and partial markers stay out of streaming text. Completed and
history projections strip the suffix before native rich-text parsing, copy,
favorites or full-output snapshots. Provider history intentionally retains the
original suffix, as in the existing web workflow. Automatic updates require a
captured native turn context; unrelated/historical tool requests are rejected.
A conflict or uncertain write reloads the pane's authoritative snapshot, retains
the explanation, and leaves explicit Map actions usable after recovery. Progress
and revision remain visible separately from feedback. The native client now
uses the common gateway error decoder, including the real `error.message` shape.

Final seven-turn acceptance `map-sync-h9yal8kd` passes through actual Send/Queue
controls and the owned Wayland clipboard. It verifies offscreen duplicate tool
calls, tool-versus-fallback precedence, unsupported-tool configuration fallback,
split streaming markers, offscreen updates, queued context revisions
`1, 2, 3, 4, 5, 6, 6`, a newer human Done surviving stale output, empty/malformed
no-ops, conflict recovery, and the actual Copy action. It starts seven source
turns and zero repair/utility inference turns. Sampled native/gateway peaks were
344,896/46,056 KiB. The screenshot was inspected. Native tests now pass 230 cases
(two normally ignored), including generation changes, nested API errors and an
accepted tool write whose reply is lost before a fallback arrives.

The final controls/restart case `session-map-trf_bies`, translation case
`translation-nt8q8bzf`, Rust 1.92 check and all 23 browser Map tests pass.
Four-launch regression `run-ssiwbfe3` also passes: the real 4,097,226-byte report
opened in 903 ms with 56 opening GTK ticks, with source anchors, editing/conflicts,
comparison, Files/Git, providers and restart retained. Sampled native/gateway peaks
were 597,132/188,160 KiB. These describe the direct-sync debug checkpoint; the
optimized bundle has since been refreshed with routed Map support below.

### Routed Session Maps

The server-side Thread Router now owns Map synchronization for routed Codex
target turns. Native direct turns and routed turns share the same pure Rust
context builder, tool schema, operation validation and private-suffix parser.
The Router controller keeps its strict decision JSON protocol and clears inherited
Map tools/instructions; its Map is available for manual actions. OpenCode
automatic Map synchronization remains unsupported.

Before dispatch, the gateway freezes the target Map ID/revision and persists that
scope with the routed job. The private turn acknowledgement binds ownership
before the next provider frame can arrive. The gateway consumes owned tool calls
before broadcasting to viewers, acknowledges duplicate calls without repeating
writes, and permits a captured same-turn metadata fallback only when no tool
mutation was attempted. User changes, replacement Maps and provider-generation
changes reject stale writes. Map failures retain the source reply and surface a
bounded notice in the Router record; private metadata stays out of replies,
rendered items and context excerpts. Store-change events refresh an open native
Map pane. Worker admission, tracked scopes and receipt caches are bounded.

Restart recovery monitors the existing source turn and reconciles its captured
metadata against the persisted scope. It recognizes a write already committed
by that turn. It never starts or replays inference, and rejects resumed live
tool calls whose original ownership cannot be established.

`scripts/smoke-gnome-router-map.py NATIVE GATEWAY --api-only` covers nine dispatch
cases and two gateway restarts with private providers. `router-map-nge3wior`
passes tool precedence, duplicate/early/unsafe tools, unsupported-tool fallback,
malformed/empty metadata, newer human Done, Map replacement/removal, controller
isolation and both restart cases. Removing `--api-only` also exercises actual
native Router Send, live target panes and record notices. Wayland run
`router-map-ui-ub8rm7_h` starts two controller and two source turns with zero
extra Map inference turns; native/gateway peaks were 345,356/48,616 KiB. Its
painted record screenshot was inspected.

The existing Router API/UI regressions, seven-turn direct/queued Map acceptance,
23 browser Map tests, 230 native tests and 237 gateway tests pass. Rust 1.92
checking also passes. Four-launch regression `run-ns6qtmdx` opens the real
4,097,226-byte report in 923 ms with 56 opening GTK ticks. The matched optimized
pair passes installed `release-ui-9vd9nll9`, including all five optional groups
without application hooks or a gateway override. Optimized gateway run
`router-map-vr7rbrpe` repeats all nine routed cases and both restart recoveries.
Installed Map run `release-map-vi2askk5` checks ordinary keyboard menu navigation,
current topic, human Done, Undo, pane closure and persisted revision 4 through
real accessibility controls, with zero model turns. Native/gateway peaks were
312,324/28,100 KiB. This run also exposed empty accessible names on generated
GMenu rows; the subsequent shell-menu implementation is described below. Extended language,
concurrent-client, physical accessibility and actual-provider acceptance remain
migration gates. The full application migration remains open.

### Native shell menu accessibility

A minimal GTK-only reproduction on this host also exposes unnamed generated
model-menu rows, including after explicitly setting their accessible label.
Ordinary GTK buttons expose their labels correctly. Studio's shell menu now
projects its existing GMenu actions/sections into public GTK buttons with the
MenuItem role inside a scrollable native popover. Window action identity, targets
and enabled state stay with GTK Actionable. The original GMenu remains the source
of translated labels. Coalesced model changes rebuild at most 128 entries with
bounded section depth and retain focused action identity. Arrow/Home/End
navigation skips disabled actions; Enter, Tab and Escape retain native behavior.
This change does not alter generated text-selection menus elsewhere.

Debug accessibility run `release-map-t198vpjs` verifies that all shell action
names are present and ordinary keyboard navigation reaches Session Map, followed
by actual current/Done/Undo actions and persisted revision 4. It starts zero
model turns. The native suite still passes 230 tests with two normally ignored.
Rust 1.92 checking and the optimized build pass. Optimized Map run
`release-map-cj3cfdio` repeats the named-action/keyboard/persistence checks.
Installed `release-ui-9xzd46qa` passes all five workflow groups, including menu
names in Chinese and English after live locale changes. Sampled native/gateway
peaks were 632,872/108,980 KiB, with explicit EPUB/browser content sampled
separately. Physical assistive-technology acceptance remains open.
Final installed Map run `release-map-ezqyw17t` additionally activates Settings
through its named menu action, verifies menu dismissal and closes the dialog
with Escape. Named actions, keyboard navigation and revision-4 Map persistence
all pass with zero model turns; sampled native/gateway peaks were
340,268/28,868 KiB.

## Native Git Review

Git Review has native Unstaged, Staged and History scopes, explicit Stage/
Unstage, and paged commit retrieval. The file filter matches current and previous
rename paths. In History it instead searches loaded commits by full/short hash,
subject, author name and email; its placeholder/counts explicitly describe that
retrieved scope. Matching is literal and case insensitive, with surrounding query
whitespace trimmed, as in the existing web review. Displayed paths remain exact.
Commit files have a separate count and retain the gateway's truncation notice.

Filtering runs on the bounded Rust worker lane against immutable shared model
arrays. History pages also merge and deduplicate there. GTK receives selected
indices and reconciles only changed model spans; unchanged prefix/suffix objects
survive paging/filtering. The reusable `list_store` helper avoids rebuilding
unchanged rows. Latest-request and project/scope generations reject obsolete
results. Selecting another file clears its old diff and disables actions until
the matching response arrives. A hidden file/commit selection clears its stale
content. Filtering failures do not clear an unrelated in-flight mutation guard.
The gateway retains Git execution, confinement and the existing API. Local Rust
workers prepare origins/projections from already-needed decoded data, avoiding
large per-line JSON arrays or repeated Git reads over the private connection.

Diffs remain read-only, highlighted GtkSourceView text with native old/new gutters.
The reusable Rust `unified_diff::Index` is built alongside the text index on a
worker, without copying or reformatting the source. It accounts for hunk ranges,
zero-length sides, no-newline notes, content resembling file headers, Unicode and
CRLF. Unrecognized/combined hunks have blank origins. The origin index retains at
most 2 MiB (262,144 physical diff rows); subsequent origins stay blank with an
explicit limited-mapping notice. Index construction, transport and the existing
text index have separate costs. The visible buffer keeps the existing 200-line/
128 KiB windows, including a stable origin when one long line crosses windows.
Git's 4 MiB response truncation and binary status remain visible. Conversation
turn diffs reuse the same index/gutters and charge the index to their byte cache.
Ordinary source reads, including `.patch`/diff syntax and unknown languages, keep
physical file line numbers; the operation type determines whether origins apply.

Open File dispatches a working-tree file through the same native viewers as Files;
it is unavailable in commit History. Closing a Git-opened Markdown/source file
restores Review. The Markdown parent-navigation callback runs before hiding the
pane, so GtkStack cannot select an unrelated fallback first; comparison panes
retain their existing post-close behavior. The shared viewer return policy below
also covers images, PDF, EPUB, tables and Browser. Advanced review/comment
presentation and physical input remain in scope.
An inactive Git panel's delayed close cannot hide a subsequently opened document.

Native file/commit rows now have literal accessible names, and activating a
focused row selects it. The Git search field has an explicit accessible name,
independent of its placeholder. Fixed UI-owned accessible names use the shared
weak translation registry; model paths/subjects and editable values stay literal.
Core controls, scope choices, counts and ordinary file-state labels follow live
interface-language changes. This does not establish full workspace translation
or assistive-technology acceptance.

The 35-test shared model suite covers malformed/exhausted ranges, UTF-8/CRLF
windows, long-line splits and bounded origin storage. Native tests cover rename/
scope and author/email filters, worker-side history merging, binary/source
preservation and model identity retention. The 175-test native suite and
210-test gateway suite pass (two and one normally ignored checks respectively).
Rust 1.92 checking and optimized builds pass.

The expanded four-launch Wayland fixture uses a real private Git repository with
103 commits. It checks old/new origins, file/commit filtering, hidden-selection
and loading guards, explicit stage/unstage, two history pages, stable model
objects, commit switches during paging, generation isolation, and actual
Markdown/source Open File with return to Review. `run-3akiikhx` passes alongside
the real 4,097,226-byte Markdown report, paged editing/comparison, Files, terminal,
conversations, settings, restart and owned-process cleanup. These private checks
do not send prompts to real providers. See the installed `--git` variant below
for ordinary accessibility/keyboard control coverage without application hooks.

The installed fixture also refreshes AT-SPI caches and requires showing controls
before activation. Git selection uses its exposed selection interface or actual
Tab/Enter focus, ignoring focused ancestor windows. With both `--language` and
`--git`, it opens Git in Chinese and checks the actual accessible search name,
returns through desktop file activation, then checks the same control in English.
The combined optimized run `release-ui-sl_dsd6r` passes all four optional workflows,
including independent verification of the staged file. Physical GNOME input and
assistive-technology sign-off remain separate gates.

Completed fixture installations can live on the external build disk while their
logs/profiles/screenshots remain under `target/gnome-native-smoke`. The completed
install prefixes were copied with SHA-256 verification, replaced by symlinks and
archived under `/media/rui/studio-gnome-deps/native-test-installs`; live executables
were skipped. This moved 7,425,405,872 bytes from 32 private installations without
changing their artifact paths. These are acceptance fixtures, not the user's
installed application or profile.

## Native viewer navigation

The application owns a bounded navigation chain of native pane identities; it
does not copy document content or rebuild a parent viewer when returning to it.
Files and Review use the same open dispatch as document links and desktop file
activation. Closing an image, source file, table, PDF or EPUB returns to the
originating pane. Hiding Browser follows the same policy while retaining its
existing tab lifecycle. Nested viewers unwind in order, closed parents are
skipped, and reopening an ancestor collapses the chain rather than creating a
cycle. Explicit workspace selection starts a new base. Closing the final pane
hides the workspace and clears its history.

Open callbacks run at the accepted user intent, independently of worker replies.
An EPUB open deferred by a pending reading-position save cannot take focus again
when that save completes. Internal table refreshes and EPUB save/load replies do
not present a pane. A stale close can dispose its own resources but cannot hide
the active viewer. Markdown retains its save/discard guard and its pre-hide
navigation hook; comparison panes retain their own close behavior.

`scripts/smoke-gnome-workspace-navigation.py NATIVE GATEWAY` creates private files,
fake providers and an owned Wayland compositor. It exercises real Markdown link
dispatch, close-button signals, nested image/PDF return, source/table return,
deferred EPUB completion, Browser hide during preference loading, ancestor cycles,
closed-parent skipping, hidden reset and unsent conversation draft retention.
EPUB's content renderer is intentional;
the application shell and viewer controls stay GTK. The separate installed
`--viewers` case uses ordinary desktop activation and accessible native buttons
without application test hooks. It also opens and closes an image over an unsaved
Edit buffer, checks that the exact buffer returns, and confirms no implicit save.

The 180-test native suite passes (two normally ignored checks), as does Rust 1.92
checking. The dedicated Wayland run `workspace-navigation-twoh8pc1` passes all
viewer cases, including deferred Browser/EPUB focus isolation and owned-process
cleanup; its initial image/table/source/PDF sequence recorded 23 heartbeat ticks
with a 49 ms maximum gap. These are fixture observations, not latency bounds.
The four-launch regression `run-0ljnwwtl` also passes, including the real
4,097,226-byte report, comparison, paged editing, Git and restart. Report opening
took 1,029 ms with 63 main-loop ticks.

The optimized installed pair passes all five optional workflows in
`release-ui-cd61jyfw` (`--all-diagrams --language --backends --git --viewers`).
It samples native/gateway peak RSS of 634,140/77,000 KiB and intentional EPUB
content-process peak RSS of 241,444 KiB; those values exclude other worker peaks
and do not define a memory bound. No model turns are sent. These checks do not
replace physical GNOME input/portal acceptance or establish full migration parity.

## Native viewer recovery and split-pane bounds

The native table, PDF and EPUB viewers now restore their read-only leases after
explicit gateway replacement. `native/gnome/src/viewer_resume.rs` performs the
open/validate/read sequence on Rust workers. The retained canonical path and
source fingerprint must match before a candidate is exposed to GTK. Failures
close the candidate and keep the original display, drafts and settings available;
server operations stay disabled until an explicit reopen or successful retry.
No document contents or conversation requests are written/replayed by recovery.

Table recovery recreates the last **successfully executed** read-only SQL query,
not the text currently being edited. A separate draft can therefore survive
without running automatically. It restores worksheet/page position and uses new
document/query IDs; column widths, Wrap and editor state remain in GTK. Result
pages are bounded by the existing table budgets. Dynamic SQL can produce a fresh
result when rerun; cell selections and pending comment captures are invalidated.

PDF recovery validates the opening fingerprint, then reads the retained page
from that new immutable lease. The compact page endpoint omits the name/path/hash
sent by Open, so the worker carries that validated metadata forward and verifies
the page's lease ID and index. It releases the first raster before requesting
another. GTK keeps zoom, fit/preview/text modes, Preview scroll position and search options.
Outline/search callbacks are recreated with the new generation. Refreshing search
does not force a previously closed sidebar open or change its selected tab.
Text-mode search retains its normal behavior of revealing the first matching hit.
Encrypted files require unlocking again; passwords are not retained as recovery
preferences.

EPUB recovery validates the book fingerprint and original root/path, then applies
the retained CFI, chapter, font scale, theme, flow and contents-panel preference to
the fresh metadata. Its content surface is replaced only after validation. Old
asset requests and late messages cannot authorize the new document. Candidate
cleanup Close replies are distinguished from owned reading-position receipts, so
they cannot release another close guard. Reading-position persistence resumes
through the normal new-document flow after validation; recovery itself sends no
Save or final reading update.

`scripts/smoke-gnome-discovery.py` adds `runtime-viewers` and
`runtime-viewers-conflict`. The private profile opens all three viewers, selects
the second SQL-result page, leaves a different SQL draft, adjusts a column and
Wrap, changes PDF page/zoom/scroll/search, and sets an EPUB CFI and reading options.
It exits the owned gateway while a paused queue, dirty Markdown buffer and admitted
export also exist, then explicitly reconnects. The unchanged case checks fresh
leases, exact EPUB CFI and retained view state; the conflict case changes all three
private files and checks disabled operations with the old content retained.
Both inject retired replies and verify background recovery does not steal focus.
Real providers and user documents are not modified.

The layout probe also exposed pane clipping with large fonts: allowing a
`GtkPaned` child to shrink below its measured minimum could allocate that child
partially outside the divider. Both children now enforce their GTK minima, so
divider travel adapts to content and typography. The allocation test checks the
effective position against the actual minimum/maximum; it never accepts clipped
controls simply because a requested position was impossible. Twenty Wayland
cases cover normal/long model names, context search, a three-message paused queue
and large typography, including both sides of the divider and a wrapped model
popup. The first queued fixture attempted another Enqueue before persistence
finished; it now waits for each scoped write rather than bypassing that guard.

Validation passes 274 native tests (two ignored), Rust 1.92 all-target checks and
release all-target checks. Final recovery runs are
`discovery-runtime-viewers-ntrkwtwn` and
`discovery-runtime-viewers-conflict-qwo42ot6`. The former requires an actual
mid-chapter EPUB position, the exact retained CFI, and a position-version advance
after gateway loss from the new document surface. Its inspected screenshot shows
paragraphs 49–54, rather than accepting a copied chapter-start CFI as location
proof. The earlier probes only required nonempty CFIs; they were insufficient to
prove this stronger behavior. The fixture now lets reading-preference reflow
settle before navigating to paragraph 50. Observed debug native/gateway peaks in
the final unchanged case are 503,164/94,424 KiB with all viewers and Markdown open;
these are sampled workflow observations, not release bounds.

`layout-ui-1clej66a` passes all twenty allocation cases with no clipping; captures
were inspected at the large-type narrow-pane limit. Ordinary regressions pass in
`run-10m8ayqg` (four document launches), `epub-ui-g3fjwnmn` (EPUB controls, persistence
and security), `pdf-ui-k1oghw7p` (including password handling), and
`tables-ui-1d53y1sf` (CSV/XLSX, SQL, full-cell Copy, wrapping and chart controls).
The table regression exposed an older capture race: the dialog object existed
before its content mapped, and the synchronous capture helper quit the test app.
That fixture now waits for actual GTK mapping/painting before capture and Close.
A separate PDF probe first hid search results before GtkSearchEntry's delayed
search signal had fired; it now waits for that real signal before recording the
sidebar state. Failed and intermediate runs are retained with final evidence in
`/media/rui/studio-gnome-deps/qa-artifacts/native-smoke-jwfgtzto/viewer-recovery-validation/VALIDATED.json`.

The installed candidate and release are unchanged. Full migration remains active;
expanded viewer-recovery combinations and the existing rendering/selection,
language, physical-desktop and platform/release acceptance remain in scope.

### Multi-sheet workbook recovery

Worksheet changes now require an available document lease. A late chooser
notification during gateway loss restores the retained selector instead of
clearing its SQL result page or editor draft. Selecting the already active sheet
also leaves its result intact. This fixes the reproduced offline callback failure
in `discovery-runtime-viewers-dyhjglo9`.

The owned Wayland fixture can use a private three-sheet XLSX workbook:

```bash
python3 scripts/smoke-gnome-discovery.py DEBUG_NATIVE GATEWAY \
  --mode runtime-viewers --table-recovery xlsx
```

Each worksheet has distinct row values. The test executes different queries on
the first two sheets, leaves separate Unicode SQL drafts, and reconnects from
the second sheet's second result page. It checks fresh document/query IDs,
worksheet selection, exact rows, Wrap and column width. A post-recovery worksheet
round trip retains both drafts and their separate last-executed queries; selecting
a sheet does not execute its draft. The private query trace verifies that only
the previously executed query is rerun during recovery. The conflict variant
changes the workbook and retains the old display with server controls disabled.

`discovery-runtime-viewers-h9x4_ruv` and
`discovery-runtime-viewers-conflict-lk05ao2f` pass these cases. The options can be
combined with `--pdf-recovery encrypted --book-search inflight`;
`discovery-runtime-viewers-inflight-rhcnzq6h` and
`discovery-runtime-viewers-conflict-inflight-r_gur1zu` pass that combined workflow,
including the paused queue, dirty Markdown buffer and admitted export. No provider
prompt is sent. Sampled debug native/gateway peaks in the unchanged combined run
are 518,852/95,312 KiB, excluding WebKit subprocesses; these are observations,
not memory bounds. The inspected table capture shows sheet Second, rows 51–100
of 150, descending values starting at 1099 and the separate unsent draft.

The ordinary table regression `tables-ui-279q_a8e` passes CSV/XLSX, SQL, paging,
wrapping, column resizing, full-cell Copy and chart controls. Native tests pass
274 cases (two ignored), with Rust 1.92 and release-profile all-target checks.
The four-launch application regression `run-utpigezb` passes against a private
copy of the 8,340,599-byte report, including paged editing, comparison and restart;
its reader checkpoint records 1,320 ms to open and 80 opening heartbeat ticks.
The original CSV recovery workflow passes as well. Evidence, source hashes and
the original failing offline probe are retained in
`/media/rui/studio-gnome-deps/qa-artifacts/native-smoke-jwfgtzto/xlsx-recovery-validation/VALIDATED.json`.
The installed candidate and release are unchanged. The broader migration
acceptance remains in scope.

### Supplementary EPUB navigation recovery

EPUB resources outside the spine retain their resource path and latest link
fragment during gateway replacement. A same-section link now records its new
fragment before scrolling; previously it left the original fragment in state,
so recovery could jump back to an earlier passage. It also clears the superseded
navigation CFI/end target. Normal spine reading keeps its existing CFI behavior.

Supplement navigation is transient: it does not replace the last main-chapter
reading CFI or invent a spine entry. The verified recovery lease loads the
retained supplement with its fragment, while preserving the main chapter and
font/theme/flow/contents choices. This covers link targets, not exact restoration
of an arbitrary subsequent scroll within a supplement, nor supplement restoration
after closing and relaunching the application.

The private fixture adds a non-spine XHTML resource with an encoded-space path,
Unicode text and two successive fragment links. It uses the real content-link
policy, verifies paragraph 70 is visible, and checks that the restored content
URL belongs to the new document lease. A retained chapter CFI alone is not
accepted as proof of the supplement's location. Changed files retain the old
display and disabled controls.

```bash
python3 scripts/smoke-gnome-discovery.py DEBUG_NATIVE GATEWAY \
  --mode runtime-viewers --book-location supplement
```

Use `runtime-viewers-conflict` for changed-file protection. The location option
can also combine with `--table-recovery xlsx --pdf-recovery encrypted
--book-search inflight`.

`discovery-runtime-viewers-k6kwju5j` and
`discovery-runtime-viewers-conflict-57zxowqm` pass unchanged and changed books.
The combined variants pass in `discovery-runtime-viewers-inflight-7u3ddvjh`
and `discovery-runtime-viewers-conflict-inflight-_82rmkig`, including the actual
held search reply, explicit password reentry, separate worksheet drafts, dirty
Markdown buffer, paused queue and admitted export. No provider prompt is sent.
The unchanged combined run samples native/gateway peaks of 519,444/94,796 KiB,
excluding content subprocesses. The inspected recovered EPUB capture shows
paragraphs 65–70 in the retained sepia/paginated view, rather than the earlier
fragment. The original failing same-section probe is retained alongside these
runs in
`/media/rui/studio-gnome-deps/qa-artifacts/native-smoke-jwfgtzto/epub-supplement-recovery-validation/VALIDATED.json`.

The ordinary EPUB API/two-launch native regression `epub-ui-e80o7dn1` also passes
styles, Copy, contents, hidden anchors, rapid replacement, search, preferences
and restart CFI restoration, with no network-canary requests. All 274 native
tests pass (two ignored), as do Rust 1.92 and release all-target checks. These
debug runs do not promote the installed candidate or complete the remaining
physical-desktop, rendering and platform/release acceptance.

### Native PDF unlock recovery

PDF unlocking now uses the application's native libadwaita AlertDialog, with
localized response labels, a suggested Open action and the password entry's
standard default-action support. The controller owns the prompt until its
response settles. Its identity, document generation and view-request serial must
all match before a password can dispatch an Open or recovery request. Opening a
different file, closing the PDF or losing the gateway clears the entry and
retires the prompt. A late response cannot act on its replacement.

The entry is read only for Open and cleared when its response settles, including
Cancel and Escape. Duplicate recovery attempts leave a currently active prompt
and typed value intact. A wrong password creates a fresh prompt; cancellation
keeps the previous verified page visible with server controls disabled. Explicit
retry asks again. After a correct password, the Rust recovery worker still checks
the original file fingerprint/path and selected page before returning a candidate
to GTK. Changed files keep the old snapshot and require reopening.

The implementation handles credentials in AlertDialog's
[response signal](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/main/signal.AlertDialog.response.html),
which settles when the dialog closes. Its initial prototype did not progress past
the unlock prompt; the final implementation owns the dialog through that response
and waits for native response settlement in the fixture. That failed probe is
retained rather than counted as acceptance. Passwords are not cached or used as
recovery preferences, and the fixture proxy logs only whether a password was
provided, never its value.

On an Ubuntu 26.04 host with the native dependencies and PDFium runtime, the
owned Wayland discovery fixture can select a private encrypted PDF:

```bash
python3 scripts/smoke-gnome-discovery.py DEBUG_NATIVE GATEWAY \
  --mode runtime-viewers --pdf-recovery encrypted
```

The corresponding `runtime-viewers-conflict` case appends a deliberate change
before reopening it.
Native Wayland cases `discovery-runtime-viewers-1t3btk88` and
`discovery-runtime-viewers-conflict-686kc19w` pass wrong-password retry, Cancel,
explicit retry, duplicate-attempt suppression, cleared entries and retired-prompt
rejection. Each verifies seven Open attempts: initial empty/wrong/correct, then
empty after gateway replacement, empty after Cancel/retry, wrong and correct.
The first replacement attempt carries no cached password. The unchanged case
retains page 2, zoom 1.35, Preview scroll 40 and the search/case/sidebar choices;
the changed case leaves the old view disabled with a persistent explanation.
Viewer source bytes are unchanged except the conflict fixture's exact appends,
and settings/SQLite do not contain the fixture password values. Screenshots of
the actual unlock controls and changed-file state were inspected. Sampled debug
native/gateway peaks are 511,512/94,480 KiB unchanged and 472,868/94,868 KiB with
conflicts; these exclude WebKit subprocesses and are workflow observations.

The normal PDF API/native regression `pdf-ui-0zn735x8` passes, including replacing
a document while its unlock entry contains text, rejecting that old prompt's
response, retrying a wrong password and the existing page/search/selection/copy
workflows. Native tests pass 274 cases (two ignored); Rust 1.92 and release-profile
checks cover all native targets. The four-launch application regression
`run-7rvdaweo` passes after a fixture-only terminal receipt fix: the old probe
accepted ANSI/Unicode output before the separately delivered PID line, causing
`run-_qgvi1w_` to fail early. It now waits for all output markers before checking
owned-child cleanup. Production terminal transport is unchanged. Final, failed
and intermediate evidence is retained in
`/media/rui/studio-gnome-deps/qa-artifacts/native-smoke-jwfgtzto/encrypted-pdf-recovery-validation/`.
The installed candidate and release are unchanged.

### EPUB search during gateway replacement

An open whole-book search dialog now survives an owned gateway replacement.
Previously its request and navigation closures captured the departed document ID
and generation. The retained dialog could display old results, but Search could
fail to dispatch and a result could close the dialog without navigating. Callbacks
now resolve the current document only when the original canonical root, path and
book fingerprint still match. Search response generations and serials remain
scoped to the owning request.

Gateway loss retires pending search replies and disables Search, Stop, paging and
result activation. The query draft, case choice, active query, result rows and
scan/page cursors remain in the same GTK widgets. A visible Reconnect button uses
the normal window action from inside the modal dialog. Revalidation does not
start a search automatically. After a verified reopen, the user can submit the
retained draft, continue a paused search or navigate the retained results. A
changed book retains the old display and explains that it must be reopened;
search and navigation remain disabled. A busy resume queue keeps an explicit
Reconnect attempt available.

Selecting a result immediately retires the closing panel's callbacks. Reopening
during its close animation creates a fresh dialog; the old closed handler can
remove only its own panel. Closed controls cannot send a query or navigate a
replacement book. Whole-book extraction and its existing byte/section limits stay
on Rust workers; recovery adds no browser-side book parsing or document writes.

Private Wayland runs `discovery-runtime-viewers-_p1la17c` and
`discovery-runtime-viewers-conflict-k_kymuu7` pass the full owned-gateway exit and
replacement workflow with the book-search modal open. They also retain the dirty
Markdown editor and undo history, paused message queue, admitted local export,
CSV query state, PDF view state and EPUB's exact mid-chapter CFI. Actual mapped
screenshots of the dialog and underlying viewers were inspected. The unchanged
case proves four search requests: one against the original lease, then an explicit
Unicode draft with a trailing space, a restored query in the same dialog, and a
query from the immediately reopened dialog, all three against the new lease ID.
It also checks the draft's case option and expired callbacks. The changed-book
case has only the initial request. No automatic search or provider prompt occurs.
Viewer source bytes stay unchanged, except for the conflict fixture's exact,
deliberate appends; hashes are retained with the reports. Sampled debug native/
gateway peaks are 480,832/95,364 KiB unchanged and 469,588/95,292 KiB with conflicts,
including all open viewers. These are whole-workflow observations, not release
memory bounds.

The ordinary API/native search regression `epub-search-ui-o2a8ueu9` also passes
140-section paging, case and trailing-space matching, Stop/late-reply rejection,
rendered-match navigation, adaptive dialog layout and replacement-book isolation.
Its API scans 24 MiB of filler in two bounded requests. Native unit tests pass
274 cases (two ignored); Rust 1.92 and release-profile checks cover all targets.
The first recovery passes did not query through the retained dialog or capture
the underlying viewers separately from its modal; they remain supplementary
evidence rather than replacing the final stronger checks.

The ordinary four-launch application regression `run-sz5nflht` passes as well.
Evidence, source hashes and intermediate runs are retained in
`/media/rui/studio-gnome-deps/qa-artifacts/native-smoke-jwfgtzto/epub-search-recovery-validation/`.

#### Paged and in-flight book-search recovery

Continue now distinguishes an interrupted result page from a full page. A page
below the shared 100-row soft limit resumes at its saved scan cursor and appends
to its existing rows. Only a full page records the next page boundary and clears
its display. Automatic batch scanning and manual continuation use the same
threshold; existing section/byte/row limits are unchanged.

Previously, interruption after 64 matches followed by Continue cleared those
matches and recorded a new page beginning at section 65. Previous then rebuilt
sections 1–112, overlapping the second page. The new behavior finishes the first
page before recording the boundary. In the all-matching 140-section fixture,
16-section batches produce exactly 112 rows on the first page and 28 on the last,
without missing or repeated section identities.

The owned Wayland discovery fixture accepts `--book-search paged` or `inflight`
with `--mode runtime-viewers` or `runtime-viewers-conflict`. It extends the private
book while preserving the original chapter source and paragraph-50 anchor. The
in-flight case holds a real gateway response for sections 65–80 after 64 results
are visible. It retains that validated response and its actual request serial,
exits the gateway, and verifies that replaying the old generation cannot change
the recovered dialog. Resume, Previous and Next are checked against every section
identity; request traces also verify exact scan cursors, draft case/trailing space,
old/new document IDs and zero automatic search. The changed-book cases keep
operations disabled. All source bytes are unchanged except the conflict fixture's
explicit appends.

For example, on an Ubuntu 26.04 host with the native development dependencies:

```bash
python3 scripts/smoke-gnome-discovery.py DEBUG_NATIVE GATEWAY \
  --mode runtime-viewers --book-search inflight
```

The original CFI probe used a three-chapter whole-book progress threshold; it now
computes the equivalent local-chapter threshold for either book size. It still
navigates to paragraph 50 and requires an exact retained CFI plus a fresh position
message from the replacement surface. A second fixture assertion now expects
"More sections available" for a bounded first page of the larger book, rather
than mistaking that pause for whole-book completion. The initial failed probes
are retained with final evidence.

Four final cases pass: `discovery-runtime-viewers-inflight-mzaaq0lh`,
`discovery-runtime-viewers-paged-p2477rgc`,
`discovery-runtime-viewers-conflict-inflight-5dkey2o1` and
`discovery-runtime-viewers-conflict-paged-zwacmarz`. The in-flight unchanged run
verifies 42 request records, including the interrupted batch; the paged run
verifies 41. Changed-book runs issue only the five/nine original requests.
Native/gateway debug sampled peaks are 498,716/94,924 KiB in the in-flight run;
these exclude WebKit subprocess memory and are workflow observations, not limits.
The normal search API/dialog regression `epub-search-ui-245m9zdk` also passes,
including paging, Stop/late-reply handling, adaptive layout and book replacement.
Native tests pass 274 cases (two ignored), Rust 1.92 and release-profile checks
cover all native targets. The ordinary three-section recovery cases
`discovery-runtime-viewers-4dq861lw` and
`discovery-runtime-viewers-conflict-jr4mtksf`, plus the four-launch application
regression `run-80ogu12m`, also pass after the final build. Evidence and manifests
are retained in
`/media/rui/studio-gnome-deps/qa-artifacts/native-smoke-jwfgtzto/epub-search-pages-validation/`.

The installed release and candidate are unchanged. Expanded multi-sheet XLSX
and supplementary EPUB recovery checks remain in scope, alongside
the renderer, input/accessibility, physical/provider and platform/release gates.

## Native gateway replacement and snapshot recovery

Reconnect can now replace an owned gateway that exits after readiness. The
supervisor keeps the shell's bounded channels, retires the old API, rejects
unadmitted requests and waits for admitted receipts and owned process cleanup.
It then waits for an explicit Reconnect attempt. Work submitted during the
replacement launch is rejected before new-service admission, rather than replayed
with new credentials. An independent Close signal remains available.

The document actor outlives gateway instances. Its local Insert/Delete tracking
continues while disconnected, preserving the original token identities used for
precise UTF-16 edits. A replacement opens a provisional reader from the original
canonical root, canonical path and access mode returned by the original gateway.
A relocated canonical target is rejected even when its bytes are identical.
A bounded 30-second revalidation checks the complete
64-digit source revision, access mode, exact page byte range, line range, source
size and baseline text. The visible GTK text must also equal the retained Rust
buffer. Only a verified snapshot replaces the session's reader/document IDs.
Failure closes the provisional lease and retains the original tracking state.
Local generation advancement also preserves further typing after a conflict.

GTK retains the edit buffer and undo history during rebind. It expires old reply
serials and advances the document generation before resuming. No Save is replayed;
the user must click Save after successful verification. A changed file or an
uncertain prior Save fails the hash check and leaves Save disabled. Persistent
recovery notices explain why edits remain in memory. Clean read-only Markdown
uses the same whole-snapshot check. Table, PDF and EPUB leases are retired on loss;
their state restoration is now implemented as described in
[viewer recovery](#native-viewer-recovery-and-split-pane-bounds). EPUB
reading-position timers are cancelled while disconnected.

The native suite adds private HTTP tests for exact offline edit composition,
new document headers, full-file conflicts outside the current page, inconsistent
page metadata, mismatched visible buffers, typing and retry after conflict,
read-only ownership, stale Close and an interrupted Save that must not replay.
Supervisor tests cover explicit attempts, offline RPC rejection, launch backlog
rejection and closing with a full request queue. Private Wayland modes in
`scripts/smoke-gnome-discovery.py` cover `runtime-recover`, `runtime-conflict`,
`runtime-stop` and `runtime-paged --document PATH`. The paged case copies the
specified real report into its private profile, edits a nonzero source window and
checks the final file byte-for-byte, including the unchanged prefix and suffix.
All provider prompts remain fake and explicit; these cases send none.

The initial replacement GUI run reached the new registry but did not reconnect
the existing backend ID: the normal backend switch intentionally skipped that ID.
Replacement discovery now requests a fresh connection when that ID is not ready,
preserving the draft and verifying history before Send. A further generation
confirmation handles Reconnect racing the launch readiness notification. The
private Python proxy also uses unbuffered stdin reads so owned process exit does
not race interpreter finalization's buffered-I/O lock. Per-launch PID history
allows cleanup checks to include every replaced process.

Final validation passes 272 native tests (two ignored), Rust 1.92 and release
all-target checks. The final owned Wayland cases are
`discovery-runtime-recover-eq81zp0c`, `discovery-runtime-conflict-7jedke2q`,
`discovery-runtime-stop-rfdvrf_j` and `discovery-runtime-paged-fk8g8rzr`.
The successful rebind cases perform real GTK Undo and Redo, then explicitly Save.
The paged case uses an 8,340,599-byte private copy of the user's current report,
edits byte window 4,118,353–4,160,556 and checks all unchanged surrounding bytes.
Its sampled debug native/gateway peaks are 383,884/47,884 KiB; these are workflow
observations, not release bounds. Screenshots show the persistent conflict notice
and the retained nonzero source line numbers. No real provider prompt is sent.

The ordinary four-launch report regression `run-rq4iqu3t` passes, including
document editing, saved preferences, comparison and restart. Provider regression
`backend-recovery-aepjvvir` passes selected/background Codex child exits and both
providers' transport recovery with the gateway alive. Initial launch replacement,
full-queue Close and permanent discovery failure also pass in
`discovery-launch-replace-qcpo_i6w`, `discovery-launch-close-0u3noyou` and
`discovery-permanent-vge1xgt5`. The installed candidate and release are unchanged.
All completed evidence, including intermediate runs and the initial same-backend
reconnection failure, is recorded in
`/media/rui/studio-gnome-deps/qa-artifacts/native-smoke-jwfgtzto/gateway-replacement-validation/VALIDATED.json`.
Full migration remains active. In addition to other viewer recovery and the
existing renderer/language/physical acceptance gates, review composer action
visibility at narrow split-pane widths shown in the current captures.

## Native running-gateway loss boundary

The native command loop now observes its owned gateway process after readiness.
An exit retires the shared API lease, closes ordinary request admission and
reports a distinct stopped state. API clones refuse operations begun after
retirement, including HTTP reads/writes, long commands, inline images, SSE and
socket construction. Existing requests remain pinned to the departed endpoint;
they cannot acquire a replacement gateway's credentials implicitly.

Queued commands that never reached a service receive scoped failure replies with
their original request IDs, generations, dialog IDs, serials and settings routes.
The exhaustive adapter does no I/O and never retries their work. Already-admitted
general workers settle against their original API, allowing an immutable local
output export to finish and release its close guard only after its real outcome
arrives. Document I/O is cancelled; dirty GTK edit buffers remain in
memory, with the existing normalization and 512-KiB edit-page limit. Offline
insertion/deletion updates that local buffer without queuing mutations for later
replay. Save, view switches and server-backed paging stay disabled until the
document service is available again. In particular, failure expires pending Markdown reply serials so an old
Save acknowledgement cannot mark newer text clean after the saving lock releases.

The shell pauses queues, retires backend generations and cached verification,
stops reconnect timers and disables live session controls. MCP/agent controls are
disconnected. Late transport/catalog/terminal events cannot restore readiness;
late conversation reads settle as errors. Programmatic session navigation also
preserves the offline draft's context. Confirmed local export receipts still use
their original scoped handlers. This is the retirement boundary for coordinated
service replacement. The original retirement checkpoint required restarting
Studio. Explicit in-app replacement and Markdown snapshot revalidation are now
implemented as described above. Dirty edits are not persisted as drafts.

`scripts/smoke-gnome-discovery.py NATIVE_DEBUG GATEWAY --mode runtime-stop` ends
the real private gateway through its parent pipe while fake providers are owned
by the fixture. It opens a dirty Markdown page, persists a paused queued message
and holds an already-admitted immutable export. After loss it verifies disabled
Send, stopped retries, retained draft/queue, an export close guard and exact
completion after releasing the latch. It injects an old scoped Save reply after
further typing, late backend/terminal readiness and notification-style navigation.
The source file and SQLite queue are checked independently; no provider prompt
is sent. Earlier run `discovery-runtime-stop-bxnyusm4` passes with complete owned
process cleanup and sampled debug native/gateway peaks of 356,136/45,360 KiB.
These whole-workflow measurements include fixture/capture work and are not
release bounds.

Four new Rust tests cover request/receipt identity on rejection and refusal of
all retired API routes without making a TCP connection. The complete native suite passes 263 tests (two ignored), Rust 1.92 and
release-profile checks cover all native targets, and the ordinary four-launch
real-report regression `run-d_ulkvf2` passes.
Individual provider process recovery remains distinct: `backend-recovery-ja6a85_k`
passes selected/background Codex exits and both-provider transport recovery while
the gateway stays alive, with exactly one explicit fake Send and no replay.

The final offline-edit run `discovery-runtime-stop-htdip_op` also requires that
the post-loss text was actually inserted, Save stays disabled and the source file
stays unchanged. Its inspected screenshot shows both the pre-loss unsaved line
and the new post-loss line. Observed native/gateway peaks were 356,176/45,268 KiB.
Earlier probes checked equality around the stale reply but did not assert that
insertion itself succeeded: the disconnected tracking channel had blocked it.
That evidence was insufficient for post-loss typing. The corrected bounded local
buffer path and explicit content assertion close that gap. Earlier root captures
also caught the output dialog's closing animation; the final capture waits for
normal animation settlement and records export confirmation separately.
The ordinary four-launch regression also passes after this local-buffer change
in `run-6gr8bgzh`, with 263 native tests, Rust 1.92 and release all-target checks.

The first runtime-stop fixture paused before Enqueue, which intentionally
releases an empty queue; its expected wait failed after one private fake prompt.
The corrected fixture pauses synchronously after Enqueue, before persistence can
release dispatch. A later probe wrongly assumed the catalog must still contain
another row during shutdown; a transport retry can already have cleared it.
The final navigation probe preserves the actual nullable selection. These failed
development runs are retained separately from passing evidence. The installed
candidate and release are unchanged; full native migration remains active.

The twelve completed runtime-stop and regression runs are archived with verified
file manifests in
`/media/rui/studio-gnome-deps/qa-artifacts/native-smoke-jwfgtzto/gateway-retirement-validation/VALIDATED.json`.
Workspace artifact paths remain available as links. The archive distinguishes
the failed fixture assumptions and insufficient earlier typing assertions from
the final corrected acceptance evidence.

## Native gateway launch recovery

The native shell now remains alive when its owned gateway cannot launch, exits
before reporting readiness, rejects endpoint validation or times out. Reconnect
starts a new explicit launch attempt without discarding text already typed in the
composer. A coalescing startup-control channel remains available independently of
the ordinary bounded request queue. Attempt generations protect progress,
failure and settings/backend results throughout launch and discovery.

Launch supervision retains the child and parent pipe while waiting for the
private endpoint. Reconnect during that wait closes the pipe and waits for owned
child cleanup before starting a replacement. Accepted window close uses the same
path, including when the ordinary request queue is full. Cleanup allows five
seconds for graceful exit, then reaps the owned child with the existing forced
termination path. Once startup succeeds, normal ordered preference writes still
precede the ordinary Shutdown command. Credential-bearing stdout remains private;
endpoint validation retains the existing loopback-only policy. Launch failures
wait for explicit correction/retry rather than repeatedly spawning processes.

The discovery fixture also supports `--mode launch-retry`, `launch-replace`,
`launch-close` and `launch-timeout`. These use actual owned executable wrappers that exit before the
endpoint or wait on their parent pipe, followed by the real gateway and fake
providers. Final Wayland runs `discovery-launch-retry-g8tkvbna` and
`discovery-launch-replace-czkcn8tl` recover through the existing native Reconnect
action, render seeded existing conversation history, retain the draft and reject
old callbacks. The replacement trace confirms EOF before the next process starts.
`discovery-launch-close-efvv0fku` fills the ordinary request queue, then closes
while readiness is pending; the child receives EOF, no replacement starts and no
HTTP request is issued. All owned processes exit, and no provider prompts are
sent. The failure and recovered-history screenshots were inspected. Debug sampled
native/gateway peaks were 346,052/43,160 KiB for retry and 345,792/43,880 KiB for
replacement; the close case observed 332,324 KiB native and no running gateway.
These are workflow observations, not release performance bounds.

`discovery-launch-timeout-_6zdu78b` also waits for the actual 15-second readiness
deadline, observes graceful EOF cleanup, then recovers through explicit Reconnect
with rendered existing history and its unsent draft. The trace records about
15.1 seconds between owned attempts, with no automatic replacement. Its sampled
native/gateway peaks were 343,528/44,156 KiB.

Three new process tests cover cancellation of an endpoint wait while retaining
ownership, missing executables/early exit/invalid endpoints, and successful pipe
transfer through managed shutdown. Private scripts are invoked through their
interpreter in unit tests to avoid concurrent newly-written-script execution
races. The complete native suite passes 259 tests (two ignored); Rust 1.92 and
release-profile checks cover all native targets. All three registry-discovery
cases also pass after this control-channel change, now requiring rendered seeded
history for recovery. The ordinary four-launch regression `run-5ir2lcmq` passes,
including preferences, large-document editing and restart behavior. Its earlier
`run-enco30m_` counterpart also passes; the final run uses the refined status-only
Reconnect progress feedback. Earlier launch captures had empty fake histories;
the final fixture seeds a completed turn and checks its actual rendered content.

This covers initial launch and discovery. A gateway that exits after successful
startup has a [retirement boundary](#native-running-gateway-loss-boundary) and now
supports [explicit replacement and Markdown snapshot recovery](#native-gateway-replacement-and-snapshot-recovery).
Other viewer restoration remains pending; provider requests are not replayed.
Remaining native parity and physical/provider/
fresh-machine/platform acceptance remain in scope. This increment leaves the
installed candidate and release unchanged.

## Native startup discovery recovery

Backend registry discovery no longer owns the native command loop. Initial
preferences, appearance, language, visibility, notification settings and the
registry load run in one cancellable Rust startup task. Its ordered results carry
an attempt generation, so explicit Reconnect cannot apply a previous attempt's
preferences, backend list or failure notice. Startup still applies preferences
before choosing a backend. Documents and ordinary commands remain usable when
registry discovery fails; failure does not shut down the gateway or document
actor.

Transport failures and HTTP 408/429/5xx registry responses retry only the read-only
GET, after 0.5, 1, 2, 4, 8, 16, 30 and 30 seconds. Credentials/other permanent HTTP
rejections and malformed successful registries stop with an explicit failure.
Reconnect starts a fresh discovery attempt while startup is incomplete; once
discovery succeeds it retains the normal backend-reconnect behavior. A replacement
attempt or accepted window close aborts the old startup task and its backoff.
No prompts, approvals, settings writes or queued turns are replayed. Text typed
before the first connection is preserved through restoration of the configured
session. Retry/failure notices have live English/Chinese bindings; returned error
details remain literal.

`scripts/smoke-gnome-discovery.py NATIVE_DEBUG GATEWAY --mode MODE` uses an
authenticated private proxy, the real gateway, fake providers and owned Wayland.
`automatic` rejects two registry GETs with 503, then verifies selected history and
the unsent draft. `permanent` verifies a single 401 with no automatic retry, opens
a native Markdown document while discovery remains failed, activates the existing
Reconnect action, rejects a stale-generation callback and verifies recovery.
`close` exits during backoff and checks that no GET occurs after accepted close.
All three runs pass with no provider prompts and complete owned-process cleanup:
`discovery-automatic-rfiyba71`, `discovery-permanent-esupeypi` and
`discovery-close-a065xr_w`. The retry and independent-document screenshots were
inspected. Sampled whole-workflow native/gateway peak RSS was 343,240/43,364,
353,280/44,752 and 340,596/38,176 KiB respectively; these are debug fixture
observations, not release performance guarantees.

Four new Rust tests cover the finite budget, actual temporary HTTP/non-JSON error
responses, credential/invalid-success rejection and cancellation without a late
publication. The complete native suite passes 256 tests (two ignored), Rust 1.92
and release-profile checks cover all native targets, and four-launch ordinary
startup/document regression `run-mfqvv8xy` passes. Existing selected/background
Codex/OpenCode initial-handshake recovery also passes in `backend-recovery-b9j4bi1u`,
including paused queues and exactly one explicit fake Send without replay.

Initial process launch now has separate [explicit retry and cleanup](#native-gateway-launch-recovery).
This discovery policy does not recover a gateway process that exits after
startup, interrupted document snapshots, or pending provider requests.
Gateway-wide lifecycle recovery, remaining application parity and
physical/provider/fresh-machine/platform acceptance remain in the full scope.
The installed candidate and release have not been replaced by this increment.

## Native backend transport recovery

Codex WebSocket and OpenCode SSE connections now recover automatically when
their transport task terminates, including attempts that fail before the first
ready notification. Explicit selected/backend-reference connections register a
new budget before dispatch; automatic attempts move that budget to the new
generation instead of resetting it. The connection supervisor
emits a distinct disconnection event; ordinary request/reply queue errors retain
the healthy connection and display their error. The recovery policy keeps no
requests or message bodies. It holds at most eighteen backend records (sixteen
configured plus the two built-ins) and one pending timer per backend.

Retries wait 0.5, 1, 2, 4, 8, 16, 30 and 30 seconds. Eight failures without a
stable connection stop automatic attempts. Thirty seconds of healthy connection
reset the failure budget; duplicate ready notifications do not postpone that
health window. A reconnect attempt advances the connection generation. Timer,
wire and disconnection events must still match their original generation.
Explicit Reconnect cancels the pending timer and starts a fresh budget. Accepted
window close cancels all timers; a close rejected by an unsaved-change guard does
not stop normal recovery. The pending-delay/attempt status follows the interface
language through explicit native bindings.

Selected-session recovery reuses ordinary bounded history hydration and restores
the selected conversation and in-memory composer. Send stays behind history
verification. The pending conversation owns its composer even while the new
catalog has no selected row: typing during that interval survives restoration,
including the current selection. Switching to another conversation still saves
the draft under its original conversation. A lost acknowledgement retains its
original draft; no Send, command, permission reply or queued message is replayed.
Queues pause and need explicit Resume. Background recovery reconnects only that backend and leaves
the visible backend, generation, immutable transcript rows and draft untouched.
Its notification stream resumes, while invalidated cached history still requires
verification when selected. This is not continuous replay of missed history.

An initial backend transport failure shares the eight-retry limit even if no
attempt reaches ready. Same-backend retry preserves typed text also when no
conversation has been selected yet; selecting another conversation explicitly
changes its scope. Retry cannot repair invalid configuration or missing
credentials: correct those settings, restarting Studio when required by the
backend-configuration editor, then use Reconnect for a fresh budget. Gateway
process launch failures use the separate explicit [launch recovery](#native-gateway-launch-recovery).
Registry failures after
gateway launch now use the separate [startup discovery policy](#native-startup-discovery-recovery).
Backend transport recovery does not restart the owned gateway following a gateway-wide
failure. It does not change OpenCode's documented
busy/idle limitation, recover unpersisted output, or add Codex pending-request
replay. Those wider lifecycle/parity gates remain separate.

The private fixture uses the actual authenticated loopback proxy and gateway,
closing real WebSocket/SSE connections while fake providers remain alive. It
checks selected and background recovery for both protocols, a completed fake
turn with its acknowledgement deliberately withheld, authoritative history,
retained draft and paused durable queue, typing and selection during a deliberately
held reconnect catalog, blocked Send in that interval, stale generations, manual
override of a pending timer, healthy-connection isolation from a nonterminal error, live
bilingual wait feedback and closing with a retry pending. Provider logs must show
exactly one explicit fake Send and no replay or OpenCode mutation. Proxy traces
must show no new transport after the close marker, and owned PID cleanup is
verified. Flapping limits, clock-driven healthy reset, duplicate failures,
replacement generations and the eighteen-entry bound have separate pure tests.
The reconnect-wait screenshot was inspected. The optional `--startup-failures`
fixture rejects the first two authenticated WebSocket and SSE handshakes with
HTTP 503, before either backend reaches ready. It checks their accumulated
second-attempt delay, typed text both with and without a pending conversation,
blocked Send, eventual automatic catalog/history loading and all established
recovery cases. The pure policy test also exhausts all eight retries without a
single ready notification and checks explicit reset and stale generations.

```bash
python3 scripts/smoke-gnome-backend-recovery.py /path/to/debug/native /path/to/gateway --startup-failures
```

Established-transport checkpoint: native tests pass (243 passed, two ignored); Rust 1.92 and release
checks pass for all native targets. Final private transport fixture
`backend-recovery-c1a17od8` and translation workflow `translation-bf1mzdgo` pass
on the rebuilt debug binary; both clean up their owned providers. The four-launch Wayland regression
`run-r5jkg1yf` passes with criticals fatal, including copied real report
opening (4,097,226 bytes, 1,055 ms, 64 main-loop ticks), comparison, paged editing,
Git, OpenCode historical context and restart. Its application log retains one
known `GtkGizmo` snapshot-allocation warning, with no GTK critical or panic;
this checkpoint does not claim to fix that separate warning. Builds also retain
existing warnings.

Earlier translation trials failed because the development PDFium path was
unset, then because the legacy synchronous dialog screenshot helper captured
before GTK painted and quit the test. Translation and OpenCode context captures
now await real mapped content frames through the shared capture helper; no
forced allocation or synthetic frame signals are used. Failed trials are kept
alongside successful runs in the `backend-recovery-validation` archive under
`/media/rui/studio-gnome-deps/qa-artifacts/native-smoke-jwfgtzto/`. The installed
optimized release is unchanged. Physical desktop/provider acceptance and broader
native parity remain open.

Initial-connection checkpoint: native tests pass (244 passed, two ignored), with
Rust 1.92 and release checks for all native targets. Private Wayland fixture
`backend-recovery-vumz91mo` passes with two initial HTTP 503 handshakes for each
transport, all established recovery cases and no GTK warnings or criticals.
It observes exactly one explicit fake Send, no replay, unchanged durable queue,
and owned provider/proxy cleanup. The startup-wait screenshot was inspected.
`router-ui-oaj3ld0p` passes reference completion and controller/target scoping,
including the lazy metadata backend connection path. An earlier run,
`router-ui-dqcsjjbx`, exited during the same legacy blank dialog screenshot
problem; that asynchronous target-picker capture now awaits the shared real
content-frame helper, and the Python runner reports a missing result explicitly.
The four-launch regression `run-wyhf5dzz` passes; its log still includes the
separate known `GtkGizmo` allocation warning. These runs do not establish
gateway restart, missed-history replay or physical/real-provider acceptance.
The matched rebuilt optimized candidate passes installed
`release-ui-myyeup_z` with all five optional workflow groups, no application
hooks, no GTK warnings/criticals and provider cleanup. Sampled native/gateway
peak RSS is 638,428/109,168 KiB, with intentional EPUB content-process peak RSS
239,044 KiB; other worker peaks are excluded, so this is not a memory bound.
The package is staged separately and the previous installed package hashes
are unchanged. Completed runs, build/check logs, source hashes and package
metadata are kept in `startup-recovery-validation` under the same external QA
archive. A checkout launch link is documented under
[native release validation](#native-release-validation).

## Native Codex process recovery

Codex can exit while the gateway keeps its WebSocket open. The native transport
now treats the gateway's `studio/appServer/status` `stopped`/`error` notification
as a terminal backend failure. It retires that native connection and enters the
same bounded retry policy. The next connection asks the existing gateway to
start the backend normally; it does not restart the gateway or replay a prompt.
Selected recovery revalidates history and retains the latest composer. Background
recovery leaves the selected backend, transcript, generation and draft intact.
Queues remain paused and expired request dialogs are not automatically accepted.

A small Rust lifecycle tracker distinguishes the gateway's process generation
from the native socket generation. Notices older than the latest observed process
generation are discarded before presentation, including buffered stale `ready`
and `stopped` events. Ordinary RPC responses and other notifications do not
advance that tracker. Legacy status without a process generation still works,
including startup errors emitted before a process exists. Terminal diagnostics
are limited to 4 KiB on a Unicode boundary, with a truncation marker. Native
error toasts now explicitly disable Pango markup as well as retaining the literal
status label, so diagnostic text and paths render unchanged.

```bash
python3 scripts/smoke-gnome-backend-recovery.py /path/to/debug/native /path/to/gateway --process-exit
```

This private fixture makes its owned fake Codex process exit with status 23,
first during a completed turn whose acknowledgement is withheld, then while
OpenCode is selected. It verifies distinct replacement processes, recovered
history, retained newer typing/selection, unchanged background-view ownership,
one explicit fake Send and no replay, paused durable queue and process cleanup.
The proxy does not drop either crashed process's WebSocket; this exercises the
gateway's actual stopped notification. The final nonterminal error also includes
literal `<b>` tags, `&` and Unicode. The fixture excludes the status label when
checking its mapped toast text and captures that real GTK rendering.

Validation: 247 native tests pass (two ignored), along with Rust 1.92 and
release checks for all native targets. `backend-recovery-pmoo1hzr` passes both
actual process-exit cases, literal mapped-toast inspection, unchanged durable
queue and all owned PID cleanup. Its process ledger records two exits and three
distinct starts; the provider wire records exactly one explicit fake Send.
`backend-recovery-48f8uvwm` passes initial HTTP 503 failures and established
WebSocket/SSE recovery. The four-launch regression `run-nmi6tb6x` passes with
criticals fatal, including the current copied real report (4,098,113 bytes,
1,003 ms opening, 62 main-loop ticks), editing, comparison, Git and restart.
These final application logs contain no GTK warning, critical or panic. This
does not claim to fix the intermittent `GtkGizmo` warning recorded in earlier
runs. Gateway-wide recovery, continuous missed-history replay, real-provider
and physical/fresh-machine acceptance remain open.

The rebuilt optimized candidate passes installed `release-ui-mgzqy9tc` with
`--recovery` alone, and `release-ui-9jerfuho` with all five broader workflow
groups plus recovery. Both use the ordinary installed launcher and actual
AT-SPI controls, without application hooks or a gateway override. Their fake
provider actually exits; the checks wait for a replacement PID, restored
conversation and enabled verified Send, retain the exact draft/document, then
verify no model turns and cleanup of every recorded Codex PID. The same
test-only process-exit helper supplies the debug and installed fixtures; its
generated code is byte-identical to the previously verified focused provider.

An earlier installed trial (`release-ui-wrh6k138`) passed recovery but failed
later at Ctrl+O because the fixture's `showing` helper existed only inside its
`--viewers` branch. That helper now has common scope; the recovery-only and
combined cases both pass. The older startup-only candidate and installed package
remain unchanged. `target/gnome-native-candidate-20261102` now points atomically
to this tested process-recovery candidate. Logs, immutable run hashes, source
hashes and candidate metadata are retained in `process-recovery-validation`
under the external QA archive.

## Native missed-event recovery

The native Codex client now treats `studio/appServer/lagged` as a loss of trusted
stream state, matching the WebView workflow's resynchronization requirement.
It retires that native socket into the existing bounded retry policy, invalidates
backend history caches, pauses queues and expires old request dialogs. Selected
history is revalidated before Send becomes available. Background recovery retains
the selected backend, transcript rows, generation and composer; returning to the
affected backend loads authoritative history rather than trusting the old cache.
No pending model turn, shell command, approval reply or queued message is replayed.

The loss count is diagnostic metadata, not a process generation. An unknown,
malformed or zero count still causes revalidation when the gateway explicitly
reports a gap; ordinary RPC responses cannot masquerade as that notification.
The error uses the existing localized missed-event message and the normal literal
diagnostic toast. Continuous event replay and recovery of unresolved Codex request
dialogs remain separate work, as does gateway-wide restart.

```bash
python3 scripts/smoke-gnome-backend-recovery.py /path/to/debug/native /path/to/gateway --event-gap
python3 scripts/smoke-gnome-release.py /path/to/package --event-gap
```

The focused fixture pauses reads only in its owned TCP proxy, then publishes
8,192 fake-provider log notifications with 4 KiB of noise each. This overflows
the real gateway's 2,048-event broadcast channel. The proxy records the gateway's
actual lag marker without logging payloads or credentials; it does not fabricate
the notice or drop that Codex socket. Both selected and background cases recover
new stored history, preserve newer typing/selection and the paused durable queue,
and retain exactly one explicit fake Send. Only one provider initialization occurs:
the socket recovers while the Codex process remains alive.

The installed fixture uses the ordinary packaged gateway and actual accessibility
controls. It briefly pauses only its owned native child with SIGSTOP, publishes a
bounded 16,384-event burst, and always resumes the child in `finally` before any
further accessibility calls. It checks the real visible missed-event notice,
additional authoritative history reads, the new stored history item, restored
verified Send and unchanged draft/document/provider PID. It sends no model prompt.

Validation: 248 native tests pass (two ignored), as do Rust 1.92 and optimized
checks for all native targets. Real-overrun run `backend-recovery-08kedby0`
observes two gateway lag notices, only one provider initialization and no
replayed turn. Both 8,192-event bursts retain the GLib heartbeat: from starting
the proxy hold through invalidation, measured intervals are 949/970 ms, with
94/96 ten-millisecond ticks and a maximum observed gap of 10 ms. These are
fixture measurements, not a physical rendering or full recovery latency promise;
history verification follows the bounded retry and deliberate catalog hold.
Sampled native/gateway peak RSS is 346,520/55,880 KiB, excluding other processes
and kernel socket buffers. The reconnect/paused-queue screenshot was inspected.

Process-recovery regression `backend-recovery-hor8auve` and the four-launch
real-report regression `run-o2066ev5` pass. The latter opens the current private
4,098,113-byte report in 1,002 ms with 62 main-loop ticks, then exercises editing,
comparison, Git and restart. Earlier overrun trial `backend-recovery-b6r7lcy5`
also passed; its proxy logged expected connection-reset tracebacks when retiring
the old socket. The test proxy now handles those normal peer closes quietly.

The optimized installed pair passes `release-ui-pav59pjb` with event-gap recovery
alone (13,695 missed events) and `release-ui-z7nyubwx` with all five broader groups,
process recovery and event-gap recovery (13,699 missed events). Both use real
accessibility controls without application hooks or a gateway override. As a
negative control, the preceding immutable process-recovery candidate fails at
`native missed-events notice` in `release-ui-8jyfv1g7`, after the same bounded
burst: the old client ignores that notice. This expected failure distinguishes
the new behavior from merely displaying an incoming history item.

The launch link now selects the tested event-gap candidate; previous candidates
and the installed pair are retained unchanged. Run hashes, source/build checks,
negative-control evidence and package metadata are in `event-gap-validation`
under the external QA archive. Broader native/physical/provider/platform gates
remain open.

## Native backend configuration

**Settings → Backends → Edit backend configuration** opens a native
GtkSourceView JSON editor for the gateway's configured `backends.json` path.
The file format and credential guidance remain those in
[local backend instances](backend-instances.md). The editor displays the exact
UTF-8 source, including an invalid file that needs repair. It supports syntax
highlighting, live theme changes, ordinary undo/redo, explicit Validate, Save and
Reload. Unsaved text stays in memory; closing or reloading asks before discarding
it, and pending operations guard both editor and parent Settings closure.

Validate and Save use the same Rust parser as startup: a 256 KiB file budget,
versioned schema, at most 16 entries, reserved builtin IDs and validated launcher
arguments. Reading or validating a missing file does not create it. Save writes
the exact source bytes through a staged, flushed atomic replacement with private
file permissions; it does not reformat JSON or modify `settings.json`.
Existing file symlinks are preserved. A revision includes resolved path, existence
and source bytes. The gateway checks it before staging and immediately before
replacement, then confirms the resulting path and source. Changes detected
outside the editor reject the save and retain its text. This is optimistic
conflict detection; it does not lock out unrelated external writers.

The separate **Active backend registry** reports what this process loaded at
startup. Saving a configuration does not reload that registry, reconnect sessions
or launch providers. Restart Studio to apply the file; an enabled configured
provider starts when its backend is selected. Disabled entries remain excluded.

The authenticated, origin-protected `/studio/native/backend-config` endpoint
accepts GET (read), POST (validate) and PUT (revision-checked save). It only edits
the configured path, rejects unknown request fields, and has a 2 MiB transport
envelope for escaped JSON containing at most 256 KiB of source. File reads,
validation, hashing, locking and writes run on bounded Rust worker lanes. Schema
diagnostics are bounded before reaching GTK; paths and provider names remain
literal text. Editor generations reject obsolete replies, and a save
acknowledgement clears the modified flag only for the submitted edit generation.

`scripts/smoke-gnome-backends.py NATIVE GATEWAY --api-only` checks authorization,
origin rejection, file/type/size budgets, invalid-file repair, exact Unicode/CRLF
saves, stale conflicts, unrelated preference preservation and absence of runtime
reload or provider starts. The two-launch Wayland case, without `--api-only`,
additionally exercises actual native editing, discard cancellation, dirty/busy
close guards, undo/redo across Save, dark JSON highlighting, restart activation,
literal provider names and lazy process startup. It sends no model prompts and
checks owned-process cleanup. Pure transaction tests also cover symlink retargets
and an external edit between save preparation and commit, preserving the external
file and removing the staged temporary file.

The hardened Wayland run (`backend-config-adv667q3`) passed both launches. Its
edit/restart workloads sampled native peak RSS of 400,220/294,476 KiB and gateway
peak RSS of 43,576/44,012 KiB, excluding worker peaks. The edit heartbeat's maximum
gap was 190 ms, including screenshot capture. These are observations from the
private fixture, not global memory or latency bounds. Physical keyboard/IME and
assistive-technology acceptance remain separate gates.

The matched optimized pair also passes
`scripts/smoke-gnome-release.py PACKAGE --all-diagrams --language --backends`.
In `release-ui-zup4kh2w`, ordinary AT-SPI actions and fixture-owned Wayland keys
open Settings, select Backends, activate the editor, edit JSON, validate it and
explicitly save the exact Unicode/CRLF file. No application test hooks are
enabled. The same installed run retains its composer/document, completes the
language/question, Markdown edit, four-diagram 400% zoom, image-save portal and
desktop-handoff checks, observes no WebKit content process, and reaps its owned
providers. Rust 1.92 compatibility and both optimized builds pass. Existing
libadwaita MessageDialog deprecation warnings remain; these checks do not replace
fresh-machine, physical desktop, real-provider or other-platform acceptance.

## Native interface language

Settings → Appearance → Interface language offers system, 简体中文 and English,
with explicit Apply language and Reload controls. Confirmed changes update
registered GTK properties and the application menu without restarting. System
selection follows GLib's primary preferred language: Chinese resolves to the
shared simplified Chinese catalog, with English as the fallback. This changes
Studio's controls; GTK and desktop portal strings retain their system locale.

The authenticated `/studio/native/language` GET/PATCH API shares the ordinary
`settings.json` language field, not `gnome.json`. A worker transaction validates
the three supported values, rereads the current document and changes only that
field. Its revision covers the language projection: unrelated concurrent edits
are preserved, while another client's language change rejects a stale save.
Rejected choices stay in the form until Reload or another explicit choice.
The existing authentication/origin policy, an 8 KiB request limit and atomic
preference persistence apply. Closing protects unapplied choices and admitted
saves; gateway failure clears pending state without discarding the choice.

The embedded catalog reuses `ui/i18n.mjs` and checked-in native additions.
`node scripts/vendor-gnome-i18n.mjs --check` verifies the generated snapshot in
development/CI; no Node process or translation service runs in the application.
Catalog parsing is warmed on a Rust worker. Weak, explicitly registered UI
properties update on the GTK thread; no content-tree scan translates documents,
conversation text, paths, secrets or editable values. String-property bindings
detect superseding model assignments; an explicit literal assignment also clears
a binding when the model text happens to equal its old UI label. Choice factories
translate visible UI labels while
retaining their underlying keys and selection. Context distinguishes Markdown
Source (源码) from a reference's Source (来源).

Coverage includes core shell/menu/status actions, reader modes, settings pages,
appearance/font/layout/diagram controls, browser permissions, notification
preferences, shared directories and session visibility. Agent-input headings,
actions/placeholders, approval actions, policy-picker labels, permission counters
and queued-message controls now use the same bindings. Formatted interface text
keeps literal substitution values; changing language updates existing counters
and dialog response labels without changing response IDs, availability, close
decisions or the prepared provider payload. Grant/policy accessibility names and
descriptions follow their visible labels through GTK relations. Exact requested
paths, policy scopes, agent-authored questions/options, typed answers and queue
text/references remain literal. Remaining workspace
panels, some dynamic explanations/errors and confirmation dialogs still need
translation; this increment does not claim complete Chinese/English parity.

The private API fixture covers authentication, foreign origins, malformed and
oversized requests, concurrent edits, conflicts and unknown-field preservation.
The Wayland fixture changes existing and newly created settings widgets, restores
English through Reload and restores Chinese after restart. Chat drafts,
unsaved editor text, selection and unmarked content remain literal; there is no
implicit file save. A subsequent explicit save retains the document's CRLF
format. It uses private fake providers and checks that no model turn starts.
The Chinese appearance and settings screenshots were inspected. Contextual
choice labels distinguish the Reading layout from the generic reading activity,
and font weights retain their original values. The normal 170-test native suite,
206-test gateway suite, Rust 1.92 check, optimized native build and four-launch
large-document regression pass at this checkpoint. Both executables are current
in the staged release package, whose installer also copies the catalog's project
license and provenance. The installed `--all-diagrams --language` workflow passes
without compiled-in application test hooks; remote CI and physical desktop
acceptance have not been run for this increment.

### Native Files language

Files controls, visibility options, filter guidance, folder paging, fixed scan/
cache notices, live/count/timing statuses, read-only badges and context actions
now follow interface-language changes. The common native workspace button helper
registers explicit tooltip bindings, including Source paging/close actions. The
Source heading uses the Markdown code context. Folder chooser request titles use
the current language; portal/system contents retain the desktop locale.

Only synthetic Project/shared-root prefixes are translated. File/folder names,
paths, cursor tokens, menu action targets and filter text remain literal. A
shared name containing braces or UI words is a substitution value, not a new
translation key. Workspace titles explicitly retire interface bindings before
showing model names. Fixed tree notices carry a static message key separately
from optional literal diagnostic text. Errors keep their exact text while the
Retry affordance or file-manager failure prefix translates.

The tree uses one shared row presenter for GTK bindings and the focused fixture.
Rebinding retires both label and tooltip ownership, including when a new filename
exactly equals the previous translated notice. The fixture explicitly reuses
those row widgets through that presenter; it does not assume that GTK always
reuses a particular ListItem. It also changes a real open Files context menu
through Chinese, English and Chinese, checking labels and unchanged action
target variants. Navigation generation, project, filter, expanded folders and
visibility choices remain unchanged. The Chinese row screenshot was inspected.

Private Files checks cover real live create/rename/delete notifications, hidden/
ignore controls, directory paging and trailing-space filtering alongside this
language coverage. The shared-folder run also verifies real keyboard/pointer
Copy, an independent Wayland clipboard read, read-only launcher configuration
and native reader activation. It does not launch the external file manager.
The focused runs are `files-ui-tz2cbfpk` and `files-ui-m5727hx5`; language
persistence is checked by `language-n2tsuonh`. Full four-launch real-report run
`run-zp3e28ul` also passes with GTK criticals fatal, including the new Files gates;
its application and restart logs contain no GTK criticals or warnings. QA logs,
results and file hashes are retained under
`/media/rui/studio-gnome-deps/qa-artifacts/native-smoke-jwfgtzto/files-language-validation/`.
The existing two-launch language Apply/Reload/conflict/save/restart fixture passes
with unsaved editor text and queue/Favorites/Comments drafts preserved and no
model turns. Native tests (241 passed, two ignored), Rust 1.92 and release-target
checks pass. The installed optimized package has not been rebuilt. Other native
workspace dialogs, raw diagnostics and physical/provider acceptance remain in
scope; this is not a claim of complete language or application parity.

The expanded input fixture passes 29 provider replies in each of English and
Chinese, with the same typed answers, permission subsets and policy scopes.
It changes the saved language while an input form, selected policy and selected
permission subset are open, and checks selection/draft retention before any
reply. The queued-message fixture edits a real saved queue entry across language
changes, saves explicitly, restarts and checks the SQLite contents including
reference extension fields. Text equal to UI keys (such as `Pause` and `Send`)
stays literal. The Chinese dialog screenshots were inspected. These checks use
private fake providers; they do not run commands or model turns. Existing
OpenCode input recovery and the full real-report Wayland regression pass too.
Worker-prepared summaries, some validation errors and remaining workspace
dialogs are still outside the translated coverage.
The refreshed optimized pair passes the expanded installed workflow too, including
two real provider questions answered through native editable text and radio
controls in Chinese/English without application test hooks. Rust 1.92 checking,
the 170-test native suite and the larger Wayland regression pass; the real
4,097,226-byte report opened in 1,060 ms with 64 opening main-loop ticks in that
fixture. These are local workload observations, not a physical keyboard/IME or
real-provider sign-off. The migration remains in progress.

```bash
node scripts/vendor-gnome-i18n.mjs --check
python3 scripts/smoke-gnome-language.py "$NATIVE" "$GATEWAY" --api-only
python3 scripts/smoke-gnome-language.py "$NATIVE" "$GATEWAY"
python3 scripts/smoke-gnome-input.py "$NATIVE" "$GATEWAY" --headless
python3 scripts/smoke-gnome-input.py "$NATIVE" "$GATEWAY" --headless --language zh-CN
```

### Native Favorites language

Favorites now binds its headings, catalog/search accessibility names, filtered
counts, metadata placeholders, Preview/Source tabs, actions, notifications and
save/discard/remove/source confirmations to the interface language. Response IDs,
view keys and enabled-state guards remain stable. Titles, notes, tags, questions,
saved responses and source identities stay literal, including text equal to UI
keys or containing markup-like text and placeholder braces. Copy/export retain
the existing stored-content format; export reads the persisted library while
metadata edits remain unsaved. Catalog rows have explicit literal accessible
names and descriptions.

Shared window-owned export failures translate their headings, template labels
and acknowledgment buttons. Original source identifiers, redacted diagnostics
and destination paths remain literal substitution values. Details and notices
use plain text. Raw request errors clear any older translated summary binding;
only explicitly identified interface notices enter the translation catalog.

`scripts/smoke-gnome-release-favorites.py PACKAGE` exercises actual accessibility
and owned Wayland input without application hooks or a gateway override. Debug
pair run `release-favorites-44fm9v5u` passes English/Chinese save/cancel, filtering,
Source/Preview, dirty source guards, native export cancellation/failure/success,
byte-preserving source identity, independent clipboard reads and cancellation of
removal. The failure destination contains `<b>`, `&` and `{source}`. Settings
changes occur between library visits, respecting GNOME modal behavior. Peak
native/gateway RSS was 359,640/22,784 KiB; this is a small functional workload.

The existing live-language fixture also retains an unsaved Favorite and its
open confirmation while applying/reloading language through the real settings
controller. It verifies stable response IDs, literal metadata and source bytes,
then explicitly saves and checks SQLite after restart. Final run
`language-_cr_h5ac` passes with zero model turns; its prompt/editor screenshots
were inspected after normal dialog transitions. Shared write-failure regression
`write-failures-gz_7edkr` also passes six chooser calls, five real filesystem
failures, queued/closed/replaced-viewer errors and admitted local writes during
gateway loss. Native tests pass 230 cases (two normally ignored),
Rust 1.92 checking passes, and four-launch regression `run-2a0zgzux` opens the real
4,097,226-byte report in 1,074 ms with 65 opening GTK ticks. Other workspace
translation, physical desktop/provider and platform acceptance remain open.

The refreshed optimized pair passes `release-favorites-rt2ax7gs` through the same
actual controls and private export portal, with zero model turns and sampled
native/gateway peaks of 337,156/27,708 KiB. Installed `release-ui-5f9cmyqi` passes
all five diagram/language/backend/Git/viewer groups; `release-map-1so8qsv1` also
retains named menu actions, keyboard/dialog focus and persisted Map Undo. These
checks use sibling executable discovery without a gateway override or application
hooks. They do not replace physical GNOME/IME/accessibility/provider acceptance.

### Native Comments language

The Comments library now translates its controls, editor accessibility names,
status/cleanup messages and reload/close confirmations. The field caption uses
a noun-specific key, independent of the shared “Comment” action. Translations
retain response IDs, enabled/default responses and submission tickets. Notes,
instructions, excerpts, session titles, source anchors and diagnostics stay
literal. Empty-state instructions retire their binding when replaced by an
excerpt, even if that excerpt equals the old interface text.

Source locations use reusable `i18n_ui::Part` bindings: explicit interface
fragments can translate while literal identifiers and substitution values remain
unchanged. Optional document/chat/table/PDF/EPUB fields need no combinatorial
catalog entries. Unknown/legacy descriptions remain literal. This changes only
presentation; shared source records and generated submission prompts keep their
existing schema and format. Widget accessible names use GtkAccessible, with an
explicit literal assignment that removes an older interface binding.

Live-controller run `language-p6d6cdw9` passes English/Chinese Apply/Reload with
an open dirty Comments prompt, six source-description variants, literal/UI-key
collisions, opaque anchor fields and explicit save/restart. The mapped Chinese
prompt screenshot was inspected. Native controls pass two-launch editing,
conflict/cancel/reload/source checks in both languages (`annotations-ui-me72t33j`
and `annotations-ui-frdoyqwc`). All fourteen submission cases pass: each locale
tests private fake Codex/OpenCode/Router acceptance, rejection, cleanup-only retry,
conflict and accepted-close recovery. Provider-wire checks retain exactly the
expected source submissions; these are not real-provider tests.

`scripts/smoke-gnome-release-comments.py PACKAGE` exercises actual accessibility
controls without application hooks or a gateway override. Debug pair run
`release-comments-ensdb87z` passes both locales, literal editor/anchor data,
dirty source/reload/close guards, explicit save, prompt insertion without sending,
native source opening, explicit removal and composer retention. Both phases
start zero model turns. Native tests pass 230 cases (two normally ignored),
Rust 1.92 checking passes, and four-launch `run-fcx3zyhr` opens the real
4,097,226-byte report in 935 ms with 56 opening GTK ticks. Remaining workspace
language, worker diagnostics, cross-widget selection and physical/platform/
provider acceptance stay within the full migration scope.

The refreshed optimized pair passes `release-comments-8obfi1cj` with those actual
controls in both locales and zero model turns. Separate English/Chinese phases
sampled native/gateway peaks of 317,260/28,860 and 309,156/29,012 KiB respectively.
It also passes `release-favorites-_p6h7k03`, all five workflow groups in
`release-ui-zjewflp6`, and named menu/Map Undo checks in `release-map-6n1d1mmu`.
These checks use the installed sibling gateway without an override or compiled
application hooks. They do not replace physical GNOME/provider/platform gates.

```bash
python3 scripts/smoke-gnome-annotations.py NATIVE_DEBUG GATEWAY --headless --language zh-CN
python3 scripts/smoke-gnome-annotations.py NATIVE_DEBUG GATEWAY --headless --language zh-CN --submit cleanup
python3 scripts/smoke-gnome-release-comments.py PACKAGE
```

### Native text-flow experiment

The application's default Preview still uses its existing virtualized block widgets. Standalone
`native/gnome/examples/text_flow_selection.rs` evaluates a native GTK TextView
with [child anchors](https://docs.gtk.org/gtk4/class.TextChildAnchor.html) for
tables, formulas and diagrams. This is an architectural experiment, not a new
application preference or a completed selection port. The internal development
switch described below also exercises this adapter through normal document requests.

`studio_markdown_native::flow` prepares a bounded presentation-neutral projection
on Rust workers. Unicode scalar positions identify synthetic objects separately
from literal U+FFFC characters. Objects retain their original TeX, diagram source
or full table text in the Rust model. Copy expands selected objects atomically,
preserves whitespace and containing original byte/line ranges, and rejects an
oversized result explicitly. GTK receives compact anchors instead of large hidden
alternative strings. Existing parsing stays unchanged unless the projection
explicitly requests short-text attributes. Shared rich table cells retain full
content and source identity; their existing bounded excerpts serve labels.

The projection allows 512 KiB of text-buffer content, 256 objects and 2,048 source
segments. The experimental GTK adapter further limits admission to 32 preparation
jobs (distinct math source/style pairs, local tables and eager diagrams), 64 columns for
standalone unindexed tables and 32 MiB of raster data. Images and lazy diagrams
use viewport admission instead of the eager preparation slots; they still share
the projection's 256-object limit with code headers and all other anchors.
Snapshot-owned indexed tables use the normal reader's column controls instead;
the current indexer separately limits source tables to 256 columns. All engine work stays on a
Rust worker. Tables mount at most 16 rows / 512 cells per page; changing pages
retains the immutable object text and outer scroll position. Selecting an object
copies its entire source fragment, including table rows outside the visible
table page. This copy policy still needs integration with production table
column/page selection semantics.

#### Selectable Preview through normal document requests

`STUDIO_GNOME_CONTINUOUS_PREVIEW=1` enables the experimental adapter in the native
reader's ordinary Open, outline navigation and Preview/Source/Edit workflow.
It is a development switch, not default promotion or a second user-facing mode.
The document actor requests the existing bounded context pages, verifies each
fragment's document and revision, then projects text and prepares syntax colors
on the bounded Rust compute pool. GTK-independent preparation lives in
`flow_prepare.rs`; the portable document model does not depend on GTK.

The reader owns the full mounted view, so syntax theme handling and projected
selection survive normal requests. Replacement advances the mode epoch, cancels
preparation, detaches expired table anchors and rejects old comment callbacks,
including replacement within the same document revision. The insertion mark maps
back to its containing source segment for view switching. This is not a complete
character-level Markdown source map or arbitrary cross-page selection.

The real workflow fixture is `scripts/smoke-gnome-flow-preview.py NATIVE GATEWAY
--reference FILE`. It copies the reference and uses fake providers on an owned
Wayland compositor, with GTK criticals fatal. It exercises cross-block Unicode
Copy, a heading jump, stale comment ownership, dirty Edit retention and Undo,
256-column indexed-table filtering, and the real report's bounded table context.
The preparation-only test also accepts a 2,000-column indexed fragment; this
does **not** imply the current server indexer accepts that file.

Private run `flow-pipeline-cl3dbnd3` passes with the actual keyboard-operated
column picker, unchanged source hashes and zero model prompts. The 4,098,113-byte
report contains 1,153 index pages; opening the bounded initial context took
1,052 ms, with 102 ten-millisecond GTK timer callbacks and a longest sampled gap
of 21 ms. This interval includes index preparation and a normal GTK frame, but
excludes later table navigation and screenshots. Native/gateway sampled peaks
were 399,096/47,976 KiB over the whole fixture, excluding other processes. This is
one debug run, not a controlled speedup comparison or a whole-file layout claim.
Small, wide-table and settled report screenshots were inspected.

The default-renderer four-launch regression `run-z82f4p9s` also passes, including
real diagrams, complete indexed-table Copy/primary selection and saved comments.
Its comment fixture now explicitly rejects remapping an expired same-revision
page, then constructs a fresh view for destination-switch and toolbar checks.
Native tests pass 249 cases (two ignored); all native targets pass Rust 1.92 and
release-mode checking. Formatting, generated locales, Python syntax and diff
checks pass. Successful runs, failed controls, the GTK debugger trace and source
hashes are retained in
`/media/rui/studio-gnome-deps/qa-artifacts/native-smoke-jwfgtzto/flow-pipeline-validation/`.

Rapid mode switching exposed a GTK 4.22.4 scroll-animation teardown assertion.
The debugger trace reaches `GtkScrolledWindow` unmap and
[`gtk_adjustment_enable_animation`](https://github.com/GNOME/gtk/blob/4.22.4/gtk/gtkadjustment.c#L897),
whose target-value notification can reenter adjustment updates before handler
disconnection. Before hiding a reader mode, the native owner ends its pending
scroll via the public nonanimated `set_value(current_value)` path. Desktop
animations and normal scrolling stay enabled. This is a targeted workaround,
not a claim that all upstream GTK lifecycle issues are fixed.

Remaining promotion gates include dense mixed-content layout,
broader input/accessibility and cold-layout measurements,
and selection spanning different bounded contexts. The installed candidate is
unchanged by this experiment.

#### Dense deferred content in selectable Preview

Preparation admission now distinguishes eager work from deferred metadata. The
32 eager-work slots still bound distinct math source/style pairs, local tables and eager diagrams. Images
and viewport-rendered diagrams consume the existing 256 total object slots,
without decoding images or rendering diagrams during document preparation. A
worker test admits 128 image/diagram pairs and rejects a 257th object; separate
checks retain rejection of 33 distinct math jobs, tables or eager diagrams. Repeated
math shares a job, while every occurrence still counts toward object and raster
limits. The text, source
segment, context and raster budgets remain bounded.

`native/gnome/examples/flow_admission.rs` measures preparation without initializing
GTK or opening a profile/provider. It exercises 8/32/33/64/128 objects of each
kind and optionally the complete lines within a real file's first 64 KiB. That
prefix is a preparation sample, not a whole-file loading benchmark. Before the
change, 33 deferred objects were rejected by the blanket eager limit. Afterward,
64 and 128 image/diagram metadata objects prepare with zero raster bytes; math
and local-table limits remain in force. Debug timings are retained with the
validation artifacts rather than presented as controlled parser speedups.

The real document-actor fixture now opens separate 64-image and 64-Mermaid files,
using the normal image decoder and diagram engine. Only viewport content is
admitted; no more than two diagram jobs run at once. Actual Wayland Ctrl+End,
Page Up, Ctrl+Home and repeated Ctrl+End verify the last object fully fits in the
viewport, user navigation supersedes endpoint correction, old pixels are released
and Source cancels pending work. Image anchors share the allocated text width,
preventing shrink-to-minimum thumbnails.

This uncovered two native layout issues. Initial source-anchor scrolling now
waits for a real mapped frame. Endpoint navigation also follows replacement of
the TextView's scroll adjustment and subsequent child geometry changes, retaining
GTK's normal caret/selection handling. Corrections use one-shot frame callbacks;
keyboard, pointer and scroll input cancel the endpoint hold, and unmapping
disconnects it. The anchor subclass retains parent measurement side effects and
reserves GTK's cursor pixel when assigning child widths. See GTK 4.22.4's
[TextView implementation](https://github.com/GNOME/gtk/blob/4.22.4/gtk/gtktextview.c).
Broader accessibility/assistive scrolling remains an acceptance gate.

Final debug run `flow-pipeline-kn550dbq`, with temporary trace logging removed,
passes these checks
with 252 native tests (two ignored). The 64-image and 64-diagram cases first
displayed content in 298/415 ms, with four/one initially loaded textures; first
Ctrl+End completion took 78/225 ms. Both workflows observed a maximum 56 ms GTK
heartbeat gap, including screenshots. The copied 4,098,113-byte report opened in
1,064 ms with 101 opening ticks and a maximum 24 ms gap. Whole-workflow sampled
native/gateway peaks were 508,672/67,612 KiB. These debug observations include
fixture/capture costs and concurrent regression work; they are not release or
cold-cache performance guarantees. Source hashes stayed unchanged, no provider
prompts were sent, and owned processes exited. Screenshots show the full final
diagram and readable image widths. Earlier failed visibility/layout probes are
retained alongside the passing runs.

The default-renderer four-launch regression `run-b6vvmzic` also passes after these
changes. Rust 1.92 and release-profile checks cover all native targets, including
the preparation example. The benchmark's final debug run prepared 64/128 image
metadata objects in 548/1,079 µs and diagram metadata in 199/395 µs, all with zero
prepared raster bytes. These measurements exclude actual rendering and GTK.

This increment does not promote selectable Preview to the default or rebuild the
installed candidate. Dense mixed math/table throughput,
selection across separate contexts, input/accessibility and physical/provider/
platform acceptance remain in the full migration scope.

#### Native code controls in selectable Preview

The Rust projection now emits a typed, textless code-header anchor before each
ordinary code body. GTK mounts the same language/Copy header used by the normal
conversation and document code viewer. Language names stay literal and ellipsize
within the available width; unnamed fences use the localized Code label. Syntax
preparation remains on Rust workers, and the code body remains in the shared
selectable text buffer. Headers neither run code nor invoke a syntax engine.

Copy uses the immutable parsed body, preserving its whitespace and Unicode.
Cross-block selection expands a header to an empty string, so language labels,
icons and placeholders never enter the clipboard. Literal U+FFFC in code remains
literal. Textless headers are excluded from selected source ranges, including
trailing empty fences; saved quote relocation therefore still agrees with the
actual copied text. Full source segments remain intact for layout and scanning.

The code action checks the mounted view, original TextBuffer identity and content,
and the reader's document/revision/page epoch. Replaced or unmapped controls cannot
overwrite the clipboard. Saved Preview can still copy while an Edit page is dirty;
this uses snapshot ownership rather than comment-submission eligibility. No code
is reparsed when Copy is pressed.

Controls share the projection's 256-object limit but do not consume the separate
32 eager-content preparation slots. A 64-fence preparation check verifies
zero raster work and all syntax blocks retained. Dense controls still need layout
measurements before promotion; this change does not lift document/context limits.

The private Wayland fixture now drives Copy using actual native keyboard input,
reads the clipboard from the owned compositor, checks retained selection, stale
page and changed-buffer rejection, dirty Edit/Preview ownership, empty-code Copy
and header/body geometry. It retains separate code and empty-fence screenshots.
The normal/default-reader application regression also exercises the shared header.

Final private run `flow-pipeline-ovw0i9f5` passes all of those checks with unchanged
source files and zero provider prompts. Its 4,098,113-byte real report opens in
1,042 ms with 101 ten-millisecond GTK callbacks and a longest sampled gap of
21 ms. Whole-fixture native/gateway sampled peaks are 397,132/48,480 KiB,
excluding other processes. These debug observations are not controlled speedup
measurements. Code and empty-fence screenshots were inspected; the final fixture
also verifies that the header does not overlap the first code glyph.
The earlier `flow-pipeline-g47fk9s1` code screenshot had scrolled past the header;
the final fixture explicitly scrolls to it and requires visible viewport bounds
and a normal GTK frame before capture. Both runs are retained for review.

The four-launch default-renderer regression `run-48npzx65` passes with GTK
criticals fatal. Native tests pass 250 cases (two ignored); shared Markdown tests
pass 96 without syntax highlighting and 100 with it. Rust 1.92 and release checks
cover all native targets, including debug-only modules, and formatting/locales/
Python syntax/diff checks pass. A debug-only import problem exposed by the prior
cleanup is corrected with explicit imports in both affected acceptance modules.
Runs, logs and source hashes are retained in
`/media/rui/studio-gnome-deps/qa-artifacts/native-smoke-jwfgtzto/code-controls-validation/`.
The installed candidate and existing installed release remain unchanged.

#### Native code-body surfaces in selectable Preview

The worker now prepares both themes' base code colors even when highlighting
falls back because of an unknown language or a byte/line/run limit. Source text,
selection mappings and the existing syntax budgets are unchanged. The fallback is
initialized only after validating a code span and checking cancellation. GTK
reuses the normal code-font and Markdown line-height preference watchers for code
spans.

GTK paragraph-background tags proved insufficient in actual rendering: empty
lines and the first code line sharing a paragraph with a header anchor remained
unfilled. The anchored TextView now exposes GTK's supported
[below-text snapshot hook](https://docs.gtk.org/gtk4/vfunc.TextView.snapshot_layer.html). The syntax presentation paints the visible code extent in buffer coordinates,
then GTK paints text, native selection and the caret above it. The callback uses
weak ownership and rejects changed/replaced buffers; it never parses source or
runs a syntax engine. Code headers, body text, Copy and source anchors retain their
existing ownership. Repeated theme changes still use at most the worker's bounded
128-paint palette; no extra paragraph-tag palette is retained. Rendering bounds
remain the existing context and object limits.

Private Wayland run `flow-pipeline-xtda31jy` validates 18 actual background pixel
samples: first and empty lines in highlighted Rust, unknown-language plain code,
and a long-line fallback, across light/dark/light themes. Two neighboring prose
samples retain the ordinary view background. Each code body copies exactly,
including Unicode and the complete overlong line; theme changes preserve source,
selection and dirty state. Screenshots of both themes were inspected. The same
normal document-actor run also passes code controls, source/edit view switching,
256-column selection, dense 64-image/diagram viewport admission and large-report
paging. Its copied 8,340,599-byte report opens in 1,324 ms, with 129 opening timer
callbacks and a maximum 23 ms gap. Whole-fixture sampled native/gateway peaks are
505,436/60,176 KiB. These debug observations include automation costs and are not
a controlled speedup measurement or a release-memory limit.

The full selectable-text example `text-flow-wcmmii2y` passes 11 cases, including
TypeScript/TSX token colors, themed plain fallbacks, changed/replaced source
rejection, exact clipboard text, table interactions and real document content.
Default-renderer four-launch run `run-0qksqqz9` also passes. Native unit tests pass
274 cases (two ignored); shared Markdown tests pass 96 cases without syntax
highlighting and 100 with it. Rust 1.92 and release-profile checks cover all
native targets.
The preceding stronger run `flow-pipeline-llqpzn9j` also passes.
The earlier `flow-pipeline-pcp26zr5` pass checks empty-line pixels but lacks the
final first-line/prose pixel assertions. The two initial mapping/scroll probe
failures and two paragraph-background paint failures are retained with the passes
for review; none is counted as final acceptance.

This increment leaves selectable Preview experimental. Broader
mixed-content layout, selection across separate contexts, input/accessibility and
physical/provider/platform acceptance still precede promotion and completion of
the full native migration. The installed candidate and installed release are
unchanged; this increment uses debug acceptance and compiler checks. Evidence
is retained in
`/media/rui/studio-gnome-deps/qa-artifacts/native-smoke-jwfgtzto/code-surfaces-validation/`.

#### Native code gutters in selectable Preview

The shared Rust syntax preparation now supplies logical code-line starts in
projected Unicode character coordinates, including blank lines and plain syntax
fallbacks. Trailing newlines do not add a phantom line. These offsets are part of
the syntax page's cache weight. Markdown parsing already normalizes code line
endings; the new metadata follows that existing projection and does not change
Copy or original source mappings.

GTK renders decorative line numbers through its native
[TextView gutter API](https://docs.gtk.org/gtk4/method.TextView.set_gutter.html).
It uses the current character rectangles and buffer-to-gutter coordinates,
skips offscreen source lines and numbers a wrapped logical line only once.
The gutter never inserts text, shifts selection offsets or runs a parser. GTK
owns glyph measurement and painting; theme, scroll adjustment and text-tag size
changes invalidate the gutter. A replaced buffer rejects the old page. The
widget cannot take pointer/keyboard focus and has the Presentation accessibility
role, leaving accessible source text and header controls unchanged.

On this GTK 4.22 host, the gutter getter returns a border container around the
supplied widget. The adapter accepts either that representation or the widget
directly, without depending on the private container's type name. The initial
acceptance probe assumed a direct widget and failed; the retained diagnostic
identified the actual child before the getter and invalidation paths were fixed.

The real document-actor Wayland case `flow-pipeline-5wqoa265` passes 18 gutter-ink
samples across highlighted Rust, unknown-language and long-line fallbacks in
light/dark/light. First and blank lines are numbered; scrolling into an overlong
wrapped continuation does not repeat its number in any of the three themes.
Replacing the buffer removes the old numbers. Actual code-background/prose
pixels, complete Unicode code Copy, selection, dirty state, native header actions,
source comments, tables, dense diagram/image admission and view transitions still
pass. Light/dark screenshots were inspected. The copied 8,340,599-byte report
opens in 1,195 ms with 115 opening timer callbacks and a 24 ms maximum gap;
sampled native/gateway peaks are 503,764/64,932 KiB. These debug observations are
not controlled speedup measurements or release memory bounds.

This closes the line-number gap for the experimental selectable adapter. The
default block renderer and installed packages are unchanged; mixed-content,
cross-context selection and broader input/accessibility acceptance still precede
promotion. Shared parser tests cover Unicode, CRLF input, blank lines, empty
fences, final-newline handling and cache accounting.

The shared selectable-text regression `text-flow-vmwaeszb` passes its 11 cases,
including native input/Copy, syntax fallbacks, tables, picker recycling and image
ownership. Shared parser suites pass 96 default and 101 syntax-enabled tests;
274 native tests pass (two ignored), with Rust 1.92 and release all-target checks.
The evidence group includes the initial getter failure, its diagnostic, an
intermediate pass and final expanded checks, with source/binary hashes:
`/media/rui/studio-gnome-deps/qa-artifacts/native-smoke-jwfgtzto/code-gutters-validation/VALIDATED.json`.

#### Repeated formulas in mixed selectable content

The real reader fixture now includes 16 inline formula occurrences and eight
indexed 60-row tables, with formulas and Unicode in table cells. Table views keep
their normal 50-row pages; full projected Copy resolves all rows, including those
outside the visible table page. Native End navigation, Source retirement, source
bytes and read-only state are checked alongside opening timer gaps.

Preparation previously ran the isolated Rust math renderer for every repeated
formula. A context-local map now shares the immutable image's existing Arc pixels
for equal source and display style. One preparation has one theme, and nothing
is reused across contexts/themes. Keys borrow the immutable source; the map
expires when preparation returns. External diagram engines keep their existing
render path. The original experiment counted every occurrence toward 32 eager
objects. The work-based admission update below counts distinct renders instead;
every occurrence still counts toward the 256-object and 32-MiB raster limits.
Cancellation is checked before reuse and
before returning the completed context. Focused tests distinguish different
sources/styles, verify shared pixels and retire a partially prepared context on
cancellation.

Debug baseline `flow-pipeline-a_ilxosf` performed 16 inline renders in 567,329 µs;
the same mixed source in `flow-pipeline-2tjmtvd3` performs one render and reuses
15 results in 96,139 µs. Time until table pages were available changes from
1,109 to 645 ms. These are one before/after host observations, not a statistical
benchmark or a general latency guarantee. Both retain exact formula Copy, all
eight tables' complete source text, native navigation and view cleanup. Inspected
first captures show the real fraction images and table-cell math. Both also run the
copied 8,340,599-byte report and the existing code/table/image/diagram workflows.

Stronger paint acceptance exposed a separate shared-label issue: mapped
offscreen table labels could enter the bounded inline-math queue before visible
labels, exhausting its admission limit. The final-table check in
`flow-pipeline-0ciuiiqh` timed out with literal TeX still displayed. The shared
inline-label renderer now uses the existing viewport tracker and coalesced
after-paint mechanism. Map, scroll, allocation and theme changes recheck actual
nested-scroll geometry; only visible labels enter or leave the pending queue for
rendering. It retains the existing active-job, raster and lifecycle budgets,
generation guards, source text and Copy behavior. There is no idle polling.

`flow-pipeline-dp8eu5s6` passes the stronger first/last-table checks: every tested
visible cell formula has a completed surface and an actual native draw. Its final
capture shows rendered superscripts instead of pending literal TeX. Formula
preparation still uses one render plus 15 reused images (91,429 µs); table-page
availability is 653 ms. The initial GTK gap remains 385 ms. Sampled whole-workflow
native/gateway peaks are 532,996/65,212 KiB, excluding other process peaks.

The normal math-table probe also needed a genuine visibility setup. Its retained
diagnostic shows the target at y=144 below a 100-pixel inner viewport, with no
render failure; resetting only the outer reader could not reveal it. The probe
now uses GTK ColumnView's row scrolling before asserting formula painting. Its
source, Copy, resizing, comment and theme assertions remain intact.

The UI still has a substantial initial layout pause: the mixed workflow's maximum
opening timer gap is 358 ms before and 366 ms after. Formula reuse therefore does
not close the dense-content responsiveness gate. The table-model admission work
below addresses that measured pause separately from Rust preparation.
Dense contexts above the current eager-work limit still need
bounded handling before selectable Preview can be promoted. The installed
candidate and release are unchanged; the ordinary block renderer remains default.

The shared production label suites pass in `inline-labels-eiagcusm` (normal
labels, table excerpts, resizing, Copy, comments, themes and lifecycle) and
`inline-labels-logod8ir` (paged rich math, rapid navigation and literal fallback).
The rebuilt selectable-text example `text-flow-3emz639r` passes its 11 cases.
The four-launch default-application regression `run-ji999bln` passes, including
the copied real report, comparison, paged Edit/Save and restart. Its reader
checkpoint opens the 8,340,599-byte report in 1,327 ms with 81 opening timer ticks.
All 276 native tests pass (two ignored), as do Rust 1.92 and release all-target
checks. Failed paint/visibility probes and the focused pre-optimization sources
and binary hash are retained alongside accepted runs in
`/media/rui/studio-gnome-deps/qa-artifacts/native-smoke-jwfgtzto/mixed-formulas-validation/VALIDATED.json`.

#### Indexed table model viewport admission

The selectable reader now activates GTK table row factories only for table bodies
intersecting their scroll viewports. `viewport::Visibility` reuses the shared
scroll tracker and coalesced after-paint callbacks for map, unmap, resize and
scroll changes. It publishes state transitions rather than polling or rebuilding
on every frame. Parsing and indexed row preparation remain on Rust workers.

Each table retains its existing bounded row page, column objects and metadata.
Its GTK NoSelection model detaches while the body is offscreen and reattaches
when it returns. This releases cell widgets, label shapes and their selection
registrations without losing the current result page or resized columns. A
bounded table viewport keeps the outer text-anchor geometry stable while rows
are absent. Horizontal/vertical offsets are retained in memory and restored
after GTK has laid out the returned model, with at most three extra paint checks
before clamping to available bounds. Re-hiding during restoration retains the
pending target. Document generation/revision and table retirement still guard
callbacks. Full-table Copy uses the immutable backend snapshot rather than the
currently mounted cells.

The optimization currently gates GTK models in the selectable adapter. Indexed
row pages are still requested/cached with the bounded context; it does not defer
those backend reads. The ordinary block renderer keeps its existing GTK list
virtualization. No stored file or preference format changes.

The current before-change Wayland case `flow-pipeline-prtus6jr` records a 386 ms
maximum opening timer gap and 664 ms until all eight mixed-table pages arrive.
Final case `flow-pipeline-8fplmalc` records 71 ms for both that opening gap and the
longer interval through painted cell formulas; pages arrive in 365 ms and the
first painted formulas are ready at 479 ms. Only two table models activate
initially, with 300 cell-factory binds; six tables have no binds. These are host
debug observations from the same mixed source, not a statistical speedup or a
latency guarantee. Sampled whole-workflow native/gateway peaks are
487,996/65,168 KiB, excluding other process peaks.

Actual native End/Home navigation checks hidden-model release and return to the
first table's second page. Its 306-pixel first column, ten remaining rows and both
scroll offsets survive; the inspected restored capture shows rows 51–60 of 60.
All visible formula checks, complete formula/table Copy, code controls, source
comments, view transitions, 256-column controls, dense image/diagram admission
and the copied 8,340,599-byte report still pass. Viewport callbacks settle and
cell bindings stay unchanged during an idle interval. An earlier accepted run
without the final idle/capture checks records a 72 ms opening gap.

Dense contexts exceeding the current eager-work limit, cross-context selection and broader
input/accessibility/platform acceptance remain in scope before promotion.
The installed candidate/release and default-renderer choice are unchanged.

All 276 native tests pass (two ignored), with Rust 1.92 and release all-target
checks. The four-launch default-application regression `run-2m4pckua` passes,
including real-report comments/tables, comparison, paged Edit/Save and restart;
its 8,340,599-byte reader checkpoint opens in 1,325 ms with 81 opening timer ticks.
The before-change, intermediate, final and default regression artifacts, focused
baseline sources and binary hash are retained with verified manifests in
`/media/rui/studio-gnome-deps/qa-artifacts/native-smoke-jwfgtzto/table-admission-validation/VALIDATED.json`.

#### Repeated-formula work admission and Preview errors

Selectable preparation now admits up to 32 distinct render/table jobs rather
than rejecting a context because it contains more than 32 identical formulas.
Math keys include both literal source and display style; tables and eager
diagrams each consume a job. Preflight rejects excess work before starting a
renderer, with cancellation checks throughout. Deferred diagram queues remain
separate. Every anchor still counts toward 256 objects, and each occurrence's
image bytes still count toward the 32-MiB raster admission bound even when
immutable pixels are shared. This does not admit unbounded formula density.

Four new worker tests cover repeated formulas through 256 objects, a 257th
object's rejection, per-occurrence raster accounting, 33 distinct renders
rejected before rendering, and the separate text/display keys at the work limit.
The real document actor opens 64 repeated formulas, mounts 64 native pictures
using one render and 63 reused results, navigates to the last picture with actual
Wayland input, and exports exact TeX/Unicode through an independent clipboard
reader. Source/Preview switching retires the previous widgets.

Rejected fragment requests previously left a blank Preview after the ordinary
footer update replaced the transient error. The reader now retains the error in
its notice area while Preview is selected, with Source still enabled. Opening 33
distinct formulas checks this visible notice, full Source text, retry behavior
and clearing the error on another file. The notice is hidden in Source and does
not change Edit/Save ownership or dirty-state guards.

In `flow-pipeline-3m1p2zs3`, the first 64-formula opening takes 849 ms, including
85,441 µs in rendering, with a 478 ms maximum main-loop timer gap. Returning from
Source to Preview takes 47 ms with a 12 ms gap. These cold/warm paths also differ
in cache state, so this is not evidence that a particular font or layout stage
causes the initial pause. The renderer/cache comparison below now separates
these paths; uncached graphics startup remains a responsiveness consideration.
First/last captures show actual fraction images; the rejection capture shows the
persistent notice. All existing mixed-table, code, diagram/image, source-comment
and copied real-report checks remain in the same pipeline. The default block
renderer and installed packages remain unchanged.

All 280 native tests pass (two ignored), with Rust 1.92 and release all-target
checking. The 11-case standalone selectable-text regression `text-flow-bp0p4sv4`
and four-launch default application regression `run-3gil0m9n` pass. The latter
opens the copied 8,340,599-byte report in 1,325 ms with 81 opening timer ticks;
comparison, paged Edit/Save, source comments and restart remain covered. The
formula pipeline's sampled native/gateway peaks are 520,896/65,240 KiB, not
release memory bounds. Initial rejection/helper/focus failures, final captures,
focused baseline sources, source/binary hashes and unchanged installed-package
hashes are recorded in
`/media/rui/studio-gnome-deps/qa-artifacts/native-smoke-jwfgtzto/repeated-formula-admission-validation/VALIDATED.json`.

#### Separating graphics startup from document layout

Debug reader measurements now distinguish the first shell frame, document
open call, worker raster work, native construction, mount, first painted frame
and a Source-to-Preview return. Construction stages are cumulative microseconds;
native text-view measure/allocation/snapshot-layer counters record total and
maximum microseconds. These counters exist only in debug builds. Heartbeat
measurements include the unfinished interval at the end of each observation:
a blocking first paint can finish before its overdue timer is dispatched, which
previously allowed a misleading zero at that boundary.

The original 478 ms document-opening gap included initial shell startup. With
the same 64-formula source and first shell frame measured separately, debug
`flow-pipeline-3n7j_mbf` uses GTK's `GskVulkanRenderer`: the shell frame/gap is
462 ms, document readiness is 565 ms and its maximum gap is 302 ms. Construction
takes 1,801 µs, mount 3,616 µs and initial native text allocation 1,833 µs. Those
scopes do not account for the large pause. The same functional workflow with
`GskCairoRenderer` takes 17 ms for the shell and 267 ms for document readiness,
with a 27 ms document gap. This points to graphics startup on this host rather
than Markdown projection or widget construction; it does not isolate a specific
driver call or establish a universal renderer ranking.

The script's `--graphics-cache-from` comparison copies only known GPU cache
directories from a completed owned fixture, with file hashes, regular-file/link
checks and an 8,192-file / 128-MiB admission bound. Application/profile, font and
document caches stay fresh; no user cache is read, moved or cleared. All fixture
sources retain their hashes. The actual selected renderer is recorded, along
with the requested environment override and cache-seed manifest.

| Renderer / GPU cache | First shell frame / gap | First document ready | Document max gap | Source → Preview |
| --- | ---: | ---: | ---: | ---: |
| Vulkan / empty | 462 ms | 565 ms | 302 ms | 46 ms |
| Vulkan / owned seed | 43 ms | 287 ms | 27 ms | 46 ms |
| OpenGL / empty | 428 ms | 514 ms | 248 ms | 45 ms |
| OpenGL / owned seed | 28 ms | 270 ms | 28 ms | 47 ms |
| Cairo / empty | 17 ms | 267 ms | 27 ms | 79 ms |

These are individual Ubuntu 26.04.1 / GTK 4.22.4 host observations, not controlled
multi-device benchmarks. Vulkan and OpenGL both lose most of the measured cold
pause when only their owned graphics cache is supplied. The mature GTK renderer
and driver already provide this reuse; the application preserves GTK's renderer
selection and the normal persistent XDG cache environment. No shader cache is
implemented by the Markdown parser or application, and cache survival is not a
correctness requirement. Fresh-install/driver-update cold startup, physical
devices, scrolling, accessibility and full renderer/content acceptance remain
separate gates.

For diagnosis, GTK documents `GSK_RENDERER=vulkan`, `opengl` and `cairo`; these
select the drawing renderer while the tests retain `GDK_BACKEND=wayland`. See
[GTK's running/debugging reference](https://docs.gtk.org/gtk4/running.html#gsk_renderer).
For example, compare the real debug reader workflow with an empty private cache,
then reuse only that run's GPU cache:

```bash
GSK_RENDERER=opengl python3 scripts/smoke-gnome-flow-preview.py DEBUG_NATIVE GATEWAY --reference LARGE_MD
GSK_RENDERER=opengl python3 scripts/smoke-gnome-flow-preview.py DEBUG_NATIVE GATEWAY --reference LARGE_MD --graphics-cache-from target/gnome-native-smoke/flow-pipeline-RUN
```

Both paths keep the complete formula/Copy, Source error recovery, code/gutter,
table, diagram/image, comment and copied real-report checks. No rendering feature
is disabled to obtain the timing comparison.

The final hash-checked seeded-Vulkan run `flow-pipeline-pf_gcif5` passes with a
50 ms first shell frame, 278 ms document readiness and 27 ms document gap. Its
Source-to-Preview return takes 45 ms with an 11 ms gap; sampled native/gateway
peaks for the entire workflow are 475,184/65,424 KiB. The rebuilt standalone
selectable-text example `text-flow-lzog10h2` passes all 11 cases. All 280 native
tests pass (two ignored), along with Rust 1.92 and release all-target checks.
These debug diagnostics do not promote the selectable renderer or rebuild an
installed release. Nine complete reader pipelines, the standalone regression,
initial probes with their timing caveats, source/binary hashes and unchanged
installed-package hashes are retained in
`/media/rui/studio-gnome-deps/qa-artifacts/native-smoke-jwfgtzto/graphics-startup-validation/VALIDATED.json`.

#### Deferred native diagrams

`text_flow::prepare_lazy_diagrams` prepares the projection and code styles without
running standalone Mermaid, PlantUML, D2 or display-math engines. Inline math still
uses the existing worker preparation. `Options.diagrams` supplies a fallible native
widget factory; a deferred projection without a factory is rejected. The original
diagram source remains in the selection model, so engine errors do not discard
surrounding text or source Copy. The eager path remains available for the existing
standalone comparisons and bounded rich-paragraph preparation.

The experimental reader adapter now uses the same native controls and separate
render queue as conversation diagrams: Source, vector inspection, retry and live
theme handling. Each reader/comparison side has an independent renderer generation
and event route. Opening another file, leaving Preview, closing or receiving a
normal block fragment restores the production reader and cancels the experimental
widgets. Stale replies cannot update a replacement view. Saved-snapshot rendering
is independent of the stricter comment eligibility for dirty edits.

Each reader's deferred previews charge at most 32 MiB of retained pixel and SVG
data, and preview height is capped at 720 pixels; full vector inspection remains
available. Excess results are rejected before creating a texture, leaving native
Source/retry controls. Unmap/reset clears textures, SVG references and charges.
This is an owner budget, not a process RSS limit: engine work, in-flight replies,
the existing 16-MiB backend cache, GTK frame caches and explicit inspectors are
separate. Queue admission remains bounded. Viewport admission and indexed-table
integration are described below. Cross-page selection and broader rendering/input
acceptance still precede promotion. **The ordinary reader still uses its block
renderer.**

The private real-reader fixture installs this adapter explicitly on an indexed
small document, tests all four real engines plus an invalid Mermaid diagram,
original-source Copy, theme replay, live English/Chinese Source controls and
actual texture/SVG release on Source switching. A synthetic native accounting
probe verifies budget rejection, released-budget reuse, reset and a pathological
1×512 aspect ratio. Selection tests explicitly remount their old experimental
projection after a mode switch and deliberately map a stale revision after Save,
retaining the existing comment-safety checks.

Final private Ubuntu 26.04 Wayland run `run-c53_si9v` passes this adapter check
inside the four-launch application regression. Its small, already-indexed
diagram fixture prepared metadata in under 1 ms, allocated selectable text in
22 ms and retained zero pre-mount diagram raster bytes. These are debug fixture
measurements, not cold-opening figures for the large report or a comparison
against the eager probe's different workload. The same run independently checks
the ordinary reader against the full 4,097,226-byte report. It records no
application GTK warnings; the earlier Source scrollbar warning reproduced in
`run-vdkbr1pw` and remains unresolved.

All 238 native unit tests pass (two normally ignored), and Rust 1.92 checks all
native targets. `inline-labels-k6nmjbpi` retains production inline math, full-cell
Find, native pane dragging and source-selection coverage. The eleven-case eager
probe `text-flow-gghcxax0` retains pointer/keyboard selection, independent Copy,
code colors, table/image/column controls and its real-report prefix cases. Its
reference input is only the first 64 KiB through a complete line, not the entire
report. Formatting, catalog, Python syntax and diff checks pass. The installed
optimized package is unchanged, and the complete native application goal remains
active.

#### Diagram viewport admission

The experimental reader uses `Renderer::new_viewport_bounded`. Diagram cards
observe all ancestor native scroll regions through a shared `viewport` helper,
also used by table images. Both horizontal and vertical clipping must allow a
card before its engine request starts. Adjustment/layout signals coalesce into a
single update after that window's normal GTK layout/paint cycle; there is no idle
polling timer. Two active requests and the
existing pending-queue bound remain in place. Ordinary conversation rendering
keeps its existing mapping-based policy.

The same one-shot `viewport::FrameUpdate` also schedules table-image visibility.
Each affected window gets its own frame callback, so one window's paint does not
consume another window's pending update. Weak frame references and explicit
disconnects release retired clocks/handlers. The earlier 16-ms timeout could run
before GTK updated a scroll transform and leave an actually offscreen preview
resident; the Retry/scroll fixture exposed that race.

Leaving the viewport cancels queued/running requests, invalidates their serials
and releases retained pixels/SVG data before admitting replacements. The native
picture retains its measured aspect ratio, so releasing a preview does not
collapse the card. Reentering can use the shared backend cache. A native spinner
overlays retained-size previews while they reload, without adding a status row
that shifts the surrounding text. Theme changes
invalidate previews and render only the visible cards. Errors settle until Retry
or an explicit theme/preferences change; scroll notifications do not repeatedly
run a failed engine. Source remains available independently of a raster.

The private Wayland probe uses an owned native window and synthetic pixel replies
to check two-slot admission, nested vertical/horizontal clipping, cancellation,
late replies, retained geometry, failure/Retry, idle stability and reset cleanup.
The separate real-reader fixture now visits Mermaid, PlantUML, D2, display math
and the invalid Mermaid source sequentially instead of requiring all offscreen
rasters to remain resident at once. This preserves the real-engine, Copy, theme,
language and Source-mode checks. Whole-file continuous rendering, cross-page
selection and broader input/accessibility acceptance remain separate work.

Final private Ubuntu 26.04.1 Wayland run `run-tt4hihbv` passes all four application
launches, including the new native Retry/idle-stability check and indexed-table
regression. The real diagram and viewport screenshots were inspected. The
eleven-case `text-flow-jvlwb6ec` probe also passes shared table-image clipping,
late replies, unmap cleanup, source selection, column controls and independent
Copy. Its reference workload is only the real report's first 64 KiB through a
complete line; the application regression separately opens the complete report.
These checks do not establish whole-file continuous rendering performance.

The pre-fix reproduction `run-n2gqm054` records an offscreen card at y=1840 with
204,800 retained pixel bytes, no queued visibility callback and `inView=false`.
`run-7yb2n_b_` also fails that release check. The shared frame scheduler passes it
in `run-be5531rd` and the final run, without moving focus or weakening the release
assertion. The native spinner/ready-frame polish removes blank transitional
captures. The final run still records the previously tracked GtkGizmo warning;
this change does not resolve that separate warning.

All 239 native tests pass (two normally ignored), as do Rust 1.92 all-target and
release-mode all-target checks, formatting, locale generation, Python syntax and
diff checks. The installed optimized package remains unchanged.

#### Indexed tables in continuous text

The experimental projection now retains each table's optional index ID and row
offset. `Options.indexed_tables` mounts the same `TableUi` used by the ordinary
reader, rather than treating a fragment's rows as the complete table. Backend
requests still retrieve at most 50 rows and project the selected original
columns. Native column filtering, widths, wrapping, header/full-cell readers,
images, math and source-linked cell comments use the shared production widgets.
Local, nonindexed fragment tables retain their existing in-memory paging path.

Each reader keeps a separate registry for experimental table anchors. Replies
must match the reader generation and live table serial; errors from a discarded
table cannot overwrite the new view's status. Mode changes drain that registry,
release row models and invalidate old factories even when the file revision is
unchanged. Valid adjacent continuations now join on the worker as described
below; missing factories, inconsistent repetitions and invalid index/row values
remain explicit errors. Existing object/fragment/column preparation bounds still
apply.

Snapshot-backed continuous views now resolve selected indexed tables through a
bounded gateway read lane. Copy, primary selection and comments treat each table
as one atomic object: its complete original columns and rows are exported,
independent of the displayed row page or column filter. Views without a trusted
snapshot resolver still reject indexed-table selection; exact cell actions remain
available. General cross-page text selection and continuous-reader default
promotion remain separate requirements.
Private Wayland run `run-prmfmgya` passes the four-launch application regression.
The table check opens the complete 4,097,226-byte report copy, then explicitly
mounts only its first indexed table fragment in the experimental adapter. It
checks the actual nine-column, 81-row table through backend pages of 50 rows,
column filtering/width restoration, stable outer scrolling, cell clipboard text
and source-linked comments, mode cleanup and rejected stale replies/factories.
The report hash is unchanged, and the screenshot was inspected. This is bounded
integration coverage, not a whole-file continuous rendering benchmark.

The first attempt, `run-9v675wu2`, stopped at a blank Router-settings screenshot
before writing the final application report. That fixture now waits for a real
GTK frame after saving. A reader checkpoint is written immediately, so later
workflow failures retain its independent result. The final run passes Router
settings and all restart checks, but still records the previously tracked
unallocated-GtkGizmo warning; this increment does not resolve that warning.

Validation also passes 239 native tests (two normally ignored), 87 shared
Markdown tests without syntax highlighting, 91 with it, and Rust 1.92 checking
all native targets. Formatting, generated locale, Python syntax and diff checks
pass. The installed optimized package remains unchanged.

#### Snapshot-owned table continuations

`flow::project_context` accepts ordered source fragments from one document and
revision. `text_flow::prepare_lazy_context` uses it on a worker; the single-page
diagram preparer delegates to the same path. Multi-fragment contexts require
nonempty snapshot identities, increasing page IDs and nonoverlapping, ordered
body source windows. Borrowed headers keep their separate original coordinates.
Already-parsed `project_blocks` callers remain responsible for snapshot ownership.

Adjacent indexed-table chunks join only when IDs, headers, structure and row
offsets agree. Rows and cell source objects move unchanged; one header and one
table anchor remain. An exact adjacent replay with identical source coordinates
is idempotent. Different table IDs and unindexed local tables stay separate.
Gaps, partial overlaps, reversed ranges, changed headers/structure, overflowing
row indices and nonadjacent reuse of an ID are rejected instead of being sorted
or silently deduplicated.

Context work is bounded to 64 fragments, 2 MiB of source/header text and 2,048
display rows before joining. Parsed table work is additionally limited to
65,536 cells and 2 MiB of plain cell text including separators. Existing compact
buffer/object limits and the 512-KiB per-table alternative limit still apply,
including header-only tables. These bound worker/model work; they are not a
process RSS guarantee or permission to load the complete document.

The private real-report fixture reads two actual adjacent index-page ranges
from its immutable report copy, borrowing the original header. The worker joins
32 rows into one anchor and compares every cell, including its source coordinates,
against cells independently parsed from the gateway's table-source response.
The native widget
continues to retrieve 50-row pages for the full 81-row table. This checks actual
index boundaries and real data without claiming that the adapter renders the
entire file continuously. The intermediate `run-cx5v_omo` used an artificial
split within one index page; `run-yw112bat` strengthens that to actual page ranges.

The final four-launch Wayland regression passes, as does the eleven-case
`text-flow-n1a6s3n_` compatibility probe. That standalone probe still uses only
the report's first 64 KiB through a complete line; it is not a whole-file
continuous-rendering benchmark. The final application log retains the tracked
GtkGizmo warning and a disconnected fake OpenCode HTTP writer (`BrokenPipeError`);
restart logs and the standalone probe log are clean. Neither diagnostic is
claimed fixed by this change.

Validation passes 240 native tests (two normally ignored), 92 default shared
Markdown tests, 96 with syntax highlighting, Rust 1.92 all-target checks and
release-mode all-target checks. Formatting, locale, Python syntax and diff checks
pass. The installed optimized package remains unchanged. Complete indexed-table
selection is described below; general cross-page text selection and
continuous-reader promotion remain separate work.

#### Complete indexed-table selection

The shared projection first freezes selected table IDs and the byte allowance
remaining after ordinary selected text. The gateway's authenticated `table-text`
operation reads those tables from the existing immutable reader snapshot on a
Rust worker. It validates contiguous rows and unchanged headers, emits one header,
and returns original source bounds. No diagram engine or browser is required.
Requests accept at most 32 distinct table IDs, 512 KiB of output, 4,096 source
pages and 16 MiB of source/header work. Oversized results fail without publishing
partial clipboard content. Comments retain their separate 16-KiB limit.

Two independent selection slots leave navigation/source requests available while
a clipboard read is pending. Native view generation, mode epoch, document ID and
revision must still match when the result arrives. Unmapping/replacing a view
cancels its pending receiver and withdraws its primary offer. The same worker
pipeline supplies Copy, Wayland primary selection and source-linked comments.
Flow comment relocation scans complete atomic tables, rather than treating a
repeated fragment header as a new table. A known oversized atomic table creates a
scanner boundary; text on either side cannot become a false contiguous match.

The private API fixture covers full CRLF/Unicode tables spanning real index
chunks, complete-table quote relocation, duplicate contexts, oversized-table
boundaries, request authentication/origin, stale identity and closed snapshots.
The native transport test holds a table response open, verifies Source still
responds, then drops the offer and verifies the socket closes. The real-report
Wayland fixture compares clipboard and primary text against all 81 original rows
(11,195 bytes), validates the complete source-linked comment, and cancels a held
selection on Source switching without replacing the previous clipboard.

A debugger reproduction caught GTK measuring an inert, fixed-width column cell
with no child, from background TextView validation after an old adapter unrooted.
The cancellation fixture had replaced that adapter without its reader cleanup.
Reader cleanup now detaches expired indexed anchors before unrooting and releases
their models; the fixture uses that same lifecycle. Schema changes also detach
the row model while columns change and retain stable factory cell containers.
GTK 4.22.4's [measurement path](https://raw.githubusercontent.com/GNOME/gtk/4.22.4/gtk/gtkcolumnviewcellwidget.c)
contains an unguarded child measurement for fixed widths. This is distinct from
the previously tracked GtkGizmo warning.

The diagram fixture now navigates using TextView buffer anchors. Its previous
child-bounds navigation could target negative placeholder allocations for
unexposed anchors; a captured failure recorded x=-1631, y=-1125 with scroll=0.
It waits for real GTK frames and retains engine, clipping and cancellation
assertions; it does not force allocation or extend the deadlines.
Final private Wayland run `run-7zhxs13a` passes all four application launches
with GTK criticals treated as fatal; its application and restart logs have no GTK
criticals or warnings. Earlier runs and debugger traces are retained in the QA
archive. The separate tracked GtkGizmo warning is not claimed universally fixed.
The table screenshot was inspected. Observed whole-workflow native/gateway peaks
were 590,300/216,988 KiB, including deliberately retained test views and the other
application workflows; these are not per-table or cold-start memory guarantees.

The eleven-case `text-flow-egevjd1r` probe passes actual pointer selection,
independent clipboard reads, real diagram engines, local tables, column controls,
image clipping and unmap cleanup, with an empty application log. Its reference
input remains only the real report's first 64 KiB through a complete line. The
application check separately opens the complete 4,097,226-byte file; neither
check establishes whole-file continuous-rendering performance.

Validation passes 95 default shared Markdown tests, 99 with syntax highlighting,
237 gateway tests (one ignored), and 241 native tests (two ignored), plus Rust
1.92 and release-mode checks of all native targets. Formatting, locale, Python
syntax and diff checks pass. Results and file hashes are retained in
`/media/rui/studio-gnome-deps/qa-artifacts/native-smoke-jwfgtzto/table-selection-validation/`.
The installed optimized package has not been rebuilt by this increment.

#### Earlier standalone measurements

Private Wayland run `text-flow-nrcui1jf` passes actual pointer selection spanning
a paragraph, inline formula, code and table; independent compositor clipboard
reads; Unicode; exact atomic object copies; actual Mermaid/PlantUML/D2/display
math engines; and actual table Next input. Paging advances original row lines
13 to 29, mounts 16 rows and leaves the outer scroll unchanged. Screenshots were
inspected. The reference is the real 4,097,226-byte `tests.review.md`; this probe
reads **only its first 64 KiB, through the last complete line**, not the whole
file or the production index/transport workflow. That prefix contains four tables
of 81×9, 81×5, 13×2 and 69×2 data cells. Copy retains 60,754 bytes and containing
source lines 1–269 while GTK's compact text buffer holds 820 bytes.

Local debug observations, in milliseconds:

| Variant | First reference allocation | Repeated allocation | Longest opening GTK tick gap, first / repeat |
|---|---:|---:|---:|
| Hidden alternatives, unpaged cells (`text-flow-60uszjm0`) | 452 | unmeasured | 349 / unmeasured |
| Compact anchors, bounded excerpts, unpaged rows (`text-flow-rq0cghr3`) | 399 | 220 | 273 / 126 |
| Compact anchors, bounded excerpts, paged rows (`text-flow-nrcui1jf`) | 260 | 83 | 214 / 43 |

The compact-anchor-only intermediate (`text-flow-ormfylzy`) still took 447 ms
to allocate and had a 312 ms tick gap. Thus buffer size alone does not explain
the pause. Bounded GTK table layout materially helps; cold layout/font costs
still need investigation. These are individual diagnostic runs, not distributions
or release-performance guarantees. The unpaged repeated case also ran alongside
a native test build; compare it cautiously. Allocation means the view and child
anchors have sizes, not a measured presentation timestamp. The separately
reported settled-open time adds a fixed 120 ms observation interval. Final paged
worker/widget construction took about 6/9 ms; native-process peak RSS was
333,336 KiB across all cases. That peak excludes sandbox engine-worker memory.
The synthetic diagram case spends about 1.3 seconds on cold engine work while
the GTK timer continues; this is not a diagram latency improvement.

The initial probe rejected images; the document-bound image adapter described
below now covers that projection gap. Table cells still need complete math
integration; shared column controls and live paging labels are covered below, and
diagram sizing/theme/zoom are simplified. The document-bound comment adapter is
covered below. Cross-page selection, primary-selection clipboard,
bidi/IME/accessibility, broader resize coverage and cold
responsiveness remain acceptance work. The renderer is not promoted while these
gaps remain. The initial checkpoint passed 65 shared Markdown and 230 native cases (two native
cases normally ignored); Rust 1.92 checking passes all native targets, including
the probe. The initial anchor-only experiment did not change the optimized
application/gateway package. Current package validation is recorded above.

The native implementation now has a reusable `src/lib.rs`; its binary entry
point delegates to the same application startup and pre-GTK math-worker dispatch.
The text-flow probe imports this library instead of compiling private copies of
native widget modules. Embedded tables use the application's existing rich cell
widgets, header menus and full-cell windows, including native selection, bounded
long-cell display and owner-unmap cleanup. Small cells retain the ordinary short
label policy; requesting paragraph flow attributes does not turn every short
table cell into a paged text view.

Read-only TextViews now share `text_links` input handling and caller-owned URI
routing. This also fixes production link hit-testing: GTK's
[character-position lookup](https://docs.gtk.org/gtk4/method.TextView.get_iter_at_position.html)
identifies the character under the pointer, whereas the previous insertion-cursor
lookup could skip narrow glyphs or the trailing half of a link's last glyph.
Ordinary selection, native click timing, Ctrl+Enter and the existing routing
policy remain intact. Unmapped views ignore stale link actions; paged paragraphs
also check current source ownership before resolving a destination.

Clean private run `text-flow-034y4tkz`, without diagnostic event observers, passes
actual Ctrl+Enter, Ctrl+click on a narrow glyph, the right edge of a final Unicode
emoji, plain-click selection and adjacent blank-space rejection. It also opens
the shared full-cell window through the pointer, verifies styled/selectable
content, copies all 1,744 bytes through the real control and an independent
clipboard read, closes through native Alt+F4, and checks owner-unmap cleanup plus
stale-link rejection. Cross-block copying, real diagram engines and reference
table paging remain passing. The full-cell screenshot shows original source
line 12. Native-process peak RSS across this extended fixture was 323,600 KiB;
engine workers are excluded. Its longer synthetic/full-cell phase warms fonts,
so its reference timings are not a cold-layout comparison with the earlier table.

The production application passes `rich-text-ui-07myn6rn` after library extraction:
chat/reader long-paragraph styles, Unicode selection, exact comment source lines,
source/preview restoration, width reflow and reading-position retention, paged
search targets, trailing-space matching, relative link routing and draft retention.
It starts no model turns and cleans up its private providers. Rust 1.92 checks all
native targets; 230 native and 65 shared Markdown tests pass. This increment
removes duplicated cell/viewer widgets and fixes character hit-testing; it does not promote
the experimental renderer or close the remaining migration gates.

Four-launch production regression `run-d_e0e_yl` also passes after library
extraction. Its private copy of the real 4,097,226-byte report opens in 940 ms
with 57 loading GTK ticks, preserving original source lines, column selection,
table paging, paged editing, dirty view switches, Unicode/CRLF saves, undo/redo,
save conflicts and provider cleanup. These are local observations, not a new
latency guarantee. The refreshed optimized pair passes installed
`release-ui-0y2jeua7` with all five diagram/language/backend/Git/viewer groups,
no application hooks and sibling-gateway discovery. That broader workload
samples native/gateway peaks of 639,228/106,368 KiB, excluding worker peaks;
two lazy web-content processes appear only for the explicit viewer workflows.
The pair is now at the current package path above. The previous validated pair
is retained in `current-release/package-before-native-library-20261026` on the
external dependency disk. Library inspection retains GTK4/WebKitGTK 6 without
GTK3 in the frontend and no desktop engines in the gateway. The 237-test gateway
suite also passes (one normally ignored test), and all three private-desktop
cleanup tests pass.

```bash
cargo build --manifest-path native/gnome/Cargo.toml --example text_flow_selection --locked
STUDIO_GNOME_PLANTUML_JAR=/absolute/path/to/plantuml.jar \
STUDIO_GNOME_D2_BIN=/absolute/path/to/d2 \
python3 scripts/smoke-gnome-text-flow.py \
  /absolute/path/to/target/debug/examples/text_flow_selection \
  --reference /absolute/path/to/tests.review.md
```

The fixture requires a local Mutter with working user namespaces for the real
external engines. It starts its own headless Wayland compositor, clipboard/input
session and private profile; it never automates the user's desktop or sends
model prompts. It is optional host acceptance, not a container-CI requirement.

### Native worker code colors

The continuous-flow prototype now consumes worker-prepared syntax colors from
the optional `studio-markdown-native/syntax-highlight` feature. A syntax pass
produces Unicode ranges; two theme projections reuse those parse operations.
GTK applies bounded tags in the existing text buffer. It neither reparses code
nor replaces text to switch themes. Source byte/line ranges, Unicode selection,
copy and dirty state are retained. The default application still uses its
existing block/GtkSourceView renderer; this closes a prototype compatibility gap.

The implementation uses Syntect 5.3.0 with two-face 0.5.2+bat-0.26.1's
[extended syntax definitions](https://docs.rs/crate/two-face/0.5.2%2Bbat-0.26.1/source/).
Syntect's default bundle did not recognize TypeScript. The extended bundle
covers TypeScript, TypeScriptReact, JSX, TOML and Dockerfile alongside the other
tested languages. It is loaded lazily on a Rust worker; pages without code do
not initialize the syntax set. The default shared Markdown feature set has no
new syntax dependency.

**Oniguruma is preferred for this native adapter.** The same private eleven-case
Wayland workload compared the pure-Rust fancy-regex backend
(`text-flow-h9mz2c62`) with Oniguruma (`text-flow-eidw8d54`):

| Observation | fancy-regex | Selected Oniguruma |
|---|---:|---:|
| First TypeScript preparation | 207.6 ms | 11.4 ms |
| Repeated TypeScript preparation | 4.6 ms | 0.20 ms |
| First TSX preparation | 196.4 ms | 10.8 ms |
| Repeated TSX preparation | 0.41 ms | 0.17 ms |
| Native process peak RSS, complete workload | 426,440 KiB | 335,620 KiB |

The per-stage RSS observations place most of the difference in grammar
preparation: TypeScript/TSX worker-stage increases were about 43.7/40.8 MiB with
fancy-regex and 3.4/3.4 MiB with Oniguruma. These are process observations,
including allocator/background activity, not exact engine heap accounting.
They exclude sandbox engine-worker memory. The code samples are small and the
numbers are individual diagnostic runs; they are not general file-opening or
large-code performance guarantees. The real 4,097,226-byte report still uses
only its first 64 KiB in this probe, and contains no code in that prefix.

Both workspaces optimize dependency kernels in development builds while keeping
application/debug control flow unchanged. Unoptimized cold TypeScript regex
compilation had exceeded the color budget; the optimized implementations retain
the original budget. Release optimization settings are unchanged. Regex backend
selection is a build-time choice, with no runtime engine fallback. Oniguruma's
bundled C sources compile statically through `onig_sys`; the existing C build
toolchain suffices. Library inspection found no dynamic `libonig` dependency.
The current source pins all dependencies in both lockfiles. The previously
validated optimized application/gateway package remains unchanged.

Coloring admits at most 64 KiB per code block, 256 KiB of code per fragment,
4 KiB per line, 8,192 non-base runs across both themes, and 128 paints. A 500 ms
deadline and cancellation are checked cooperatively between engine calls;
they do not preempt an individual regex operation. Unknown languages or exceeded
color budgets retain the complete monospace code and expose an explicit status
in the prepared model. Syntax metadata estimates exclude the shared engine and
compiled-grammar caches. Text is never truncated to satisfy a color budget.

The actual GTK fixture verifies light/dark colors for Rust, TypeScript and TSX,
unchanged text/selection/source lines and dirty state, independent clipboard
copies, and bounded tags through repeated theme changes. It also verifies native
links/full-cell controls, diagram rendering, real-report paging, complete unknown
and long-line code, and destruction of old widgets. Changed or replaced buffers
invalidate syntax callbacks and source-aware copying; stale copies leave the
clipboard untouched. Dark-code screenshots were inspected. Diagram/inline-math
theme refresh remains a separate, visible prototype limitation.

Unit checks pass 69 Markdown cases with the feature, 65 with the default feature
set, and 230 native cases (two normally ignored). Rust 1.92 checks all native
targets. The packager includes unmodified Syntect, two-face, Rust-Onig and
Oniguruma notices. A small Rust example reproduces two-face's syntax attribution
from its official formatter; CI compares the output and tests the optional
feature. Package notice contents were checked using temporary external-disk
output. These checks leave the full native migration and renderer promotion
gates active.

### Native text-flow image anchors

The experimental continuous reader now embeds paragraph images at their actual
Unicode positions. Its worker model retains complete alt text for Copy and the
containing original source range, including compacted data-image references.
Empty-alt images occupy an anchor without capturing an adjacent word's link or
style. Nested image alt text and inline math do not create overlapping objects;
bounded captions do not truncate copied text.

`view_with_images` takes the owning document's existing image callback. The
native image renderer supplies that callback through `flow_images`, keeping the
workspace root, document path and reader/revision context. Paragraph anchors,
table cells, full-cell windows and header reading windows share the same loader.
There is no new file reader, HTTP fetcher or codec in the TextView adapter.
Existing limits still govern image metadata, two concurrent requests, pending
work, decoded pixels and cached resources. Offscreen previews are admitted by
the existing viewport checks; unmapping cancels callbacks and releases textures.
Unsupported remote references retain readable alt text without fetching them.
An image-bearing page without a document renderer fails explicitly.

The private Wayland fixture covers three source kinds (workspace file, decoded
embedded data and revision-bound snapshot handle), paragraph and table images,
header windows, native Copy and close input, deferred offscreen admission,
unmap/remap and replies arriving after close. It uses the production GTK image
renderer with **synthetic RGBA replies**, not real codec or file-service results.
The separate image API fixture checks real PNG/SVG decoding, original bytes and
identity, changed-file rejection, root escape rejection, authentication, bounded
SVG resources and a canary proving that SVG references cause no remote fetch.
That API fixture uses the existing optimized gateway; this experiment does not
replace the validated installed package.

The shared Markdown suites pass 74 cases with syntax colors and 70 without;
the native library passes 230 cases (two normally ignored). Full text-flow
acceptance also retains the real report prefix, diagrams, Unicode links,
cross-block selection, paged tables, code themes and stale-source rejection.
The real-report measurements still cover only its first 64 KiB, not the complete
4.1 MB indexing/opening workflow. Image sizing, full theme/zoom integration,
selection comments, production copy/column-selection semantics, broader input/accessibility and cold layout
remain promotion gates. The application continues using its existing renderer.

Evidence: `text-flow-812ifsgj` passes the complete eleven-case workload plus
image lifecycle checks. Focused run `text-flow-av19yn_x` additionally centers the
table image's pixels inside its inner viewport before capture; table and header
screenshots were inspected. `images-ui-rr30p14y` passes the actual gateway image
API checks. Rust 1.92 checks all native targets; formatting, Python syntax and
diff whitespace checks also pass. Completed runs and validation logs are kept
under the external QA artifact archive, with local run paths retained as links.

To isolate image lifecycle and viewport checks from diagram measurements:

```bash
python3 scripts/smoke-gnome-text-flow.py \
  /absolute/path/to/target/debug/examples/text_flow_selection --images-only
```

### Shared native table column picker

Chat, the indexed document reader and the experimental continuous reader now use
one GTK ListView column picker. Its sorted original-column indices preserve
source identity through filtering and recycling. The policy keeps at least one
column selected, prevents duplicates and rejects proposals exceeding the caller's
cell/column budget. The document and chat paths retain their 512-column limit;
the continuous prototype remains limited to 64 columns and at most 16 rows per
table page. Choices and resized widths remain in the open table's memory; no
new settings file or persisted preference is introduced.

The virtual list rebinds a checkbox to its current item, instead of capturing a
column index at widget creation. A 2,000-choice private Wayland fixture creates
205 checkbox widgets on this host and keeps that count while moving between the
first and last choices. Native keyboard input checks the selection limit,
recycled identity, rejection by an expired owner, and unmapped callbacks.
Small tables use the same component and retain ordinary checkbox interaction.
Checkboxes use the platform's GTK theme and native focus indicators.

Document tables retain resized widths by original column index when filtering
rebuilds the displayed columns. An old table freezes its reader generation and
revision; navigation and column callbacks cannot query a replacement document.
The continuous reader keeps the existing column objects, preserves the first
source row on the current page, and adjusts its bounded page size to the visible
column count. Its visual filtering still leaves atomic table Copy as the complete
cached table fragment, including hidden columns and other rows in that fragment.
This prototype copy policy remains explicit; comments over a complete atomic
table use the same cached alternative. Selection across rendered table cells and
across reader pages remains separate parity work.

Picker counts, navigation tooltips and row counts bind through the existing live
language mechanism. Column names remain literal data, even when a header equals
an interface word such as `Open` or `Columns`. Native additions are regenerated
into the checked-in catalog with `node scripts/vendor-gnome-i18n.mjs`; the
application loads no Node process. The fixture checks live Chinese/English state,
literal headers, stable source rows and widths, complete source-aware Copy and
stale owner handling. The application regression exercises the actual chat and
document picker, including hiding/restoring a resized column and replacement
document rejection. The continuous reader remains experimental.

Private Wayland evidence: focused run `text-flow-h1pxyjsx` and complete reader
run `text-flow-g9osn5te` both keep 205 checkbox widgets for 2,000 choices while
scrolling. The latter retains the real report's first 64 KiB, actual diagram
engines, physical selection/link/table controls and image lifecycle checks.
Screenshots of the wide picker and literal source labels were inspected.
The four-launch application regression `run-d9gfb8n9` opens the complete
4,097,226-byte report and passes chat columns, document columns, width restoration,
stale table rejection, source/edit/save/conflict handling and managed provider
cleanup. These are distinct workload scopes; the prefix probe does not stand in
for complete-file loading. Rust 1.92 checks all native targets, and native unit
checks pass 232 cases (two normally ignored). Catalog generation consistency,
formatting, Python syntax and diff checks pass. The installed optimized package
is retained at its previous validated checkpoint.

```bash
python3 scripts/smoke-gnome-text-flow.py \
  /absolute/path/to/target/debug/examples/text_flow_selection --columns-only
node scripts/vendor-gnome-i18n.mjs --check
```

### Responsive continuous-reader tables

The experimental reader no longer requests a fixed 720-pixel table viewport.
A small native TextView subclass lets GTK allocate the wrapped text first,
then fits table anchors to that width minus the original paragraph indent and
reading margins. A previous wide anchor cannot become the TextView's minimum
width and prevent a pane from shrinking. Unwrapped text retains GTK's ordinary
measurement behavior. Vertical layout, text interaction and accessibility
continue through the parent TextView implementation.

Allocation changes only the anchor width request. It does not replace the
buffer, rebuild the table model, change column widths or rewrite source ranges.
The adapter keeps weak child references and rejects detached anchors. Height-only
allocations leave widths alone, and an unchanged width schedules no further
width updates. No frame callback, timer or backend round trip drives this sizing.
Table controls use the shared libadwaita wrapping toolbar; wide columns scroll
inside the table viewport.

The application fixture exercises the actual document pane at requested widths
900, 420 and 700 pixels, including returning to 420. It checks table/margin
geometry, reachable table and reader-header controls, fixed 700/220-pixel columns,
horizontal scroll, unchanged buffer/selection and syntax ownership. An independent
Wayland clipboard reader verifies complete projected table Copy after resizing.
An idle observation checks that width updates stop. The small math fixture and
real-report prefix/full-application checks retain their distinct scopes; this
adapter remains experimental while the broader reader parity gates are open.

Private Wayland acceptance observes pane/table widths of 899/851, 419/371 and
699/651 pixels, keeping 24-pixel reading margins. Each requested width change
updates the single anchor once; an additional 120 ms idle interval leaves the
update count unchanged. Allocation was observed after 20–21 ms in this debug
run; the reported settled time also includes a deliberate 40 ms observation
interval. These are individual fixture observations, not release-performance
guarantees or sustained-drag measurements.

The owned 1600×1000 Wayland fixture also drags the actual GtkPaned divider by
120 pixels. It requires the native separator press, a recognized button-1 drag
and the final −120-pixel gesture update, in addition to the resulting pane/table
geometry and unchanged source/selection/columns. Pointer focus is established
with a zero-displacement click before recording the asserted drag; keyboard
activation alone did not provide that precondition in this standalone compositor
fixture. The check does not substitute a position setter for native input.

Verified private run `inline-labels-tolspq34` preserves table state through those
width changes and the native drag, then passes exact math/excerpt/complete-cell
Copy, original source-line comments and unmap cleanup. Full-window and pane-sized
screenshots were inspected; the header wraps instead of losing its final actions.
`text-flow-tgtgl8vp` passes the complete eleven-case experiment, including actual
diagram engines, the real report's first 64 KiB, physical selection/link/table
input, image lifecycle and the shared column picker. Four-launch application run
`run-iz318y72` opens the complete 4,097,226-byte report with the default block
renderer and passes source/edit/save/conflict, column/paging, comment reopening
and restart checks. Native unit checks pass 232 cases (two normally ignored),
Rust 1.92 checks all native targets, and formatting/Python syntax/diff checks pass.
The installed optimized application/gateway pair is unchanged by this increment.
The primary-selection gate described at that checkpoint is now covered for a
bounded fragment below. Complete table math, broader layout/styles, cross-page
selection, IME/accessibility and cold responsiveness still precede promotion.

### Continuous-reader primary selection

The experimental projected TextView now offers expanded plain text through
Wayland primary selection, using the same immutable model as Copy. Formula,
diagram, image and table anchors export their original text alternatives; a
literal U+FFFC in a document stays literal. Tables remain atomic and export the
cached fragment's complete alternative, independently of visible columns or rows.
This does not provide selection across separate reader fragments or change
native drag-and-drop export.

The buffer subclass runs GTK's parent selection handlers while temporarily
omitting the raw selection clipboard. Its registration is restored synchronously
for normal TextView replacement/unrealize cleanup. This avoids both raw anchor
exports and replacing GTK's provider after publication, which would clear its
selection. The public [selection clipboard API](https://docs.gtk.org/gtk4/method.TextBuffer.remove_selection_clipboard.html)
is used; no GTK private memory is modified. An immutable GdkContentProvider
advertises UTF-8 plain text. It extracts the frozen range only on a paste request,
through the existing shared two-worker admission limit, capped at 512 KiB with
no partial export. GTK receives bytes through an asynchronous output stream.
There is no polling or extraction on each pointer movement.

Before and after worker extraction, the provider checks the mapped view, original
buffer/presentation, range and current clipboard ownership. Clearing a selection,
editing/replacing its buffer or unmapping the source withdraws its offer. A stale
provider rejects a read. Withdrawal never clears another owner's primary
selection; unrelated layout marks cannot reclaim it. The ordinary clipboard is
independent. The application follows the desktop primary-paste preference rather
than enabling middle-click paste globally.

Focused private Wayland run `text-flow-xqvh2y6a` reads exact text through a separate
`wl-paste --primary` process and performs a real middle-button paste into an
editable native TextView. The fixture enables primary paste only in its private
process, since the installed GNOME schema defaults it off. It verifies Chinese,
emoji, literal U+FFFC, formula/table alternatives, ordinary clipboard preservation,
foreign ownership, unrelated marks, remapping, mutation, replacement, stale reads
and zero output for an oversized aggregate selection. The oversized table
alternative is a synthetic worker-prepared budget fixture. Its native application
log has no GTK criticals; standalone Mutter emits a nonfatal stack-position
assertion around the external clipboard client's popup lifecycle.

Reader run `text-flow-ezmficu3` passes all eleven cases, actual diagram engines,
the real report's first 64 KiB, pointer selection and expanded-cell workflows.
Application run `inline-labels-ixx1w8zx` passes source-aware comments, complete-cell
Copy, responsive tables and the native divider drag. Native tests pass 232 cases
(two normally ignored), and Rust 1.92 checks all targets. These checks leave the
default block reader and installed optimized package at their prior checkpoint.

```bash
python3 scripts/smoke-gnome-text-flow.py \
  /absolute/path/to/target/debug/examples/text_flow_selection --primary-only
```

### Native continuous reader comments

The experimental TextView renderer now accepts document-bound comment options
from the native reader. The header button and context action extract the same Unicode text as
Copy, including complete formula, image and table alternatives. It retains
containing source byte/line ranges, the canonical file/root and immutable index
revision through the existing version-1 document provider. Browser character
offsets remain unset. Embedded table labels and expanded full-cell windows use
the existing source-selection action with their original cell ranges.

Copy and comment extraction share a two-worker, non-queuing admission lane.
Comments retain the existing 16 KiB quote limit; Copy retains its 512 KiB limit.
GTK selection and destination conversation are frozen at action time. Both
before and after worker extraction, the reader checks the mapped widget,
unchanged buffer, generation, revision, Preview mode and clean document state.
The existing shared `CaptureTarget` rejects a conversation switch while work is
pending. Old table/cell actions are also invalid after saving or replacing the
document. Explicit Save remains in the native Comments panel.

The reader adapter registers its projected root with the existing header button;
that target invokes the worker-backed action rather than reading placeholder
characters from the GTK buffer. Ordinary paragraphs and cells register their
actual read-only text leaves. The bounded registry keeps at most 512 weak targets
for the current document generation/revision and prefers a focused selection,
then the most recent native interaction. TextViews use buffer mark notifications;
[GTK4 Labels](https://docs.gtk.org/gtk4/class.Label.html) use native focus and
capture-phase input observation without claiming gestures or polling. Moving
between a selected table cell and the projected document does not reuse the
other widget's older selection. Successful projected capture restores the page
status after an earlier rejected action.

The opt-in anchor discriminator `textKind: "flow"` preserves this renderer's
Copy text instead of reinterpreting legacy Preview or Source quotes. Reopening
uses the existing Rust index worker and snapshot checks. A streaming matcher
bridges separate indexed paragraph/table fragments, retains quote-sized overlap
and source provenance, and requires complete synthetic alternatives. It reuses a
preprocessed memchr pattern and scans in bounded batches. A table or formula
larger than the query breaks the overlap, preventing a match inside an atomic
object. Ambiguous or missing quotes retain the existing unverified-Source
behavior. The browser target normalizer preserves known source text kinds;
opaque source records remain forward-compatible.

Private four-launch Wayland run `run-1z3iu_zy` exercises the actual reader-bound
context action, native Comments Save, Source-button reopen and SQLite restart
round trip. A separate nine-line fixture spans a heading, Unicode paragraph,
inline math, table and footer. Its 65-byte quote retains source lines 1–9;
appending text through Edit/Save changes the revision, and Rust then relocates
the quote across real index fragment boundaries. Mode changes during extraction,
destination conversation changes and actions from the old mapped revision are
rejected. The screenshot was inspected. The same run opens the complete
4,097,226-byte report in the default block reader; this does not measure a
complete-file continuous rendering or a cross-page UI selection.

The complete eleven-case TextView workload in `text-flow-lru2_aff` also passes
physical selection/link/table input, real diagram engines, synthetic image
lifecycle checks and the 2,000-choice column picker. Shared Markdown checks pass
74 default cases and 78 with syntax highlighting, including UTF-8 matches across
64 KiB scan boundaries, final-flush deduplication, atomic skips and bounded
overlap across repeated fragments. Native checks pass 232 cases (two normally
ignored), the gateway suite passes 237 (one normally ignored), and Rust 1.92
checks all native targets. The first parallel gateway run hit the existing
three-second routed-tool test timeout; that unchanged test passes individually
in 0.01 seconds, and the complete suite passes with temporary storage on `/tmp`
and two test threads. No timeout or assertion was relaxed. Browser comment and
document checks pass 81 cases; UI typechecking, generated assets/catalogs,
formatting, Python syntax and diff whitespace checks also pass.

The toolbar increment in `run-451l5vlx` is exercised inside the application's
actual reader scroll container, with the existing header and footer, instead of
an auxiliary window.
It checks the complete 65-byte projected quote, the original line-7 `one` cell,
returning to the projected selection, status recovery and toolbar rejection of
an old mapped revision. The Save/reopen/restart and full-report workflows also
remain part of that four-launch regression. Existing long-paragraph acceptance
(`rich-text-ui-_dne_j39`) and full-cell acceptance (`table-cells-ui-ezyqm96m`)
pass after the registry change, including source reopening, draft retention and
saved/replaced source rejection. Native checks still pass 232 cases and Rust
1.92 checks all native targets. The application fixture activates the real GTK
button; physical pointer/keyboard and accessibility acceptance remain separate.

The continuous reader remains experimental. Cross-page selection, primary
selection, complete table math, remaining styles,
diagram theme/zoom, physical input/accessibility and cold-layout acceptance
still precede promotion. The installed optimized application/gateway pair is
unchanged by this checkpoint.

### Native document and comparison language

Native comparison now follows live interface language changes for Previous/Next
change, return to Reader, the empty/loading/version-change messages, navigation
tooltips and the worker-produced change/count/timing summary. The counts and
timing remain literal variables. Its bounded-alignment detail uses the shared
status binding rather than capturing a translated suffix in the old language.

The same weak, explicitly registered binding supports Markdown page statistics,
unsaved status, loading/save messages and outline line tooltips. Code/text and
Git diff windows translate their line ranges and every combination of binary,
Git truncation and limited line-mapping details. Locale changes update UI text
without replacing the source buffer or rebuilding a document/comparison.
Paths, headings, document content and worker errors remain literal. Explicit
literal assignment also prevents a filename equal to a UI key, such as `Source`,
from being translated; cleared and superseded statuses cannot return after a
language change.

The private four-launch Wayland regression exercises live comparison language
changes with one file, both Preview panes and both Source panes. It checks mode,
file identity, navigation state and unchanged source files. Source coverage checks
all eight status-flag combinations, Unicode selections, byte ranges, an exact
UI-key filename, literal status replacement and clear/reopen. The separate
language/save/restart fixture also checks the dirty Markdown footer. Installed
`--language` acceptance opens comparison through real native controls in both
languages, checks left-document/draft retention and returns through Reader.
These checks do not complete language coverage for other dynamic workspace
dialogs, worker diagnostics or physical keyboard/IME/accessibility acceptance.

Final debug run `run-3lbh3sst` passes the four-launch full-application regression,
including the new comparison and eight Source status combinations. Its real
4,097,226-byte report opened in 1,002 ms with 62 opening heartbeat ticks; the full
workflow sampled native/gateway peaks of 598,332/200,652 KiB. The language
save/restart run `language-namsh8zf` passes dirty Markdown footer translation,
unchanged selections and drafts, explicit queue saving, conflicts, and unknown
preference preservation. Code-edit run `source-edit-r4mfqz0m` passes explicit and
page-change save, undo/redo across save, CRLF preservation, shared read-only
policy, replacement/close guards and conflict Cancel/Overwrite on a private
4,209,198-byte file. Its first save took 293 ms with 22 save heartbeat ticks and
a maximum observed save gap of 52 ms. The 204-test native suite passes with its
two existing runtime-dependent checks ignored; Rust 1.92 checking passes.
These are private debug-workload observations, not release/physical input
latency guarantees. Chinese comparison screenshots were inspected.

The matched optimized package passes installed run `release-ui-geou0_w1`, with
all five optional groups, no application hooks and no gateway override. Actual
native controls verify English/Chinese comparison actions, precise document
statistics (including the final empty line after a trailing newline), the loaded
left file, draft retention and return to Reader. The existing source/edit/save,
four-format diagram inspection, question selection, backend configuration, Git,
table-image handoff and broader viewer checks remain passing. Sampled peaks over
this complete installed workflow were 614,232/78,924 KiB native/gateway. Two lazy
web-content processes were observed only in the browser/EPUB portion; native
Markdown uses GTK widgets. This is local private-desktop evidence; fresh-host,
remote CI, real-provider and physical input/portal acceptance remain open.

- Native SHA-256: `70c890a094ce2f41e2dd38c3ece3446245c0dbf524fd4a9919292739949aac3c`.
- Gateway SHA-256: `4df9f28a2a4cd93633e0edb5c7c309ebaeab1af2301206de11bb467ec3d83855`.

## Native diagram inspection

Document and conversation diagrams now expose **Inspect diagram**, opening a
native libadwaita dialog with Fit, actual size, zoom out/in, scrollbars and Copy.
The shared control applies to Mermaid, PlantUML, D2 and display math. It is
disabled while its source has no rendered texture, including recycled/offscreen
conversation rows, and becomes available again when rendering completes.

The inspector initially shares the existing immutable `GdkTexture`; opening it
performs no image decoding, PNG encoding, GPU readback, diagram-engine work or
pixel copy on GTK. Fit reduces oversized images without magnifying their logical
size. Actual size retains the baseline preview's logical extent; explicit zoom
ranges from 10% to 400%, with native scrolling when the requested size exceeds
the viewport. Copy transfers the complete current rendered texture, including
the higher-resolution vector result, rather than cropping the viewport. It is
separate from the image viewer's original-file Save.

Mermaid, PlantUML, D2 and display math retain an opaque, immutable reference to
their generated SVG, capped at 4 MiB. The existing 16 MiB diagram-cache weight
now includes this vector output. Higher zoom or GTK display scale requests
fresh pixels from the retained vector using the existing resvg/usvg renderer
on Rust workers. The diagram engine is not rerun, and GTK never parses SVG or
enlarges a bitmap to produce the additional detail. File/network SVG resource
resolvers remain disabled; bounded embedded bitmaps use the shared
[resource resolver](#embedded-bitmap-resources-in-svg). Native widgets carry the opaque reference
and completed pixels; source parsing and rasterization stay off the GTK thread.

Requests are debounced for 80 ms and superseded waiters are cancelled. Two vector
raster jobs are admitted globally. A running CPU closure retains its admission
permit even if its GTK waiter is aborted, so replacement/close cannot increase
actual CPU concurrency. Cancellation is checked before parsing, before rendering
and after rendering; a resvg call already running is allowed to finish. Generation
checks reject late results. A logical-size `GdkPaintable` keeps the original
intrinsic dimensions while replacing physical pixels, preserving Fit layout and
scroll geometry as resolution changes. Copy waits for the requested frame.

Recycling a source row or changing its theme does not replace an already-open
inspector's vector snapshot. Closing the dialog cancels queued work and clears
its base texture, vector reference and paintable, even when another client
retains the closed dialog/paintable objects. The source renderer/cache and
clipboard can independently retain references. Each upgraded frame is capped
at 8,192 pixels per edge and 8 megapixels; beyond this budget, magnification can
still become pixelated. These frame/cache/concurrency limits are separate from
transient SVG trees, active worker buffers and GTK/GSK memory, and are not a
total application RAM bound. Tiled viewing and full renderer parity remain work.

The four-format private Wayland fixture opens each actual rendered conversation
diagram through its inspection button, checks actual-size and 400% geometry,
scroll extents, Fit, upgraded raster dimensions, stable logical size, full-frame
Copy, rerasterization after source unload/restoration and pixel release on close,
and captures Fit/zoom screenshots. A worker test uses a subpixel-width stroke
whose recovered opacity/detail at higher resolution cannot be produced by bitmap
enlargement. Invalid scales, cancellation and pixel budgets are also covered.
The installed-release fixture checks the document inspector's actual accessible
GTK actions, upgraded raster dimensions and normal Escape dismissal without
application test hooks. Physical high-DPI, touch/pinch and expanded assistive-
technology acceptance remain separate desktop work.

The final four-launch debug application check passes with all four actual diagram
engines, including the inspector's texture lifecycle assertions and inspected
Fit/zoom screenshots. The real 4,097,226-byte report opened in 905 ms with 56
loading ticks; these are local observations, not release performance guarantees.
The 169-test native suite (two ignored runtime-dependent checks), Rust 1.92 checking, optimized build,
packaged installation and expanded installed-release fixture pass. The installed
fixture uses no application test hooks and confirms the reader inspection button,
native zoom/actual-size/Fit controls and Escape dismissal alongside existing
editing, clipboard, image-save and backend-cleanup workflows. No remote CI or
physical-desktop acceptance is claimed by these checks.

The measured debug 400% upgrades below include the 80 ms debounce, worker
rasterization and installation of the native texture. The 16 ms GTK timer runs
only during this interval; deliberate PNG screenshot readback/encoding is
outside it. These are small synthetic fixtures, not first-compositor-paint or
physical high-DPI benchmarks.

| Diagram | Upgraded pixels | Texture ready | GTK ticks | Longest tick gap |
|---|---:|---:|---:|---:|
| Mermaid | 2,152 × 568 | 120 ms | 7 | 16 ms |
| PlantUML | 1,248 × 1,104 | 140 ms | 8 | 16 ms |
| D2 | 2,328 × 3,603 | 2,875 ms | 173 | 108 ms |
| Math | 2,456 × 756 | 100 ms | 6 | 16 ms |

These original measurements used unoptimized development graphics dependencies.
The stage profile and targeted development optimization below supersede the
slow D2 result; they do not establish uniformly fast zoom for arbitrary diagrams.
The complete four-launch debug workload sampled native/gateway peak RSS of
595,284/201,760 KiB, excluding separate worker peaks. The installed optimized
fixture independently confirms the increased document-diagram raster dimensions
through native accessibility, without application test hooks; its smaller
workflow is not comparable to these debug figures.

The D2 stage benchmark isolates generation, parsing and rasterization without
GTK. For the same 8,912-byte SVG and 2,328 × 3,603 zoom frame, default debug
rasterization took 2,753–2,771 ms while optimized release took 117–120 ms. Repeated
SVG parsing took about 3.4 ms in debug and 0.7 ms in release; D2 generation took
26–31 ms. The measured bottleneck was the unoptimized raster kernel, so no new
parsed-tree cache or reduced image-quality policy is introduced.

The native workspace now builds `resvg`, `tiny-skia` and `tiny-skia-path` at
optimization level 3 in the development profile. Application code retains its
development settings, and release/Tauri/platform profiles are unchanged.
This reusable dependency-level change also serves Mermaid, PlantUML and math.
It reduced D2 development rasterization to 170–176 ms; the preview and zoom pixel
digests exactly match the baseline. In the subsequent four-launch Wayland run,
400% D2 texture readiness improved from 2,875 to 281 ms with 17 GTK ticks and a
16 ms maximum gap. Mermaid/PlantUML/math were ready in 100/120/100 ms, with
18/16/16 ms maximum gaps. Screenshot work remains outside the measured intervals.
The real 4,097,226-byte report opened in 1,059 ms with 64 loading ticks. Sampled
native/gateway peak RSS across the complete debug workload was 582,652/201,556
KiB, excluding worker peaks; these are local observations, not memory caps or
performance guarantees on other systems.

Run the stage benchmark explicitly, then repeat with `--release`:

```bash
STUDIO_GNOME_D2_BIN=/path/to/package/d2 cargo test \
  --manifest-path native/gnome/Cargo.toml --locked \
  diagram::tests::profile_d2_vector_stages -- --exact --ignored --nocapture
```

Diagram inspection buttons now expose format-specific accessible names, such as
“Inspect d2 diagram,” while retaining their compact visual icon. The optional
installed `--all-diagrams` fixture opens four separate files through the actual
desktop entry and selects each format by that accessible name. It verifies 400%
raster dimensions, actual size, Fit, Escape dismissal and composer retention with
the packaged Mermaid/PlantUML/D2/math engines. Its reported action-to-ready time
includes multiple accessibility round trips and is not the stage benchmark.
All four checks pass in the actual optimized installation without application
test hooks. The seven acknowledged zoom actions plus texture readiness took
2.36–2.63 seconds per format, including GTK accessibility activation/animation and
driver round trips. The fixture waits for each changed zoom label before issuing
another activation; repeatedly queueing GTK accessibility activations without
acknowledging their state can coalesce them. Existing editing, explicit saves,
original-image export, draft retention and owned-backend cleanup still pass.
The standard 169-test suite passes, and both normally ignored runtime-dependent
checks were run explicitly: the D2 stage benchmark and real PlantUML/D2
file-import confinement. Rust 1.92 checking and the refreshed release package
also pass. These checks do not replace physical GNOME/IME/high-DPI acceptance.

## Native rich streaming

Normalized assistant deltas now retain the latest rendered Markdown snapshot
while the authoritative text continues accumulating. A selected, verified
ordinary conversation admits a preview request at most once per 150 ms. A
dedicated latest-request slot runs one Rust CPU job at a time, bounded to the
existing approximately 128-KiB presentation excerpt. It does not queue one parse
per token or copy source through another HTTP/JSON route. Previous completed
turns' unfinished previews take priority over a newer continuously changing
reply. Unselected conversations and historical windows do not start preview work;
their live source and existing read-only history behavior remain separate.

This is coalesced whole-snapshot parsing with the existing reusable Rust Markdown
engine, not an incremental lexer. The worker compares immutable display blocks
and shares identical ones. Growing tables can retain reading controls when their
header/source identity remains compatible. Late reference definitions are
reparsed, so a changed rendered link cannot be mistaken for an unchanged source
paragraph. Display allocations are included in the presentation budget; previous
snapshot links are weak and do not retain a chain of complete transcripts.

GTK splices changed gaps between retained block objects. Closed paragraphs,
tables and diagrams keep their widgets, selection, reading position, column
widths, wrapping and active render work while another block changes. Actions
resolve the current message snapshot, so preserving a widget does not preserve
an obsolete favorite/link context. Final item metadata stays authoritative, and
matching prefix blocks survive completion while its remaining text is parsed.
State equality distinguishes the rendered prefix length from the full received
body. Full raw text and source anchors remain in the model; the existing complete
output viewer still handles provider-completed large snapshots.

Backend, socket generation, history generation, thread, item and turn checks
reject late work. A result may enrich a still-matching prefix while newer deltas
remain in the authoritative body; it never replaces those newer bytes or newer
image/output metadata. A started CPU job is allowed to finish with bounded input
rather than becoming detached work on a selection change. Failures expose the bounded
plain text and suppress repeated attempts until that input changes. Valid source
that exceeds Markdown formatting limits now uses the shared
[literal source pages](#native-formatting-limit-source-pages). Output/transport
bounds and broader real-provider validation remain migration gates.

The optional Wayland fixture sends actual notifications from a private Codex
provider through the gateway, with no inference or production credentials:

```bash
python3 scripts/smoke-gnome-streaming.py NATIVE_DEBUG_BINARY GATEWAY_BINARY --headless
```

It checks rich text before completion, 80 deltas in two timed bursts, retained
paragraph selection/scrolling, resized/paged/wrapped table controls, Mermaid and
math images before completion, stable diagram widgets, final-text preservation,
current action snapshots, draft retention and a real queued CPU result after
switching to a second catalog session. Exact viewport coordinates are checked
separately by the ordinary/unbroken paragraph fixtures; those prepare fresh,
value-equal history content to force widget recreation instead of mistaking
successful widget retention for a failed replacement.

The Ubuntu 26.04 Wayland debug fixture committed its first rich rows in 104 ms;
80 timed deltas produced 13 preview requests. The longest measured event/model
handler was 1 ms and the longest list reconciliation was 23 ms. There were 152
heartbeat ticks with a 215 ms maximum gap, including initial GTK layout and
fixture work. Sampled native/gateway peak RSS was 365,020/43,376 KiB. These
measurements describe a synthetic 23,685-byte seed and local provider; they are
not first-paint/network guarantees or proof of stall-free rendering. The active
reading/table screenshot was inspected.

The checkpoint passes 159 native Rust tests (one external-engine test normally
ignored), 24 shared Markdown tests, the native optimized build check, an isolated
Rust 1.92 compatibility check, the
streaming fixture, ordinary/unbroken fresh-presentation reading checks and the
four-launch application regression. The real 4,097,226-byte report opened in
1,064 ms with 65 loading heartbeat ticks. That full workload sampled peak
native/gateway RSS of 506,856/185,692 KiB, excluding external workers. No fresh
release bundle or remote CI run is claimed here. Real-provider and physical
Wayland/IME/accessibility acceptance remain separate migration gates.
The private OpenCode input/reconnect fixture also passes all nine scoped POST
requests, draft retention, lost-ack/recovery behavior and owned-backend cleanup.
This does not change the separately deferred upstream stuck-busy issue.

## Native display math

Reader, comparison and conversation blocks now recognize standalone `$$...$$`
paragraphs and `math`, `latex` or `tex` fences. They render TeX expressions through
[RaTeX 0.1.15](https://github.com/erweixin/RaTeX), using its Rust parser, layout
and embedded-font SVG engine, then the existing Rust rasterizer and GTK image
widgets. No browser, TeX executable or additional download is involved. These
fences accept supported expressions, not complete LaTeX documents. Short native
paragraph labels and ordinary table cells also render inline/mixed expressions,
as described below, including complete formulas inside bounded long-cell excerpts.
Bounded multiline paragraphs now use the native object layout described below.
Long paragraph TextViews now share the formula-aware pages described below;
Mermaid formula labels use the bounded SVG resolver described below.
Source/Edit and source line/byte anchors remain unchanged.

Each cache miss runs a private mode of the native executable before GTK, gateway
or profile initialization. The shared one-job render lane owns cancellation,
theme/generation checks and the 16 MiB raster cache. Limits are 16 KiB formula
source, 2 MiB SVG, two CPU seconds, a three-second parent timeout and 256 MiB
worker data; core dumps are disabled. The parent clears inherited environment
variables. Unsupported commands and excessive expansion/depth produce visible
errors with the original Source available. RaTeX has no TeX file/shell loader;
the rasterizer retains disabled file/data image resolvers. This is a resource
limited Rust subprocess, not a separate filesystem sandbox. Embedded KaTeX fonts
carry MIT engine and SIL OFL notices in `licenses/ratex`; CJK fallback uses system
fonts, included by the development dependency installer.

The reproducible [two-engine benchmark](../benchmarks/native-math/README.md)
compares equal display style and nominal 24 CSS-pixel fonts. On Ubuntu 26.04,
Rust 1.97 debug builds without debuginfo, fourteen synthetic engineering examples
gave the following results. Cold/warm measurements are formula-to-SVG only;
they exclude GTK, process launch, transport and painting.

| Measurement | RaTeX 0.1.15 | LaTeX-Rust 2.1.1 |
|---|---:|---:|
| Valid examples rendered | 14/14 | 12/14 |
| Median cold time | 3.771 ms | 62.038 ms |
| Median warm time, 20 iterations | 0.448 ms | 0.369 ms |
| Maximum process peak RSS | 32,760 KiB | 11,276 KiB |

RaTeX handles the tested chemistry and Chinese-text examples that LaTeX-Rust
rejects. Both reject the five invalid/file/command/recursive/deep cases. These
measurements support RaTeX for cold latency and this sample's coverage, with
higher memory and no warm-speed advantage. They do not establish full TeX
compatibility or visual equivalence. LaTeX-Rust 3.0.x requires Rust 1.99 and was
excluded rather than raising this application's Rust 1.92 floor. The pinned
native dependency graph also passes an actual Rust 1.92 check.

The native suite passes 155 tests (one external-engine test normally ignored),
and the shared Markdown crate passes 22 tests. The private worker fixture checks
effective Linux limits, outlined glyphs, light/dark changes, malformed/oversized
input, rejected file/shell/image commands and absence of user-profile creation:

```bash
python3 scripts/smoke-gnome-math.py /path/to/codex-thread-studio-gnome
```

The actual Wayland regression renders reader and chat formulas, recolors them
on theme changes, preserves Source/Edit text and shows invalid-formula errors.
Reader dark-theme and chat screenshots were inspected. The unrelated real
4,097,226-byte report also remains covered; it contains no matching math syntax,
so it is not formula-coverage evidence. The final four-launch run opened its
bounded reader view in 1,060 ms with 64 GTK timer ticks during loading; the three
reader formulas produced native textures in 313 ms with 15 timer ticks. These are
debug workload observations, not optimized startup benchmarks or proof that no
individual frame stalls. Restart persistence, chat and owned-backend cleanup also
passed. This full workload sampled peak native/gateway RSS of 509,304/170,672 KiB;
these parent-process observations exclude render-worker peaks and are not total
application memory or a controlled before/after comparison. The normally ignored
PlantUML/D2 rendering/private-file isolation test
was run separately and passed after extracting their shared process transport.
CI now checks the private worker and
packaged notices, but that workflow has not run remotely. An optimized build
check passes; no fresh release bundle is included in this checkpoint. Full inline
math and the rest of the native application migration remain in progress.

### Math inside Mermaid labels

The following describes the earlier Pele checkpoint and its recorded evidence.
The current GTK queue uses the isolated Merman worker and its native RaTeX path;
see [earlier integrated candidate](#earlier-flow-prefix-candidate) and the
[worker contract](../native/mermaid-worker/README.md#process-contract).


The existing pinned Pele 1.9.0 bundle exposes a math resolver for `$$...$$`
labels. The native worker now connects that hook to the existing isolated RaTeX
subprocess, returning self-contained glyph-outline SVG and pixel dimensions.
Pele sizes nodes, edges and sequence labels around those dimensions; usvg/resvg
and GTK retain the normal raster/vector-inspection path. No WebView, MathML,
browser measurement, network runtime or new dependency is introduced. The
upstream contracts are documented by [Pele](https://pele.run/api#math) and
[Mermaid](https://mermaid.js.org/config/math).

The resolver uses display-style math, scales the native 24-unit coordinate system
to the requested label font size, and replaces pt-sized envelopes with pixel-sized
SVG envelopes. Exact TeX stays in the original diagram for Source/Copy. The
diagram's theme is passed to the math worker. Unsupported expressions return no
markup, allowing Pele to retain escaped literal TeX rather than dropping the
label. This supports RaTeX's expression subset, not every KaTeX extension.

Each diagram admits at most 32 distinct formula sources and 2 MiB of retained
outline data. Equal sources reuse the immutable generated outline across label
font sizes; failed expressions are remembered within the same render as well.
Formulas retain the existing 16-KiB source bound; font sizes must be finite,
positive and at most 256 pixels, with bounded dimensions. Final diagram SVG,
QuickJS memory, raster pixels and render-queue limits remain in force. The
Mermaid three-second deadline is also passed as the math subprocess's remaining
time, with cancellation/deadline checks before dispatch and after completion.
Cached results cannot bypass cancellation. Work exhaustion fails the diagram
through its existing Retry/Source controls.

The real reader acceptance uses formula-bearing flowchart node and edge labels,
sequence participants, messages and a note. It checks three flowchart SVG math
occurrences and seven sequence occurrences: participants are repeated at the
top and bottom. An unsupported macro remains readable in the flowchart. Native
rasters, original-source Copy, opposite-theme replay, lazy visibility, native
diagram controls and Source cleanup use the ordinary application machinery.
Conversation acceptance also verifies two native formula labels and exercises
the same chart's Source control, vector inspector, zoom/Fit/Copy and cleanup.
The same hook serves comparison diagrams; separate acceptance
for every supported Pele diagram family and advanced formula styling remains.

Final four-launch Wayland application run `run-h3gcbssk` passes reader, chat,
comparison, paged Edit/Save, terminal and restart coverage. Its copied
8,340,599-byte report opens in 1,309 ms with 80 opening timer ticks. Formula
captures were inspected in dark/light themes; seven sequence occurrences include
the repeated participant headers. The rebuilt standalone text-flow regression
`text-flow-y_txbvcr` passes all 11 cases. All 286 native tests pass (two ignored),
including six new resolver/deadline tests, with Rust 1.92 and release all-target
checks. The first GUI probe's five-versus-seven sequence assertion and the initial
missing shared-module import in a standalone example are recorded as development
failures; both are corrected. Evidence, source/binary hashes and unchanged
installed-package hashes are retained in
`/media/rui/studio-gnome-deps/qa-artifacts/native-smoke-jwfgtzto/mermaid-math-validation/VALIDATED.json`.
The installed release and selectable-renderer promotion status remain unchanged.

### Native inline math

The shared Rust parser now records inline/mixed-math candidates without changing
their displayed text. Each candidate has original source line/byte ranges,
rendered UTF-8 byte ranges, Unicode scalar ranges, formula-body ranges and its
text/display style. At most 256 candidates per rich-text value and 16 KiB per
formula are recorded; excess candidates remain literal source. Cache accounting
includes the metadata. Tests cover repeated identical formulas, entity decoding,
Chinese/emoji, CRLF, NUL normalization, tables and nonzero fragment offsets.
Ambiguous currency closers followed immediately by a digit, such as in
`$5 and $10`, remain literal text.

The existing resource-limited math worker now also supports compact inline SVG
and returns bounded em/baseline metrics through the raster result. Text-sized
operators and display-style operators use separate internal render kinds/cache
keys. Existing standalone formulas retain their display sizing. Invalid modes,
metrics and oversized formulas fail without creating a gateway or user profile.

Production short Markdown labels and ordinary table cells use Pango's
[shape attributes](https://docs.gtk.org/Pango/struct.AttrShape.html) and
[Cairo shape callback](https://docs.gtk.org/PangoCairo/func.context_set_shape_renderer.html).
It draws the formula on the opening delimiter and assigns zero-width shapes to
the remaining TeX characters. The original label text, character offsets and
native selection/copy machinery stay intact; no object-placeholder characters
are inserted. Pixel conversion and formula layout run on a Rust worker. GTK
keeps text reflow, baseline placement, selection and painting. Conversation,
reader and comparison content share this label path. Complete formulas in
bounded table excerpts use it too, as described below. Expanded math cells now
use bounded label pages as described below. Paged paragraph TextViews now use
the shared object renderer described under [paragraph math](#math-across-paragraph-pages).

Labels use the existing Rust diagram lane/cache, not another parser or rendering
service. At most four requests/conversions are admitted concurrently, with a
128-label pending queue and round-robin formula admission. Each formula raster
is limited to 2 MiB and each label group to 4 MiB. A process-wide 16 MiB budget
tracks Cairo pixel allocations until their last Cairo reference is released,
including retained snapshots; it is separate from the existing 16 MiB backend
raster cache. These are allocation bounds, not a total application/GSK memory
limit. Admission failures leave the original source readable with an explanatory
tooltip.

Unmapping, label reuse, view switches and theme changes cancel old work and
release label-owned surfaces. Generation checks reject late conversions even
when a label is reused for identical text. Cancelled conversions retain their
admission slots until they finish. A weak widget paintable observes layout-size
changes, so typography updates recompute shape metrics without a permanent
frame callback; existing Pango attributes and text selection are preserved.
Formula font size comes from the actual Pango text run, including headings.

```bash
python3 scripts/smoke-gnome-inline-labels.py /path/to/native /path/to/gateway
```

This private Wayland/Mutter fixture exercises the production queue and mapped
chat/reader/table labels. An independent compositor clipboard reader verifies
the native Copy action for a whole paragraph and a selected formula. It also
checks theme changes, font resizing with selection retained, original currency
and unsupported-command text, local-link dispatch, Source/Preview switches,
draft retention, source-file immutability and owned-provider cleanup. The native
reader comment action captures the exact selected TeX and original CRLF source
line. A deliberately late pixel conversion is rejected across equal-text reuse;
unmap/remap releases and restores the surfaces. Light conversation and dark
split reader screenshots were inspected. Capture waits for a real mapped
content node before adding its opaque background.

The production fixture's debug run reached mapped conversation formulas in
820 ms, including initial gateway/history setup; sampled native/gateway peak RSS
was 356,032/43,764 KiB, excluding worker peaks. These small-fixture measurements
are not a dense-math benchmark or an optimized startup comparison. The initial
inline-math checkpoint passed 31 shared Markdown tests and 161 native tests,
with one normally ignored. Optimized-profile checking and the private worker
limits/errors fixture pass. The existing four-launch application regression also
passes. A current optimized bundle and installed acceptance are recorded under
[native release validation](#native-release-validation).

A standalone layout probe remains available:

```bash
cargo build --manifest-path native/gnome/Cargo.toml --example inline_math_shape --locked
python3 scripts/smoke-gnome-inline-shapes.py /path/to/target/debug/examples/inline_math_shape
```

The owned Wayland/Mutter experiment renders three formulas, including a red
formula, in light/dark themes. An independent compositor clipboard reader checks
GTK's copy action for the whole paragraph and each original TeX expression.
Native links still activate; narrowing the window reflows from two to four lines
without changing the text. Both full-window screenshots were inspected. The
experiment and worker compile with Rust 1.92. The production path reuses its
shape renderer; the probe is still useful for isolated reflow and clipboard
checks.

Bounded multiline paragraphs now have a [native object layout](#bounded-multiline-math-paragraphs).
Dense-math performance, bidi, formula search/selection
across all panes and physical accessibility/input need further acceptance.
The shape callback's bounding-box path is not a glyph-outline print/export
implementation. Mermaid labels now share the bounded resolver above. This checkpoint
does not complete native application parity.

### Math in native table excerpts

The Rust cell model now prepares indices for complete math spans inside its
existing 256-byte, UTF-8-safe preview. The indices refer to the shared full
content; formula bodies, original byte/line ranges and Unicode offsets are not
cloned or rebased. A formula ending exactly at the prefix boundary is eligible;
one crossing it stays literal. Literal Source content remains unformatted.
The additional index capacity participates in the existing block/cache weight.

The shared cell widget decorates that exact prefix through the existing native
math queue, raster cache and Pango/Cairo renderer. The label retains its original
TeX, native ellipsization and bounded row height. It does not parse a truncated
formula or clone the complete long cell on the GTK thread. Ordinary and
experimental continuous tables use the same implementation. Read full cell
and complete-cell Copy retain the full shared value, including text beyond the
excerpt. The original excerpt checkpoint left paged full cells as literal TeX;
the formatted cell reader below now covers complete single-line formulas.

The extended private Wayland inline-label fixture checks light/dark excerpt
formulas, exact native Copy through an independent compositor reader, original
CRLF source line 14, clipped-formula exclusion, full-cell Copy, equal-text reuse,
late-conversion rejection and unmap cleanup. It mounts the actual continuous
table adapter inside the reader, scrolls the inner ColumnView to realize both
tested rows and verifies that the formula remains source-linked. The original
fixture file and unsent chat draft remain unchanged. These are small math
fixtures; the complete 4.1 MB report uses its separate application regression.

Validated run `inline-labels-4wtx9bhv` includes an explicit formula-glyph check
inside the bounded continuous-table viewport; light/dark and continuous-table
screenshots were inspected. The fixture clears selection before selecting an
ellipsized label again: GTK treats the ellipsis as an atomic range and toggles
an already selected endpoint ([GTK Label implementation](https://github.com/GNOME/gtk/blob/4.22.0/gtk/gtklabel.c)).
The clipboard assertions still require exact original text. The full-application
four-launch run `run-kzl6uneq` opens the complete 4,097,226-byte report and passes
source/edit/save/conflict, column/paging and comment Save/reopen/restart checks.
Shared Markdown tests pass 75 default cases and 79 with syntax highlighting;
native tests pass 232 (two normally ignored), and Rust 1.92 checks all native
targets. Formatting, Python syntax and diff whitespace checks pass. The installed
optimized pair remains at its previous validated checkpoint. Responsive table
widths are covered above. That historical checkpoint left full-cell/header math
open; the bounded formatted reader below now covers those reading windows.
Broader layout remains parity work; the small-fixture result is not complete-file
continuous rendering evidence.

### Math in expanded cells and header readers

Native **Read full cell** and **Read column header** now use bounded formatted
label pages when their shared rich content has math and prepared attributes.
Text requiring literal carriage returns uses the exact-text layout below.
This applies to conversation/document tables and the continuous-reader adapter.
Short math cells keep the existing label path; non-math and literal Source cells
keep their styled TextView pages. GTK's column titles remain bounded plain-text
previews; the header's native menu provides the complete formatted reading view.

The reusable Rust `rich_pages` model consumes the parser's existing Unicode
style/link index and formula ranges. It does not parse Markdown again. Ordinary
pages contain at most 4 KiB, preferring whitespace and retaining every character
of prepared plain text. A formula crossing the boundary moves intact to the next
page; a
single longer formula gets its own page, bounded by the existing 16 KiB body
limit and delimiters. Pango markup is escaped on the worker, complete math ranges
are rebased to the page, and their original source byte/line ranges are retained.
Preparation accepts at most 512 KiB of formatted cell text and 4 MiB of generated
markup in total, with a separate 64 KiB markup cap per label page so repeated
long URLs cannot inflate GTK parsing work. Two Rust preparation workers are
admitted without a pending queue.

Only the current label is mounted. It uses the existing native formula lane,
Cairo budget and theme/typography handling. Changing a page unmaps the old label
and releases its surfaces. Selection and native Copy keep the original
TeX; the title-bar Copy still exports the complete cell. New page labels register
with the existing source-cell selection/comment adapter. Closing before work
starts skips preparation; replies for an unmapped window are ignored. Formatting
budget/admission errors leave the existing paged TextView readable, with the
reason in its tooltip. No document, session, index or save policy changes.

Private Wayland run `inline-labels-php5fmde` verifies both themes, a formula placed
across the 4 KiB boundary, first/next/last pages, exact independent clipboard
bytes, styled links, complete-cell Copy, source-line-14 cell comments and
source-line-11 header comments. It exercises the actual native header action,
closing/reopening, close before preparation and surface release. Screenshots of
first/last/header pages were inspected; the final formula is explicitly scrolled
into view. The original CRLF file and unsent draft stay unchanged, no model turn
starts, and private providers exit. The body-cell workflow observed its first
formula at 107 ms and 50 heartbeat ticks over 1,658 ms; the maximum gap was 155 ms,
including screenshots. These debug observations are not dense-math or release
performance guarantees.

Shared Markdown checks pass 80 default cases and 84 with syntax highlighting;
native library checks pass 232 cases (two normally ignored). The associated
eleven-case reader run `text-flow-6fap5odb` retains real diagram engines, report
prefix content, physical selection and shared full-cell lifecycle checks.

At this expanded-cell checkpoint, paragraph math and broader input acceptance
remained open. Bounded and paged paragraph math are implemented below;
dense-math/bidi/accessibility and physical desktop acceptance remain.
The installed optimized package remains at its prior validated checkpoint.

### Multiline math layout and source fidelity

The inline-shape probe now includes LF, CRLF and a formula followed by an outside
paragraph break. On GTK 4.22.4/Pango 1.57.0, ordinary shape attributes retain the
formula's hard line break: the following word moves to another line. GTK's public
single-line mode removes that break, but also collapses the outside paragraph
break. The outside-break case changes from three lines to one. This is diagnostic
evidence, not a multiline-rendering implementation or a promotion gate passed.
The [layout returned by GtkLabel](https://docs.gtk.org/gtk4/method.Label.get_layout.html)
is owned by GTK and considered read-only; the implementation does not rewrite
its text or layout behind the widget. Pango's
[single-paragraph mode](https://docs.gtk.org/Pango/method.Layout.set_single_paragraph_mode.html)
applies to the complete layout rather than individual formula spans.

The probe also exposed a source-fidelity defect: a raw markup label normalizes
CRLF inside a retained formula to LF. Its copied quote then differs from the
original text, and subsequent Unicode/math/link offsets can shift. Encoding the
carriage return as an XML character reference preserves it through a direct
Pango parse, but did not preserve it through GtkLabel's markup path in the probe.
That attempted workaround is not used in production.

Rust now prepares inline attributes for any rich text retaining a carriage
return, including short paragraphs. The existing attributes carry a worker-set
`carriage_returns` flag; GTK does not rescan a large string to decide its layout.
The shared native formatter uses its exact-text TextView with the same prepared
styles, links and source mappings. Short CR-bearing paragraphs use an 80-pixel
minimum viewport rather than the long-paragraph 320-pixel minimum. Formatted
cell/header readers retain this exact-text path, and the reusable label-page
builder rejects carriage returns instead of silently normalizing them.

Private probe `inline-shapes-xpc9ly9y` records the layout limitation, validates
exact native TextView Copy and preserves the existing single-line math, native
link and reflow checks. Application run `inline-labels-p3_6nx5b` opens a private
CRLF document through the real reader. Whole-paragraph and selected-formula Copy
retain exact CRLF/Unicode bytes through the independent compositor reader. Bold
text, the link after the formula, compact layout and source-line-3–4 comments
pass. The native link action opens the correct local document. Both fixture
files and the unsent draft remain unchanged; no model turn starts, and owned
providers exit. The native screenshot was inspected. Worker tests verify that
the original CRLF quote reopens against freshly parsed Preview blocks while an
LF-normalized quote is missing, preserving the original source-line range.

Shared Markdown checks pass 82 default cases and 86 with syntax highlighting.
Native library tests pass 232 cases (two normally ignored), Rust 1.92 checks all
native targets, and the gateway checks all targets without desktop features.
Formatting, catalog consistency, Python syntax and diff checks pass.
This checkpoint preserves source text; the following increment adds bounded
multiline formula painting. Paged paragraph math, dense-math/bidi and physical
input/accessibility acceptance remain open. The installed optimized package is
unchanged.

### Bounded multiline math paragraphs

The shared native formatter now uses its text-flow child-anchor model for
multiline formulas in a rich paragraph of at most 8 KiB. Short single-line math
keeps its existing label path. This promotes only the bounded paragraph adapter;
the whole-fragment continuous reader remains experimental. Conversation,
document and comparison formatters share this widget.

Rust workers consume already-parsed rich text, retaining styles, links and
original TeX. A direct map connects projected Unicode positions to original
UTF-8/scalar boundaries. A formula is one atomic native child anchor; a literal
U+FFFC in the document remains literal text. Formula-internal LF/CRLF stays in
the immutable original alternative, while outside paragraph breaks remain in
the GTK buffer. Search matches inside a formula expand to the complete object.
Source comments retain the containing document generation/revision and original
line ranges rather than inventing source locations for the projected buffer.

Preparation admits at most 128 pending requests and two active CPU workers.
Cancellation is checked before worker execution, and mapped/epoch guards reject
late results. Each paragraph admits 32 objects and 32 MiB of prepared rasters;
each math texture is at most 2 MiB. A separate 32 MiB live-texture charge follows
the actual GBytes lifetime, including retained texture snapshots/slices. Failed
or over-budget preparation leaves the exact styled source readable. Existing
parent image previews remain responsible for images; the projection preserves
their alt text without starting a second image renderer.

Native math widgets supply measured width, height and baseline. Map/content-size
notifications track the actual text font; content invalidation handles font
changes that leave a compact paragraph's allocation unchanged. The font value
is cached, so selection/cursor paints do not revisit every formula. This uses
GTK layout and public widget/signal APIs, without rewriting a Pango layout or
running a permanent frame poll.

Copy and primary selection expand objects back to original text. Selected-text
dragging supplies a text content provider instead of GTK's raw anchor buffer;
the scoped gesture adapter preserves ordinary click, word/line selection and
new selection drags. Theme replacement restores selections/reading positions in
original coordinates and preserves a foreign primary owner. Dynamic selection
menus follow the current child; closed/replaced views cannot capture text.

Application run `inline-labels-wussob9m` verifies actual conversation and reader
multiline objects, independent native Copy, selected-formula source comments on
lines 3–4, styles and link navigation. A separate paragraph in the same private
window verifies an outside break, a literal U+FFFC, source search,
dynamic capture, light/dark replacement, foreign primary ownership and exact
native drag/drop into an editable TextView. Real clicks exercise clearing an old
selection, double-click words, triple-click lines and a fresh selection drag
across the formula. Font growth/restoration retains selection; rapid unmap/remap
rejects stale capture and leaves only the current view. Native screenshots were
inspected. No model turn starts, original files/draft stay intact, and owned
providers exit.

The interaction fixture caught two implementation defects before this checkpoint:
GTK's default drag exported an anchor buffer without the formula, and size-only
observation missed a font change within an unchanged compact allocation. The
scoped text-export gesture and cached content observer address those paths.
The standalone probe `inline-shapes-oenuakaj` now waits for the actual rendered
CRLF object before checking exact Copy and single-line layout; it no longer
accepts the temporary literal-source child as production rendering evidence.

Shared parser suites pass 84 default cases and 88 with syntax highlighting;
native tests pass 233 cases (two normally ignored). Worker mapping tests cover
every projected selection and partial-formula match expansion. A lifetime test verifies
that sliced GBytes retain their original pixel charge until the final slice is
dropped. Rust 1.92 checks all native targets, and all gateway targets check with
desktop features disabled. Existing reading-position regression
`rich-text-ui-o0gb0prw`, eleven-case diagram/text-flow run `text-flow-ee9zt_zc`
and four-launch real 4,097,226-byte report run `run-bfcy8k5f` pass. The diagram
probe uses only the report's first 64 KiB; the application regression uses the
complete report. These are debug acceptance runs, not optimized startup or
dense-math performance measurements.

This was the initial bounded-paragraph checkpoint. The following increment
extends it to longer paragraphs. Dense-math throughput, bidi, screen readers,
touch, fractional scaling and physical desktop acceptance remain open. The
installed optimized package is unchanged.

### Math across paragraph pages

Long rich paragraphs now share the same native object renderer as short
multiline formulas. Prepared math also renders when short text exceeds the
markup budget or retains carriage returns outside a formula. Ordinary short
labels keep their existing presentation. Conversations, historical context,
Favorites, reader/comparison paragraphs and formatted cell fallbacks share this
formatter; application-level file Source/Edit remains separate.

The Rust parser now chooses formula-aware boundaries for its existing indexed
pages. Pages normally stay within 8 KiB and never split a retained formula or
CRLF pair. One expression can occupy a dedicated page up to the existing 16 KiB
formula-source limit plus delimiters. Unsupported/excessive rendering leaves
the exact page text available. Such an exceptional source page uses horizontal
scrolling rather than wrapping an unusually long TeX line in GTK. The original
Markdown is never rewritten to create these display pages.

The reusable `rich_page` module consumes the parser's indexed attributes on Rust
workers. It copies only the requested page, clips indexed styles/links and
rebases display offsets while keeping each formula's original file coordinates.
The GTK root retains shared attributes and bounded math metadata, avoiding a
whole-paragraph text/markup clone. Existing two-worker/128-request admission,
render limits and pixel accounting remain in force. Page changes cancel old
generations; only the current mapped owner can commit output or reading state.
The persistent native toolbar uses first/previous/next/last actions and live
English/Chinese status labels.

Search coordinates remain relative to the original paragraph. The native
adapter converts them through the page start and object map; a partial formula
match selects the whole expression. Selection/reading restoration retains the
original page and scalar range through theme changes, Source/Preview and equal
history replacement. Temporary plain views preserve a deferred source selection
without claiming primary ownership; real input supersedes it. Discarded widget
callbacks cannot reclaim the replacement's state. Selection translation receives
the original text and its native menu stays available beside link/comment actions.

Inline styles now reuse at most four GTK tags per buffer instead of allocating
a tag for every emphasis span. Bold keeps priority over configured inline-code
weight. This is shared with the experimental continuous view. A markup-heavy
paragraph with 1,200 emphasis spans renders its formula using only two native
tags and retains the full 320-pixel reading viewport. Dense range
application/layout remains a performance gate; a small tag
table does not eliminate that work.

```bash
python3 scripts/smoke-gnome-inline-labels.py NATIVE GATEWAY --paged-math
```

This owned Wayland/fake-provider fixture checks actual conversation/reader
controls, every page's independent Copy, partial-formula source search,
line-3–4 comments, rebased local links, rapid page changes, obsolete actions,
theme and language changes, equal-history replacement and Source/Preview.
It also checks exact Copy after a large unsupported expression fails rendering,
and the markup-heavy style case. Original files and the unsent composer draft
remain unchanged; no model turn starts. Screenshots and timing/memory evidence
are retained with the validation checkpoint. The wider native migration and
installed/physical acceptance gates remain open.

Run `inline-labels-5mfsw9pp` passes this complete workflow, including independent
primary reads during both Source/Preview round trips. It mapped the first chat
page in 785 ms including provider/history setup and the first reader page in
215 ms after that work. Subsequent reader page allocations took 68/34/50 ms.
The oversized-source phase's maximum main-loop gap was 39 ms; startup,
interactive checks and the dense-style case reached 436/302/341 ms respectively.
Sampled native/gateway peak RSS was 390,040/45,264 KiB, excluding render-worker
peaks. These debug measurements include fixture work and screenshots; they are
not cold-release benchmarks or a general responsiveness guarantee. A preliminary
wrapped-source run recorded a 1,083-ms gap and motivated the exceptional
horizontal-scroll path. Dense styled layout still needs improvement.

Shared Markdown tests pass 86 default cases and 90 with syntax highlighting;
native tests pass 233 cases with two normally ignored. Rust 1.92 checks all
native targets, and all gateway targets check with desktop features disabled.
Formatting, catalog, Python syntax and diff checks pass. Related Wayland runs
`inline-labels-yujg79e7`, `rich-text-ui-uvglg48_`, `text-flow-npbz7g4a` and
`inline-shapes-kpobxitf` retain inline/table/dragging, reading-position,
eleven-case diagram/text-flow and layout coverage. Full application runs
`run-5z1eo0ko` (protocol logging) and `run-7a1ln2db` (ordinary settings) complete
the four-launch workflow against the entire 4,097,226-byte report, with owned
providers reaped. The diagram probe uses only its first 64 KiB.

The ordinary full-app run still records an intermittent GTK warning about
snapshotting an unallocated GtkGizmo. Earlier `run-0wygo216` lost its Wayland
connection after that warning. A fatal-warning GDB run (`run-t55hh7kv`) now
identifies the widget as the vertical scrollbar trough beside the reader's
GtkSourceView, with the document stack in Source mode. The stack is GTK's normal
frame-clock paint, rather than a direct screenshot call. Removing forced layout
from captures does not eliminate the warning; disabling overlay scrolling and
reserving the vertical scrollbar also failed and those experiments were removed.
[GNOME's discussion](https://discourse.gnome.org/t/warning-trying-to-snapshot-gtkgizmo/19261/8)
describes a similar first-appearance scrollbar warning, but a standalone SourceView
probe with synthetic text and the same report copy did not reproduce it. Neither
an upstream cause nor a connection to the earlier disconnect is established.

Debug screenshot helpers now observe mapped widgets with WidgetPaintable and
wait for actual frame-clock painting. Private workflows register a weak root
observer before the first normal frame, including document-only tests. They no
longer allocate descendants or
emit frame-clock signals. Observers hold widgets weakly and release their cached
nodes on unmap. Missing content or an unmapped dialog fails the workflow instead
of manufacturing layout for a screenshot. Chat, diagram, reader and immediate
dialog checks now wait for a real frame. This follows GTK's
[WidgetPaintable](https://docs.gtk.org/gtk4/class.WidgetPaintable.html) and
[frame-clock](https://docs.gtk.org/gdk4/class.FrameClock.html) contracts; it changes
debug validation, not the installed release's rendering path.

Full-app run `run-9s7p8y7z` passes the four-launch workflow against the entire
4,097,226-byte report with the natural-frame helpers, including table controls,
source lines, editing/save conflicts, all four diagram engines, queue editing,
commands, settings and restart checks. Reader and queue-editor screenshots were
inspected. Initial reader metadata/model readiness is 1,072 ms with 65 main-loop
ticks; this does not measure first painted content. Sampled native/gateway peaks
are 562,260/185,316 KiB during this debug screenshot workflow, excluding diagram
workers. These are diagnostic observations, not optimized performance claims.
The run still contains one scrollbar warning and no application Wayland failure.
Final follow-up `run-mbkea7q5`, after root observation was moved to the common
private-workflow entry point, also passes all four launches without application
GTK warnings. Because reproduction is intermittent, this does not establish a
scrollbar fix; the warning/disconnect diagnostic remains open. The installed optimized package
is unchanged and the full native migration remains active.

Related natural-frame runs `inline-labels-usz96_4i` (paged paragraphs),
`inline-labels-udoq7at3` (inline/table math and native pane dragging) and
`source-edit-ca0i2kut` (4,209,198-byte source editing) pass without application
GTK warnings. The source-only case covers exact CRLF/page boundaries, dirty
view switches, explicit/page-change saves, conflicts, undo/redo and close guards;
it does not depend on the conversation test registering a screenshot observer.
These workflows send no real model prompts and reap their private providers.

## Native long Markdown table cells

Row pagination alone does not bound the layout cost of a very long cell. Native
reader and conversation tables now share a cell widget with worker-prepared
text excerpts: at most 256 UTF-8 bytes plus an ellipsis, displayed in at most
five lines. **Read full cell** appears above the excerpt, so it stays reachable
inside a short table viewport. Ordinary cells retain inline formatting and links.
The rule also catches markup-heavy cells whose plain text is small. Column/menu
labels are bounded; original headings remain in Markdown Source.

The explicit native full-cell window retains complete rendered text, source line,
selection, Copy and validated links. Long or markup-heavy non-math text reuses
the existing worker-prepared styled pages, at most 8 KiB per GTK TextBuffer.
Formatted math cells use the bounded label pages described above. The page
controls stay above a single expanding text scroller; shorter cells use a
scrollable selectable label. This does not change the document index, original
source, save behavior, table row/column identities or document search corpus.
The window also has full-cell Find, described below. Native cell comments retain the source-cell identity
when capturing selections from either page presentation.

Cells hold immutable reference-counted rich content, so row projection/binding
does not clone a long cell's complete text on the GTK thread. Excerpt storage and
rich allocations are counted by the presentation budget. The window is modal and
owned by its source row: unmapping/replacing the row closes it, preventing stale
relative links after a file/session change. Repeated open/close also handles a
retained GTK object whose native window has already been destroyed.

The optional private fixture uses an 87,760-byte Markdown table with a long
unbroken Unicode cell and 116 rows, across the actual conversation and reader:

```bash
python3 scripts/smoke-gnome-table-cells.py NATIVE_DEBUG_BINARY GATEWAY_BINARY --headless
```

It checks bounded excerpts, table page changes, full-cell first/last pages,
styled tags, literal HTML, exact Unicode selection, actual on-screen tail
visibility, reopening, source-row unmapping and relative-link navigation. It
uses private profiles/providers, writes no user document and sends no model
turn. Screenshots of the table and full-cell views are inspected separately.
Physical desktop clipboard/IME/accessibility acceptance remains outstanding.

On Ubuntu 26.04 GNOME Wayland, the final debug fixture mapped the chat table in
500 ms and the reader table in 182 ms. The first full styled cell took 622 ms,
and the subsequent reader cell took 48 ms; 63 GTK heartbeat ticks ran, with a
584 ms maximum gap. This exposes a remaining first-use text-layout/font cost;
bounded content is not a claim of stall-free rendering. Sampled peak native and
gateway RSS was 345,520/44,264 KiB. These observations include private provider
startup and fixture work, not a controlled before/after benchmark or total
desktop memory measurement.

The native suite passes 155 tests (one external-engine test normally ignored),
and the shared Markdown suite passes 24. Unit coverage includes unchanged
Unicode/CRLF source ranges, shared full-cell ownership and short but markup-heavy
cells. The four-launch full application regression also passes table wrapping,
column selection, stable paging, Source/Edit/save conflicts, diagrams and restart
state. Its real 4,097,226-byte report opened in 952 ms with 57 loading heartbeat
ticks; sampled native/gateway peak RSS was 516,816/202,068 KiB, excluding external
workers. An optimized native build check passes. No fresh release bundle or
remote CI execution is claimed by this checkpoint.

### Native full-cell Find

Expanded cell and column-header windows share a native Find control, Ctrl+F,
previous/next matches, case sensitivity, F3/Shift+F3 and Escape. The hidden search
bar takes no reading space. Find searches the complete original plain cell text,
including the TeX source of formulas, and opens the containing text page. Literal
spaces, Unicode and CRLF scalar positions are retained. Selection of a partial
formula expands to the complete formula in the formatted label; a match spanning
pages selects its visible portion and retains the existing continuation notice.
Short formatted labels use the same control. Query entry keeps keyboard focus
while Find is open; closing it returns focus to the selected cell text so Ctrl+C
copies that original selection. Find reveals through both the text scroller and
any containing image scroller.

The implementation reuses Source Find's literal Unicode-aware Rust regex engine.
Two admitted workers search immutable cell content, with a 120-ms debounce,
4-KiB query limit, 8-MiB text limit and 1,000 returned matches. Limits are visible;
the count marks truncated results. Enter can retry a failed/busy search. Page
character ranges are prepared with formatted pages on the worker, avoiding a
whole-cell GTK character scan. GTK handles bounded-page selection, layout and
scrolling. Query generations and cancellation discard superseded results;
closing the cell or unmapping its owner cancels work. Search has no model calls,
file writes or persistent state. Existing source comments and complete-cell Copy
keep their original payloads.

The private table-cell fixture now exercises Find in short labels and the long
styled/image cells in both chat and reader, including visible last-page matches,
no-result/case/space behavior, match navigation, the result cap and live language
changes. The inline-label fixture checks partial-formula matches, real owned
Wayland Ctrl+F/F3/Shift+F3/Escape input and subsequent Ctrl+C/exact source Copy, themes,
comments and cleanup. Find screenshots are captured only after the selected
passage is actually inside the GTK viewport. Physical IME/accessibility and the
installed optimized build remain separate acceptance gates for this increment.

Final debug runs `table-cells-ui-3q8kqzae` and `inline-labels-ivevbe_f` pass these
checks, including a long cell with a 256×512 image and initial outer scrolling,
and actual Escape followed by Ctrl+C. Short, long and formula Find screenshots
were inspected. A preliminary independent clipboard read once returned empty;
the final copy probe first requires an exact, mapped source selection and then
allows up to two seconds for the asynchronous Wayland clipboard offer to reach
the separate client. It still fails an incorrect or absent offer. This is test
transport synchronization, not a production copy retry or a claimed diagnosis
of that earlier observation.

All 236 native tests pass (two normally ignored), the shared syntax-enabled
Markdown suite passes 90, and Rust 1.92 checks all native targets. The final
four-launch real-report regression `run-tv5rily5` also passes. Formatting,
catalog, Python syntax and diff checks pass. The installed optimized package is
unchanged; the wider native migration and the scrollbar diagnostic remain open.

## Native agent input

Codex `item/tool/requestUserInput` uses native grouped choices with visible
descriptions, optional custom answers and masked secret entries. The first
choice is selected for review; nothing is submitted automatically. A submission
must answer every question and retains the exact custom text. Cancel/window
closing returns the explicit JSON-RPC cancellation error rather than successful
empty answers. The conversation composer stays independent and unsent.

Single-choice questions with a custom answer show an exclusive **Other response**
radio option. Typing selects it. Choosing a provided option retains the custom
draft but submits the visible selected option; choosing Other again restores that
exact text, including whitespace. This fixes the previous mismatch where an old
custom draft silently overrode a newly selected provided option. The additional
provider-wire case verifies the submitted preset, not just the widget state.
Multiple-choice questions retain their additive custom-answer behavior.

OpenCode questions use the same native form controls, including multiple
selections, option descriptions and custom responses when permitted. Answers
retain question order and the original choice labels; custom whitespace is
preserved. `custom: false` omits the free-text entry. Cancel rejects the request
through OpenCode's question API. These semantics follow the
[OpenCode 1.18.31 question schema](https://github.com/anomalyco/opencode/blob/v1.18.31/packages/schema/src/v1/question.ts).

Standard MCP `mcpServer/elicitation/request` form mode renders flat string,
number/integer, boolean, single-selection and multi-selection fields. Titles,
descriptions, defaults and required markers remain visible native controls.
Titled enum choices (`oneOf` and array-item `anyOf`) and legacy `enumNames`
retain their original wire values. Optional blank fields are omitted; false
booleans, exact integer JSON and arrays keep their types. Unicode length,
numeric range and selection-count constraints are checked before sending.
Integral double defaults such as `3.0` normalize to integer JSON. Large integers
must be entered as full decimal digits within the signed/unsigned 64-bit range;
floating representations that could round silently are rejected.
Email/absolute-URI/date/RFC3339 date-time formats are supported; Chrono handles
calendar and timestamp parsing. Regular patterns use bounded Rust regex syntax;
unsupported patterns produce an explicit non-submittable request. This does not
claim complete ECMAScript-regex equivalence or arbitrary JSON Schema support.
Nested schemas, unknown validation keywords, extended `openai/form` modes and
device verification remain explicit unsupported-capability cases.

URL mode displays the requesting MCP server, host and full destination before
**Open link** launches the system browser. Only HTTP(S) URLs with a host and no
embedded credentials are accepted. Nothing opens on request receipt. **Completed**
acknowledges the interaction without returning form content. Decline and Cancel
remain distinct protocol actions. Server identity remains in the fixed header
while fields scroll, and the native action bar wraps in narrow allocations.
The standard forms/URL behavior follows the
[MCP elicitation specification](https://modelcontextprotocol.io/specification/2025-11-25/client/elicitation);
the installed Codex 0.163.0 generated protocol schema was also inspected.

Normalization and answer validation run on Rust workers. GTK gets bounded typed
fields and request/thread routing identity, not raw schemas/default JSON.
Limits are 16 questions, 32 form fields, 64 choices per selection, 256 aggregate
choices, 16,384 input characters per field, 64 KiB answers, a 256 KiB presentation budget and eight
simultaneous form dialogs. Validation errors focus the offending field and retain
all responses; editing clears stale error text. A successful queued reply closes
the form once. Request IDs preserve their JSON types. `serverRequest/resolved`,
backend errors, non-ready backend status and reconnects invalidate the matching
forms, legacy approval dialogs and pending URI launches without sending a stale
answer; late validation results and old dialog callbacks are ignored. The eight
dialog limit counts forms and legacy approvals together.
Replies stay with the original backend/generation even after a view change.
Responses are not written as drafts or preferences.

Command/file permission dialogs now also offer **Allow for session**, respecting
the command's `availableDecisions` list. Additional permissions preserve their
requested profile with `scope: session`; OpenCode maps this explicit choice to
its `always` permission response. Once and decline retain their prior semantics.
Persistent command-prefix/network amendments are covered by the native choices
described below. Permission-profile subsets and turn review are also native;
real-provider and physical desktop acceptance remain open.

### Native approval decisions

Command approvals present the server's available decisions. **Decline** allows
the turn to continue; **Decline and stop turn** returns `cancel`. File-change
approvals also expose that distinction. Additional permission grants preserve
the exact requested profile with explicit turn or session scope. OpenCode keeps
its separate once/session/reject semantics.

When offered, **Remember a rule** contains the exact proposed command-prefix or
network-host policies. Nothing is preselected. Selecting one shows the full
argument array or host/action; **Apply selected policy** then returns that
specific decision. Command arguments retain empty strings, whitespace, Unicode
and newlines using JSON quoting. **Allow once** always returns ordinary `accept`,
even if a policy is selected. Native code does not write policy files; the
original Codex backend owns application of the response.

An explicit `availableDecisions` list is authoritative, including an empty list.
Without that optional list, ordinary decisions and separately proposed rules
remain available for older servers. Unknown/malformed decisions cannot acquire
grant controls. Closing a request with no available non-grant decision returns
an explicit cancellation error. The originating backend, connection generation,
resolution and single-response guards also cover selected policy choices.

Normalization, display summaries and complete response values are prepared on
Rust workers. The review prioritizes the command/input, project, environment,
reason and requested permissions; additional fields from a newer server remain
visible. Routing IDs and duplicated proposal lists do not crowd out that review.
Approval requests are limited to 64 KiB input, 128 KiB presentation, 32 available
decisions and 16 KiB per policy. A prefix contains up to 64 arguments of at most
4 KiB each; host text is limited to 1 KiB. Requests beyond these bounds show an
error rather than offering a grant based on truncated information. The policy
picker has an accessible label and exposes the selected scope as its description.

The original policy checkpoint's private input fixture checked 20 provider-wire replies,
including exact prefix/allow-host/deny-host objects, cancel versus decline,
empty/malformed decision lists, a selected-policy/Allow-once combination and
external resolution after selecting a policy. Nine approval results were also
validated against the response schemas generated by installed Codex 0.163.0
using `codex app-server generate-json-schema --experimental --out <directory>`.
The provider records these decisions without executing commands or updating
real policy files. Screenshot checks wait for a nonblank, actually painted GTK
frame and cover the selected prefix and both host actions. Native approval
summaries, scope labels and responsive controls were inspected in Wayland.

### Selected permission grants and turn review

Additional-permission approvals have native checkboxes for each requested
network, read or write grant. **Select all** and **Clear selection** update a
visible count. Requested grants start selected for review; nothing is sent until
an explicit response. The paths are target-native strings: the client neither
resolves them on the desktop host nor expands globs or special paths locally.
Literal, glob and special entries and legacy read/write lists retain their exact
wire representation.

Equivalent literal-path grants in `entries` and legacy `read`/`write` lists
share one checkbox per access mode. Deselecting it removes all of its aliases;
selecting everything preserves the original profile. Requested deny entries,
disabled-network constraints and applicable glob scan-depth limits remain
read-only and survive partial grants. Clearing every grant returns an empty
profile, while **Decline** always returns an empty profile regardless of the
selection. Profiles containing only constraints preserve those constraints when
explicitly approved. Unknown or invalid permission structures show their details
without acquiring grant controls. Limits are 128 distinct selectable grants,
128 entries per source list and 4 KiB per path within the 64 KiB request budget.

**Allow for this turn** and **Allow for session** apply the selected subset with
the chosen lifetime. **Allow for turn with review** additionally sends
`strictAutoReview: true` with turn scope, asking Codex to review each subsequent
command in that turn. The distinction matches the
[upstream approval choices](https://github.com/openai/codex/blob/main/codex-rs/tui/src/bottom_pane/approval_overlay.rs);
subset replies and simultaneous modern/legacy representations are also exercised
by the [upstream permission round-trip tests](https://github.com/openai/codex/blob/main/codex-rs/app-server/tests/suite/v2/request_permissions.rs).
The installed 0.163.0 generated schemas remain the local wire-format reference.

GTK captures only checkbox indices and the chosen action. Final validation and
response construction run on Rust workers. Responses are committed once after
checking the original backend, connection generation, request lifetime and
submission revision. A window close invalidates a pending grant immediately;
an explicit decline can supersede validation. External resolution, disconnection
or reconnect discard late results. A queue/validation failure retains the
selection for review and retry. No selection is saved to disk.

libadwaita's message-dialog buttons close the window before emitting `response`.
The native controller defers that GTK close request until the validated response
is queued, distinguishing the synchronous button response from an actual window
close. This behavior was verified against the installed library source and a
real close-during-validation test; stopping a response signal alone is insufficient.
Eight shared input-dialog slots bound submissions, with at most one pending
grant plus a superseding cancellation per slot. Selection counting is constant
work per toggle, including the bounded 128-grant case.

At the permission-subset checkpoint, the expanded private input fixture verified
28 provider replies, including
partial turn/session grants, strict review, alias removal, empty/all selections,
unsupported structures and cancellation while validation is pending. Direct
reentrant resolution/status events and a real backend reconnect exercise late
worker-result guards. It also checks all 128 controls without truncation and
retained conversation text. Eight permission results validate against the
installed Codex response schema. Private providers record the responses without
changing real policies or running model turns. Real-provider enforcement and
physical accessibility/IME/focus acceptance remain migration gates.

This checkpoint passes 152 native Rust tests (one ignored), the optimized native
build check, the OpenCode input/recovery regression, the private two-launch
notification regression and the full four-launch application regression. The
4,097,226-byte report opened in 1,002 ms with 62 opening heartbeat ticks. In the
28-reply input run, first input mapped in 106 ms and the workflow heartbeat
maximum gap was 499 ms including capture; observed debug native/gateway peak RSS
was 324,948/43,144 KiB. Clearing/reselecting 128 controls took less than one
millisecond of synchronous event handling, excluding subsequent layout. These
are local workload observations, not performance improvements or release gates
being waived. No fresh release bundle or real-provider policy change is implied.

### OpenCode project scope and pending recovery

Permission and question replies carry the originating project's `directory`,
captured from the global SSE envelope. Switching conversation or backend does
not redirect the response. An envelope takes precedence over cached session
metadata; requests with no valid project cannot be submitted. OpenCode's
`permission.replied`, `question.replied` and `question.rejected` events resolve
the exact original request using `requestID`, session and project. This follows
the upstream [permission implementation](https://github.com/anomalyco/opencode/blob/v1.18.31/packages/opencode/src/permission/index.ts)
and [question implementation](https://github.com/anomalyco/opencode/blob/v1.18.31/packages/opencode/src/question/index.ts).

Opening or refreshing a session also reads its project's pending permission and
question inventories. History displays before these background reads complete.
Two recovery tasks share a queue of up to 16 recent projects; repeated reads of
the same active/queued project coalesce. The connection retains at most 256
pending routing records, accepts at most 256 entries per combined inventory and
limits each request's presentation metadata to 256 KiB. It does not load pending
requests across every catalog project.

Only the connection's ordered event loop projects a finished recovery snapshot
for display. Live resolutions already processed there suppress stale inventory
entries. A bounded history of 512 resolutions protects in-flight reads; a
snapshot that outlives that history is rejected with a refresh notice. A failed
inventory read leaves conversation history available. Each explicit response is
attempted once. After an uncertain failure, reconnect and inspect the recovered
request before responding again; replies are never replayed automatically.

The shared HTTP proxy now preserves encoded path segments and query scope. A
request ID containing a slash, question mark or Unicode text remains an ID at
the upstream router. Authentication, redirect and body limits remain enforced.
This gateway correction also applies to the existing browser client.

`scripts/smoke-gnome-opencode-input.py "$NATIVE" "$GATEWAY" --headless` checks
these paths with real GTK controls and private authenticated HTTP/SSE backends:
cross-project/backend view changes, multiple/custom answers, rejection, external
resolution, stale recovery snapshots, lost acknowledgements and explicit retry
after reconnect. It uses Unicode/space-containing project paths and an escaped
request ID. It also checks legacy Codex approval resolution, connection loss,
generation isolation and retained composer drafts. The run sent nine OpenCode
POSTs and one Codex response, with no model turns or real provider sessions.
Pending recovery left history usable in 20 ms while its metadata read was held;
the workflow heartbeat maximum gap was 466 ms including screenshot capture.
Observed debug native/gateway peak RSS was 322,260/44,068 KiB. The checkpoint
passes 139 native Rust tests and 205 gateway tests (one ignored in each suite),
optimized build checks for both components, the Router dispatch regression and
the four-launch application regression with the real 4,097,226-byte report.
These are local fixture observations, not release or real-provider acceptance.

`scripts/smoke-gnome-input.py "$NATIVE" "$GATEWAY" --headless` drives actual
native controls through the protocol worker and private provider connection.
It verifies choices, custom/secret input, validation before reply, typed/default
values, cancellation, consent through a private URI portal, external resolution,
numeric/string ID isolation, reconnect, session approvals and composer retention.
No model turns, real provider sessions or external browser launches are used.
The fixture also checks owned provider cleanup; its private desktop teardown
detaches only that runtime's allowlisted portal/GVfs mounts. Physical IME,
screen-reader and real provider acceptance remain part of the overall migration.
The approval checkpoint passes 145 native Rust tests (one ignored), the
optimized native build check and the four-launch application regression. The
real 4,097,226-byte report opened in 902 ms with 56 opening heartbeat ticks.
The private input run mapped its first form in 101 ms; the workflow's largest
observed heartbeat gap was 445 ms including screenshot capture. Debug peak
native/gateway RSS was 322,928/43,420 KiB. Private notification transport,
activation/deduplication and OpenCode input recovery also receive regression
coverage. These workload observations do not replace physical acceptance or
produce a release bundle.

## Native conversation history

**Menu → Browse conversation history…** or **Ctrl+Shift+H** opens a read-only
history window inside the conversation pane. **Older/Newer** replace the displayed
window; **Live** restores the current transcript and retains the unsent draft.
Live delivery, Stop, model/options and queued work keep their existing state while
an older window is displayed. Incoming messages update the live tail without
replacing or scrolling the historical content. An explicit ordinary Send returns
to Live so its response is visible. Router controllers retain their separate
existing history controls.

### Initial session loading

Ordinary Codex selection uses `thread/resume` with `excludeTurns: true`; archived
views and refreshes use `thread/read` with `includeTurns: false`. The client then
loads 120 recent descending items and reads four recent turn summaries to verify
the active turn and user-shell identity. Resume item and turn cursors are kept
separate and passed to their matching APIs. Configured/environment resumes and
Router metadata reads retain their original semantics. Unsupported methods fall
back to four full turns and a legacy full read; servers that reject the metadata
resume parameter get one retry with the original request. Compatibility reads
can still carry large turn/history payloads and remain subject to the transport
limit. A server that ignores the parameter and returns full history retains the
existing bounded 400-item projection.

HTTP, decoding and Markdown preparation run outside GTK. Two initial readers
share a semaphore across backend connections; each connection admits at most
eight pending selections. A newer read of the same thread cancels its older job,
with socket/view generations and request identities rejecting late replies.
The existing two-thread CPU pool and the gateway's private-read admission bound
remain in force. A small bounded immutable-row reconciliation commits the recent
page together with observed live items on the owning GTK view. It does not replay
deltas on top of persisted text. Terminal events received during loading win over
an earlier busy snapshot. Existing offscreen cached views also receive observed
events; returning to an already subscribed backend preserves their partial text.
A real reconnect does not append across a known text gap.

Send, selected-session changes, shell/review commands and queued dispatch remain
gated until history and turn state are verified. Explicit Stop retains its
observed active-turn target. Malformed pages or lifecycle state retain the draft,
pause affected queued work and require a refresh. No idle status is inferred from a
catalog row. The normal live tail can subsequently grow to its existing
400-item/32-MiB bound; older history remains available through the browser below.

The [official app-server contract](https://learn.chatgpt.com/docs/app-server)
documents metadata-only resume and history paging. The installed Codex 0.163.0
was also exercised through `scripts/test-support/codex-hydration-contract.py`
against a loopback fake Responses endpoint in a private profile. Both full reads
and item pages omitted an unfinished assistant message until completion. Studio
therefore retains text it has observed, but cannot recover unpersisted text from
before subscription; a full-history fallback does not recover it either. This
local SDK probe performs no inference against a real provider.

`scripts/smoke-gnome-hydration.py NATIVE_DEBUG_BINARY GATEWAY_BINARY --headless`
uses the same owned Mutter/private-service setup as history browsing. Its
20,000-item fake provider covers active initial loading, partial/delta retention,
offscreen reads, backend reuse, completion before a stale busy reply, malformed
history/lifecycle, Send/Queue/command guards, superseded reads, cursor compatibility,
legacy reads, one old-resume retry, reconnect, drafts and backend cleanup. It
starts no model turn. The mapped-body and heartbeat measurements include initial
startup; they are synthetic debug evidence, not a release performance guarantee.

In the final hydration fixture on Ubuntu 26.04/Mutter 50.1, the first mapped
body appeared 708 ms after the GTK probe started; 11 heartbeat ticks recorded a
445-ms maximum gap. This exposes remaining startup work rather than establishing
smooth interaction. Successful modern provider page JSON totalled 135,482 bytes
over 21 replies, with a 13,468-byte maximum, excluding envelope/HTTP overhead and
legacy full reads. Sampled workload peaks were 354,300 KiB native and 68,496 KiB
gateway; these cover the whole mixed compatibility scenario, including full
20,000-item legacy responses. The hydration checkpoint passed 108 native
and 191 gateway unit tests (one ignored in each suite), the private SDK probe,
API security checks and Wayland hydration/history cases. Debug executables are
refreshed; release executables and packaging remain pending. The final full
native regression also passes with the real 4,097,226-byte report and no critical
application errors across its four logs, covering document editing, tables,
comparison, queues, commands, agents, workspace tools, restart and cleanup.

### History browsing

The client prefers Codex `thread/items/list`, requesting 120 descending entries
across the thread. Its `ThreadItemEntry` projection preserves each item and turn
identity. This shape and method were checked against the installed Codex 0.163.0
generated protocol schemas. The authenticated, origin-checked native conversation
read API exposes only the fixed read operation, page size and direction; it adds
no arbitrary RPC or mutation capability. Servers without item paging fall back
to four-turn full-item pages, and then a legacy thread read if necessary. Only a
first-page unsupported-method response changes modes; errors on later pages
retain their view and cursor rather than mixing cursor protocols.

OpenCode uses 80-message pages with encoded opaque `before` cursors and the
selected project directory. Multiple parts in one message, or more than 120
items in a compatibility turn page, are split with source anchors rather than
being silently dropped. Overlapping seams are deduplicated. A missing required
anchor, malformed response, repeated cursor or read failure retains the current
page and exposes an error; reads can be retried. A full OpenCode page without a
continuation header explicitly warns that older messages may be omitted by the
server. Legacy Codex reads are labelled.

HTTP, JSON decoding, source projection and Markdown parsing run on Rust workers.
GTK receives immutable prepared items and reuses the existing native Markdown,
table, highlighted-code, diagram, image, source anchors and favorite controls. Loaded-window
search uses the displayed history items when full provider search is unsupported.
A latest-request watch channel cancels superseded reads, and source/serial guards
reject late responses after Live or a backend/session/generation change.

A page retains at most 120 items and 32 MiB of presentation data, alongside the
existing 400-item/32-MiB live tail. Transport remains capped at 32 MiB per response;
individual text outputs still use the existing 128-KiB excerpt bound, with a
notice. Navigation retains at most 4,096 positions within a 4-MiB logical metadata
budget. Reaching it reports the limit and preserves the current view; Live can
restart navigation. Source snapshots up to 16 MiB have the separate full-output
viewer below. Larger/absent provider outputs, full rich output preview, deeper
navigation, broader lifecycle/reconnect recovery and large real-provider acceptance remain migration
work; this increment does not establish complete history parity.

`scripts/smoke-gnome-conversation-pages.py NATIVE_DEBUG_BINARY GATEWAY_BINARY --headless`
exercises turn-page/legacy compatibility; add `--items` for preferred item pages.
`--api-only` checks authentication, foreign-origin denial, fixed operation/page
parameters, opaque cursor continuation and mutation rejection without a compositor.
Controlled GUI runs require the test-only `mutter` package and use an owned
Mutter Wayland compositor, a 1600×1000 virtual monitor and no Xwayland, following
the [GNOME testing guidance](https://gnome.pages.gitlab.gnome.org/pygobject/guide/testing.html).
They use the real gateway with private providers, profiles, D-Bus and runtime
directories. Short private runtime paths isolate AT-SPI sockets from the desktop
session. Cleanup detaches only the fixture's own document-portal FUSE mount if
the service leaves it disconnected. The same helper isolates notification fixtures.
No application critical-log filter is bypassed; physical accessibility and portal
acceptance remain separate gates.

Without `--headless`, keep the test window visible on the desktop. A desktop run
timed out with no mapped bodies and an unlaid-out list while manual widget
capture produced the correct history. The strict mapped-body check is retained:
controlled Mutter runs pass it without the capture helper forcing layout. This
distinguishes normal frame delivery from the existing covered-window snapshot
fallback; those earlier model-readiness timings do not measure visible content.

The fixtures cover a 650-item turn, cross-turn pages, 250 OpenCode message parts,
opaque/overlapping cursors, native historical tables/code, loaded-page search,
draft/scroll/live isolation, retained errors and retry, delayed cancellation,
backend switches, legacy reads and one explicit fake-provider Send. Browsing
starts no turn; only the final Send does. In owned Mutter 50.1, the first mapped
history body appeared in 186 ms for item pages and 219 ms for turn compatibility.
Their initial paging sequences recorded 13/18 main-loop heartbeat ticks and
100/95-ms maximum gaps, excluding PNG capture. Sampled workload peaks were about
331/332 MiB native and 43/44 MiB gateway. The largest serialized provider item-page
payload was 12,910 bytes; 12 successful Codex page responses totalled 147,111
bytes. These payload figures exclude HTTP/envelope overhead and are synthetic
debug measurements, not release or arbitrary-history guarantees. The GTK
screenshots were inspected. The history-browser checkpoint passed 191 gateway and 101
native tests (one ignored in each Rust suite) and all 748 web tests. Both optimized
configuration checks pass. Debug executables are refreshed; release packaging
and the remaining migration gates are pending.
The full native application regression also passes on the owned Mutter display
using a private copy of the real 4,097,226-byte report: source/edit/table/compare,
conversations, workspace tools, Settings, restart and backend cleanup pass, with
no application critical errors in its four logs. Notification delivery and
post-exit activation still pass on a separate private notification bus.

## Native conversation output

The **Open complete output snapshot** button beside an excerpt opens a native,
read-only text viewer. It applies to assistant/user messages, command output,
reasoning summaries, tool results, diffs, history windows and saved-favorite
previews. The normal conversation retains its 128-KiB preview bound. Large plain
blocks above 8 KiB use a scrollable GTK text widget with bounded visible height,
instead of laying out an unbounded wrapping label inside the transcript list.
Small plain blocks and rendered Markdown retain their usual presentation.

Full textual source is retained during Rust worker projection, before truncating
the preview. A process-private LRU has a 64-MiB source/index budget and at most
256 snapshots; one snapshot accepts up to 16 MiB of UTF-8 text. An open viewer
keeps one separate shared lease so cache churn cannot invalidate the text being
read. Snapshot IDs are random opaque handles, never filesystem paths or provider
commands. Opening and paging cached output performs no additional provider read.
No automatic output files or disk cache are created. An expired unopened handle
explains how to reload the conversation/history; an oversized snapshot reports
the bound. Text omitted or truncated by the provider cannot be reconstructed.

Rust builds a sparse page index and supplies at most 64 KiB of raw source and
256 LF separators per page. A split long line is marked as a continuation and
keeps its global source line number. UTF-8 and CRLF pairs remain intact; GTK
gutter offsets and buffer line counts are checked against the source positions.
NUL, standalone CR and Unicode paragraph/line separators are shown as visible
symbols in this source view, preserving scalar selection offsets and LF-based
line numbering. The original source bytes remain unchanged for export. Unsafe
NUL bytes in ordinary previews are also replaced before reaching GTK.

First/previous/next/last, direct page entry, wrapping, page copy and case-sensitive
literal find operate on the immutable snapshot. Find preserves trailing spaces
and wraps around; it scans the whole source on a Rust worker rather than just
the visible page. Failed reads/finds retain the current page. Source IDs, dialog
IDs and request serials reject stale results, and a latest-request channel cancels
superseded reads. These operations do not change live transcript/draft/turn state.
Delta previews copy only the bytes that fit their presentation limit; a completed
provider item supplies the new full snapshot. OpenCode history uses original
user-message parts for the full text while retaining bounded live assembly state;
already bounded live user previews explicitly require a reload rather than
claiming to be complete snapshots.

**Export complete snapshot…** uses the native file chooser and a bounded Rust
worker to write a temporary file beside the chosen destination, sync it and
atomically replace the destination. The confirmed export captures a lease before
queueing I/O so closing the viewer cannot expire its source. Source text is not
copied wholesale into GTK or the clipboard. Export is the full received textual
representation, including the command/diff/tool prefixes shown by Studio; the
viewer has source text rather than a complete rich Markdown preview.

The optional Wayland fixture is:

```bash
python3 scripts/smoke-gnome-outputs.py NATIVE_DEBUG_BINARY GATEWAY_BINARY REAL_MARKDOWN_FILE --headless
```

It uses private providers/profiles, an owned Mutter compositor and `python3-gi`
as test tooling. Source headers are activated with real Wayland keyboard events
through Mutter's [private remote-input interface](https://raw.githubusercontent.com/GNOME/mutter/main/data/dbus-interfaces/org.gnome.Mutter.RemoteDesktop.xml).
Before creating a virtual keyboard session, the fixture verifies that the D-Bus
service PID is its own `/usr/bin/mutter` parent. Input is never directed to the
user's compositor. An earlier synthetic `clicked` run loaded the data but left
the dialog sheet unmapped. The current check requires a mapped source widget,
allocated text and source-glyph geometry before screenshot capture; no manual
frame-clock signal or allocation is used to satisfy that check. Default dialog
animations remain enabled. This is controlled Wayland evidence; physical input,
IME, accessibility, portal chooser and clipboard acceptance remain separate.

The real 4,097,226-byte report is tested as a received assistant message and via
OpenCode/history, alongside large commands, reasoning, tools, diffs and user
messages. All nine exported files are compared byte for byte, including CRLF,
progress-control characters and NUL. The fixture also covers full-source Unicode
find, missing matches, invalid pages, expired handles, stale dialog responses,
draft/transcript isolation, no extra provider reads and process cleanup. A traced
baseline spent 14,934 ms reaching the first output header before its viewer
opened. Replacing the large plain labels reduced that path to about 1,400 ms
in the same synthetic debug workload, which includes the fixture's one-second
activation delay. Startup and shorter remaining main-loop gaps still need work;
this is not a release smoothness guarantee.

The final private Wayland run passed all nine cases with default animations:
the first header took 1,377 ms and its first mapped source page took another
129 ms. Subsequent mapped source pages in the case entry checks took 20–161 ms.
The largest measured main-loop gap was 960 ms, including screenshot capture;
native/gateway peak RSS was 548,188/80,184 KiB in this debug stress workload.
These figures describe this fixture, not a general memory or latency guarantee.
Screenshots were reviewed for source-line positioning, backend selection and
completed dialog transitions. The native suite passes 115 tests with one ignored;
native release-profile checking, the four-launch application regression and the
20,000-item hydration regression also pass. The current debug executable includes
this work; no fresh release executable or install bundle was produced for this
increment. Later checkpoints below cover paged rich paragraphs, reading positions
and live rich previews. Full rich-output preview remains a separate parity gate.

### Output export chooser lifetime

Each output viewer now owns at most one cancellable Save dialog. Repeated Export
requests share the pending choice. Closing or replacing the viewer invalidates
the choice before cancellation; late replies cannot write a destination or clear
a newer chooser. Cancel is quiet, while chooser failures and unsupported nonlocal
destinations retain the source page and selection and restore Export. The viewer
continues to export only to local paths.

Choosing a destination admits an immutable snapshot export on the Rust worker.
That confirmed write can finish after its viewer closes. The application tracks
pending writes independently of the visible viewer and rejects main-window close
until their completion is acknowledged. An unconfirmed choice is cancelled when
window close is consulted; if another close guard keeps the application open,
the viewer can immediately open a fresh Save dialog. Failures update an open
matching viewer and also enter the window-owned error queue described below, so
closing or replacing the viewer cannot hide the outcome.

Favorites also owns one cancellable Save choice, disables duplicate Export
requests and ignores replies from cancelled choices. Closing the library, or
consulting its main-window close guard, cancels only the unconfirmed choice.
Unsaved metadata remains in the editor until the user explicitly saves or
discards it. Export still uses the persisted library through the shared gateway;
it does not save the current draft. Existing GFile destination support and the
guard on confirmed writes remain in place. Chooser failures are shown in the
library's native toast overlay and restore Export.

Close confirmation is reserved synchronously before scheduling its async task.
This fixes a race where repeated close attempts could queue a second save prompt
after the first Discard had already closed the library. The fixture reproduces
that sequence and requires an empty dialog stack before final application close.
Libadwaita can consume a window close to dismiss its visible dialog first (see
its [dialog host close handler](https://github.com/GNOME/libadwaita/blob/1.9.1/src/adw-dialog-host.c#L187));
the export test therefore also attempts close after dismissing the output viewer
while the worker is still paused, to exercise Studio's pending-write guard.

The additional optional private Wayland fixture is:

```bash
python3 scripts/smoke-gnome-save-portal.py NATIVE_DEBUG_BINARY GATEWAY_BINARY
```

It exercises thirteen real GTK SaveFile requests, five protocol cancellations and
five deliberately late successful replies. It covers duplicate clicks, chooser
errors, nonlocal rejection, same-viewer reopen, viewer replacement and source-page
selection retention. Both admitted files match their original Unicode/CRLF/NUL
snapshots byte for byte, and all five cancelled destinations remain unchanged.
A private-profile, debug-only worker gate pauses a confirmed export so the
fixture can invoke the actual main-window close handler, verify that it refuses
close, then close the viewer and allow the write to finish. Release builds omit
this gate. Five of the requests exercise Favorites, including an actual main
window close attempt with dirty metadata, immediate chooser reopening, persisted
library export and explicit Discard during panel close. Toast assertions require
mapped, allocated labels; the Favorites screenshot is captured after the native
transition. Both screenshots were inspected, including the exported confirmation
alongside the still-unsaved editor title.

This checkpoint passes 159 native tests with one ignored and optimized-profile
checking. The four-launch application regression and the 11-request Open/folder/
file-manager portal fixture pass. The application regression opens the real
4,097,226-byte report in 1,059 ms with 64 main-loop ticks during opening. The
Save-specific debug fixture observed native/gateway peak RSS of
348,568/43,640 KiB; these are workload observations, not performance guarantees.
The existing nine-source output/export regression timed out once waiting for a
mapped header during concurrent fixture execution, then passed in two isolated
runs with all nine exact exports and cleanup checks. The next checkpoint
reproduces and fixes a navigation-ordering race in this area; the original
timeout was not captured with enough detail to prove it had the same cause.
Other Save/export choosers, physical GNOME portal UI and fresh release packaging
remain separate gates.

### Background export failures

Output and Favorites exports now share a window-owned native failure queue.
An admitted output export retains its source title and destination separately
from its viewer. Completion removes that exact operation once; a duplicate or
stale completion cannot report again or change a replacement viewer. Failures
remain visible after viewer dismissal, with the original title, destination and
filesystem error. Multiple failures appear one at a time and require ordinary
Close acknowledgment before the next is shown. The application does not silently
close with queued unacknowledged errors. The existing guard on unfinished writes
remains independent of those error dialogs.

Details use bounded plain, selectable text in a native scroller, not markup or
the exported content. Remote GFile destinations keep their original I/O identity;
only diagnostics remove URI user info, query and fragment data. Known URI
credentials are removed before display clipping, with bounded input/field counts
and replacements that cannot expand repeatedly. Local Unicode paths stay intact
within the display limit. Unit tests cover these diagnostic bounds/redaction;
this fixture does not exercise a real remote GVfs destination.

Favorites distinguishes waiting for the gateway's persisted-library snapshot
from writing the resulting GFile. Once local I/O is admitted, a gateway failure
keeps the export serial, close guard and concurrency guard until that write's
own completion. A late duplicate gateway reply cannot start another write or
clear those guards. Failure releases the write guard, reports the original
destination and keeps unsaved metadata intact. A successful local write can
finish despite losing the gateway; it still exports the persisted snapshot.
The transient output-close notice is coalesced and dismissed on completion;
it no longer replaces the conversation's connection status with a stale
“wait for export” message.

```bash
python3 scripts/smoke-gnome-write-failures.py NATIVE_DEBUG_BINARY GATEWAY_BINARY
```

The owned Wayland/private SaveFile fixture makes six real Save requests and
causes five actual filesystem failures by choosing existing nonempty directories.
It covers a closed viewer, a replaced viewer with its page/selection retained,
duplicate completion rejection and two admitted failures queued behind one
visible alert. It attempts actual main-window close during a held write.
Favorites receives a simulated gateway-loss callback during each of two
admitted local writes: one fails on disk and the other completes successfully.
That is component-level fault injection, not a physical server/network outage.
Both paths keep dirty metadata and reject concurrent Export. Independent checks
verify destination sentinels, temporary-file cleanup, the unchanged persisted
favorite, exported content, no model prompts and owned-provider cleanup.

The error screenshots show real mapped window nodes and native button allocation.
The fixture focuses only its owned compositor window and waits for the normal
dialog transition; it never emits frame-clock signals or forces GTK allocation.
Closed/replaced-viewer and Favorites screenshots were inspected. The shared
capture helper also serves inline-math acceptance. The fixture sampled peak
native/gateway RSS of 354,316/43,540 KiB; these are small debug-workload
observations, exclude worker peaks and do not establish total-memory or latency
bounds. Physical GNOME chooser/accessibility acceptance, other export tools and
the remaining migration gates still apply.

This checkpoint passes 166 native tests (one normally ignored), the thirteen-call
Save/cancellation regression, shared-capture inline-math acceptance and the
four-launch application regression. The latter opened the real 4,097,226-byte
report in 1,028 ms with 63 loading ticks and preserved source anchors, explicit
page saves, Unicode/CRLF, conflict checks and backend cleanup. The optimized
frontend was rebuilt and refreshed in the current package; the installed-pair
AT-SPI/Wayland check also passes without application test hooks, preserving
Preview/Source/Edit, Undo/Redo, explicit saves, desktop handoff and lazy web
engines. That separate small release workload sampled peak native/gateway RSS
of 317,684/27,416 KiB. It does not inject gateway-loss/disk-failure timing into
the release executable. The debug gates used for those causal checks are omitted
from optimized builds; remote CI and physical desktop/provider sign-off remain
unverified.

### Conversation navigation priority

Conversation search and Thread Router source jumps now share a GTK list
navigation coordinator with history-page and follow-tail scrolling. An explicit
jump cancels an older deferred scroll before asking GTK to reveal its target.
Only one automatic edge-scroll idle is retained. Its generation and row identity
are checked before use, and scope changes or controller destruction cancel it.
Widgets and the viewport are held weakly; pending navigation does not retain an
old window or conversation.

Reading intent remains active while GTK allocates the selected row, so another
model update cannot mistake the old bottom offset for permission to follow the
tail. Adjustment position and extent changes track leaving and returning to the
end; automatic following resumes within the existing 80-pixel end margin.
Explicit history-page navigation still chooses its requested edge. This uses
GTK's existing [list scrolling](https://docs.gtk.org/gtk4/method.ListView.scroll_to.html)
and adjustment machinery, without replacing its layout or virtualization.

The extended `smoke-gnome-outputs.py` fixture deliberately queues follow-tail
immediately before selecting an earlier OpenCode search result through the real
selection handler. Before the fix, that selection left the viewport after the
idle ran. The check now also reconciles again before GTK layout, requires actual
viewport intersection rather than only a mapped widget, and verifies that three
new provider messages leave the chosen message visible. After returning to the
end, three more scoped OpenCode SSE messages must be followed through their last
rendered block. These are private provider events; no model turn or provider
write is issued. All nine original Unicode/CRLF/NUL exports remain exact.

The extended Wayland check passes alongside 159 native tests (one ignored),
optimized-profile checking, the four-launch application regression, native rich
streaming and the three-launch Router UI regression. In the isolated debug
output run, native/gateway peak RSS was 562,256/79,736 KiB. The broader application
run opened the real 4,097,226-byte report in 902 ms with 56 opening heartbeat
ticks. These observations do not claim a general speed or memory improvement.
Two concurrent runs of the extended output fixture also pass all navigation,
provider-read isolation, exact-export and process-cleanup assertions. A final
mapped conversation screenshot was inspected after closing the output viewer.
Physical scrolling, focus/IME/accessibility behavior and longer real-provider
sessions remain acceptance work. The earlier intermittent output-header timeout
was not independently traced to this race; the stronger ordering checks cover
the reproduced failure, rather than proving every navigation timeout resolved.

## Native long rich paragraphs

Markdown paragraphs and headings above 8 KiB use a native paged text view in
conversations, historical context, Favorites, the document reader and comparison
panes. Smaller ordinary blocks retain the wrapping-label presentation; prepared
math uses the shared object path described above. A long block has
first/previous/next/last controls and a 320-pixel reading viewport with a visible
scrollbar. Selection and copy operate within the displayed page; the original
Markdown remains available through the existing Source/Edit and message actions.
The application never saves the display pagination into the document.

The reusable Rust Markdown model prepares Unicode scalar ranges for bold,
italic, strikethrough, inline code and safe links on its bounded parsing worker.
It also indexes UTF-8 pages, normally at most 8 KiB, preferring whitespace and
preserving CRLF pairs. Retained formulas are atomic, with the bounded dedicated
page exception described above. Per-page attribute indexes keep GTK from scanning the whole block.
Immutable shared snapshots retain the complete bounded fragment; only one slice
and five reusable text tags enter each GTK text buffer. This uses
[GTK's text buffer/view and tag model](https://docs.gtk.org/gtk4/section-text-widget.html).
It avoids reparsing Markdown or Pango markup on the GTK thread for these blocks.
Inline HTML remains literal text; NUL is displayed as a visible symbol without
changing the original file's source bytes or anchors.

Ctrl+click, Ctrl+Enter and the native context menu open a link through the same
scoped source policy as small paragraphs. Worker-prepared search targets retain
the parser's original source ranges and resolve the exact occurrence to a page
and Unicode selection. Equal quotes on different pages remain distinct. Search
positions the selected line with surrounding context; a match spanning pages
selects its visible prefix and shows a continuation notice. Paragraph page state
survives equal conversation refreshes and row reuse.

The shared model retains [pulldown-cmark event/source ranges](https://docs.rs/pulldown-cmark/latest/pulldown_cmark/struct.OffsetIter.html)
during the existing parse. Borrowed text slices and verified code normalization
keep source bytes separate from rendered scalar offsets, including Unicode,
escapes, entities, CRLF and trimmed multiline code. Mapping uses the supplied
source occurrence and verifies the rendered text; it does not search the whole
paragraph for an equal quote. Markdown link destinations and marker-only hits never
select another visible occurrence. These hits clear the old selection and show
**match is in Markdown source**; the existing message Markdown copy action
provides the original text. Nonzero fragment bases and byte-preserving bare
PlantUML normalization retain their original source positions. The additional
sparse maps are included in the existing presentation memory budgets.
The reader retains up to 512 recent paragraph reading positions per document.
Page and within-page positions survive same-revision fragment replacement and
Source/Preview switches. A new document/revision clears the cache. Explicit
outline, search and page navigation resets its destination context, so a stored
reading position cannot override a requested jump. These positions stay in memory.
Source-linked comments continue to capture the selected Unicode quote and the
original block's source-line range.

The shared native text widget retains a visible Unicode character, its signed
line offset, caret and selection, including GTK's negative top-margin coordinates.
It restores the
same text when an equal conversation refresh recreates the row, including when
the replacement wraps at a different width. It uses
[GTK's deferred text-mark scrolling](https://docs.gtk.org/gtk4/method.TextView.scroll_to_iter.html)
and at most one pending frame callback per paragraph, limited to eight frames.
Replaced widgets cannot overwrite a newer view's position or page. Pointer,
touch, wheel and key events cancel a pending restore without consuming native
input. This adds no source parsing, full-text copies, disk state or autosaving.
It does not yet promise character retention during a width/font change that
reflows an existing widget without replacing it; physical input acceptance also
remains separate from the programmatic widget tests.

The optional private Wayland workload is:

```bash
python3 scripts/smoke-gnome-rich-text.py NATIVE_DEBUG_BINARY GATEWAY_BINARY --headless
python3 scripts/smoke-gnome-rich-text.py NATIVE_DEBUG_BINARY GATEWAY_BINARY --headless --unbroken
```

It opens the same long paragraph through the real conversation and reader,
checks realized native widgets, formatting, Unicode selection, source comments,
relative links and navigation to the tail. The unbroken-text case contains
121,944 source bytes. A whole-paragraph text-view experiment improved initial
layout but reached 530,956 KiB peak native RSS while navigating its wrapped line.
At the paragraph-paging checkpoint, the final run including equal-refresh
retention and search navigation peaked at 335,468 KiB; reader mapping took
205 ms with a 20-ms measured main-loop
gap. Conversation mapping took 807 ms with a 435-ms startup gap. The ordinary
84,144-byte paragraph passed the same checks, with 202-ms reader mapping and
a 25-ms gap. These are debug workload measurements, not a general smoothness or
memory promise. The initial wrapping-label baseline took 1,014 ms to map the
unbroken paragraph in the reader and allocated a 26,653-pixel-tall label.

At the original paragraph-paging checkpoint, the reusable Markdown model passed
20 tests and the native suite passed 117, with one ignored. Both private paragraph workloads, the four-launch application
regression with the real 4,097,226-byte report, the two-launch Markdown comment
regression and all nine complete-output export cases pass. Screenshots were
reviewed for readable viewport size, formatting, toolbar alignment and source
positioning. Rust formatting and the native release-profile check also pass.
The comment fixture now implements the bounded item-history method used by
initial hydration. Physical pointer/keyboard link activation, IME and assistive
technology acceptance remain separate from these widget/action checks. The debug
binary is current; release executables/install bundles were not rebuilt here.
The source-mapping increment adds repeated-occurrence, trailing-space, split-page
and Markdown-only-hit checks on the actual Wayland widgets. Its extended normal
and unbroken fixtures contain 84,198/121,998 source bytes. Captures require complete
selected lines inside the text viewport when the match fits; larger matches must
show their first complete line. The source notice and four page controls also
stay within the 320-pixel conversation pane. Screenshots were reviewed after the
scrolling animation settled. The four-launch application regression with the
real report and optimized native checking were rerun after mapping integration.
The original one-result-per-message search behavior remains; this does not add
an all-occurrences search mode.
The later reading-position fixture checks two consecutive equal-content row
replacements, deliberately emits obsolete adjustment callbacks, and recreates the
conversation paragraph at 844 then 672 pixels wide. It verifies the original
character remains near the viewport top, preserves the caret/selection, retains
the outer transcript position, and checks exact top margins after another
refresh. Reader checks cover repeated Source/Preview round trips and explicit
source-line jumps overriding a cached position. Existing repeated-source,
trailing-space, split-page search, source comments and relative links still pass.
The final ordinary/unbroken debug runs mapped reader views in 196/187 ms with
159/157 ms maximum startup gaps, and chat in 830/851 ms with 432/448 ms gaps.
Sampled peak native/gateway RSS was 351,684/45,008 KiB and 350,800/45,068 KiB.
These include provider startup and fixture work; they do not establish a
performance improvement or guarantee smooth physical scrolling.
The reading-position checkpoint passes 155 native Rust tests (one ignored),
optimized native checking, both paragraph workloads, the four-launch application
regression, the long-cell fixture and the two-launch Markdown comment workflow.
No fresh release executable/install bundle or remote CI run is included here.
Full-cell search/comments, live preview beyond the excerpt bound and
full rich rendering of oversized conversation output remain separate gates.

## Native formatting-limit source pages

A valid, bounded Markdown window that exceeds the block, table-cell or nesting
formatting budget remains readable as literal source. The shared Rust worker
prepares the same 8-KiB pages and source projections used by native rich text.
Conversation history, live previews, saved-message previews and document Preview
consume that presentation; GTK does not parse one large fallback markup label.
Source range/size errors still fail validation. The strict table parser continues
to reject invalid table responses rather than silently replacing their rows.

The fallback uses read-only monospace text, preserves Markdown markers and shows
the physical source line range on each page. Header and body continuations keep
their own original byte/line coordinates. HTML, image syntax and URLs remain
literal text, with no active links or diagram processes. Search projects the
exact original occurrence, including Unicode and trailing spaces; it never picks
an equal quote elsewhere. NUL is displayed as a visible control symbol while the
original source remains unchanged. Normalization source maps are capped at 4,096
spans; an unverified occurrence falls back to the existing source-only search
notice rather than receiving an approximate highlight. Source/Edit remain
available through their normal validation and explicit-save rules.

The optional debug acceptance fixture forces a nesting-limit failure through a
private provider and opens the same document in the actual reader:

```bash
python3 scripts/smoke-gnome-fallback.py NATIVE_DEBUG_BINARY GATEWAY_BINARY --headless
```

It verifies both presentations, first/last pages, inert literal syntax, exact
late-page search selection, physical line captions, bounded read-only buffers,
Source/Preview switching, unchanged original file bytes, unsent message retention
and owned-provider cleanup. The 57,539-byte CRLF/Unicode fixture mapped chat in
496 ms and reader content in 203 ms, with 22 heartbeat ticks and a 444-ms maximum
gap including startup/layout. Sampled native/gateway peak RSS was
334,996/44,408 KiB, excluding compositor and external workers. Both screenshots
were inspected. These debug observations do not establish a general performance
improvement or prove smooth physical input.

The shared parser's 28 tests include formatting-limit recovery, disjoint header/
body coordinates, invalid/oversized range rejection, NUL source projection and
dense-normalization map limits. Full rich rendering beyond snapshot bounds,
physical Wayland input, wider provider coverage and release installation remain
separate migration gates.

This checkpoint passes 159 native tests (one external-engine test ignored), the
optimized native compile check and an isolated Rust 1.92 compatibility check.
The unbroken rich-paragraph/source-projection fixture, streaming fixture and
four-launch application regression pass. The real 4,097,226-byte report opened in
1,025 ms with 63 opening heartbeat ticks; that full workload sampled peak
native/gateway RSS of 511,248/178,340 KiB, excluding external workers. Streaming
retained paragraph/table/diagram widgets through 80 deltas and completion, with
late-session work rejected. These checks produce a debug executable, not a fresh
release installation bundle.

## Native desktop notifications

**Settings → Sessions → Desktop notifications** controls the existing shared
`desktopNotifications` preference, off by default. The native app uses
[GIO notifications](https://docs.gtk.org/gio/class.Notification.html), with alerts
for new ordinary conversation completions/failures and agent input requests when
the application has no active window. GNOME notification policy and Do Not Disturb
can suppress presentation. A click activates the application and withdraws the
alert; it does not answer an approval or send a message. Approval details and
response content are excluded from the notice body. Router controller SDK activity does
not produce ordinary conversation completion alerts. New Router job results now
use authoritative job summaries to notify completion/failure or request
clarification, with the same opt-in, foreground suppression, translated titles,
opaque controller ID, replacement and withdrawal rules. Prompt, response and
backend error content are excluded from these notice bodies.

Router alert admission follows live active → terminal transitions. Initial
terminal history, historical-page navigation, repeated replies, rejected stale
controller requests, stopped/deleted jobs and unconfigured controllers do not
create alerts. An accepted new delivery can also finish immediately. Watches are
bounded to 256 active jobs (the gateway's 64 × four limit) with a 512-entry
finished ledger in memory. The latest per-controller summaries already supply
statuses; no extra rich replies or hidden conversation trees are loaded. Only
watches outside the latest summary window add status-only reads, behind existing
queue/input gates within the shared 128-request, two-worker polling limit.

The updated private-bus regression `notifications-ui-m0eod03k` records seven
expected GIO notices across ordinary work, Router completion/failure/clarification
and approval requests, with no historical/stale/duplicate/stopped Router alerts.
It verifies controller-independent watches, unchanged hidden reply trees and
conversation/nonempty Unicode draft, plus the existing restart
activation/ledger/withdrawal checks.
The Router fixture also passes `router-ui-sipyaj9o` through real gateway summaries,
Codex/OpenCode fake deliveries, history navigation, shared unread state and
restart without replay. `router-ui-0nxv_cvb` also passes durable queued playback,
cross-backend delivery gates, paused restoration and explicit resume. The
notification and Router tests now await the shared
asynchronous GTK capture helper; premature synchronous captures had been quitting
the test app before its report was written. Screenshots remain real mapped GTK
captures, with no forced widget allocation. These are development-build checks;
the frozen optimized context candidate and default installation are unchanged.
The native suite passes 291 tests (two ignored), including transition admission,
metadata reuse, bounds and queue-priority checks. All native release targets pass
with Rust 1.92; formatting and generated-catalog checks also pass. The
`router-alert-validation/VALIDATED.json` QA record retains source/executable
hashes and both the completed workflows and the premature-capture diagnostics.

`GET/PATCH /studio/native/notifications` requires authentication and a trusted
origin. Its revision covers only the boolean. The bounded, atomic Rust preference
transaction preserves unrelated/future values and rejects stale revisions without
retrying the mutation. Invalid saved types remain untouched until an explicit
toggle or **Disable** repair. The native controls roll back after errors and offer
Reload; pending saves guard Settings closure. An enabling change takes effect
after acknowledgement; a pending disabling change suppresses new alerts.

Connection generations and bounded lifecycle/request records reject stale and
duplicate candidates. History loading and idle state alone do not notify. Per
backend/session IDs replace existing notices rather than accumulating one per
turn. A bounded ledger in `gnome.json` retains at most 32 IDs, coalesces writes
through the native preferences worker, and allows disabling to withdraw alerts
issued before a normal restart. Startup merging retains newer live IDs within
the same cap; activation before ledger loading cannot resurrect the clicked ID.

`scripts/smoke-gnome-notifications.py NATIVE_DEBUG_BINARY GATEWAY_BINARY` uses
fake providers, a private profile and a private D-Bus notification service. It
captures actual GIO transport while injecting canonical lifecycle events at the
native wire-event entry point. Two launches verify opt-in, duplicate/stale/idle
suppression, one approval dialog for a replayed request, explicit denial, click
activation without answering, stale-setting rollback/Reload, persistence,
bounded ledger merging, withdrawal and D-Bus activation after process exit.
`--api-only` needs no compositor or PyGObject. The full private-bus test needs
`dbus-run-session` and system Python PyGObject; these are test dependencies only.
`/usr/bin/python3 scripts/test-support/gnome-install-activation.py` checks both
installed launch paths using a disposable Gio application on another private bus.
Neither fixture sends test alerts to the user's notification service.

The original notification checkpoint passed with four expected notices and
owned-backend cleanup. Sampled peaks were about 338/308 MiB native and 40 MiB gateway across the
two launches. The Settings screenshot was inspected. Real GNOME banner/notification
center display, Do Not Disturb, physical focus/input-serial activation, real
provider alert delivery and a fresh release installation remain acceptance gates.
GNOME denied unattended foreground activation in this fixture; its result
explicitly records that physical-focus acceptance remains manual.
This checkpoint passes 191 gateway tests, 95 native tests (one ignored in each
Rust suite), all 748 web tests, and both optimized configuration checks. The full
native Wayland regression also passes using the real 4,097,226-byte report,
including source/edit/table/compare, conversation, workspace, Settings, restart
and owned-process cleanup checks. These checks refresh debug executables; they
do not produce a fresh release bundle or complete the migration's remaining
acceptance gates.

## Native session visibility

**Settings → Sessions → Session visibility** edits hidden folder paths and ordered
directory ignore rules, with explicit Save/Reload and native discard confirmations.
The folder chooser appends a path without saving. Scope is the session list and
Router candidate discovery; existing conversations, history, drafts and filesystem
permissions are preserved. Hidden folders match their descendants at path
boundaries. Rules support `*`, `?`, `**`, bracket classes/ranges and negation,
comments, escaped leading `#`/`!`, and later-rule precedence. Windows drive/UNC
paths match without case distinctions. Visibility paths use the existing browser
normalization; shared document access paths retain their separate literal-path
policy.

`GET/PATCH /studio/native/session-visibility` is authenticated and origin-checked.
A revision covers only `hiddenSessionDirectories` and `sessionDirectoryIgnore`;
the bounded Rust preferences transaction preserves unrelated/future settings.
Conflicts and rejected saves retain editor contents. Limits remain 256 hidden
folders, 512 rules, 4,096 bytes per entry without control characters, a 2 MiB
preferences document, and 4 MiB budgets for expanded regex source and the compiled
regex set. GET returns the original bounded projection and a `validationError`
when saved values cannot be applied. The native list stays empty until valid rules
load. Save validates only the proposed replacement before comparing the original
projection revision; malformed old fields do not prevent an explicit repair.

Readable invalid lists remain editable. Unsafe values (wrong types, control
characters, lines over 4,096 bytes, or more than 4,096 presentation rows) show an
explicit **Replace…** action. Replacement is confirmed in a native dialog and
changes memory only; Save writes the corrected values. Reload/cancel can keep the
original saved projection. Unedited fields retain their original arrays, including
blank ignore entries. Dirty checks are cached per field rather than repeatedly
copying whole text buffers during control updates. Invalid JSON, non-object or
over-2-MiB preferences documents still report an error and remain untouched.
Reverting an edit restores the loaded validation warning. Closing an editor with
an open replacement dialog cancels that dialog without retaining a mutable GTK
callback borrow; the recovery fixture exercises closing and reopening this state.

The native list and Router now reuse `session_directory_policy.rs`. It also fixes
the Router's missing bracket-class support and aligns path normalization with the
web client. The same 33-case conformance fixture runs against browser JavaScript
and both Rust clients. Malformed glob classes are ignored as in the web client;
Rust-specific regex set operations are escaped rather than gaining new meaning.
Class endings are precomputed in one pass, and class fragments are assembled
without compiling an individual regex for each fragment. Expanded-source
admission rejects excessive work before the combined regex compiler runs.

List search, rule matching, activity projection and stable pin ordering run on one
Rust worker. A direct watch channel coalesces replacement requests; source rows
use shared immutable snapshots. GTK applies only prepared results. Sequence
guards discard superseded results, including during backend changes. There is
one active computation and one replacement request, with the existing two-thread
CPU pool. The loaded catalog is capped at 10,000 entries, with an explicit notice
and disabled further paging when incomplete; sessions are not deleted.

`scripts/smoke-gnome-session-visibility.py NATIVE_DEBUG_BINARY GATEWAY_BINARY`
uses private profiles/fake providers. `--api-only` needs no compositor.
`--large-catalog` pages a 10,030-session source through the real UI, exercises
visibility changes and measures responsiveness/RSS. Two launches test Save/Reload,
parent/busy/dirty close guards, cancel/discard, validation, stale conflicts,
ordered unignore, history/draft retention, stale list responses and restart.
Actual folder-chooser portal completion remains a manual gate.

`--recovery` starts with malformed folders and a one-MiB rule line, verifies that
neither is sent to text layout, exercises replacement/cancel/discard, checks saved
bytes until Save, then repairs a readable 513-rule list. A second launch verifies
persistence. Cold-start conversation loading keeps its existing Router-registry
gate; repairing the configuration refreshes registry metadata and releases the
pending ordinary conversation automatically. The private provider asserts that
reads/resumes do not bypass an unknown registry. History/drafts remain intact
during subsequent invalid-rule reloads. API checks additionally cover control
characters, wrong field types, stale revisions and over-budget filter expansion.

The recovery checkpoint passes 189 gateway tests, 93 native tests (one ignored in
each Rust suite), all 748 web tests, two-launch recovery and ordinary large-catalog
Wayland checks, plus optimized configuration checks. Recovery sampled about
328 MiB native and 44 MiB gateway peak RSS. The normal 10,000-session run retained
34–37 ms maximum pagination heartbeat gaps. The full application regression also
passes with the real 4,097,226-byte report opening in 902 ms with 56 opening ticks;
sampled workload peaks were about 452 MiB native and 222 MiB gateway. These debug
checks verify owned-process cleanup and lazy browser/EPUB startup. Fresh release
packaging and the broader feature/manual acceptance gates remain pending.

The bracket-rule Wayland run paged 10,000 of 10,030 sessions in 4,648 ms, with
312 heartbeat ticks and a 36 ms maximum main-loop gap. The restored run took
5,950 ms with 433 ticks and a 38 ms maximum gap. The measurements exclude later
PNG capture. Sampled workload peaks were approximately 347/335 MiB native and
42/41 MiB gateway. Actual history and an unsent draft remained intact throughout
visibility changes. These are debug synthetic-catalog measurements, not physical
input or release-bundle acceptance.

The earlier session-visibility checkpoint passed 187 gateway tests, 92 native tests (one intentionally
ignored in each Rust suite), all 748 web tests, and optimized configuration checks.
The full Wayland regression also passes on a private copy of the real
4,097,226-byte `latest/tests.review.md` report: open took 902 ms with 56 opening
heartbeat ticks; sampled full-workload peaks were approximately 453 MiB native
and 221 MiB gateway RSS. Source anchors, editing/conflicts, tables, comparisons,
conversations, workspace tools, settings, restart and owned-process cleanup pass;
browser/EPUB engines remain lazy. No fresh release bundle was produced by these
checks. The broader migration and existing manual gates remain active.

## Native Files

The Files pane now uses GTK's [TreeListModel](https://docs.gtk.org/gtk4/class.TreeListModel.html)
and [TreeExpander](https://docs.gtk.org/gtk4/class.TreeExpander.html). Only expanded
folders request their children; collapsed descendants do not trigger recursive
filesystem scans. Standard GTK tree keyboard navigation, row activation and
virtualization apply. The project chooser establishes the root. Selecting a
different conversation updates a visible Files pane to that session's project;
reopening the same project preserves its loaded tree.

`POST /studio/native/files` validates project-relative paths and performs
enumeration, canonical confinement, metadata reads, ignore matching, filtering,
sorting and cursor hashing on Rust blocking workers. The service admits two
concurrent scans and returns 200 entries per page. Busy read requests have finite
backoff retries; mutations elsewhere do not acquire retry behavior. Scans retain
at most 10,000 matched entries within a 4 MiB metadata-weight budget and check a
two-second work limit between entries. Filesystem calls themselves may take
longer; this is not a hard I/O deadline or a total-memory limit. Incomplete scans
show an explicit notice. Cursors include the listing revision: a directory change
rejects a stale page with HTTP 409, and the UI refreshes rather than appending a
different snapshot. Requests have a 32 KiB body limit.

The mature [`ignore` 0.4.33](https://docs.rs/ignore/0.4.33/ignore/) library
(Unlicense/MIT, Rust 1.88 minimum) applies `.ignore`, `.gitignore`, Git excludes
and global Git ignore configuration. Hidden and ignored files have independent
native visibility controls. The literal case-insensitive file-path filter
preserves trailing whitespace and applies inside expanded folders, retaining
directory rows so users can navigate further. It is not a whole-project index.
Unicode and Unix backslashes remain literal paths. Directory symlinks are omitted
to avoid cycles; file symlinks remain usable within the project or configured
shared directories. Other targets are excluded.

Configured shared directories now appear as separate lazy groups with read-only
badges. `/studio/native/files/roots` canonicalizes/deduplicates their paths and
coalesces nested roots so the tree has one identity for each shared directory.
The selected project remains the request root when browsing/opening a shared
file. Relative paths belong to the project; shared groups retain absolute entry
paths, including a project encountered beneath a shared parent. This avoids
colliding tree identities and preserves the existing policy: files inside the
selected project are writable; configured shared files outside it are read-only.
Roots refresh on pane reopening, explicit Refresh, and shared-folder settings
Save/Reload.

**Settings → Files → Shared folders** now provides native path entry, a GTK
folder chooser, removal, and explicit Save/Reload. Paths are not trimmed; Unicode,
spaces and unavailable mounts remain intact. A typed path must be added before
Save. The list supports at most 256 absolute UTF-8 paths of 4,096 bytes each.
Closing or reloading with edits requires a native discard confirmation; requests
in flight protect closure. Conflicts/errors retain edits. Removing a shared root
revokes the gateway's access and refreshes the Files root inventory without an
application restart. Existing project permissions still take precedence.

`GET/PATCH /studio/native/shared-directories` uses the normal authenticated,
same-origin gateway boundary. A revision covers only the shared-folder array, so
unrelated/future preferences survive concurrent saves. Bounded document reads,
validation, revision checks and atomic persistence run on Rust workers under the
common preferences lock. Save updates live access policy after persistence;
explicit Reload also installs trusted external settings changes without writing
the file. Appearance uses the same bounded preferences-document I/O helper.

`scripts/smoke-gnome-shared-settings.py NATIVE_DEBUG_BINARY GATEWAY_BINARY` checks
authentication/origin denial, validation, unrelated-value preservation, stale-save
rejection, live grant/revocation and external-change reload. Two private Wayland
launches verify actual add/remove/Save/Reload controls, cancel/discard and parent
close guards, conflict edit retention, Files root refresh and restart persistence.
This fixture does not invoke the folder chooser. The separate
[file portal fixture](#native-file-chooser-and-portal-handoff) checks its actual
D-Bus request/completion path; physical GNOME chooser interaction remains a
manual gate. `--api-only` runs without a compositor.
The shared-settings checkpoint passes 183 gateway tests, 88 native tests (one
intentionally ignored in each Rust suite), all 747 web tests, both two-launch
settings fixtures, the broad Wayland regression, and optimized configuration
checks. The broad run used the generated 20,000-row, 2,828,960-byte table fixture:
opening took 306 ms with 19 opening heartbeat ticks; sampled full-workload peaks
were approximately 444 MiB native and 166 MiB gateway RSS. These are debug
measurements, not fresh release-bundle or physical-input acceptance.

The context menu's Show/Open in Files and Copy path actions use native GTK action
parameters scoped to the current view generation. `/studio/native/files/location`
revalidates the item against current project/shared configuration before returning
its canonical path and access class. Native file-manager handoff uses
[`GtkFileLauncher`](https://docs.gtk.org/gtk4/class.FileLauncher.html): files use
`open_containing_folder`; directories use `launch`, with the current native window
as parent. Directory launches use a readable descriptor even for writable
projects: GTK's writable-launch flag opens the target with `O_RDWR`, which fails
for a directory. Workspace access classification remains independent of this
transport flag. File launch options retain their access class. No shell
command or manually assembled file URI is used. The application does not impose
its review permissions on unrelated external applications.

Transient menus keep their parent/action context until GTK finishes activation.
[GTK's menu-item implementation](https://github.com/GNOME/gtk/blob/main/gtk/gtkmodelbutton.c)
closes the popup before activating its inherited GAction. Detaching the popup
synchronously from `closed` removed that context too soon: a real menu action
could disappear or encounter a native object-lifetime failure. Automatic cleanup
now runs once on the main-loop idle queue, holding the popup until dispatch
returns. Explicit pane/scope teardown still detaches immediately so shutdown does
not depend on another event-loop iteration. Acceptance checks GTK's actual popup
mapping and rendering without supplying a test-only allocation.

GTK retains at most 20,000 entry references; File visibility → **Reset file tree**
releases loaded descendants. There are at most two active directory requests,
128 queued paths and 64 expanded directories for native automatic updates.
Filter/visibility changes coalesce before reads. Unchanged row objects and
expanded paths survive live refreshes; an obsolete project/filter response cannot
replace the current tree.
Clicking Load more during a pending refresh retains the pagination intent.
Replacement pages are staged before reconciling the displayed rows, keeping the
loaded page extent intact while the same listing is refreshed. Folder page
targets share the bounded loaded-folder budget. Hiding the pane also invalidates
pending reads, clears queued paths and drops staged pages; already-running
gateway filesystem calls finish within their ordinary scan/I/O constraints.

Native automatic updates use `/ws/native-files`, sharing the existing `notify`
watcher with the browser's project-only `/ws/workspace-files` route.
Subscriptions cover expanded directories without recursive watches, coalesce
notifications, refresh after setup to close the initial enumeration/watch gap,
and discard old-generation events. Changing ignore files also refreshes expanded
descendants. OS watch validation/setup/replacement/cleanup now run on blocking
workers for both clients; the browser protocol and listing response stay
compatible. Hiding Files stops its subscription; connection failures reconnect
after five seconds, while unsupported/invalid subscriptions retain explicit
Refresh recovery. Pending watch controls coalesce under channel backpressure.
Native subscriptions revalidate configured shared roots and canonical targets
before emitting activity. Revocation or a changed target closes the subscription;
the UI refreshes its root inventory. A real authenticated loopback WebSocket/OS
notification regression verifies shared changes, revocation and connection
cleanup without a compositor. The legacy browser subscription stays project-only.

`scripts/smoke-gnome-files.py NATIVE_DEBUG_BINARY GATEWAY_BINARY` uses private files
and fake providers. The API checks cover authentication, nested confinement,
ignore precedence, Unicode/literal paths, pagination and stale cursors. The actual
Wayland tree checks cover lazy expansion, live creation/rename/deletion, retained
expansion, hidden/ignored toggles, multi-page folders, literal whitespace and
obsolete-watch isolation. Assertions inspect the current GTK model, not objects
retained by GTK's offscreen row cache. `--api-only` runs without a compositor;
`--large-folder` adds a 10,020-file API scan and a 10,021-file native folder.
API checks now also cover shared root deduplication, outside-root/symlink denial,
read-only review and rejected saves, validated locations and configuration
revocation. `--shared` opens the actual shared tree row in the native Markdown
reader and checks preserved project context, disabled Edit, live shared changes,
context actions and the read-only GTK launcher configuration. Its private launch
probe constructs GtkFileLauncher but deliberately does not launch the user's file
manager. Actual GNOME file-manager/portal completion remains a desktop acceptance
gate. Add `--headless` to use a private Mutter compositor with real input:

```bash
python3 scripts/smoke-gnome-files.py NATIVE_DEBUG_BINARY GATEWAY_BINARY --shared --headless
```

The fixture opens the menu using Shift+F10 and activates its actual Copy path
control with Return. It then right-clicks a different shared file and copies that
menu's path. An independent compositor clipboard transfer verifies both exact
canonical paths, including spaces, Unicode and `#`; GTK's own clipboard read is
checked too. The old unfocused test's `No compatible transfer format found` result
did not establish a user-facing clipboard failure. The replacement obtains real
Wayland input serials and lets GTK handle a held key press before its release, so
popup grabs are not attempted with an already-released synthetic key's serial.
The driver verifies the compositor's parent PID and D-Bus owner and never reads
the user's desktop clipboard. The shared debug input handshake is also used by
the EPUB clipboard fixture. Physical desktop clipboard acceptance remains open.
Heartbeat measurements exclude the fixture's intentional PNG readback/encoding;
they do not establish physical-input, accessibility, portal or release parity.
The broad real-report regression also exercises these controls before source,
Git, terminal, document, settings and restart checks.
In the final debug Wayland large-folder run, the API returned 200 of its retained
10,000 entries from 10,020 source files in 196 ms end to end (194 ms scanning),
with an explicit incomplete-scan notice. Opening the 10,021-file native folder's
first page took 427 ms, including the initial watch/setup refresh. The workflow's
16 ms heartbeat advanced 233 times with a 26 ms maximum observed gap after the
initial snapshot; PNG readback/encoding is excluded. The test forces Load more
while a refresh is pending and verifies the larger page extent eventually
appears. Sampled peaks across the native workflow were approximately 319 MiB
native and 51 MiB gateway. These private synthetic debug measurements do not
establish release, physical-input or whole-machine memory guarantees.
This checkpoint passes 88 native tests, 180 gateway tests (one intentionally
ignored in each Rust suite), 747 web tests and optimized native/gateway checks.
The final broad Wayland and restart regressions pass after the pagination and
pane-close fixes. The real 4,097,226-byte report opened in 902 ms with 56 opening
heartbeat ticks; tables, source anchors, comparison, explicit editing/conflicts,
conversations, terminal, settings and owned-process cleanup pass. The complete
debug workload sampled approximately 454 MiB native and 191 MiB gateway peak RSS.
Browser and EPUB web-content processes remained lazy until their tools were
explicitly opened. No fresh release bundle was produced by these checks; the
remaining application parity and release gates still apply.
The shared-scope increment passes 182 gateway tests, 88 native tests (one
intentionally ignored in each Rust suite), 747 web tests and optimized build
checks. Shared API/save-denial and real OS-watch revocation checks pass. The
existing broad Wayland regression passes with the real report opening in
1,052 ms and 65 opening heartbeat ticks; sampled full-workload peaks were about
454 MiB native and 164 MiB gateway. The large-folder baseline with configured
shared roots also passes: the native 10,021-file folder's first page took 433 ms
with a 16 ms maximum workflow heartbeat gap after snapshots. Direct popup
snapshots confirm the native context-menu rendering. The expanded shared-action
fixture now passes actual keyboard/pointer menu activation, both clipboard
transfers, read-only tree/reader activation and launcher configuration. It does
not exercise an external file-manager/portal launch. Those remaining desktop
gates are not counted as passed by the broader regressions.

The shared-action checkpoint passes 159 native Rust tests (one external-engine
test ignored), the optimized native compile check, the owned-compositor Files
keyboard/right-click fixture and the EPUB selection/clipboard fixture. Both menu
and read-only reader screenshots were inspected. The Files workflow sampled peak
native/gateway RSS of 338,136/46,464 KiB and a 32-ms maximum workflow heartbeat gap,
excluding screenshots. These debug measurements include fixture work and do not
prove smooth physical input. The four-launch application regression also passes:
the real 4,097,226-byte report opened in 1,053 ms with 64 opening heartbeat ticks;
sampled full-workload native/gateway peaks were 509,136/176,592 KiB, excluding
external workers. Browser and EPUB engines stayed lazy until opened. No fresh
release installation bundle or remote CI run is included in this checkpoint.

## Native file chooser and portal handoff

The main menu's **Open file…** action (Ctrl+O) opens local documents, tables,
images, code and plain text. Its format filter includes a separate **All files**
choice; suffix matching remains case-insensitive. The existing toolbar document
control still returns to the reader. The chooser title, accept label, filter names
and main-menu tooltip follow the application language. Nonlocal/non-UTF-8 paths
and errors keep the current document available and show an explanation.

The application chooser and Markdown reader share a small Rust request-ownership
controller. Each owns one cancellable `GtkFileDialog` request at a time. Repeated
Open requests coalesce; the reader control is disabled while its selection is
pending. For the application action, explicit file opening, viewer presentation, workspace navigation and
window unmap/close invalidate its pending request before cancellation. A result
must still own the current request before it can change the document. Opening another document, hiding/closing the reader,
window-close handling and reader destruction cancel pending selection. An old
cancellation callback cannot clear a newly opened chooser. Local UTF-8 paths
are required; invalid/nonlocal selections and chooser errors leave the current
document available and show an explanation. Selected replacements use the normal
unsaved-edit confirmation. Shared-folder selection uses the same request-identity
guard and still writes settings only after explicit Save.

The default GNOME desktop chooser is also covered without a portal emulator or
application hooks:

```bash
python3 scripts/smoke-gnome-desktop-portal.py NATIVE_BINARY GATEWAY_BINARY
```

This host test creates its own Mutter Wayland display, D-Bus/AT-SPI services,
profile and fake providers. It uses the installed GNOME portal configuration,
verifies the private-bus owners/executables for `xdg-desktop-portal`,
`xdg-desktop-portal-gnome` and Nautilus, and interacts with the actual Nautilus
chooser. It requires Ubuntu GNOME's Mutter/Nautilus/portal packages, system
Python with PyGObject/AT-SPI, and `dbus-run-session`. These Python dependencies
belong to the test harness, not the application runtime. No portal backend is
replaced and no forced widget allocation or application test action is used.

Seven chooser calls cover Open cancellation, opening a Unicode/space/`#` Markdown
path, shared-folder cancellation/selection with explicit Save, and original-image
Save cancellation/selection. The suggested Unicode image filename is retained in
a private export directory; its bytes must match the source exactly. A nonempty
Unicode composer draft and all source files remain unchanged. Shared-folder
selection stays in memory before Save, and unrelated settings survive that Save.
The fixture closes preferences through their actual controls and uses ordinary
keyboard activation for preference rows without an AT-SPI action.

The development pair passes `desktop-portal-544nkk1l`; the frozen optimized
context pair passes `desktop-portal-4g7opkoo`. The latter records all seven chooser
calls, exact source/export hashes, real service identities and unchanged unrelated
settings. No model turn or application WebKit content process is started, and
owned providers are reaped. Sampled native/gateway peaks for this small-document
workflow were 383,776/46,116 KiB (development) and 359,368/27,884 KiB (optimized);
these omit the external GNOME desktop-service processes and are not whole-desktop
memory figures. Host packages were Mutter 50.1, Nautilus 50.2.2, portal 1.21.1 and
GNOME portal 50.0 on Ubuntu 26.04. The
`desktop-portal-validation/VALIDATED.json` QA manifest retains completed runs,
source/executable hashes and the initial automation diagnostics. Existing
optimized packages and the default installation are unchanged.

This proves local default-GNOME chooser transport and the listed normal controls
on the owned desktop. Physical pointer/touch, fractional scaling, remote/GVfs
locations, sandboxed application deployments, browser-specific chooser flows and
file-manager launching still need their own acceptance. The private portal
emulator below remains necessary for malformed/late/error responses and request
ownership/lifecycle cases that a normal chooser cannot deliberately produce.

The optional private Wayland fixture crosses the actual GTK and D-Bus APIs:

```bash
python3 scripts/smoke-gnome-file-portal.py NATIVE_DEBUG_BINARY GATEWAY_BINARY
```

It owns a Mutter compositor, session bus and a narrow desktop-portal emulator.
Eighteen calls cover user cancellation, a local Unicode/CRLF document, an empty
successful selection error, a nonlocal URI, duplicate Open, explicit document
replacement, cancellation/reopen in the same main-loop turn, reader unmap/window
close, shared-folder selection/Save/close, and two actual GtkFileLauncher
completions. The application cases also cover Rust/source opening, filename-based
GtkSourceView syntax detection in both source buffers, dirty editor Cancel/Discard,
same-turn reopening and two deliberately late successful portal replies. Seven
held requests receive Close; the test waits for actual transport completion before
shutdown. All requests carry exported
Wayland parent identifiers. The handoff service inspects received Unix file
descriptors, verifies their canonical private targets and readable access modes,
and returns success without launching an external file manager. The project
directory retains its writable workspace classification; the shared file stays
read-only. Original source hashes and the unsent chat message are checked.

The emulator follows the
[FileChooser](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.FileChooser.html)
and [OpenURI](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.OpenURI.html)
interfaces. Its chooser replies include an empty URI array on cancellation,
matching the URI lookup expected by
[GTK 4.22's decoder](https://github.com/GNOME/gtk/blob/4.22.4/gtk/gtkfilechoosernativeportal.c).
The production UI uses the modern GtkFileDialog API. GTK's
[local-file transport](https://github.com/GNOME/gtk/blob/4.22.4/gtk/gtkopenuriportal.c)
explains the directory descriptor requirement. GIO is restricted to local files
inside this fixture to avoid unrelated volume/network discovery; production GIO
configuration is not altered.

The expanded portal fixture passes on Ubuntu 26.04 Wayland in
`file-portal-ui-trmt5kru`, with eighteen calls, seven cancellations and both late
responses. It samples peak native/gateway RSS of 375,152/23,024 KiB, excluding
compositor/desktop services. Original private file hashes and chat draft remain
unchanged, and no provider turn is started. Its source screenshot was inspected.
These debug observations include fixture work and are not a performance claim.
Actual GNOME chooser presentation, external file-manager behavior, desktop
permissions/activation and physical input remain acceptance gates. The emulator
does not establish installed desktop-service compatibility or release packaging.
The reader screenshot was inspected with its actual document workspace mapped.
The current checkpoint passes 191 native Rust tests (two ignored), Rust 1.92
checking and optimized compilation. The four-launch application regression passes
in `run-r6hy8sjm`, including a 4,097,226-byte real report opening in 1,075 ms with
65 GTK ticks, paging/editing/conflicts, comparison, diagrams and viewer returns.
These private-run observations include fixture work rather than isolated timings.

The refreshed optimized pair passes installed `--all-diagrams --language
--backends --git --viewers` acceptance in `release-ui-k1spuu2f`, without application
test hooks. It sends actual Ctrl+O through the owned Wayland compositor, selects
a Unicode-named Rust file through GTK's portal and verifies its content, unchanged
bytes, All files filter and retained chat draft. Existing Source/Edit Find,
explicit saves, native diagrams, language, backend, Git and viewer cases pass;
owned providers/content processes are cleaned up. The bundled native hash is
`344bfc87d26a6b693ae2106d740c709348aa7ff82c9a416b4723245349b4dfe7`;
the gateway remains
`2d1435f540b8fd4d10afa1fd9a5094b8be4e418407b21bbaafac243d5ba7745a`.
Both match their optimized build outputs. Physical desktop-service behavior and
fresh-machine/cross-platform acceptance remain migration gates.

## Native images

The authenticated `/studio/native/image` service performs file resolution, reads,
hashing, decoding, EXIF orientation, resampling and SVG rasterization on Rust
blocking workers. It reuses the existing project/shared-directory confinement;
a clicked absolute path never grants a new read root. Native file choosers and
launcher file selection establish the selected file's parent, as with Markdown.
Existing browser image routes and model attachment preparation are unchanged.

The service uses `image` 0.25.10 with only PNG/JPEG/WebP/GIF codecs and the same
resvg 0.48.1 stack already used for native diagrams. Both dependency manifests use
MIT/Apache-2.0 licenses and fit the project's compiler requirement. Decoders apply
dimension/allocation limits; raster inputs are capped at 25 MiB, 32,768 pixels per
side and 16 × 1,024 × 1,024 pixels total. The 128 MiB decoder allocation limit is
best effort, as [documented by image](https://docs.rs/image/latest/image/struct.Limits.html),
and is separate from caches, worker buffers and process RAM.

SVG source is capped at 2 MiB/16,384 XML tag markers and intrinsic dimensions
of one million per side. SVGs rasterize to a bounded preview without allocating
the intrinsic-sized image. Small vectors render at the preview resolution so
enlarging them does not reuse a tiny raster. Scripts are not executed; image href
resolvers do not read files or fetch network resources. Bounded embedded bitmap
resources now use the [shared Rust resolver](#embedded-bitmap-resources-in-svg).
The pure Rust renderer and native
controls require no browser engine.

All previews fit within a 2,048-pixel edge. A small binary header carries original
and preview dimensions followed by premultiplied RGBA; GTK validates the complete
byte count on a worker and constructs a texture without running an image decoder.
GTK paints validated pixels without running data-URL parsers or image codecs.
Immutable compressed source leases can remain attached to the presentation for
original-file saving; encoded URLs never become giant image-widget links. `100%`
means one preview pixel per logical display pixel; larger sources show both
original and preview dimensions. Copy copies the displayed preview. JPEG
orientation is applied before sizing. GIF/APNG/WebP use the bounded animation
player described below; still previews remain available when animation exceeds
its budget. Remote-image loading, color-management parity and higher-resolution/
tiled viewing remain migration work. Ordinary embedded image URLs are supported below.

The viewer also offers **Save original image…** through the native file chooser.
It preserves the original PNG/JPEG/GIF/WebP/SVG bytes, format, metadata and
resolution; Copy continues to copy the displayed preview. File and inline preview
responses carry a SHA-256 source identity and MIME type in response headers,
without changing the `STIMG001` pixel body shared with PDF rendering. The
authenticated `/studio/native/image/original` endpoint revalidates the current
project/shared-directory access, file-size budget and digest before returning
bytes. A changed source returns HTTP 409 and asks the user to reload. It does not
decode or resample the file, and never writes a client-selected destination.

An admitted inline save leases its immutable compressed source before the worker
starts, so closing the viewer or evicting its history cache cannot change the
saved content. File reading, transport, digest verification and temporary-file
writing run on Rust workers. The destination is replaced only after the digest
matches and the temporary file is synced. GTK receives source references and
completion status, rather than compressed source buffers. Saves support local
absolute destinations; remote chooser results receive an explanation. These
transient leases, writes and transport buffers are separate from the cache limits
below, not a total memory bound.

Only one unconfirmed Open/Save choice is retained per viewer. Closing, replacing
or reloading the image cancels that choice; a late reply cannot open a file or
start a write. Confirmed writes continue independently of the viewer, with at
most two admitted saves and one per displayed-image generation. Closing the
application waits for them. Failures use the shared native export-failure queue,
retaining the original image title and destination after the viewer is closed or
replaced. Completion of an older save cannot overwrite the current image status.

`scripts/smoke-gnome-image-save.py NATIVE_DEBUG_BINARY GATEWAY_BINARY` exercises
twelve real SaveFile requests and four OpenFile requests on an owned Wayland
compositor/private portal. It covers cancellation, five deliberately late replies,
duplicate choices, nonlocal/empty results, main-window close, a changed source,
real destination failure, and confirmed writes surviving viewer replacement,
close and inline-cache eviction. PNG, SVG and GIF outputs match their source
bytes exactly; existing rejected destinations and source files remain unchanged,
and temporary files are cleaned up. Screenshots verify the actual mapped
controls and failure dialogs. The final debug run sampled native/gateway peak
RSS of 403,364/94,520 KiB; this complete-workflow observation excludes worker
peaks and does not establish a memory cap or physical-portal acceptance. No model
turns are sent. API checks also cover authentication, foreign origins, digest
validation and revoked symlink access. Physical portal and remote-filesystem
behavior remain separate desktop acceptance gates.

The matched optimized frontend/gateway package also passes the installed-release
fixture's real accessible Save action and private SaveFile request, saving the
original 3,000 × 1,000 PNG byte for byte. The gateway's full test suite passes
205 tests with one ignored; the native suite passes 168 with one ignored, and
Rust 1.92 checking passes. The PDF API and four-launch application regression
remain passing. After updating the history-image fixture to use the production
explicit-navigation coordinator, the 10 MiB source case also verifies visible
thumbnails, native viewer controls and cache restoration on both providers.
The older fixture's direct `ListView::scroll_to` bypassed this coordinator: a
queued follow-to-tail could unmap the lazy image while the fixture waited for it.
The check now keeps the intended row visible and still requires loaded pixels
to intersect the viewport; it does not change production scrolling or relax
the visibility assertion.

The gateway admits two image requests concurrently and keeps up to eight
content-addressed frames within a 32 MiB raster cache. File access is revalidated
and content rehashed before a cache hit. The shared native preview service admits
two in-flight requests and at most 32 pending/active callbacks. Unmapped or
replaced widgets discard queued work and late replies; a running request keeps
its slot until it finishes. Offscreen paragraph previews release textures while
retaining their measured aspect ratio. The standalone viewer retains one current
texture until close/replacement. These budgets do not represent total app RAM.

Up to four local image references per Markdown paragraph receive lazy previews;
remaining images retain clickable alt links. Source line/byte anchors remain on
the original Markdown block. Relative reader links resolve against the document's
directory; conversation/Favorites/history previews retain their source project.
Remote URLs remain explicit external links, without automatic downloads.

Codex `image`/`localImage` inputs and OpenCode image file parts now have their own
flat virtualized attachment rows. Worker projection decodes inline data URLs into
a process-private source store and replaces them with opaque random references.
The existing `lru` crate bounds compressed sources to 64 MiB/8,192 entries across
views; exact byte comparison prevents hash collisions from aliasing sources.
The bounded history tail is projected in display order so newest sources remain
most recent in this cache. There are at most 16 preview references per message.
OpenCode's text-part cache retains only image metadata, preserves it across text
updates, and handles image file parts in older history pages. Unavailable or
remote sources get an explanation; no remote image request is started.

Mapped thumbnail rows request rendering through the same two-request native
lane. `/studio/native/image/inline` accepts authenticated binary raster sources
up to 10 MiB and reuses the existing Rust decoder, dimension limits and 32 MiB
frame cache. Frame keys now use SHA-256. Source decoding/lookup, upload copying,
pixel decoding and frame validation stay on workers; raw bytes travel only from
the native worker to its owned gateway. Local attachment paths retain project/
shared-directory checks and never establish a new read root. Inline previews
open with the existing fit/zoom/copy viewer. Unmapping cancels callbacks and
releases GTK textures; evicted source references explain that history must be
reloaded. Transport, pending composition inputs, active workers, raster frames
and source cache overhead are separate memory costs; none is a process-RAM cap.

### Bounded image animation

The gateway now has worker-side GIF, animated WebP and APNG decoding through its
existing pinned `image` 0.25.10 codecs. `gif` 0.14.2 is an explicit dependency on
the already-resolved codec, used to read GIF loop metadata without scanning
compressed pixel data. No new dependency version is introduced. GIF repetitions
are normalized to total plays, including one play without a loop extension;
APNG/WebP play counts retain their decoder semantics. Zero denotes indefinite
playback. Decoders provide composited canvases; Rust applies orientation, resizes,
and premultiplies alpha before transport. GTK does not decode these formats.

Animation is opt-in. Existing file requests and inline/document responses remain
`STIMG001`; static images also retain that response when animation is requested.
To request a timeline, use `animation: true` in the JSON file request,
`X-Studio-Animation: 1` on `/studio/native/image/inline`, or `animation=true` on a
snapshot-owned Markdown image URL. The existing `original=true` envelope retains
the original compressed bytes followed by the chosen preview format. Source
identity headers, revision checks, original-byte saving, authentication, origin
policy and project/shared-directory confinement remain in effect.

`STIMA001` is a little-endian binary envelope: eight magic bytes, then six u32s
(oriented original width/height, preview width/height, frame count, total plays).
Each frame has a u32 delay in milliseconds followed by one full premultiplied RGBA
canvas of the declared preview dimensions. Delays are rounded up and have a
20-ms minimum; long delays are preserved. There are no offsets, paths, URLs,
compressed codecs or partial canvases for GTK to interpret.

Each admitted timeline has at most 128 frames, a 768-pixel preview edge and
16 MiB of frame/delay bytes. Source and best-effort decoder allocation bounds
remain those of the image service. A three-second cooperative budget is checked
around frame processing; it cannot interrupt a single codec operation. Excess
animation work returns the ordinary still preview, with
`X-Studio-Animation-Limited: 1`; corrupt sources still fail if the still decoder
also rejects them. A limited result is not cached as a permanent animation
failure, since a cooperative deadline can be transient. No partial timeline is
published. The existing two worker slots and shared 32-MiB cache remain in use,
with distinct still/animation
entries sharing the same original SHA-256. Decoder working buffers, temporary
serialization and process RSS are separate from the retained timeline limit.

The image API fixture now exercises all three formats through confined files,
inline bodies and long projected Markdown image sources. It checks delays,
premultiplied pixels, play counts, exact original envelopes, default still
responses, cache separation, authentication/origin guards and closed readers.
The projected-source probe initially used an ineligible short data URI; its
corrected fixture uses a long ignored URI fragment while decoded source bytes
stay unchanged. Small embedded images continue through the ordinary inline path.

The native client now requests timelines through all three source routes and
validates dimensions, count, byte length and delays on Rust workers before any
GTK object is created. A shared GTK player uses a fixed-size `GdkPaintable`:
changing a frame invalidates its contents without replacing the picture or
requesting layout. Immutable frame buffers are shared with `GdkMemoryTexture`;
codecs and packet parsing do not run on GTK's main thread.

Play/Pause/Replay controls appear beside the viewer controls and inline image
captions. Automatic playback follows GTK's reduced-motion setting; explicit Play
can override it for that presentation. Finite loops stop on their final frame,
and Replay starts at frame zero. Timers stop outside the observed scroll viewport
or when unmapped; unmap/Source/close cleanup releases timelines and textures.
Retained presentations keep a manual pause choice across reloads. Recycled chat
rows may create a new presentation. Copy takes an immutable snapshot of the
currently displayed frame; Save retains the original encoded animation. The
limited-animation control is disabled and explains that a still preview is shown.

Private Wayland checks cover viewer GIF/APNG/WebP pixels and play counts, infinite
playback, explicit controls, reduced motion, live translated controls, hidden
window suspension, stable paintable identity, current-frame clipboard pixels and
close cleanup. The real conversation factory checks scroll suspension/resume and
unmap release. A projected Markdown GIF checks Source/Preview cleanup, expansion
to an owned original source and exact-byte saving after the reader replaces its
snapshot. A 129-frame GIF remains readable through the still preview. Physical
compositor occlusion/minimization and live-provider acceptance remain unverified.
The installed release is unchanged.

The transport foundation's gateway suite passed 245 tests (one ignored), including GIF cropped-canvas
and Previous disposal, all three formats' timing/play counts, orientation, long
delays, premultiplied alpha, resizing and rejected frame/byte/time budgets.
Actual HTTP run `images-ui-jxadjexu` passes file/inline/projected-source transport
and original/snapshot guards. The still-image Wayland regression
`images-ui-jjb547as` passes viewer Fit/zoom/Copy, original identity, source-mode
transitions, SVG resource isolation, conversation factories and texture cleanup.
Its image-dialog capture now waits for natural GTK mapping/paint rather than
trying to capture an unmapped sheet. No production layout is forced.

Gateway and native-client Rust 1.92/release checks pass. The legacy Tauri desktop
check stops at missing `gdk-3.0.pc` in this GTK4 development SDK, so it is not a
successful desktop-build verification. That environment acceptance remains open.
Five owned API/GUI runs, initial fixture/capture/compiler diagnostics, source and
binary hashes, the focused lockfile delta and unchanged installed-package hashes
are retained in
`/media/rui/studio-gnome-deps/qa-artifacts/native-smoke-jwfgtzto/animation-transport-validation/VALIDATED.json`.

The playback checkpoint passes 288 native tests (two ignored) and 246 gateway
tests (one ignored), plus both components' Rust 1.92 and release-profile checks.
Final Wayland run `images-ui-jk8kdpdq` passes the animation controls, reader/chat
lifecycle and original-save checks above; its sampled native/gateway peaks are
447,752/142,092 KiB across 136 samples. Still-image cold/steady open times are
1,131/986 ms, with maximum observed main-loop gaps of 367/16 ms; these include
the previously investigated cold graphics initialization, and do not measure a
large-animation throughput ceiling. The four-launch application regression
`run-owtfsobg` passes against the actual 8,340,599-byte report, including document,
edit, restored-session, terminal and chat-diagram checkpoints. Screenshots were
inspected. The existing GtkBox height-for-width warnings also occur in the prior
still-image checkpoint and remain unresolved.

One development fixture switched back to Preview before Source's worker request
was ready; its corrected readiness wait passes without changing production mode
handling. Four owned image runs and the application run, including that failed
fixture, logs, source/binary hashes, focused baseline copies and unchanged package
hashes are retained in
`/media/rui/studio-gnome-deps/qa-artifacts/native-smoke-jwfgtzto/animation-playback-validation/VALIDATED.json`.

### Embedded bitmap resources in SVG

`crates/svg-resources` is a reusable GTK-independent Rust library shared by the
gateway's file/inline/document image decoder and the native diagram/vector-zoom
renderer. It uses [usvg's resource hook](https://docs.rs/usvg/0.48.1/usvg/struct.ImageHrefResolver.html)
with `resvg`'s raster-image feature. PNG, JPEG, GIF and WebP data resources now
appear inside SVG previews. JPEG orientation is applied; GIF/WebP use still
images. File paths, relative hrefs and network URLs remain unresolved, and SVG
scripts do not execute. Nested SVG/SVGZ resources are still omitted.

The existing `image` 0.25.10 codecs decode on bounded Rust workers with dimensions
checked before pixel allocation. A streaming, byte-limited PNG encoder supplies
canonical RGBA PNGs to resvg; original compressed codecs/metadata never reach its
second decoder. Identical resource bytes share canonical PNGs within one parse,
while every occurrence still charges rendering work. The original SVG, its
SHA-256 and original-file Save remain unchanged. No temporary files or browser
engine are involved.

Per-parse limits are 64 resource attempts, 16 megapixels across bitmap
occurrences, 32,768 pixels per bitmap side, and 8 MiB of canonical PNGs across
unique admitted resources. The decoder receives the existing best-effort 128 MiB
allocation limit. A budget failure rejects the complete image before rendering;
invalid or unsupported bitmap data is omitted. Callers retain their SVG source,
element, output-pixel, queue and cache budgets. These limits do not cap total
process RSS or all codec-internal allocations. See the
[library contract](../crates/svg-resources/README.md).

Five shared tests cover rendered PNG/JPEG/GIF/WebP, alpha, JPEG orientation,
duplicate normalization and occurrence charging, resource/streaming byte limits,
and inert nested vectors/unknown resources. Gateway tests verify that rendered
bitmap pixels retain the original SVG's file identity and exact original bytes.
Native diagram tests retain bitmap pixels through vector zoom and propagate
resource-budget failures. The private image fixture checks actual API pixels,
dimension/count budget failures, exact originals, private-file denial and
zero requests to a live private HTTP canary. The GTK fixture independently
downloads the displayed RGBA texture and checks the embedded bitmap's pixels;
its readiness/heartbeat measurement excludes that debug readback and capture.

Final Wayland run `images-ui-z8ibf2io` passes actual SVG bitmap pixels in the
viewer, source/Preview transitions, conversation images, cache/late-response
guards and invalid-image recovery. SVG readiness took 301 ms with 18 GTK
heartbeat ticks and a 16 ms maximum gap; its API SVG request took 269 ms.
The initial PNG/app-start phase still observed a 370 ms gap, separate from SVG
rendering. Sampled native/gateway peaks over this debug image workflow were
447,352/142,856 KiB. Its private-file check targets an existing absolute PNG path
and samples inside the would-be letterboxed image, rather than an unaffected
corner. No HTTP canary requests or model turns occurred. The SVG screenshot was
inspected. These are workload observations, not general latency/RAM guarantees.

The four-launch full-app run `run-gtxyj65a` passes the real 4,097,226-byte report,
including source lines, table paging, editing/conflicts, comparison, all diagram
families, both providers and restart. Report opening took 950 ms with 57 opening
heartbeat ticks. Table-image run `table-images-v7xzpajb` retains viewport admission,
exact original Save/comment lines and modal viewer handoff. All 534 unit tests
pass (205 native, 227 gateway, 61 Markdown, 36 MarkNoria, five SVG resources), with
the three existing runtime-dependent checks ignored. Rust 1.92 checking passes.

The matching optimized package passes installed `release-ui-jamaitlm` with all
five optional workflow groups, without application hooks or a gateway override.
Native controls open the SVG bitmap file, save its exact original through a
private SaveFile portal, close it and return to the document with the composer
draft retained. Existing diagram inspection, editing, languages, backend
configuration, Git and viewer checks remain passing. Sampled peaks over the
complete installed workflow were 640,896/108,460 KiB native/gateway; its two lazy
web-content processes belong to browser/EPUB content. Optimized API run
`images-ui-32dkmqcw` also passes bitmap pixels, dimension/count rejection, the
absolute-file/HTTP denial checks and exact originals, with a 30 ms SVG response.
Fresh-machine, physical GNOME/portals, real-provider and remote CI gates remain.

- Native SHA-256: `90ffa3fdc3f386906909dcab6dd90f8d71b12e6f7fc4a43ab96d7817c84e45f0`.
- Gateway SHA-256: `a5c1dc018477bea557671770be10c00492b9c430942b35906646f3f675393534`.

### Embedded image URLs

Markdown paragraph images can now use base64 or percent-encoded `data:` URLs for
PNG, JPEG, GIF, WebP and SVG. Shared Rust parsing produces a MIME-tagged immutable
byte lease and bounded alt text. The decoded bytes are included in block/cache
weight accounting; debug output includes only MIME and byte count. The original
Markdown, source line/byte anchors, alt-text selection and safe surrounding links
remain unchanged. Invalid/non-image data gets an explanation. Relative local
images retain their existing workspace confinement.

The shared decoder uses pinned [`data-url` 0.3.2](https://docs.rs/data-url/0.3.2/data_url/)
for Fetch-standard MIME parsing, percent decoding and forgiving base64 behavior.
It bounds encoded URLs to 30 MiB plus a 1 KiB header, limits decoded data to
10 MiB per image and rendered paragraph, and applies a 2 MiB SVG limit. The
canonical base64 form uses the existing `base64` block decoder with a checked
allocation bound. Missing padding, escapes, whitespace, parameters and fragments
continue through the bounded standards decoder. This shared implementation also
prepares history attachments into their existing process-private LRU references;
it does not change model send payloads or enable remote downloads.

The **512 KiB compact Markdown formatting window remains in effect**. Oversized
inline image URLs now use the snapshot-bound projection described below, retaining
the original file's coordinates. Independent history attachments retain their
existing 10 MiB cap. Embedded bitmap image-hrefs in SVG now use the
[shared bounded resolver](#embedded-bitmap-resources-in-svg). Nested SVG/SVGZ and
external resource coverage remain in scope.

Previews reuse the two-request Rust image service and content-addressed pixel
cache. Native controls open an image directly from its immutable source, without
a temporary file or a browser. **Save original image** leases those same bytes,
verifies the returned SHA-256 and atomically writes the native chooser destination;
zoom/resizing cannot substitute a preview. Unmapping/replacing the source releases
its preview texture and rejects retained hidden Open buttons. An already opened
viewer keeps its independent source lease until close/replacement.

The native endpoint now accepts SVG bytes through the same bounded Rust SVG
rasterizer as local files. Actual content is checked independently of the data
URL's claimed MIME. HTML/scripts are not rendered, and file/remote image href
resolvers stay disabled. Authentication, origin, source-size, element and pixel
limits remain enforced. The MIT notice and pinned upstream checksum are included
in the native package.

The checked-in decoder comparison can be reproduced with:

```sh
cargo run -p studio-markdown-native --example embedded_image_bench --release -- 10 7
```

Each case returns the same immutable byte type, verifies every result and rotates
execution order after warming all cases. One 10 MiB/7-iteration run observed median
4.638 ms for existing base64, 16.422 ms for the direct standards decoder and
4.569 ms for the bounded shared decoder with the common-form path. This isolates
URL decoding/allocation; it excludes image codecs, transport and GTK layout.
These are local measurements, not a universal throughput guarantee.

`scripts/smoke-gnome-embedded-images.py NATIVE GATEWAY` uses private providers,
Mutter Wayland and native file portals. Run `embedded-images-7ttubslu` verifies an
88,428-byte Markdown source with a 65,752-byte PNG and percent-encoded SVG in chat
and reader, native viewer/zoom, byte-exact original PNG/SVG saving, stale-action
rejection, texture release and preservation of the unsent composer draft. HTTP
canaries receive zero requests, and no web-content process starts. Screenshots
were inspected. The debug workflow observed 763/339 ms chat/reader preview readiness,
37 main-loop ticks, a 469 ms maximum heartbeat gap across startup and forced
snapshots, and native/gateway peaks of 451,420/60,740 KiB. These are complete-fixture
observations rather than memory or responsiveness caps.

The shared-source 10 MiB regression `history-images-yfxpxm3s` passes both Codex and
OpenCode history, visible thumbnails, native viewer controls and cache restoration.
It observes 1,674 ms first-thumbnail readiness including captures, 179 main-loop
ticks, a 64/35 ms maximum gap in the Codex/OpenCode history phases, and native/
gateway peaks of 490,188/243,436 KiB. Startup and screenshot costs are reported
separately by the fixture. No model turn is sent.

The four-launch real-report regression `run-9ob69_5a` passes with the actual
4,097,226-byte Markdown report, including columns/paging, explicit editing,
conflicts, undo/redo and source-line mapping. Its document open took 904 ms with
56 main-loop ticks while opening. The native and gateway Rust suites pass 203
and 226 tests respectively (two and one normally ignored); all 52 shared parser
checks pass. These validations do not complete the remaining application parity
or fresh-host/physical/provider acceptance gates. Rust 1.92 checking, interface
catalog generation, formatting and fixture syntax checks pass.

The preceding ordinary embedded-image checkpoint refreshed its matching optimized
pair in `/media/rui/studio-gnome-deps/current-release/package`, including the pinned
`data-url` MIT notice and existing optional runtimes. Its recorded hashes and runs
below are historical; the large-source checkpoint follows. Installed acceptance
`release-ui-0t_umnun` passes `--all-diagrams --language --backends --git --viewers`
with application hooks disabled. It verifies the existing editor/portal/diagram/
language/backend/Git/viewer workflows, lazy web-content startup and owned cleanup;
the debug image fixture above separately covers the new data-image actions.
The packaged gateway also passes the extended image/SVG API cases as
`images-ui-gusmqadw`. Packaged hashes match their respective build outputs:

- Native SHA-256: `3b989cd6fca3e7793ddd2da801ffbe08eaacadc90cd3b80f20a32aa5e2dc6db4`.
- Gateway SHA-256: `9a53cb2c63abf9c4f0296d1682eeef2d1f31ccdced723d4a2826cf167a75b41e`.

### Large embedded image sources

A large inline data image is now projected at the native API boundary. The indexer
already caches its alt text instead of its URL; its source-byte guard now accounts
for recognized image URLs separately at block completion. Actual event-count,
HTML/cache limits and the 128 MiB source limit remain enforced. Oversized ordinary
text and code still fail explicitly. No source file is rewritten by this change.

The native `fragment` and `table-source` endpoints read the immutable indexed
snapshot on a single admitted Rust projection worker. Data-image URLs over 8 KiB
become compact private references. Validated substitutions retain the exact
original byte spans and physical line counts, including multiline captions and
continued table headers. Captions retain their original Markdown formatting,
inline code and literal TeX. Source/Edit remain ordinary paged original text.
The shared quote locator consumes the same projected blocks, so comments do not
acquire synthetic byte positions or lose their source lines.

The shared `memchr` candidate scan skips projection parsing when a fragment cannot
contain a large data image, retaining mixed-case scheme support. Its pinned MIT
notice is included in the package. Per-fragment admission is capped at 128
substitutions, 512 KiB of compact source and 128 KiB for repeated table headers.
An indivisible source range is bounded by the existing 128 MiB document cap.
Backend image work remains limited to two requests. These are work/data budgets,
not a total process-RAM guarantee.

Only mappings produced for an actual source image become native document image
references. A literal `studio-doc-image:` URI cannot acquire that authority.
Requests retain reader ID, source revision, page and image ordinal; an optional
document header must match. The service verifies the current document/revision
before and after work, rejects expired/closed readers, and decodes through the
existing bounded image service. Thumbnail requests transfer pixels only. Opening
the viewer additionally returns an immutable original in the private `STIMS001`
envelope; the native worker verifies its size and SHA-256 before enabling Save.
The frozen byte lease survives closing/replacing the document. Existing image/PDF
`STIMG001` bodies are unchanged.

The vendoring adapter reproduces the pinned MarkNoria `lib.rs` and `server.rs`,
including source-budget regression checks and snapshot document identity. Parsing
grammar and original offsets are unchanged. The standalone/browser reader retains
its existing inert image rendering; this new transport is native-specific.
File-wide reference definitions now resolve through the bounded backend path
below. Image display inside table cells remains separate native rendering
coverage in earlier checkpoints; the native table-image path is documented below.
A data URI is still subject to the existing 10 MiB decoded-image and 2 MiB SVG caps.

`scripts/smoke-gnome-embedded-images.py NATIVE GATEWAY --large-source` verifies
private API and actual GTK/Wayland/native chooser behavior. Run
`embedded-images-o55th96t` uses a 13,981,772-byte source containing a 10,485,760-byte
PNG. The image-containing fragment is 536 bytes and its thumbnail body 65,560
bytes. It verifies unchanged Source text/file, original-coordinate captions and
quote lookup at line 3, revision/document/origin/authentication guards, closed-reader
rejection, byte-exact PNG/SVG saves, and a PNG save after reader closure. No HTTP
canary request, model prompt or web-content process starts. Screenshots were
inspected. This debug run observed 1,233 ms reader preview readiness, 148 main-loop
ticks, a 480 ms maximum gap across startup/snapshots and native/gateway peaks of
445,416/128,956 KiB. These observations do not establish release performance caps.

The final four-launch real 4,097,226-byte report workflow passes as
`run-v5ja05fo`, including existing native table paging/columns, editing, save
conflicts, undo/redo and source-line mapping. It opened in 1,002 ms with 62 loading
main-loop ticks. Comment-location run `comment-location-u2qjkplv` separately verifies
Source/Preview relocation, ambiguity/missing-text fallbacks, the dirty-edit guard,
and the real report at original line 10,494. Native/gateway/shared parser/indexer
tests pass 204/226/56/35 checks respectively, and Rust 1.92 checking passes.
Full migration gates remain open.

The matching optimized pair is packaged at
`/media/rui/studio-gnome-deps/current-release/package`, preserving the pinned
PlantUML/D2/PDFium runtimes and including the new `memchr` MIT notice. Installed
run `release-ui-yll2vdt8` passes `--all-diagrams --language --backends --git
--viewers` through the actual launcher/accessibility/portal controls, without
application hooks or a gateway override. It covers explicit editor/source saves,
all four diagram formats, live language/question controls, backend JSON, Git
review, source search/edit/find, and image/source/table/PDF/EPUB viewer returns.
Web-content processes appear only after intentionally opening EPUB; they and the
owned providers are reaped on closure. Native/gateway sampled peaks are
609,100/77,884 KiB, with a separate observed web-content peak of 242,100 KiB;
these are observations of that workflow, not memory ceilings. Optimized image API
run `images-ui-cbq78doi` verifies source identity, byte-exact originals, changed-file
rejection and existing PNG/SVG bounds. The new large-data-URL GUI actions above
remain separately covered by the debug fixture.

Retained native fixture artifacts now live under
`/media/rui/studio-gnome-deps/qa-artifacts/native-smoke-jwfgtzto/runs`.
Completed run entries under `target/gnome-native-smoke` link individually to
that archive, preserving recorded paths. New active runs use the local directory:
putting their SQLite profiles on the external disk inflated startup latency and
pushed the broad regression past its 75-second fixture timeout. Completed runs
can then be moved to the archive with their individual links preserved. The
archive move recovered approximately 12 GiB on the system filesystem; selected
original-image/report/log hashes were verified across both layout changes. Build
caches and the current package remain on the external disk.

Packaged binaries match the optimized build outputs:

- Native SHA-256: `f7fb28a003ed083a80298941c60fec66a36c2e79e0f067f72f817c3440997fcd`.
- Gateway SHA-256: `8fe7f671505e8ea05d73463538f3362846a94261fc814c2ac547e7fc480e290b`.

### File-wide Markdown references

Native reader pages now resolve reference-style links and images whose definitions
are outside the requested source window. The existing pulldown-cmark index pass
writes canonical destinations/titles to private `references.bin` and only
label/offset/length tuples to `references.json`. Neither destinations nor the
reference table enter the public document/outline index. The binary stream counts
toward the existing 512 MiB cache budget. Source text is unchanged, including
reference definitions inside containers and ignored lookalikes inside code.
The vendoring adapter reproduces this indexer change and its regression test.

A single metadata cache is keyed by immutable cache path and revision. Rust
workers load at most 8 MiB of serialized metadata, 32,768 definitions and 128 MiB
of canonical reference bytes. They seek only requested definitions, reusing each
within the request. A formatted window carries at most 256 definitions and
128 KiB of combined context; ordinary destinations/titles are capped at 8 KiB
apiece. Continued table headers retain their own original source ranges and share
the same context/duplicate checks. These admission limits are explicit errors,
not silent dropping of definitions or a total application-RAM guarantee.

The shared native parser uses pulldown-cmark's full/collapsed/shortcut label
handling and `unicase` matching, including Unicode case folding and normalized
whitespace. The first definition in the full document wins over a local duplicate.
Inline links remain independent. External reference identity is resolved before
attaching destinations to emitted events: returning a giant URL from the parser
callback would consume the short input's expansion allowance and suppress later
references. Explicit context/event/image bounds apply to the native path instead.
The pinned `unicase` MIT notice is packaged alongside the other notices.

Large data-image references reuse the snapshot-bound image ordinal protocol;
their URLs stay off native transport. An ordinary link to that same definition
keeps its visible caption and an inert data destination. Formatting does not
append definitions to visible text or shift original bytes/lines. Existing scheme
checks, image resource isolation, request authentication, revision/closed-reader
guards and immutable-original Save behavior remain in force. This covers native
reader fragments and continued tables; ordinary whole-message chat parsing retains
its existing source/expansion limits. Table-cell image presentation is now shared
by reader and conversation tables as documented below.

`scripts/smoke-gnome-embedded-images.py NATIVE GATEWAY --large-source --references`
checks actual native link navigation and return, 14 MB source/10 MiB original
PNG, PNG/SVG saves, original coordinates, a continued 200-row table, equal-label
isolation between two readers, and the existing privacy/lifetime checks.
Adding `--api-only` runs the authenticated reference/image/table and source-line
checks against an optimized gateway without a compositor or application hooks;
this case is included in GNOME CI (remote CI has not yet run). The
final local-profile debug run is `embedded-images-brocm1mf`: 1,417-byte
fragment transport, 374 ms index readiness (328 ms index worker), 17 ms first
fragment, 704 ms reader image readiness and 786 ms chat readiness. It observed
146 main-loop ticks, a 489 ms maximum gap across startup/snapshots, and
native/gateway peaks of 449,668/184,768 KiB. The ordinary-inline control
`embedded-images-a8e1b_2v` retained its 536-byte fragment, 218 ms projection,
1,235 ms reader readiness and 752 ms chat readiness. These describe the two source
forms and this debug workload; they are not release latency/RAM ceilings.

The earlier external-profile run `embedded-images-4xaygkxv` passed the same
reference actions but took 9,744 ms to show chat previews; its ordinary-inline
control took 10,117 ms. Moving live SQLite profiles back to local storage restored
both to about 0.8 seconds, while old artifact paths remain linked to the external
archive. The broad external-profile run was stopped at its existing 75-second
fixture timeout; it is not counted as passing. No model prompt, HTTP canary fetch
or web-content process started in the passing image checks.

Final local-profile four-launch run `run-6pp8rizx` passes the real 4,097,226-byte
report, opening in 902 ms with 56 loading-loop ticks. Existing paged tables,
columns, Source/Preview/Edit, save conflicts, undo/redo, Unicode/CRLF and backend
cleanup pass. Comment-location run `comment-location-vfljy6cq` verifies moved,
ambiguous, missing, unchanged and legacy text, dirty-edit protection and the real
report at line 10,494. All 204 native/226 gateway/60 shared parser/36 embedded
indexer tests and Rust 1.92 checking pass. The wider native migration gates still
apply.

The optimized pair is refreshed at
`/media/rui/studio-gnome-deps/current-release/package`, with matching build hashes,
the `unicase`/`memchr`/`data-url` notices and the existing optional runtimes.
Installed run `release-ui-u6clte9k` passes all five optional workflow groups through
the real launcher/accessibility/portal controls, with application hooks disabled.
It retains editing, native Git review, diagram inspection, language/backend
settings, viewer returns, lazy content startup and owned-process cleanup. Sampled
native/gateway peaks are 610,340/76,888 KiB; intentional EPUB content processes peak
separately at 238,152 KiB. These are workflow observations, not total RAM bounds.
The compositor-free large-reference case also passes against this optimized
package as `embedded-images-wzvv6d5n`: 54 ms index readiness (40 ms index worker),
16 ms first fragment and 1,417 transport bytes, including source/quote checks,
200-row table headers, cross-reader label isolation and immutable originals.
The existing optimized image API passes separately as `images-ui-k9b29ovy`.
Remote CI, fresh-machine and physical/real-provider acceptance remain open.

- Native SHA-256: `754bf157524e42a49e8ff2357a8ea20de0f7f3cfe07088ea2a9ceca092ec7a13`.
- Gateway SHA-256: `f19547939e4b4d059f1ad26028c0090d12608592bdab3c597f6f0759670b60fa`.

### Native Markdown table images

Reader and conversation Markdown tables now reuse the same image service as
ordinary paragraphs. Cells show thumbnails in a fixed 120-pixel image area,
retaining row height while pixels arrive. The full-cell reading window shows
bounded 320-pixel previews and the existing image viewer provides inspection and
original-image Save. Long formatted text retains its existing bounded styled
pages. Native column-header context menus offer **Read column header** for full
formatting and images, preserving GTK's plain titles, column resizing and layout.
The shared cell reader also binds its interface text to the native language
catalog; captions, paths and source line numbers remain literal data.

Images nested inside another image's alt text contribute only caption text;
they do not get separate previews or decode jobs. Alternative-text extraction
continues through nested endings, retaining text after the inner image. Original
lookup consumes the same outer-image event span as projection, so the next
image ordinal cannot accidentally resolve to the inner image. A shared parser
regression checks distinct outer/inner/next bytes and original source coordinates.

Table preview admission is viewport-driven: a thumbnail must intersect every
ancestor scroller, horizontally and vertically. Adjustment value/range changes
and the image widget's actual allocation schedule one coalesced pass after GTK
layout. Allocation notifications matter when page replacement leaves scrollbar
ranges unchanged. There is no continuous polling or frame-clock callback.
Offscreen previews cancel queued callbacks and release
textures while retaining size. A newly visible cell joins the existing two-worker,
32-callback admission limit; large original bytes remain on bounded Rust workers.
Cell controls retain document/revision guards, and recycled unmapped controls
cannot reopen their images. Opening an image from a modal full-cell reader closes
that window before presenting the workspace viewer, which retains an independent
original-byte lease.

`scripts/smoke-gnome-table-images.py NATIVE GATEWAY` uses private files, fake
providers, an owned Wayland session and a private file portal. It exercises
portrait thumbnails, nested source originals, header/full-cell viewing, scrolling,
paging, exact comment lines, Source/Preview cleanup, stale buttons and original
Save after reader closure. `--api-only` checks nested ordinals, source identity,
header images and paged table source against optimized gateways without a
compositor; this is included in native CI. Remote CI and wider migration gates
remain open.

Actual Wayland run `table-images-2xybnj3w` passes reader/chat cells, PNG/SVG
originals, header menus, full-cell modal handoff, scrolling/paging, comment lines 3
and 53, Source/Preview cleanup and stale-action rejection. It observed 859 ms
chat readiness, 440 ms reader readiness and 148 ms page readiness including its
deliberate debug capture. Visibility passes and image callbacks each measured
less than 1 ms. The 707,471-byte fixture's table source response was 15,176 bytes.
Sampled native/gateway peaks were 431,796/70,256 KiB, with no web-content process
and no model turns. Captures were inspected. These are debug observations, not
release latency or total memory guarantees.

The final long-cell/comment/lifetime run `table-cells-ui-gq762qb2` passes styled
pages, exact Unicode selections, bounded/full-cell comments, revision guards and
source reopening after the shared cell-reader refactor. The four-launch full-app
run `run-zduv09hh` passes the real 4,097,226-byte report, native documents,
comparison, terminal, Files/Git, both providers, session tools and restart. Report
opening took 952 ms with 57 opening heartbeat ticks; sampled native/gateway peaks
over the full debug workflow were 590,996/173,568 KiB. All 527 unit tests pass
(204 native, 226 gateway, 61 shared parser and 36 embedded indexer), with the
three existing runtime-dependent checks ignored. Rust 1.92 checking passes.

The final 10 MiB history-image run `history-images-pjcwn7u_` passes both providers,
visible textures, cached restoration and native viewer controls. It recorded a
1,584 ms first thumbnail including debug capture, a 441 ms startup heartbeat gap
and a 249 ms cache-switch gap; those costs remain profiling work. The large-source
reference run `embedded-images-yzcmy3y3` passes exact originals and source lines
with a 13,981,860-byte file and a 1,417-byte first fragment, returned in 18 ms
after indexing. These measurements do not establish physical-input guarantees.

Matching optimized executables are packaged at
`/media/rui/studio-gnome-deps/current-release/package`, with the pinned optional
runtimes and notices retained. Installed run `release-ui-xzd0wkj8` passes all five
optional workflow groups through actual accessibility/Wayland controls, without
application hooks or a gateway override. It additionally opens a table image from
the native full-cell modal, verifies modal closure and returns to the table while
preserving the composer draft. The complete installed workflow includes EPUB and
browser content, so its sampled 624,208/76,040 KiB native/gateway peaks and two
web-content processes are separate from the Markdown-only no-web-engine result.
Optimized API run `table-images-8nor4b55` passes original identity, nested ordinals,
headers and paging. Fresh-machine, remote CI, physical desktop and real-provider
acceptance remain open.

- Native SHA-256: `ac7de7291d99e1477c090fe71e3deedd7b123386cf677b8b6ed8da105746e18d`.
- Gateway SHA-256: `4df9f28a2a4cd93633e0edb5c7c309ebaeab1af2301206de11bb467ec3d83855`.

`scripts/smoke-gnome-history-images.py NATIVE_DEBUG_BINARY GATEWAY_BINARY` checks
both providers' actual history, source-specific loaded textures, inline viewer
navigation, zoom/fit, metadata bounds, conversation switching and source-cache
restoration with private providers. API cases reject unauthenticated/foreign-
origin requests, non-image/HTML bytes, oversized bodies and excessive dimensions,
and verify SVG rasterization with private/remote image resources disabled.
`--api-only` runs without a compositor; `--large-source` adds a synthetic PNG with
padding to exercise a 10 MiB image source and its base64 history transport.
The GTK heartbeat report excludes deliberate debug PNG snapshot readback/encoding
and reports that overhead separately. First-thumbnail timing includes capture
overhead; these fixtures do not establish physical-input or release guarantees.
The final 10 MiB synthetic-source Wayland check verifies the specific attachment's
loaded pixels intersect the actual viewport on both providers and after returning
to a cached session; GTK's mapped overscan rows alone do not count. The first
visible Codex thumbnail took 1,649 ms including debug capture. The longest observed
heartbeat gaps in the image/history/viewer/cache-switch phases were 40/29/16/41 ms;
image callbacks took less than 1 ms, event delivery at most 1 ms and transcript
reconciliation at most 36 ms. A separate 448 ms gap occurred in the startup phase
before history selection. Cold startup still needs profiling; its underlying cost
has not been established. Across this complete debug workflow the sampled peaks
were approximately 450 MiB native and 200 MiB gateway. Compressed-source/cache
budgets are not total RAM limits. The API and owned-process cleanup checks pass;
no prompts are sent to real providers.

`scripts/smoke-gnome-images.py` uses private PNG/SVG files and fake providers to
check authentication, decoded wire dimensions/transparency, cache reuse, confined
symlinks, invalid/oversized files, native fit/zoom/copy, stale loads, source/preview
switching and actual conversation factories. Rust tests also cover JPEG EXIF,
GIF/WebP, allocation dimensions and bounded SVGs. CI runs `--api-only`; actual
Wayland checks exercise widgets and inspect screenshots. Real image libraries,
physical portals, color fidelity and high-DPI zoom remain manual acceptance gates.

The final debug fixture opened a fresh 3,000 × 1,000 PNG in 1,025 ms after the
window had initialized, including worker decode/resampling, transport and texture
installation. Its 16 ms heartbeat ticked 67 times, with a maximum observed gap of
16 ms, including a short first-paint settling interval. Opening the initial PNG
while starting the app took 1,114 ms and showed a 371 ms heartbeat gap; the latter
is a cold-start/rendering cost, not a steady-state responsiveness result. Sampled
peaks across the complete image fixture were approximately 407 MiB native and
131 MiB gateway. These synthetic debug figures are not release guarantees.
Optimized-build decode latency and cold startup remain performance acceptance
work. The shared fixture RSS sampler observes only the owned native/gateway
processes and records no command lines, environments or credentials.

## Native PDF

The native PDF surface follows live English/Chinese interface changes through
explicit weak UI bindings: toolbars, search placeholders and result page labels,
page/link limit notices, link destinations and hover text, password prompts,
and accessible names. A newly opened chooser uses the current interface language;
the portal's own chrome follows desktop settings. File names, outline headings,
extracted text, snippets, URLs, query/selection state and raw backend diagnostics
remain literal. Retiring a UI binding before assigning a diagnostic or URL also
protects equal-valued model strings from a later language switch.

The private Wayland PDF regression (`scripts/smoke-gnome-pdf.py`) now switches
English → Chinese → English with an open page, selected text, real search results
and mapped link rows. It checks fresh page/search replies in Chinese, preserves
the query, selection, mode and document identity, and switches an already-open
password prompt while retaining typed input and response IDs. Screenshots and
results from `pdf-ui-7gze79v7` confirm the translated controls and dialog;
existing paging, selection/Copy, comment capture, password retry and bounded
search-window checks also pass. The separate `pdf-links-ui-dhlfmwde`
regression passes with actual owned Wayland pointer/keyboard input, all four
rotation fixtures, and cancellation through a recording URI portal. This is
development-build evidence; the frozen
`20261103-context` package has not been replaced by this localization change.
The native unit suite passes (288 tests, two ignored), and all native release
targets pass `cargo check` with Rust 1.92. Formatting and the generated language
catalog checks pass. The `pdf-language-validation/VALIDATED.json` QA manifest
records executable/source hashes, the completed run artifacts, and the corrected
owned-compositor fixture invocation; no default installation was changed.

The authenticated `/studio/native/pdf` API opens immutable, memory-only snapshots
through the same workspace/shared-directory confinement as other documents. A
native chooser establishes the selected parent directory; model-provided links
cannot grant new read roots. The original PDF and password stay in Rust workers.
Password entry is not logged, persisted, or passed in process arguments or the
environment. The GTK process receives page metadata, bounded extracted text and
validated premultiplied RGBA pixels; it runs no PDF decoder or browser engine.

The mature [pdfium-render Rust wrapper](https://github.com/ajrcarey/pdfium-render)
loads the packaged [PDFium build](https://github.com/bblanchon/pdfium-binaries/releases/tag/chromium/8111)
only inside disposable worker processes. Build-time downloads use pinned x64 and
arm64 archive hashes and retain the upstream/third-party license notices. Each
worker has no network, home or project mounts, receives source bytes through
stdin, and uses a private 1 MiB tmpfs. Read-only system libraries/fonts are
available. Limits are 768 MiB virtual address space, 12 CPU seconds, 64 file
descriptors, zero core dumps and a 20-second parent deadline. Worker failure
becomes a document error; GTK and the gateway do not load the C++ engine.

Inputs are capped at 50 MiB, with at most four cached snapshots. Idle snapshots
are reclaimed on a subsequent open after ten minutes. Two requests can render or
search concurrently; Close has a separate lane. Native clients assign a UUID
before opening so Close can cancel even an unfinished first render. Closed IDs
remain tombstoned for 60 seconds, up to 128 IDs, to reject late Open requests.
Replacement/close cancels active workers and rejects stale callbacks. Raster
responses have a 2,048-pixel edge limit; at most 16 cached pages fit within a
combined 32 MiB raster/metadata budget. These are component bounds, not total app
RAM. PDF worker memory is additional to native/gateway RSS measurements.

Only the current page becomes a GTK texture. Contents are limited to 500 entries
and 16 nesting levels; bookmarks without a page destination are omitted.
Extracted page text is bounded to 256 KiB and large text pages are labeled or
skipped during search. Search preserves literal whitespace and scans at most 32
pages per request, returning up to 100 hits. Next results replaces the previous
window; incomplete extraction or hit limits are labeled as partial coverage.
Page navigation coalesces rapid clicks to the most recent requested page.
Typing a query during first-page loading defers it until the snapshot is ready.

The read-only Text view provides native selection, highlights and clipboard copy.
Preview also supports dragging from one text position to another, Copy/Ctrl+C
and source-linked comments. GTK maps the displayed image bounds to normalized
coordinates; the authenticated `select` operation asks the isolated PDFium worker
to hit-test its endpoints and return text and highlight rectangles. The same
PDFium device/page transforms used by rendering handle rotated pages. Native
widgets paint the returned rectangles and do no PDF decoding, text layout or
character indexing. Selection follows PDFium's reading order, which can differ
from visual order in complex layouts. It is limited to one page, 16 KiB of text,
16,384 selected character entries, 128 regions and a 64 KiB JSON response. Pages
with more than 128,000 character entries are rejected for selection. Work runs
only on release, rather than creating a worker for each pointer movement.

Selection requests have their own native generation/serial. Replacing a file,
changing pages or starting another selection invalidates a late response. Source
comments keep the shared PDF path/hash, one-based page and normalized region
schema, with an optional native `charRange`. Reopening checks the file hash and
re-extracts that range; only an exact quote match restores a Preview highlight.
Legacy anchors without a range use a uniquely matching quote in Text view; their
stored regions are not trusted as a verified selection. A persistent text tag
keeps that source highlight visible when closing the dialog changes primary
selection ownership. Changed/unverified files and mismatched ranges show the
page without claiming a verified region. Comment metadata occupies at most three
visible lines, including anchors containing all 128 regions.

Advanced PDF destination views/cross-file links, persistent PDF annotation objects, higher-resolution/tiled
zoom, reading-state persistence and export remain migration work. Physical mouse,
touch/drag and focus transitions still require expanded manual acceptance; the
automated checks exercise the coordinate-selection path and actual GTK controls.
This checkpoint does not edit PDF files.

PDF Preview now has native link hit testing and a **Links on this page** button.
The virtualized link list is also available in Text mode and supports keyboard
activation. Internal links resolve explicit and named page destinations through
PDFium; navigation retains the chosen Preview/Text mode. Hovering a link shows
its page or canonical URL and a pointer cursor. Only a press/release within the
same page/view generation and four screen pixels activates a link. Longer drags
continue through text selection; stale clicks cannot open a target after a page
or file changes.

The existing disposable PDFium worker reads link annotations while preparing a
page, so GTK receives validated data alongside the cached raster. It inspects at
most 2,048 annotations and returns at most 256 links, eight quad-region bounds
per link and 512 regions in total. Malformed entries do not hide later valid
annotations. Rotation/cropping uses PDFium's page/device transforms; quads retain
separate axis-aligned bounds so gaps between lines are not clickable. Exact
skewed-quadrilateral hit testing remains pending. Finite bounded rectangles,
page indices and URI targets are revalidated by both the gateway and native
worker projection. Limits/unavailable links are visible in the status and link
button tooltip, including on pages whose extracted text is also limited.

Only absolute HTTP(S) URIs without credentials/control characters are passed to
[GTK's asynchronous URI launcher](https://docs.gtk.org/gtk4/class.UriLauncher.html)
after an explicit activation. URLs are canonical and capped at 4 KiB. They remain
web URIs even when a localhost path ends in `.pdf`; they never grant a Studio file
read root. JavaScript, Launch, remote/embedded-file actions and other schemes are
inert. Hidden, nonviewable and read-only annotations are ignored; unsupported
fixed-size/nonrotating link flags are reported as unavailable. Pending URI
requests are cancelled on page/file replacement or close, and late completions
cannot alter the new page. Cancellation cannot revoke an application already
launched by the desktop. Exact XYZ/fit/zoom destinations, cross-file navigation
and unannotated plain-text URL detection remain separate parity work.

`scripts/smoke-gnome-pdf-links.py NATIVE GATEWAY --api-only` verifies metadata,
rotations/cropping, named/explicit/action destinations, quad gaps, disabled actions,
malformed annotation recovery and link/region caps through the sandbox. `--headless`
adds real native controls and Return-key/pointer input delivered only to its owned
compositor, retained view mode and stale-page/file guards. Its fullscreen test
verifies a sole 1600×1000, scale-one monitor before sending pointer motion; the
input helper validates the compositor process and unique bus owner. Actual GDK
click/drag recognition navigates a page link and selects its text; additional
native signal cases exercise moved/stale presses and Preview/Text transitions.
Its private [OpenURI portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.OpenURI.html)
records URI launches and cancellation; it launches no browser. The portal models
only the interfaces needed by this case, and GDK warnings about its unimplemented
Registry/Inhibit interfaces are fixture limitations. Physical mouse/touch,
assistive technology and actual GNOME/browser-handler behavior still need manual
acceptance. No model turns or source-document writes are performed.

The final private run (`pdf-links-ui-4_xnzo7a`) passes all link/region budget,
rotation/crop, malformed-entry, pointer, keyboard, selection, mode/replacement
and URI-cancellation checks. Both 256-link and 512-region limits are exercised.
Its inspected screenshot shows the native link list beside Preview. Sampled
peaks were approximately 428 MiB native and 133 MiB gateway, excluding disposable
PDFium workers; zero owned browser content processes were observed. These are
debug workflow observations rather than app RAM limits or release benchmarks.
The five-launch PDF comment regression (`annotations-ui-t4dfpyjq`), existing PDF
reader regression (`pdf-ui-3ewq6p6l`) and four-launch application regression
(`run-9bx5miuk`) pass after core link integration. Gateway/native suites pass
196/120 tests, one ignored each, and both optimized checks pass. Application logs
have no GTK criticals, panics or theme parser errors, apart from the limited
portal fixture's explicit GDK warnings described above. The host PDF CI job now
includes API-only link checks. Fresh release executables/install bundles and the
remaining desktop/platform acceptance gates are still pending.

`scripts/smoke-gnome-pdf.py` creates private ordinary, 70-page and password-locked
PDFs, including a rotated document, and uses fake providers. It verifies
authentication, raster/text/outline transport, search windows, case handling,
confined symlinks, invalid documents,
close-before-open rejection, selection range/coordinate bounds, forward/reverse
hit testing on ordinary/rotated pages and snapshot cleanup. Actual Wayland checks
exercise page controls, coalesced navigation, contents, selected-text Copy, dark
Text, password entry, Text-view comment capture, page hints and deferred search;
screenshots are inspected separately. `--api-only` requires just the gateway
executable and no compositor.
CI defines a separate host-runner PDF job because ordinary job containers do not
reliably expose Bubblewrap's user namespaces; that remote job has not yet run.

A local debug API measurement used Mozilla's real 1,016,315-byte, 14-page
[TraceMonkey reference PDF](https://github.com/mozilla/pdf.js/blob/master/web/compressed.tracemonkey-pldi-09.pdf).
First open took 391 ms; uncached pages 2, 8 and 14 took 326, 332 and 306 ms.
Literal search took 131 ms and returned 42 hits. Page responses were approximately
12.2 MB of uncompressed pixels/metadata, an explicit local transport cost. These
timings include worker startup, decoding/rendering and HTTP transport, and are
not release-build or arbitrary-PDF guarantees. No model prompts were sent.

A later selection check on the same real PDF opened in 198 ms, selected its
first six character entries in 21 ms and returned 247 JSON bytes. Repeating that
selection through its normalized point coordinates reproduced the quote. This
measures an actual reference document through the authenticated Rust service;
it does not measure pointer/paint latency or establish performance for all PDFs.

The final private Wayland fixture opened its initial PDF in 372 ms. A fresh page
after startup took 201 ms; the 16 ms GTK heartbeat ticked 18 times with a maximum
observed gap of 16 ms, including a short paint-settling interval. The cold-open
timer ticked only once, so that measurement does not establish startup
responsiveness. Sampled fixture peaks were approximately 391 MiB native and
151 MiB gateway, excluding disposable PDF worker processes. These debug figures
are observations, not whole-application memory or latency limits.

The five-launch PDF comment fixture passes Preview selection/Copy, stale page
rejection, save/insert/source reopening, restart, legacy 128-region anchors,
wrong ranges and changed files. Actual Preview and Text source highlights and
the native Comments dialog are inspected. The private selection took 50 ms with
three GTK heartbeat ticks and a maximum observed gap of 16 ms. Sampled peaks
were about 421 MiB native and 95 MiB gateway; disposable PDFium workers are
additional. No browser content processes or model turns start.

## Native EPUB

The authenticated `/studio/native/epub` service reads immutable ZIP snapshots on
bounded Rust workers. It uses the existing `zip` library and `roxmltree` for
container/package/navigation XML, with DTDs disabled and an XML node budget.
No archive is extracted to disk, and the GTK process receives metadata and
individual requested resources rather than a whole compressed book. Canonical
archive URI paths handle Unicode, spaces and escaped `#` characters. Manifest
membership, filesystem/shared-root confinement, duplicate names, traversal,
symlinks, encryption and per-entry/expanded sizes are checked independently.

Limits are two 128 MiB snapshots, 10,000 archive entries, 64 MiB per declared ZIP
entry and 512 MiB combined declared expansion. A delivered resource is at most
16 MiB; XHTML/HTML/CSS/SVG is capped at 8 MiB and markup at 100,000 `<` markers.
Package/navigation XML is capped at 2 MiB/100,000 XML nodes. Two worker requests
run concurrently, with a separate Close lane. A 16-entry/32 MiB resource cache
is shared across books. Native URI requests queue at most 64 resources and fetch
one at a time. Contents and spine metadata are capped at 2,000 and 5,000 entries;
contents truncation is explicit. These are component budgets, not app RAM bounds.

WebKitGTK 6 renders only chapter content, inside an ephemeral network session.
The book receives no gateway URL/token or filesystem mount. Custom URI delivery
is scoped to the current snapshot UUID. A response CSP blocks book scripts,
frames, objects, forms, network connections and remote image/CSS/font resources;
script markup is also disabled. The trusted CFI/layout helper and native message
handler run in an isolated script world. Book scripts cannot access that handler.
Permissions, popups and file selection from content are denied. Explicit HTTP(S)
link clicks can use the system browser. The engine's 512 MiB memory-pressure
setting is a monitored, best-effort renderer limit, not an OS memory quota or a
bound on all WebKit/native/gateway processes.

CFIs use the existing EPUB.js 0.3.93 implementation, bundled without its ZIP/book
loader. Its BSD license notice is packaged with the executable. Reading position,
text scale, paper theme, layout and contents visibility reuse Studio's existing
`epub-reading.sqlite3` and content hash. Snapshot-scoped monotonic reading
revisions prevent late saves from replacing a newer close-time update. Window
shutdown waits for a final position capture/save before stopping the gateway.
No document editing or persistent edit drafts are introduced.

The private API fixture passes EPUB2/EPUB3 navigation, Unicode resource paths,
reading-state reopen/stale-save checks, authentication, source confinement,
hostile archives, disabled scripts and oversized resource rejection. The current
Copy checkpoint passes 120 native Rust tests with one ignored and the
optimized-profile check; the four-launch application regression also passes.
The initial GTK checks render styled tables and Chinese/Arabic, verify response
CSP isolation and find matches. EPUB Copy passes on an owned headless Wayland
compositor with real pointer selection and pointer/keyboard toolbar activation,
plus WebKit's standard Ctrl+C. A separate process reads the compositor clipboard
over its private session, verifying exact Chinese/emoji/trailing-space content
and ownership retention; it does not read the user's desktop clipboard.
The previous failure came from synthetic actions with Wayland input serial `0`.
The existing controls/restart fixture now uses valid input and passes too.
Physical-desktop clipboard acceptance remains open.
Native section entry, contents navigation, paging, paper/theme/text size, hidden-view
position retention, rapid replacement and restart restoration now pass. The full
four-launch regression also passes with no eager WebKit content process.
Source-linked EPUB comments now use the native session library described below.
External CFI links, fixed/vertical layout parity and export remain unfinished.
The search bar retains **Search this section** and its rendered previous/next
matches. **Book…** opens native full-book search, described below.

Copy reads DOM selection in the isolated reader world and hands plain text to
GTK's clipboard. It admits one pending operation per view, requires a ready
surface and validates both book generation and document epoch before publishing
a result. In-section navigation, including navigation to a fragment of the same
resource, invalidates an old result; close also invalidates pending work.
The native UTF-8 bound is 2 MiB, independently of the helper's UTF-16 length
guard. Oversized selections leave the clipboard intact, and whitespace-only
selections are preserved. These bounds apply to the native Copy action;
WebKit owns its standard browser accelerator.

Run the focused owned-compositor acceptance test with debug executables:

```sh
python3 scripts/smoke-gnome-epub-clipboard.py "$NATIVE" "$GATEWAY"
python3 scripts/smoke-gnome-epub.py "$NATIVE" "$GATEWAY" --headless
```

The focused test also verifies late same-resource results, duplicate admission,
close isolation, the actual DOM helper's UTF-8 overflow and whitespace-only
selection. Both tests use private profiles/books/fake providers, check owned
renderer/backend cleanup and make zero model requests. A loopback canary verifies
that hostile book scripts, images and styles make zero network requests.
Observed debug peaks for the focused run were 369,784 KiB native, 44,088 KiB
gateway and 307,416 KiB combined sampled WebKit processes, including its 700,000
character overflow case. The controls/restart run opened its first book in 699 ms
and restored in 259 ms; these are local observations, not release budgets.
No optimized executable or install bundle was produced for this increment.

Full-book search uses html5ever 0.26.0's streaming tokenizer, the version already
in the workspace lockfile, with no DOM tree or browser execution in the gateway.
Character references, XML/SVG CDATA and inline text are decoded; ASCII layout
whitespace and block boundaries collapse to spaces. Head/title/script/style/
template/noscript/iframe/object/form controls and explicit hidden elements are
excluded. Search remains literal,
including punctuation and trailing spaces, with optional Unicode case matching.
It searches the immutable spine, including unopened sections. UTF-8/markup/depth
failures are shown as skipped sections rather than silently treated as no matches.
CSS-derived visibility, embedded metadata/alternate content and non-UTF-8 chapter
encodings can differ from rendered section search; exact CFI occurrence anchors
are not produced by this text index.

Each request scans at most 16 sections, admits up to 16 MiB of declared resource
bytes, and bounds each section/text at 8 MiB, 100,000 markup markers and 256
nested tags. One dedicated search worker leaves the existing resource/Close lanes
available. Snapshot closure is checked between 16 KiB tokenizer feeds and before
returning. The API returns at most 16 matching-section records and 16 explicit
issues. Match counts cap at 10,000 with a visible `+`; a snippet is at most 806
UTF-8 bytes. No whole-book text or index is retained or sent to GTK.

The native dialog accumulates fewer than 116 section rows before pausing for
**Continue search**; **Previous results** reruns its bounded snapshot range.
Unmatched batches continue asynchronously, with progress and a **Stop** button.
Stop rejects late replies; book replacement closes the dialog and generation/
serial checks prevent old results from appearing in a new book. Selecting a row
loads that section and highlights its rendered matches through the existing
section search. This is section navigation, not a claim that an extracted source
offset maps to a particular rendered occurrence.

`scripts/smoke-gnome-epub-search.py NATIVE GATEWAY --api-only` checks the Rust
API; `--headless` adds the real GTK dialog and WebKit match navigation using a
private Wayland compositor and fake providers. `--reference PATH` searches a
private copy of a cached real EPUB as well. The API fixture covers 140 sections,
Unicode/entities, inline tags, literal metacharacters, trailing spaces, case
matching, capped counts, invalid requests, closure and one deliberately malformed
section. It also checks six sections containing 24 MiB of filler in two bounded
requests. The reference run (`epub-search-ui-8fy4e77p`) searched Moby-Dick's 144
sections in nine requests, with 141 matching sections, and the WOFF sample in one
request; neither skipped a section. Maximum observed debug request times were
20/11 ms for the real books and 912 ms for the large-text batches. Sampled gateway
RSS peaked near 51 MiB across that fixture. These measurements include local API
transport and serialization, exclude GTK rendering, and are observations rather
than whole-application limits or optimized-build benchmarks.

The first private Wayland case (`epub-search-ui-liruvqwo`) passes bounded result
paging, case matching, skipped-section notice, Stop/late-reply rejection,
navigation to an initially unopened section with an actual WebKit found-text
signal, and book replacement isolation. An inspected screenshot exposed a desktop
toolbar wrap; the search entry's natural width is now constrained to keep the
action buttons together. Both search bars now use the shared native adaptive
toolbar, with Search/Stop grouped together. A later narrow capture exposed a
blank artifact from the old helper's forced allocation, so this case now waits
for focus on its owned compositor and captures GTK's mapped widget paintable
without allocating widgets or emitting frame-clock callbacks; blank pixels fail
the fixture. The final mapped run (`epub-search-ui-s4cvq1m8`) passes the actual
340-pixel toolbar bounds and both inspected desktop/narrow screenshots. Its first
result page arrived in 141 ms; 89 heartbeat ticks had a maximum 72 ms gap,
including screenshot encoding. Sampled peaks were about 363 MiB native, 45 MiB
gateway and 235 MiB summed WebKit content RSS. These include the whole private
workflow, not just the text index, and do not establish a responsiveness limit.

Gateway/native suites pass 195/118 tests, one ignored each, with the final XML/
excerpt tests rerun after the extraction changes. Both optimized checks pass;
the four-launch application regression (`run-3kcdf5z1`) also passes after search
integration. Its logs and the final search logs have no GTK criticals, panics or
theme parser errors. CI includes the API-only search case. The html5ever MIT
notice is included by the native package build script, and installation copies
that license directory. Fresh release executables/install bundles, physical
input/clipboard/portal checks and the wider migration gates remain pending.

Two real [IDPF samples](https://github.com/IDPF/epub3-samples/releases/tag/20230704)
were tested through the Rust API: `moby-dick.epub` (1,628,868 bytes, 144 spine
entries, 141 contents entries) opened metadata in 23 ms with a 27,545-byte response;
`wasteland-woff.epub` (433,966 bytes, one spine entry, six contents entries) took
5 ms with a 1,017-byte response. Actual GTK rendering also passes for both books:
Moby-Dick opens in 715 ms and navigates to spine entries 73 and 144; the WOFF
sample opens in 429 ms and loads all three embedded fonts. Images and stylesheets
render correctly. The Moby-Dick cold run records a 418 ms maximum GTK heartbeat
gap (an earlier run reached 521 ms), so cold-start responsiveness remains a
performance gate. These are debug observations, not release guarantees. Combined
WebKit-process peak RSS is about 243 MiB for Moby-Dick and 239 MiB for the WOFF
sample; shared pages can be counted more than once in summed RSS. The references
stay outside the repo.

## Native browser workspace

The application menu's **Browser** action opens a GTK4/libadwaita workspace with
native tabs, address entry, Back/Forward, Reload/Stop, page search, zoom and a
Downloads dialog. Ctrl+L focuses the address, Ctrl+F searches, and Ctrl+T/Ctrl+W
open/close tabs. **Comment on selected text** opens the shared native session
comment library with the page URL/title and quote. It supports explicit saving,
note editing, source reopening, prompt insertion and the ordinary acknowledged
submission workflow described below. Capturing a selection sends no model turn.
Opening its source loads the URL and searches for the quote; this is not a
persistent DOM selector or an exact position after a page changes.

The engine is constructed only after the authenticated Rust preferences request
succeeds. The existing HTTP/HTTPS, credential, localhost and private-network
navigation rules now live in a portable Rust module shared by both desktop
shells. Browser pages receive no Studio IPC handlers, gateway credentials, EPUB
resource scheme or local document capability. Each browser workspace has its own
ephemeral WebKit network session, separate from the EPUB engine. Hide keeps its
tabs; Exit cancels transfers/file dialogs, closes tabs and drops that session.
A page process failure stays visible until Reload. No automatic recovery loop is
introduced.

**Settings → Browser → Navigation and JavaScript** edits the shared Browser
enabled, HTTP, private-IP, localhost and page-JavaScript preferences with native
switches and explicit Save/Reload. Toggling a switch alone does not write the file.
Close/Reload offers discard/cancel while edits are dirty, and a conflicting save
retains local edits until Reload. The authenticated 16 KiB PATCH API hashes only
the five navigation fields plus the restore-tabs preference and atomically
updates the bounded settings document on a Rust worker; unrelated browser/agent, future and root settings are preserved. Invalid
existing browser configuration is reported rather than overwritten.

Successful Save or Reload applies navigation policy to open tabs. Newly denied
addresses close; page-JavaScript changes reload existing pages, and disabling
Browser cancels its private session. Loosening a navigation permission retains
allowed pages. Settings alone do not start the engine, and enabling Browser does
not replay an earlier rejected Open request. This editor does not expose agent
profiles or other advanced browser settings. Another client's saves are adopted through Reload or a fresh Browser Open after Exit; continuous
cross-client policy synchronization remains pending.

Bounds are eight tabs, four active transfers/file dialogs and 100 download
records. Downloads require an explicit native Save destination, sanitize the
suggested filename, revalidate the response URL and do not overwrite files.
WebKit receives an owned staging path; Rust publishes the completed file atomically
without replacing a destination created before or during download. Uploads use a
native chooser with the page's MIME filter, at most 16 files/32 KiB of paths, and
reject stale results after navigation (including same-URL reload), tab closure,
process failure or Exit. See [file-transfer acceptance](#native-browser-file-transfers). TLS errors are rejected; page permissions,
automatic popups, local-file navigation, privileged schemes and script clipboard
access are denied. Navigation rules are not a DNS firewall or a subresource
network allowlist, matching the existing desktop policy's scope.

The private Wayland fixture passes actual page rendering, normal page scripts,
absence of Studio bridges, navigation/history, multiple tabs, page search, zoom,
selection capture, crash/reload and local-storage isolation after
Exit/reopen, including restarting through the address entry after Exit. Debug
opens observed so far take 713–921 ms. Observed peak RSS is about 348 MiB
for native, 38 MiB for the gateway and 417 MiB summed across owned WebKit content
processes; summed RSS can count shared pages more than once. Owned content and
backend processes are reaped after application exit. Real-desktop Save/upload
portal presentation, focused physical clipboard, persistent profiles, full
advanced browser preferences and persistent in-page annotation overlays remain migration gates.

The original controls checkpoint passed the four-launch Wayland regression,
including chat, document editing/comparison, settings and lazy content engines.
Current file-transfer and release evidence follows below.

The navigation-settings increment adds
`scripts/smoke-gnome-browser-settings.py NATIVE GATEWAY --api-only` and its
`--headless` native Wayland case. API checks cover authentication/origin,
preservation, validation and stale writes. The Wayland case opens Settings with
actual Ctrl+comma in its owned compositor, checks explicit-save/close/reload and
conflict guards, clicks native discard buttons, and verifies JavaScript reload,
tab retention, HTTP/localhost revocation, private-IP policy, disable/enable and
content isolation on private localhost pages. It sends no model turns. The
inspected `browser-settings-ui-41zbgej9/browser-settings.png` shows the mapped
native preference rows; its run observed about 400 MiB native, 41 MiB gateway
and 593 MiB summed WebKit content RSS across repeated private-session launches.
Summed process RSS includes shared pages; this is not a steady-state requirement
or a performance comparison. A second private run
(`browser-settings-ui-wtbhig94`) also verifies owned backend cleanup. The gateway
suite passes 193 tests and native passes 117, one ignored each; both optimized
checks pass, with the existing native deprecation warnings. The four-launch
application regression (`run-8ude6mg6`) and the existing browser regression
(`browser-ui-wbf79g0u`) pass after integration, with no GTK criticals, panics or
theme parser errors in their application logs. The full regression includes the
real 4,097,226-byte Markdown report, editing/conflicts, tables, Git, terminal,
settings, conversation workflows and lazy browser/EPUB construction.
These earlier navigation observations are separate from the current transfer
checkpoint below; real desktop-service presentation remains a manual gate.

### Native browser tab restoration

**Settings → Browser → Restore browser tabs** opts in to saving at most eight
HTTP(S) addresses and the selected tab. Save is explicit for the preference.
Once enabled, navigation, tab selection/order and individual tab closure update
small address metadata through a Rust worker; no page HTML, titles, cookies or
back/forward history are stored. Username/password URL forms are rejected. Each
canonical address is bounded to 8 KiB, and the metadata file and request are bounded to 128 KiB. The file is
`gnome-browser-tabs.json`, beside the shared settings file. Atomic replacement
uses the shared bounded-document I/O rather than a GTK filesystem operation.

Ordinary application startup and changing this setting do not construct a browser
engine. Opening **Browser** with no requested address restores the permitted saved
tabs. Opening a specific URL starts that address alone; a rejected first Open
keeps saved addresses intact. Hide retains live tabs;
Exit drops their private network session while retaining opted-in addresses.
Closing the last tab records an empty selection. Disabling restoration forgets
saved addresses immediately; enabling it again does not resurrect closed tabs.
If Browser is already open, its live tabs become the new saved selection. Page
storage remains ephemeral across real application restarts.

The authenticated `/studio/native/browser-tabs` GET/PUT routes share the settings
transaction lock. Reads filter against current URL/navigation policy; writes
revalidate all addresses and require an opaque revision. Permission changes cannot
restore blocked addresses. The narrow native settings writer and existing shared
settings writer rotate an empty generation on restore off/on transitions, so an
old request cannot recreate forgotten addresses even when the setting returns to
its earlier value. Legacy five-field native navigation patches still preserve
restore-tabs when omitted. Unsupported/corrupt saved-tab documents are reported
and retained instead of being silently overwritten.

The native controller retains one in-flight request and one replaceable latest
snapshot. It invalidates replies from older settings generations, preserves local
unsaved addresses on failure, and waits for the latest save before normal window
closure. A failed save offers Retry, Cancel or Close without saving. Cancel also
disarms deferred file-cleanup/metadata close callbacks and retains live pages;
explicit Browser reopen can retry against a fresh revision. This adds no browser/runtime dependency.

```bash
python3 scripts/smoke-gnome-browser-settings.py NATIVE GATEWAY --api-only
python3 scripts/smoke-gnome-browser-settings.py NATIVE GATEWAY --headless
```

The scoped API check verifies authentication/origin, URL/type/size validation,
unknown settings preservation, revision conflicts, forgetting and generation
rotation through both native and shared-settings writers. The Wayland check uses
owned localhost pages and fake providers. Its initial launch exercises live
navigation permissions plus Exit/reopen, selected-tab/order retention,
same-document fragment addresses, individual/last tab closure, intentional single-URL Open, rejected Open without saved-state loss,
restore off/on forgetting, and a real
foreign revision. Clicking the actual Cancel button leaves the window mapped after
an admitted cleanup worker finishes; explicit reopen recovers the save. A final
window close starts with a save pending and a newer selection queued. A second
application process restores the final addresses/selection from disk with empty
private page storage. Neither launch sends a model turn, and owned providers are
reaped. The final focused run is `browser-settings-ui-eyeq54tv`; the mapped native
settings screenshot was also inspected in `browser-settings-ui-k4c61k_f`.
Metadata synchronization is skipped when restoration is off. An unnecessary
read in the initial implementation reapplied the fixture's older JavaScript policy
while a destination cancellation was pending; the owned native process crashed
inside WebKitGTK. The corrected opt-in gate passes the same sixteen-operation file
fixture. Every owned transfer also shares a cancellation gate set before entering
the engine. Repeated Cancel/Exit/policy requests cannot call engine cancellation
twice, and after network completion they cancel only Rust publication. The final
file regression (`browser-files-_ukpmo3u`) additionally emits Cancel twice in the
same GTK turn. Exact upload/download bytes, no-overwrite saves, six cancelled/late
portal replies and the file-worker close guard all pass. The full four-launch
application regression also passes (`run-5tskx12y`), using the real
4,097,226-byte Markdown report and private editing/conflict/repository fixtures.

Final Rust validation passes 195 native tests (two ignored) and 214 gateway
tests (one ignored). Rust 1.92 checking and both optimized builds pass. The native
build retains only the existing deprecation warnings. The local bundle is
`/media/rui/studio-gnome-deps/current-release/package`; executable hashes match the
optimized build outputs:

- Native: `54f3a049eb1a3b2e2d1bbb1818f3199c30c93ba6b974f7bf418a84cef82ce0b6`.
- Gateway: `808de8d4c12ba6b7be9473a4afc00b3012531e9edf95bec06ed7771a02ef8d6c`.

The exact installed optimized pair passes the combined editor/Find, four diagram
engines, language, backend-file, Git and viewer acceptance with application hooks
disabled (`release-ui-hc5pzgx9`). The focused address-restoration and transfer tests
above use debug acceptance controls; the installed check covers the ordinary
release UI and lazy content-engine boundary. No new dependency was added.

Persistent cookie profiles, continuous cross-client live policy synchronization,
advanced browser preferences and full in-page annotation overlays remain gates.

### Native browser file transfers

The chooser registry now retains a page or download owner for every native file
request. Upload replies must still own their token and the same document epoch;
a same-URL reload invalidates the request. Navigation, tab closure and process
failure cancel the owner's picker without closing other tabs. Download Cancel,
failure, policy changes and Exit invalidate pending destination ownership before
cancelling the engine; late chooser callbacks cannot cancel a newer request or
repeat an engine cancellation. MIME filters are supplied in GTK's filter list,
and oversized multiple-file results are rejected before collecting their paths.

WebKitGTK 6 receives an absolute filesystem staging path rather than a file URI.
It never receives the chosen final destination. The shared GTK-independent
`download_file::Target` creates an owned `tempfile` beside that destination and
publishes with `persist_noclobber` after successful transfer and file sync. Existing
files and symlinks are refused; a destination created while downloading is also
preserved. Failed/cancelled jobs clean only owned staging files. Progress names
refer to the selected filename rather than the internal path. No document body is
copied through GTK or the gateway for downloading.

`file_io::Jobs` runs filesystem operations on Rust workers behind a four-permit
lane. New preparations are refused at sixteen outstanding file jobs; already-owned
cleanup is retained. Download state distinguishes network completion from final
publication so Exit does not cancel a completed WebKit request again. Cancellation
flags invalidate queued preparation/publication. Window closure waits for admitted
file work and cleanup, then retries the normal close path. Browser engines remain
lazy, and this adds no new application runtime dependency.

```bash
python3 scripts/smoke-gnome-browser-files.py NATIVE GATEWAY
```

The owned Wayland fixture uses real page file inputs, pointer input, local HTTP
uploads/downloads and GTK's FileChooser portal transport. Sixteen chooser calls
cover single/multiple selection, exact Unicode/CRLF contents, cancellation, empty
and nonlocal results, same-URL reload, closing an owner with another tab alive,
Save, destination errors, Download Cancel, Exit/reopen, an existing destination and
a pending destination revoked by policy. Six held requests receive Close and six
deliberately late responses write nothing. It verifies exactly 196,632 downloaded
bytes, unchanged source/existing files, absence of owned temporary files, zero
model turns, and owned provider/content-process cleanup. A real owned stage is held
across the outer Window::close call; the window stays mapped until worker cleanup
is released, and no final destination is published.

The staging checkpoint passes in `browser-files-655y7ugw`: native/gateway sampled
peaks are 385,744/20,728 KiB; summed sampled WebKit content RSS peaks at 626,616 KiB
across repeated private-session launches. These are debug fixture observations,
exclude compositor services and can count shared pages multiple times. Native
Rust tests pass 195 cases (two ignored), including exact publication, existing
and late-created destinations, cancellation cleanup and symlink preservation;
Rust 1.92 checking passes.

The shared private portal also now advertises an explicit 1.0 text scale and
version-correct Settings Read/ReadOne replies. An empty successful settings reply
left GTK Wayland's text-scale fallback at zero, causing later WebKit views to have
zero viewports. A minimal two-host probe reproduced this with the empty portal and
rendered correctly after the scale fix; no tab-layout workaround remains in the
application. The relevant conversion and fallback are visible in
[GTK's portal settings implementation](https://github.com/GNOME/gtk/blob/main/gdk/wayland/gdksettings-wayland.c).
The existing eighteen-call file-portal regression passes after the shared fixture
fix (`file-portal-ui-fo280k_7`). Real portal presentation, physical clipboard,
persistent profiles and broader browser parity remain migration work. See
[opt-in address restoration](#native-browser-tab-restoration) for the later restore-tabs increment.

The final screenshot/close run also passes (`browser-files-rlte3lsi`); its mapped
Downloads dialog shows the existing-destination explanation without changing that
file. The polled close-state marker uses atomic publication. Final native tests
remain 195 passing (two ignored), and Rust 1.92 checking passes. Browser policy
and the four-launch application regression pass; `run-4t701dmk` opens the real
4,097,226-byte Markdown report in 1,052 ms with 64 GTK heartbeat ticks. These
private observations are not isolated performance measurements.

The refreshed optimized pair passes installed `--all-diagrams --language
--backends --git --viewers` acceptance in `release-ui-2mci3mrc`, without application
test hooks. It verifies the ordinary editor, Find, diagrams, native language and
backend configuration, Git, Ctrl+O, viewer returns, lazy web content and cleanup;
the focused browser-transfer cases above use debug acceptance controls. The native
package hash is
`a78148885f89c41766be01fff417718a4dfa1dd0ebf310e869b50b9e2a8d974f`;
the gateway hash remains
`2d1435f540b8fd4d10afa1fd9a5094b8be4e418407b21bbaafac243d5ba7745a`.
Both match their optimized build outputs. Full browser/desktop/provider/platform
migration gates remain open.

## Native selection translation

The native GTK selection menu and **Ctrl+Shift+T** open a translation dialog for
the selected conversation. Source and result are literal, selectable text;
Markdown/HTML in a model response is never executed. The dialog shows English
IPA and Chinese pinyin when supplied and copies the exact translation. Closing
the dialog cancels its owned task. Starting another translation does not send a
message to the source conversation or replace an unsent composer draft.

**Settings → Translation** edits the shared engine, local Ollama model, and
backend-specific model/effort overrides. A blank backend model uses the session
model. Codex defaults to Low effort; the effort control is disabled for OpenCode,
whose adapter has no corresponding supported option. Loading installed local
models neither downloads a model nor runs inference. Explicit Save uses the
authenticated, origin-checked `/studio/native/translation-settings` GET/PATCH
endpoint with revision conflict detection. Unknown settings and other backend
overrides survive a native save. Conflicts retain the local form for review.

Computation stays on Rust workers. Source input is bounded to 16,000 Unicode
scalars, structured output to 512 KiB, active task admission to two, and the
memory cache to 64 entries / 4 MiB including key and string capacities. Codex
uses an ephemeral, read-only, approval-never task and consumes notifications
instead of reading ephemeral history. Early completion is correlated with the
returned task/turn identities. OpenCode uses a newly created session with a deny
permission policy, reads only its own messages, and rechecks its identity before
deletion. Cleanup never targets the conversation being viewed. An uncertain
mutation is not resent; an unconfirmed cleanup is reported. Local Ollama retains
the existing fixed loopback service and is independent of the selected backend.

Browser selection is read in an isolated content world and guarded by the page,
URI, navigation epoch and original backend context. Native chat/Markdown/source
TextViews and rich labels have the selection action. PDF Preview/Text and EPUB
also have native translation buttons. Actual PDFium Preview selection and GTK
Text selection produce the same translation/cache result; retained EPUB DOM
selection preserves literal Unicode after toolbar focus collapses the live
range. These paths pass the private fixture with actual PDF and EPUB files.
OpenCode utility-session catalog isolation is shared by the gateway and native
client, as described below.

The private `scripts/smoke-gnome-translation.py` fixture covers Codex, OpenCode,
local translation/model listing, exact native Copy, literal Unicode output, early
completion, cache reuse, cancellation before task-creation acknowledgment,
malformed structured output, browser selection, stale backend rejection,
per-session draft retention, owned cleanup and process shutdown. Its Ollama
responses come from an authenticated fixture-only proxy; it never contacts port
11434. The final checked run is `translation-e5907_bk`, using the debug native
client, optimized gateway, real EPUB/PDF fixtures and the silent speech server
described below. Observed peak RSS was 431,824 KiB native,
38,656 KiB gateway, and 431,764 KiB across the three observed
WebKit content processes. These are separate observed peaks, not application
memory limits. API-only checks also cover authentication, foreign origins,
invalid profiles, unknown-field preservation and stale revisions. The scoped
API check is included in native CI. No real provider prompt or audio playback
was used. The 218-test gateway suite and 200-test native suite pass (one gateway
subprocess helper and two optional native engine/benchmark tests are ignored by
the default test runner); the shared-contract check
passes all seven selected tests. The four-launch private regression of the real
4,097,226-byte Markdown report passes as `run-g1hrzv02`. The two-launch browser
restoration/close-cancellation regression passes as
`browser-settings-ui-6l59kqip`. Rust 1.92 checking also passes. The full native
application goal remains incomplete.

The speech checkpoint's installed accessibility acceptance `release-ui-kmdqteue`
passes all five optional groups
(`--all-diagrams --language --backends --git --viewers`) without application test
hooks. It covers actual native editing, undo/redo, save/open portals, unsent
draft retention, all four diagram engines, backend settings, Git review and
viewer returns. Its packaged executables matched the build outputs: native SHA-256
`66c427496db7da86b0c2135f57d51d67425b502c258b627c946e8f5b7968517b`, gateway
`02e0e01961f3e77f3962ab448aac3042a0ab44af95ec6dfc2db29e221e4f5071`.

### Native speech playback

The translation dialog has **Listen to English**, **Listen to Chinese** and
**Stop**. Output playback uses the returned pinyin when available, otherwise the
literal Chinese translation. Source/output text remains selectable and Copy
keeps its original contents. Closing the dialog or approving application close
cancels that dialog's playback. New dialogs wait for an older playback to finish
closing before enabling Listen again.

The Linux frontend uses the pinned, unmodified
[`ssip-client` 0.10.2](https://docs.rs/ssip-client/0.10.2/ssip_client/) crate on Rust
workers. It connects to the local Speech Dispatcher Unix socket, with one
utterance per connection. The standard `SPEECHD_ADDRESS=unix_socket:/absolute/path`
override is supported; otherwise it uses
`$XDG_RUNTIME_DIR/speech-dispatcher/speechd.sock`. An explicit Listen can run
`speech-dispatcher --spawn` when the default service is absent. Opening Studio,
opening the dialog and running a translation do not start a speech service.
Install `speech-dispatcher` and `speech-dispatcher-espeak-ng` for local voices;
the native dependency installer includes them. No synthesis model or C binding
generator is bundled. The crate's MIT notice is packaged under
`licenses/ssip-client/`.

Stop sends the fixed `CANCEL self` command on a clone of the same playback
connection. It never opens a control connection or targets another client's
identifier. This follows the connection scope defined by the
[Speech Dispatcher protocol](https://github.com/brailcom/speechd/blob/master/doc/ssip.texi).
Playback uses text priority. SSML is disabled, and the at-most-16,000-character
input is newline-normalized and dot-stuffed before entering SSIP data mode.
Text containing a lone dot, `CANCEL all`, markup or pipe-control syntax remains
literal data. Ambiguous requests are not replayed.

Admission allows one active playback, with one notification worker and at most
one short-lived Stop worker. Stop and joining run independently of the shared
Markdown parsing/indexing CPU pool. Initial command/read and write operations
have two-second I/O timeouts; playback has a fifteen-minute deadline. After Stop
is sent, a missing acknowledgment is bounded to three seconds before shutting
down the owned socket, joining its worker and reporting an unconfirmed stop.
Foreign-client terminal notifications are ignored. Completion before its queued
acknowledgment is accepted because the connection owns just one utterance.

The private translation fixture uses a silent Unix SSIP server and an independent
second connection. Five real native playback requests verify English, pinyin,
Mandarin fallback without pinyin, fragmented replies, early completion, exact
multiline/dot-stuffed text, scoped Stop, dialog-close cleanup and an unresponsive
Stop response. All spoken connections close; the independent connection remains
open until fixture teardown. No audio device, real speech service or real model
is used. Actual voice quality, pronunciation, audio routing and physical
desktop/screen-reader interaction remain manual acceptance gates.

### OpenCode utility ownership and catalogs

Native OpenCode translation uses the authenticated, origin-checked
`POST /studio/native/utility-session` endpoint. Its three fixed operations are
`create`, `catalog` and `close`; callers cannot choose arbitrary SDK routes.
Create verifies the source session's project, reserves a random receipt/title
before sending the SDK request, then confirms the new identity, project and final
deny-all tool policy. Source verification and creation share a 35-second deadline.
There are at most four concurrent creation workers and four active receipts.
The source conversation is never prompted, renamed, interrupted or deleted.

The sidebar/agent catalog, native Router picker and gateway Router discovery use
the same Rust-projected catalog pages. A pending creation is hidden only by its
exact random title/project, excluding the source ID. Confirmed active tasks are
hidden by backend and ID. Similar user-created names remain visible. Filtering
retains the cursor from the original 100-row SDK page, even when every row is
hidden. Irrelevant SDK summaries/diffs and non-scalar metadata are discarded on
the worker before transport to GTK. SDK reads are bounded to 32 MiB, projected
pages to 100 rows, and concurrent catalog reads to four. The existing raw
`/opencode` proxy response remains unchanged for compatibility.

Close uses its creation receipt, rechecks the exact identity/title/project and
optionally interrupts only that owned task before one delete attempt. A changed
identity relinquishes filtering without authorizing deletion. A lost or false
deletion acknowledgment is reported and never retried; repeated successful Close
returns the stored result. Explicit Router target choices also revalidate utility
ownership before any model prompt. Up to 256 retired receipts filter late pages;
their original title/project must still match so an adopted or reused identity
does not hide a user session. Ownership is process-local. No cleanup is guessed
or replayed after an unknown creation result or gateway restart; this does not
claim persistent orphan recovery.

`scripts/smoke-gnome-utility-sessions.py` uses private fake providers to check
authentication/origins, pending creation, a multi-megabyte upstream catalog,
original tail cursors, automatic/explicit Router exclusion, similar user names,
ordered permissions, adopted sessions, lost/false cleanup acknowledgments and
active admission. It verifies that no model prompt or source mutation occurs.
The native Wayland translation fixture additionally refreshes the sidebar and
opens the actual Router picker while an owned OpenCode prompt is held, then
finishes translation and checks draft retention and cleanup. Run
`translation-f8i4vovb` passes these checks, PDF Preview/Text, EPUB, independent
local translation and silent scoped speech. Observed separate peaks were
430,668 KiB native, 57,340 KiB gateway and 431,972 KiB across three WebKit content
processes; these are fixture observations, not memory guarantees. The
cross-backend Router runtime regression passes as `router-runtime-dg5j3v7s`, and
the four-launch real 4,097,226-byte Markdown regression passes as `run-orb_wnti`.
Final API run `utility-sessions-otnaklgw` also verifies four active receipts and
rejects a fifth before SDK creation. The gateway suite passes 223 tests and the
native suite 200 (one gateway subprocess helper and two optional native
engine/benchmark tests remain ignored by the default runner). Rust 1.92 checking
and both workspace format checks pass. The utility API check is included in
native CI. The entire native migration remains incomplete.

The catalog checkpoint's optimized gateway passes the utility API fixture
(`utility-sessions-8jei4rf6`). Installed acceptance
`release-ui-piqrbfbh` passes all five optional groups (`--all-diagrams --language
--backends --git --viewers`) without application test hooks. It covers actual
editing, undo/redo, dirty view switches, explicit save, file portals, all four
diagram engines, language/backend controls, Git review and viewer returns.
That checkpoint's package hashes matched the optimized build outputs:

- Native SHA-256: `e9466db0987ac42a58132b3d3ab3b677fee9d6e8c27a0d544285655cf0db05cc`.
- Gateway SHA-256: `94a6f5d998475fdf49d3fc1d71fa2dd73a6f4464687df1f1c3d0dda236f0975e`.

### Native OpenCode utility events

The native adapter now reads authenticated, origin-checked
`GET /studio/native/opencode-events`. The gateway connects only to the fixed SDK
global-event route with its own credential and filters owned utility events on
Rust workers, before native message projection, activity, Attention or completion
notifications. The legacy `/opencode/global/event` proxy remains unchanged.
Ordinary session messages/status and global service events continue through the
native endpoint.

An exact reserved nonce/project in a session metadata event or catalog can supply
a filtering identity before the creation acknowledgment. This observed identity
never authorizes a prompt, interruption or deletion: Close still requires the
identity confirmed by the SDK creation reply. A confirmed active task is filtered
by its exact ID. Retired receipts also remove late events; later complete
metadata showing adoption relinquishes that filtering. Partial status/message
events cannot prove adoption. Source IDs and similar user-created session names
are preserved. Existing four-active/256-retired ownership bounds still apply.

There are at most four native SSE streams. Each stream permits 16 MiB between
parsed blocks, processes one JSON/filter task at a time, releases its admission
slot on closure and sends a keep-alive every fifteen seconds. Invalid JSON,
recognized-field UTF-8 errors and oversized input terminate that connection;
requests are not replayed. Both gateway and native adapter use the pinned,
unmodified [`sse-stream` 0.3.0](https://docs.rs/sse-stream/latest/sse_stream/)
decoder with its default `memchr` and `simdutf8` features. It consumes partial
lines incrementally, replacing the observed long-line rescanning in
`eventsource-stream`. The old hand-written BOM prelude workaround is removed.
Metadata-only blocks, explicit event/id/retry fields, Unicode, LF, CR and CRLF
remain supported; metadata-only blocks do not enter native JSON presentation.
The selected MIT notice and provenance are packaged under `licenses/sse-stream/`.

The private utility API fixture checks filtering before creation acknowledgment,
live message/status events, late completion, adopted-session events, unchanged
legacy output, fragmented UTF-8/BOM and multiline fields, stream admission and
slot release, metadata-only blocks, malformed JSON and oversized events. It also
tries Close using an observed pending nonce and requires rejection before SDK
cleanup. No real provider prompt, source mutation or audio service is used.
In `utility-sessions-qnn7xnld`, a 13,631,488-byte valid payload arrived in 501 ms;
oversized-event rejection took 171 ms, with observed gateway peak RSS 66,376 KiB.
The original parser's rejection test timed out after eight seconds. These are
debug fixture measurements including private-provider serialization and HTTP,
not arbitrary-network throughput or memory guarantees.

Actual GTK run `translation-0pijzsrm` holds an OpenCode translation while its
utility busy/text events and an ordinary session's busy event share the global
stream. The ordinary session becomes active and later receives Attention;
utility activity never appears. It also passes catalog/Router filtering,
source-draft retention, PDF/EPUB translation and silent scoped speech. Observed
separate peaks are 431,080 KiB native, 57,568 KiB gateway and 431,532 KiB across
three WebKit content processes. The 10 MiB image-history fixture passes as
`history-images-lmshk0z5`; the four-launch real-report regression passes as
`run-yit3kiqk`. The gateway suite passes 226 tests (one helper ignored). The native
suite passes 200 (two optional engine/benchmark tests ignored).

Rust 1.92 checking, both workspace format checks and shared Codex/OpenCode
contract checks pass. The optimized gateway passes the complete utility fixture
as `utility-sessions-7s9r2_h8`, including invalid UTF-8 rejection and refusal to
clean up from observation alone. Its 13 MiB valid event took 277 ms,
oversized-event rejection 105 ms, and observed peak gateway RSS was 37,088 KiB.

The utility-event checkpoint's installed acceptance `release-ui-py14d0c3`
passes all five optional groups (`--all-diagrams --language
--backends --git --viewers`) with application hooks disabled. It verifies native
editing/save/undo, dirty view switches, portals, four diagram engines,
language/backend controls, Git review, viewer returns, lazy web-content engines
and managed-process cleanup. That checkpoint's packaged executables matched its
build outputs:

- Native SHA-256: `6410f12e4c93a0a036fd7e71f05985def4db45641db4df263938e7c9a70c71ac`.
- Gateway SHA-256: `5db3a0318724ce357011789447b9e5c140bff2d345c7dcdd2e2a90758798d59a`.

The packaged SSE parser notice/provenance also matches its checked-in files.

OpenCode turn completion still follows its authoritative status events/snapshots.
This does not work around the upstream source-session stuck-busy issue. Broader
physical desktop, voice quality and full application migration gates remain.

## Native session comments

**Comments…** in the application menu opens the selected session's native
comment library. The Markdown reader and both comparison panes also have a
**Comment on selected text** action. Preview paragraphs/headings, Source and
saved Edit text can create document comments. A comment keeps the canonical file
path, project root, indexed source revision and one-based source line range.
Source/Edit selections use the selected lines exactly, including Unicode and
end-at-next-line boundaries. Preview uses the original containing block's line
range; it does not claim an exact character mapping through Markdown formatting.
Dirty document edits must be saved before creating source-linked comments.
Rendered code blocks and table cells use the source-anchored capture described
under [reader comments](#native-reader-and-expanded-cell-comments).
Image-region capture remains in progress.
Chat selections use the native actions described below.

The EPUB header and browser page menu also provide selection comments. Their
DOM-dependent range capture runs in an isolated content world; application
controls, session ownership, metadata and storage stay native. A retained range
survives native toolbar/menu focus changes. Excerpts are bounded to 16 KiB and
metadata to 32 KiB. Pending captures freeze their session and document/tab epoch;
changing sessions, chapters or frames rejects a stale result. Browser content
receives no Studio message handler or gateway capability.

EPUB records preserve the existing root/path, book hash, chapter and CFI range
schema. Element selection boundaries are normalized to text before calling the
vendored CFI engine, and the encoded range must reproduce the captured quote.
Opening a saved source reopens a Rust snapshot, checks the book hash, restores
its section and highlights only a range whose text matches the quote. An absent
or mismatched range shows the section with an explanation; malformed source
anchors are rejected. A changed or unverified book edition does not reuse the
old CFI. EPUB prompts explicitly ask
for read-only explanation of the passage. Browser records preserve URL/title
and nullable legacy target/selector fields; reopening uses native page search
without claiming a verified DOM location.

PDF Preview and Text selection comments share the same library. Page/hash/region
anchors, optional native character ranges, safe source reopening and compatibility
with older anchors are described in [native PDF](#native-pdf). PDF prompts retain
the page, regions and quote and explicitly treat the document as read-only.

CSV/TSV/XLSX cell comments use the same library and retain the sheet, one-based
cell address, file hash and column header. SQL result comments also keep the
query that produced that result, independently of later editor changes. Source
reopening and legacy handling are described in [native tables](#native-tabular-documents).

The dialog supports note/additional-instruction editing, explicit Save, removal,
source reopening, and insertion into the current session's composer. Insert is
idempotent for an unchanged trailing prompt, preserves existing composer text,
and sends no model turn. **Send comments** first saves the batch with a revision
check, freezes its records and builds the prompt on a Rust worker. It uses the
normal Codex/OpenCode send path, or the Router delivery path, and refuses a
session change or an unresolved existing Router request. The dialog remains
scoped to its original session. Comments are retained after insertion alone.
Save/Discard/Cancel guards protect dialog and application close. Reload after a
conflict has its own discard confirmation; cancelling keeps local edits.

Storage remains Studio's existing `studio.sqlite3` annotation tables and record
format. Opaque provider/version/anchor data and legacy quote/comment/target
records survive native round trips. The shared route now accepts a narrow
session GET and optional `expectedRevision` on PUT. Native saves compare a
SHA-256 content revision inside an immediate SQLite transaction; stale writes
return 409 and leave the native edits intact. Older PUT clients retain their
204 response and unconditional-write behavior, so complete concurrent-writer
reconciliation also requires those clients to adopt revision checks. Native reads
fetch only the selected session. Database reads/writes, hashing, response
serialization and prompt assembly run on Rust workers. Preview selection checks
only bounded weak references to currently displayed GTK labels and never scans
the full document.

Acknowledged sends POST the frozen records to the shared annotation `consume`
route. An immediate SQLite transaction removes only records still exactly equal
to the submitted snapshot, including their IDs, notes and source anchors. Overall
instructions clear only when unchanged. Codex/OpenCode use a matching successful
RPC response; Router waits for the target turn to be accepted, rather than merely
for controller/job creation. Rejected or unconfirmed sends keep the comments.
The native Rust worker merges cleanup with newer local/shared comments and
preserves a newer composer draft. Simultaneous edits to the same comment block
saving until an explicit reload resolves the conflict. Clearing an edited legacy
note also clears its stale legacy comment field.

If cleanup fails after acceptance, **Retry cleanup** repeats only the idempotent
storage operation. It never sends another model request. Closing in that state
requires choosing to keep the remaining stored comments; unsaved edits retain
the ordinary close guard. A pending model request is not replayed automatically
after connection loss or restart. The native pending acceptance receipt is not
persisted across an application crash, so inspect the conversation before
resending a batch whose outcome is unknown.

Limits remain 32 comments per session, 16 KiB per excerpt/note and 32 KiB of
additional instructions, with a 1 MiB shared request-body limit. Concurrent
reconciliation can retain up to 64 local/shared records in the editor; reduce
them to 32 before the next save/submission rather than silently truncating them.
Archive/source-specific metadata bounds use the existing shared validator.
The private API fixture passes authentication, session scope,
stale-save rejection, legacy-writer detection and opaque-source preservation.
A two-launch Ubuntu 26.04 Wayland fixture passes formatted Preview and exact
Source Unicode selections, line 3 anchors, note/additional persistence, source
reopening, removal, duplicate-free insertion, close cancellation/discard,
conflicting-save retention, reload cancellation/recovery and restart restoration.
It changes only private documents/stores and sends no model turn. Screenshots of
the actual native dialog were inspected. Observed native peak RSS is about
314 MiB and gateway peak RSS about 39 MiB; no WebKit content processes start.
The submission fixtures additionally pass actual Codex, OpenCode and Router
acceptance through fake providers, duplicate-send prevention, frozen prompts,
rejected-send retention, newer local/shared edits, conflicting same-record edits,
cleanup-only retry after a lost reply, and cancellation of application close
while accepted cleanup remains unconfirmed. Request ledgers prove one model
request per direct batch and one controller plus one target request for Router;
cleanup retry adds none. No real model providers are called.
The EPUB comment fixture passes four Wayland launches: capture/insert/source,
restart restoration, a deliberately wrong range and a changed private book.
The browser fixture passes capture/source search and restart restoration.
Both reject an asynchronous capture after changing conversations. These checks
also preserve an opaque legacy record, verify Unicode, send no model turns and
reap owned backend/content processes. EPUB hostile content makes no remote
requests. Actual native dialog, verified EPUB highlight and browser source
screenshots are inspected. These source-comment checks do not close the EPUB
physical clipboard acceptance gate documented above.
The gateway/native Rust suites pass 176/83 tests with one ignored in each, and
both release-profile checks pass. The full four-launch Wayland regression also
passes after integration, including document editing/comparison, settings,
conversation/lifecycle handling and no eager browser/EPUB engine startup.
The existing web suite passes 747 tests and TypeScript checking. Round-trip tests
verify native table query metadata and PDF extension fields survive known-provider
normalization; the Windows compile-boundary check still requires desktop support
to remain enabled by default. This is source/static coverage, not a Windows build.

### Native chat comments

**Comment on selected text** is available in the native context menus for
rendered conversation text, code and visible table cells, including paged older
history. It opens the existing session Comments dialog with the literal selection.
Save, insertion, revision conflicts, close guards and explicit Send use the same
shared SQLite store and submission workflow as document comments. Capture and
insertion alone send no model request. There is no additional draft store.

The capture keeps the actual turn/item IDs and the source session key in the
existing `chat` provider/version-1 schema. Router-displayed responses retain their
target session/turn/item identity while the comment batch belongs to the
currently selected conversation. Browser `startOffset`/`endOffset` are left null:
independent GTK text widgets do not share the web UI's rendered UTF-16 coordinate
space. These are exact rendered quotes and message anchors, not claimed browser
character positions. Native records survive the existing web provider's source
normalization without loss.

Activation reads at most a 16 KiB selection from a mounted native text widget.
The displayed live/history item must still be current; recycled cells and old
session items cannot populate another library. Stable rows use the current Item
handle after history/stream reconciliation. Expanded full-cell dialogs and reader
Preview code/table selections are now covered by the shared selection action
[below](#native-reader-and-expanded-cell-comments). Selections across independent
GTK widgets remain unsupported; separate snippets can be added to the same batch.

Comment, Favorite and Translate menu entries now use a reusable weak GMenu label
binding. Changing interface language updates those fixed labels without rebuilding
the reply, changing action names or translating provider content. The Comments
dialog also has an explicit adaptive minimum size, avoiding the prior libadwaita
minimum-size warnings during rapid open/close.

The private `scripts/smoke-gnome-chat-comments.py` fixture uses real mounted GTK
labels/TextViews/table cells, private fake providers and two application launches.
Run `chat-comments-5lm2zqvm` verifies paragraph/code/table/older-history captures,
Router provenance, retained-row refresh, stale-session and recycled-cell rejection,
oversized selection rejection, live English/Chinese menu labels, explicit Save,
restart, exact SQLite records and idempotent insertion without losing the unsent
composer draft. The stored anchors also pass the current web normalizer. No model
prompt or web-content process starts, and owned providers are reaped. Observed
capture/restore peaks were 349,432/317,248 KiB native and 45,188/44,144 KiB gateway;
these are private debug-fixture observations, not application memory limits.

All 202 native tests pass (two optional engine/benchmark tests ignored), as do
Rust 1.92 checking, interface catalog generation and legacy comment tests. The
selection translation/PDF/EPUB/scoped-speech regression passes as
`translation-qv3hz0ko`, and the four-launch actual 4,097,226-byte Markdown workflow
passes as `run-lq6_f542`. These checks do not complete physical GNOME/provider
acceptance or the full application migration.

The chat-comments checkpoint was packaged in
`/media/rui/studio-gnome-deps/current-release/package`, paired with the unchanged
validated gateway. Its installed acceptance `release-ui-op4v2_ni` passes
`--all-diagrams --language --backends --git --viewers` with application hooks
disabled, covering the existing editor/portal/diagram/language/backend/Git/viewer
flows and managed cleanup. The debug chat fixture above covers the new selection
actions and persistence. That checkpoint's executable hashes were:

- Native SHA-256: `fbd5e333f3dd68db0f7fa829cc00617b97ba90be2abd0a631dadfc3b8e4d5085`.
- Gateway SHA-256: `5db3a0318724ce357011789447b9e5c140bff2d345c7dcdd2e2a90758798d59a`.

### Native reader and expanded-cell comments

Markdown Preview code blocks and indexed table cells now expose **Comment on
selected text**, using the same bounded selection-menu binder as conversation
text. Code quotes keep the containing fenced block's physical source lines;
table quotes keep the individual cell's source range. These are containing ranges,
not invented character offsets through syntax highlighting or Markdown formatting.
The existing reader toolbar can also capture these mounted selections.

The table widget configures both its initial label and its later full-cell text
view. Long full cells retain the existing 8 KiB styled pages; the action uses the
literal selected page text, up to 16 KiB. A valid quote is frozen before closing
the transient cell window and presenting Comments on the main window. Detached
cell buttons cannot reopen a window, and actions reject unmapped old widgets.
Reader callbacks additionally retain their original generation, file revision
and source range, and reject another document/revision, another mode, pending
loads/saves or dirty edits. The toolbar uses the same snapshot identity checks.
Chat keeps its current reply identity and existing session/history checks.

The model's synthetic trailing ellipsis is now excluded from selectable excerpt
text. Pango handles visual truncation, and **Read full cell** remains available.
This prevents an invented ellipsis from making a saved quote fail source lookup.
Selection across separate widgets or separate text pages is still unsupported.

The extended `scripts/smoke-gnome-table-cells.py NATIVE GATEWAY --headless` fixture
uses real GTK widgets with private fake providers. Run `table-cells-ui-o1stoc5w`
verifies excerpt/full-cell comments in both chat and reader, Unicode, styled first
and last pages, exactly five saved shared-SQLite records, table line 3 and code
lines 1–3, detached/recycled cell rejection, old-file action rejection, relative
links, source quote verification, and an unchanged composer draft. A real
edit/save then remounts the old code widget with a valid nonempty selection and
Preview active: it must reject the stale revision even though GTK reports the
widget mapped and the document generation has not changed. Its no-heading
fixtures correctly reopen verified Source at the saved line, preserving the
existing heading navigation policy. No model prompt is sent. The expanded-cell
screenshots were inspected. This debug run observed 502 ms initial chat loading,
199 ms reader opening, 163 main-loop ticks and a 444 ms maximum heartbeat gap
across the complete workflow including forced screenshot rendering; these are
fixture observations, not a responsiveness limit or a release performance claim.
Observed native/gateway peaks were 362,776/47,112 KiB; selection registration keeps
at most 512 weak widget references and does not retain previous document indexes.

The shared-menu regression `chat-comments-y48cnsk5` also passes two launches,
legacy source normalization, live menu language, Router/older-history identity,
stale-session rejection, persistence and idempotent insertion without model prompts
or a web-content process. All 202 native Rust tests pass, with two optional tests
ignored. Rust 1.92 compatibility checking passes. The four-launch application
regression `run-rg0zw26e` passes with the actual 4,097,226-byte report, including
paging/columns, explicit edits, save conflicts, undo/redo and source-line mapping.
It observed 956 ms document opening with 58 main-loop opening ticks and native/
gateway peaks of 586,168/204,260 KiB across the full debug application workflow.
Fresh-host, physical GNOME and real-provider validation remain open gates for the
whole migration.

The reader-selection checkpoint was packaged in
`/media/rui/studio-gnome-deps/current-release/package`, including the existing
optional runtimes and license notices. Its installed acceptance `release-ui-6q32e1hd` passes
`--all-diagrams --language --backends --git --viewers` with application hooks
disabled, including native accessibility/input/clipboard, editor/portal handoff,
Mermaid/PlantUML/D2/math inspection, live language, backend save, Git, viewer
returns and managed cleanup. The debug fixture above separately covers the new
comment capture and revision checks. That checkpoint's native and gateway
executable hashes were:

- Native SHA-256: `1e40e7b965c7d1835711c985e964b131d5a046ad8625911086ae574156128e9f`.
- Gateway SHA-256: `5db3a0318724ce357011789447b9e5c140bff2d345c7dcdd2e2a90758798d59a`.

### Markdown comment quote verification

**Comments → Open source** opens a fresh document snapshot and verifies the stored
excerpt before navigating. New Markdown anchors carry an optional `textKind`
(`source` or `preview`); existing/foreign records remain intact. Source/saved Edit
quotes match exact UTF-8 text, including CRLF and trailing spaces. Preview uses
the same presentation-neutral parser and block/cell text as the native reader,
so emphasis, entities, inline code and repeated table headers follow its rendering
semantics. Preview positions remain containing block/cell source ranges, not
character-level mappings through formatting.

A matching stored revision permits verification within its saved line range.
After a revision changes, only a unique match relocates. Overlapping duplicates
count as ambiguous; the nearest match is not guessed. Legacy anchors without a
kind check both literal and rendered text conservatively; literal matches open
Source, including text present only in Markdown syntax such as a link destination.
Header copies across
table pages share their source identity. Missing/ambiguous/limited matches and
lookup errors open the clamped saved location in Source with a persistent,
translated unverified notice. No quote highlight or updated stored anchor is
invented. Lookup never writes a document or the comment library.

Verified Preview opens target the matching block rather than its chapter heading.
Its line is retained when switching to Source. An ordinary manual switch back to
Preview still uses the shared nearest-heading policy. Dirty editors retain their
existing Cancel/Discard/Save prompt. A pending comment target, including its
quote, survives explicit Save before the new file opens.

The protected `POST /studio/native/markdown/{id}/locate-quote` accepts only an
opened reader ID, expected snapshot revision and bounded quote/anchor data; it
does not accept a file path. A pinned snapshot retains its matching index/cache.
The gateway checks revision before and after work, rejects closed readers, and
admits one lookup at a time on Rust blocking workers. Literal search retains a
64 KiB chunk plus at most a 16 KiB quote overlap. Preview parses one existing
bounded fragment at a time (512 KiB body / 128 KiB header), then drops its blocks.
This adds no browser, diagram-engine or Node runtime. It keeps the existing
128 MiB reader source limit and reports incomplete Preview coverage explicitly.

```bash
python3 scripts/smoke-gnome-comment-location.py NATIVE GATEWAY --api-only
# Include admission/close races while a real lookup worker is active:
python3 scripts/smoke-gnome-comment-location.py NATIVE GATEWAY --api-only --races
python3 scripts/smoke-gnome-comment-location.py NATIVE GATEWAY --reference /path/to/report.md
```

The second command creates its own private Wayland compositor, fake providers and
file copies. `comment-location-gys07_td` passes real native Source actions for
moved Source/Preview quotes, duplicates, removed text, unchanged repeated text,
legacy records and the report copy. It checks stored records remain unchanged,
unsent conversation text, exact line ranges, Source/Preview switching and real
Cancel/Save buttons with unsaved edits. Source, Preview and warning screenshots
were inspected. No model prompts are sent.

In this debug run the 33,550,302-byte table's literal quote lookup took 396 ms;
the 4,097,299-byte report copy's Preview lookup took 1,123 ms. End-to-end native
reopening of the report comment took 2,142 ms with 211 heartbeat ticks and a
19 ms maximum gap. The original 4,097,226-byte file is unchanged. These are private
fixture observations, not performance bounds; sampled native/gateway RSS excludes
indexer and other worker peaks. The 41-test shared model suite, 180-test native
suite and Rust 1.92 check pass. The four-launch regression `run-72xc69u8` passes
the original report, paged editing/comparison, Git, Files, terminal and restart;
ordinary report opening took 952 ms with 58 loading ticks. Full native parity,
physical GNOME/provider acceptance and legacy platform builds remain open.

The refreshed optimized gateway also passes the same API checks with `--races`
(`comment-location-z0h2wn7a`). The fixture observes a second lookup rejected while
the first owns admission, closes that reader, then verifies the running request
returns 410 rather than a verified location. The 32 MiB literal lookup measured
57 ms and the report Preview lookup 285 ms; these exclude indexing and native
mounting. The production native binary and gateway have both been rebuilt and
packaged. This API result does not claim installed GUI coverage of the new
comment workflow; its native widget coverage is the private debug run above.

The additional `comment-location-pta0vaz9` run uses the debug GTK frontend with
the optimized gateway. All eight Source-action cases pass, including a legacy
quote present only in a link destination; that case opens Source. The report
reopens in 623 ms with 59 heartbeat ticks and a 28 ms maximum gap in this setup.
The existing installed five-option regression passes in `release-ui-w_pmdu1h`;
the subsequent legacy view correction is covered by the dedicated native fixture.
The packaged frontend includes that correction. Both full workspace/native
format checks, shell syntax checks, translation snapshot validation, native
unit tests and Rust 1.92 checking pass.

### Source comments for code and text files

Ordinary code/text files now offer **Comment on selected text** in their Source
toolbar. Capture retains the canonical file path, original workspace root, the
existing review-file content hash, exact selected lines and literal text. Unicode,
CRLF, trailing spaces and a selection ending at the next line's start retain
their source meaning. The existing document-provider schema and session store
are reused; saving a comment is independent of writing the source file.

Markdown comments still reopen in the native reader. Other document-provider
comments reopen in Source through a fresh `/studio/review-file` read. Indexing
and literal quote matching run on bounded Rust workers; GTK receives the existing
200-line / 128 KiB windows. A verified byte offset can open a quote beyond the
first window of a very long line. Unchanged hashes permit checking the saved line
range; changed content requires a unique match. Ambiguous/missing quotes show the
same translated unverified notice and keep their stored anchors intact. Ordinary
patch files retain physical line numbers; Git/turn diffs remain separate views.

Capture requires a complete identified file snapshot and a mapped Source view.
Diffs, binary/truncated data, missing identity, hidden/closed views and selections
over 16 KiB cannot create a file comment. The action is hidden in unsupported
non-file contexts and follows actual selection changes in file views. Syntax
marks do not trigger repeated selection extraction. Replaced pages place their
cursor at the start and use GTK's layout-aware scroll-to-mark operation, keeping
the first line fully visible. Opening a new file clears stale path tooltips.

This uses the existing 5 MiB review-file API and its UTF-8 requirement; UTF-16
returns an explicit error with capture unavailable. The current generic Source
viewer can also edit identified writable files as described below. Advanced diff
comments and physical accessibility acceptance remain migration work.

```bash
python3 scripts/smoke-gnome-source-comments.py NATIVE GATEWAY
```

This fixture creates an owned Wayland desktop, two native launches, private code
files and fake providers. `source-comments-icq2t68q` verifies actual selection,
Comment/Save/Source buttons, exact CRLF and Unicode, restart, and relocation from
lines 205–206 to 208–209 after an explicit external fixture edit. It also checks
ambiguous/missing quotes, a match at byte 300,000 of one long line, physical patch
line 5, unsupported UTF-16, capture exclusions and unsent composer retention.
Stored records and source files remain unchanged by reopening; owned providers
exit and no model prompts are sent. Source screenshots were inspected, and
layout checks confirm the first source line is completely visible. These are
private desktop checks, not physical GNOME/provider sign-off.

The full four-launch regression also passes in `run-s16tvchb`, including the
4,097,226-byte real report (952 ms to open, 57 heartbeat ticks), dirty edits,
save conflicts, table paging, Git handoff and viewer return. Native tests pass
183 cases (two ignored); gateway tests pass 210 (one ignored), and the shared
Markdown library passes 41. Rust 1.92 checking and optimized native compilation
pass. These measurements describe this private test setup.

The refreshed optimized pair also passes the installed `--all-diagrams
--language --backends --git --viewers` run in `release-ui-aj9_k9f1`, using actual
GTK controls and no application test hooks. This covers the established reader,
settings, Git, viewer-return and cleanup workflows; the new code-comment actions
have the separate two-launch coverage above. Packaged binary hashes match their
release outputs. Dependency inspection confirms GTK4 without GTK3 in the native
frontend and no GTK/WebKit/JavaScriptCore dependency in the gateway.

### Native Source find

The generic Source toolbar now provides full-snapshot literal Find, previous/next
with wraparound, and a Unicode-aware Match case option. `Ctrl+F` opens the bar;
Enter / Shift+Enter, `Ctrl+G` / `Ctrl+Shift+G` and F3 / Shift+F3 navigate; Escape
closes it. Spaces are preserved, including an all-space query. Queries are
limited to 4 KiB and results to the first 1,000 nonoverlapping occurrences, with
an explicit limit notice. This is literal search, not a regex input mode.

The existing immutable `TextDocument` is shared by `Arc` with the bounded Rust
worker pool. Search does not reread the file or send its contents over HTTP.
Regex's escaped literal matcher supplies Unicode simple case folding and exact
byte ranges; the shared line index supplies physical source lines. GTK receives
bounded results and the existing 200-line / 128 KiB source pages. A match late in
a long line opens a window at its byte position.
[GtkSourceView SearchContext](https://gnome.pages.gitlab.gnome.org/gtksourceview/gtksourceview5/class.SearchContext.html)
asynchronously highlights only the displayed page; full-file counts are from
the worker. Binary snapshots disable Find. Truncated diffs search only their
available snapshot and retain the existing truncation notice.

Input is debounced for 140 ms. New queries, replaced files, closed or hidden
panes cancel obsolete work, and independent query/file generations reject late
results. Matching and navigation never write the source or send a model prompt.
Returning to a hidden Source pane retains a completed search's selected match
and current page; interrupted searches resume against the same snapshot.
Search controls and result labels follow the current interface language; file
text and the exact query remain literal. Source/Edit switching preserves the
in-memory edit page independently of the saved snapshot searched by Find.

```bash
python3 scripts/smoke-gnome-source-search.py NATIVE GATEWAY
```

The private Wayland fixture uses a 4,229,226-byte code file plus a copy of the
actual `native/gnome/src/app.rs`. It checks exact trailing spaces and Unicode,
cross-page and long-line navigation, case folding, bounded results, live language
changes, obsolete replies and source-file preservation. The installed `--viewers`
fixture separately exercises native Source keyboard search without application
test hooks. Source find does not complete advanced diff review
or physical-desktop acceptance.

`source-search-mg5ze8fa` passes the workflow above, including retaining a match
and its page when the pane is hidden and shown. Native tests pass 187 cases
(two ignored), and Rust 1.92 checking passes. The separate two-launch
`source-comments-2q12fjcp` regression passes after the search integration.

The debug private fixture observes roughly 0.5-second heartbeat gaps during cold
opening on the default renderer path. Temporary instrumentation found Source
dispatch below 1 ms, Source language/page presentation at 4 ms, and event handlers
at most 4 ms; disabling syntax highlighting did not remove the gap. A controlled
`GSK_RENDERER=cairo` Wayland run (`source-search-07d5lrsj`) opens the same
4,229,226-byte file in 43 ms, with a 25 ms opening heartbeat gap and a 52 ms
maximum subsequent gap, including debug screenshots. Observed native peak RSS
falls from 361,656 to 163,432 KiB; both samples exclude gateway/indexer memory.
This comparison points to renderer-path cost in this setup, rather than proving
a parser or syntax-highlighting bottleneck. It does not establish performance
on physical GNOME hardware. The application retains GTK's renderer selection;
`GSK_RENDERER=cairo` is a tested optional Wayland workaround. The fixture records
opening and later heartbeat gaps separately; physical-desktop acceptance remains
open.

The optimized pair passes the installed five-option run in
`release-ui-0sih2px4`, without application test hooks. Its Source case opens a
real patch file through the installed desktop entry, searches for `after  `
with two trailing spaces, and verifies Enter, Shift+Enter, Ctrl+G and Escape
using owned Wayland keyboard input. The broader diagram, language, backend
editor, Git, viewer-return and process-cleanup checks also pass. Packaged binary
hashes match the final release outputs.

### Native code and text editing

Identified writable UTF-8 files now have parallel **Source code** and **Edit**
views. Git/turn diffs, binary/truncated snapshots, missing file identities and
shared-directory files remain read-only. Markdown uses its existing native
Preview/Source/Edit reader; this pane serves ordinary code and text files.

GtkSourceView owns a bounded edit page, syntax highlighting and native Undo/Redo.
The existing 200-line / 128 KiB source window supplies each initial page, extended
at most one byte to avoid splitting CRLF. Edited pages are limited to 512 KiB,
with at most 8,192 tracked operations / 2 MiB inserted-operation data between
saves and the shared composer's 2,048 disjoint-replacement limit. The existing
complete-file API limit is 5 MiB. Undo history is local to
the current page and survives successful saves; opening another page starts a
new history. Cross-page selections/replacements are not implemented.

View switches retain unsaved edits in memory and continue showing the saved
Source snapshot. They do not write the file. Save / Ctrl+S writes explicitly;
page buttons first save a dirty page, then navigate using its updated byte
boundary. Closing/replacing a file or closing the application offers
Save/Discard/Cancel; Cancel retains the current buffer, while in-flight saves
block replacement/close. There are no persistent editing drafts or blur saves.
Dirty titles use a dot; edit gutters and footer line ranges follow the current
page. New controls, notices and editor footer labels follow interface language
changes without translating file text or paths. Source Find searches the saved
snapshot; Edit Find includes the current in-memory page as described below.

GTK records scalar edit operations, with bounded input normalization and
admission. A reusable GTK/filesystem-independent Rust module replays those
operations through the existing `EditBuffer`, verifies the current page text,
and composes exact byte replacements on a bounded worker. Untouched mixed
newlines, Unicode and bytes outside the page remain intact. No Markdown parse or
whole-file diff is needed. The result uses `/studio/workspace/save` with the
existing hash conflict check and atomic temporary-file replacement. Conflicts
retain local edits and offer Cancel or an explicit destructive Overwrite.
Shared-directory read-only metadata is checked in both the UI and save worker.
Comments cannot capture unsaved edits or hidden saved-source selections.

The shared HTTP save route previously inherited Axum's 2 MiB body limit despite
its 5 MiB file contract. Its explicit body budget now allows JSON escaping
(`6 × 5 MiB + 64 KiB`), while the decoded file limit remains 5 MiB. Text
read/save filesystem work, UTF-8 file decoding and response serialization run on a shared
two-worker admission gate, leaving async session/notification lanes free.
Other platforms retain the same endpoint, path restrictions and conflict data;
WSL's existing editing limitation is unchanged.

```bash
python3 scripts/smoke-gnome-source-edit.py NATIVE GATEWAY
```

`source-edit-brmuvzmg` passes on owned Wayland with a private 4,209,198-byte code
file and a copy of the real application source. It verifies explicit and
page-change saves, exact CRLF/Unicode and untouched file bytes, undo/redo across
saves, dirty view switches, file replacement cancel/save, conflict
cancel/overwrite, close cancel/discard, window/in-flight close guards, shared
read-only policy and live editor translation. The first large-file save takes
259 ms with 22 heartbeat ticks and a 36 ms maximum gap in this debug test run.
Observed native/gateway peaks are 407,736/89,164 KiB, excluding indexer and other
worker processes. These are local fixture measurements, not physical-desktop
or isolated benchmark sign-off. No model prompts are sent. Source search and
two-launch comments regressions also pass (`source-search-xv2e0_o0` and
`source-comments-4gdv7yso`). Native tests pass 188 (two ignored), shared tests 45,
and gateway tests 210 (one ignored); Rust 1.92 checking passes.

The final cursor-visibility fixture also passes (`source-edit-gekzdcrm`). It
places the insertion caret as interactive typing would and waits for GTK layout
before screenshots; Source/Edit return keeps that caret visible. Its first save
takes 304 ms with 24 ticks and a 47 ms maximum gap. Inspected English/Chinese
screenshots show the edited prefix, aligned native controls and updated line
ranges. The final Rust 1.92 check includes these acceptance controls.

The installed optimized pair passes `--all-diagrams --language --backends --git
--viewers` in `release-ui-349xpm5x`, without application test hooks. Find leaves a
later source page open; the test edits that page through native accessibility,
switches to Source without saving, returns to retained edits, and sends actual
Ctrl+S. The entire file is checked independently, preserving the preceding
context. The wider four-launch regression passes in `run-vt6j749q`, including
the real large Markdown report, existing editing/conflicts, tables, Git and
viewer returns. Packaged hashes match both optimized build outputs. Broader
editor input and physical-desktop acceptance remain open.

### Find in editable code and text

Source and Edit share the same Find bar and keyboard navigation. **Find in
editor** searches the whole immutable file snapshot with the current unsaved
page overlaid in memory, rather than just the displayed page. Source continues
to search saved bytes. Literal spaces, Unicode case controls, previous/next,
wraparound, 140 ms debounce, 4 KiB queries and the first-1,000-result limit are
retained. Result-limit tooltips remain complete when the count label is elided.
An empty/failed request can be retried with Enter.

Typing a query selects its first match on the current edit page. If all returned
matches are elsewhere, the count says **Other pages** and waits for an explicit
Next/Previous action. A clean page can navigate without writing. Leaving a dirty
page offers the existing Save/Discard/Cancel guard. Cancel retains the buffer,
selection and file. Save composes the page normally and re-verifies the chosen
logical occurrence; Discard uses its original snapshot coordinates when those
are available. Cross-boundary draft-only matches are searched again after
discard, never presented using stale coordinates. Moving within a page changes
only selection, preserving the edit buffer and undo history. Find itself does
not save, reload or persist drafts.

A bounded Rust worker constructs the LF-normalized working corpus and runs the
existing escaped-literal matcher. It returns at most 1,000 local scalar ranges
and original byte targets. The reusable `text_coordinates::NormalizedCursor`
maps sorted endpoints with a streaming cursor, cached CRLF boundaries, standard
substring search and bulk scalar counts. It avoids per-character mapping tables
and repeated prefix scans. GTK only extracts its bounded edit page, presents
results and selects ranges; GtkSourceView highlights the visible buffer. No
HTTP/file read or Markdown parse is added to Find, and search remains independent
of the save composer's operation/change limits. Editor changes, view switches,
file replacement and late results retain separate query/file generations.

```bash
python3 scripts/smoke-gnome-edit-find.py NATIVE GATEWAY
```

The owned Wayland fixture uses a private 4,207,444-byte file plus a copy of the
real application source. It verifies unsaved matches, exact Unicode/trailing
spaces, case controls, retained undo/redo, cross-page Cancel/Save/Discard,
long-line navigation, mutation-stale replies, saved-versus-draft view searches,
live language labels, Enter retry and unchanged real-source bytes. It checks
that only the explicitly confirmed save writes the fixture file. The initial
run `edit-find-rhqgj4jz` completes a draft query in 222 ms including debounce,
with 22 GTK heartbeat ticks and a 10 ms maximum gap; native/gateway peak RSS is
417,836/69,244 KiB, excluding worker/indexer processes. These describe the private
debug run, not isolated or physical-desktop benchmark acceptance.

The final context/limit run `edit-find-w7jwgp2f` also verifies complete limit
tooltips and replacing an editor with a read-only turn diff. It records 221 ms,
22 ticks and a 10 ms maximum gap, with native/gateway peaks of
399,864/67,592 KiB. Shared tests pass 49 cases and native tests pass 191 (two
ignored), including Unicode/CRLF mapping and matches spanning a page boundary;
Rust 1.92 checking passes. Saved Source, editing and two-launch comments
regressions pass in `source-search-7ancttll`, `source-edit-y_bh0dum` and
`source-comments-mmjjdtdg`. Cross-page text replacement, broader input and
physical desktop acceptance remain separate migration work.

The final optimized pair passes installed `--all-diagrams --language --backends
--git --viewers` acceptance in `release-ui-g_kka1q4`, without application test
hooks. Its editor case finds newly typed unsaved Unicode through actual GTK
controls, verifies the file remains unchanged, navigates with Ctrl+G and closes
Find with Escape before the existing view-switch/save checks. The four-launch
real-report regression passes in `run-ktkd6u6z`. Packaged hashes match the final
optimized outputs; GTK4 remains isolated from the gateway's dependency graph.

## Native tabular documents

CSV/TSV and XLSX files open as read-only immutable snapshots. Data keeps the
source's first row and uses letter column labels. SQL uses the first row as field
names, normalizing empty/duplicate names with the existing Studio SQL engine.
The initial query explicitly lists all fields, one per line; Compact/Expand
changes that list without rewriting expressions, literals, filters or ordering.
Ambiguous SQL/comments are preserved. Exceptionally long Unicode field lists use
`SELECT *` to stay within the 64 KiB SQL limit. Each worksheet retains its SQL and
formatting choice in memory. There are no persistent drafts or automatic saves.

Queries use a temporary in-memory SQLite database, not a process or disk database.
Source values are TEXT; use `CAST` when numeric ordering/comparison is needed.
Projection, `WHERE`, `ORDER BY`, `CASE` and read-only CTEs use the shared engine.
Writes, attached databases and multiple statements are rejected. An invalid query
keeps the previous result visible. Results are bounded to 5,000 rows/8 MiB, with
a two-second SQLite execution deadline and 512 KiB SQLite value/row limit;
source import time is separate from that execution deadline.

Parsing and response serialization run on bounded Rust workers. CSV uses the
[csv reader](https://docs.rs/csv/latest/csv/struct.ReaderBuilder.html), including
quoted separators, newlines, doubled quotes, BOM and ragged rows. Automatic
detection compares comma/tab/semicolon consistency in a bounded sample; the
delimiter menu allows correction. Text must be UTF-8. XLSX uses
[Calamine's streaming cell reader](https://docs.rs/calamine/latest/calamine/struct.XlsxCellReader.html),
with the shared ZIP validation before parsing; a sheet declaring an enormous
sparse dimension never causes allocation of that dense range. Displayed values
include cached formula results; formulas are not evaluated. Calamine is MIT and
csv is MIT/Unlicense; both fit the project's license and compiler requirements.

Files are limited to 20 MiB; XLSX archives also retain the existing 200 MiB expanded
ZIP/10,000-entry bounds. At most 64 worksheets, 10,000 rows and 200 columns per
sheet are retained, with explicit truncation. Cells are limited to 256 KiB.
Snapshot accounting has a 128 MiB budget per workbook and a 256 MiB combined
source/result cache budget, with at most four snapshots. These accounting budgets
are not whole-process RAM limits; parser, SQLite, transport and worker buffers
are additional. Two workers handle normal operations; closing has a separate
lane. Replaced/closed/expired data is dropped outside the cache lock. The client
closes replaced snapshots; idle entries are reclaimed on a later open.

GTK receives at most 50 rows/512 cells per page. Visible cell text is clipped at
2 KiB; Copy and the full-cell dialog retrieve the original value. Native
ColumnView rows wrap to the current column width and columns preserve their
width while paging. Query results and callbacks carry snapshot/sheet/request
identities, preventing late data from replacing a newer file. Charts use bounded
native bars, including negative values, and at most 100 points.

**Comment on selected cell** reads the complete value through the authenticated
Rust `comment` operation. Capturing freezes the snapshot, sheet, row/column,
result ID and conversation owner. The service snapshots the canonical file
identity and applied SQL alongside result data under the same cache lock. A
changed selection/page/query/file/session rejects a stale capture. Comments use
the existing table provider schema and session SQLite store, with optional
header, actual delimiter and frozen SQL fields; opaque/legacy records survive.
The web registry now preserves serialized extension fields for known providers
while still normalizing their established fields. Its table provider keeps the
native query/header/delimiter and describes SQL result coordinates accurately.
This also preserves native PDF character ranges during web normalization.
Empty cells use their bracketed address as the excerpt. A comment supports at
most 16 KiB of value and 16 KiB of SQL, within the shared 32 KiB anchor limit.
Headers carry up to 128 characters. Source lookup supports one A1-style cell
within the existing 10,000-row/200-column preview bounds.

Reopening reads a fresh snapshot and restores its worksheet and bounded page.
A matching file hash and quoted value are required before highlighting and
scrolling to the cell. SQL result coordinates explicitly belong to the result,
not the raw file: the stored read-only query is rerun only for a matching file
hash, then its cell value is verified. Literal spaces in SQL and file paths are
preserved in comment prompts. Changed/unverified files never silently rerun a
stored query or claim a verified cell. Rejected SQL, missing sheets/cells and
changed values leave the source unverified. Older `Query` anchors without saved
SQL open the workbook with an explanation and no inferred cell location.

The private comment fixtures pass five CSV and five XLSX launches plus six SQL
launches: complete clipped-preview values, Unicode/newlines, a later CSV page,
the second worksheet, projected result coordinates, frozen prompts, session
switches during capture, save/restart/source, legacy anchors, mismatches, changed
files and rejected stored SQL. The API fixture also checks TSV delimiters,
authentication, oversized values/queries and stale/closed snapshots. A store
test verifies that capture still returns the immutable value/hash after the
original CSV changes. Actual native dialog and source/result cell highlights
are inspected. No model turns or browser content processes start. These checks
do not establish multi-cell selection, editing or export support.

`scripts/smoke-gnome-tables.py` tests the authenticated service and actual GTK
controls with private CSV/TSV/XLSX fixtures, full-cell Unicode/newlines, manual
delimiters, paging, wrapping/resizing, SQL projection/filtering/sorting, rejected
SQL, stale queries/opens, per-sheet state, charts, highlighting and light/dark
screenshots. The fixture sends no model prompts and verifies backend cleanup.
CI runs its `--api-only` mode without a compositor; GTK acceptance runs separately
on Wayland.
In the latest debug run, a 3,261,041-byte CSV opened in 183 ms and its sorted,
filtered SQL query took 100 ms. A 10,000-row/200-column fixture opened in 311 ms.
The main loop ticked during open/query, with a maximum observed gap of 68 ms
during the large open. Sampled process peaks were approximately 329 MiB native
and 191 MiB gateway across the full fixture, including the wide snapshot. These
are observed synthetic debug measurements, not release or arbitrary-workbook
guarantees. Complex real Excel workbooks, portal selection and physical pointer/
keyboard interaction remain manual acceptance checks.

## Performance and verification

The GTK main loop owns widgets and layout. JSON transport, bounded history
projection, Markdown parsing, and backend work run on Rust workers. Tokio uses two
I/O threads and a bounded two-thread CPU pool; larger JSON, history/markup,
Markdown blocks/table projection, and source/diff indexing use the CPU pool.
Large raw history objects are dropped on that worker before reaching
GTK. Requests/events use bounded
channels, streaming UI updates are coalesced to at most one batch per 33 ms, and
`GtkListView` recycles rows outside the viewport. Only changed transcript rows are
replaced. HTTP and WebSocket messages are limited to 32 MiB; API calls time out.
OpenCode events are bounded to 16 MiB so a supported 10 MiB image part can be
received as base64. Worker projection replaces image payloads with bounded source
references and flat lazy attachment rows before delivery to GTK.
Replacing a backend connection aborts that backend's old socket; other subscribed
backends remain connected. Stale backend events are ignored.

`crates/markdown-native` provides reusable, GTK-independent Markdown blocks,
continued-table source maps, bare PlantUML recognition, and page edit composition.
The editor composes GTK Unicode-scalar edits into normalized UTF-16 patches on a
Rust worker. Raw source windows retain original UTF-8 byte/line anchors.

Isolated Wayland checks with native rich conversation widgets and the debug
gateway opened the real 4,097,226-byte acceptance report in about 1.0 seconds and
a 32,777,808-byte wide-table fixture in about 6.6 seconds, while a 16 ms GTK timer
advanced 62/411 times respectively.
These are development-build measurements on this Ubuntu 26.04 GNOME host, not
release latency guarantees. Source ranges, table paging/column selection,
Unicode/CRLF save, undo/redo across save, and external-file conflict handling passed
on private copies. Reader API tests additionally check authentication, origin,
confinement, stale document identifiers, and table row coverage.

The actual Wayland VTE test uses a private shell/project, VTE paste/commit,
ANSI/Chinese output, and verifies PTY-child reaping. Workspace acceptance tests
use private Git repositories with global configuration/hooks disabled; native
staging, unstaging, history, commit-file/diff selection, folder navigation,
out-of-root symlink confinement, and Unicode source paging passed. Screenshots
are inspected after exercising the actual widgets.

Wayland acceptance also exercises both native comparison panes, mixed modes and
source change navigation; OpenCode SSE, permission/question dialogs and status
ordering; dirty-document replacement cancellation/discard; native pin/unpin; and
a second application launch restoring the pinned session and dark appearance.
Conversation acceptance also covers Codex occurrence search, older turn pages,
legacy read/search compatibility, and a read-only source context. Favorites tests
activate the actual message and selected-text actions, save notes/tags, cancel
closing, retain edits after a rejected duplicate save, discard edits, confirm
removal, copy and export; independent checks read
the shared SQLite rows and exported Markdown, and verify that live session/draft
state stays unchanged. Native Preview/Source screenshots are inspected.
Native preference checks also verify exact multiline environment values,
preservation/redaction of existing secrets, private file permissions, explicit
Save/Reload, discard cancellation, backend registry display and appearance
changes. The native diagram lane has deterministic cancellation/cache tests and
a document-control responsiveness test with an intentionally busy renderer.
`scripts/smoke-gnome-appearance.py NATIVE_DEBUG_BINARY GATEWAY_BINARY` checks the
scoped appearance API and two isolated Wayland launches. It verifies legacy font
defaults, unknown/unrelated preference preservation, invalid font rejection,
stale-edit retention, Save/Reload and close cancellation, GNOME font reset,
existing/new reading widths, VTE typography and the actual composer's Pango font
after saving and restarting. Light/dark screenshots are inspected and GTK theme
parser errors fail the fixture. `--api-only` runs the authenticated persistence
checks without a compositor. These are private fake-provider checks and do not
send prompts to real providers.
The appearance checkpoint passes 747 web tests, 178 gateway tests and 85 native
tests (one intentionally ignored test in each Rust suite), plus optimized native
and gateway checks. Both broad Wayland regressions pass. With the width controls
integrated, a private copy of the real 4,097,226-byte `tests.review.md` opened in
1,004 ms while the 16 ms GTK heartbeat advanced 62 times; table paging, columns,
source mappings, compare and edit/conflict checks also pass. This is a development
measurement on this host. It does not constitute a fresh release bundle or
completion of the full native migration.
The subsequent responsive-layout checkpoint passes
`scripts/smoke-gnome-layout.py NATIVE_DEBUG_BINARY GATEWAY_BINARY` on Wayland.
It measures actual widget bounds against the visible split pane across 320, 420,
600 and 960 pixel widths. Sixteen cases cover normal/long model labels, visible
search, queued-message/reference context, Router controls, and maximum 20-pixel
interface/24-pixel reading typography. The popup's full wrapped model name and
selection marker are rendered and inspected; actual Pango measurements verify
heading hierarchy. Draft text and full model values survive resizing. The
fixture uses private fake-provider state and reaps its owned processes.
`--probe` records diagnostics and explicitly does not count as acceptance.
These are conversation/reader layout checks, not proof of all workspace tools,
physical dragging, screen-reader behavior or fractional-scaling parity.
The broad Wayland regression still passes after this change. The same real
4,097,226-byte report opened in 902 ms with 56 opening heartbeat ticks; source
anchors, tables, comparisons, explicit edits/conflicts, conversations, terminal,
diagrams, settings and restart behavior passed. The full debug workload sampled
approximately 447 MiB native and 169 MiB gateway peak RSS. These are observed
development measurements, not release performance or memory guarantees.
The subsequent conversation-attachment checkpoint passes 88 native tests,
178 gateway tests (one intentionally ignored in each Rust suite), 747 web tests,
and optimized native/gateway checks. The existing file-image viewer/Markdown
fixture and the 10 MiB, source-specific history-image fixture both pass on
Wayland. The broad regression and restart launches pass with the same real
4,097,226-byte report opening in 902 ms with 56 opening heartbeat ticks. Its full
debug workload sampled approximately 440 MiB native and 201 MiB gateway peak RSS.
These checks did not rebuild a release bundle; cold-start, physical interaction
and the remaining application parity/release gates remain open.
Conversation acceptance exercises actual table paging/width retention, column
selection, wrapping, GtkSourceView highlighting/theme changes, native diagrams
and lazy diagram Source controls. Test reports also sample the native process
and owned gateway's RSS/high-water memory from `/proc`; these are observed
debug-workload measurements, not whole-machine or release memory guarantees.
Search checks inspect the matching table cell's actual native viewport bounds,
in addition to case/role/whitespace behavior and obsolete-query isolation.
OpenCode acceptance traverses older message pages, checks explicit legacy partial
coverage and cursor-cycle errors, and opens a saved source from another backend/
session while the original draft stays intact. Transport tests cover opaque
cursor/directory encoding and authentication. A worker regression preserves a
replacement diagram when old-generation cleanup arrives; a Wayland widget check
rejects a late image with reused item/serial IDs, then accepts the current image.
The renderer enables resvg's standard `memmap-fonts` feature. In a paired run of
the same 32.8 MB workload, the native process's observed peak RSS was about
367 MiB before font mapping and 348 MiB after; the gateway remained about
41 MiB. This is a single debug-workload comparison, not a general memory bound.

Chat rows rebind their actions when a history refresh replaces a message with an
equal immutable snapshot. Pointer-identical prefixes retain their widgets;
unchanged table content retains pagination, selected columns, wrap and resized
column widths across rebinding. This preserves exact-current-message guards for
favorites instead of allowing callbacks captured from an older snapshot. Unit
checks cover row identity and reading state, and the Wayland favorites flow
forces an equal-snapshot refresh before clicking the real message action.

Queue acceptance covers both backends, offscreen dispatch, depth limits, ordered
turn options, Pause/Resume, Stop, early completion, durable dequeue and paused
restart recovery. A private SQLite fault verifies that failed edits keep their
text and can be saved after storage recovers. Image acceptance exercises actual
clipboard/file-drop/remove controls, signature validation, a >1 MiB inline image,
failed-send retention, newer images/text added while a send is pending, session
isolation, image-only prompts and explicit queue rejection. Independent checks
verify exact bytes at both fake backends and server-owned OpenCode message IDs.
Native reference acceptance additionally exercises file/skill/plugin selection,
disabled filtering, keyboard/IME-key guards, Unicode offsets, quoted paths,
out-of-order file search, late filter isolation, stable selection, failed sends,
newer references added while sending, session isolation, reference removal and
durable queue playback. OpenCode checks verify exact file URLs, direct and queued
skill commands, response-only rich rendering/completion, server-owned identity,
and restoration of the original paused queue. Screenshots include the actual GTK
completion popover. Unit tests retain non-literal backend fuzzy results and reject
symlink escapes; a delayed HTTP fixture verifies that command waits do not weaken
ordinary request deadlines.

Command acceptance exercises actual palette rows, inline slash selection,
draft retention, recreated-dialog isolation, full-access confirmation/cancellation,
stale permission dialogs, future queued-message policy, shell completion after
an immediate acknowledgement, offscreen shell events and failed-shell drafts.
A completion-before-acknowledgement case queues its first follow-up in that
interval and verifies dispatch after the acknowledgement. Codex and OpenCode
both exercise compact/review; scoped usage and accumulated diffs are inspected
in native status/source widgets. Unit tests also cover oversized diff/usage
metadata and OpenCode structured diff normalization without changing activity.
Private probes against Codex CLI 0.163.0 confirmed the real shell RPC's empty
acknowledgement, `userShell` item/turn lifecycle and child cleanup when the owned
backend process group exits; these probes did not send model prompts or use
existing sessions. The unattended clipboard fixture uses GTK's local image
provider in the same main-loop dispatch. A background Wayland window can lose
its exported selection; the fixture now replaces the clipboard immediately after
initiating paste and checks that the original image still attaches. GTK starts
the read before yielding; the delayed `read_future` initialization previously
allowed that race. Cross-application paste and physical input/focus remain
manual acceptance items, separate from this local-provider check.
MCP acceptance traverses an opaque Unicode cursor, inspects the actual native
inventory dialog, rejects a repeated page cursor, ignores late replies after
reopening and rejects results after a session switch while retaining the draft.
Worker tests verify that schemas/resources are dropped, counts and bounded status
survive, malformed pages remain distinct from an empty inventory, and pagination
budgets hold.
A private installed-app-server probe also verified the thread-scoped
`mcpServerStatus/list` request against an ephemeral thread with an empty MCP
configuration, without using existing sessions or sending a model prompt.
Native agent acceptance opens actual row buttons across three generations,
checks per-agent drafts, refresh retention, unrelated/cyclic-family exclusion,
cursor and ancestry errors, replaced dialogs and session changes during a pending
summary read. Two further Wayland launches select an agent through the UI, then
verify that it restores with its parent and remains in the refreshed sidebar,
without eagerly starting an unrelated backend or dispatching paused queues.
A private CLI 0.163.0 probe verified empty-turn ancestry summaries and the exact
agent source filters without using existing sessions or sending model prompts.
Worker tests also cover a 10,000-candidate scan and explicit partial coverage.
Router settings acceptance exercises real GTK controls and adaptive alert buttons,
both backend catalogs, ordered fallbacks, duplicate rejection, original controller
IDs, newer edits during a save, stale saves, reload cancellation and parent-window
close guards. Independent SQLite/JSON assertions verify exact metadata and
unrelated preference preservation. Gateway tests cover authorization, unknown
preference/capture fields, concurrent-row updates and failures before a settings
commit. A loopback HTTP test checks authentication, the exact read endpoint,
opaque paging and ephemeral/child exclusions. Wayland also verifies an existing
Codex and OpenCode controllers using the actual setup button, including retaining
unsaved provider/fallback edits during verification. The independent authenticated
gateway lifecycle fixture covers creation, concurrent requests, lost HTTP clients,
restart recovery, uncertain creation without replay, registration conflicts,
rename failure, workspace mismatches and unknown/concurrent settings preservation.
OpenCode checks also cover permission confirmation/refusal, recovery without
duplicate creation, Unicode directories and oversized metadata. An optional
installed-server pass verified create/restrict/restart behavior with OpenCode
1.18.31 in an isolated profile, without model prompts or existing sessions.
Native OpenCode prompt transport now preserves developer instructions and JSON
output schemas, with independent 512 KiB limits validated on workers. Structured
assistant JSON is preserved with a stable message/parent anchor in live events,
history, commands and paged source/search, without inventing an idle status.
Unit checks cover the authenticated prompt wire format, escaped JSON budgets and
projection agreement; Wayland checks mount streamed structured output and verify
its parent anchor. These settings/transport checks alone do not establish routed
turn delivery.

An independent headless acceptance fixture exercises the new Router execution
API using isolated deterministic backends, without real model/API calls. It
covers automatic selection and explicit target bypass, all four Codex/OpenCode
controller/target combinations, clarification, rejected/hidden/busy targets,
target model/effort/Fast, project environment behavior and scoped permissions.
Environment checks cover a profile applied through the existing shared API and
then cleared before routing; process-generation isolation and the 4,096-scope
ownership bound are tested without retaining secret values in the registry.
It also tests idempotent request UUIDs, changed-input rejection, completion before
a stale in-progress acknowledgement, lost target ACKs without retry, owned-turn
stops, refusal to stop a different OpenCode message, authoritative OpenCode idle,
confirmed delivery monitoring after restart, uncertain delivery without replay,
inactive unresolved turns after restart, unavailable-backend diagnostics, shared
delivery metadata and child cleanup.
The execution API and native conversation still need browser/native history-key
reconciliation and real-provider/manual verification.
The later checkpoints below add direct images/references, text follow-ups and
explicit targets, followed by OpenCode skill-command dispatch described above.
Queued images remain blocked with their drafts retained.
Its current text prompt limit is 64 KiB,
catalog limit is 2,048 projected sessions, context budget is 512 KiB and
tracked-request limit is four. GTK and the API support bidirectional cursor
paging, with only one page of rich replies retained. Large responses keep a bounded excerpt plus original
backend/session/turn/message identities. Persisted requests are never
automatically resent.

A separate Wayland acceptance fixture now clicks the native send/stop/source/
favorite controls with isolated Codex/OpenCode backends. It verifies visible
reply widgets, correct favorite and history source anchors across backends,
newer draft retention, draft preservation after a stale-configuration HTTP
rejection, controller-only metadata loading, restored job history, no prompt
replay after reopening and owned backend cleanup. A third launch seeds 50
additional private records and clicks Older/Newer/Latest and visible-read controls.
It verifies unread counts beyond the latest page, offscreen/hidden read guards,
versioned read persistence, Attention retention for other unseen replies,
historical-page retention during polling and eviction of old reply trees.
The target picker checks actual GTK rows across two result pages, Unicode/backend
search, empty results, explicit selection, return to Automatic routing, draft/model
isolation and late callbacks after session changes or same-session reopening.
The final explicit send preserves the raw request and adds no controller turn.
Endpoint checks reject unauthenticated, malformed and foreign-controller discovery;
unavailable backends report warnings while retaining available targets. Deferred
transcript scrolls now verify that their original row still occupies the intended
position before changing scroll/focus after a session or page replacement.
The fixture also clicks live target/Return actions for both backends, verifies
original questions in actual GTK labels, drafts/model isolation, targets omitted
from the catalog page, and source lookup beyond the live tail. Delayed metadata
cannot replace a newer session/draft. Native metadata tests reject changed IDs,
project directories and archived targets. A headless context test verifies that
conclusions beyond the display preview reach the next routing request without
extra target-history reads. The context budget/privacy tests include UTF-8,
JSON escaping, legacy records and changed/hidden target scopes.
A separate `--queue` Wayland fixture clicks Queue/Stop/Resume, waits for the full
routed target before advancing, routes a queued follow-up to OpenCode, verifies
newer draft retention, stops with a remaining queued message, then reopens and
requires explicit Resume. Shared SQLite and fake-provider ledgers verify four
exact deliveries and an empty durable queue after playback. Headless API tests
cover stale queue text, atomic dequeue, scoped/absent status results and accepted
UUID deduplication. Transaction tests cover busy-controller rollback and opaque
metadata on remaining messages; native state tests reject unrelated job outcomes,
controller-only completion and reconnect-based release.
The two-launch `--queue-refs` variant queues a 30 MiB file using only metadata,
routes it to OpenCode, preserves newer text/reference chips, and restores a
reference-only file/skill/plugin entry after Stop and restart. A missing file
pauses that entry without consuming it; restoring the file and explicitly
resuming delivers the original references to Codex. Structured server errors
reach the actual queue status. Fake ledgers and SQLite check exact inputs,
reference labels, empty final queues and owned-process cleanup. It also discards
one queued acknowledgement at the native boundary, observes completed status
without consuming the local snapshot, then clicks Retry with a newer `/help`
draft. The same request is confirmed without another model turn or draft loss.
Headless API
checks also reject changed/omitted references and verify accepted UUID lookup
after a file disappears. These checks use private files and fake model providers.
The `--images` Wayland fixture sends a >2 MiB image-only request, keeps a newer
text/image draft while the controller is pending, confirms exact input removal
only after target acknowledgement, preserves images after HTTP rejection and
clarification, and routes to OpenCode. SQLite checks exclude image payloads from
job history. Headless checks cover image input on both controller backends and
both target backends, malformed/remote/type-mismatched input, idempotency and
oversized-history Content-Length preflight with a reduced read window. Only
isolated fake providers are used; real model vision support remains a manual gate.
The `--refs` Wayland fixture exercises actual `@`/`$` completion and removal,
30 MiB file metadata preparation, late preparation after a session change, a newer
file/text draft during routing, clarification, cross-backend file URIs, disabled
target skills, reference-only sending and incompatible Codex skill retention on
an OpenCode target. It also sends a target-project OpenCode skill with a file,
checks the visible pending-identity notice and actual Stop control, and verifies
that final acknowledgement removes only submitted chips and preserves newer text.
It checks completion from a Codex controller to an OpenCode target and from an
OpenCode controller to a Codex target, with project-specific metadata and rejection
of a retained popup action after changing targets. These metadata checks send no
additional model turns except the explicitly submitted private-fixture command.
Fake-provider ledgers verify that controllers receive no typed target references,
that target skill/plugin inputs are preserved, and that file contents do not
enter routing transport. Unit checks cover symlink escape, metadata budgets,
deduplication and disabled target inventory entries. System file-portal interaction
and real provider reference behavior still require manual validation.
Screenshots are inspected separately from model assertions. The headless fixture
also rejects unauthenticated, duplicate, stale and foreign receipts. Store tests
cover legacy migration and cursor boundaries during concurrent insertion.
Physical foreground/focus transitions and real model providers still require
manual verification; the unattended tests use explicit visible-read actions.

Before EPUB integration, the automated checkpoint passed 172 gateway tests and 76 native tests
(one ignored in each suite), the existing four-launch Wayland regression and
the three-launch Router fixture, separate two-launch text/reference queue fixtures, and
one-launch Router image and reference fixtures. The table increment also passes
the compositor-free API fixture and actual Wayland table controls, with the
four-launch regression and queued-reference restart fixture rerun successfully.
The image increment passes its native viewer/reader/conversation fixture,
including clipboard pixels, source switches, unmapped texture release, cache
access revalidation and bounded raster/SVG responses. The full four-launch
Wayland fixture, table fixture and queued-reference restart fixture were rerun
after integration. The reusable Markdown library passes 12 tests, including
image references, nested safe links and original source anchors. Runtime library
inspection found no WebKit/GTK3 in native and no GTK/WebKit in the gateway.
The EPUB increment adds GTK4 WebKit to native; GTK3 remains excluded, and the
gateway remains independent of all GTK/WebKit runtimes. The EPUB increment also
passes the full four-launch Wayland regression and both release-profile checks;
its remaining clipboard and cold-start gates are tracked above.
The PDF increment also passes its compositor-free API fixture and real Wayland
controls, including password entry, deferred search, bounded search windows and
page hints. The full four-launch regression and native table controls were rerun
after PDF integration. Its real-reference-file timings are documented above.
Gateway and native release-profile `cargo check`
also pass.
An additional check of the existing Tauri desktop could not complete on this
development host because its `glib-2.0` development package is unavailable.
That build remains a platform validation gate; the native GTK4 SDK is separate.
The runnable debug binaries are current; release executables/install packages
have not been rebuilt for this increment.

```bash
cargo test -p codex-thread-studio --no-default-features --locked
cargo test --manifest-path native/gnome/Cargo.toml --locked
bash scripts/build-gnome.sh --debug
python3 scripts/smoke-gnome-router.py target/gnome-native/package/codex-thread-studio-gateway
python3 scripts/smoke-gnome-router-runtime.py target/gnome-native/package/codex-thread-studio-gateway
python3 scripts/smoke-gnome-router-command.py target/gnome-native/package/codex-thread-studio-gateway
# Actual GNOME Wayland desktop (debug automation hooks; isolated fake backends):
python3 scripts/smoke-gnome-annotations.py \
  target/gnome-native/package/codex-thread-studio-gnome \
  target/gnome-native/package/codex-thread-studio-gateway
# Source and submission cases use only isolated fake providers:
for source in epub browser pdf; do
  python3 scripts/smoke-gnome-annotations.py \
    target/gnome-native/package/codex-thread-studio-gnome \
    target/gnome-native/package/codex-thread-studio-gateway --source "$source"
done
# Single-cell comments and safe source/result reopening:
for kind in csv xlsx sql; do
  python3 scripts/smoke-gnome-annotations.py \
    target/gnome-native/package/codex-thread-studio-gnome \
    target/gnome-native/package/codex-thread-studio-gateway --source table --table-kind "$kind"
done
for case in codex opencode router reject cleanup conflict cleanup-close; do
  python3 scripts/smoke-gnome-annotations.py \
    target/gnome-native/package/codex-thread-studio-gnome \
    target/gnome-native/package/codex-thread-studio-gateway --submit "$case"
done
python3 scripts/smoke-gnome-browser.py \
  target/gnome-native/package/codex-thread-studio-gnome \
  target/gnome-native/package/codex-thread-studio-gateway
python3 scripts/smoke-gnome-router-ui.py \
  target/gnome-native/package/codex-thread-studio-gnome \
  target/gnome-native/package/codex-thread-studio-gateway
python3 scripts/smoke-gnome-router-ui.py \
  target/gnome-native/package/codex-thread-studio-gnome \
  target/gnome-native/package/codex-thread-studio-gateway --queue
python3 scripts/smoke-gnome-router-ui.py \
  target/gnome-native/package/codex-thread-studio-gnome \
  target/gnome-native/package/codex-thread-studio-gateway --queue-refs
python3 scripts/smoke-gnome-router-ui.py \
  target/gnome-native/package/codex-thread-studio-gnome \
  target/gnome-native/package/codex-thread-studio-gateway --images
python3 scripts/smoke-gnome-router-ui.py \
  target/gnome-native/package/codex-thread-studio-gnome \
  target/gnome-native/package/codex-thread-studio-gateway --refs
# Optional real-server verification (isolated profile, no model prompts):
python3 scripts/smoke-gnome-router.py target/gnome-native/package/codex-thread-studio-gateway /absolute/path/to/opencode
python3 scripts/smoke-gnome-native.py \
  target/gnome-native/frontend/debug/codex-thread-studio-gnome \
  target/gnome-native/gateway/debug/codex-thread-studio
python3 scripts/smoke-gnome-tables.py \
  target/gnome-native/frontend/debug/codex-thread-studio-gnome \
  target/gnome-native/gateway/debug/codex-thread-studio
python3 scripts/smoke-gnome-images.py \
  target/gnome-native/frontend/debug/codex-thread-studio-gnome \
  target/gnome-native/gateway/debug/codex-thread-studio
python3 scripts/smoke-gnome-pdf.py \
  target/gnome-native/frontend/debug/codex-thread-studio-gnome \
  target/gnome-native/gateway/debug/codex-thread-studio
# PDF API only, using libpdfium.so beside the gateway or the explicit override:
python3 scripts/smoke-gnome-pdf.py \
  target/gnome-native/package/codex-thread-studio-gateway --api-only
```

The optional Python script tests the real GTK4 window on Wayland using an isolated
fake Codex and profile. Its automation hooks are compiled only in debug builds,
require the fixture's private backend/profile paths, and never send to real
sessions. Screenshots use GTK's own render nodes, not desktop-wide capture.
Logs, reports, and screenshots stay beneath ignored `target/gnome-native-smoke`.

Ubuntu release notes describe the GNOME 50/Wayland target:
[Ubuntu 26.04 desktop changes](https://documentation.ubuntu.com/release-notes/26.04/summary-for-lts-users/).
The native bindings follow the upstream
[GTK4/libadwaita Rust guidance](https://gtk-rs.org/gtk4-rs/stable/latest/book/libadwaita.html).



### Native remote HTTP(S) images

Markdown image references and historical Codex/OpenCode image attachments can now
use public HTTP(S) URLs through Rust I/O workers. Reader/chat/table factories
share the existing bounded image renderer, animation decoder and GTK viewer;
opening the viewer freezes the downloaded original so an explicit Save writes
its exact PNG/JPEG/GIF/WebP/SVG bytes. Source text, anchors and unsent drafts stay
unchanged. Images do not start WebKit or execute HTML/JavaScript.

The independent GNOME workspace enables Reqwest's native TLS connector using
Ubuntu's OpenSSL and certificate trust. Other-platform and shared gateway
dependency inputs remain unchanged. The development installer already supplies
`libssl-dev` and CA certificates. Linux transport notices are reproducible with
`python3 scripts/vendor-gnome-tls.py --check` after fetching the locked GNOME
packages, included in the native bundle, and checked by GNOME CI. OpenSSL itself
is dynamically linked from Ubuntu rather than bundled.

Every initial URL and redirect rejects user-info, non-HTTP schemes and invalid
references. DNS results are checked before connecting and pinned for that hop;
any loopback, private, link-local, reserved or supported translation-path address
rejects the entire result. This automatic image path intentionally accepts public
addresses only; private-network images retain their readable link/alt text.
HTTP clients never receive gateway authorization, browser cookies, a Referer or
proxy configuration. Four redirects, a ten-second whole-transfer deadline,
three-second connection/read deadlines, 10 MiB source and two network workers
bound each load. HTTPS redirects cannot downgrade to HTTP. Existing image codec,
SVG and decoded-pixel bounds still apply; SVG subresources remain embedded-only.
A 32-entry/32 MiB, five-minute process-memory source cache avoids repeated reads;
there is no new disk cache. Unmapping/replacing a preview signals cancellation;
its admission slot stays owned until the cancellation/result is acknowledged.

The private `remote-images-5lat77jj` Wayland run uses immutable PNG/SVG files from
[the official Rust artwork repository](https://github.com/rust-lang/rust-artwork)
with commit, digest and original artwork license retained. It checks actual
production DNS/TLS, native chat/attachment/reader display, viewer controls, exact
Save originals and retained draft/source. A loopback canary receives no requests;
no browser engine or model turn starts. Representative screenshots were inspected.
The original embedded-image regression also passes as `embedded-images-dada0hbw`,
including original bytes, source lines, stale controls and SVG resource isolation.

Reproduce the explicit public-network UI test using the retained private fixture:

```bash
python3 scripts/smoke-gnome-remote-images.py /path/to/debug-native /path/to/gateway \
  --fixture /path/to/pinned-public-artwork-fixture
```

Installed optimized run `release-ui-0444cnie` additionally exercises public HTTPS
reader/viewer/original Save with `--remote-fixture` alongside every existing
release group and process/event-gap recovery, without application hooks. The
first test run saved the remote original correctly but raced asynchronous Close
with the next desktop activation. A wait initially placed in the wrong test
branch was corrected; both failed traces remain. The accepted run observes the
reader return and requires the next viewer status to be actually showing.

Normal unit/CI tests use owned HTTP fixtures and make no internet requests.
An otherwise ignored `public_https_image_uses_production_dns_and_tls` test is
separately exercised with the explicit fixture URL and expected digest. All 298
native unit tests pass with four opt-in fixtures normally ignored; all native
release targets check on Rust 1.92. Full migration and the wider desktop/provider/
platform acceptance gates remain open.


### Native continuous-selection translation

The continuous native Preview now exposes the same Translate context action and
Ctrl+Shift+T shortcut as ordinary selections. Shortcut dispatch calls the selected
widget's bound capture action, allowing asynchronous projected text rather than
raw GTK child-anchor characters. TextBuffer mark changes keep the current
selection owner up to date. Ordinary labels and small rich paragraphs retain
their existing capture path.

A reusable Rust `flow::Prefix` cursor walks the immutable projection once, expands
formula/image alternatives, excludes decorative controls, and suspends only when
it needs an indexed table. The native reader requests that table's remaining
Unicode prefix through its independent two-request selection lane. Both Copy and
prefix reads use the same bounded table-parser traversal; the prefix path stops
before visiting another fragment once enough text is available. It does not turn
partial table text into a complete Copy/comment capture or invent source anchors.

The authenticated `table-prefix` endpoint takes a table ID, character limit and
revision, validates the document header/snapshot before and after worker I/O, and
returns an explicit completion flag. Its outer `work` counters describe budgeted
fragment/header bytes and pages, not physical storage I/O. The API allows at most
131,072 Unicode scalars / 512 KiB; translation requests at most its existing
16,000-scalar limit. Source/selection changes, unmapping, replaced buffers,
backend changes and cancelled replies cannot start an obsolete translation.

Private `documents-8y36fpi1` validates every tested Unicode prefix against exact
full-table output, CRLF/source/quote behavior, auth/origin/revision checks, invalid
budgets/IDs and closed readers. Its 33,554,459-byte generated table yields 16,000
characters after six fragments and 17,646 budgeted source bytes. One observed API
read took 3.22 ms, excluding indexing; this is a host sample, not a latency bound.
Two actual held HTTP tests verify exact/prefix selection reads leave document
controls responsive and close their connection when the offer is cancelled.

Private Wayland `translation-1mbvy8c6` uses an actual owned-compositor Ctrl+Shift+T
event plus the native context action on a 33,554,550-byte table document. It checks
the exact Unicode/math/table prefix in the dialog and private provider request,
cache reuse, source/draft retention, and a paused table read invalidated by a
Source switch. Existing Codex/OpenCode/local-provider, browser/EPUB/PDF, speech,
cleanup and stale-backend translation checks pass. Only private fake providers
and silent speech are used; no real model turn or production session changes.
The source/result screenshot was inspected.

All 104 shared Markdown tests with syntax enabled, 247 gateway tests and 299
native tests pass. The usual 1 gateway / 4 native opt-in fixtures stay ignored.
Rust 1.92 checks all native release targets; the existing Linux GTK3/Tauri desktop
also passes compilation with its separate SDK. The matched optimized pair is
packaged separately; earlier accepted bundles and the default install remain
untouched. Normal continuous-reader promotion/input/accessibility and the wider
migration gates remain open; this increment does not silently enable that
experimental renderer by default.

#### Selection-lifecycle follow-up

Projected translation capture now has selection-owned cancellation, separately
from the document's shared cancellation token. Changing/clearing a selection,
changing/replacing its text buffer or unmapping its view drops the pending read
and immediately releases its action. An old completion cannot clear a new
request. Two lifecycle unit tests cover supersession, duplicate admission and
ownership release. Private Wayland `translation-zzkt7mmd` verifies actual cursor
input drops a paused table delegate before release, permits a fresh selection's
cached translation, and also drops a paused read on Source switching. The full
existing translation/provider/speech fixture passes with exact request counts;
only private fake providers are used. Its source/result screenshot was inspected.

All 301 native tests pass with four opt-in fixtures ignored; Rust 1.92 checks all
release targets. Debug and optimized builds are packaged separately in
`/media/rui/studio-gnome-deps/candidates/native-selection-cancel-20261103`.
Optimized frontend SHA-256:
`ceca313a861846da2789ad0118ba428042e9b93d2a29d9e05dc8bdc8f048f43b`.
Gateway and Mermaid worker hashes, dependency inputs and every other packaged
file match the preceding 488-file bundle. That bundle and the default install
are unchanged. `BUILD.json` and private
`native-selection-cancel-20261103/SELECTION-CANCELLATION-EVALUATION.json` retain
source/package/evidence identities. This is focused adapter acceptance; the
broader installed-workflow evidence remains the preceding candidate's results.
It does not promote continuous Preview or close the full migration goal. The
[current parity inventory](gnome-native-status.md) records the next porting work.


## Native structured-file Preview

Identified, complete ordinary files now offer native Preview/Source/Edit. Rust
workers prepare an immutable formatted snapshot with sparse copied-byte mappings;
the original document remains the sole save input. JSON variants use pinned
`jsonc-parser` token ranges, XML uses `quick-xml` pull events, assignment formatting
preserves quoted/multiline values, and YAML indentation remains unchanged.
Comments, duplicate keys, numeric spelling, Unicode and escape spelling stay
literal. XML mixed content and `xml:space="preserve"`, invalid input and exhausted
budgets retain the complete original text. No parser resolves external resources.

Formatting admits at most 256 KiB input, 1 MiB output, 65,536 tokens/map segments
and depth 128. Generic Source/Preview/Edit windows admit 16 KiB / 200 lines,
with up to two extra bytes to keep CRLF intact. This changes ordinary-file text
windows, not the separate indexed Markdown reader. The full immutable original
still occupies memory and full-file search remains available; a small widget
window is not a claim of zero-copy or bounded whole-file storage.

Preview numbers describe formatted display lines. Search addresses display text,
including generated whitespace; comments map literal quotes back to original
physical lines/hash. Persistent anchors keep the existing schema, so duplicate
quotes can remain unverified. Explicit source-line targets (including line 1),
comment reopen and diffs retain Source. Ordinary opens choose Preview where
supported. Dirty view switches preserve in-memory edits; explicit Save and page
crossing save the original editor operations, then refresh the reading snapshot.
No formatted text is written over the original.

Private baseline `structured-1xq2gt9w` measured a 7,695 ms event gap on a 4 MiB
single-line JSON file and missed its 5-second paint deadline. Disabling syntax
paint alone left GtkSourceView's language context engine attached; detaching that
engine removed criticals but did not remove the large text-layout stall. The
16 KiB window reduced it: repeated later checks paint the context in 489–538 ms,
with maximum sampled event gaps of 129–134 ms. These are staged host samples,
not portable latency bounds. Baseline logs are retained, but its executable was
not frozen and must not be represented as an immutable benchmark binary.

The shared native syntax admission detaches languages on unsafe pages/long lines
and restores them after safe edits/Undo. Native read-only Preview keeps Adwaita
syntax colors without reporting absent page context as a syntax error. Actual
paint checks exposed both a Preview selection above its viewport and a dirty
Source/Edit return with a hidden cursor (`source-edit-j_g8ko0z`). Source, Preview,
Edit and search now share a frame/adjustment-aware reveal helper, scoped to the
current buffer/selection and cancelled by user input or view invalidation. It
uses affected GTK frame/adjustment events, without idle polling. Two subsequent
Source/Edit runs pass, followed by the final packaged regression.

Final private `structured-_u9co6i2` verifies native keyboard Copy with an independent
clipboard read, source comments, cross-page Find, exact original bytes, explicit
and page-change saves, Undo/Redo, long-line syntax admission, read-only roots,
stale replies, narrow panes, live language and dark style. It paints the 4 MiB
context in 489 ms and samples a 134 ms maximum event gap; peak RSS over the entire
fixture is 392,068 KiB native / 58,336 KiB gateway. These are whole-process peaks,
not marginal file allocations. No web-content process or provider prompt starts.
Final `source-edit-1fuk8p0t` and `edit-find-2ut8886y` also pass exact CRLF saves,
conflicts/close guards and search in dirty pages. Earlier failed scroll/style
probes are preserved; stronger assertions were retained.

All 110 shared syntax/structured tests and 302 native tests pass. The existing
247 gateway tests (one opt-in ignored) and Linux Tauri compilation passed in this
batch; subsequent changes are confined to native presentation and the optional
structured formatter. Four native opt-in fixtures remain ignored. Rust 1.92
checks all native release targets. Exact pinned formatter notices are checked
and included in both packages; existing clients do not enable this optional
formatter feature.

The final optimized matched pair passes private installed
`release-ui-_s78j823` through native AT-SPI/Wayland input, without application
hooks or a gateway override. Every optional workflow group is enabled: native
Preview/Source/Edit and exact original saves, language/backend/Git controls,
viewer returns, PDF/table/image/animation, all diagram inspectors, native portals,
actual public HTTPS artwork Save, and process/event-gap recovery. Only private
profiles, fake providers and copied/generated files are used. Earlier broad
native/real-report runs predate the last viewport/style fixes; they are retained
as earlier regression evidence, not relabeled as tests of this binary.

The separate 490-file bundle is
`/media/rui/studio-gnome-deps/candidates/native-structured-preview-20261103/package`;
its local alias is `target/gnome-native-candidate-20261103-structured-preview`.
`BUILD.json` and private
`native-structured-preview-20261103/STRUCTURED-PREVIEW-EVALUATION.json` retain
source/package/evidence identities. Earlier accepted bundles and the default
installation remain unchanged. Remote CI was not run. Static HTML Preview,
continuous-reader promotion/compatibility and remaining release/physical/provider
checks remain work for the full migration.


## Native passive HTML Preview

Local `.htm`, `.html` and `.xhtml` files now use native Preview/Source/Edit,
outline, Find and save controls. A bounded Rust worker prepares passive HTML with
Ammonia 4.2.4; html5gum 0.8.4 supplies preflight/source-heading tokens. The
sanitizer's HTML5 correction is authoritative for security. Source tokenization
is not a replacement sanitizer. Author scripts, resource/style URLs, active
attributes, forms/embedded content and author styles are removed; this is a
passive subset, not an interactive website view or a claim of identical DOMPurify
serialization for every HTML construct.

Only the HTML content uses a lazily constructed GTK4 WebKit surface. Application
controls stay GTK4/libadwaita. The surface has an ephemeral session, disabled
markup JavaScript, denied permissions/file choosers/downloads/new windows and no
navigation beyond its application-owned `about:blank` document. Independent CSP
blocks resource connections, frames, objects, media, workers, forms and base URLs.
An isolated application world handles only bounded selection, theme and layout
messages; document-world code cannot access its bridge. No gateway credentials
or project URLs are passed to this content surface. Opening other text formats
does not start a web-content process.

Preparation admits 2 MiB input, 4 MiB serialized output, 16,384 elements, 131,072
tokens, depth 128 and 2,000 headings. Oversize/invalid-budget HTML has an explicit
Preview notice and retains bounded Source/Edit. The full original is retained
in the existing immutable text snapshot; these widget/format bounds are not a
zero-copy or bounded whole-file storage claim. Native outline labels admit
4 KiB plus an ellipsis. A regression test reproduced a single large Unicode text
token bypassing the initial label check; the fixed scanner keeps complete
rendered text while refusing source-heading verification for truncated labels.
The failed test and earlier trial binaries/source fragments are preserved.

Unique complete heading labels retain original UTF-8 byte offsets, including
CRLF and multiline tags. Ambiguous/truncated headings remain unmapped. Native
outline navigation in Source/Edit uses the existing save guard and freezes a
unique label/depth target, resolving it after saving instead of retaining an
obsolete ordinal. This matters when editing inserts an earlier heading. Reading
view switches retain the old snapshot until explicit Save; sanitized output is
never written over the original. Save operations, conflicts and page limits
continue through the ordinary source editor. Content-process failure retains
Source; explicitly returning to Preview recreates its surface. Pending outline
intent belongs to the current document epoch and survives its initial load.

Rendered HTML comments capture exact bounded quotes and file identity without
inventing source lines/offsets. Existing literal Source verification may leave
entity/markup selections unverified. Precise projected HTML source-selection
mapping is not claimed. The native context action and Ctrl+Shift+T are connected
to translation, but end-to-end HTML provider acceptance and persisted comment
reopen remain part of the control audit. Passive HTML corpus compatibility also
remains distinct from the focused fixtures; stricter sanitization is not proof
that every formerly displayed document has equivalent presentation.

All 308 native tests pass (four opt-in fixtures ignored); Rust 1.92 checks all
native release targets. The root desktop/server manifests and lockfile match the
preceding candidate. The independent native lock adds 24 exact package versions
without removing any previously locked version. Reproducible original license
notices/checksums cover the added graph and are packaged; CI checks them but
remote CI was not executed. The sibling gateway and verified Mermaid component
remain the previously accepted binaries.

Final private Wayland `html-vyzepd31` covers real keyboard Copy with an independent
clipboard read, native outline activation and Find, exact CRLF saves, dirty view
switches, an earlier-heading insertion across the native Save guard, read-only
shared roots, the large-label popup, live language/dark style, oversize Source,
stale file responses, actual owned-renderer termination and explicit retry.
The live HTTP canary has a successful positive control and receives no requests.
An application-owned default-world fetch probe independently proves CSP denial,
so this boundary is not inferred solely from removed markup. Both final
screenshots were inspected. One host run observed 873 ms initial opening and an
81 ms maximum sampled event gap; whole-fixture peak RSS was 427,520 KiB native,
54,956 KiB gateway and 272,172 KiB web content. Process counts include successive
owned document surfaces/retries, not a simultaneous-process guarantee. These
samples are not portable latency or per-file memory bounds.

Final installed `release-ui-7mwegz3a` uses native AT-SPI/Wayland and the installed
launcher, without application hooks or a gateway override. It verifies actual
HTML Preview/Source/Edit and exact original saves alongside structured files,
all optional diagram/language/backend/Git/viewer groups, native portals, public
HTTPS artwork Save, and process/event-gap recovery. Broader focused structured/
source-editor/search checks also pass in the preceding trial; they are recorded
as preceding the final HTML-only label-bound fix, rather than relabeled as tests
of this exact binary. All tests use private profiles, fake providers and
copied/generated files. No real model turn or production session is modified.

The separate 492-file package is
`/media/rui/studio-gnome-deps/candidates/native-html-preview-20261103/package`;
its local alias is `target/gnome-native-candidate-20261103-html-preview`.
`BUILD.json` and private
`native-html-preview-20261103/HTML-PREVIEW-EVALUATION.json` retain exact
source/package/evidence identities. The preceding accepted package and default
installation remain untouched. The [current parity inventory](gnome-native-status.md)
records generic-file Reload, remaining HTML action/reopen acceptance, continuous
reader promotion and wider compatibility/release gates. This checkpoint does not
complete the full native migration.


## Native ordinary-file Reload

Identified files in the native Source workspace now have the same “Reload saved
document” control as the native Markdown reader. This rereads the existing file
service; the global `win.refresh` continues to refresh session catalog/history.
The control is also available for read-only shared files. Synthetic output and
review diffs do not acquire a working-file Reload target.

Reload uses the existing Save/Discard/Cancel guard. Saving still validates the
original revision and handles conflicts before rereading; conflict cancellation
retains the dirty buffer. Discard explicitly drops its edits, and Cancel keeps
them. Successful Reload prepares a fresh editor snapshot/undo history. No
background save or persistent draft is added. Search resets as the file is
reopened; preserving a current view/page is not a promise of identical pixel or
selection restoration after arbitrary external edits.

The new presentation context retains Source/Preview/Edit where the fresh file
supports it. Identical revisions can reuse a valid byte page, including a long
physical line. Changed revisions fall back to a clamped physical line. Invalid
UTF-8 offsets and an empty indexed terminal line after file shortening are not
reused. Empty files remain valid. An Edit/Preview mode no longer supported by the fresh file safely uses Source
(for example, when an editable file becomes read-only). A failed read retains the file target and retry intent;
restoring a deleted file can be followed by Reload without reopening the chooser.
Generation/serial checks reject obsolete replies before changing read state or
restoring another file's mode. Reopening the current viewer preserves the existing
parent-view chain through the shared navigation model.

All 310 native tests pass (four opt-in fixtures ignored); Rust 1.92 checks every
native release target. Two new pure context tests cover changed/shortened/empty
files, terminal LF, long-line pages and invalid UTF-8 offsets. Private Wayland
`source-reload-4alw39_k` uses the real file API and native button/dialog controls
for unchanged-page context, external shortening, Cancel, Discard into Edit,
Save/Reload with exact CRLF, save-conflict cancellation, delete/recreate retry,
a superseded read, read-only structured Preview and HTML refresh. Its synthetic
failed diff has no file target. The actual Chinese Reload tooltip is asserted
independently of the translation lookup helper; that key was previously absent.
The read-only screenshot was inspected. No real provider prompt is sent and all
owned backends/content processes are cleaned up. One host sample observes a
68 ms maximum event gap; this is not a portable responsiveness bound.

The current packaged Source/Edit, editable-Find, structured-Preview and passive
HTML fixtures all pass, respectively `source-edit-dwyurgod`, `edit-find-c7a1prri`,
`structured-dyxp8t30` and `html-74d_op54`. The HTML run retains the independent CSP
probe and actual owned-renderer termination/retry checks. Final installed
`release-ui-6ceubmp2` uses the actual launcher and native AT-SPI/Wayland controls,
without application hooks or a gateway override. It modifies a private source
file externally, activates the file-header Reload and verifies the new bytes are
displayed without rewriting the file. All existing optional diagram, language,
backend, Git, viewer, portal, HTTPS-artwork and process/event-gap recovery groups
pass in the same run. Only private profiles, fake providers and copied/generated
files are used.

The separately packaged matched pair is
`/media/rui/studio-gnome-deps/candidates/native-file-reload-20261103/package`, with
local alias `target/gnome-native-candidate-20261103-file-reload`. It retains all
492 package files; only the native executable changes from the preceding bundle.
Root desktop/server manifests/lockfile, the independent native dependency inputs,
gateway, Mermaid worker and other packaged files are unchanged. `BUILD.json` and
private `native-file-reload-20261103/FILE-RELOAD-EVALUATION.json` retain exact
source/package/evidence identities. Earlier accepted bundles and the default
installation remain untouched. Remote CI was not run. HTML action/reopen
acceptance, continuous-reader promotion and the wider compatibility/release gates
remain part of the full migration.


## Native HTML comments and focused translation

The existing client (`ui/review-notes-controller.mjs`'s
`reopenDocumentComment`, and `ui-src/comment-source-providers.mts`'s relocation)
reopens ordinary HTML comments in Source and locates the rendered quote in literal
source. Native acceptance now exercises that same behavior through the file actor,
actual Comment button, explicit Save, fresh Comments-library load and Open source.
A unique literal quote verifies. A quote spanning entities/inline markup and a
repeated quote remain explicitly unverified. Capture retains rendered text, file
identity and its hash without manufacturing DOM offsets or original-source lines.
The private SQLite check confirms that reopening changes no stored records;
original CRLF document bytes and the conversation draft remain unchanged.
This verifies shared-store reload, not a second application restart.

The first workflow exposed a real keyboard defect. The window's Ctrl+Shift+T
accelerator ran before the HTML key controller, but the global translation action
only considered a remembered GTK text selection. The corrected dispatch first
activates the focused widget's scoped `translation.translate` action. HTML's
existing content menu uses that same action. Selection clear, document retirement
and renderer failure disable it; the negative fixture retains selected composer
text while clearing the focused HTML selection, then verifies no task/dialog is
started for the composer. Source/Preview presentation and source persistence are
unchanged. No project JavaScript or additional resource access is enabled.

A real right-click at measured content coordinates builds the existing WebKit
menu. The fixture verifies and activates the enabled Translate GAction obtained
from that event; it is not physical/AT-SPI activation of the menu item. The separate
assistive probe observed unnamed items in both the existing popup and a
provisional GTK PopoverMenu, even though GTK's descendants contained the visible
labels. Those experiments did not establish a WebKit-only defect or a corrected
accessible menu. Their replacements/helpers were removed. Assistive menu
activation and broader keyboard/menu coverage remain in the control audit.
Failed fixture expectations, focus/menu experiments and provisional sources are
retained privately, rather than presented as successful acceptance runs.

All 310 native tests pass (four opt-in fixtures ignored), and Rust 1.92 checks all
native release targets. Final packaged `translation-f6nrfzz2` passes existing
Codex/OpenCode/local-provider, speech, cache, cleanup, PDF/EPUB/browser and 32 MiB
projected-selection/cancellation checks as well as the new HTML cases. Exactly
one provider turn contains the rendered entity/markup quote; its subsequent menu
action uses the translation cache. Only private fake providers are used.
`html-_k066tuv` passes the existing original-file Save, native controls, independent
CSP canary, renderer termination/retry and other passive HTML checks, with no
provider prompts. Its maximum sampled event gap is 73 ms on this host, not a
portable performance guarantee. Translation and unverified-location screenshots
from the identical focused debug build were inspected.

Optimized installed `release-ui-fcnzzm2y` passes the actual launcher/AT-SPI/Wayland
workflow and every existing optional diagram, language, backend, Git, viewer,
portal, HTTPS-artwork and process/event-gap recovery group. It uses no application
hooks or gateway override and verifies native file Reload, original Source/Edit
and structured/HTML saves, clipboard and managed-process cleanup. There are no
real provider/model turns in this installed check.

The matched package is
`/media/rui/studio-gnome-deps/candidates/native-html-actions-20261103/package`,
with local alias `target/gnome-native-candidate-20261103-html-actions`. Its 492-file
inventory differs from the prior accepted package only in the native executable.
Root desktop/server manifests/lockfile, native dependency inputs, gateway,
Mermaid worker and all other packaged files retain their preceding identities.
`BUILD.json` and private `native-html-actions-20261103/HTML-ACTIONS-EVALUATION.json`
record source/package/evidence hashes and the exact scope above. Earlier bundles
and the default installation are preserved; remote CI has not been run. The full
application goal remains active. Continuous-reader promotion, the control/menu
accessibility audit and release/physical/provider gates remain outstanding.


## Continuous-reader projected drag and input

The full continuous reader used GTK's default text drag, whereas short rich-text
paragraphs already supplied projected drag data. Private baseline
`flow-input-blezchez` loaded a generated document through the normal reader actor,
selected heading/prose/inline math/code and checked independent Wayland Copy and
primary selection before dragging into a native text field. Both clipboard
transfers retained `$x^2$`; the actual default GTK drop omitted the formula. The
baseline executable, source overlay, dependency/source-base identity and logs are
preserved in private `native-flow-input-baseline-20261103`, rather than described
as a hypothetical defect.

Full-document drag offers now reuse the existing lazy primary-selection provider
and its bounded Rust extraction. A drag has its own transfer identity instead of
depending on primary clipboard ownership. Reads validate the mounted view,
immutable buffer/presentation and current selection range before/after worker
extraction. Indexed tables use the existing revision-scoped table-text delegate,
limits and two-worker admission. No original Markdown, comment anchors or API
coordinates are rewritten. The exported format is the same projected plaintext
as Copy: normal rendered prose, formula/source alternatives, code and table TSV;
it is not a serialization of the original Markdown file. Incomplete or over-budget
selection resolution remains an error, not a silently truncated export.

The existing short-paragraph export path stays in place. The shared native gesture
continues to let GTK own click, word/line and fresh drag selection; full-document
exports additionally exclude hits on embedded child controls, so those controls
keep their own gestures. No new parser, web content or dependency is introduced.

New private fixture `scripts/smoke-gnome-flow-input.py` drives real Ctrl+Home /
Ctrl+Shift+End, Copy, independent clipboard/primary reads and pointer input on an
owned Wayland monitor. The reader uses the real document actor; its generated
files and temporary GTK text-field drop target are private. Final packaged
`flow-input-pnu02ccg` verifies exact formula/Unicode/code drops, click-to-clear,
double-click word selection, triple-click formula-line selection and a new mouse
selection spanning paragraphs/formula. Controller-offer memory-stream probes
verify foreign-primary retention, changed-range rejection, buffer-replacement
rejection and Source-mode retirement. A second actual drop exports all 70 indexed
table rows while the native page holds only 50 rows. All fixture document hashes
remain unchanged, no provider prompt is sent, no web process starts, and owned
backends are cleaned up. The text/table drop screenshots were inspected.

The first fixture attempt hit standalone Mutter's stack-position assertion during
startup/fullscreen focus before document loading. The fixture now awaits a real
native window frame and scopes fatal-critical checks to the application. Mutter
still emits stack-position assertions in this headless setup; those stderr logs
are retained, separately from the clean native application log. This is a fixture
compositor limitation, not proof of physical GNOME acceptance or a claim that the
compositor assertion has been fixed.

All 310 native tests pass (four opt-in fixtures ignored), and Rust 1.92 checks all
native release targets. Packaged `inline-labels-x9xgtje0` retains actual existing
rich-paragraph drag, click selection, theme/font/source and primary-ownership
checks. `translation-9jor4eje` retains the 32 MiB table-prefix/cancellation and
Codex/OpenCode/local-provider, speech, HTML/PDF/EPUB/browser workflow checks with
private providers. `flow-pipeline-1abjkivv` uses the actual 6,065,514-byte report
copy and passes code controls, source/dirty-editor transitions, native column
filtering, tables, dense deferred diagrams/images, repeated math and paging. It
seeds only validated GPU caches from an earlier successful owned fixture;
application/document caches remain fresh. Sampled maximum main-loop gaps are 8 ms
for input and 57 ms for this report workflow on the current host; they are not
portable latency guarantees or measurements of a different renderer.

Optimized installed `release-ui-7xio72uw` passes the existing launcher, native
AT-SPI/Wayland controls and every optional diagram, language, backend, Git,
viewer, portal, HTTPS-artwork and process/event-gap recovery group. It uses no
application hooks or gateway override. Root desktop/server dependency inputs,
independent native dependency inputs, gateway, Mermaid worker and the other
packaged files retain their preceding identities. Existing platform binaries are
not claimed to have been run by these native-only checks.

The separately verified package is
`/media/rui/studio-gnome-deps/candidates/native-flow-input-20261103/package`, with
local alias `target/gnome-native-candidate-20261103-flow-input`. `BUILD.json` and
private `native-flow-input-20261103/FLOW-INPUT-EVALUATION.json` preserve the exact
source, 492-file package and evidence hashes. Previous candidates/default
installation remain untouched; remote CI was not executed. The continuous reader
is still opt-in: assistive text, selection presentation for embedded objects,
remaining compatibility/control checks and physical/provider/release acceptance
remain part of the active full-application goal.


## Continuous-reader accessible text and object selection

Private baseline `flow-input-4uuvha16` loads the real document reader and queries
only the owned child's native accessibility tree. Its full-document formula
picture had neither an accessible name nor description. Parent text omitted the
two child-anchor slots: it reported 67 characters for a 69-character GTK buffer,
and a ranged read using that count returned `plain cod` instead of the complete
`plain code` line and its newline. The original executable, source overlay and
raw trees are preserved in private `native-flow-accessibility-baseline-20261103`.
The finding is scoped to the observed target-host implementation, not every GTK
version or text widget.

Full-document formula/eager-diagram pictures now set GTK's literal alternative
text, matching the existing short-paragraph math path. On this host GTK exposes
that property as the image's accessible description; the fixture checks that
actual metadata rather than assuming it must be the name. User formula/source
strings remain literal across interface-language changes. Full Preview also has
a fixed native interface label; small embedded paragraphs are not relabeled.

The existing `AnchoredTextView` implements the public `GtkAccessibleText`
interface, preserving child-anchor slots in its UTF-8 content slices. Counts and
caret/selection offsets therefore share the toolkit's character space. It retains
GTK's attributes, geometry, caret/selection and boundary calculation; character
and line queries return a slice using those same bounds. It does not expand
formula or indexed-table content into an unrelated accessibility coordinate
system or rewrite the backing buffer. This follows the character-range/full-end
contract in [GTK's contents interface](https://docs.gtk.org/gtk4/vfunc.AccessibleText.get_contents.html)
and [boundary interface](https://docs.gtk.org/gtk4/vfunc.AccessibleText.get_contents_at.html),
also checked against the installed GTK 4.22 header. The picture property is GTK's
standard [alternative description](https://docs.gtk.org/gtk4/property.Picture.alternative-text.html).
These APIs predate the target runtime; no dependency update is required.

A bounded selection presenter marks opaque anchored widgets with an accent
outline. It uses the existing `FrameUpdate` mechanism to coalesce actual map/mark
changes, updates only changed classes and schedules no idle polling. Range tests
operate on already-bounded native object metadata; widget state and painting
stay in GTK. Buffer mutation/replacement or unmapping clears its pending work and
outlines. Parent checks prevent an old page from clearing a child already
reparented to another view. Inline formulas use the existing text-selection
background: an inset border was removed after screenshot inspection showed it
boxing their glyphs unnecessarily. Text/model/source positions are unchanged.

Final packaged `flow-input-bfxdloba` uses a private AT-SPI client with bounded
traversal and explicit owned-PID checks on the private bus. It verifies literal
formula metadata; complete counted/full contents; character and line boundary
queries over anchor/tail offsets; native caret and selection after actual
Ctrl+Shift+End; and access to the table header/first visible row. Independent
Wayland Copy/primary/native drops and stale transfer checks remain passing,
including export of 70 table rows from a 50-row native page. Actual rendered
pixels match the native accent in light/dark themes, differ after clearing and
retain the same widget size. Non-vacuous preselected-object checks verify clearing
on Source retirement and buffer replacement. The refined light/dark/table/text
screenshots were inspected; no formula border or new application control remains.
All generated source files retain their hashes, with no provider prompt or web
content process in this fixture. The inherited headless Mutter stack-position
assertions remain separately documented; fatal-critical checks still apply to
the native application, whose log passes them.

All 310 native tests pass (four opt-in fixtures ignored), and Rust 1.92 checks all
native release targets. Existing packaged `inline-labels-gc9gf738` verifies rich
paragraph/source/font/theme/drag behavior; `translation-n53uwnin` retains private
provider, HTML/PDF/EPUB/browser, speech, 32 MiB table-prefix/cancellation and cache
coverage. Actual report `flow-pipeline-b9o5ljge` uses the current 6,065,514-byte
copy and passes existing diagram/image/math/code/table/navigation/editing checks.
Only verified GPU caches are seeded; application/document caches remain fresh.
Its sampled maximum event gap is 56 ms, and the focused input run samples 4 ms;
these are host observations, not portable performance bounds.

Optimized installed `release-ui-w4zwz6ow` passes the launcher, native controls and
all existing optional diagram, language, backend, Git, viewer, portal, artwork
and recovery groups, with no application hooks or gateway override. Dependency
inputs, gateway, Mermaid worker and other packaged resources retain their
preceding identities. Existing non-native platform executables were not run by
these native-only changes/checks.

The matched package is
`/media/rui/studio-gnome-deps/candidates/native-flow-accessibility-20261103/package`,
with alias `target/gnome-native-candidate-20261103-flow-accessibility`. `BUILD.json`
and private `native-flow-accessibility-20261103/FLOW-ACCESSIBILITY-EVALUATION.json`
record source, 492-file package and evidence identities. Earlier bundles and the
default installation remain untouched; remote CI has not been run. This is native
API and pixel acceptance, not a physical screen-reader/IME/fractional-scaling
result or complete format parity. Continuous Preview remains opt-in; the next
interaction audit includes returning to a document from other workspace panes
and its indexed-table transfer lifetime, before default/release acceptance.
The full-application migration goal remains active.

## Continuous-reader workspace return and transfer lifetime

The real pane-navigation fixture exposed a retained-document defect. Full table
Copy worked before opening an image, then failed with `Selection document snapshot
changed` after closing it. The same document and TextView were still current:
unmapping had canceled the permanent indexed-table token. Private
`native-workspace-selection-baseline-20261103` preserves the failing executable,
source overlay and `workspace-navigation-efh7_55r` evidence against the preceding
full source snapshot.

Document invalidation and presentation lifetime now have separate scopes. A
renewable presentation scope cancels on hiding and provides a new token on
remapping; old tokens are never reset. Copy, comments, primary selection and drag
exports capture the visible period that started them. Cancellation drops held
reads, releases busy state and prevents old completions from publishing or
clearing a fresh request. Old drag providers stay invalid after remapping the
same selection. Actual text mutation/buffer replacement still invalidates the
immutable projection and its table token permanently. Translation retains its
existing independently scoped selection-request cancellation.

The packaged `workspace-navigation-wdf9ipu7` uses actual Markdown image dispatch
and native Close controls, then returns from table, source, nested PDF/image and
EPUB panes. Each retained-parent check verifies TextView identity, unchanged
location, exact complete 70-row Copy and independent Wayland primary/clipboard
text. It also freezes the actual drag controller's offer and verifies rejection
after returning. A deliberately held real-table delegate tests Copy and comments:
unmapping closes the old receiver; a fresh request remains busy until its own
reply, then publishes the complete text exactly once. The old reply cannot be
sent or reach the comment sink. Temporary fixture widgets do not reload or replace
the real retained document. Source hashes, no model turns and owned-process
cleanup are checked. Ordinary `workspace-navigation-o9fqpver` passes as well.

All 312 native tests pass (four optional fixtures ignored); Rust 1.92 checks every
native release target. Packaged `flow-input-teqqwkfn` retains keyboard/pointer/
projected-drop, AT-SPI and light/dark selection coverage; `inline-labels-_dwyeud2`
retains rich-paragraph/table/font/source interactions. `translation-2myz0ax5`
passes existing provider, HTML/PDF/EPUB/browser, speech and 32 MiB prefix/cancel/
retry checks. Actual `flow-pipeline-d3duv090` uses a fresh copy of the
6,065,514-byte report, with only verified GPU caches seeded; sampled maximum event
gap is 56 ms, versus 13 ms in the focused input fixture. These are host observations,
not portable bounds. Indexed text extraction still runs on the existing bounded
Rust worker/API path; no recurring visibility polling or document reload was added.

Optimized installed `release-ui-ks9xznsx` passes all existing diagram, language,
backend, Git, viewer, portal, explicit artwork and recovery groups through native
controls, without application hooks or a gateway override. The matched package is
`/media/rui/studio-gnome-deps/candidates/native-workspace-selection-20261103/package`,
with local alias `target/gnome-native-candidate-20261103-workspace-selection`.
`BUILD.json` and private `native-workspace-selection-20261103/WORKSPACE-SELECTION-EVALUATION.json`
record exact source/package/evidence identities. Only the native executable
changes in its 492-file package; dependency inputs, gateway, renderer and optional
runtimes retain their preceding identities. Previous bundles/default installation
remain untouched and remote CI was not executed. Continuous Preview remains
opt-in for the remaining format/control compatibility and promotion audit.
Physical GNOME/IME/screen-reader and real-provider acceptance are separate; this
checkpoint does not complete the full-application migration goal.

## Default native Markdown reader

Ordinary launches and both comparison panes now use the continuous native text
surface. No renderer override is required. The retained block adapter is an
explicit diagnostic selection (`STUDIO_GNOME_CONTINUOUS_PREVIEW=0`), and existing
label-geometry/block-model fixtures select it explicitly. Normal-reader acceptance
uses installed controls and the continuous pipeline. Rust snapshot preparation,
indexed-table extraction, bounded contexts and source anchors are unchanged.

Before promotion, optimized `release-ui-4he75j6b` passes every installed group with
an explicit override. The final default-on `release-ui-75buj_b3` passes without
that override or application hooks: it requires the named native Preview text
interface, exact formula-only Copy and complete cross-block Copy including table
rows. Its normal launcher, independent comparison modes, language changes, Git,
all viewer returns, table-cell/image handoff, original Save, four diagram engines,
portals, explicit public artwork and process/event-gap recovery checks also pass.
The installed fixture now reads actual rendered text instead of assuming that
paragraphs and headings must each be separate accessible labels.

Installed `release-large-s7w4g8jh` opens a fresh copy of the 6,065,514-byte report
and generated 33,555,074-byte LF / 33,574,894-byte CRLF files. All three expose the
default continuous surface. The checks retain exact outline-to-Source line
identity, bounded Source/Edit, explicit/page-change saves, unchanged bytes during
view switches, undo after save, table paging and column selection. The private
outline driver uses Home/Down and actual selected-row identity so GTK realizes
offscreen rows. It does not treat virtualized accessibility child indexes as
model indexes or rely on unavailable global Wayland window coordinates. An older
lookup failed on the current report's offscreen heading; its snapshots/logs are
preserved. The final check reaches line 355 in the report and line 19,818 in both
generated files through normal controls. Its Source windows are approximately
115–131 KiB, with only 51/54 bytes at the generated files' final sections.

All 312 native unit tests and Rust 1.92 release-target checks pass. Packaged
`flow-input-8rnh2e7f`, `inline-labels-jw6_6p2u`, `translation-gkui0dnm`, ordinary
`workspace-navigation-igz1hs7k` and indexed-transfer
`workspace-navigation-_mrcfur3` retain input, accessibility, rich text, translation,
32 MiB prefix cancellation and workspace lifetime coverage. Actual-report
`flow-pipeline-e8vgzzu4` observes 1,066 ms opening and a 54 ms maximum event gap on
this host. Only verified GPU caches are reused; application/document caches are
fresh. The report screenshot was inspected: its table/section alignment is intact;
a transient toast visible there belongs to the preceding intentional distinct-
object admission test. Successful readiness and settled real-report table checks
are separate from that expected negative case. These timings and virtual-desktop
results are not portable guarantees or physical GNOME/IME/screen-reader results.

The retained broad diagnostic now starts its own headless Mutter and private
session bus rather than relying on the caller's desktop. Its generated first
table fits the projection probe's documented 64-fragment / 2 MiB source / 512 KiB
text budgets. This is distinct from the installed 32 MiB tests; their input and
checks were not reduced. Final `run-0zoo3zng` passes its reader checkpoint, comparison, conversation/provider tools,
workspace tools, shared data and restart behavior. Earlier unsuitable first-table
inputs and the old launch failure remain in private evidence.

The optimized package is
`/media/rui/studio-gnome-deps/candidates/native-default-reader-20261103/package`,
with alias `target/gnome-native-candidate-20261103-default-reader`. Its source and
492-file package identities are recorded by `BUILD.json` and private
`native-default-reader-20261103/DEFAULT-READER-EVALUATION.json`. Only the native
executable differs from the preceding accepted package; dependencies, gateway,
renderer and optional runtimes retain their identities. Root/native formatting
checks pass. The final formatting cleanup is confined to debug declarations and
fixtures: rebuilding the optimized executable gives the exact same SHA-256 as
all installed acceptance runs. The debug fixture overlay is preserved separately
for reproducing those debug runs. Current formatted source also passes all 312
native tests and Rust 1.92 targets. Previous candidates/default installation stay
untouched; remote CI was not run. The full-application parity/packaging audit and
explicitly documented physical/provider acceptance remain open.

## Standalone native distribution and runtime validation

ELF dependency inspection confirms that WebKitGTK 6 is a mandatory linked system
library, even though WebKit content processes remain lazy. The runtime package
list now explicitly installs GTK4, libadwaita, GtkSourceView, VTE, WebKitGTK/OpenSSL,
icons/fonts, Bubblewrap, Java/Graphviz and the speech service. The development
installer reuses that list and adds headers/build tools. `--simulate` was used
locally; no host packages were installed or upgraded by this validation.

The 496-file bundle includes `install.sh`, `install-runtime-deps.sh`, a README and
`SHA256SUMS`. Component provenance is still checked during packaging, and the
standalone installer verifies the entire bundle before writing the prefix and
after staging. It uses standard Ubuntu shell/coreutils without Python or a
checkout. Files are installed under a manifest-derived private version directory,
with Studio-named executable links in `bin`. Existing generic tools and the
original Tauri application are not overwritten. Old versions retain their sibling
engines; reinstalling an identical bundle rechecks and reuses its version.

Private `bundle-install-jsa5ths6` uses an actual package and an installer PATH
without Python. It verifies private sibling resolution, a prefix containing
Unicode/quotes/dollar/percent/spaces, unchanged unrelated `d2`/Java/PDF/Tauri
canaries, repeat installation, old-version retention, absence of a removed optional
engine in a new version, checksum rejection before prefix creation and a valid
GNOME desktop entry. The original source bundle retains its complete checksums.
Optimized installed `release-ui-z2x7d1fa` then passes all existing diagram, language,
backend, Git, viewer, portal, public-artwork and recovery groups from the private
versioned layout, without application hooks or a gateway override.

A separate empty package status and official Ubuntu source/keyring resolved and
downloaded 337 runtime/dependency archives into a private external directory.
Extraction ran with the host filesystem read-only, wrote only that directory and
executed no package maintainer scripts. The native binary's `--help` succeeds
against that clean extracted library set in a read-only Bubblewrap root: no host
system filesystem, home, source checkout, network, display or backend is mounted.
This proves dependency loading/CLI startup, not a configured fresh GNOME session;
post-install triggers and interactive desktop acceptance are separate. A new CI
runtime-only Ubuntu 26.04 job installs the archive without a checkout or Rust
compiler and runs the same startup smoke after normal APT configuration. Remote
CI has not yet executed. The tar archive preserves executable modes.

Current preservation checks pass 725 web tests; 247 gateway tests (one optional
fixture ignored); 36 indexer tests; 110 syntax/structured Markdown model tests;
and five SVG resource tests. The default Tauri desktop path type-checks against
the isolated GTK3 SDK, preserving GTK4's independent workspace. This is a Linux
check; it does not claim that macOS or Windows desktop executables ran locally.
The native release/gateway/renderer and all previous runtime bytes match the
preceding qualified candidate. Its 312 native tests and reader/input workflows
remain covered by that unchanged native source/executable identity.

The distribution candidate is
`/media/rui/studio-gnome-deps/candidates/native-distribution-20261103/package`,
with alias `target/gnome-native-candidate-20261103-distribution`. `BUILD.json` and
private `native-distribution-20261103/DISTRIBUTION-EVALUATION.json` preserve exact
source, package and evidence identities; previous packages and the default
installation are untouched. This closes concrete distribution gaps, not the full
migration: the control audit still requires native session resources, editable
opening questions/session information and AI responsibility generation. Existing
manual role editing must be reused, and connection/activity views must be mapped
against their existing equivalents before adding duplicate controls.

## Native session Resources

The native application menu's Resources action opens an exclusive GTK workspace.
It indexes only the latest turn's user, assistant and plan text, excluding tool
output and fenced code. The existing shared `pulldown-cmark` parser with SIMD
handles Markdown/reference links and images; bounded plain-URL/path matching
complements it. All parsing, normalization and case-insensitive filtering run on
Rust workers. GTK receives immutable resource/occurrence models for its virtual
list. The worker serializes and coalesces pending requests rather than launching
an unbounded set of blocking tasks while text streams.

Search and All/Web/Files/Code filters retain up to 16 sessions' in-memory view
preferences. Each resource offers Open, occurrence selection/Go to message, Copy
address and Save resource. Source navigation uses the existing read-only history
window and keeps the live composer unchanged. Saving opens the existing shared
favorite editor with selection scope and exact backend/session/turn/item anchors;
no second database or favorite schema is introduced. Forwarded Router occurrences
retain their original session/root. Local file/media views reuse their existing
read services, including shared-root and symlink checks. Folder references expand
the existing project tree, retaining its normal expansion/pagination limits; they
never promote an assistant-provided folder to an authorized root. Viewer Close
returns to the retained resource search. Explicit Refresh rebuilds the index and
rereads shared-folder settings. Saving those settings also invalidates the resource
index, including when the resource pane is temporarily hidden. Search includes
decoded filenames/addresses as well as the original encoded address.

Extraction admits 1 MiB per item, 4 MiB of latest-turn narratives, 100 references
per item and 2,000 references per index. Complete process-private output snapshots
provide bounded prefixes when the transcript preview was clipped. Missing/oversize
source context or exhausted budgets mark the index partial in its footer. URL
credentials and lexical paths outside the current project/configured shared
folders cannot be opened; the read service remains the final authority. Context
and serial guards reject obsolete result/action objects after filtering, hiding
or switching sessions.

All 316 native tests pass (four opt-in fixtures ignored). They include reference
links, Unicode/line hints, latest-turn deduplication and ordering, excluded fenced
and tool output, full-output prefixes, explicit limits, shared-folder boundaries,
unsafe schemes and credential-bearing loopback URLs. All native release targets
also check with Rust 1.92.

The matching packaged private Wayland fixture `session-resources-o43uqukw` uses
provider-supplied paged history and actual GTK controls. It verifies latest-turn
counts, search/type filters, decoded filename navigation, automatic resource
authorization refresh after shared-folder Save, explicit Refresh, Markdown/image/code/folder opening
and Close returns, read-only source navigation, exact Copy and saved favorite
source/content/question round trips. It checks blocked Open controls, rejection of
an old response, per-session filter restoration after a real session switch, live
Chinese interface labels with literal addresses unchanged, and preserved chat
state/draft and original project/shared-file hashes. Its painted English/Chinese captures
were inspected. The sample's largest main-loop gap was 214 ms across the entire
multi-view workflow; native peak RSS was 395,664 KiB and no web-content process
started. These are debug workflow samples including viewer/dialog work, not an
isolated indexing benchmark or a performance guarantee. All owned provider
processes exit; no real model turn is sent.

The separate candidate is
`/media/rui/studio-gnome-deps/candidates/native-session-resources-final-20261103/package`.
Its matching optimized installed `release-ui-s06679tl` passes all existing language,
backend configuration, Git, comparison, viewer, source-edit/Reload, portal,
remote-image, provider-process and event-gap recovery groups. Math, Mermaid,
PlantUML and D2 inspector/zoom checks pass alongside default continuous Preview
and exact cross-block Copy. This run uses actual installed controls, no application
test hooks, no gateway override and no continuous-reader environment override.
Its gateway, Mermaid component and optional runtime inputs retain the preceding
distribution candidate's identities. The earlier packages and default installation
remain untouched. Physical GNOME/assistive-client and real-provider acceptance,
session information and the connection/activity parity audit remain separate work.
