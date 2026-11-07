# Large Markdown in Studio

Markdown files of at least 1 MiB use MarkNoria's embedded reader and paged editor.
Smaller files keep the existing editable Comrak/WASM document view. Non-Markdown
readers are unchanged. The current large-document source limit is 128 MiB.
This initial integration supports native workspace paths; Windows/WSL workspaces
retain their previous reader and size limit until remote indexing is implemented.

## Shared implementation

The source/resource snapshot in `crates/marknoria` is the exact reviewed public
package pinned externally by `scripts/marknoria-release.json`. Its own
`reader-manifest.json` records source revision, API/protocol versions, all file
hashes, build tools, source mappings and third-party notices. Normal builds need
no upstream checkout, token, npm asset build, separately installed reader binary
or backend Chromium. All reader resources come from this package; Studio does
not substitute its own sanitizer, editor, virtualizer or diagram versions.

The integration uses only `marknoria::reader`, `marknoria::web::Service` and the
versioned parent protocol. Static component modules are served by the separate
public `reader_assets()` router, which grants no document API/file authority.
The host imports the official client through this route. It does not inspect
child DOM, diagnostic globals or implementation controls, rewrite reader HTML,
inject CSS, or relax the reader's CSP. The old embedding and SQL/jq source patches
are removed. `toolbar: 'host'` delegates header and status presentation to Studio.

The Studio executable handles the index-worker argument before normal startup;
parser memory peaks leave the main process when the worker exits.

Authenticated document opening resolves canonical paths using Studio's existing
project/shared-directory boundaries. For large Markdown the response contains
reader metadata, not the complete source string. A per-document private loopback
service is exposed through a same-origin, allowlisted streaming proxy; remote
browser/SSH users need only the Studio port. Reader API tokens are separate from
Studio's gateway token. Embedded sessions cannot browse/open other files or
access the privileged diagram-rendering page. Only project files enable Edit;
documents opened through configured shared directories outside that project
remain read only. The proxy checks the original canonical path/project boundary
again before editing, limits request bodies, and only permits the explicitly allowlisted read/query/edit/note operations.
No gateway credentials are forwarded to the private service. Existing gateway authentication
and Host/Origin checks remain in place.

## Bounded work and lifecycle

- One process-wide indexing slot, four retained reader sessions at most.
- A two-minute idle lease, renewed by authenticated reading/heartbeat requests.
  Closing, replacing or abandoning a reader cancels indexing/rendering and removes
  its private cache; the idle sweeper catches abruptly disconnected clients.
  The host probes every 30 seconds and on return to the foreground/online, with
  one request in flight and a 10-second timeout. A closed/expired or unauthorized
  lease stops polling and shows a refresh/reconnect message. Refresh still asks
  before discarding unsaved edits; late child events cannot reactivate a dead lease.
  Temporary network errors remain retryable without repeatedly reporting the same failure.
- Disk-backed private caches by default, with the same storage/quota checks as
  MarkNoria. `READER_CACHE_DIR` is an optional disk-backed override; an explicitly
  configured `TMPDIR` also overrides the default. Avoid quota-limited tmpfs.
- Virtualized document and outline, bounded neighboring fragments, paged tables
  and selectable columns, exact physical-line Source windows (up to 128 KiB).
- Preview/Source/Edit, Save, outline and search reuse Studio's existing document toolbar,
  with the same action order and one shared footer for line count/size/read-only
  status. The standalone reader's header and footer are hidden before first
  paint; its virtualized outline and paged views remain inside the reading area.
  Link line anchors use the preceding heading, falling back to exact Source lines.
- Mermaid, PlantUML and D2 use lazily loaded browser engines, serialized for
  visible diagrams only, with bounded SVG caches, source limits and sanitization.
  Studio's proxy does not expose the backend Chromium-rendering API. This avoids
  adding a server-side browser dependency; very complex individual diagrams can
  still consume browser CPU even though document/table DOM work is bounded.

Edit supports current-page Markdown/code highlighting and undo/redo, including
undo after Save. Pages normally contain at most 200 lines / 128 KiB, with a 512 KiB
edited-page limit. Save and Ctrl/⌘ S write the original file; changing pages or
using Go saves first. View switches preserve the unsaved page in memory without
saving; Preview/Source show the saved revision. Contents maps headings through
unsaved page changes, preserves edits for page-local jumps, and saves before
cross-page jumps. New headings appear after Save. Studio theme changes update
the editor and syntax palette without replacing its frame or text.

Rust checks source revisions, locks and file identity, streams a same-directory
replacement, and atomically replaces the file. Conflicts keep the editing page;
the paged path does not offer force-overwrite. Reload and closing/replacing the
document ask before discarding edits; in-flight operations block those actions.
There is no background save or browser draft storage. Undo history is scoped to
the page; cross-page selections and whole-document replacement are unsupported.
The existing source-size, UTF-8, LF/CRLF and safe-file restrictions apply. Cache
snapshots and replacement files need disk space. Unix advisory locks cannot rule
out a noncooperating writer racing the final version check and rename. Windows
must release target handles after its final checks to permit replacement; external
writers can also race that short interval.

Large documents do not support Studio's selection-comment
workflow yet. Source is paged; browser Find only sees
mounted content, so use the reader's server-side search. Inline local images
currently remain placeholders, as in the standalone reader. Indexing reads the
whole source before publishing the index, so peak memory is structure-dependent,
not constant-memory. First-content streaming during indexing remains future work.

## Refresh and verify

Review a fixed public archive and its external SHA-256 pin, then run:

```sh
node scripts/vendor-marknoria.mjs /path/to/reviewed-reader.tar.gz
node scripts/vendor-marknoria.mjs --verify
python3 scripts/import-marknoria-test.py
cargo test --manifest-path crates/marknoria/Cargo.toml --target-dir target/reader-sdk --locked --test reader_api --test markdown_profile
cargo test -p codex-thread-studio --bin codex-thread-studio
node --test ui/document-workspace-controller.test.mjs
STUDIO_PLAYWRIGHT_MODULE=/path/to/playwright/index.mjs \
STUDIO_CHROMIUM_PATH=/path/to/test-browser \
node scripts/smoke-large-markdown.mjs /path/to/large-report.md
```

The importer checks the externally reviewed archive hash before extraction,
rejects links, path traversal, duplicate/foreign entries and resource-budget
violations, verifies the full inventory against the separate pin, and atomically
replaces the owned vendor directory. Failure preserves the previous package.
`--check` validates a candidate without installing it; `--verify` checks installed
source before creating build artifacts. No imported source is patched or
reformatted. The nested SDK workspace stays excluded from Studio's formatting
workspace; its library is still a normal path dependency. A consuming Cargo lock
records compatible patch resolution independently of the standalone SDK lock.

On Linux, add `--webkit` to the smoke command to exercise native WebKitGTK as well
as Chromium (a graphical session and Python GI/WebKit2 bindings are needed only
for this optional test). The smoke deliberately sets the backend Chromium path
to an unavailable executable and verifies all three diagrams still render.

The initial integration was checked against a 31.26 MiB fixture made from repeated
copies of the real wide-table report: 9,224 indexed fragments, 80 tables, including
a 9,970-row/54-column table. Chromium and native WebKitGTK both retained fewer
than 1,000 document DOM elements at the measured opening viewport. Pagination
kept the outer viewport stable; two-column projection, paged-source byte fidelity,
small-file editability, authorization boundaries and cache cleanup passed.
These are fixture-specific checks, not universal latency or memory guarantees.

The optional actual-server expiry smoke uses an isolated offline backend, closes
only its own lease to exercise the real `410` response, and checks that refresh
requires consent before discarding a dirty document. It also verifies no further
polls, successful reopening, unchanged source bytes and owned cache cleanup:

```bash
STUDIO_REMOTE_BINARY=/absolute/path/to/codex-thread-studio \
STUDIO_PLAYWRIGHT_MODULE=/absolute/path/to/playwright/index.mjs \
STUDIO_CHROMIUM_PATH=/absolute/path/to/chrome \
node scripts/smoke-reader-expiry.mjs
```
