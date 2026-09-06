# Incremental TypeScript migration

Studio remains framework-free. TypeScript is a development-time correctness tool,
not a rendering-performance optimization. Existing JavaScript modules are not yet
type-checked; this first phase migrates queue normalization, model display,
transcript revisions, and history-tail merging. The second batch migrates the
backend registry, session dispatch/preparation, and catalog merging. Shared
backend contracts are in `ui-src/backend-types.d.mts`.

Dispatch keeps adapter results opaque by default; typed consumers can parameterize
the registry with adapter-specific result types after validating responses.
Backend IDs remain extensible strings, while adapter kinds are a closed union.
The preparation generation counters, in-flight deduplication, and one-time
thread-not-found recovery retain their existing behavior.

Further batches extract these typed boundaries from `app.js`:

- `session-model-preferences`: stored preference validation, serialization, and
  dispatch-time model options (Queue does not snapshot a model).
- `codex-lifecycle-diagnostics`: unknown event decoding and bounded, backend-scoped
  duplicate suppression. Reset maps remain owned by the existing connection code.
- `session-model-cache`: model identity, cache validation bookkeeping, and
  notification routing to selected or background models.

The Codex view-model reducer is now in `codex-native.mts`, with explicit turns,
items, notification parameters, and approval/interaction requests. OpenCode's live
event reducer is in `opencode-event-reducer.mts`. The complete `opencode-native.mts`
adapter now checks conversion helpers, paginated history/tail reads, message
metadata, status normalization, history/live event replay, and loop detection.
Image inputs are checked in `composer-images.mts`.

Codex selected/background history loading is in `codex-history-loader.mts`.
The `history-load-coordinator.mts` module shares resume/refresh flights and retains
OpenCode's connection-epoch boundaries. RPC transport and environment-profile
application are injected; production calls retain their original sequence.

Transcript presentation/windowing, scroll-state decisions, and mounted DOM reuse
are checked in `transcript-presentation.mts`, `transcript-scroll.mts`, and
`transcript-dom.mts`. Catalog sorting/filtering/cache freshness, session occurrence
search, and environment text parsing are also migrated.

`serialized-state-writer.mts` owns the FIFO persistence mechanism. Preferences and
session state use independent instances; payloads are serialized at enqueue time,
and a failed request does not stop subsequent writes.

Comment drafts, provider registration, prompt entries, text markers, and the six
source providers (chat, document, browser, PDF, EPUB, table) are now checked. The
document-review implementation is now checked directly, including formatting,
syntax tokens and annotation-to-source mapping.

Router configuration, candidate selection, decision parsing, and prompt/schema
construction are checked in `thread-router.mts`. Session Map normalization,
navigation, bounded context, structured output, safe automatic operations and
per-map worker serialization are checked in `session-map.mts`. Untrusted Map
operations remain `unknown` until validation; automatic operations cannot mark
items done or delete them. Existing server-side validation remains authoritative.

Asynchronous controller boundaries now use three more checked modules:

- `comment-submission`: waits for an explicit Composer send acknowledgement,
  rejects duplicate in-flight submissions, and removes only the unchanged
  drafts from that acknowledged batch. Failed sends retain drafts and retrying
  an already-inserted batch does not duplicate its text.
- `router-coordination`: per-session start exclusion, atomic claiming of Router
  completions, target dispatch states and cross-backend target completion.
  Transport preparation and existing monitor intervals remain unchanged.
- `session-map-coordination`: shared loads, revision/lifetime-aware writes,
  deletion, and deduplicated AI synchronization. Old reads cannot resurrect a
  deleted Map; obsolete worker results cannot update a replacement Map. Logical
  cancellation suppresses results; it does not claim to cancel a backend request.

DOM rendering, dialogs and transport implementations remain in the JavaScript
controllers and are injected at these boundaries. Browser smoke tests import
the real embedded modules, while unit tests cover rejection, duplicate events,
out-of-order responses and session isolation without invoking paid models.

The next seven batches cover these additional boundaries (54 generated runtime
modules at that milestone):

1. Translation/Continue output parsing and asynchronous result polling, including
   stale-result rejection after an awaited read.
2. Composer draft/reference helpers and Queue dispatch, acknowledgement,
   persistence failure handling and per-session execution exclusion.
3. Preference normalization, thread fork metadata and workset changes.
4. Resource extraction, indexing, revision/count helpers and favorite data.
5. Document formatting/source mapping, table parsing/column sizing/chart data,
   and Git Review diff/filter/read deduplication helpers.
6. Pending RPC requests, one-shot socket requests and the selected connection
   lifecycle. Existing transport topology, reconnect delay and timeout values
   are retained. Synchronous send failures now clear pending requests immediately.
7. Turn navigation, document outlines, Mermaid configuration, right-rail sizing
   and bounded performance statistics.

Remaining JavaScript includes `app.js` integration/orchestration, settings and
annotation/session-map/router DOM controllers, rich document viewer integrations,
and cross-backend lifecycle/SSE wiring. Regular send/Steer and hidden utility-task
creation/cleanup still call typed boundaries from JavaScript; these callers are
not themselves type-checked. Pure DOM controllers and vendor libraries are not
scheduled for blanket conversion. This migration does not add a framework,
bundle the UI, or claim that all frontend code is now checked.

The following focused batch brings the total to 57 runtime modules:

- `composer-send`: acceptance/failure/finalization ownership for ordinary Send
  and Steer. Local acknowledgement errors are distinguished from a rejected
  transport request; there is no new automatic retry. Input capture, backend
  request construction, optimistic rendering and shell/Router sends remain in
  the JS caller.
- `lifecycle-connection`: cross-backend Codex/EPT socket generation guards,
  initialization barrier, reconnect scheduling and notification dispatch;
  OpenCode shared SSE ownership, readiness waiters and gap notification.
  Existing 750ms/1500ms barriers and 1800ms WebSocket reconnect delay are retained.
  OpenCode continues to use EventSource's native reconnect. Backend history
  reconciliation and catalog mutation remain separate existing callers.
- `session-state-persistence`: typed per-session annotation, opening-message,
  model, queue, pin and deletion requests. It retains the existing writer's
  synchronous serialization and FIFO, with no database or endpoint changes.
  Settings form handling and full preference snapshot assembly remain JS.

These are incremental boundaries, not a claim that all sending, lifecycle or
persistence orchestration has been converted. No performance changes or fixes
for the intermittent Continue disabled-state report are included in this batch.

`hidden-utility-session` brings the total to 58 modules. Translation and Continue
share the checked hidden-session creation, structured notification model binding,
turn acknowledgement, existing result polling and backend-pinned asynchronous
cleanup. Their distinct model-selection and staleness policies remain in the
caller; Ollama continues to use its separate gateway. The original timeouts,
polling intervals and Codex-vs-OpenCode request shapes are retained. Tests cover
completion before acknowledgement, failed creation, selection change after
creation and delayed/failed cleanup without invoking a real model.

`started-session-catalog` brings the total to 59 modules. It owns provisional
new/forked session records, catalog generation invalidation, metadata retention,
confirmation logging and the existing 800ms debounced catalog confirmation.
Backend restart/deletion/confirmation cancel only their matching records and
timers. Actual catalog transport, selected-session activation and DOM updates
remain injected from `app.js`; no polling loop or extra catalog request is added.

`selection-coordinator` brings the total to 60 modules. It sequences the remaining
work after a selection's cache has been rendered: Session Map/environment
companions, cached completion, conditional resume and final bootstrap. Codex
still waits for environment configuration before resume; OpenCode does not.
Cross-backend selection still waits for the ready handler's existing initial
load and fresh cache rather than starting another selection. Existing timeout
values and current-selection checks are retained. History result installation,
event replay, scroll restoration and the DOM remain in their existing modules
or JS callers; this does not claim that all history orchestration is migrated.

`history-installation` brings the total to 61 modules. Resume and refresh now
share the typed synchronous sequence: attempt OpenCode tail merge, hydrate if
needed, restore OpenCode metadata, replay buffered events, mark history ready,
merge catalog metadata and cache the result. Selection and OpenCode epoch guards
remain before installation in both callers. Existing reducers and replay
algorithms are reused; no deep copies, requests or scroll changes are added.
Transport, error presentation and performance reporting remain in the callers.
Type annotations describe protocol data but do not replace runtime validation.
No new polling loops, resume requests, or backend model fallback are added.

The release browser smoke imports every generated module through the embedded
HTTP routes, catching missing registrations and transitive imports. This is not
a substitute for live-provider testing: real Codex/EPT/OpenCode disconnects,
send/Steer/Queue acknowledgement races and document selection geometry remain
the highest-risk manual acceptance areas. Tests do not submit paid model turns.

## Source and generated assets

Edit `ui-src/*.mts`, never the corresponding generated `ui/*.mjs` files.
Shared type-only contracts live in `ui-src/session-types.d.mts`.
Keep sources flat for now; adding nested directories requires updating both build
manifest scanners. Generated modules retain their original URLs and imports, so
desktop embedding, SSH routes, and lazy loading do not change.

Run:

```sh
npm ci --ignore-scripts
npm run typecheck
npm run build:ui
npm test
cargo test --workspace --locked
```

Commit sources, generated modules, and `ui-src/generated.sha256` together.
`npm test` checks that compiler output is current; `typecheck` also compiles
negative type-contract tests. CI performs both checks. The pinned compiler is a
development dependency and adds no browser runtime.

Cargo checks source, configuration, dependency, and output hashes before embedding
assets. Thus a checkout can still build with Cargo without installing Node, but
stale assets fail explicitly. After changing dependencies or generation settings,
run `npm run build:ui` again. Hashes normalize CRLF for Windows checkouts.

## Next migrations

Migrate small, independently tested modules before extracting typed components
from `app.js`. Keep strict checks enabled; use `unknown` and runtime narrowing at
untrusted JSON boundaries rather than broad `any` or unchecked assertions.
Types describe history/model contracts but do not validate backend responses.
Keep runtime guards and integration tests for malformed input, notification
ordering, scrolling, route registration, and backend differences.
