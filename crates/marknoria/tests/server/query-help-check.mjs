// Execute the examples shown in the UI against the shipped engines, with real inputs.
import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { once } from 'node:events'
import { mkdtemp, mkdir, writeFile, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { sqlExamples, jqExamples, queryError } from '../../web/query-help.mjs'
const binary = resolve(process.argv[2] || 'target/debug/marknoria-server')
const dir = await mkdtemp(join(tmpdir(), 'marknoria-query-help-'))
let assertions = 0
const equal = (actual, expected) => { assert.deepEqual(actual, expected); assertions++ }
async function withInput(name, source, test) {
  const path = join(dir, name), cache = join(dir, name + '-cache')
  await writeFile(path, source); await mkdir(cache)
  const child = spawn(binary, [path], { env: { ...process.env, READER_CACHE_DIR: cache, MARKNORIA_CONFIG_DIR: join(dir, 'config') }, stdio: ['ignore', 'pipe', 'pipe'] })
  let output = '', errors = ''
  child.stdout.on('data', b => { output += b }); child.stderr.on('data', b => { errors += b })
  try {
    const deadline = Date.now() + 20000
    while (!output.includes('http://') && Date.now() < deadline && child.exitCode == null) await new Promise(r => setTimeout(r, 25))
    assert.match(output, /http:\/\/127\.0\.0\.1:/, errors)
    const url = new URL(output.trim().split('\n')[0])
    const api = async (path, body) => {
      const response = await fetch(new URL(path, url), { headers: { 'x-reader-token': url.hash.slice(1), 'Content-Type': 'application/json' }, ...(body ? { method: 'POST', body: JSON.stringify(body) } : {}) })
      if (!response.ok) throw Error(await response.text())
      return response.json()
    }
    let document
    do {
      document = await api('/api/document')
      assert.notEqual(document.state, 'error', document.error)
      if (document.state !== 'ready') await new Promise(r => setTimeout(r, 25))
    } while (document.state !== 'ready' && Date.now() < deadline)
    equal(document.state, 'ready')
    await test(api, document.index.revision)
  } finally {
    if (child.exitCode == null && child.signalCode == null) { const exit = once(child, 'exit'); child.kill('SIGINT'); await exit }
    equal(await readFile(path, 'utf8'), source)
  }
}
try {
  for (const [name, csv, expectedNames] of [
    ['one.csv', 'Only\ntext\nfirst\nsecond\n100%\n', ['Only']],
    ['named.csv', '"quote""列",Value,Name,Name\ntext,12,x,y\nfirst,2,x,y\nsecond,20,x,y\n100%,5,x,y\n', ['quote"列', 'Value', 'Name', 'Name_2']]
  ]) await withInput(name, csv, async (api, revision) => {
    const schema = await api('/api/table-query/schema?table=0')
    equal(schema.columns, expectedNames)
    const query = sql => api('/api/table-query', { revision, table: 0, sql })
    for (const [, sql] of sqlExamples(schema.columns)) {
      const result = await query(sql)
      equal(result.kind, 'tableQuery')
      await api('/api/table-query/close', { id: result.id })
    }
    equal((await query("SELECT c1 FROM t WHERE c1 LIKE '%!%%' ESCAPE '!'")).rows, [['100%']])
    equal((await query("SELECT 'A' LIKE 'a', 'Æ' LIKE 'æ', instr('A', 'a')")).rows, [['1', '0', '0']])
    equal((await query("SELECT c1 FROM t WHERE c1 IN ('first', 'second')")).totalRows, 2)
    await assert.rejects(query('SELECT * FROM t WHERE "DCDC-1297522" IN c1'), /no such table: c1/); assertions++
    await assert.rejects(query("SELECT '中',\n )"), /line 2, column 2/); assertions++
    equal((await query('SELECT count(*) FROM t')).rows, [['4']])
  })
  const object = { name: 'A', value: 2, active: true, level: 'error', 'field name': 'text' }
  for (const [name, source, indexes, slurp] of [
    ['object.json', JSON.stringify(object), [0, 1, 2, 4], false],
    ['array.json', JSON.stringify([object, { ...object, value: 3, active: false }]), [3, 5, 6], false],
    ['records.jsonl', [object, { ...object, value: 3, level: 'info' }].map(JSON.stringify).join('\n'), [7], false],
    ['slurp.ndjson', [object, { ...object, value: 3 }].map(JSON.stringify).join('\n'), [8, 9, 10], true]
  ]) await withInput(name, source, async (api, revision) => {
    const query = (filter, asArray = slurp) => api('/api/jq-query', { revision, filter, slurp: asArray })
    for (const index of indexes) {
      const result = await query(jqExamples[index][1])
      equal(result.kind, 'jqQuery')
      assert.ok(result.totalValues > 0); assertions++
      if (index === 8) equal(result.text.trim(), '5')
      if (index === 10) equal(result.text.trim(), '2')
      await api('/api/jq-query', { revision, id: result.id, close: true })
    }
    await assert.rejects(query('. |\n missing_filter'), /line 2, column 2: undefined filter missing_filter\/0/); assertions++
    equal((await query('type', true)).text.trim(), '"array"')
    if (name === 'array.json') equal((await query('length', true)).text.trim(), '1')
  })
  equal(queryError(Error('Reader request failed (400): no such table: c2'), 'SQL').includes('Reader request failed'), false)
  assert.match(queryError(Error('HTTP 400: no such table: c2'), 'SQL'), /IN expects a list/); assertions++
  console.log(JSON.stringify({ ok: true, assertions, sqlExamples: sqlExamples(['one']).length, jqExamples: jqExamples.length, engines: 'SQLite and embedded jaq', sourceUnchanged: true }))
} finally { await rm(dir, { recursive: true, force: true }) }
