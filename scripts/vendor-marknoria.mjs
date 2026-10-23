import { execFileSync } from 'node:child_process'
import { readFile, mkdir, writeFile } from 'node:fs/promises'
import { resolve, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createHash } from 'node:crypto'
import { transform } from 'esbuild'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const checkout = resolve(process.argv[2] || '../anything/marknoria')
const revision = '42a9b8deb7c0068f52d4e22f11b444d1a2a0c586'
const commit = execFileSync('git', ['-C', checkout, 'rev-parse', `${revision}^{commit}`], { encoding: 'utf8' }).trim()
const paths = execFileSync('git', ['-C', checkout, 'ls-tree', '-r', '--name-only', commit], { encoding: 'utf8' }).trim().split('\n')
const assets = {
  'mermaid.min.js': 'mermaid.min.js', 'plantuml.js': 'plantuml.js',
  'plantuml-viz-global.js': 'plantuml-viz-global.js', 'd2.js': 'd2.js',
  'purify.mjs': 'purify.es.mjs', 'virtual-core.mjs': 'marknoria-virtual-core.mjs',
  'editor.mjs': 'marknoria-editor.mjs',
}
const embedding = await readFile(resolve(root, 'scripts/marknoria-embedding.mjs'), 'utf8')
const { embedSource } = await import('./marknoria-embedding.mjs')
for (const path of paths.filter(path => path === 'EMBEDDING.md' || path === 'Cargo.toml' || /^(src|web)\//u.test(path) && !/^src\/(main|desktop)\.rs$/u.test(path))) {
  let source = execFileSync('git', ['-C', checkout, 'show', `${commit}:${path}`], { encoding: 'utf8' })
  if (path === 'Cargo.toml') source = source
    .replace('default-run = "marknoria-server"', 'autobins = false')
    .replace('libc = "=0.2.191"', 'libc = "0.2.174"')
    .replace('uuid = { version = "=1.23.5"', 'uuid = { version = "1.23.5"')
    .replace(/\[\[bin\]\][\s\S]*?(?=\[dependencies\])/u, '')
    .replace(/^tao = .*\n|^wry = .*\n/gmu, '')
    .replace(/\n\[profile.release\][\s\S]*/u, '\n')
    .trimEnd() + '\n'
  if (path === 'src/server.rs') for (const [name, destination] of Object.entries(assets)) source = source.replaceAll(`../web/vendor/${name}`, `../../../ui/vendor/${destination}`)
  source = embedSource(path, source)
  const destination = resolve(root, 'crates/marknoria', path)
  await mkdir(dirname(destination), { recursive: true })
  await writeFile(destination, source)
}
const virtual = await readFile(resolve(checkout, 'web/vendor/virtual-core.mjs'))
await writeFile(resolve(root, 'ui/vendor/marknoria-virtual-core.mjs'), virtual)
const editor = await transform(await readFile(resolve(checkout, 'web/vendor/editor.mjs'), 'utf8'), { format: 'esm', target: 'es2022', minify: true, supported: { 'template-literal': false }, legalComments: 'inline' })
await writeFile(resolve(root, 'ui/vendor/marknoria-editor.mjs'), editor.code)
await writeFile(resolve(root, 'ui/vendor/licenses/marknoria-virtual-MIT.txt'), await readFile(resolve(checkout, 'web/vendor/virtual-MIT.txt')))
const checkbox = await readFile(resolve(checkout, 'web/checkbox.css'))
await writeFile(resolve(root, 'ui/vendor/marknoria-checkbox.css'), checkbox)
const checkboxHash = createHash('sha256').update(checkbox).digest('hex')
await writeFile(resolve(root, 'crates/marknoria/UPSTREAM.md'), `# MarkNoria source snapshot\n\nRepository: https://github.com/messon007/marknoria\nRevision: ${commit}\n\nRefresh with \x60node scripts/vendor-marknoria.mjs /path/to/marknoria\x60 after building that checkout's assets with \x60npm run build\x60.\nOnly library and reader sources are imported. The manifest omits standalone binaries and desktop dependencies; diagram/sanitizer assets reuse Studio's pinned runtime assets. Editor/virtualizer bundles are copied into \x60ui/vendor\x60. No network or sibling checkout is needed at build/runtime.\n\nStudio-specific embedding changes are applied reproducibly by \x60scripts/marknoria-embedding.mjs\x60 (SHA-256: ${createHash('sha256').update(embedding).digest('hex')}). They add an explicitly editable single-document service, enable its embedded UI, and expose a bounded host state/control bridge. The default service and comparison remain read only. Parsing, source offsets, and save/conflict logic are unchanged.\n`)
execFileSync('cargo', ['fmt', '--manifest-path', resolve(root, 'Cargo.toml'), '--package', 'marknoria'], { stdio: 'inherit' })
console.log(`Vendored MarkNoria ${commit}`)
await writeFile(resolve(root, 'crates/marknoria/CONTROLS.md'), `# Reader controls\n\nThe host injects \x60ui/vendor/marknoria-checkbox.css\x60, copied from the pinned MarkNoria \x60web/checkbox.css\x60.\n\nSHA-256: \x60${checkboxHash}\x60\n\nThe vendoring command refreshes this stylesheet alongside the source snapshot and built assets. No sibling checkout is required to build or run Studio.\n`)
