# Project guidance

## Interaction and interface design

- Before designing or changing UI, read [the interaction and interface guidelines](docs/design/interaction-guidelines.md) and [the typography standard](docs/typography.md). These record project-wide user preferences, not just Agent Teams conventions.
- Design the user journey before controls. Preserve the central chat and reuse the shared right workspace, existing components, icons, typography and interaction patterns. Prefer compact, aligned tables for comparable objects; make common actions direct and keep low-frequency actions in established menus.
- Preserve drafts, reading position, focus and shared workspace width across relevant transitions. Accurate status and responsive interaction are part of UI correctness.
- For substantial new or redesigned interactions, present a representative design/prototype for user review before implementation unless already approved in the current task. Validate the rendered result and actual journeys; source checks alone cannot establish visual quality.
- Keep durable design decisions in the guidelines and scoped feature documents. Update superseded decisions rather than accumulating contradictory rules; do not generalize a feature-specific removal into a project-wide ban.

## Performance work

- Prefer the Rust backend for computation that does not require browser APIs. Evaluate backend placement first for parsing, indexing, search, transformations, and shared caches; use bounded CPU parallelism and SIMD-capable libraries where measurements justify them.
- Keep browser-dependent DOM construction, layout measurement, painting, selection, and interaction in the frontend, and minimize the work required there.
- Evaluate reusable, high-performance libraries and general optimization mechanisms before writing project-specific replacements. This includes Markdown and diagram engines as well as frontend DOM and rendering techniques.
- Measure end-to-end latency, main-thread responsiveness, memory, and transport/serialization costs. Preserve rendering compatibility, source anchors, and security boundaries.
- Follow the detailed [optimization principles](docs/performance-optimization.md#optimization-principles).

## Shared document maintenance

- MarkNoria is the primary maintenance source for shared Web desktop document/reader capabilities; Studio owns AI/workspace integration and host authorization. Track the Web/browser/WebView reader and its actual Rust dependencies only; exclude platform-native desktop UI/FFI and the parallel Studio native branch. Follow [the maintenance and sync plan](docs/marknoria-maintenance.md).
- Studio explicitly excludes MarkNoria’s standalone side-by-side document comparison (Compare), as requested by the user. Do not treat it as a parity gap or request an upstream embedding API for Studio; existing Git Review/diff remains in scope.
- Reuse verified, fixed-version upstream library/component deliveries. Do not infer compatibility from matching feature names or a closed issue; preserve source mappings, resource budgets, platform support and independent builds.
- Use the union of both Web readers' generic capabilities: upstream Studio-only reading features, follow upstream improvements, and converge shared behavior/defaults with common fixtures. Do not waive reader differences merely as product differences. Keep AI and host-specific behavior in adapters. A necessary urgent generic fix may land locally with tests, an upstream issue and a recorded patch-removal condition; return it upstream and remove the temporary patch when adopted.
- Ordinary builds and public PR CI must not depend on private MarkNoria credentials or a sibling checkout's sources, build assets, caches or runtime. Do not change another session's development worktree to synchronize shared code.
