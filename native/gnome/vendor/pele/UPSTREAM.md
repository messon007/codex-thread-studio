# Pele

`pele.js` is the unmodified standalone bundle from the MIT-licensed npm package
`pele@1.9.0`, maintained at <https://github.com/obsidianmd/pele>.
It is evaluated inside QuickJS on a bounded Rust worker, without DOM, network,
filesystem, Node.js, or browser APIs.

- npm archive SHA-256: `d9dd5bc81e8d4d5747a2af07d54e2b8b9e897bff49cc4af7612d219c3d70ba33`
- bundle SHA-256: `323c16de7844d4b609743f70449ebce041a531e4f48f23b1e9e7ec9d3b917287`

The accompanying LICENSE is copied unchanged from that package. To reproduce,
download `https://registry.npmjs.org/pele/-/pele-1.9.0.tgz`, verify the archive,
then copy `package/dist/pele.global.js` and `package/LICENSE`.
