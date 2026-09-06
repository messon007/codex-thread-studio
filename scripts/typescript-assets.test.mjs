import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile, readdir, mkdtemp, mkdir, cp, symlink, writeFile, rm } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { spawnSync } from 'node:child_process'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = new URL('../', import.meta.url)
test('TypeScript manifest covers every migrated source and unchanged embedded module URL', async () => {
  const manifest = await readFile(new URL('ui-src/generated.sha256', root), 'utf8')
  const entries = new Map(manifest.trim().split(/\r?\n/).map((line) => line.split('\t')))
  const routes = await readFile(new URL('src-tauri/src/main.rs', root), 'utf8')
  for (const name of await readdir(new URL('ui-src/', root))) {
    if (!name.endsWith('.mts')) continue
    assert.ok(entries.has(`ui-src/${name}`), name)
    if (name.endsWith('.d.mts')) continue
    const output = name.replace(/\.mts$/, '.mjs')
    assert.ok(entries.has(`ui/${output}`), output)
    assert.ok(routes.includes(`"/${output}"`), `Missing route for ${output}`)
    assert.ok(routes.includes(`../../ui/${output}`), `Missing embedded asset ${output}`)
  }
  for (const [path, hash] of entries) {
    const content = (await readFile(new URL(path, root), 'utf8')).replace(/\r\n/g, '\n')
    assert.equal(createHash('sha256').update(content).digest('hex'), hash, path)
  }
})

test('asset check accepts CRLF and rejects stale source/output without rewriting files', async () => {
  const temporary = await mkdtemp(join(tmpdir(), 'studio-ts-check-'))
  try {
    await mkdir(join(temporary, 'scripts'))
    await mkdir(join(temporary, 'ui'))
    for (const path of ['ui-src', 'package.json', 'package-lock.json', 'tsconfig.json', 'scripts/build-ui.mjs']) {
      await cp(new URL(path, root), join(temporary, path), { recursive: true })
    }
    await symlink(fileURLToPath(new URL('node_modules', root)), join(temporary, 'node_modules'), 'junction')
    const run = (...args) => spawnSync(process.execPath, [join(temporary, 'scripts/build-ui.mjs'), ...args], { encoding: 'utf8' })
    assert.equal(run().status, 0)
    const outputPath = join(temporary, 'ui/model-revision.mjs')
    const output = await readFile(outputPath, 'utf8')
    await writeFile(outputPath, output.replace(/\n/g, '\r\n'))
    assert.equal(run('--check').status, 0, 'Windows line endings are equivalent')
    await writeFile(outputPath, output + '// stale\n')
    const staleOutput = run('--check')
    assert.notEqual(staleOutput.status, 0)
    assert.match(staleOutput.stderr, /model-revision\.mjs is stale/)
    assert.equal(await readFile(outputPath, 'utf8'), output + '// stale\n')
    await writeFile(outputPath, output)
    const sourcePath = join(temporary, 'ui-src/model-revision.mts')
    await writeFile(sourcePath, await readFile(sourcePath, 'utf8') + '\n// new source revision\n')
    assert.notEqual(run('--check').status, 0)
    assert.equal(await readFile(outputPath, 'utf8'), output)
  } finally {
    await rm(temporary, { recursive: true, force: true })
  }
})
