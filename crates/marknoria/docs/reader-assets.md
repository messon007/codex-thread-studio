# Serve the shared UI assets from a host

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

The [minimal component host](../examples/reader-assets-host.rs) opens no file:

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
