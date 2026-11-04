# Project guidance

## Performance work

- Prefer the Rust backend for computation that does not require browser APIs. Evaluate backend placement first for parsing, indexing, search, transformations, and shared caches; use bounded CPU parallelism and SIMD-capable libraries where measurements justify them.
- Keep browser-dependent DOM construction, layout measurement, painting, selection, and interaction in the frontend, and minimize the work required there.
- Evaluate reusable, high-performance libraries and general optimization mechanisms before writing project-specific replacements. This includes Markdown and diagram engines as well as frontend DOM and rendering techniques.
- Measure end-to-end latency, main-thread responsiveness, memory, and transport/serialization costs. Preserve rendering compatibility, source anchors, and security boundaries.
- Follow the detailed [optimization principles](docs/performance-optimization.md#optimization-principles).

## Shared document maintenance

- MarkNoria is the primary maintenance source for shared document/reader capabilities; Studio owns AI/workspace integration and host authorization. Follow [the maintenance and sync plan](docs/marknoria-maintenance.md).
- Reuse verified, fixed-version upstream library/component deliveries. Do not infer compatibility from matching feature names or a closed issue; preserve source mappings, resource budgets, platform support and independent builds.
- Keep Studio-specific behavior in adapters. A necessary urgent generic fix may land locally with tests, an upstream issue and a recorded patch-removal condition; return it upstream and remove the temporary patch when adopted.
- Ordinary builds and public PR CI must not depend on private MarkNoria credentials or a sibling checkout's sources, build assets, caches or runtime. Do not change another session's development worktree to synchronize shared code.
