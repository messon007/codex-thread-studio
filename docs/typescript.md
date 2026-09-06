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
document-review implementation remains JavaScript behind a narrow declaration
contract; that declaration does not imply its implementation is type-checked.

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

The remaining `app.js` is primarily UI and backend transport orchestration. The
settings dialogs, annotation/session-map/router DOM controllers, rich document
viewers and resource extraction remain JavaScript. Migrating these would require
separate feature-level contracts; renaming them with weak `any` types would not
provide the same benefit. This migration does not add a framework, bundle the UI,
or claim that all frontend code is now checked.
Type annotations describe protocol data but do not replace runtime validation.
No new cache copies, polling, resume requests, or backend model fallback are added.

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
