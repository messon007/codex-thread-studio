# Document components

Immutable projections, shared assets, images and explicit Markdown resource
authority for ordinary DOM and embedded hosts. Runtime modules remain separate;
this reference groups their related contracts.

## Projection windows

`mountProjectionView` mounts bounded immutable Rust projections in an ordinary
host element. The standalone virtual reader uses the same `projectionHtml`
sanitizer; both paths use `mountTableView`, including independent complete-table
pagination, column controls, widths, cell references and horizontal scrolling.
Both paths also use `document-prose.css`, including identical default headings,
paragraphs, code and quote typography. Host content font/size/weight settings use
the same CSS variables and `readerPresentation` adapter.
The component does not run a second Markdown parser or treat a partial DOM table
as the query dataset.

```js
import { mountProjectionView, readerProjectionProvider } from './projection-view.mjs'
const owner = await client.ready()
const view = mountProjectionView(container, {
  provider: readerProjectionProvider(client, owner),
  viewport: scrollingElement,
  workspace: scrollingElement,
  translate,
  onSelection(selection) { /* Notes or host actions */ },
  onQuery(table) { /* mountTableQuery with readerQueryProvider */ },
  onChart(table) { /* mountTablePlotDialog with readerChartProvider */ },
})
await view.show(0, 5)
const selection = view.selection()
view.dispose()
```

The caller owns its surrounding navigation, virtualization and viewport. `show`
replaces one window; it does not append the complete document. A headless host
can supply a provider using `Reader::window` and `Reader::table` directly rather
than using an iframe service. Such a provider supplies `documentId`, `revision`,
`window({start,count,signal})`, and optionally
`table(id,{signal}) -> {table,revision,metadata,load}` with the existing table-view
provider contract. Notes, SQL, charts, diagram/image hydration and surrounding
controls remain explicit separately owned component integrations. This window
component does not replace the full reader's format renderers or editor.

At most five pages, 128 KiB UTF-8 HTML per page, 16,384 parsed nodes per page and
20,000 nodes per window are accepted. The immutable total, revision, contiguous
page numbers and source coordinates are checked. Unsupported/stale/oversized
candidates fail without replacing the previous window. A newer request aborts
the previous one; late responses cannot commit. Table fragments sharing one
original table identity are coalesced into one complete-table view per window.
Per-instance column/row state and widths survive remounts, capped at 256 tables;
different instances never share that state. Disposal cancels pending windows and
table requests and releases DOM, observers, floating tracks and scoped styles.

`selection()` / `onSelection` return the public typed selection shape. Ordinary
text has approximate one-based source-line precision; it never invents exact
Markdown byte offsets. A single original table cell has exact table/row/original
column coordinates. Quotes are bounded to 16,384 UTF-16 units without splitting a
surrogate pair. Rectangles are viewport CSS pixels of the host document. The host
can pass these references to the shared Notes component.

The v1 parent protocol adds `projectionWindow({start,count})`,
`sourceWindow({line})`, `sourceWindow({offset,previous})`, or
`sourceWindow({offset,end})` (an explicit range of at most 128 KiB), and
`sourceSelection({start,end,fromUtf16,toUtf16})`. All retain document/revision
checks and request cancellation. `sourceSelection` converts a source window's
LF-normalized UTF-16 positions to exact original UTF-8 bytes in Rust, preserving
BOM/CRLF/Unicode. `navigate` also accepts the resulting `source` byte range:
it opens Source, selects that range, and preserves its original bytes in the
returned selection. Ranges crossing the normal 200-line boundary use an explicit
bounded range window. Invalid UTF-8 boundaries and ranges over 128 KiB fail;
there is no silent clamping or byte-to-DOM-offset substitution.
Original-container PDF/EPUB use their own public components;
projection windows explicitly reject their placeholders. Binary formats reject
original UTF-8 source requests. No route grants an additional file.

```sh
node tests/browser/projection-view-check.mjs
node tests/browser/projection-view-check.mjs --webkit
node tests/browser/embedded-check.mjs target/release/examples/embedded-host
```

For durable Comments, await `view.captureSelection({signal})` and retain its
optional `textRange` alongside the typed location. The legacy `selection()` stays
synchronous. The [shared comment marker](notes.md#comment-markers) paints exact selectors
without changing the projection tree.

## Asset delivery

`marknoria::web::reader_assets()` returns an Axum router of the fixed reader's
static modules, styles and generated resources. It grants no document, path,
query, source, original-container or write API. Hosts can mount the exact shared
PDF/EPUB/table/Notes/jq/projection components without copying frontend sources,
rebuilding another bundle or starting an extra document service just for assets.

```rust,ignore
let app = axum::Router::new()
    .nest("/reader-assets", marknoria::web::reader_assets());
```

```js
const { mountProjectionView } = await import('/reader-assets/projection-view.mjs')
const { mountNotes } = await import('/reader-assets/notes.mjs')
const { mountPdfReader } = await import('/reader-assets/pdf-reader.mjs')
const { mountImageView } = await import('/reader-assets/image-view.mjs')
```

Module-relative imports, component stylesheet leases and PDF worker/font/CMap
URLs retain the mount prefix. Components keep their explicit data/storage/grant
providers and ownership contracts; serving their code does not authorize a file.
The consuming application owns its existing origin, CSP and gateway policies.
The full reader uses this same static route builder alongside its separately
guarded document APIs.

The shared diagram engine is available at `engine.mjs`; callers own a
`createDiagramRenderer()` instance, pass bounded source with a unique render ID,
and dispose it on close. Its module-relative sanitizer/script/worker paths also
retain the asset prefix. No document grant or backend Chromium is needed for
that browser rendering path.

Clean Web builds also generate the filesystem `web/vendor/purify.mjs` entry and
its license from the same locked package as the shared Rust-served asset. This
lets ordinary components be bundled or served directly without relying on the
HTTP alias. The build checks all owned Web module imports; the full controller's
intentional `engine.mjs` route alias is explicitly mapped to its shared source.

The [minimal component host](../../examples/reader-assets-host.rs) opens no file:

```sh
cargo build --locked --example reader-assets-host
node tests/browser/reader-assets-check.mjs target/debug/examples/reader-assets-host
```

It mounts assets under `/reader-assets`, runs ordinary components at that prefix,
checks the exact served source bytes and rejects document/file/query/source
routes. The example listens only on an ephemeral loopback address and shuts down
when stdin receives a newline.

`outline.mjs` supplies the shared virtualized Contents interaction over indexed
`{id,level,label,navigable?}` metadata. It does not parse document source.
`mountOutline` owns filter/collapse/active/focus state and cancellation; call
`setItems` on a new document and `dispose` when its owner closes. PDF destination
objects remain private to the renderer; public `outline({resolve:false})` pages
and `navigateOutline` use revision-scoped IDs.

## Images and animation

The main reader and `mountImageView` use the same `assetController` and scoped
`image-view.css` for Fit, zoom, 100% and animation controls. Rust owns decoding,
orientation, SVG sanitization, composited animation frames and source budgets.
JavaScript handles transport, layout and interaction; it does not decode source
formats into its own image model.

```js
import { mountImageView, readerImageProvider } from './image-view.mjs'
const owner = await client.ready()
const provider = await readerImageProvider(client, owner, 0, { signal })
const view = mountImageView(container, {
  ...provider, viewport: readingViewport, translate: hostTranslate,
  onChange() { hostMeasureLayout() },
})
await view.ready
const presentation = view.state() // owner/revision, zoom, paused, frame, status
view.dispose()
```

The provider uses only public `assetInfo`/`assetFrame` commands. No child DOM,
private globals, file/cache paths or private service URLs/tokens enter the view.
Metadata and frame responses are checked against the captured document/revision
and asset/frame IDs. A later document switch cannot silently retarget the view.

An ordinary host may instead provide `{owner,metadata,frame}` directly. `owner`
has an opaque documentId and original SHA-256 revision. Metadata is the public
`AssetInfo` record: id, revision, alt, projected width/height, dimensionsExact,
frames `{frame,delayMs}`, plays and optional original one-based pdfPage. The frame
provider receives `{documentId,revision,id,frame,signal}` and returns a projected
PNG Blob. It must honor AbortSignal, its owning scope and the same decode/transport
profile as the Rust implementation. The view is appended to its own figure;
neighboring host content and other image instances are preserved.

Metadata has at most 128 frames / 1 KiB alt text. The view bounds dimensions at
32,768 per side and 16 megapixels. Frames are at most 20 MiB and must be PNG;
source images are never inserted as active SVG/HTML. The public bridge permits
two concurrent frame commands and transfers immutable Blobs without JSON/base64
expansion. HTTP frame reads enforce their byte limit while streaming and cancel
rejected, oversized or aborted bodies. Busy frame requests retry with a bounded,
cancellable delay. These image limits do not replace the original-container PDF
renderer's separate canvas/pixel/selection profile.

Dimensions and 100% refer to the declared image resolution profile. Web image
documents and Markdown resources preserve source resolution within the existing
16-megapixel, 20-MiB-frame and animation/cache budgets; oversized full-resolution
output fails explicitly instead of silently reducing pixel detail. The default
indexed/native profile retains its existing previews. Original-container
bytes remain a separate explicit core/service capability. Lazy raster PDF layout
estimates are marked dimensionsExact:false; Web PDF/EPUB use their dedicated
original-container components. A projected image frame is not an invented source
byte selection or a PDF rectangle.

Dispose aborts requests/timers/listeners, revokes candidate and displayed object
URLs, removes decoded image nodes and releases owned style leases. Late providers
cannot repaint disposed figures. Styles and translation are scoped; alt/caption
text remains literal. Animation pause/resume, zoom and lifecycle code are shared
with the main reader, rather than copied into the host adapter.

`compact:true` keeps an inline image at its content size and reveals the same
controls on hover or keyboard focus, positioned within the explicit reading
viewport. Markdown [reference images](documents.md#markdown-resources-and-links) use this mode;
the default standalone figure layout remains unchanged.

`node tests/server/image-frames-check.mjs` tests stream bounds and cancellation.
`node tests/browser/image-view-check.mjs` runs the ordinary component contract;
`--webkit` selects native Wayland WebKitGTK and `--compact` uses a 480x480 viewport.
The contract covers owner/zoom isolation, literal data, translation, animation,
invalid metadata/PNG, cancellation and bitmap/URL/style ownership. The real public
embedding regression also mounts the view over an actual granted image service.
Markdown-linked resource authorization and hydration are a separate integration;
this component does not implicitly grant a document's parent directory.

## Markdown resources and links

The Web projection records Markdown-syntax and accepted raw HTML image/link targets during Rust
parsing. Accepted destinations become opaque reference IDs in image/link slots;
large data URIs stay out of those slots and index transport.
`Reader::reference(revision,id)` returns kind, inline
status and exact original UTF-8 byte range; inline URI data is omitted.
`Reader::reference_target` is a bounded trusted-Rust accessor, not file authority.
Markdown syntax has `precision:"exact"`. Raw HTML uses the enclosing original
HTML event's byte range, a zero-based occurrence within that event, and
`precision:"approximate"`; it does not invent attribute/tag byte offsets. The
existing html5ever tokenizer extracts decoded URI/alt/title values. A fragment
must pass the whole projection policy before its references are committed.
Events, srcset, styles and user-supplied internal markers do not pass through.
Raw table structure remains literal inside a parser-owned Markdown table cell.
The default indexed/native Markdown projection retains its existing inert image
fallback and does not collect these Web references.

Opening a single file grants no sibling access. An embedding host must explicitly
authorize each read-only resource directory before starting its service:

```rust,ignore
let options = marknoria::web::ServiceOptions::default()
    .reference_root(authorized_project_directory)?;
let service = marknoria::web::Service::start_with_options(grant, options).await?;
```

These are fixed service-lifetime grants, at most 257 distinct canonical directories.
Close the service when the host revokes a grant. Resolution checks both the
requested path and canonical final target against these roots, including encoded
Unicode/spaces and symlinks. Unix request paths resolve directory aliases first,
accepting macOS `/var` → `/private/var` without allowing an outside file symlink
back into a grant, or an inside file symlink escaping it. Windows retains its
URL-normalized lexical grant check before filesystem access, including UNC paths.
HTTP images are not fetched. Inline PNG,
JPEG, GIF, WebP and SVG images need no folder grant; SVG is decoded to inert PNG
by the existing supervised image worker, with external SVG resources disabled.
Standalone browser/server mode uses its existing authorized server root.
Root-relative `/image.png` resolves under the document's authorized root, and
local backslashes normalize to path separators, matching the frozen Studio
resource adapter. An explicit `file:` URL retains filesystem-absolute semantics;
normalization and canonical checks still confine every accepted target.

Hosts whose directory permissions change can add a trusted Rust policy callback:
`options.reference_roots_provider(move || current_authorized_directories())`.
Each local reference request takes one current policy snapshot, canonicalizes and
deduplicates its roots, and checks authority before reading even a cached image.
The callback augments static grants; do not add revocable directories as static
grants. Changes apply to new metadata/frame/link requests without closing the
document, discarding an editor draft or clearing unrelated project resources.
An image symlink resolving to a different canonical file starts a new snapshot.
Already accepted reads and browser-held images are not retroactively withdrawn.
The provider has no HTTP/parent configuration command and receives no document
credentials. Its work must remain bounded by the trusted host's grant policy.

Public parent-client commands are additive to v1:

| Command | Result |
| --- | --- |
| `referenceInfo({id})` | `{reference,localPath,openPath}`; an unauthorized sibling has null paths |
| `referenceImageInfo({id})` | `{state:'indexing'}` or `{state:'ready',reference,imageRevision,asset}` |
| `referenceImageFrame({id,imageRevision,frame})` | Owning revision/IDs and a projected PNG `blob` |
| `navigateFragment({fragment})` | Navigates a checked, decoded document fragment; missing targets fail explicitly |

Every command retains document identity/revision checks. Image metadata supplies
its own immutable snapshot revision; a frame from another image revision fails.
The public state declares `document.references` and `capabilities.referenceInfo`,
`referenceImages`, `referenceProfile`. A parser-owned link emits `linkRequested`
with the original target, declared source precision and owning identity. Bare autolinks
have no invented source range and declare approximate precision. Hosts authorize
and handle file/external navigation; this event grants no new read or write access.
Explicit raw HTML IDs/names are converted into bounded internal anchor mappings,
never real IDs capable of replacing a host/source widget. Anchor names are at
most 256 UTF-8 bytes, with at most 4096 entries. The existing heading navigation
and these explicit fragments use the same public command; no new heading-slug
dialect is claimed. Source byte selections remain
distinct from rendered text or image coordinates.

Ordinary DOM hosts use the same component as the virtual reader:

```js
import { mountReferences, readerReferenceProvider } from './references.mjs'
const owner = await client.ready()
const references = mountReferences(projectionContainer, {
  owner, viewport: readingViewport,
  provider: readerReferenceProvider(client, owner),
  onLink(value) { hostOpenAuthorizedReference(value) },
  onChange(article) { hostMeasureLayout(article) },
})
// Mount only sanitized Rust projections. Dispose before replacing the owner.
references.dispose()
```

Images reuse `mountImageView` in compact mode. Focus/hover shows the common zoom
and animation controls within the reading viewport. Offscreen/unmounted images
release frames, object URLs and requests; retained layout slots avoid collapsing
the document. Metadata preparation has two concurrent tasks and a 75-second
deadline; busy responses retry with cancellation. Mounted images are limited to
32 / 16 megapixels in total. Rust caches four owned image snapshots with LRU
eviction and limits source images to 25 MiB. The Web profile preserves source
resolution within 16 megapixels/32,768 per side, 20 MiB per PNG transport frame,
128 frames, 16 MiB animation timeline and 128 MiB encoded image-cache budgets.
An over-budget full-resolution image fails explicitly rather than presenting a
2048-pixel reduction as original 100%. The indexed/native profile retains its
existing still/animated preview limits. Small SVGs retain their
intrinsic dimensions rather than being enlarged to the preview edge. All preview
dimensions and 100% zoom describe the declared resolution profile. SVG remains
an inert, supervised raster with bounded embedded-resource decoding.

Reference metadata is limited to 4096 targets and 32 MiB of private URI data;
non-inline targets/links are at most 8192 bytes. These limits fail explicitly.
Large inline URI bytes do not consume the rendered page's 128-KiB HTML budget;
event and final HTML limits remain enforced. Indexing never reads a referenced
file or starts a network fetch. Closing an owning document cancels its supervised
decoders; closing the service removes owned resource snapshots and inputs.

`npm run check:markdown-references` checks actual Chromium services plus an
ordinary DOM host; `-- --webkit` additionally executes the public contract in
native Wayland WebKitGTK. Generated fixtures cover BOM/CRLF, Unicode, table SVG,
inline/local images, denied/remote targets, link identity, stale versions,
busy retry, cache eviction and cleanup. Core source/audit tests preserve the
indexed projection and exact original bytes. This does not establish complete
Studio adoption, every heading slug dialect or release performance acceptance;
those remain separate compatibility work. The same check additionally executes
the finite raw HTML/explicit-anchor/source-pixel corpus in both engines: local,
inline, table, root-relative and backslash resources, duplicate occurrences,
unsafe markup, Unicode outlines, and 4096×1024 line/small-text pixels at Fit/100%.

`npm run check:reference-policy` opens an actual editable Chromium reader and
verifies adding a shared directory, rejecting metadata/frames from a revoked
cached image, refreshing a retargeted symlink, and retaining/saving the same dirty
draft. Unit checks cover one current policy snapshot per resolution and the
257-directory limit including duplicate grants.

## Markdown compatibility

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

### Bounded safe HTML projection

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
[reference profile and shared corpus](documents.md#markdown-resources-and-links). This does not add
heading-slug dialects absent from the frozen baseline.

Parser-owned row sentinels attach exact Markdown source ranges before final
projection. Raw attributes/comments cannot forge these sentinels. A raw table
preceding a nested Markdown table cannot steal its row anchors; structural raw
HTML inside a Markdown cell stays literal so cached row/cell projection cannot be
restructured. Normal raw inline formatting in cells remains available. Autolinks
also respect raw anchor/code/pre scopes, preventing nested generated links.
Source bytes/revisions/UTF-8 anchors are unchanged by projection.

The synthetic BOM/CRLF/Unicode corpus covers safe inline/block/link cases,
original table anchors and unchanged source bytes. Use the shared Markdown,
reference, projection and embedded-host checks when changing this profile;
measure browser transport/layout separately from native parser performance.
