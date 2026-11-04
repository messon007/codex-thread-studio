# Native application acceptance review

This review uses the current [product specification](product.md), the implemented
features in the [README](../README.md), and the native migration constraints.
The target is the whole application on Ubuntu 26.04 GNOME/Wayland. Future roadmap
items and explicitly deferred product features are not new migration requirements.

The application UI is GTK4/libadwaita. Website and sanitized HTML/EPUB content
have isolated, lazy GTK4 WebKit surfaces; Markdown, conversation and application
controls do not use an HTML frontend. Existing Tauri/server entry points remain.

## Functional requirements

The numbers below refer to the product's functional requirements. Evidence links
identify the implementation's qualified checkpoints, not a claim that every
historical fixture was rerun for every later commit. Exact candidate identities,
new regression results and inherited checks are in the
[current inventory](gnome-native-status.md).

| Requirement | Implementation and reuse | Evidence and scope |
| --- | --- | --- |
| 1. Threads and session metadata | Native navigation and session controls over the existing backend transport; actual provider IDs, roots, parent and fork metadata. | [Conversation history](gnome-native.md#native-conversation-history), [transport recovery](gnome-native.md#native-backend-transport-recovery), installed core and both-provider navigation checks. |
| 2. Structured transcript and Markdown | Native virtual rows, bounded Rust Markdown model and source snapshots; no terminal scraping. | [Rich streaming](gnome-native.md#native-rich-streaming), native reader/diagram and large-report checks. HTML remains passive or literal text rather than active conversation markup. |
| 3. Composer, approvals, mentions, images, Turn navigation | Native controls reuse authenticated protocol requests, permission projection, bounded attachment preparation and existing queue/Router transport. | Installed command/input/image/navigation fixtures, [agent input](gnome-native.md#native-agent-input), [composer flow](gnome-native.md#native-composer-continue-and-message-flow). |
| 4. Comments | Native selection actions and editor over shared annotation records and prompt templates. | [Session comments](gnome-native.md#native-session-comments), installed bilingual save/restart, relocation and insertion checks. Insertion does not send. |
| 5. Favorites | Shared SQLite, limits, immutable original output, original anchors and source navigation; native editor/library/export. | [Source preservation](gnome-native-status.md#native-favorites-source-preservation), [cross-page question reads](gnome-native-status.md#native-favorites-questions-across-history-pages). Store limits produce explicit errors rather than silent clipping. |
| 6. Appearance | Native language/theme/fonts/contrast/reading controls over shared preferences; literal user/model content retained. | Installed language and appearance checks. [Reading-width and shared-gutter qualification](gnome-native-status.md#native-reading-width-parity) covers actual GTK allocations and restart. Physical text-scale/contrast perception remains a manual check. |
| 7. Diagnostics | Native connection, backend and activity inspection over existing process/transport metadata, with actionable errors. | [Backend configuration](gnome-native.md#native-backend-configuration), recovery and installed language/backend workflows. |
| 8. Streaming performance | Bounded Rust preparation and coalescing; retained immutable completed rows; GTK performs native layout/painting. | [Rich streaming](gnome-native.md#native-rich-streaming), native streaming/model tests and recorded main-loop/RSS samples. Native GTK scheduling replaces browser animation-frame scheduling. |
| 9. Backend isolation | Backend/session/generation-scoped state and shared gateway authentication. | [Transport recovery](gnome-native.md#native-backend-transport-recovery), both-provider navigation, request and stale-result tests. |
| 10. Session Map | Shared revisioned store/context and operation protocol; native controls, Undo, isolated AI work and guarded synchronization. | [Map record](gnome-native.md#native-session-map-in-progress), later synchronization/controls records and installed Map regressions. Its old heading does not describe the present completion state. |
| 11. Structured input | Typed native user-input/MCP controls, secret masking and source-scoped replies. | [Agent input](gnome-native.md#native-agent-input), explicit unsupported-type and permission tests. |
| 12. Artifact workspace | Shared root-confined Rust readers and bounded table/query engines; native PDF, grids and charts. | [PDF](gnome-native.md#native-pdf), [tabular documents](gnome-native.md#native-tabular-documents), installed viewer/query and permission checks. |
| 13. Project environment | Existing profile/secret store, redacted reads, environment ownership and network policy; GTK editor and VTE PTYs. | Environment/API and installed settings/terminal checks; provider processes use private test profiles. |
| 14. Git Review | Existing Rust Git status/history/diff/stage APIs; native list/diff controls. | [Git Review](gnome-native.md#native-git-review), installed private-repository workflow and worker projection tests. No destructive discard action. |
| 15. Session Resources | Reusable Rust Markdown parser and bounded worker index; original shared-folder authority and Favorites store. | [Resources](gnome-native.md#native-session-resources), installed filtering, source occurrence, viewer return, shared-root and literal-copy checks. |
| 16. Selection translation | Existing backend-specific utility-task lifecycle and cache; native dialog/selection actions. | [Translation](gnome-native.md#native-selection-translation), isolated provider/local-model tests, cancellation and hidden-session cleanup. Actual speech quality remains manual. |

## Product acceptance criteria

The product currently repeats displayed numbers 25 and 26. Here **25a/26a**
mean project grouping/cold loading, and **25b/26b** mean Turn navigation/OpenCode.
These suffixes identify the existing passages without renumbering that document.

| Product criterion | Verification and practical boundary |
| --- | --- |
| 1–4: initialize/list, lazy resume, create, rename | Native transport/selection and installed session controls, plus [real installed cold D-Bus activation/Open](gnome-native-status.md#installed-cold-desktop-activation); private real Codex daemon/CLI integration is recorded separately. Real installed CLI credentials/providers are not used by deterministic fixtures. |
| 5–7: completion, steer/interrupt, approvals | Native lifecycle, late-event/reconnect, typed permission tests and private-provider GTK controls. Idle is not assumed equivalent to successful completion. |
| 8–9: three selections, persistence and original anchors | Shared annotation store and native selection/comment fixtures, including repeated selections, restart, exact source and insertion without send. |
| 10, 13–14: offline Markdown, source anchors and local dependencies | Native parser/source model, safe renderer/resource policies and pinned notices; actual table/code/heading selection and source-comment tests. A native renderer does not apply the browser stylesheet, but retains appearance preferences and document safety boundaries. |
| 11: appearance and reading widths | Existing shared-save/restart checks, actual GTK width allocations and the matching optimized installed workflow; see [width parity](gnome-native-status.md#native-reading-width-parity). |
| 12: missing executable/initialization errors | Native spawn/transport recovery tests, diagnostics and installed replacement-process workflow. |
| 15: named project checks | Default-feature root workspace: 430 tests; native workspace: 408 tests; front end: 753 tests; offline npm audit passes at the question-lookup checkpoint. Optional ignored cases and inherited binary checks remain explicitly identified. |
| 16–19, 21–22: fuzzy files, palette, models, skills, tools and local shell | Existing provider metadata and protocol-shaped overrides; scoped native completion/command controls. Private fixtures verify literal input and stale/disabled decisions rather than sending real model turns. |
| 20, 23: retained completed rows and follow behavior | Native streaming/row-reuse tests and actual GTK transcript/navigation checks. Main-loop samples are host observations, not performance guarantees. |
| 24, 25a, 26a: cache/grouping/cold-loading semantics | Backend-scoped native state and history verification, configured Router metadata path, selected-session reads and lifecycle reconciliation. Native initial histories are bounded/paged instead of eagerly projecting full sessions. |
| 25b: Turn navigation | Native Turn markers/source identities and actual both-provider navigation fixtures. The navigator does not manufacture a user question from a response excerpt. |
| 26b–28: OpenCode transport/project scope and backend-local drafts | Reused password-protected gateway, maintained SSE parser, original session directory, native request/generation guards and isolated navigation/reconnect fixtures. Upstream OpenCode lifecycle behavior is not claimed repaired by this port. |
| 29–30: Favorites originals, question choice, restart/search/export/source | Matched installed capture, cross-page source/question and packaged library regressions; exact SQLite/clipboard checks and save/conflict guards. Question failure is visible and does not silently substitute another Turn. |
| 31–32: typed requests and reconnect reconciliation | Scoped request/approval models, native form/URL controls, request-resolution and recovery fixtures; obsolete responses cannot claim a new session. |
| 33: document confinement and limits | Existing authenticated root-confined Rust read/index services, signature/archive bounds, passive document surfaces and private API/viewer acceptance. |
| 34: redacted secrets and matching project environments | Shared environment/profile store and guarded native process ownership. Tests use private profiles/PTYs, never real user secrets or model commands. |
| 35: bounded Git status/history/diffs/staging | Shared NUL-delimited/root-validated Git services, native immutable worker projections and private repository acceptance. Historical views remain read-only. |
| 36: latest-Turn ephemeral Resources | Native latest-narrative worker tests and installed Resources fixture: excludes tool/fence activity, keeps authorized roots, retains source occurrences and returns after Close. |
| 37: bounded/signature-checked image inputs | Shared transport limits and native attachment models; picker/clipboard/drop/history/Router fixtures and failed-send retention tests. |
| 38: Codex Agent hierarchy | Existing ancestry metadata with bounded native preparation and standard source selection; OpenCode's unsupported status is explicit. |

## Remaining release gates

The reading-width follow-up now passes actual GTK allocation/restart checks
and the optimized installed workflow, with its exact package/test identity in
the current inventory. No additional missing implementation was confirmed by
this review. The grouped rows do not establish untested physical interactions
or guarantee freedom from defects.

Remote GNOME CI has not run: the available GitHub credential lacks permission to
publish the new workflow, and the earlier push was rejected. Local tests do not
substitute for remote macOS/Windows builds or an actual fresh GNOME login.

Physical GNOME launcher/notification interaction, real IME composition,
fractional scaling/multiple monitors, actual Orca use and voice quality require
manual desktop acceptance. Real installed cold D-Bus activation/Open, private Wayland/AT-SPI, actual GNOME
portal and isolated Ubuntu runtime checks establish only their documented
automated scope. Cold D-Bus startup is now checked on the native executable,
including service-mode proof, independently of the older fake installer probe. Real-provider
compatibility also remains separate from deterministic fake-provider coverage.

The native migration adds no requirement to implement product-deferred features,
new roadmap ideas, image-region comments or richer Git review comments absent
from the existing application. Changes to common services remain reusable by the
existing client; native controls reuse the bounded gateway rather than copying
provider processes, databases, query engines or document indexing.
