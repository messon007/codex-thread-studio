# Native Mermaid worker

This independent Rust 1.95 workspace supplies the native GNOME application's
Mermaid renderer. Normal GTK diagram requests use its prepared-font cache and
process boundary; normal build, package and install scripts include the verified
worker and notices. The GTK parent remains on Rust 1.92; other platform dependency
graphs are unchanged. The parent-side transport is
`native/gnome/src/mermaid_worker.rs` and its standalone probe is the
`mermaid_worker_eval` example. The old Pele code remains for the earlier standalone
evaluation/eager preparation path, not the GTK diagram queue.

The worker uses pinned Merman 0.9.3, its native font measurement and RaTeX math,
plus the evaluated scoped foreground patch, shared font-role API, source-cluster
font fallback patch and actor-switch adapter. It outlines
all text with the same font context that measured it. Output has no text,
foreignObject or image nodes, so the parent rasterizer needs no replacement font
selection. There is no browser, Node, JS runtime or Python runtime in this worker.
Python is used for build preparation, packaging and acceptance only.

## Build a component

Use Rust 1.95 or newer, Python 3.12+, `patch` and normal Rust build tools. For
execution/testing on Ubuntu 26.04 install `bubblewrap`, `fontconfig`, DejaVu and Noto CJK fonts.
Keep `CARGO_HOME`, `CARGO_TARGET_DIR` and optionally `TMPDIR` on the build disk.

```bash
bash scripts/build-gnome-mermaid.sh /absolute/path/to/new-component
cargo build --release --locked --manifest-path native/gnome/Cargo.toml \
  --example mermaid_worker_eval
python3 scripts/smoke-gnome-mermaid-worker.py \
  /absolute/path/to/new-component/codex-thread-studio-mermaid \
  "$CARGO_TARGET_DIR/release/examples/mermaid_worker_eval" \
  --auto-fonts --output /absolute/path/to/new-private-results
```

Repeat acceptance with `--corpus /private/path/to/paired-mermaid-results` for the
actual 60-case corpus. The output must be new and may contain private diagram
content. `CARGO_NET_OFFLINE=true` builds from existing verified caches. Running
`python3 scripts/prepare-gnome-mermaid.py --offline` separately prepares the
ignored vendor link; `--workspace benchmarks/native-mermaid/paint` also serves the
independent evaluator. Never replace a source link during a Cargo build.

Use the Cargo wrapper for this worker's builds, tests and metadata:

```sh
python3 scripts/cargo-gnome-mermaid.py test --release --locked --offline
```

It prepares verified sources and supplies canonical fingerprinted paths through
Cargo's [configuration patch table](https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html).
Retargeting a stable vendor symlink alone can leave Cargo dep-info pointing at
the previous source tree; the wrapper makes a changed patch directory a changed
Cargo package identity. Unchanged dependencies remain reusable. Package assembly
also checks the actual metadata-resolved source fingerprints and records their
Cargo IDs. This avoids clearing the shared build cache.

The component contains the executable, dependency versions/checksums/notices,
the exact downstream patches and original crate fingerprints. EPL-2.0 source
provenance and both RaTeX software MIT and embedded font OFL notices are retained.
Published RaTeX subcrates that omit their common workspace license use the
already pinned 0.1.15 upstream notices from `native/gnome/vendor/ratex`. An existing
component output is preserved; the build requires a new output directory.

Run the pinned fallback unit regressions from a disposable source copy (Cargo
cannot run a dependency's dev-tests from the worker workspace):

```sh
python3 scripts/test-gnome-mermaid-clusters.py --output /absolute/path/to/new-cluster-tests
python3 scripts/smoke-gnome-mermaid-fonts.py \
  /absolute/path/to/new-component/codex-thread-studio-mermaid \
  "$CARGO_TARGET_DIR/release/examples/mermaid_worker_eval" \
  --output /absolute/path/to/new-mixed-script-tests
```

The second check uses actual installed fonts and the sandboxed parent. Its
optional `--baseline OLD_WORKER` also reproduces the original shaping failure.

## Process contract

One UTF-8 JSON request arrives on stdin, terminated by EOF. Required fields are
`version: 3`, `source` and `dark`. Optional `style` contains `style`, `layout`,
`fontFamily`, `fontFamilyMono`, `fontSize`, `density` and `curve`; unknown fields
are rejected. Presentation values match the existing web preferences. Auto layout
omits a global override; a requested Dagre/ELK applies globally, whereas the older
paired experiment forced Dagre only inside flowchart defaults. Source directives
retain upstream precedence. A deterministic hand-drawn seed is set only for the
HandDrawn preference. The optional parent-owned `fontDefaults` pins actual
admitted reading/code family names for both measurement and export. Unknown
fields and invalid/absent admitted family names are rejected.

SVG output starts with the fixed worker-owned header
`<svg data-studio-math-labels="N" `. The bounded count comes from actual generated
native math SVG nodes before outlining, including repeated sequence actors;
unsupported literal formulas do not increase it. The parent validates this
protocol header and retains the count alongside the vector. It does not infer
rendering success from source delimiters. The new metadata is the only change to
all 59 preceding corpus SVGs; removing that header attribute restores their exact
geometry/paint bytes. Version-2 components are rejected before packaging.

Font files are parent-owned `--font FILE` arguments, never diagram-controlled
paths. The parent canonicalizes and bounds the selection, mounts each file at an
ordinal sandbox path, then sends JSON. No system font discovery happens in the
worker. `--font-plan` returns bounded JSON instead of SVG: Merman's controlled
semantic parser supplies source/config/class font requests and label characters.
HTML labels use html5ever tokenization without a DOM. Comments and HTML
attributes do not introduce painted glyph requests.

The Rust parent resolves these family stacks with the system Fontconfig matcher,
including regular/bold/italic faces and needed script coverage, with bounded
process output/deadlines and escaped literal family names. Font bytes are read
with independent limits and checked with the existing `ttf-parser`. Content
hashes and explicit role names form the font fingerprint. A 32-entry, 2 MiB
weighted metadata cache reuses selections for ten seconds, rechecking file
identity/timestamps. Prepared fonts are checked again before and after rendering.
No font bytes remain in that cache. See the
[Fontconfig pattern/matching contract](https://fontconfig.pages.freedesktop.org/fontconfig/fontconfig-user.html).

Success writes only self-contained SVG to stdout. Failure writes bounded
stderr and exits nonzero. Limits are 128 KiB source, 800 KiB serialized input,
sixteen requested family stacks, twenty-four font files / 64 MiB, 8,192 model
items, 131,072 SVG elements, 4 MiB output
and 32,768 units per dimension. A cooperative three-second operation deadline
covers font loading, generation and outlining. The parent allows four seconds
and owns cancellation/output enforcement. Linux limits also impose 1 GiB address
space, 512 MiB data, four CPU seconds, 64 descriptors, no child processes,
no regular-file output or core dumps, and no privilege escalation.

The parent uses Bubblewrap with private PID/user/network namespaces, empty
writable temporary storage, no home/profile/workspace mounts, selected read-only
fonts and the worker plus system loader libraries. Environment variables are
cleared. The existing process transport owns bounded pipes, cancellation and
reaping; destroying the sandbox also ends descendants. The single native diagram lane owns admission and the weighted image cache.

## Current integration and limits

GTK now selects this worker through its prepared resource/cache adapter. Normal
build/package/install and GNOME CI include the verified component; the shared
appearance dialog exposes style/layout controls. The current qualified candidate
and repeatable checks are described in [GTK adoption](#gtk-adoption-2026-11-03).
The retained dated checkpoints below describe their state when recorded.

Unsupported macros remain readable literals; class-member math, dynamic family
combinations and unsupported HTML geometry retain explicit limits. The measured
80-node case is slower than Pele; this change does not claim universal speedups.
Full desktop/provider/fresh-machine/cross-platform acceptance remains open.

## Initial version-1 component checkpoint (2026-11-03)

On Ubuntu 26.04.1 x86_64, the optimized component replays all 59 valid diagrams
in the 60-case corpus through the actual parent sandbox and rejects the malformed
case. Ten diagrams come from real project documents. All output is bounded and
self-contained. Fifty-four SVGs are byte-identical to the paired experiment;
the five class/state/ER/mind-map differences follow explicit global Dagre.
Controlled Auto requests reproduce all five earlier outputs exactly, and the
actual Auto/Dagre comparison sheet is inspected.

The acceptance probe verifies denial of a private host file, home directory,
environment secret and connection to a listening host loopback port. It observes
the actual worker's Linux limits, strict protocol/source rejection and output
flood handling. Timeout and cancellation each observe a four-process sandbox;
all observed descendants disappear. The failure paths of the test harness also
clean up only its owned processes. No host-desktop input is used.

The worker tests pass six cases; the shared patched evaluator passes 15 test
executions and the baseline evaluator passes four. The final parent probe passes
optimized Rust 1.92 checking; the worker is built using Rust 1.97. The production
bootstrap passes fresh extraction, path-with-spaces, reuse, source-link,
local-edit/tamper preservation and corrupt/oversized-download checks. All 204
component dependency entries have retained notices, including original ELK port
derivation references and software/font license distinctions.

Private evidence is
`/media/rui/studio-gnome-deps/qa-artifacts/native-smoke-jwfgtzto/native-mermaid-worker-20261103-qualified/QUALIFIED.json`.
The component is `native-mermaid-worker-component-20261103-ready` beside it,
with worker SHA-256
`619d59828ed64445352a68f9a728068cb0a45866c22f43715c30636cad5b6be5`.
At that checkpoint all 856 source inputs matched the existing integrated app
candidate. Its installed package and renderer remain unchanged. This proves a component
boundary, not GTK integration or full migration completion.

## Automatic-font evaluation (2026-11-03)

The version-2 worker replays all 59 valid inputs in the original 60-case corpus
with automatic fonts and rejects the malformed input. Fifty-four SVGs remain
byte-identical to the explicit-font paired experiment, including all ten real
diagrams. Namespace, actual worker limits, flooding, deadline and cancellation
cleanup still pass. A private copied-font fixture verifies warm reuse, invalidation
after font bytes change, rejection of an old prepared selection, and restoration
of the original content identity when the bytes are restored.

Expanded profiles verify reading serif fonts, source directives, class fonts,
literal Fontconfig metacharacters, comment-only unsupported characters, and
separate code-family aliases on monospace class labels even when body text is
also monospaced. The explicit body/code alias control changes the actual SVG.
The measured cold font-selection calls in these synthetic profiles are about
146–213 ms; same-source warm selections take 52–100 microseconds. This is only
font preparation, not GTK latency, a throughput benchmark or a speedup claim.

The first automatic-font evaluation retained three compatibility failures:

- One label combining Greek, Cyrillic, Arabic and Chinese fails strict shaping.
  Diagnostic glyphs show unresolved Chinese characters despite admitted CJK
  fonts. usvg 0.47's fallback loop stops when whole-string reshaping changes glyph
  count (33 versus 34 in the captured probe); the same assumption exists in 0.48.1. The isolated diagnostic preserves
  the failed text/probe and does not modify the pinned source cache.
- Merman's native measurement rejects inline HTML `font-family` geometry styles.
- Inline HTML `<code>` keeps the body font: changing the code preference leaves
  its SVG identical, unlike the positive monospace-class control.

The later inline-family qualification below addresses the static text-family case.
Other unsupported geometry remains guarded. Do not switch to heuristic
measurement or suppress glyph checks to make the failures appear successful.
An isolated source-cluster replacement prototype rendered that exact mixed
label under strict checks, preserves all 59 existing corpus SVGs byte for byte,
and still passes sandbox cleanup. The inspected raster contains the expected
scripts. The pinned worker now carries the qualified patch described below;
the original diagnostic artifacts remain unchanged.
The worker's eight tests, the parent probes' Rust 1.92 release checks, and all
291 native library tests pass (two explicit tests remain ignored). The current
source adds parent font dependencies/shared byte transport; it has not replaced
the renderer or rebuilt/promoted the installed native application.

Private evidence is under
`/media/rui/studio-gnome-deps/qa-artifacts/native-smoke-jwfgtzto/native-mermaid-auto-fonts-20261103/FONT-EVALUATION.json`,
with expanded images and failures in `native-mermaid-font-profiles-20261103-final`.

## Pinned cluster fallback qualification (2026-11-03)

The worker now patches usvg 0.47.0 at its shared shaping fallback loop. Complete
resolved clusters are substituted by their exact UTF-8 source ranges instead of
glyph array position/count. Existing valid clusters, their fonts and visual bidi
order are retained. A partially missing fallback cluster or one spanning a
different source range is left unresolved, preserving strict glyph checks.
This is a conservative fallback correction, not a replacement text shaper or
a claim of coverage for every script/ligature boundary.

Six dependency tests cover unrelated ligature count changes, combining marks,
one-to-many substitutions, RTL order, incomplete fallback clusters and overlapping
source ranges. Six actual-font diagrams combining Latin ligatures, Greek,
Cyrillic, Arabic, Hebrew, marks and Chinese now render; all six fail with the
previous unpatched worker. Their raster gallery was inspected. The 59 original
valid corpus SVGs are byte-identical to the preceding automatic-font worker,
and malformed input, resource limits, isolation, flooding and process cleanup
still pass. The eight worker tests pass. No dependency versions changed.

Source preparation supports each component's own version and pins the usvg
archive checksum, zero-fuzz patch, source-tree fingerprint and ignored vendor
link. The component includes all three patches and original crate records,
204 dependency notices and its exact binary hash. Fresh preparation, reuse,
path-with-spaces, preservation and tamper/download rejection checks pass.

Private evidence is `native-mermaid-cluster-corpus-20261103/CLUSTER-EVALUATION.json`
under the same QA root above, with component, font-case, unit-test and preparation
artifacts alongside. The GTK renderer, its separate usvg 0.48.1 dependency and
installed application are unchanged. Inline HTML geometry, cache/UI
integration and normal package/install/CI adoption remain.

## Intrinsic code face and source identity (2026-11-03)

HTML `<code>` now receives its intrinsic monospace face in the SVG fallback,
matching the existing label measurement. The admitted author cascade takes
precedence, including explicit `inherit`/`initial`, selector rules and important
declarations. Descendants inherit the resolved code face. Two public-renderer
tests exercise these cases through the worker's existing dependency graph;
the eight worker unit tests also pass.

Actual-font controls now produce different outlined SVGs when the code-family
preference changes for a complete `<code>` label, including when the body uses
a different monospace family. Source/class aliases and the explicit role-pin
control still pass. All 59 prior corpus SVGs remain byte-identical, and the
malformed-input, resource limits, namespace and process cleanup checks pass.

The earlier expanded 13-profile evaluation retained two failures: inline
HTML `font-family` was rejected before layout, and a label mixing body text with
`<code>` lost its per-run font during fallback projection. That checkpoint
selected common-ancestor typography for the whole label. The qualification below
now corrects per-run projection; the native geometry guard remains until its
measurement path supports the requested inline styles.

The regression also caught stale Cargo reuse after a vendor symlink changed.
Canonical patch paths now select the actual source directory, and package
provenance is checked against Cargo's resolved sources. Existing failed probes
are retained alongside corrected-build checks. Private evidence is under
`native-mermaid-code-font-corpus-20261103/CODE-FONT-EVALUATION.json` in the QA root.
This component remains independent of the installed GTK renderer.

## Per-run font projection qualification (2026-11-03)

The pinned renderer now keeps computed fonts/colors for each text owner, sharing
font records across sibling leaves and creating them only for ancestors that
actually own text. Entity decoding and CSS whitespace normalization retain owner
ranges. The existing line planner supplies exact normalized UTF-8 ranges and
the selected discretionary-hyphen marker. Measurement and SVG `tspan` projection
use those ranges, preserving repeated words, wrapped code, bold, italic and text
colors. Complete uniform labels retain their previous projection path, including
styled leaves whose entire contents collapse away. Generated spans pass through
the existing byte/element/depth limits and cancellation checks.

The repeated-word/entity regression fails against the preceding worker and passes
with this patch. The worker passes eight unit and three public font tests. The
renderer passes all 633 default-feature library tests, including 14 line-planner
and 98 fallback tests. One upstream state-viewBox fixture is absent from the
published crate; the exact file was restored from upstream commit
`3b915f69c02d55b873d252cf6c8a6048e7f3427d` with SHA-256
`4e0ee9998ecf613720070cdfef42703c5d55f458413e4c2550d6f459a8f9e340`.
An owned workspace layout and isolated test target directory ensure the fixture
test compiles with its actual manifest path; no tests are skipped and no shared
build cache is cleared. This is not an all-features upstream test-suite claim.

All 59 corpus SVGs remain byte-identical to the previous worker, and malformed
input, resource limits, namespace denial, flooding, timeout and cancellation
cleanup pass. All 12 renderable cases in the 13-profile font evaluation now pass
their controls, including distinct code-family output inside mixed body/code
labels. Additional actual-font images show serif prose, bold, italic, color and
code together, plus Arabic/Greek/CJK/code, without missing glyphs. The raster
gallery was inspected.

At that checkpoint the single retained profile failure was inline HTML
`font-family`. The qualification below now fixes its supported text-layout route;
the earlier failed artifacts remain unchanged. The GTK renderer, installation and
native Rust 1.92 graph are unchanged; cache/UI integration, normal packaging/CI
and the full application acceptance gates remain.

Private evidence is under
`native-mermaid-rich-font-corpus-20261103/RICH-FONT-EVALUATION.json` in the same QA
root, with the exact fixture provenance and original failed probes retained.

## Inline family measurement qualification (2026-11-03)

Static inline `font-family` lists and explicit `inherit`/`unset` now participate
in wrapped label measurement. Resolved terminal family records travel through
natural width, min-content, style grouping and wrap probes. Equivalent resolved
runs coalesce before measurement, preserving non-additive shaping across tags.
Arbitrary families use the existing observable measurement route rather than
the intrinsic eight-face streaming table. Labels without family changes retain
their previous measurement path. Shared CSS keyword parsing preserves comments,
escaped identifiers, priority and inheritance in measurement and SVG projection.

Admission now accepts that measured text-only contract. Font-size, font-weight,
font-style and other unsupported geometry declarations retain their guards.
Dynamic families and family overrides combined with formulas, SVG/image objects
or Font Awesome icons remain rejected; their geometry adapters do not yet share
the text-run contract. No heuristic fallback is introduced for those cases.

The selector now includes regular, bold, italic and bold-italic variants for
additional requested families as well as reading/code roles. The observed
profiles need up to twenty font files, so the file-argument cap is twenty-four;
the aggregate 64 MiB font budget, sixteen family-stack cap and process limits
remain unchanged. Tests accept 24 file arguments, reject 25, and reject an
over-budget sparse file before parsing without allocating its logical size.
The parent still passes Rust 1.92 release checks.

Eight worker unit tests, three projection tests, five family measurement/admission
tests, and all 633 default-feature renderer library tests pass. The original
family-wrapping and non-additive shaping failures are retained. All thirteen
font profiles now pass. Five real-font pairs compare inline declarations against
equivalent class-based labels: regular serif, italic, bold-italic, wrapped text
and code-face inheritance with a CSS comment. Each pair has identical dimensions
and pixel-identical raster output. All 59 prior corpus SVGs remain byte-identical;
namespace denial, real limits, flooding, timeout/cancellation cleanup and copied-
font invalidation/stale-selection checks pass.

Private evidence is
`native-mermaid-inline-family-corpus-20261103/INLINE-FAMILY-EVALUATION.json` under
the same QA root. The worker is ready for cache/GTK integration within this
documented contract; it has not replaced the installed app's renderer. Style/
layout controls, normal packaging/install/CI and full migration acceptance remain.

## Prepared image-cache qualification (2026-11-03)

The native diagram queue now prepares an owned renderer before image lookup on
its single bounded CPU lane. Cache identity includes the prepared environment;
both warm hits and fresh output validate that environment before delivery.
Cancellation also covers preparation. Resource identity bytes count against the
existing 32-entry / 16 MiB image cache. A failed CPU job retires its cache.

The compiled Merman adapter captures the request, canonical worker path and exact
font selection used to render. Font selection identity now includes worker
metadata as well as font bytes and role aliases; selected worker/font stamps are
validated before and after rendering. A retargeted worker alias is rejected by
the standalone selection API, while the prepared adapter continues using the
canonical runtime it captured. Outlined previews and subsequent vector zooms
use an empty font database, avoiding the GTK rasterizer's system-font scan.

Run the explicit real-process regression with private copies:

```bash
python3 scripts/test-gnome-mermaid-queue.py \
  /absolute/path/to/new-component/codex-thread-studio-mermaid \
  --output /absolute/path/to/new-private-queue-results
```

The test builds the native library test executable, runs one otherwise ignored
fixture test, modifies only copied font/ELF bytes and restores their contents.
Five queue requests invoke three actual renders: a warm request reuses its image,
a font change invalidates it, restoring the font reuses the earlier valid image,
and a worker replacement invalidates it again. Separate alias checks preserve
the captured runtime. The inspected preview is 736 × 180; vector zoom renders
2,944 × 720. These dimensions describe the fixture, not a speed claim.

The five queue unit tests cover profile keys, resource identity/validation,
preparation cancellation, generation cleanup and shutdown. The default native
library suite passes 295 tests; the two existing opt-in tests and this private-
fixture test remain normally ignored, with this fixture separately passing.
All native targets pass optimized compilation checks on Rust 1.97 and Rust 1.92.
Private evidence is
`native-mermaid-prepared-queue-runtime-20261103/PREPARED-QUEUE-EVALUATION.json`
under the same QA root. At that checkpoint the installed renderer remained Pele
and normal GTK/build/install had not selected the adapter. The adoption below
supersedes those integration gaps; the historical evidence is preserved.

## GTK adoption (2026-11-03)

The native GTK queue now uses Merman through the prepared resource adapter.
Protocol 3 reports the actual native math SVG count before text outlining;
previews and zoom keep this metadata and use no system font scan. The shared
appearance controls save style/layout alongside fonts, density and curves.
The parent/gateway remain compatible with Rust 1.92; building the independent
worker requires Rust 1.95. Verified prebuilt components support older parent
builds. Packaging validates protocol, source/patch fingerprints, binary and all
notices before writes; installation includes three executables. CI is wired but
has not been run remotely for this worktree.

Full native acceptance found a label containing supported formulas and an
unsupported sibling lost all its formula rendering. The pinned patch now renders
supported formula islands and XML-escapes declined islands as literal text.
Entirely unsupported labels retain their previous fallback. Flowchart and
sequence measurement follow the same mixed-label contract. Three new worker
regressions reproduce the original failure and pass after the fix. The existing
GTK flow/sequence acceptance still requires actual native counts of 3 and 7.

The corrected optimized worker is SHA-256
`39e3c2ece5a8b80e79bf88c0d47650185e3b37bf3b40888c30094e0bc9cbe4fa`.
Its 20 worker/font regressions pass. A disposable dependency workspace runs
1,029 renderer library tests with `math,diagram-flowchart,diagram-sequence`;
all pass with no ignored tests. Its pinned development lock retains shared
worker dependency versions, and three exact upstream fixtures have recorded
commit/hash provenance. Upstream conditional Node/browser helper paths are not
exercised; this is not an all-features/browser-parity claim. Repeat with:

```bash
python3 scripts/test-gnome-mermaid-renderer.py --output /path/to/new-renderer-tests
python3 scripts/test-gnome-mermaid-package.py /path/to/component --output /path/to/new-package-tests
```

All 59 valid sandbox corpus cases pass and remain byte-identical to the earlier
protocol-3 trial. Thirteen font profiles pass; five inline/class-based controls
are pixel-identical. Namespace denial, output/resource limits, timeout and
cancellation cleanup pass. Real prepared-cache checks verify changed font/worker
invalidation, restored-font reuse, alias rejection and vector zoom. All nine
package mutation/copy checks pass.

Private Wayland run `run-tj7qaak0` passes the complete native workflow including
the real 4.1 MB report, math counts 3/7, source/dirty/table controls and stale
viewport cleanup. Two-launch `appearance-ui-_se9ptct` checks saved styles/layouts,
reader/chat parity and edits retained across changes. Installed optimized run
`release-ui-qx568plm` passes every existing diagram/language/backend/Git/viewer
group plus process/event-gap recovery using the ordinary installer/launcher and
owned accessibility/Wayland input, with no application hooks. Representative
math and hand-drawn screenshots were inspected. The earlier trial's real
math-label failure remains preserved separately.

The source/package fingerprints and raw evidence are recorded in
`native-mermaid-gtk-adoption-20261103/GTK-ADOPTION-EVALUATION.json` under the private
QA root. See [current integrated candidate](../../docs/gnome-native.md#current-integrated-candidate)
for launch instructions and current limitations. Earlier installed packages and
the default installation are unchanged. This is an integrated checkpoint, not
completion of the full native application migration.
