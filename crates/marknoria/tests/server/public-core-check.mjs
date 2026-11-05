// Exercises the public headless example and its self-hosted jq worker.
import assert from 'node:assert/strict'
import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { mkdir, mkdtemp, readFile, readdir, realpath, writeFile } from 'node:fs/promises'
import { join, resolve } from 'node:path'
const base = await realpath('target'), directory = await mkdtemp(join(base, 'public-core-'))
const binary = resolve(process.argv[2] || 'target/debug/examples/reader-host')
const csv = '\ufeffname,value\r\n' + Array.from({ length: 3001 }, (_, n) => `"row ${n} 中文😀",${n}\r\n`).join('')
const jsonl = Array.from({ length: 1501 }, (_, id) => JSON.stringify({ id, text: 'JSONL 中文😀' }) + '\n').join('')
let assertions = 0
for (const [name, source] of [['public.csv', csv], ['public.jsonl', jsonl], ['public.md', '# Heading\n\nPublic 中文😀 text.\n']]) {
  const path = join(directory, name), cache = join(directory, name + '-cache')
  await writeFile(path, source); await mkdir(cache)
  const { stdout } = await promisify(execFile)(binary, [path], { timeout: 45000, maxBuffer: 1024 * 1024, env: { ...process.env, READER_CACHE_DIR: cache, MARKNORIA_CONFIG_DIR: join(directory, 'config') } })
  const records = stdout.trim().split('\n').map(line => JSON.parse(line))
  const metadata = records[0]
  assert.equal(metadata.capabilities.apiVersion, 1)
  assert.equal(metadata.capabilities.edit, false)
  assert.equal(metadata.capabilities.datasetScope, 'complete-input')
  assert.equal(metadata.origin, 'file')
  if (name.endsWith('.csv')) {
    const result = records.find(r => r.totalResultRows != null)
    assert.equal(result.totalResultRows, 3001)
    assert.ok(result.queryPageRows < 3001)
    assertions += 2
  }
  if (name.endsWith('.jsonl')) {
    const result = records.find(r => r.queryType === 'jq')
    assert.equal(result.totalValues, 1501)
    assert.ok(result.resultPages >= 1)
    assertions += 2
  }
  assert.equal(await readFile(path, 'utf8'), source)
  assert.deepEqual(await readdir(cache), [])
  assertions += 6
}
console.log(JSON.stringify({ ok: true, assertions, headless: true, selfHostedJq: true, cleanup: true, artifacts: directory }))
