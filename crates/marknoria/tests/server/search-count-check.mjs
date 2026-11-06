import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { once } from 'node:events'
import { mkdir, mkdtemp, writeFile, readFile, readdir, realpath } from 'node:fs/promises'
import { join, resolve } from 'node:path'

const base = await realpath('target')
await mkdir(join(base, 'search-count-checks'), { recursive: true })
const directory = await mkdtemp(join(base, 'search-count-checks/run-'))
const root = join(directory, 'files'), cache = join(directory, 'cache')
await mkdir(root); await mkdir(cache)
const source = '\ufeff' + '中文😀needle '.repeat(250) + '\r\n'
const sourceBytes = Buffer.from(source)
await writeFile(join(root, 'matches.txt'), source)
await writeFile(join(root, 'other.txt'), 'needle\nneedle\nneedle\n')
const server = spawn(resolve(process.argv[2] || 'target/release/marknoria-server'), [join(root, 'matches.txt'), '--root', root], {
  env: { ...process.env, READER_CACHE_DIR: cache, MARKNORIA_CONFIG_DIR: join(directory, 'config') },
  stdio: ['ignore', 'pipe', 'pipe']
})
let output = '', errors = ''
server.stdout.on('data', chunk => output += chunk)
server.stderr.on('data', chunk => errors += chunk)
const pause = () => new Promise(resolve => setTimeout(resolve, 25))
try {
  for (let attempt = 0; attempt < 800 && !output.includes('http://') && server.exitCode == null; attempt++) await pause()
  assert.match(output, /http:\/\/127\.0\.0\.1:/, errors)
  const url = new URL(output.trim().split('\n')[0])
  const headers = { 'x-reader-token': url.hash.slice(1), 'Content-Type': 'application/json' }
  const response = (path, options = {}) => fetch(new URL(path, url), { ...options, headers })
  const api = async (path, options = {}) => {
    const result = await response(path, options)
    assert.equal(result.status, 200, await result.clone().text())
    return result.json()
  }
  const ready = async (prefix = '/') => {
    for (let attempt = 0; attempt < 800; attempt++) {
      const value = await api(prefix + 'api/document')
      if (value.state === 'ready') return value
      assert.notEqual(value.state, 'error', value.error)
      await pause()
    }
    throw Error('Document did not become ready')
  }
  const document = await ready()
  const revision = document.index.revision
  const path = offset => `/api/search?query=needle&offset=${offset}&revision=${revision}`
  const legacy = await api('/api/search?query=needle')
  assert.equal(legacy.matches.length, 100)
  assert.equal(legacy.limit, 100)
  assert.equal(legacy.total, undefined)
  assert.equal(legacy.matches[0].occurrence, undefined)
  const started = performance.now()
  const first = await api(path(0))
  const firstWindowMs = performance.now() - started
  assert.equal(first.revision, revision)
  assert.equal(first.total, 250)
  assert.equal(first.offset, 0)
  assert.equal(first.matches.length, 100)
  assert.equal(first.limit, 100)
  assert.equal(first.complete, false)
  const second = await api(path(100)), final = await api(path(200))
  for (const [offset, window] of [[0, first], [100, second], [200, final]]) {
    assert.equal(window.total, 250)
    assert.equal(window.offset, offset)
    assert.equal(window.matches.length, Math.min(100, 250 - offset))
    for (const [index, match] of window.matches.entries()) {
      assert.equal(match.occurrence, offset + index)
      assert.equal(match.line, 1)
      assert.equal(sourceBytes.subarray(match.start, match.end).toString(), 'needle')
    }
  }
  assert.equal(second.complete, false)
  assert.equal(final.complete, true)
  const starts = [...first.matches, ...second.matches, ...final.matches].map(match => match.start)
  assert.equal(new Set(starts).size, 250)
  assert.deepEqual(starts, [...starts].sort((left, right) => left - right))
  for (const offset of [250, Number.MAX_SAFE_INTEGER]) {
    const empty = await api(path(offset))
    assert.equal(empty.total, 250)
    assert.deepEqual(empty.matches, [])
    assert.equal(empty.complete, true)
  }
  const absent = await api('/api/search?query=absent&offset=0')
  assert.equal(absent.total, 0); assert.equal(absent.complete, true); assert.deepEqual(absent.matches, [])
  const strictCase = await api('/api/search?query=NEEDLE&offset=0&matchCase=true')
  assert.equal(strictCase.total, 0); assert.equal(strictCase.matchCase, true)
  const insensitive = await api('/api/search?query=NEEDLE&offset=100&matchCase=false')
  assert.equal(insensitive.total, 250); assert.equal(insensitive.matches.length, 100)
  assert.equal(insensitive.matchCase, false); assert.equal(insensitive.mode, 'case-insensitive-source-lines')
  for (const match of insensitive.matches) assert.equal(sourceBytes.subarray(match.start, match.end).toString(), 'needle')
  const legacyInsensitive = await api('/api/search?query=NEEDLE&matchCase=false')
  assert.equal(legacyInsensitive.matches.length, 100); assert.equal(legacyInsensitive.matchCase, false)
  for (const value of ['null', '1', 'FALSE', '']) assert.equal((await response('/api/search?query=needle&matchCase=' + value)).status, 400)
  assert.equal((await response('/api/search?query=needle&unknown=true')).status, 400)
  assert.equal((await response('/api/search?query=needle&offset=0&revision=stale')).status, 409)
  assert.equal((await response('/api/search?query=needle&offset=-1')).status, 400)
  assert.equal((await response('/api/search?query=&offset=0')).status, 400)
  assert.equal((await response('/api/search?query=' + encodeURIComponent('中'.repeat(86)) + '&offset=0')).status, 400)
  assert.equal((await fetch(new URL(path(0), url))).status, 401)
  const leftPrefix = `/pane/${document.documentId}/left/`
  const left = await api(leftPrefix + 'api/search?query=needle&offset=200&revision=' + revision)
  assert.equal(left.total, 250); assert.equal(left.matches[0].occurrence, 200)
  const opened = await api('/api/panes', { method: 'POST', body: JSON.stringify({ side: 'right', path: 'other.txt' }) })
  const rightPrefix = `/pane/${opened.documentId}/right/`
  const other = await ready(rightPrefix)
  const right = await api(rightPrefix + 'api/search?query=needle&offset=0&revision=' + other.index.revision)
  assert.equal(right.total, 3); assert.equal(right.matches.length, 3)
  assert.deepEqual(right.matches.map(match => match.occurrence), [0, 0, 0])
  assert.equal((await api(path(0))).total, 250)
  console.log(JSON.stringify({ ok: true, total: 250, windowLimit: 100, firstWindowMs, checks: ['legacy-first-100', 'true-total', 'same-line-window-ordinal', 'original-utf8-bytes', 'final-and-empty-windows', 'revision-409', 'input-and-auth-validation', 'pane-isolation'], artifacts: directory }))
} finally {
  if (server.exitCode == null && server.signalCode == null) { const exited = once(server, 'exit'); server.kill('SIGINT'); await exited }
  assert.equal(await readFile(join(root, 'matches.txt'), 'utf8'), source)
  assert.deepEqual(await readdir(cache), [])
}
