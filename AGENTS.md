# Project guidance

## Performance work

- Prefer the Rust backend for computation that does not require browser APIs. Evaluate backend placement first for parsing, indexing, search, transformations, and shared caches; use bounded CPU parallelism and SIMD-capable libraries where measurements justify them.
- Keep browser-dependent DOM construction, layout measurement, painting, selection, and interaction in the frontend, and minimize the work required there.
- Evaluate reusable, high-performance libraries and general optimization mechanisms before writing project-specific replacements. This includes Markdown and diagram engines as well as frontend DOM and rendering techniques.
- Measure end-to-end latency, main-thread responsiveness, memory, and transport/serialization costs. Preserve rendering compatibility, source anchors, and security boundaries.
- Follow the detailed [optimization principles](docs/performance-optimization.md#optimization-principles).
