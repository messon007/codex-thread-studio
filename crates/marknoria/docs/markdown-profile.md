# Shared Markdown profile

The parser remains Rust pulldown-cmark **0.13.6**, preserving original UTF-8
source offsets, immutable snapshots, bounded pages and existing native widgets.
The frozen Studio comparison is Comrak **0.57.0**, configured in
`a2f87a37d5a3d0de7c0dbe0f0f9666e0b13c3a4a:crates/markdown-wasm/src/lib.rs`.
This is source evidence, not proof of an actual updated Studio window.

| Behavior | Shared Rust implementation | Frozen Studio evidence |
| --- | --- | --- |
| Tables, task lists, strike-through | Enabled | Enabled |
| Soft line breaks | Render as hard breaks | `hardbreaks:true` |
| Extended URL/email links | HTTP(S), `www.` and email; punctuation-aware | `autolink:true` |
| Inline/fenced code and existing links | Excluded from text autolinking | Parser-owned code/link nodes |
| BOM, CRLF, Unicode | Original bytes retained; exact source selections tested | Byte/UTF-16 heading offsets recorded separately |
| Footnotes/math | Not enabled by this profile | Not enabled in the frozen options |
| Raw HTML | Bounded safe inline/block/table tags; unsafe/unsupported fragments remain literal | Unsafe HTML emitted, then host sanitization |
| Markdown images/local resources | Web syntax/raw HTML use the shared Rust resource authority and image component; indexed/native retain their inert alternative text | Host resource adapter |

The safe HTML subset, Web raw resources and explicit internal anchors are now
implemented, but full HTML styles and actual common-entry Studio adoption remain
work; the rows are not silently counted
as equivalent. Hosts must never compensate by inserting untrusted HTML
without the shared sanitizer or by granting a document's entire parent folder.
Diagram handling remains the separately bounded shared engine.

Text autolinking uses pinned [linkify 0.11.0](https://docs.rs/linkify/0.11.0/linkify/)
(MIT OR Apache-2.0) for its linear URL/email scanner. Adjacent parser text events
are merged with pulldown-cmark's own `TextMergeStream` before scanning, so an
underscore inside a URL is not mistaken for an event boundary. It never rewrites
the source snapshot or source ranges. The existing per-page HTML/event bounds
also bound expanded link events; excessive expansion fails explicitly.

`tests/support/markdown.rs` generates synthetic shared BOM/CRLF/Unicode/GFM bytes.
The public Rust test and `examples/reader-corpus` use the same function. Browser
and actual Tao/Wry WebView use `tests/browser/contracts/markdown.mjs` against the
same generated file, including code literals, opener isolation, safe inline/block HTML, script
inertness and original table line anchors. Generated documents stay in private test dirs.

`benchmarks/markdown/` has a separate locked native Comrak comparison. Core does
more work (indexing, anchors, caches and bounded windows) than a whole-string HTML
conversion, so timings must retain those labels. Native parser timings do not
substitute for WASM startup, DOM, bridge transport or actual Studio adoption.

## Native comparison observation

On Ubuntu 26.04.1, three fresh processes per engine read the same **663,555-byte**
generated corpus. Core indexed it in **36.4–37.1 ms**, returned its first
**678-byte** window in **38.5–39.2 ms**, and read all 4,096 windows in
**50.6–51.4 ms**; process peak RSS was **18,000–18,084 KiB**. Comrak returned its
whole **1,268,736-byte** HTML string in **18.8–23.8 ms**, at
**24,564–24,688 KiB** peak RSS. Core's total HTML was 1,777,549 bytes because it
also contains source anchors and syntax markup.

These observations favor neither engine universally: Comrak's native whole-string
conversion is faster in this case; core also pays for indexing, cache writes and
source-aware bounded delivery. The comparison supports retaining Rust work away
from the UI and measuring browser DOM/transport separately. It is not a WASM,
Studio, cold GUI or hard memory-limit result. The separate benchmark lockfile
records its exact dependency graph; the production lock adds only linkify.


## Bounded safe HTML projection

`src/core/markdown_html.rs` uses the already locked html5ever **0.26.0** tokenizer
(previously transitively used by kuchikiki). No new registry package is added.
It preserves separate opening/closing inline events without fragment-tree
normalization prematurely closing them. Supported inline/block/table/detail tags
are rendered; event handlers, IDs/names/classes/styles, data attributes and foreign
namespaces are not passed through. Attribute values are bounded to 2,048 bytes;
links allow HTTP(S), mailto and fragments with control characters rejected and
opener isolation added. Limited title/table-span/alignment/list-start/time/detail
attributes retain their semantics. Comments/doctypes are not document UI.

In the indexed/native profile, scripts, frames, images/resources, SVG/MathML,
forms and other unsupported tags
retain the existing literal inert fallback for their fragment. Existing per-page
128 KiB HTML/16,384-event bounds also limit token/output work. CSS/resources are
not authorized by safe HTML rendering; the shared DOM sanitizer/CSP remains
mandatory. This subset is not a claim that every frozen Studio HTML style or
local-resource behavior is already accepted.

The Web profile uses that same tokenizer transactionally to create raw image/link
slots and safe internal explicit-anchor mappings. Raw references declare the
enclosing source fragment plus occurrence and approximate precision; Markdown
syntax remains exact. Source IDs never replace reader widgets. See the bounded
[reference profile and shared corpus](markdown-references.md). This does not add
heading-slug dialects absent from the frozen baseline.

Parser-owned row sentinels attach exact Markdown source ranges before final
projection. Raw attributes/comments cannot forge these sentinels. A raw table
preceding a nested Markdown table cannot steal its row anchors; structural raw
HTML inside a Markdown cell stays literal so cached row/cell projection cannot be
restructured. Normal raw inline formatting in cells remains available. Autolinks
also respect raw anchor/code/pre scopes, preventing nested generated links.
Source bytes/revisions/UTF-8 anchors are unchanged by projection.

The synthetic BOM/CRLF/Unicode corpus now includes safe inline/block/link cases.
Release Chromium 149 and actual native Wayland Tao/Wry WebView each pass **22
presentation assertions**. The required Rust suite passes **123 library tests**,
9 public-facade tests, 16 native-session tests and the remaining source/Markdown/
architecture/desktop tests. The release table/shared suite retains **286**, and
the real two-pane public host passes. The native Comrak timing above belongs to
the earlier corpus before this fixture extension; it is not a new timing claim
for the safe-HTML implementation. Current WebView navigation-to-useful observation
is about 2.9 seconds and does not meet/prove any unresolved cold-start parity
budget; no universal parser/performance win is claimed.

## Actual frozen Studio WASM observation

The optional `tests/browser/markdown-engines-check.mjs` reads only the fixed Git
objects for Studio's checked Comrak JS/WASM resources and verifies their recorded
hashes. It runs the actual WASM parser in three fresh Web Workers, not a native
Comrak substitute. On the 489,983-byte generated BOM/CRLF/Unicode/safe-HTML corpus
(512 separated units, one leading BOM), both engines report 1,024 headings with
matching initial labels/source lines, and retain checked GFM/raw-HTML semantics.

On this Ubuntu 26.04.1 / Chromium 149 run, WASM parse/return takes 44.6–63.1 ms;
worker startup, transfer and the comparison's detached-DOM inspection take
92.6–111.8 ms in total. WASM's linear-memory capacity is 13,697,024 bytes; that is
not browser RSS or a live-allocation measurement. Its whole HTML is 793,600 bytes.
The supervised Rust index takes 37.2 ms with 17,748 KiB process peak RSS, producing
2,048 cached pages / 1,012,840 HTML bytes including source/syntax markup. Its first
five-page HTTP window is 3,389 bytes and takes 2.62 ms after indexing. Indexing,
first-window transport, complete-string conversion and DOM inspection are
different work; these observations do not establish a universal engine winner,
cold GUI parity or a hard browser-memory limit.

```sh
node tests/browser/markdown-engines-check.mjs /path/to/studio-git \
  a2f87a37d5a3d0de7c0dbe0f0f9666e0b13c3a4a
```

Only generated public fixtures and private ignored timing artifacts are used.
Studio's active worktree, build output and runtime are not read by this check.
