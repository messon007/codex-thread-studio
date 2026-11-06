# Reduced syntax profile and public translations — 2026-11-06

Studio starts at main `13f696191bdb073bfcea701eada504ef58a25f3f` and adopts
unmodified public delivery `c8947e7eb3a5471780dbdadf9089252c95eaedc8`.
Upstream main `9d882044fde656eec9f54c48bc98cc8404b35bef` only adds acceptance
records. The runtime commit closes MarkNoria #28 and #29: complete live reader
translations and selectable linked syntax profiles. The previous
[comment/search adoption](marknoria-comments-adoption.md) remains historical.

Two clean offline exports reproduce the official 847-file package:

- Archive SHA-256: `2018b8ad930fd5dfe75765cdd59b35b8874834718053096b75ff816bc7d2d2c8`
- Tree SHA-256: `bad66fe99af8a68250f54424bfd0314365f66e67f4fdccc1322853b79fc44208`

All nine repeat/inventory/tamper/profile checks pass. The public npm cache was
warmed from locked registry packages before those two offline exports. Installed
SDK files remain byte-for-byte identical to the manifest. No platform-native
widgets/FFI, sibling checkout artifacts, user files or private build dependencies
are imported. The fixed package still supports its default full profile.

## Integration

Studio explicitly selects `default-features = false` and `web,syntax-reduced`.
Exactly COBOL, D, HLSL, F#, OCaml, Idris, Odin, C#, Perl, Ruby, Elixir, Visual Basic
and PHP are excluded at dependency/link time. The other 97 grammars remain,
including Lean and Verilog. SDK feature conflicts reject any additive dependency
that attempts to restore an excluded parser. The outer lock only drops the 13
excluded Arborium packages and their dependency entries; no versions change.

Excluded fences, source files and aliases retain original text, escaping and
source anchors with no syntax coloring; they do not borrow another grammar.
Retained languages and supported injections keep their normal highlighting.
Parsing, bounded search, selection, editing and host permissions are unchanged.

The shared catalog now includes case/search statuses, the explicit full-cell
label and tooltip, marker actions and `Comment {current} / {total}`. Studio removes
the four-key temporary adapter supplement from #28, supplies Chinese translations
through the normal public catalog, and preserves numeric placeholders. Only its
actual host-specific `Ask AI` key remains appended. Live popup labels update
through the shared public translation implementation.

Copy/Comment/Translate remain Studio-owned. Reader Favorite/Notes entries remain
absent; Studio's separate global favorites and session-owned persistent comments
are preserved. Exact textRange/revision and SQL provenance checks are unchanged.
The obsolete session write-access menu stays removed; no native branch is merged.

## Validation and measurements

Delivery verification, UI type/generated checks and the existing UI suite pass.
The retained UI translation test now exercises the public catalog plus translator,
including overlapping-comment placeholders. The ordinary Studio shell smoke also
asserts all 13 excluded fences preserve Unicode/inert HTML without colored spans,
while Rust still highlights; its four-format flows pass Chromium and native
Wayland WebKit 2.52.6. Public translation contracts pass 32 assertions per engine;
exact comment-marker contracts pass 46 per engine. Independent actual Studio
acceptance passes all nine groups, including real SQLite save/reload, exact second
occurrence, Source bytes, overlap actions, case-search and deletion/clear. Source
bytes remain unchanged and owned caches are reclaimed.

An independent ELF audit compares the actual Studio programs using identical
release settings. The previous full program is 275,626,712 bytes; the new reduced
program is 221,479,720 bytes: 54,146,992 bytes (51.64 MiB, 19.64%) smaller. Exactly
the requested 13 parse tables disappear; all 97 retained table byte fingerprints
are unchanged. This is a before/after Studio ELF comparison across this small
reader update, not an AppImage/installer size or isolated profile benchmark.
No speedup or cold-disk startup claim is made. Harness times include browser
interaction/polling and cannot be used as isolated highlighter latency.

CI retains full-profile independent SDK tests and runs the actual Studio reduced
dependency contract on Linux, macOS and Windows: all 13 excluded source files,
escaping, original bytes and exact UTF-16/UTF-8 selections, plus retained Rust.
The reduced SDK API/source/registry suite is also run independently locally. Final adopting PR records executed Rust, browser,
linked-program measurements and all six required final-head CI results. Local
reports/provenance are retained in `marknoria-syntax-validation/` and
`marknoria-syntax-build.json` beside the dedicated build caches.
