import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('../', import.meta.url))
const manifest = 'crates/markdown-wasm/Cargo.toml'
const target = resolve(root, 'target/markdown-wasm-production')
const output = resolve(root, 'target/markdown-wasm-vendor')
const run = (command, args) => {
  const result = spawnSync(command, args, { cwd: root, env: { ...process.env, CARGO_TARGET_DIR: target }, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 })
  if (result.status !== 0) throw result.error || new Error(result.stderr || `${command} failed`)
  return result.stdout
}
run('cargo', ['build', '--manifest-path', manifest, '--release', '--locked', '--target', 'wasm32-unknown-unknown', '--lib', ...process.argv.includes('--offline') ? ['--offline'] : []])
const bindgen = process.env.WASM_BINDGEN || 'wasm-bindgen'
if (!run(bindgen, ['--version']).includes('0.2.126')) throw new Error('wasm-bindgen 0.2.126 is required')
run(bindgen, ['--target', 'web', '--out-name', 'comrak', '--out-dir', output, resolve(target, 'wasm32-unknown-unknown/release/studio_markdown_wasm.wasm')])
await mkdir(resolve(root, 'ui/vendor/licenses'), { recursive: true })
for (const name of ['comrak.js', 'comrak_bg.wasm']) await writeFile(resolve(root, 'ui/vendor', name), await readFile(resolve(output, name)))
const metadata = JSON.parse(run('cargo', ['metadata', '--manifest-path', manifest, '--locked', '--offline', '--format-version', '1', '--filter-platform', 'wasm32-unknown-unknown']))
const notices = []
for (const entry of metadata.packages.filter(entry => entry.source).sort((left, right) => left.name.localeCompare(right.name))) {
  notices.push(`${entry.name} ${entry.version} (${entry.license || 'see license text'})`)
  const directory = resolve(entry.manifest_path, '..')
  const files = (await readdir(directory)).filter(name => /^(?:LICENSE|COPYING|UNLICENSE)/iu.test(name)).sort()
  for (const file of files) {
    const text = await readFile(resolve(directory, file), 'utf8').catch(() => '')
    if (text) notices.push(`${file}\n${text.trimEnd()}`)
  }
}
await writeFile(resolve(root, 'ui/vendor/licenses/comrak-wasm-NOTICES.txt'), `${notices.join('\n\n')}\n`)
const paths = [manifest, 'crates/markdown-wasm/Cargo.lock', 'crates/markdown-wasm/src/lib.rs', 'scripts/vendor-comrak.mjs', 'ui/vendor/comrak.js', 'ui/vendor/comrak_bg.wasm', 'ui/vendor/licenses/comrak-wasm-NOTICES.txt']
const hashes = Object.fromEntries(await Promise.all(paths.map(async path => {
  const bytes = await readFile(resolve(root, path))
  return [path, createHash('sha256').update(path.endsWith('.wasm') ? bytes : bytes.toString('utf8').replace(/\r\n/gu, '\n')).digest('hex')]
})))
await writeFile(resolve(root, 'ui/vendor/comrak-assets.json'), `${JSON.stringify(hashes, null, 2)}\n`)
console.log('Vendored Comrak 0.57.0 WASM and dependency licenses')
