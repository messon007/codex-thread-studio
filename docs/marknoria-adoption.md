# Fixed public reader adoption — review candidate

The reviewed pin is `scripts/marknoria-release.json`; the unchanged SDK inventory
is `crates/marknoria/reader-manifest.json`. Source revision:
`384876c0391e2d64bbf512fc46af5dec41f7a00e`, 814 inventory files, archive SHA-256
`fe301649a8bb387c618f03bb6eb5b2d3cdb5fe14f2bae5c096d9363f7b54f87c`, tree SHA-256
`d8bb84f1512004395a0291901a44a40dffaa66fc49d5c5677828c444932d6feb`.
Two clean offline exports match; eight archive/inventory/Git-byte checks pass.
The shared JavaScript source and generated resources remain identical to the
runtime-tested `87df867` package. This revision also fixes Unix resource-directory
aliases while preserving Windows authorization before requested filesystem I/O.
The actual three-platform CI exposed that macOS path issue and a Windows-only
owned-runtime/Arc return mismatch in Studio; both now have targeted corrections.
Current platform results must be tied to this new consuming commit.

No private MarkNoria checkout, token, build output, assets or installed executable
is needed. The SDK's MIT allowlist and original third-party licenses/provenance
are retained. Builds never fetch private main/latest. Imported files are not
patched or reformatted. CI formats the application package only and runs the
fixed SDK's independent locked workspace/all-target tests on all three platforms.
The SDK retains its own lock; Studio's dependency unification stays in Studio's
workspace lock.

## Product boundary and public interfaces

Studio uses `marknoria::reader`, `marknoria::web::Service`, the stateless
`web::reader_assets()` router and `createReaderClient` protocol v1. Its host keeps
AI, gateway authentication, projects/shared grants, original-byte transport and
shell controls. No AI/gateway credential enters documents, renderers or the SDK.
Production adapters do not read child DOM or diagnostic globals, inject reader
CSS, rewrite HTML or relax CSP/source/origin checks. Some QA scripts intentionally
inspect rendered DOM/diagnostics; they are not production host dependencies.

All ordinary document entries use the shared reader. Legacy generic Markdown,
table, EPUB and editor renderers/fallbacks are removed; the chat renderer,
existing backend routes and thin public-component test adapters remain.
Fresh documents use Preview; supported explicit/restored Source/Edit stays.
Initial settings/location finish before reader controls become usable. The mapped
child is temporarily inert behind a loading cover, preventing duplicate chrome.
The public client's delayed-child WebKit handshake preserves its caller realm
without weakening exact-window/origin validation.

Shared Notes owns document comments/favorites through Studio's scoped durable
store and toolbar slot; Ask AI calls the existing Studio workflow. Session/chat
annotations remain Studio-owned. Live en/zh/en settings update reader controls,
including Notes and CodeMirror, while literal saved text such as `Close` stays
unchanged. Synonymous translation keys preserve their English control names.

Typed references retain original UTF-8 ranges, source cells, PDF rectangles or
EPUB CFIs and their revision/precision. Legacy line references explicitly declare
approximation; unverifiable offset-only references fail visibly. SQL AI drafts
store a bounded verified historical snapshot and query provenance, survive the
actual database reload after query close, and open without rerunning SQL.

EPUB CFI/theme/font/flow autosave is verified while open and again on reopen.
The read-only WSL adapter uses bounded supervised original-byte transport, stable
distro/user/path identity and per-request image reauthorization before cached
metadata/frames. Its conditional loader does not repeatedly transfer unchanged
original images. Linux runs the exact portable transport; **actual Windows/WSL
execution is not claimed**. EPUB remains explicitly unsupported by that transport
as in the previous implementation. No generic-renderer fallback hides denials.

## Local checks

Ubuntu 26.04.1; Chromium 149 and mapped native Wayland WebKitGTK 2.52.6. The latest
host passes 792 UI tests, generated/type checks, 152 application Rust tests (one
pre-existing ignored) and 14 unchanged public SDK tests. Original SDK/component,
ordinary entry, permissions and two-pane tests are documented in the packaged
`docs/web-parity.md`; that matrix supersedes historical checkpoints.

Actual release checks run without AI credentials or a backend Chromium:

```sh
python3 scripts/import-marknoria.py --verify
npm test
cargo test --workspace --locked
cargo test --manifest-path crates/marknoria/Cargo.toml --workspace --all-targets --locked
# Supply STUDIO_REMOTE_BINARY=.../release/codex-thread-studio and Playwright paths.
node scripts/smoke-reader-shell.mjs --webkit
node scripts/smoke-reader-resources.mjs --common-entries --webkit
node scripts/smoke-shared-tables.mjs --webkit
node scripts/smoke-large-markdown.mjs --edit --webkit
# Reuse the first script's private generated corpus; supply MARKNORIA_SERVER.
node scripts/smoke-standalone-reader.mjs CORPUS_DIRECTORY --webkit
```

`--webkit` requires the native Wayland environment described by the smoke tools.
The complete shell test runs unmodified `app.js`; the large/resource/table tools
also exercise the actual workspace/controller, with their narrower QA fixtures
explicitly distinguished. Four-format same-corpus theme/control/dialog scenes
pass in both products and engines; screenshots were inspected. Chromium tests
physical keyboard events; native helper shortcut tests use synthetic DOM events.
These are not a full screen-reader audit or product-shell pixel identity claim.

Release large Markdown (1.94 MB) and three-engine diagrams (1.15 MB) pass exact
source references, table/query completeness, stable paging, editing/save/undo,
conflict/discard, read-only/symlink/token/origin enforcement and cache cleanup.
Indexing took 49/35 ms at 53,092/49,944 KiB worker RSS. Chromium content took
201/194 ms with 37/19 ms opening heartbeat gaps. Full-shell four-format first-open
and reopen metrics, DOM counts and measurement limits are in the packaged matrix.

**Unpassed cold-platform performance:** fresh private `XDG_CACHE_HOME` Wayland
Markdown still records 1952 ms first content and an 1839 ms heartbeat gap; same-cache
editing is 198/90 ms and diagrams 47/34 ms. Earlier cache-isolation experiments
show GStreamer registry dependence, not the exact blocking call. No prewarming or
copied registry is presented as a fix. Warm results do not replace cold acceptance.

Astra independently accepted the local feature/API/authorization and fixed-package
evidence, but rejected whole-goal acceptance. A focused QA correction separates
product interaction samples from screenshots and drains both realms before
sampling. With Notes already open, first locale update through paint still has
a 433 ms heartbeat spike (398 ms without probes), separate from cold startup;
it is not attributed to Contents or screenshot capture. Cumulative maxima do
not measure repeat-switch latency. This second performance item remains open.
Derived fixture throughput is about 40/33 MB/s; wire byte counts, serialization
CPU, sustained throughput and whole-application peak memory remain unmeasured.
Current downstream CI/review/merge and Astra whole-goal approval remain open. Upstream private CI last reported a billing/spending block;
earlier downstream CI results do not validate this candidate. The original Studio
checkout stays untouched, and commits remain local until the final batch push.
