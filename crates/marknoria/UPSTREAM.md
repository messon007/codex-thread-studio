# MarkNoria source snapshot

Repository: https://github.com/messon007/marknoria
Revision: 42a9b8deb7c0068f52d4e22f11b444d1a2a0c586

Refresh with `node scripts/vendor-marknoria.mjs /path/to/marknoria` after building that checkout's assets with `npm run build`.
Only library and reader sources are imported. The manifest omits standalone binaries and desktop dependencies; diagram/sanitizer assets reuse Studio's pinned runtime assets. Editor/virtualizer bundles are copied into `ui/vendor`. No network or sibling checkout is needed at build/runtime.

Studio-specific embedding changes are applied reproducibly by `scripts/marknoria-embedding.mjs` (SHA-256: 3068797bdad5432b3ee4204f32a7823ea07c683e7332068dd5e4c034770f5d17). They add an explicitly editable single-document service, enable its embedded UI, and expose a bounded host state/control bridge. The indexer stores canonical reference destinations in a private binary stream plus compact label/range metadata for on-demand native page resolution; URLs are excluded from the public document index. Native snapshot handles retain the matching immutable index and document identity for bounded backend quote/image lookup. Source budgeting charges recognized data-image URLs separately because cached HTML emits alt text; actual HTML and event limits remain unchanged, with regression checks for oversized ordinary text/code. The default service and comparison remain read only. Parsing and source offsets are unchanged. The adapter also normalizes cross-platform test paths and releases validated Windows target handles before atomic replacement; revision and identity checks still run before replacement.
