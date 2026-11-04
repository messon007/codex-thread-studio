# Native Mermaid renderer evaluation

This records the earlier Merman 0.9.3 versus Pele 1.9.0 evaluation. Its isolated
workspaces and retained Pele baseline remain reproducible. The native GTK queue
now uses the separately packaged [Merman worker](../../native/mermaid-worker/README.md),
with shared style/layout controls and prepared-font/image caches. The worker needs
Rust 1.95; the parent remains Rust 1.92. Frozen earlier packages are unchanged.

## Reproduce

Run from the repository root, with Rust 1.95 or newer and the normal native build
dependencies. The second executable includes the retained Pele baseline modules,
including their native math-worker entrypoint; it does not start GTK or a backend.

```bash
export CARGO_TARGET_DIR=/path/on/build-disk/studio-mermaid-eval
export CARGO_HOME=/path/on/build-disk/cargo-home
cargo build --release --locked --manifest-path benchmarks/native-mermaid/Cargo.toml
cargo build --release --locked --manifest-path native/gnome/Cargo.toml --example mermaid_eval
python3 benchmarks/native-mermaid/run.py \
  --merman "$CARGO_TARGET_DIR/release/studio-native-mermaid-eval" \
  --pele "$CARGO_TARGET_DIR/release/examples/mermaid_eval" \
  --output /path/on/build-disk/new-private-results \
  --reference /absolute/path/to/real-document.md
```

Repeat `--reference` for additional read-only documents. The directory must be new.
It contains actual diagram source and rendered document content: keep it private,
outside the repository. Input document hashes are verified again after the run.
Python is an evaluation dependency, not an application runtime requirement.
Optional inspection sheets require Pillow:

```bash
python3 benchmarks/native-mermaid/gallery.py /path/on/build-disk/new-private-results
```

Sheets alpha-composite the measured transparent PNGs over white/dark backgrounds
and scale them to fit; original PNGs and Merman SVGs are retained unchanged.

## Method and limits

Every engine/case uses a fresh process, sequentially, with three renders. Engine
construction is inside each render's timer for both paths. The first render is
cold; the later two warm immutable/global font caches. Timing includes parsing,
layout, SVG creation/parsing and native rasterization; Merman also reports its
generation/raster phases. PNG encoding is outside render timers. Whole-process
time includes all three renders, startup and writing results. This is worker
cost, **not GTK end-to-end latency or a large-Markdown opening benchmark**.

The driver enforces 2 GiB address space, 12 CPU seconds, 18 seconds wall time,
8 MiB output per file and 128 descriptors. A wall timeout kills the entire owned
process session, including math children. Each input is limited to 128 KiB; SVG
to 4 MiB; raster to 2048 × 4096, at most 8 megapixels. Both paths use the same
resvg/font-family/preview scaling policy. SVG image resolvers deny data/file/network
images; external nodes/images are outside this corpus. System fonts are readable.
These evaluation limits do not prove production cancellation or resource parity.

Merman's `resvg_safe` pipeline lowers HTML labels to native SVG: there is no DOM,
browser or Node. Its compiled math renderer must be explicitly enabled, with
semantic HTML labels enabled before lowering. Disabling `htmlLabels` leaves
literal TeX even when rendering succeeds. The evaluator explicitly selects
Dagre for default flowcharts, because Merman otherwise defaults to ELK; per-source
configuration overrides this. `requiredCapabilities` records actual ELK/Math
admission, and XML checks record native math geometry, literal formulas and any
remaining `foreignObject`. Parse success is not a compatibility assertion.

Peak RSS comes from GNU time `%M`: the maximum of a process/its waited children,
not a simultaneous sum of their memory. Different layouts produce different
pixel dimensions and raster costs. Fixed engine order, three iterations, one
machine and sixteen small family examples do not establish universal speed or
full Mermaid syntax/visual parity. Executable hashes, host, input hashes and
harness hash are recorded in private `results.json`.

## Initial 42-case baseline (2026-11-03)

Release builds on Ubuntu 26.04.1, x86_64, Intel Core i5-12500, Rust 1.97; no
concurrent compilation during measurements. The final corpus contains 32 generated
cases and ten diagrams extracted from three real project documents. Both engines
render all 41 valid inputs and reject the malformed input. The production probe
also passes a release-profile Rust 1.92 compile check.

Median across the ten real diagrams (per-case warm value is the median of two
renders):

| Measurement | Merman 0.9.3 | Production Pele 1.9.0 |
|---|---:|---:|
| First render, including raster | 23.9 ms | 80.5 ms |
| Warm render, including raster | 11.3 ms | 68.1 ms |
| Whole process, all three renders | 60.7 ms | 229.6 ms |
| Peak RSS | 31.6 MiB | 28.5 MiB |

All Merman outputs contain zero `foreignObject` elements. Classic, Neo, Document
and dark palettes differ visibly; hand-drawn changes actual paths (23 versus 11
in the profile fixture). The same crossing graph reports `LayoutElk` only for ELK
and changes geometry from 884 × 768 to 848 × 800. An 80-node graph takes 62.7 ms
warm with Dagre and 52.9 ms with ELK, versus 105.2/99.2 ms in Pele, which uses its
own layout for both requests. The hand-drawn fixture takes 56.5 ms, so the real
diagram median must not be presented as a speedup for every style.

Visual inspection confirms Chinese/mixed flowchart labels and flowchart/message formula
geometry. **Merman sequence participant formulas remain literal `$$x^2$$`** in
both actor boxes, although the message fraction is typeset. The production Pele
path typesets both. Default measurement also clips labels in the class, ER and
mind-map fixtures; the native-font follow-up below investigates that path.
These prevent a universal renderer replacement. CJK labels also wrap differently;
image/font/complex-diagram coverage needs broader testing.

The final frozen `native-mermaid-validation/default` and `native-fonts` directories
are authoritative, with copied executables/harness/lockfile. Earlier exploratory
directories retain diagnostic runs where math was disabled, engine lifetime
differed, or the standalone Pele probe lacked its math-worker dispatch; their math
timings and pictures must not be used as application results.

## Native-font qualification

Merman's upstream `NativeFontContext` fixes the observed class/ER/mind-map clipping
with explicit DejaVu Sans regular/bold, DejaVu Sans Mono and Noto Sans CJK files.
A same-family control still clips with deterministic measurement, so this is an
actual measurement change. Render evidence reports selected faces and Host
measurement routes, with no fallback in the valid corpus. Strict glyph coverage
is enabled. All 41 valid inputs render; the malformed input is rejected; sequence
participant math remains literal.

To reproduce, append these flags to the evaluation command, with a new output
directory and paths appropriate for the machine:

```bash
--native-fonts \
--font /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf \
--font /usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf \
--font /usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf \
--font /usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc
```

Without explicit files, `--native-fonts` tests whole-system discovery. On this host
it hits upstream's 16 MiB system-cmap metadata limit. That error is retained; the
limits were not increased. Explicit acquisition avoids unrelated system metadata
and keeps the upstream 64 MiB font-byte budget. Font hashes are in the private
results. The JSON evaluator also accepts `fontFamily` for equal-family controls.

The native-font profile is the relevant candidate for further integration, with
these medians across the ten real diagrams:

| Measurement | Merman with native measurement | Production Pele |
|---|---:|---:|
| First render, including font acquisition/raster | 52.8 ms | 81.8 ms |
| Warm render, including raster | 20.4 ms | 68.0 ms |
| Whole process, all three renders | 106.4 ms | 233.3 ms |
| Peak RSS | 53.7 MiB | 29.5 MiB |

The 80-node Dagre case is approximately tied at 103.3/103.4 ms warm; ELK is
88.9/103.5 ms. This is not a universal performance win. The native measurement
module currently depends on usvg 0.47 while this benchmark paints with the
production resvg/usvg 0.48.1. The explicit families are available to both, but
shared font acquisition/residency, fallback selection across scripts and engine
versions still need qualification. Do not advertise the faster heuristic numbers
as the cost of a production-ready replacement.

The forced-deadline fixture also verifies that both the owned fake renderer and
its child exit after the driver's session timeout. The production probe passes
Rust 1.92 release checking. All 856 frozen application build inputs still match
the integrated candidate; this research only adds an example/isolated workspace
and documentation. No application rebuild or package promotion is needed for it.
Private evidence on the evaluation host is under
`/media/rui/studio-gnome-deps/qa-artifacts/native-smoke-jwfgtzto/native-mermaid-validation/VALIDATED.json`.

## Actor math and self-contained vectors

The corpus now has 51 cases: the original cases plus single/multiline actors,
mixed prose/formulas, dark actors, mixed/multiline flowcharts, note/loop math and
an intentionally unsupported macro. A narrow, opt-in XML adapter in
`native/mermaid-worker/src/math_switch.rs` (shared by this evaluator) addresses the
earlier participant-formula loss. Merman emits
both a formula HTML branch and literal SVG fallback text in a `switch`; its
native pipeline prefers the literal fallback. The adapter preserves a generated
math branch for actor labels before upstream conversion. It handles each line's
separate fallback, leaves ordinary/unsupported branches intact, and keeps the
original native SVG finalizer/resource checks.

The adapter uses roxmltree with DTD/external entities disabled, 4 MiB input and
131,072-node ceilings. Subtree facts are computed in a linear pass; operation
cancellation is checked during traversal and rewriting. Four focused tests cover
namespace/conditional/ordinary/unsupported preservation, input/node/DTD/cancel
rejection, actual single/multiline Merman actor/message geometry, and final
file/network structural-resource rejection after promotion. Run with:

```bash
cargo test --release --locked --manifest-path benchmarks/native-mermaid/Cargo.toml
```

Appending `--prefer-math-switches --outline-text` to the native-font evaluation
enables the current prototype. The latter normalizes SVG text into paths using
the native measurement context's own usvg 0.47 font resolver. The display-side
resvg/usvg 0.48.1 then uses an **empty font database**. This removes font
substitution at that handoff without importing the new engine into GTK's Rust
1.92 dependency graph. The pre-outline SVG is saved separately: formula/text
checks use that artifact, because outlining alone would conceal literal TeX.
All generated SVG remains subject to the 4 MiB limit; image resolvers grant no
ambient access. A shared three-second cooperative deadline now covers each
Merman operation, with the driver's independent process-session timeout retained.
The JSON-only `rawSvg` diagnostic selects upstream parity output for inspecting
the conversion stage; it cannot be combined with outlining.

Both engines render the 50 syntactically valid cases and reject the malformed
case. Before outlining, only the originally broken sequence formula changes in
the original corpus; the ten real documents retain identical layout SVG and
dimensions. Single/multiline actor formulas, human actors, flowchart formulas,
note/loop formulas and mixed-label formula geometry survive; only the unsupported
macro retains literal TeX. Every outlined output has zero text/foreignObject
elements and the largest SVG is 644,907 bytes. This proves the bounded vector
handoff, **not visual compatibility**.

Screenshot inspection of that unpatched run exposes two failures: mixed actor prose uses
the actor background paint, making “Input”/“Output” disappear, and default RaTeX
paths stay black on the dark theme. Their geometry/text-presence checks pass.
These are explicit failed compatibility cases; do not enable this prototype as
the application renderer until label paint, contrast and authored colors are
preserved. The pre-outline XML and measured PNGs retain the failures.

Across the ten real diagrams, outlined worker medians are 42.5 ms first render,
20.2 ms warm, 96.1 ms for all three renders and 50.9 MiB peak RSS. The paired
production probe is 81.2 ms, 67.2 ms, 231.3 ms and 28.6 MiB. The 80-node cases
regress: Dagre takes 123.7/100.8 ms warm and ELK 106.2/99.2 ms (Merman/Pele).
Outlining itself takes about 15–16 ms there. No universal speedup or application
latency improvement is established.

Private evidence is under
`/media/rui/studio-gnome-deps/qa-artifacts/native-smoke-jwfgtzto/native-mermaid-vector-validation/VALIDATED.json`.

## Scoped foreground patch

The independent `paint` workspace evaluates a downstream patch against the exact
published `merman-render` 0.9.3 crate. It shares the evaluator and actor-branch
adapter above. The patch passes the resolved label foreground into RaTeX layout,
where explicit TeX colors retain precedence. It does not recolor completed paths.
Sequence actor prose receives the same foreground explicitly; actor, message,
note and loop defaults remain independently configurable. Flowchart node classes
and ordinary edge-label styles participate through upstream's compiled style
declarations and color parser. Invalid flowchart color overrides fall back to
theme ink without dropping formula geometry. The class-title callback also
receives its resolved theme color; this does not add class-member math support.

The added trait hooks delegate to the existing methods by default, preserving
custom renderer implementations. Existing measurement uses the original path;
paint selection does not change formula metrics. The application and baseline
evaluation dependency graphs remain unchanged.

Use canonical patched source identities when rebuilding this independent workspace:

```bash
python3 scripts/cargo-gnome-mermaid.py --workspace benchmarks/native-mermaid/paint \
  --component merman-render build --release --locked
python3 scripts/cargo-gnome-mermaid.py --workspace benchmarks/native-mermaid/paint \
  --component merman-render test --release --locked
```

This wrapper delegates to `scripts/prepare-gnome-mermaid.py`; the canonical patch
is under `native/mermaid-worker/patches`. The helper requires Python 3.12 or newer
and `patch`. It verifies the crate
archive SHA-256
`3cf1f994f9fe91cd7f5e0fbb4f7c9982585813c79aacad50d9351a23ee4d1520`,
applies the committed patch with zero fuzz in a bounded staging directory, and
fingerprints the result before atomically creating the ignored `paint/vendor`
source link. The source cache defaults under `CARGO_TARGET_DIR`; `--cache` selects
another build disk. `--offline` uses the existing verified Cargo archive and
`--no-link` prepares/verifies without replacing the workspace link. Existing
source directories and modified cache contents are preserved and rejected.
Do not replace a source link while Cargo is using it. The shared registry is
never patched. The published crate records upstream commit
`3b915f69c02d55b873d252cf6c8a6048e7f3427d` (`crates/merman-render`).
The Cargo wrapper additionally supplies the fingerprinted directory as the
package source, avoiding stale dep-info after a stable vendor link changes.
`prepare-paint.py` remains available for source preparation alone. The original
measured artifacts below retain their original patch hash; the current patch
also supplies the intrinsic HTML code face evaluated by the native worker.

For the paired corpus, use `studio-native-mermaid-paint-eval` as `--merman`, append
`--paint-cases --native-fonts --prefer-math-switches --outline-text`, and supply
the four explicit font files above. This includes 60 cases with the same ten real
diagrams, plus independent label-role colors, authored TeX black/red, alpha,
node/edge styles, dark/Neo/hand-drawn math and a class-member diagnostic. Inspect
the pre-outline XML as well as PNGs. Black explicitly authored inside TeX remains
black even on dark backgrounds. A hex `linkStyle` color immediately followed by
`;` is treated as an entity by the pinned upstream preprocessing; use a newline
after hex or a named CSS color. The tests cover newline-terminated hex, named
colors, `!important`, invalid-color fallback and authored formula precedence.

The final 60-case run renders all 59 valid inputs in both engines and rejects
the malformed input. Screenshot inspection confirms restored mixed actor prose,
visible default dark-theme math, separate label-role colors, authored red/black,
alpha, node/edge colors and Neo/hand-drawn math. All 38 non-math layouts shared
with the earlier 51-case run are byte-identical. Every outlined output contains
zero text/foreignObject elements; the largest SVG remains 644,907 bytes. The
focused suites pass 15 executions across 11 unique tests (the four adapter tests
also run in the integration test binary). A clean preparation fixture verifies
all 435 compiler input files against the measured source, cache reuse, atomic
link creation, preservation of existing source directories and tamper rejection.

Only the intentionally unsupported macro and the class-member diagnostic retain
literal formulas. Both engines leave that class member literal; Mermaid's
[documented math families](https://mermaid.js.org/config/math.html) are flowcharts
and sequence diagrams. This does not establish general class-member math support
or full Mermaid compatibility.

Final paired medians across the ten real diagrams, with no concurrent compile:

| Measurement | Patched, outlined Merman | Production Pele |
|---|---:|---:|
| First render, including font acquisition/raster | 42.8 ms | 82.3 ms |
| Warm render, including raster | 20.3 ms | 67.0 ms |
| Whole process, all three renders | 96.3 ms | 230.3 ms |
| Peak RSS | 50.9 MiB | 29.0 MiB |

The 80-node fixtures still regress: warm Dagre is 120.1/103.9 ms and ELK
104.8/99.9 ms (Merman/Pele). The added font handling consumes more memory.
These results qualify the scoped fixes and worker experiment, not application
opening latency, a universal speedup or production readiness. The measured
executable is SHA-256
`8d7be295faa4b333eb4566535f1db951c4780a29d6739f94fc42ba41695102ec`;
the patch is
`63015b6fe7ec5498c0ab05591a386f9fa104e938c40a5d45e04b5f5edfbeb4ec`.
Private measured SVGs, PNGs, harness, source-preparation evidence and exact binary
are retained under
`/media/rui/studio-gnome-deps/qa-artifacts/native-smoke-jwfgtzto/mermaid-scoped-paint-20261103/VALIDATED.json`.

## Integration decision

The next stage now has an independent [native worker](../../native/mermaid-worker/README.md)
and a Rust 1.92 parent transport. The packaged component replays all 59 valid
diagrams through Bubblewrap, rejects the malformed case and has no text/image/
foreignObject dependencies. Namespace, strict admission, real OS limits, output
flooding, timeout and cancellation checks pass; both observed four-process
sandboxes leave no survivors. Of the 59 outputs, 54 match this experiment byte
for byte. Global Dagre versus this experiment's flowchart-only Dagre explains
the remaining class/state/ER/mind-map outputs; controlled Auto requests reproduce
all five prior outputs. The worker's default style/layout mapping follows the
existing web preferences, including a hand-drawn seed only for HandDrawn.
The component includes 204 dependency entries and their exact notices/provenance.
This earlier component checkpoint preceded application/installer adoption.

The subsequent [automatic-font evaluation](../../native/mermaid-worker/README.md#automatic-font-evaluation-2026-11-03)
adds a version-2 semantic font preflight, bounded Fontconfig selection, shared
measurement/export role aliases and a content-fingerprinted metadata cache. The
original corpus and copied-font invalidation controls pass. A pinned usvg
source-cluster patch now fixes the mixed-script cases without changing any of
the 59 prior SVGs. Complete HTML code labels also honor the code-family choice.
Source-ranged projection now preserves mixed body/code fonts and colors, with
633 default-feature renderer tests and unchanged corpus SVGs. Static inline font
families now also participate in wrapped layout: thirteen font profiles and
five pixel-identical inline/class controls pass. Unsupported geometry remains
guarded, including dynamic families and formula/replaced-object overrides;
these worker checks do not establish application integration. Canonical Cargo
source IDs and package source checks prevent stale reuse after vendor-link
changes. The parent's two direct font-parser declarations reuse
versions already present in its lockfile.

The original adoption gates were production font acquisition,
source configuration, image isolation, cancellation/deadlines, weighted
caches and GTK refresh/selection behavior. Those paths now use the separately
packaged worker and shared appearance controls; real Wayland reader/chat tests
pass. Normal build/install scripts include the worker/notices and validate its
protocol and source hashes. The GTK parent still compiles on Rust 1.92. Follow
current acceptance in the worker guide; the frozen evaluation results above
remain measurements of their original binaries.

Merman's facade is MIT OR Apache-2.0; its native ELK component is EPL-2.0 and math
includes font attribution requirements. Preserve exact-version component notices
when packaging. This evaluation alone does not establish desktop/provider, fresh-machine or
cross-platform migration acceptance.

Primary references: [Merman source](https://github.com/Latias94/merman),
[Pele compatibility](https://pele.run/compatibility), and the downloaded,
lockfile-pinned 0.9.3 crate source. Upstream main targets a newer release and is
not a substitute for validating the pinned API and its declared MSRV/licenses.
