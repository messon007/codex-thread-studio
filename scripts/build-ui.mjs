import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const check = process.argv.includes('--check')
const temporary = await mkdtemp(join(tmpdir(), 'studio-typescript-'))
const normalize = (text) => text.replace(/\r\n/g, '\n')
try {
  // Use the stable CLI, not version-specific compiler API internals.
  const result = spawnSync(process.execPath, [join(root, 'node_modules/typescript/bin/tsc'),
    '--project', join(root, 'tsconfig.json'), '--outDir', temporary], { cwd: root, stdio: 'inherit' })
  if (result.error) throw result.error
  if (result.status !== 0) throw new Error('TypeScript compilation failed')
  const sources = (await readdir(join(root, 'ui-src'))).filter((name) => name.endsWith('.mts')).sort()
  const outputs = (await readdir(temporary)).filter((name) => name.endsWith('.mjs')).sort()
  const generated = new Map()
  for (const name of outputs) {
    generated.set(`ui/${name}`, '// Generated from ui-src; run npm run build:ui. Do not edit.\n'
      + normalize(await readFile(join(temporary, name), 'utf8')))
  }
  const inputs = ['package.json', 'package-lock.json', 'tsconfig.json', 'scripts/build-ui.mjs',
    ...sources.map((name) => `ui-src/${name}`)]
  const hashes = []
  for (const path of [...inputs, ...generated.keys()].sort()) {
    const content = generated.get(path) ?? normalize(await readFile(join(root, path), 'utf8'))
    hashes.push(`${path}\t${createHash('sha256').update(content).digest('hex')}`)
  }
  generated.set('ui-src/generated.sha256', hashes.join('\n') + '\n')
  for (const [path, content] of generated) {
    if (check) {
      const current = await readFile(join(root, path), 'utf8').catch(() => '')
      if (normalize(current) !== content) throw new Error(`${path} is stale; run npm run build:ui`)
    } else await writeFile(join(root, path), content)
  }
  console.log(`TypeScript: ${outputs.length} modules ${check ? 'verified' : 'generated'}.`)
} finally {
  await rm(temporary, { recursive: true, force: true })
}
