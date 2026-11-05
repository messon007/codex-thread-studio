# Markdown images and links

The Web projection records Markdown-syntax image/link targets during Rust
parsing. It emits opaque reference IDs, never a local filename or a large data
URI, into sanitized HTML. `Reader::reference(revision,id)` returns kind, inline
status and exact original UTF-8 byte range; inline URI data is omitted.
`Reader::reference_target` is a bounded trusted-Rust accessor, not file authority.
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
normalized requested path and canonical target against these roots, including
encoded Unicode/spaces and symlinks. HTTP images are not fetched. Inline PNG,
JPEG, GIF, WebP and SVG images need no folder grant; SVG is decoded to inert PNG
by the existing supervised image worker, with external SVG resources disabled.
Standalone browser/server mode uses its existing authorized server root.

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

Every command retains document identity/revision checks. Image metadata supplies
its own immutable snapshot revision; a frame from another image revision fails.
The public state declares `document.references` and `capabilities.referenceInfo`,
`referenceImages`, `referenceProfile`. A parser-owned link emits `linkRequested`
with the original target, exact source range and owning identity. Bare autolinks
have no invented source range and declare approximate precision. Hosts authorize
and handle file/external navigation; this event grants no new read or write access.
Heading fragments navigate inside the reader. Source byte selections remain
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
eviction, limits source images to 25 MiB and retains the existing 2048-pixel
preview edge/128-frame/128-MiB decode-cache budgets. Small SVGs retain their
intrinsic dimensions rather than being enlarged to the preview edge. All preview
dimensions and 100% zoom describe projected pixels, not unbounded original data.

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
Studio adoption, raw-HTML `<img>` support, every heading slug dialect or release
performance acceptance; those remain separate compatibility work.

`npm run check:reference-policy` opens an actual editable Chromium reader and
verifies adding a shared directory, rejecting metadata/frames from a revoked
cached image, refreshing a retargeted symlink, and retaining/saving the same dirty
draft. Unit checks cover one current policy snapshot per resolution and the
257-directory limit including duplicate grants.
