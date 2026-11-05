# Studio release size: measurements and approval proposal

Tracks Studio #51 and MarkNoria #10. This is a proposal, not an approved release
configuration change. Studio #51 explicitly requires approval before implementation.
On 2026-11-05 the user chose to deliver functional fixes first and retain this
proposal only. Resource deduplication, symbol changes and release compression
changes are not authorized for implementation in this round.

## Existing released artifacts

GitHub release `pre-typescript` (application 0.2.0), measured from its assets:

| Artifact | Bytes | Decimal MB |
|---|---:|---:|
| Linux AppImage | 88,017,400 | 88.02 |
| Linux DEB | 10,916,516 | 10.92 |
| Linux RPM | 10,915,782 | 10.92 |
| Windows NSIS | 8,208,172 | 8.21 |
| Windows MSI | 10,465,280 | 10.47 |

The reported approximately 80 M is the AppImage, not every release format. The
AppImage's SquashFS contains WebKitGTK (89,985,064 uncompressed bytes),
JavaScriptCore (33,712,504), ICU data (29,480,008), GTK and other portable runtime
libraries; the application itself is 31,276,832 bytes. These are individual
uncompressed file sizes, not additive compressed contributions. SquashFS already
compresses data and removes duplicates. Changing MB/MiB units is not optimization.

An isolated measurement extracted these same files without executing them and
recompressed them with XZ, 128 KiB blocks, two workers and a 256 MiB memory limit.
The original Zstd filesystem is 87,070,084 bytes; the XZ filesystem is 79,175,680.
With the same 944,632-byte launcher, the estimated artifact would be 80,120,312
bytes: about 9% smaller, still above either 50 MB or 50 MiB. This is a compression
measurement, not a tested/issued replacement AppImage. No release configuration
or shipped artifact was changed.

## Current shared-reader release sample

The local release sample with the adopted 815-file SDK is 273,744,392 bytes;
`.rodata` accounts for 228,899,224 and `.text` for 25,681,207. It is a different
version from release 0.2.0, so its size must not be substituted for that release's
package size. The final functional-fix sample at SDK revision `5e5d4de` is
275,226,112 bytes, with 228,910,752 bytes of `.rodata` and 26,698,791 of `.text`.
The 1,481,720-byte difference includes functional and dependency changes; it is
not attributed exclusively to the font-stack update. This is a local executable
measurement, not a new packaged release or an optimization result.

110 `ts_parse_table` symbols account for 120,846,560 uncompressed bytes. Language
coverage therefore has a real size cost; removing languages changes functionality
and is not the recommended default. Stripping symbols or tuning Rust codegen
alone cannot remove all this static data.

Exact byte searches in this binary found two copies each of Mermaid (5,494,801
bytes), PlantUML (3,359,666) and PDF worker (1,264,342): 10,118,809 bytes of measured
redundant payload. D2 also has host/SDK variants of about 11.5 MB each, but their
wrappers differ; merging those requires additional compatibility work and is not
included in the initial estimate. Compressed savings are not yet measured.

## Proposed first implementation, pending approval

1. Reuse the fixed SDK's public stateless asset router for the three byte-identical
   host assets above. Keep URL compatibility, MIME types, CSP, worker boundaries
   and chat/reader functionality. Remove only the now-redundant embedded copies.
2. Measure release-symbol stripping on private copies first; if adopted, keep
   diagnostic symbols as separate build artifacts and document reduced in-binary
   backtrace names. No language, format, diagram engine or offline support is cut.
3. Keep current portable AppImage runtime coverage. Evaluate its supported XZ/
   block-size options and actual startup before adopting a compression change.
   Preserve the current configuration if compatibility or gains are inadequate.

Validate the existing diagram/PDF/chat/reader paths, offline startup and three
platform CI. Record raw binary and each final artifact size at a fixed commit,
plus startup/responsiveness changes. Do not hide size by moving required data to
unrecorded downloads or by dropping languages/runtime libraries.

The initial route preserves features and offers measured opportunities, but does
not promise a 50 MB AppImage. DEB/RPM/Windows installers already met 50 MB at the
released baseline. A strict 50 MB requirement for every future portable package
would require a separately reviewed distribution/runtime tradeoff if lossless
changes do not suffice. No such tradeoff is authorized by this document.
