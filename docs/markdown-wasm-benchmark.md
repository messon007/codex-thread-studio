# Comrak WebAssembly evaluation — 2026-10-22

**Implementation update:** document previews now trial Comrak WASM in a disposable
module Worker; chat rendering is unchanged. The evaluation results below predate
that integration. See **Production trial** for the adopted boundaries and tests.

## Decision

**The frontend WASM route is feasible and worth a compatibility-focused prototype,
but is not ready for an unconditional production replacement.** Current Comrak
0.57.0 compiles and runs in Chromium under Studio's existing CSP. Warm document
parsing is roughly 2.6–3.1 times faster than the already-optimized Marked on the
non-diagram workloads. The actual preview component improves by about 10.5%, not
three times, because sanitization and browser layout remain substantial.

A later native-WebKit investigation reproduced a much more severe Marked lexer
slowdown on a different, table-heavy 1.8 MB review. Comrak WASM parsed that entire
document in 61–71 ms after first use. See the WebKit follow-up below; the original
Chromium ratios do not describe this engine-specific failure mode.

No production parser, application dependency, backend API, asset route, CSP or
release binary is changed by this evaluation. The preceding PlantUML routing fix
remains the production implementation. All new Rust code is in a separate
benchmark workspace, with generated WASM and private inputs in ignored `target/`.

## Setup

- Linux x86-64, Intel i5-12500, Node 24.5.0, Rust 1.97.0.
- Main benchmark: Chromium 145.0.7632.6. A separate follow-up uses installed Linux
  WebKitGTK 2.52; macOS Safari and Windows WebView2 remain untested.
- Comrak 0.57.0 without CLI/highlighter defaults, wasm-bindgen 0.2.126.
- Release optimization, LTO, one codegen unit; no wasm-opt, custom allocator,
  explicit SIMD, Worker or threads.
- GFM tables, tasks, strikethrough and autolinks; hard line breaks; raw HTML is
  subsequently sanitized by the unchanged application policy.
- Baseline: **current source-aware Marked 18.0.13**, not the old always-on
  PlantUML extension used in the first native-library evaluation.

Browser warm timings use two fresh-context rounds, three discarded iterations
per round, seven measured iterations per round, and rotating engine order.
Medians below use the mean of the two central values for 14 samples. Native
timings use the same Rust implementation, three warmups and seven samples.
The shared host is not an isolated performance machine; retain the raw samples
and do not interpret sub-millisecond or small percentage differences as firm wins.

The actual acceptance review is the same 563,588-byte snapshot with 865 headings:
`322f494710af9bd16efb0e1c07918ddbdced8b39b4fbd2f6673485c93ff02019`.
Synthetic sources round up to complete blocks; the approximate 5 MiB cases
slightly exceed the existing review-file cap and are parser stress tests, not
proof that Studio accepts larger files.

## Browser parser results

Median milliseconds for **HTML plus outline**, including JavaScript/WASM string
conversion, metadata decoding, IDs and hierarchy, but excluding sanitization,
DOM, file reads and initialization:

| Input | Current Marked | WASM: full result JSON | WASM: HTML + metadata separately |
| --- | ---: | ---: | ---: |
| README, 10.4 KiB | 0.65 | 0.30 | 0.25 |
| Actual review, 550.4 KiB | 34.60 | 14.75 | 13.15 |
| Prose, 5 MiB | 293.50 | 100.05 | 93.60 |
| Tables, 5 MiB | 689.90 | 293.25 | 244.40 |
| Prose + one raw PlantUML at end, 1 MiB | 391.50 | 27.55 | 24.85 |
| One raw PlantUML + prose, 1 MiB | 391.15 | 21.40 | 19.90 |

Prefer the **split result binding**: parse once in Rust, transfer the HTML string,
serialize only the small heading metadata, and free the owned WASM result in
`finally`. There is no need to serialize the full AST into JavaScript. The large
table case makes the full-HTML JSON encoding/decoding cost visible.

The large PlantUML ratios include an **algorithm change**, not just a faster
language/library: the prototype scans raw block boundaries once, whereas current
Marked still uses the original remainder-scanning extension when a marker exists.
A similar index optimization in JavaScript could remove that particular cost
without migrating to Comrak; do not attribute the entire ratio to WASM.

### Native comparison

Median parse-plus-HTML only, without outline work:

| Input | Marked in browser | Comrak WASM in browser | Same Comrak core, native Rust |
| --- | ---: | ---: | ---: |
| Actual review | 29.30 | 11.15 | 6.64 |
| Prose, 5 MiB | 263.00 | 94.40 | 47.70 |
| Tables, 5 MiB | 629.45 | 228.30 | 194.06 |

Native execution still has an advantage in this test, but its column has no
JS/WASM conversion or backend serialization/network cost. These values do not
establish that a backend endpoint would open documents faster end to end. Native
document JSON samples are also recorded, but are not the WASM split-handle API.

## Actual preview component

The real review uses the application's current HTML sanitizer/postprocessor,
an 850-pixel-wide preview pane, the production `artifact-content` class and
off-screen table containment, DOM mounting/layout, and two animation frames.
Nine paired samples follow one warmup in a fresh context, independently of the
compatibility tests. All 22,851 elements and 865 headings are retained.

| Stage, median ms | Marked | Comrak WASM |
| --- | ---: | ---: |
| Parse + outline | 35.60 | 18.40 |
| Sanitize + existing postprocessing | 124.40 | 128.10 |
| DOM mounting + forced layout | 104.10 | 92.20 |
| Through second animation frame | **308.10** | **275.70** |

The total improves about **32 ms / 10.5%** in this component scenario. Per-stage
medians do not sum to the median total. Timing variation and different generated
HTML whitespace can affect layout/GC. This is not a promise of a 10.5% gain for
every document or the complete Studio shell. Backend fetches, caches, comments,
translation observers, sidebar mounting, diagram SVG generation and SSH transfer
are outside this component test.

## Native WebKit follow-up: apparently frozen document opening

After the initial Chromium evaluation, Studio appeared frozen while opening
`check.review.md`: the composer and document close button stopped responding,
although the native window could still close. The document eventually rendered.
The screenshot after completion reports 43,758 lines and 1.8 MB. This is a
different file from the 32,500,120-byte `tests.review.md` initially investigated;
it is below the 5 MiB text-review cap, so the cap does not explain its delay.

An isolated offscreen WebKitGTK 2.52 window, using the real application modules
and that local source, reproduced the stall inside `parser.lexer(source)`, before
HTML parsing, sanitization or DOM mounting. The 12-second diagnostic budget
expired while block tokenization was still progressing. Tokenizer entry/exit
tracing reached table and paragraph rules on the remaining large source; it does
not establish that one particular regular expression is solely responsible.
The same complete file's lexer took about 292 ms in Node/V8, followed by about
25 ms for HTML generation and 21 ms for outline extraction.

Separate, uninstrumented WebKit parser probes measured:

| Probe | Time, ms |
| --- | ---: |
| Marked lexer, first 16,384 UTF-16 code units | 46 |
| Marked lexer, first 65,536 UTF-16 code units | 482 |
| Marked lexer, first 131,072 UTF-16 code units | 1,668 |
| Comrak WASM initialization, local assets | 6 |
| Comrak WASM, entire source, first parse | 160 |
| Comrak WASM, entire source, subsequent parses | 70, 71, 61 |

The complete source is 1,859,199 UTF-8 bytes / 1,499,649 UTF-16 code units.
Its SHA-256 is
`75bd44967dd8405ababa39c9e49ac5ee030110bf4e54e77c4544a008381fc741`;
diagnostic samples are saved in
`target/markdown-wasm-results/webkit-followup.json` without copying source text.
WASM times include HTML transfer and heading-JSON decoding, but not application
outline normalization, sanitization, DOM/layout, compatibility validation or a
backend round trip. Prefix lexer timings and whole-document WASM timings are
different workloads: do not turn them into a measured full-document speedup
ratio, extrapolate an exact full-Markdown duration, or claim these are end-to-end
opening times. These are single-context diagnostic samples, not the repeated
Chromium benchmark methodology above.

This confirms a real synchronous parser bottleneck in Studio's native engine,
not merely a stuck Loading flag or evidence against the existing error cleanup.
It is consistent with the upstream [Marked/JavaScriptCore performance
report](https://github.com/markedjs/marked/issues/2863), but that historical issue
does not prove which expression is slow in this version. Moving the current
parser to a Worker could keep controls responsive without removing its CPU
cost. A reusable native Rust parser should be considered first; the measured
Comrak WASM path is also a promising low-API-change alternative, subject to the
compatibility and memory requirements in this report. No production parser is
switched by these diagnostics.

## First use and deployment

Three fresh Chromium processes load the uncompressed WASM via localhost with
`Cache-Control: no-store`. The shared comparison modules are imported before
the WASM-specific initialization timer. First parses are recorded separately
from warm results; Marked first parses use three additional fresh processes
without initializing WASM.

- WASM-specific JS import + fetch/compile/instantiate: **5.7–8.9 ms**, median 6.0.
- First WASM review parse + outline: **41.9–61.3 ms**, median 48.7.
- Initialization plus first WASM parse: **47.6–70.2 ms**, median 54.7.
- First Marked review parse + outline: **54.0–67.9 ms**, median 62.6.

Ranges overlap and the sample count is small. The warm 13 ms figure must not be
advertised as first-open latency. Remote bandwidth/RTT, browser caches, hardware
and compilation tiering can change the tradeoff. `cachedCompileMs` in the raw
data is deliberately measured *after* initialization and is not cold compilation.

| New asset | Raw | gzip | Brotli |
| --- | ---: | ---: | ---: |
| WASM | 464.9 KiB | 154.7 KiB | 120.7 KiB |
| JS glue | 10.9 KiB | 2.6 KiB | 2.3 KiB |
| Combined | 475.8 KiB | 157.3 KiB | 123.0 KiB |

The current vendored Marked module is only 43.9 KiB raw. Compression sizes are
measurements, not a claim that the production server already serves these
encodings. The prototype succeeds with the actual `wasm-unsafe-eval` CSP and
`application/wasm` MIME type; it does not weaken CSP or require cross-origin
isolation. Production adoption still needs packaged assets/routes, initialization,
failure fallback and a reproducible build. A Worker would be a separate change.

## Memory and responsiveness

Dedicated fresh-context probes parse the same source once, then eight more times,
with GC outside timing. The retained quantities are different and must not be
compared to the earlier Node-process RSS figures:

| Input | Marked JS heap after GC | WASM split binding JS heap after GC | WASM committed linear memory |
| --- | ---: | ---: | ---: |
| Actual review | 6.56 MiB | 1.08 MiB | 9.125 MiB |
| Tables, 5 MiB | 88.51 MiB | 1.07 MiB | 179.25 MiB |

WASM begins at 1.1875 MiB. Its first-parse high-water capacity stays constant
across the eight repeat parses, so this probe does not show an accumulating
handle leak. **That capacity does not shrink when a result is freed.** The
prototype therefore cannot claim a browser-memory saving; a large table may
leave about 179 MiB of committed linear memory in a long-lived instance.

All memory contexts initialize WASM, including the Marked control; the small
initial WASM allocation is not included in the table's Marked JS-heap column.
CDP JS heap, backing storage and WASM committed capacity are not process RSS or
peak live allocation measurements. Lower JS heap does not mean lower total
browser memory.

Both engines still run synchronously on the main thread. The 5 MiB table timer
probe is delayed about 599–764 ms by Marked and 247–283 ms by WASM. Faster WASM
is **not automatically background execution**, and it does not remove the DOM
cost. Backend execution or a Worker remains relevant if that blocking is
unacceptable, but neither is implemented in this evaluation.

## Compatibility and PlantUML

The temporary Rust adapter handles root-level raw blocks in a single forward
scan, avoids ordinary fenced code, uses safe fence lengths, then restores each
diagram's original literal in the AST. The original user source is not changed;
heading locations use original-source UTF-16 offsets. The adapter is deliberately
not claimed as a complete custom Markdown parser extension.

Of 19 browser comparisons:

- **19/19** native and WASM document JSON outputs are identical.
- **17/19** selected sanitized rendering signatures match current Marked, including
  the real review, tables/alignment, autolinks, task lists, raw/fenced diagrams,
  literal fences, incomplete input and the tested HTML.
- All 19 pass the unsafe-markup checks under the existing sanitizer.
- **16/19** outline comparisons match, including all 865 actual-review headings,
  IDs, hierarchy, line numbers and offsets.

Remaining issues before adoption:

1. Raw PlantUML inside quotes/lists is not adapted yet; those two rendering tests
   remain ordinary paragraphs rather than diagrams. Other Markdown-container and
   HTML-block contexts also need systematic tests.
2. Indented headings point to the `#` rather than Marked's raw-token start.
3. Comrak decodes heading entities, changing the current label/slug for
   `# A &amp; B`; stable IDs/annotation behavior require an explicit policy.
4. CRLF source positions differ from the existing Marked outline fallback bug,
   which can map a later heading back to line 1. The prototype reports the actual
   line; migration must not silently break navigation or preserve a known error
   without deciding how it should be corrected.

The focused fixtures are not exhaustive CommonMark/GFM conformance or an
annotation/selection regression suite. PlantUML SVG layout, external-include
restrictions and Mermaid/D2 rendering are not replaced by Comrak.

## Next step

Prefer a small, optional Comrak WASM adapter returning **HTML + outline metadata**,
with the same Rust core reusable natively later. Complete nested raw-PlantUML and
source/ID compatibility first, retain the existing sanitizer/postprocessor, and
test Safari on the user's actual machine before enabling it by default.

The verified warm CPU benefit makes this a reasonable low-API-change transition,
not an ultimate large-document solution: first-use cost, retained WASM memory and
main-thread/DOM work are real constraints. Do not make the production switch as
part of this evaluation.

## Production trial

The production adapter is separate from the benchmark prototype:
`crates/markdown-wasm` returns HTML plus bounded heading metadata. A module Worker
performs parsing and outline construction, and the existing UI-thread sanitizer,
diagram/image postprocessor and document controller mount the result. Workers
terminate on completion, cancellation or a 30-second timeout, so a large retained
WASM linear heap is not shared across subsequent documents. New opens cancel old
work rather than queueing behind a slow parser.

Heading labels/IDs use the existing short-heading tokenizer, with original-source
UTF-16 line starts supplied by Rust. Entity labels and indented headings retain
their previous IDs/offset convention; CRLF line mapping now uses the actual line
rather than preserving the older fallback bug. Source view and line navigation
reuse these cached outlines instead of re-lexing the entire file on the UI thread.
Files containing `@startuml` explicitly use the existing Marked implementation
inside the Worker, preserving nested/raw diagram handling that the Comrak adapter
does not yet provide. Ordinary Markdown has no runtime failure fallback: a failed
WASM load or parse is reported as an error rather than switching to Marked.
There is no main-thread parsing fallback. This replaces the initial trial's
automatic load-failure fallback; repository-level rollback remains available.

The 1,859,199-byte `check.review.md` fixture opened in about **3.4 seconds** in an
isolated native WebKitGTK application fixture, including Worker startup, local
fixture transport, sanitization/postprocessing, DOM mount and initial outline
layout. The same integration took about 1 second in a Chromium sample; table,
cell text/alignment, heading, link and fenced-code signatures matched the current
Marked result, including all 1,855 tables. Neither figure includes real backend
file I/O or SSH transport, and neither is a repeated-run median.

This is not fully jank-free: the WebKit fixture still observed about a 1.4-second
maximum main-thread heartbeat gap during postprocessing/layout. The parser is now
off-thread and the severe Marked/JSC tokenization stall is avoided for ordinary
Markdown, but DOM work remains a separate bottleneck. The 5 MiB text-review limit
is unchanged; no claim is made that a 31 MiB document is supported.

Regression coverage includes WASM binding/source hashes, Rust heading offsets,
GFM and diagram compatibility, unchanged sanitization, cancellation/late results,
worker timeout/error reporting, Source switching and oversized-file errors. See the
development guide for rebuilding assets and the optional browser integration
runner. A full macOS Safari/Windows WebView2 acceptance pass remains outstanding.

## Reproduce

See [`benchmarks/markdown-wasm/README.md`](../benchmarks/markdown-wasm/README.md).
Raw timings, cold probes, memory readings, asset hashes and all compatibility
results are in `target/markdown-wasm-results/results.json`; source snapshots there
may be private. Rust tests (3), existing frontend tests (710), syntax checks and
formatting checks passed for the original evaluation. Those results describe the
pre-integration Marked build; the production trial above supersedes that rollout
status, not the historical benchmark samples.

Primary references: [Comrak](https://github.com/kivikakk/comrak) and the
[wasm-bindgen browser deployment guide](https://wasm-bindgen.github.io/wasm-bindgen/examples/without-a-bundler.html).
