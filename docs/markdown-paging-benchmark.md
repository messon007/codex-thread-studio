# Large Markdown paging evaluation

Measured on 2026-10-22. This evaluates the requested **read-only** experience:
return the **complete TOC and first few pages together**, then serve a selected
TOC target with **preceding and following context pages**. Production code and
the existing 5 MiB review limit are unchanged.

## Conclusion

Rust-side Comrak parsing plus indexed, bounded HTML windows is promising.
Generating a complete directory is not the main obstacle on these inputs.
However, simply changing the parser or using 64 KiB pages does **not** guarantee
a responsive large-document viewer:

- A 30 MiB generated report returned its full 23,960-entry TOC and first three
  pages in a median **694 ms** on the backend.
- A wide-table >=30 MiB stress case consumed **1,858 MiB peak RSS** during full
  AST processing. Moving this work to Rust does not make it memory-free.
- Its five-page table window still produced **50,718 DOM elements** at the
  64 KiB / 200-row setting. Native WebKitGTK stalled for over a second when the
  whole window was mounted synchronously.
- Reducing the source budget to 16 KiB and the table-chunk limit to 16 rows,
  and yielding between page fragments, reduced the tested wide-table jump to
  **134 ms Chromium / 171 ms WebKitGTK**, with maximum timer gaps of **37 / 65 ms**.

Recommendation: keep the existing small-document path; evaluate a separate
backend-powered, read-only large-document path with full TOC metadata, bounded
context windows, complexity-aware fragment limits, and cancellable incremental
DOM updates. Do not merely raise the current file cap or mount all returned
fragments in one synchronous task.

## Method and scope

Host: Intel Core i5-12500, Linux, 31 GiB RAM, Rust 1.97.0, Node 24.5.0,
Comrak 0.57.0, Chromium 145.0.7632.6 and native WebKitGTK 2.52. The machine was
shared and memory-constrained; swap was already occupied. Treat timings as local
measurements, not a service-level guarantee. Three fresh native processes per
configuration; tables below use medians unless noted. OS page caches are warm;
these are not cold-storage measurements.

The previously observed ~31 MiB `tests.review.md` was regenerated before this
evaluation and now has 4,097,226 bytes. It was not silently treated as the old
file. Inputs were snapshotted and hashed before measurement:

| Input | UTF-8 bytes | TOC entries | AST nodes | Largest table body |
| --- | ---: | ---: | ---: | ---: |
| Current real `tests.review.md` | 4,097,226 | 9 | 1,156,416 | 9,970 rows |
| Real automatic test report | 12,173,906 | 16,102 | 891,037 | 126 rows |
| Generated report, >=30 MiB | 31,458,205 | 23,960 | 2,914,831 | 12 rows |
| Repeated real wide-table input, >=30 MiB | 32,777,808 | 72 | 9,251,321 | 9,970 rows |

The automatic report is
`DCDC-1297547-CHARGE-SEEN+DCDC-1297547-NO-CHARGE-LATCH.tests.md`. The generated
report includes Chinese, emoji, formatted table cells and globally defined
reference links. The wide-table stress file repeats the current real input
eight times; it is **synthetic**, not a captured old version of the document.

SHA-256, in table order:

```text
a820051d1803a9f8900aee1082dd469cf739da0df6bfc8819c9c9f951785391a
a03e20b7d3ff7da09cf2ae21b42287e72ed6d83a7f513ae425891b25a12ab747
a356d07289c230ec89267fc54645cefbbcd33be4f165b41d9d6c2d17fa3108ef
c0d77e12889f9e828ca8f0e164edf8ef3bc2537517c1d754a268e038371558d4
```

### Prototype response contract

The initial request returns `readOnly: true`, full `toc`, page metadata and
pages 0–2. A jump returns target ±2 pages, clipped at the start/end, plus
continuation pointers. The full TOC is not retransmitted on ordinary jumps.
These are logical pages budgeted by source bytes and table rows, not fixed
screenfuls or one page per section. A long section or table can span many pages.

The prototype parses once with Comrak, retains original source positions and
global reference resolution, and formats AST subtrees instead of reparsing
arbitrary Markdown substrings. Long top-level tables are split on body-row
boundaries using the unchanged Comrak formatter, repeating the header. The
complete TOC has no 2,000-heading truncation.

### Backend results

64 KiB source target, 200-row table-chunk cap:

| Input | TOC ready | TOC + first 3 pages ready | Native peak RSS | Additional full HTML cache build |
| --- | ---: | ---: | ---: | ---: |
| Real 3.91 MiB | 193 ms | 194 ms | 239 MiB | 110 ms |
| Real 11.61 MiB | 329 ms | 335 ms | 321 MiB | 99 ms |
| Generated 30.00 MiB report | 685 ms | 694 ms | 664 MiB | 250 ms |
| Repeated 31.26 MiB wide tables | 2,428 ms | 2,431 ms | 1,858 MiB | 1,245 ms |

Initial readiness includes reading the snapshot, parsing, indexing, generating
selected HTML and serializing the JSON response. It excludes process startup,
HTTP/IPC delivery and browser work. First artifact writes occur after that
measurement. Cache generation is additional work after initial readiness; it
is sequential in the benchmark, not a tested background scheduler. Across all
three source-budget settings, the wide-table first-ready samples ranged from
1.45 to 2.60 seconds. Most work is parsing; those differences do not establish
that larger page budgets cause slower parsing.

The initial generated-report response is **3,732,844 bytes**, including a
**3,264,686-byte TOC** and 442,936 bytes of HTML before JSON escaping. Complete
TOC delivery is feasible locally, but metadata size and remote bandwidth must
be considered. The browser must not create 23,960 directory DOM entries at once.

After the full HTML cache exists, a fresh process can read a buffered JSON index
and seek directly to requested page ranges:

| Input | Cached index + initial response | Subsequent cached-window read + JSON | Cache-reader peak RSS |
| --- | ---: | ---: | ---: |
| Real 3.91 MiB | 1.7 ms | 1.5 ms | 8.8 MiB |
| Real 11.61 MiB | 11.9 ms | 1.4 ms | 12.3 MiB |
| Generated report | 14.5 ms | 0.6 ms | 15.6 MiB |
| Repeated wide tables | 1.6 ms | 2.5 ms | 10.2 MiB |

These are **already-valid-cache component costs**, not complete reopen times:
source revision validation, authorization, process startup and transport are
not included. Large indices must be read with buffering or from an in-memory
byte slice, not an unbuffered streaming JSON reader. No AST is built by the
cache-reader process. Exiting the parser process releases its full arena;
merely dropping an AST in a long-running process need not return allocator RSS
immediately, and concurrent full parses would multiply the peak memory risk.

Uncompressed HTML caches are substantial: about **108 MiB** for the generated
report and **237 MiB** for repeated wide tables. The smaller table fragments
increase the latter to about **256 MiB** because headers are repeated. Disk
quota, eviction and invalidation are requirements, not optional cleanup.

### Browser results

Fixtures use the app's actual `renderMarkdownHtml` sanitizer/postprocessor,
production CSS and CSP, and loopback HTTP JSON responses. They retain the full
TOC plus current window but mount at most 50 directory items. Each configuration
has three rounds per mode/window. Ready time includes fetch/JSON decode,
sanitization, insertion, layout and two animation frames. Timer gaps use a
10 ms heartbeat; they are a responsiveness proxy, not formal input-latency or
frame-time measurements. Modules/CSS are initialized before measurement.

Representative median ready time / maximum heartbeat gap:

| Case | Chromium | Native WebKitGTK |
| --- | ---: | ---: |
| Report initial 3 pages, 64 KiB, yield between pages | 167 / 62 ms | 160 / 44 ms |
| Report middle TOC jump, 64 KiB, yield between pages | 199 / 41 ms | 230 / 47 ms |
| Wide-table jump, 64 KiB / 200 rows, synchronous window | 908 / 892 ms | 1,337 / 1,233 ms |
| Wide-table jump, 64 KiB / 200 rows, yield between pages | 988 / 318 ms | 999 / 332 ms |
| Wide-table jump, 16 KiB / 16 rows, synchronous window | 165 / 164 ms | 180 / 170 ms |
| Wide-table jump, 16 KiB / 16 rows, yield between pages | 134 / 37 ms | 171 / 65 ms |

The wide-table jump window drops from **50,718** to **7,661** DOM elements
and from **2,542,856** to **379,922** HTML bytes. The smaller-window initial
response contains **59,675 bytes**, including the full 72-entry TOC and three
pages, rather than returning only the directory. Its backend initial-ready
time was 1,534 ms in one additional native run; that single run is not evidence
of a parsing speedup over the three-round configurations.

32 KiB source pages alone were insufficient: the wide-table window still had
24,679 elements and a 350 ms median maximum timer gap with page-level yielding
in Chromium. **Source bytes are not an adequate proxy for rendered complexity.**
The measured fragment settings are useful starting points, not universal limits
for arbitrary numbers of columns or huge individual cells.

After GC at the final retained window, Chromium reported about 3.47 MiB V8 heap
and 5.63 MiB embedder heap for the report; wide tables used about 1.33 / 29.73 MiB,
or 1.33 / 7.60 MiB with smaller fragments. These are CDP component readings,
not peak browser-process RSS or complete application memory. WebKit process
memory was not measured. The full app session/chat workload was not mounted.

## Correctness checks and remaining work

Seven native tests cover context clipping/continuation, cached/live response
equality, table row coverage/alignment/header preservation and AST restoration,
global reference definitions, >2,000 headings, Chinese/emoji UTF-16 offsets,
CRLF, Setext/nested headings, fenced fake headings, and atomic code blocks.
Browser checks cover TOC target/page mapping, original heading line positions,
read-only metadata, repeated table headers, and an unsafe-HTML fixture passing
through the unchanged sanitizer. Both browser engines passed.

Not proven or implemented:

- Cross-page raw HTML containers, globally normalized HTML fragment IDs,
  nested oversized blocks, and huge indivisible paragraphs/code/cells need
  explicit policies. Atomic non-table blocks currently remain oversized.
- Bare `@startuml` compatibility and diagram rendering costs are not evaluated
  here. Existing PlantUML behavior must be preserved separately; fenced diagrams
  would need lazy rendering and cancellation in a production paged viewer.
- Real scroll virtualization, full directory browsing/search, page LRU,
  cross-window selection/copy, document-wide search, and source navigation are
  not implemented by this fixture. No Edit UI or writes are provided.
- The prototype's table formatter temporarily moves rows between AST nodes;
  requests for one document must be serialized, not rendered concurrently.
- Production needs bounded jobs/memory, cancellation on close/switch, stale
  request rejection, workspace access checks, revision-bound caches, atomic
  cache writes and quotas. Sensitive raw HTML caches are not trusted output.
- There is no measured full-app end-to-end 30 MiB path yet. The Rust and browser
  component times must not be presented as a single integrated measurement.

The next implementation step should be the bounded read-only backend contract
and viewer together, rather than integrating only the TOC or increasing limits.
Reproduction commands and private-artifact precautions are in
[`benchmarks/markdown-paging/README.md`](../benchmarks/markdown-paging/README.md).
