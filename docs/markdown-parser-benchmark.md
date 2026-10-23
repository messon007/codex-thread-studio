# Markdown parser evaluation — 2026-10-22

## Scope

Evaluate the existing Marked parser against native Rust libraries before deciding
whether a small parser-layer replacement is worthwhile. No application parser,
backend API, sanitizer, rendering mode, dependency, or release binary changes in
this evaluation. The benchmark crate has its own workspace and lockfile.

The original comparison measures the pre-fix, always-on PlantUML extension.
The later source-aware routing fix is reported separately below; the original
tables are preserved rather than relabeled as current production measurements.

Candidates are Marked **18.0.13**, pulldown-cmark **0.13.5**, and Comrak
**0.57.0**. pulldown-cmark is built both with and without its explicit `simd`
feature. Marked runs with Studio's current PlantUML extension; `Marked core`
disables only that extension to distinguish library cost from extension cost.

## Method

Linux x86-64, Intel Core i5-12500 (6 cores / 12 threads), Rust 1.97.0 release
builds, Node 24.5.0. Runs are single-threaded, without `target-cpu=native` or
application-level parallelism. Source reads and process startup are outside the
timed region. Each fresh process records its first parse, warms up five times,
then records 15 parse-plus-HTML operations. Three rounds rotate engine order:
45 steady-state samples per engine and input. GC is not disabled or forced.

The actual acceptance review is snapshotted once for every engine:

- File: `models/dcdc_unified/validation/acceptance/331_coverage.review.md`.
- Size: **563,588 bytes**, 10,564 newline characters.
- SHA-256: `322f494710af9bd16efb0e1c07918ddbdced8b39b4fbd2f6673485c93ff02019`.

Other inputs are the current 10,600-byte README, repeated mixed syntax fixtures,
and table/prose-heavy synthetic documents. Synthetic 5 MiB cases round up to a
complete block and slightly exceed the current review-file limit; they are parser
stress tests, not a claim that the UI accepts larger files.

All engines use hard line breaks, tables, tasks and strikethrough. Comrak enables
bare autolinks. Rust raw HTML is enabled to match Marked before the mandatory
sanitizer; these parser outputs are **not safe HTML on their own**. Comrak's CLI
and syntax-highlighting default features are disabled.

## Native versus Node results

Steady-state median, milliseconds; parsing plus HTML generation only:

| Input | Marked (Studio) | Marked core | pulldown | pulldown + SIMD | Comrak |
| --- | ---: | ---: | ---: | ---: | ---: |
| README, 10.4 KiB | 0.629 | 0.594 | 0.026 | 0.022 | 0.051 |
| Actual acceptance review, 550.4 KiB | 54.32 | 30.98 | 2.18 | 2.20 | 6.83 |
| Mixed syntax, 2.00 MiB | 280.98 | 262.83 | 23.51 | 23.12 | 65.28 |
| Table-heavy, 5.00 MiB | 577.79 | 539.59 | 61.93 | 69.95 | 172.19 |
| Prose-heavy, 5.00 MiB | 7,106.15 | 264.61 | 19.89 | 19.54 | 48.97 |

On the real document, pulldown's parser-plus-HTML operation is about **25 times**
faster than the current configured Marked, or **14 times** faster than Marked
without the custom extension. Comrak is about **8 times** and **4.5 times** faster,
respectively. These are parser microbenchmark ratios, **not file-open speedups**.

On that document the p95 values are 63.05 ms (Marked), 37.76 ms (Marked core),
2.39 ms (pulldown), 2.31 ms (pulldown + SIMD), and 7.78 ms (Comrak).

The shared development machine was not idle throughout the stress tests. The
table-heavy input in particular had a slower round: p95 was about 1,122 ms for
configured Marked and 264 ms for Comrak. Preserve the raw samples and repeat on
an otherwise idle host before treating small differences as meaningful.

### Memory

Linux whole-process peak RSS on the real review, maximum over three processes:

| Engine | Peak RSS |
| --- | ---: |
| Marked (Studio) | 277.8 MiB |
| Marked core | 280.5 MiB |
| pulldown | 8.1 MiB |
| pulldown + SIMD | 8.0 MiB |
| Comrak | 12.2 MiB |

These include the Node/Rust runtime, loaded code, source/output, allocator
retention and GC behavior. They are not parser-only allocation counts, not
incremental Studio memory, and not browser-renderer memory measurements. The
large Node RSS does not mean that amount is permanently retained by an open
document in Studio.

First-parse medians on that input, excluding startup/import/read time, are
83.11 ms (Marked), 51.43 ms (Marked core), 3.10 ms (pulldown), 3.17 ms
(pulldown + SIMD), and 9.39 ms (Comrak). Thus the native advantage is not solely
an artifact of comparing warm Rust code with a cold JavaScript parser.

The returned HTML also has a transport cost. On the real review, the byte counts
are 760,507 (Marked), 728,779 (pulldown), and 762,235 (Comrak). On the table-heavy
stress input they are 11,852,400, 13,442,016, and 11,908,176 respectively.
pulldown's current table output is larger there; faster parsing does not imply a
smaller network response. Neither JSON encoding nor SSH/HTTP transfer was timed.

### SIMD

The explicit pulldown feature does not consistently improve these workloads.
Real-document medians are essentially identical; the tiny README improves, while
the noisy table test does not. Do not select a library or promise a speedup based
on its SIMD label. This feature accelerates HTML escaping on x86-64; the base
build can already use optimized scanning through its dependencies. It is not a
measurement of browser Wasm SIMD or ARM performance.

### Project extension overhead

The prose stress case contains **no PlantUML**, yet enabling Studio's current
PlantUML extension changes Marked's median from 264.61 ms to 7,106.15 ms; p95
reaches 10,149.56 ms. The real acceptance review also has identical output bytes
with and without this extension, but changes from 30.98 ms to 54.32 ms.

The extension's `start(source)` in `ui-src/diagram-source.mts` performs a regular
expression search over the remaining source to find the next `@startuml` line.
Marked can request this search repeatedly as it advances through ordinary
paragraphs. Repeated scans of large remainders explain why this configuration
scales much worse on the prose fixture than the core parser. This is a
project-extension cost, not proof that Marked itself needs seven seconds.

The lowest-risk separate follow-up is to avoid this repeated search when the
complete document has no PlantUML marker, and preserve the custom syntax when it
does. This was not implemented during the initial library evaluation; the
subsequent fix is described below. This finding should not be hidden by
attributing the entire configured-Marked/native ratio to library quality.

### Follow-up: source-aware routing

The application now reuses two isolated Marked instances with identical
`async: false, breaks: true, gfm: true` options. Only one has the PlantUML
extension. Chat selects its instance once per uncached parse; document preview
selects once before lexing, then passes that instance and its tokens through HTML
generation and outline extraction. The global Marked instance no longer gets
the PlantUML extension installed on it.

The marker check deliberately looks for `@startuml` case-insensitively anywhere
in the source, rather than only at the beginning of a line. This preserves
quoted/list-indented diagrams whose prefixes Marked removes before invoking the
extension, and incomplete streamed diagrams without an `@enduml` yet. Fenced code
and inline mentions may conservatively use the extended path; the change does
not promise to remove repeated scanning from documents containing a marker.

The same SHA-256-identified snapshots were measured again on this machine using
`scripts/benchmark-plantuml-routing.mjs`. Median parse-plus-HTML milliseconds:

| Input | Old always-on extension | Routed chat | Routed document tokens + HTML |
| --- | ---: | ---: | ---: |
| Actual review, 550.4 KiB | 72.46 | 39.70 | 34.92 |
| Prose-heavy, 5.00 MiB | 8,744.81 | 303.64 | 261.20 |

The always-on path records three samples after two untimed parses, and each
routed path records 15 samples after one untimed parse. The content check is
included; file reads, HTML equality assertions, outline generation, sanitization,
DOM and transport are excluded. Every measured output is byte-identical to the
old parser output. These follow-up samples are not directly interchangeable with
the original three-round measurements, and host/JIT/GC variation remains.

Run `node scripts/benchmark-plantuml-routing.mjs` for the generated 5 MiB case, or
pass one or more Markdown paths. This needs no Rust candidates or Playwright.
The local raw samples are in `target/plantuml-routing-results.json`.

Validation: 710 frontend tests and TypeScript checking pass. An actual Chromium
145 component check exercised the application chat/document rendering functions
with 11 inputs, including nested/raw/fenced diagrams, incomplete streaming input,
unsafe HTML and the real review. Sanitized chat HTML, document HTML and outlines
match the old always-on implementation, with exactly one parser selection per
uncached render and all 865 real-document headings retained. The sanitizer, SVG
rendering, source text and navigation semantics are unchanged. No Comrak migration
is included in this fix.

## Browser cross-check

Chromium 145.0.7632.6 also measures the actual vendored Marked inside a fresh
page per corpus, separately from the compatibility DOM work. Both configurations
alternate order for 20 iterations; the first five are discarded. Median / p95
milliseconds for the remaining 15 samples:

| Input | Marked (Studio) | Marked core |
| --- | ---: | ---: |
| README | 0.60 / 1.40 | 0.60 / 1.10 |
| Actual acceptance review | 53.00 / 101.10 | 27.90 / 37.60 |
| Mixed syntax, 2 MiB | 279.50 / 353.30 | 287.80 / 352.80 |

These broadly corroborate the Node workload costs, with browser/GC/host variance.
They do not compare native Rust against a browser Wasm build, and they still do
not measure complete document opening, DOM mounting or layout latency.

## Rendering compatibility

Chromium checks 15 focused fixtures and the full README and acceptance review,
after the same DOMPurify policy used by Studio. The signatures compare visible
text, heading levels/text, links, images, table cells/alignment, checkboxes,
formatted text, lists, details and code languages/content. HTML-source whitespace
and attribute-order differences are not treated as semantic differences. These
checks use browser default styles, not a screenshot comparison with Studio CSS.

Both native candidates match the selected signatures on the real review
(865 headings, 10,176 table cells) and README. Both preserve fenced Mermaid,
PlantUML and D2 language markers, hard line breaks, task lists, raw HTML through
the sanitizer, and the tested CJK/emoji/CRLF content. All tested unsafe markup is
removed by the unchanged sanitizer.

The focused fixture differences are:

| Behavior | pulldown | Comrak |
| --- | --- | --- |
| Bare URL / email autolinks | Not linked by this parser configuration | Matches Marked |
| GFM table alignment | Emits inline `style`, stripped by the existing policy | Emits `align`, matches Marked |
| Standalone `@startuml` block | Ordinary paragraph instead of a diagram code block | Same limitation |
| Other 12 focused fixtures | Matches selected signatures | Matches selected signatures |

Comrak therefore matches 14/15 focused fixtures; pulldown matches 12/15. The
standalone PlantUML mismatch is expected: it is a **Studio extension**, not a
feature of either native library. It needs an adapter even though fenced
PlantUML already works. Bare-link adaptation for pulldown must not accidentally
link text inside existing links or code spans. Table alignment needs controlled
renderer output, not a relaxation of the sanitizer's blanket style prohibition.

Both libraries provide useful source locations: pulldown has event byte ranges;
Comrak has AST line/column positions. The native test confirms heading locations
after CJK/emoji text and CRLF. These are not automatically JavaScript UTF-16
offsets. Existing outline IDs, source/preview navigation and annotation anchors
still need an explicit compatibility adapter; the benchmark does not assert that
switching the HTML parser alone preserves all application anchors.

## Recommendation

**For the smallest compatible native-parser migration, prefer Comrak first.**
It is already substantially faster than both current Marked configurations,
matches more of the existing GFM behavior without extra adapters, and provides
an AST with source positions. It still needs the Studio PlantUML extension and
outline/anchor integration, and a real end-to-end prototype before adoption.

**For maximum parser throughput and lower peak process RSS, pulldown is the
winner.** Keep it as the performance-oriented alternative if the additional
autolink/alignment adaptation is justified. On the actual review its advantage
over Comrak is only about 4.6 ms per parse, whereas on the table stress case it
saves about 110 ms. This distinction matters more than the generic SIMD label.

The first targeted extension-search mitigation is now implemented as the
source-aware routing follow-up above; marker-containing documents retain their
existing path. Do not claim native replacement alone resolves all large
document latency: HTML sanitization, DOM construction/layout, serialization and
transport remain outside these measurements. No virtualization, chunk API,
render-mode redesign, or new production cache is proposed in this evaluation.

The tested Comrak release requires Rust 1.89 and pulldown requires Rust 1.71.1;
the benchmark itself uses edition 2024. Local Rust 1.97 builds both and this
project's workflows select stable Rust. Windows, macOS, WebKit and native WebViews
were not benchmarked here.

## Reproduction and artifacts

A separate [browser WASM evaluation](markdown-wasm-benchmark.md) compares Comrak
against the source-aware Marked follow-up, including boundary conversion, preview
rendering, startup and memory. Native timings here should not be relabeled as
WASM results; the two evaluations have different scopes and baselines.

Build/run instructions and interpretation caveats are in
[`benchmarks/markdown-parsers/README.md`](../benchmarks/markdown-parsers/README.md).
The runner writes all individual samples, first-call measurements, input hashes
and RSS readings to `target/markdown-parser-results/results.json`. Compatibility
HTML and browser semantic signatures are adjacent. The ignored input snapshots
can contain private document content; they are deliberately not checked in.

The native fixture tests cover basic configured GFM rendering plus UTF-8/CRLF
source locations. The browser check applies the existing DOMPurify policy before
comparing selected document semantics and asserts that unsafe markup is removed.
This is not a complete CommonMark/GFM conformance suite or a replacement for
application integration tests.

Primary library references:
[pulldown-cmark](https://github.com/pulldown-cmark/pulldown-cmark),
[Comrak](https://github.com/kivikakk/comrak), and
[Marked](https://github.com/markedjs/marked).
