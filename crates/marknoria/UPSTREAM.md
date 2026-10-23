# MarkNoria source snapshot

Repository: https://github.com/messon007/marknoria
Revision: 42a9b8deb7c0068f52d4e22f11b444d1a2a0c586

Refresh with `node scripts/vendor-marknoria.mjs /path/to/marknoria` after building that checkout's assets with `npm run build`.
Only library and reader sources are imported. The manifest omits standalone binaries and desktop dependencies; diagram/sanitizer assets reuse Studio's pinned runtime assets. Editor/virtualizer bundles are copied into `ui/vendor`. No network or sibling checkout is needed at build/runtime.

Studio-specific embedding changes are applied reproducibly by `scripts/marknoria-embedding.mjs` (SHA-256: f4a87d063c4d976beda93243be4826745f1eff65d82272e42a4dcb92e1d03af4). They add an explicitly editable single-document service, enable its embedded UI, and expose a bounded host state/control bridge. The default service and comparison remain read only. Parsing, source offsets, and save/conflict logic are unchanged.
