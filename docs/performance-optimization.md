# Performance optimization: first batch

## Optimization principles

Evaluate the Rust backend first for computation that does not depend on browser
APIs. Markdown parsing, outline generation, search indexing, text matching,
transformations, and shared caches should preferentially run there. Use bounded
CPU worker pools, parallel execution, and SIMD-capable libraries when benchmarks
show a benefit; keep CPU-heavy work off the asynchronous request executor.

Reserve frontend work for browser-dependent DOM construction, font and layout
measurement, painting, selection, and interaction. Backend APIs should deliver
render-ready content or a structured rendering model that minimizes frontend
computation. Evaluate efficient DOM construction, incremental updates, containment,
and progressive rendering as part of the design. A frontend Worker is a fallback
for browser-specific work, not the default home for portable computation.

Prefer reusable, high-performance libraries and mechanisms over one-off tuning.
Evaluate alternative Markdown parsers and diagram engines on the same real-world
corpus before choosing an implementation. Preserve GFM behavior, source locations,
comments and selection anchors, and untrusted-content sanitization. Apply shared
mechanisms such as content-addressed caches, request coalescing, bounded concurrency,
cancellation, and progressive delivery across document, chat, and review workflows
where their semantics permit.

Validate placement and library choices with end-to-end measurements, including
backend processing, serialization, IPC or SSH/HTTP transfer, frontend mounting,
and browser layout. Track first useful content, responsiveness, repeat-open
latency, and memory separately. Backend multithreading and SIMD are tools to
evaluate, not assumptions that total user-visible latency will automatically fall.
Check desktop WebViews and remote browsers as well as standalone library timings.

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

## Large Markdown documents

The isolated [Markdown parser evaluation](markdown-parser-benchmark.md) compares
the configured Marked parser with native pulldown-cmark and Comrak. Its timings
exclude sanitization, transport and DOM work; it does not replace the production
parser or introduce a new rendering architecture.

The follow-up [Comrak WebAssembly evaluation](markdown-wasm-benchmark.md) uses the
current source-aware Marked baseline and measures browser conversion costs,
preview mounting, cold start and retained memory. It validates a possible frontend
transition without switching production parsing or adding backend API calls.

The existing Markdown HTML/outline cache, shared parser tokens, deferred outline
binding, lazy images and off-screen table layout remain in place. Additional
paths now avoid redundant work:

- Markdown and HTML outline extraction count source newlines incrementally as
  headings advance, instead of slicing and splitting the entire preceding source
  for every heading. Source offsets, line numbers and duplicate-heading IDs are
  preserved.
- Interface translation skips an inserted subtree when it is inside protected
  document content. This also covers MutationObserver callbacks whose added nodes
  are descendants of the protected container rather than the container itself.
- Chat and document rendering select between reusable plain and PlantUML-enabled
  Marked instances. A single conservative marker check skips the extension's
  repeated remainder scans when the source has no `@startuml` marker. Document
  HTML and outline extraction share the selected parser's tokens. Sources with a
  marker, including nested blocks and incomplete streamed diagrams, retain the
  existing extension behavior; sanitization and diagram limits are unchanged.

A local Chromium component comparison used the same generated acceptance review
with roughly 550 KiB of Markdown, 870 headings and 577 tables. Across three
alternating runs per version, the median time through the second animation frame
fell from about 480 ms to 269 ms; outline extraction fell from 153 ms to 3.5 ms,
and initial translation-observer work from 57 ms to 11 ms. Both versions produced
identical outline entries and rendered the full document. These timings exclude
backend reads and network transfer and do not measure Safari or native WebViews.
Parsing, SVG/HTML sanitization and full document DOM layout still have a cost.

The separate [PlantUML routing follow-up](markdown-parser-benchmark.md#follow-up-source-aware-routing)
compares the old always-on extension against the production source-aware path.
On the same 5 MiB prose snapshot, parse-plus-HTML median fell from about 8.74 s
to 304 ms for chat and 261 ms for the document token-sharing path. This does not
include sanitization, layout, transport or cache hits. The change passes 710
frontend tests and an 11-input Chromium check of identical sanitized chat HTML,
document HTML and outlines, including the real 865-heading review.

## Right workspace divider dragging

Right workspace dividers preview their next position with a fixed overlay and
transform-only guide. Pointer movement uses geometry captured once at drag start;
it does not change the document width, resize terminals/editors, or persist
preferences. Pointer release commits the bounded width and preferences once.
Escape, pointer cancellation/capture loss, window blur/resize and workspace
switching discard the preview. Keyboard sizing and double-click reset remain
immediate.

The overlay also replaces the document-wide drag cursor/selection selectors, so
starting and ending a drag no longer changes styles on every document descendant.
All six right-workspace handles share this behavior.

A native WebKitGTK component check used the same 4,097,226-byte Markdown snapshot
with 609,370 rendered elements and a visible 9,970-body-row, 54-column table.
Across five width movements, the old live-resize handler took a median 2,779 ms
through two animation frames per movement; the preview handler took 32 ms, with
no live document-width changes and one preferences commit on release. This was
one serial comparison per implementation, not a full-app latency guarantee.
Release still incurred one roughly 2.8-second layout on this extreme table;
pagination/table virtualization remains separate work.

## Remaining work

This is not the entire high/medium-priority roadmap. Large individual Turns and
expanded raw output still need chunked rendering/paging; database connection and
schema-setup reuse, plus blocking-pool conversion for Session Map and EPUB storage,
remain separate work. The first batch deliberately does not add a persistent DOM
cache per session, cache completed Git diffs, modify agent shell execution, or
infer OpenCode completion from an AssistantMessage.
