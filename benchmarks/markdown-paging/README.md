# Large Markdown paging evaluation

Isolated evaluation only; no production viewer, endpoint, file-size limit, or
release binary changes. See [the measured report](../../docs/markdown-paging-benchmark.md).

## Native measurements

```sh
export CARGO_TARGET_DIR="$PWD/target/markdown-paging-build"
cargo test --manifest-path benchmarks/markdown-paging/Cargo.toml --locked
cargo build --manifest-path benchmarks/markdown-paging/Cargo.toml --release --locked
node benchmarks/markdown-paging/run.mjs target/markdown-paging-results \
  /absolute/path/to/table-heavy.md /absolute/path/to/report.md
```

Set `CARGO_HOME` to an existing dependency cache if necessary. `ROUNDS=3` is the
default. The runner snapshots real inputs before measuring and generates a
30 MiB report; if a real input is provided it also generates a >=30 MiB stress
case by repeating the first input. It tests 32/64/128 KiB source budgets and a
200-row table-chunk limit, in serial fresh processes. Timings and hashes are in
`target/markdown-paging-results/results.json`. Only round-zero payloads/caches
are retained; later-round timing records remain in the results file.

To reproduce the smaller table budget separately:

```sh
target/markdown-paging-build/release/studio-markdown-paging-bench \
  target/markdown-paging-results/long-tables-30mib.md \
  target/markdown-paging-results/long-tables-30mib-16384-16 16384 16
target/markdown-paging-build/release/studio-markdown-paging-bench \
  cache target/markdown-paging-results/long-tables-30mib-16384-16
```

Native mode first builds the global AST, full TOC and page index, then serializes
the first three pages with the complete TOC. Later responses contain the target
page plus two pages either side, clipped at the document ends. Those are logical
block/row pages, not browser screenfuls. Table chunks repeat their original
header and retain row alignment. Global reference links are resolved before
paging. After initial delivery, the benchmark builds all HTML pages on disk;
`cache` reads the index and requested HTML ranges without parsing again.

## Browser measurements

An independently installed Playwright and Chromium are needed; no app dependency
is added. Optional environment overrides:

```sh
export STUDIO_PLAYWRIGHT_MODULE=/absolute/path/to/playwright/index.mjs
export STUDIO_CHROMIUM_PATH=/absolute/path/to/chrome
node benchmarks/markdown-paging/browser-run.mjs \
  target/markdown-paging-results/report-30mib-65536-0
node benchmarks/markdown-paging/browser-run.mjs \
  target/markdown-paging-results/long-tables-30mib-16384-16
```

Append `--webkit` for offscreen native WebKitGTK, requiring system Python GI,
GTK 3, WebKit2 4.1 and a working display. The fixture uses a loopback-only server,
the app's actual renderer/sanitizer function, CSS and CSP. It compares mounting
the whole response synchronously with yielding between page fragments. Each
mode has three rounds over initial, middle-TOC, long-table and final windows.
Results are `chromium.json` / `webkit.json` in the payload directory.

The full TOC is retained as data but only a 50-item slice is mounted. This is a
component benchmark, not a completed virtual TOC UI or a full app integration.
Original heading positions, page mapping, read-only responses, table headers
and unsafe HTML removal are asserted. Diagram generation is not exercised.

## Safety and limitations

Generated source snapshots and unsanitized cached HTML can contain private data.
Keep everything under ignored `target/`; do not publish those files as trusted
HTML. Cache freshness, workspace permissions, atomic writes, quotas, eviction,
request validation, cancellation and concurrent request scheduling are not
implemented. Table rendering temporarily moves AST rows and must be serialized
per document. Oversized non-table blocks are deliberately not split and are
reported. This is not a general-purpose production paging library.

```sh
cargo fmt --manifest-path benchmarks/markdown-paging/Cargo.toml --check
node --check benchmarks/markdown-paging/run.mjs
node --check benchmarks/markdown-paging/browser-run.mjs
node --check benchmarks/markdown-paging/browser.mjs
```
