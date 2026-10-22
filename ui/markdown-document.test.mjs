import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { createMarkdownDocumentClient } from './markdown-document-client.mjs'
import { parseMarkdownDocument } from './markdown-document-parser.mjs'
import { extractMarkdownOutline } from './document-outline.mjs'
import { markdownParserForSource } from './diagram-source.mjs'
import initialize from './vendor/comrak.js'

await initialize({ module_or_path: readFileSync(new URL('./vendor/comrak_bg.wasm', import.meta.url)) })

test('Comrak asset manifest matches Rust sources, bindings, binary and licenses', () => {
  const manifest = JSON.parse(readFileSync(new URL('./vendor/comrak-assets.json', import.meta.url)))
  for (const [path, expected] of Object.entries(manifest)) {
    const bytes = readFileSync(new URL(`../${path}`, import.meta.url))
    assert.equal(createHash('sha256').update(path.endsWith('.wasm') ? bytes : bytes.toString('utf8').replace(/\r\n/gu, '\n')).digest('hex'), expected, path)
  }
})

test('ordinary documents use Comrak and retain heading labels, IDs and source positions', async () => {
  const source = '# A &amp; **B**\n\n   ## Repeat\n\nText\n\n## Repeat\n\nSetext\n===\n'
  const result = await parseMarkdownDocument(source)
  assert.equal(result.engine, 'comrak-wasm')
  assert.deepEqual(result.outline, extractMarkdownOutline(source))
  assert.match(result.html, /<h1>A &amp; <strong>B<\/strong><\/h1>/u)
})

test('Comrak positions use original UTF-16 offsets and correct CRLF/CR line numbers', async () => {
  for (const ending of ['\r\n', '\r', '\n']) {
    const source = `# \u4e2d\u6587 \ud83d\ude00${ending}${ending}   ## Tail${ending}`
    const result = await parseMarkdownDocument(source)
    assert.equal(result.engine, 'comrak-wasm')
    assert.equal(result.outline[1].target.offset, source.indexOf('   ##'))
    assert.equal(result.outline[1].target.line, 3)
  }
})

test('Comrak keeps tables, tasks, autolinks and fenced diagram languages', async () => {
  const result = await parseMarkdownDocument('| A | B |\n| :- | -: |\n| x | y |\n\n- [x] Done\n\n~~Gone~~\n\nhttps://example.com\n\n```mermaid\ngraph TD; A-->B\n```\n\n```d2\na -> b\n```\n\n```plantuml\nAlice -> Bob\n```')
  assert.equal(result.engine, 'comrak-wasm')
  for (const pattern of [/<table>/u, /checkbox/u, /<del>Gone<\/del>/u, /href="https:\/\/example.com"/u, /language-mermaid/u, /language-d2/u, /language-plantuml/u]) assert.match(result.html, pattern)
})

test('PlantUML explicitly uses its existing parser for nested blocks', async () => {
  for (const source of ['@startuml\nAlice -> Bob\n@enduml', '> @startuml\n> Alice -> Bob\n> @enduml', '- @startuml\n  Alice -> Bob\n  @enduml', '```text\n@startuml\nA -> B\n@enduml\n```']) {
    const parser = markdownParserForSource(source)
    const result = await parseMarkdownDocument(source)
    assert.equal(result.engine, 'marked')
    assert.equal(result.html, parser.parse(source))
    assert.deepEqual(result.outline, extractMarkdownOutline(source, parser.lexer(source)))
  }
})

test('WASM initialization failures propagate without switching parsers', async () => {
  const failure = new Error('WASM unavailable')
  await assert.rejects(parseMarkdownDocument('# Heading', async () => { throw failure }), failure)
})

test('WASM parsing failures propagate and owned documents are still freed', async () => {
  const failure = new Error('Invalid heading metadata')
  let freed = false
  await assert.rejects(parseMarkdownDocument('# Heading', async () => ({
    default: async () => {},
    parse_document: () => ({ take_html: () => '<h1>Heading</h1>', headings_json: () => { throw failure }, free: () => { freed = true } }),
  })), failure)
  assert.equal(freed, true)
})

function workerFixture() {
  const workers = []
  const client = createMarkdownDocumentClient({ createWorker: () => {
    const worker = { onmessage: null, onerror: null, onmessageerror: null, terminated: false, postMessage(value) { this.request = value }, terminate() { this.terminated = true } }
    workers.push(worker)
    return worker
  } })
  return { client, workers }
}

test('worker results terminate the worker and release its WASM heap', async () => {
  const { client, workers } = workerFixture()
  const pending = client.parse('# Doc')
  const result = { html: '<h1>Doc</h1>', outline: [], engine: 'comrak-wasm' }
  workers[0].onmessage({ data: { result } })
  assert.deepEqual(await pending, result)
  assert.equal(workers[0].terminated, true)
  assert.equal(workers[0].onmessage, null)
})

test('new documents and close cancel parsing without queuing behind old work', async () => {
  const { client, workers } = workerFixture()
  const first = client.parse('old')
  const rejectedFirst = assert.rejects(first, { name: 'AbortError' })
  const second = client.parse('new')
  const rejectedSecond = assert.rejects(second, { name: 'AbortError' })
  client.cancel()
  await Promise.all([rejectedFirst, rejectedSecond])
  assert.ok(workers.every(worker => worker.terminated))
})

test('worker load and response failures reject instead of using the UI thread', async () => {
  for (const event of ['error', 'messageerror', 'invalid']) {
    const { client, workers } = workerFixture()
    const pending = client.parse('# Doc')
    if (event === 'error') workers[0].onerror({ message: 'Worker unavailable' })
    else if (event === 'messageerror') workers[0].onmessageerror()
    else workers[0].onmessage({ data: {} })
    await assert.rejects(pending)
    assert.equal(workers[0].terminated, true)
  }
})

test('worker timeout terminates pathological parsing', async () => {
  let terminated = false
  const client = createMarkdownDocumentClient({ timeoutMs: 5, createWorker: () => ({ onmessage: null, onerror: null, onmessageerror: null, postMessage() {}, terminate() { terminated = true } }) })
  await assert.rejects(client.parse('slow'), { message: 'Markdown parsing timed out' })
  assert.equal(terminated, true)
})
