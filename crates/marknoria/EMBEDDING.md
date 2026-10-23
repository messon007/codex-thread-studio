# Embedding MarkNoria

The library and standalone server use the same reader, parser and disk cache.
`server::Service::start(canonical_source)` starts a read-only, single-document
service on an ephemeral loopback port. Retain its handle while reading; dropping
it cancels indexing and rendering, closes the server and removes its cache.
Embedded services intentionally omit file browsing and file-opening APIs.

The host executable must call `server::index_worker()` before normal CLI/startup
handling and exit with that result when present. Indexing then runs in a bounded
child process rather than retaining parser peaks in the host. A process-wide
semaphore serializes index workers across embedded services.

Hosts should enforce canonical source confinement, source size limits, a bounded
number of services, idle expiration and authenticated creation/deletion. Never
proxy arbitrary addresses supplied by clients. Proxy only the retained service's
loopback address, preserve its reader-token authorization, restrict public routes
to known assets/read-only APIs and remove upstream Origin/Host headers.

Frontend modules and API calls use relative URLs, so the host can proxy the
reader beneath a unique per-document path. The iframe URL is
`/<host-prefix>/<id>/?embedded=1&line=42#<service-token>`. Use a same-origin iframe
with scripts enabled, override the reader's standalone `frame-ancestors 'none'`
to `'self'` only on the proxied reading UI, and prohibit external navigation.
Keep the isolated diagram renderer's policy unchanged; it uses the private
service directly and never receives reader or host credentials.

The embedded toolbar hides the standalone Open action and duplicate identity.
Hosts that do not want an installed backend Chromium dependency can add
`&diagrams=browser` to the iframe URL. This lazily renders only visible diagrams
using the same bundled Mermaid, PlantUML and D2 browser engines, with serialized
work, source/SVG budgets and sanitization. Allow the `api/diagram-source` route
and trusted engine assets through the proxy in this mode, but not `render.html`.
Permit self-hosted scripts/WASM and blob workers in the iframe CSP; no CDN is
needed. The standalone default remains backend-isolated diagram rendering.
Preview, paged Source, Contents, search, table pagination and column selection
remain inside the reader. The host can send a same-origin message from its parent
window: `{type: 'marknoria:navigate', line: 42}`. Navigation uses the preceding
heading in Preview, or exact physical source lines when no heading exists.

Embedders must vendor/build the pinned runtime assets with `npm ci --ignore-scripts`
and `npm run build`. Do not bundle machine-local files, caches or private fixtures.
