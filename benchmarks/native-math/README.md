# Native math comparison

This isolated benchmark compares RaTeX 0.1.15 and LaTeX-Rust 2.1.1, with embedded
fonts, display style, nominal 24 CSS-pixel fonts (18 pt for LaTeX-Rust) and resvg
rasterization. LaTeX-Rust 3.0.x is
excluded because it requires Rust 1.99. This benchmark is not a production
dependency of either Studio frontend.

```sh
cargo build --manifest-path benchmarks/native-math/Cargo.toml --locked
python3 benchmarks/native-math/run.py benchmarks/native-math/target/debug/studio-native-math-bench
```

When `CARGO_TARGET_DIR` is set, use the executable under that directory instead.
Source/dependency caches can be located on a separate filesystem. Output defaults
to `target/native-math-benchmark`, including `report.json` and readable PNG/SVG
examples.

The same fourteen synthetic engineering/math examples and five invalid/resource
cases run separately in short-lived processes. Each process is limited to
512 MiB address space, three CPU seconds and a five-second parent timeout. The
driver records cold/warm formula-to-SVG time, whole process time, peak RSS, output
size, dimensions and non-background pixels. Twenty warm iterations follow the
first render. Cold timing includes font initialization; process time also includes
warm iterations and raster output. It is not application end-to-end latency.

Do not infer broad syntax coverage or visual equivalence from successful parsing.
Inspect the PNGs and retain errors. The original real large-report regression
contains no matching math syntax; it tests unrelated rendering responsiveness,
not formula coverage. Native GTK and process-boundary acceptance are separate.
