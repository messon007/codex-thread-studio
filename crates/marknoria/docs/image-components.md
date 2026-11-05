# Shared projected image view

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

Dimensions and 100% refer to the bounded projected preview. Original-container
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
viewport. Markdown [reference images](markdown-references.md) use this mode;
the default standalone figure layout remains unchanged.

`node tests/server/image-frames-check.mjs` tests stream bounds and cancellation.
`node tests/browser/image-view-check.mjs` runs the ordinary component contract;
`--webkit` selects native Wayland WebKitGTK and `--compact` uses a 480x480 viewport.
The contract covers owner/zoom isolation, literal data, translation, animation,
invalid metadata/PNG, cancellation and bitmap/URL/style ownership. The real public
embedding regression also mounts the view over an actual granted image service.
Markdown-linked resource authorization and hydration are a separate integration;
this component does not implicitly grant a document's parent directory.

Observed local checks: the ordinary contract passes 19 assertions in Chromium
and Wayland WebKitGTK. The granted-service embedding check also runs eight
actual WebKit cross-origin Blob/ordinary-view assertions with `--webkit`, in
addition to the Chromium multi-document suite. These are Web/WebView checks;
platform-native UI and FFI are unchanged.
