# Shared-reader maintenance

## Ownership and supported targets

MarkNoria owns its Rust document/runtime core, browser/WebView UI, public service,
components and fixed-source delivery. An integrating application owns its shell,
file grants/gateway, persistence policy and business/AI actions. Use
`marknoria::reader`, `marknoria::web::Service`, the stateless asset router,
`createReaderClient` and documented components rather than private reader DOM or
another checkout's build output.

Browser/server, WebView desktop and GTK/AppKit/WinUI native apps remain maintained
targets. Native widgets/controller/FFI are outside the Web-reader package, not
removed product support. Preserve these boundaries when updating shared code.

## Updating shared behavior

1. Change the owning core/runtime or shared UI component, preserving immutable
   source anchors, declared capability grants, bounded work and cancellation.
2. Exercise the relevant public core/component/iframe contracts and actual user
   interactions. Run Chromium and native Wayland WebKit where applicable; GUI
   suites run sequentially. Test a settled layout, not only the response instant.
3. Check ordinary and virtualized mounts, multiple documents/panes, replacement,
   abort, disposal, original-byte fidelity and private-cache cleanup. Measure
   indexing, first useful content and responsiveness separately.
4. Produce and verify an immutable [source package](delivery.md). The consumer
   pins it unchanged, removes absorbed host patches, and runs its own integration
   and platform tests. Upstream closure does not establish downstream adoption.

Hosts can disable reader Notes and supply their own selection menu/actions using
public presentation settings and bounded typed references. Extra actions are
host-owned; the reader does not require an AI account or publish selections to
another application. See [Notes](../api/notes.md) and [Embedding](../api/embedding.md).

## Protocol and lifecycle invariants

Keep document/revision checks, exact-origin/source messaging, immutable result
ownership and revoked resource authority ahead of cached lookups. Never grant an
entire parent directory just because one file was opened. New work must cancel or
supersede old callbacks before they can publish UI state. Platform-cache or font
warm-up is not evidence of a cold-start fix.

A successful format/component check does not establish complete shell, visual,
accessibility or cross-platform acceptance. Actual Windows/WSL2 execution is
different from testing a portable read-only adapter on Linux. Core Windows file
replacement tests do not imply that a native window was exercised. Keep explicit
unsupported transport/format cases and platform limits visible in the relevant
interfaces and test results.

## Documentation and test evidence

Document stable behavior by topic. Keep test commands and prerequisites in
[the build/check entry points](../../README.md#validation), API profiles in [Embedding](../api/embedding.md)
and comparative measurements in [Performance](performance.md). Per-run commands,
assertion counts, screenshots, package pins and issue status belong in the
relevant existing issue/PR, release metadata or ignored artifacts. Git history
retains superseded documents; do not keep parallel acceptance ledgers, copied
status snapshots or a new `acceptance-*` file for every fix.

## Packaging scope

Keep full functionality by default. The explicit reduced syntax profile removes
only the documented 13 grammars; it does not authorize further language/format or
portable-runtime removal. Asset deduplication, symbol separation and installer
compression were a separate deferred proposal, not implemented by the functional
reader fixes. Measure and validate such changes separately; do not promise a
50 MB AppImage from raw ELF or uncompressed asset estimates.
