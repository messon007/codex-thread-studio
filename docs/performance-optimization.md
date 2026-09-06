# Performance optimization: first batch

## Implemented

- Reconcile the mounted transcript by Turn, retaining nodes whose generated HTML
  is unchanged. Do not keep detached DOM trees for other sessions. Retained HTML
  signatures are bounded to 4 MiB; this limit never hides transcript content.
- Invalidate reused Turn nodes when streaming patches modify them. Use escaped
  ID selectors and a bounded node lookup cache instead of enumerating every
  rendered item for each streaming patch.
- Bind approval and interaction handlers once per node, including reused nodes.
- Coalesce OpenCode startup/health probes and reuse a successful health check for
  five seconds. Do not hold the process-state mutex during network I/O. The
  existing process-exit monitor still invalidates dead connections. Mutating
  requests are not automatically retried; session busy/idle semantics are unchanged.
- Run session-state and favorites SQLite handlers on Tokio's blocking pool, with
  the same database mutex and transaction behavior as before.
- Coalesce concurrent identical Git diff reads, without retaining completed
  responses. A status refresh or stage/unstage response invalidates the read
  revision. Reuse the already validated repository root for the status read
  immediately following a mutation.
- Bound the terminal output channel to 64 chunks of at most 16 KiB. A slow consumer
  blocks the dedicated PTY reader instead of accumulating unlimited queued data;
  received bytes are not intentionally discarded by the queue.
- Use native newline searches instead of per-character JavaScript iteration for
  activity output previews. Existing preview lines and omitted counts are unchanged.

## Verification

- `npm test`: 464 passed.
- `cargo test --workspace --bin codex-thread-studio`: 110 passed, one ignored
  child-process helper which is invoked by its parent test. The health-probe test
  requires permission to bind a loopback HTTP listener.
- The embedded frontend-module route test includes `/transcript-dom.mjs`.
- A Chromium component test verified retained node identity, focus, unsent input,
  expanded details, same-height update scroll position, and backend isolation.

Run the optional browser check/benchmark using an installed Playwright package:

```sh
STUDIO_PLAYWRIGHT_MODULE=/absolute/path/to/playwright/index.mjs \
STUDIO_CHROMIUM_PATH=/absolute/path/to/chrome \
node scripts/benchmark-transcript-dom.mjs
```

The module/executable overrides can be omitted when Playwright and its browser
are installed normally. No new browser dependency is bundled with Studio.

In a local synthetic scenario (30 Turns, 40 paragraphs each, 50 updates), DOM
update p95 was about 16.1 ms for full replacement and 0.2 ms for reconciliation.
This measures DOM work on pre-generated HTML, **not** history fetches, HTML
generation, postprocessing or end-to-end session switching. It is not a WebKit
or Windows WebView2 performance guarantee.

Existing `transcript.dom` performance records now include `reusedTurns` and
`cachedDomBytes`. Use these alongside `transcript.render`, presentation and
postprocessing timings when checking real sessions.

## Remaining work

This is not the entire high/medium-priority roadmap. Large individual Turns and
expanded raw output still need chunked rendering/paging; database connection and
schema-setup reuse, plus blocking-pool conversion for Session Map and EPUB storage,
remain separate work. The first batch deliberately does not add a persistent DOM
cache per session, cache completed Git diffs, modify agent shell execution, or
infer OpenCode completion from an AssistantMessage.
