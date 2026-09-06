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
thread-not-found recovery retain their existing behavior. Notification handlers
and model preference persistence in `app.js` are not yet migrated.

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
