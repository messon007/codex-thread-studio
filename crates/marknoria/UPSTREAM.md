# MarkNoria source snapshot

Repository: https://github.com/messon007/marknoria
Revision: 42a9b8deb7c0068f52d4e22f11b444d1a2a0c586

Refresh with `node scripts/vendor-marknoria.mjs /path/to/marknoria` after building that checkout's assets with `npm run build`.
Only library and reader sources are imported. The manifest omits standalone binaries and desktop dependencies; diagram/sanitizer assets reuse Studio's pinned runtime assets. Editor/virtualizer bundles are copied into `ui/vendor`. No network or sibling checkout is needed at build/runtime.

Studio-specific embedding changes are applied reproducibly by `scripts/marknoria-embedding.mjs` (SHA-256: b0eb50be23351f7c5ba0777624846162a7889490af7e98d8b524fadb0c3e9c91). They add an explicitly editable single-document service, enable its embedded UI, and expose a bounded host state/control bridge. The default service and comparison remain read only. Parsing and source offsets are unchanged. The adapter also normalizes cross-platform test paths and releases validated Windows target handles before atomic replacement; revision and identity checks still run before replacement.
