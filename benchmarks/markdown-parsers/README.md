# Markdown parser evaluation

This is an isolated developer benchmark, not an application dependency or a
production parser replacement. See [the evaluation report](../../docs/markdown-parser-benchmark.md).

The initial library comparison retains the old always-on PlantUML extension as
its baseline. To compare that baseline with the subsequent production routing
fix, run `node scripts/benchmark-plantuml-routing.mjs` (generated 5 MiB prose), or
append Markdown file paths. That standalone check needs neither native binaries
nor Playwright and verifies byte-identical HTML in both routed rendering paths.

From the repository root, with Rust and Node available:

```sh
export CARGO_TARGET_DIR="$PWD/target/markdown-parser-bench"
cargo build --manifest-path benchmarks/markdown-parsers/Cargo.toml --release --locked
mkdir -p "$CARGO_TARGET_DIR/bin"
cp "$CARGO_TARGET_DIR/release/studio-markdown-parser-bench" "$CARGO_TARGET_DIR/bin/default"
cargo build --manifest-path benchmarks/markdown-parsers/Cargo.toml --release --locked --features pulldown-simd
cp "$CARGO_TARGET_DIR/release/studio-markdown-parser-bench" "$CARGO_TARGET_DIR/bin/simd"

node scripts/benchmark-markdown-parsers.mjs --file /absolute/path/to/large.md
```

`--file` is optional. `--samples 15`, `--rounds 3`, and
`--output target/markdown-parser-results` are the defaults. The input file is
snapshotted before measurement; every engine reads that same snapshot. This
Linux runner uses `/usr/bin/time` for whole-process peak RSS. Do not run other
CPU-intensive tasks concurrently. Set `CARGO_HOME` to a disposable directory if
an isolated dependency cache is desired.

The second step checks semantic compatibility after the application's existing
DOMPurify policy and measures Marked inside Chromium. It needs an independently
installed Playwright and browser, not a new app dependency:

```sh
STUDIO_PLAYWRIGHT_MODULE=/absolute/path/to/playwright/index.mjs \
STUDIO_CHROMIUM_PATH=/absolute/path/to/chrome \
node scripts/benchmark-markdown-parser-browser.mjs target/markdown-parser-results
```

When normal Playwright/browser discovery works, omit the environment overrides.
Output files are `results.json` (all samples, first calls, peak RSS, input hashes),
`compatibility.json` (raw parser output), and `browser.json` (sanitized semantic
signatures and browser samples). These are in ignored `target/`, along with
potentially private source snapshots; do not publish them without reviewing their
contents. Raw parser HTML is deliberately unsanitized and must not be displayed
as trusted HTML.

The configuration matches `gfm: true, breaks: true` where supported. Marked uses
Studio's PlantUML extension; `marked-core` is a diagnostic without that extension.
Both Rust candidates enable tables, strikethrough, tasks and hard line breaks;
Comrak also enables bare autolinks. Raw HTML is enabled because sanitization is a
separate mandatory stage. Comrak's CLI and syntax-highlighting default features
are disabled. The explicit pulldown SIMD feature concerns HTML escaping; the
base build is not a guarantee that all dependencies avoid SIMD.

Timing includes Markdown parsing and HTML generation, not source reads, process
startup, sanitization, outline extraction, encoding/IPC, DOM insertion, or layout.
Each process records its first parse separately, warms up five times, then records
each parse. Engines rotate order across fresh-process rounds. GC and allocator
retention are not manually controlled. Peak RSS includes the runtime and is not
parser-only allocated memory. Smaller Rust measurements do not directly predict
the frontend speedup or the application's total memory savings.

The mixed corpus intentionally contains extension and unsafe-HTML fixtures, not
only ordinary prose. Synthetic 5 MiB inputs round up to a complete repeated block
and slightly exceed Studio's current 5 MiB review limit; they test parser scaling,
not supported UI file sizes. Compatibility signatures test selected semantics,
not exhaustive CommonMark/GFM conformance or pixel-identical rendering.

```sh
cargo test --manifest-path benchmarks/markdown-parsers/Cargo.toml --locked
cargo fmt --manifest-path benchmarks/markdown-parsers/Cargo.toml --check
node --check scripts/benchmark-markdown-parsers.mjs
node --check scripts/benchmark-markdown-parser-browser.mjs
```
