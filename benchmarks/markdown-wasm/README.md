# Isolated Comrak WebAssembly evaluation

This prototype does not replace the production Marked parser or change backend
APIs, application assets, CSP, package dependencies, or release binaries.
Comrak 0.57.0 and wasm-bindgen 0.2.126 are pinned in a separate workspace/lockfile.

From the repository root, with Rust, Node and a matching `wasm-bindgen` CLI:

```sh
rustup target add wasm32-unknown-unknown
export CARGO_TARGET_DIR="$PWD/target/markdown-wasm-build"
cargo build --manifest-path benchmarks/markdown-wasm/Cargo.toml --release --locked --target wasm32-unknown-unknown --lib
wasm-bindgen --target web --out-dir target/markdown-wasm-pkg "$CARGO_TARGET_DIR/wasm32-unknown-unknown/release/studio_markdown_wasm_bench.wasm"
cargo build --manifest-path benchmarks/markdown-wasm/Cargo.toml --release --locked
cargo test --manifest-path benchmarks/markdown-wasm/Cargo.toml --locked

STUDIO_PLAYWRIGHT_MODULE=/absolute/path/to/playwright/index.mjs \
STUDIO_CHROMIUM_PATH=/absolute/path/to/chrome \
node benchmarks/markdown-wasm/run.mjs --file /absolute/path/to/large.md
```

Use a wasm-bindgen CLI matching the pinned crate version. It can be built with
`cargo install wasm-bindgen-cli --version 0.2.126 --locked`, or obtained from its
[official release](https://github.com/wasm-bindgen/wasm-bindgen/releases/tag/0.2.126).
The recorded run uses the official prebuilt Linux CLI. The optional Playwright
overrides are unnecessary when normal package/browser discovery works.

`--file` is optional; `--samples 7 --rounds 2` is the default. The runner starts a
loopback-only HTTP server, serves `application/wasm` under the application's actual
CSP, and measures the browser, then the same Rust core natively. It needs permission
to bind loopback and launch Chromium. It assumes a Linux host and the output paths
shown above. There are no external runtime fetches or CDNs.

After a complete run, `--checks-only`, `--pipeline-only`, or `--cold-only` can
refresh that section of the existing JSON without repeating the parser loops.
Use the same source file and binaries when refreshing a section.

Results and potentially private source snapshots are written to ignored
`target/markdown-wasm-results/`. The JSON records all timing samples, file hashes,
asset sizes, compatibility differences, fresh-browser initialization, timer lag,
CDP heap readings, WASM committed linear memory, and an actual-document render
pipeline comparison. Do not publish the snapshots without checking their contents.
See [the report](../../docs/markdown-wasm-benchmark.md) for conclusions and limits.

## Bindings compared

- `render_html`: UTF-16 JS input converted to UTF-8, parsed and rendered in Rust,
  then HTML copied back to JavaScript.
- `render_document_json`: one AST produces HTML plus top-level heading metadata;
  the whole result crosses as JSON. Browser timing includes JSON decoding and
  conversion to the existing outline shape.
- `parse_document`: the same AST returns an owned WASM handle. HTML is transferred
  separately, only heading metadata uses JSON, and `free()` runs in `finally`.
  This avoids encoding/decoding the full HTML as JSON. The handle does not expose
  the full AST to JavaScript.

Both native and WASM builds use release optimization, LTO and one codegen unit.
No explicit WASM SIMD, threads, worker, wasm-opt, or custom allocator is enabled.
The browser baseline is the **current source-aware Marked**, not the earlier
always-on PlantUML configuration.

## Deliberate prototype limits

The bare-PlantUML adapter scans root-level blocks once, tracks ordinary backtick
and tilde fences, and builds a temporary fenced parsing view without changing the
user's source. The AST code block's literal is restored from the original text.
Line count is preserved and headings map back to original UTF-16 line offsets.
Generated fence lengths account for tilde lines inside diagrams. Fenced diagrams
already use Comrak's standard code-block parser.

This is **not** a complete Markdown-context-aware PlantUML extension. Quoted/list
diagrams, all HTML-block interactions, all CommonMark fence edge cases, old
CRLF-navigation behavior and every annotation/selection invariant are not fully
adapted. The benchmark deliberately records mismatches rather than silently
claiming production compatibility. Unsafe parser HTML still goes through the
unchanged production DOMPurify policy before mounting.

Browser parser timings include JS/WASM input/output conversion and outline
adaptation but exclude file reads, loading and DOM work. The native HTML column
has no JS conversion; native document JSON is not the WASM split-handle API.
The pipeline check separately uses the production HTML sanitizer/postprocessor,
DOM mounting/layout and two animation frames, but does not run diagram layout,
backend fetches, caches, selection handling or the complete Studio shell.

Memory probes force GC outside timed regions. CDP JavaScript heap measurements
and WASM committed linear memory are different quantities, not total process RSS
or parser-only peak live allocations. Cold runs use fresh Chromium processes and
uncompressed localhost responses with `no-store`; they do not model SSH latency.
The separate `cachedCompileMs` metric runs after initialization and is explicitly
not an independent cold-compilation cost.
