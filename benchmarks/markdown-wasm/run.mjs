import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { readFile, writeFile, mkdir } from 'node:fs/promises'
import { createServer } from 'node:http'
import { cpus } from 'node:os'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { brotliCompressSync, gzipSync } from 'node:zlib'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..')
const output = resolve(root, 'target/markdown-wasm-results')
const packageDir = resolve(root, 'target/markdown-wasm-pkg')
const binary = resolve(root, 'target/markdown-wasm-build/release/studio-markdown-wasm-bench')
const args = process.argv.slice(2)
const flag = (name, fallback) => args.includes(name) ? args[args.indexOf(name) + 1] : fallback
const samples = Number(flag('--samples', '7'))
const rounds = Number(flag('--rounds', '2'))
const checksOnly = args.includes('--checks-only')
const pipelineOnly = args.includes('--pipeline-only')
const coldOnly = args.includes('--cold-only')
const reuse = checksOnly || pipelineOnly || coldOnly
assert.ok(Number.isInteger(samples) && samples > 0 && Number.isInteger(rounds) && rounds > 0)
await mkdir(output, { recursive: true })
const block = '## Background 中文\n\nA reusable parser should preserve **formatting**, *emphasis*, links and source anchors. SIMD accelerates scanning, not browser layout.\n第二行包含中文字符和 emoji 😀。\n\n'
const repeat = (source, bytes) => source.repeat(Math.ceil(bytes / Buffer.byteLength(source)))
const diagram = '@startuml\nAlice -> Bob: hello\n@enduml\n'
const corpora = [{ name: 'readme', source: await readFile(resolve(root, 'README.md'), 'utf8') }]
if (flag('--file')) corpora.push({ name: 'acceptance-review', source: await readFile(resolve(flag('--file')), 'utf8') })
corpora.push(
  { name: 'prose-5mib', source: repeat(block, 5 * 1024 * 1024) },
  { name: 'tables-5mib', source: repeat('## Requirement 中文 😀\n\n| ID | Status | Description |\n| :--- | ---: | --- |\n| REQ-001 | pass | 中文验收 **bold** and `code` |\n| REQ-002 | pending | [reference](../source.md:42) |\n\n', 5 * 1024 * 1024) },
  { name: 'plantuml-end-1mib', source: repeat(block, 1024 * 1024) + diagram },
  { name: 'plantuml-start-1mib', source: diagram + '\n' + repeat(block, 1024 * 1024) },
)
for (const corpus of corpora) {
  corpus.inputBytes = Buffer.byteLength(corpus.source)
  corpus.sha256 = createHash('sha256').update(corpus.source).digest('hex')
  corpus.path = resolve(output, `${corpus.name}.md`)
  await writeFile(corpus.path, corpus.source)
}
const app = await readFile(resolve(root, 'ui/app.js'), 'utf8')
const renderFunction = app.match(/function renderMarkdownHtml\(source,[\s\S]*?\n\}/u)[0]
const actualRender = `import { diagramKind, markdownParserForSource } from '/diagram-source.mjs';\nimport DOMPurify from '/vendor/purify.es.mjs';\nconst t = value => value;\nconst diagramGeneration = 1;\nexport ${renderFunction}`
const rust = await readFile(resolve(root, 'src-tauri/src/main.rs'), 'utf8')
const csp = rust.match(/"default-src [^"]+"/u)[0].slice(1, -1)
const resources = new Map([
  ['/', { type: 'text/html', body: '<!doctype html><link rel="stylesheet" href="/vendor/github-markdown.css"><link rel="stylesheet" href="/styles.css"><style>body{display:block}main{width:850px;padding:20px}</style><main class="markdown-body"></main>' }],
  ['/actual-render.mjs', { type: 'text/javascript', body: actualRender }],
])
for (const path of ['/diagram-source.mjs', '/document-outline.mjs', '/vendor/marked.esm.js', '/vendor/purify.es.mjs', '/styles.css', '/vendor/github-markdown.css']) {
  resources.set(path, { type: path.endsWith('.css') ? 'text/css' : 'text/javascript', body: await readFile(resolve(root, 'ui', path.slice(1))) })
}
resources.set('/browser.mjs', { type: 'text/javascript', body: await readFile(resolve(root, 'benchmarks/markdown-wasm/browser.mjs')) })
const assets = {}
for (const name of ['studio_markdown_wasm_bench.js', 'studio_markdown_wasm_bench_bg.wasm']) {
  const body = await readFile(resolve(packageDir, name))
  resources.set(`/wasm/${name}`, { type: name.endsWith('.wasm') ? 'application/wasm' : 'text/javascript', body })
  assets[name] = { bytes: body.length, gzipBytes: gzipSync(body).length, brotliBytes: brotliCompressSync(body).length, sha256: createHash('sha256').update(body).digest('hex') }
}
const server = createServer((request, response) => {
  const resource = resources.get(new URL(request.url, 'http://localhost').pathname)
  response.setHeader('Content-Security-Policy', csp)
  response.setHeader('Cache-Control', 'no-store')
  if (!resource) { response.writeHead(404); response.end(); return }
  response.setHeader('Content-Type', resource.type)
  response.end(resource.body)
})
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve))
const origin = `http://127.0.0.1:${server.address().port}`
const { chromium } = await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
const launch = () => chromium.launch({ headless: true, executablePath: process.env.STUDIO_CHROMIUM_PATH })
const browser = await launch()
const native = (path, mode, last = String(samples)) => {
  const result = spawnSync(binary, [path, mode, last], { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 })
  if (result.error) throw result.error
  assert.equal(result.status, 0, result.stderr)
  return result.stdout
}
const report = reuse ? JSON.parse(await readFile(resolve(output, 'results.json'), 'utf8')) : { date: new Date().toISOString(), environment: { cpu: cpus()[0].model, node: process.version, browser: browser.version() }, samplesPerRound: samples, rounds, assets, csp, corpora: [], compatibility: [], memory: [], cold: [] }
if (!pipelineOnly && !coldOnly) report.compatibility = []
if (coldOnly) report.cold = []
if (!reuse || coldOnly) report.coldMarked = []
const save = () => writeFile(resolve(output, 'results.json'), JSON.stringify(report, null, 2))
async function createPage(instance = browser, initialize = true) {
  const context = await instance.newContext({ viewport: { width: 1100, height: 900 } })
  const page = await context.newPage()
  await page.goto(origin)
  const cold = initialize ? await page.evaluate(async () => (await import('/browser.mjs')).initialize()) : null
  return { page, context, cold }
}
try {
  for (const corpus of reuse ? [] : corpora) {
    const entry = { name: corpus.name, inputBytes: corpus.inputBytes, sha256: corpus.sha256, browser: [], native: {} }
    for (let round = 0; round < rounds; round++) {
      const { page, context, cold } = await createPage()
      const client = await context.newCDPSession(page)
      const result = await page.evaluate(async ({ source, samples }) => (await import('/browser.mjs')).measure(source, samples), { source: corpus.source, samples })
      entry.browser.push({ ...result, initialization: cold, heap: await client.send('Runtime.getHeapUsage') })
      await context.close()
      console.error(`${corpus.name}: browser round ${round + 1} complete`)
    }
    for (const mode of ['html', 'document']) entry.native[mode] = JSON.parse(native(corpus.path, mode))
    report.corpora.push(entry)
    await save()
  }
  const fixtures = [
    ['headings', '# 中文 😀\n\n## Repeated\n\n## Repeated\n'],
    ['indented-headings', '  # Indented\n\nText\n\n   ## Tail\n'],
    ['heading-entities', '# A &amp; B\n\n## `code` and *emphasis*\n'],
    ['hardbreaks', 'first\nsecond\n'],
    ['table', '| a | b |\n| :--- | ---: |\n| 1 | 2 |\n'],
    ['tasks-and-strike', '- [x] ~~done~~\n- [ ] waiting\n'],
    ['autolinks', 'https://example.com user@example.com\n'],
    ['reference', '[link][ref]\n\n[ref]: ../README.md:1 "title"\n'],
    ['raw-plantuml', '# Before\n\n@startuml\n# not a heading\nAlice -> Bob\n@enduml\n\n## After\n'],
    ['crlf-plantuml', '# 中文 😀\r\n\r\n@STARTUML\r\nA -> B\r\n@ENDUML\r\n\r\n## After\r\n'],
    ['quoted-plantuml', '> @startuml\n> A -> B\n> @enduml\n'],
    ['list-plantuml', '- @startuml\n  A -> B\n  @enduml\n'],
    ['unfinished', '@startuml\nA -> B'],
    ['fenced-literal', '```text\n@startuml\nA -> B\n@enduml\n```\n'],
    ['diagram-fences', '```plantuml\nA -> B\n```\n\n```mermaid\ngraph TD; A-->B\n```\n\n```d2\na -> b\n```\n'],
    ['html', '<details><summary>Title</summary>\n\n**body**\n\n</details>\n'],
    ['unsafe', '<script>alert(1)</script>\n\n<img src=x onerror="alert(1)">\n\n[bad](javascript:alert%281%29)\n'],
    ['fence-in-diagram', '@startuml\n~~~\n# hidden\n@enduml\n\n## Tail\n'],
  ]
  const actual = corpora.find(corpus => corpus.name === 'acceptance-review')
  if (actual) fixtures.push([actual.name, actual.source])
  const { page, context } = await createPage()
  for (const [name, source] of pipelineOnly || coldOnly ? [] : fixtures) {
    const path = resolve(output, 'fixture.md')
    await writeFile(path, source)
    const nativeJson = native(path, 'document', 'render')
    const result = await page.evaluate(async ({ source, nativeJson }) => (await import('/browser.mjs')).compatibility(source, nativeJson), { source, nativeJson })
    assert.equal(result.unsafe, 0, `${name}: unsafe HTML`)
    assert.equal(result.nativeMatchesWasm, true, `${name}: native/WASM mismatch`)
    report.compatibility.push({ name, ...result })
  }
  await context.close()
  if (actual && !checksOnly && !coldOnly) {
    const { page, context } = await createPage()
    report.pipeline = await page.evaluate(async source => (await import('/browser.mjs')).pipeline(source, 9), actual.source)
    await context.close()
  }
  for (const corpus of reuse ? [] : corpora.filter(corpus => ['acceptance-review', 'tables-5mib'].includes(corpus.name))) {
    for (const engine of ['marked', 'wasm-parts', 'wasm-json']) {
      const { page, context } = await createPage()
      const client = await context.newCDPSession(page)
      await client.send('HeapProfiler.collectGarbage')
      const before = await client.send('Runtime.getHeapUsage')
      const linearMemory = await page.evaluate(async ({ engine, source }) => (await import('/browser.mjs')).memoryProbe(engine, source), { engine, source: corpus.source })
      await client.send('HeapProfiler.collectGarbage')
      report.memory.push({ corpus: corpus.name, engine, before, after: await client.send('Runtime.getHeapUsage'), linearMemory })
      await context.close()
    }
  }
  await browser.close()
  for (let iteration = 0; iteration < (!reuse || coldOnly ? 3 : 0); iteration++) {
    const fresh = await launch()
    try {
      const { page, context, cold } = await createPage(fresh)
      const firstParse = await page.evaluate(async source => (await import('/browser.mjs')).firstParse(source), actual?.source || corpora[0].source)
      const stages = await page.evaluate(async () => {
        const start = performance.now()
        const bytes = await (await fetch('/wasm/studio_markdown_wasm_bench_bg.wasm')).arrayBuffer()
        const fetched = performance.now()
        await WebAssembly.compile(bytes)
        return { fetchMs: fetched - start, cachedCompileMs: performance.now() - fetched }
      })
      report.cold.push({ ...cold, firstParse, ...stages })
      await context.close()
    } finally { await fresh.close() }
    const markedFresh = await launch()
    try {
      const { page, context } = await createPage(markedFresh, false)
      report.coldMarked.push(await page.evaluate(async source => (await import('/browser.mjs')).firstParse(source, 'marked'), actual?.source || corpora[0].source))
      await context.close()
    } finally { await markedFresh.close() }
  }
  await save()
  const median = values => {
    const sorted = [...values].sort((left, right) => left - right)
    return (sorted[Math.floor((sorted.length - 1) / 2)] + sorted[Math.floor(sorted.length / 2)]) / 2
  }
  console.log(JSON.stringify(report.corpora.map(corpus => ({ name: corpus.name, documentMs: Object.fromEntries(['marked', 'wasm-json', 'wasm-parts'].map(engine => [engine, median(corpus.browser.flatMap(round => round.results[engine].document))])), nativeHtmlMs: median(corpus.native.html.samplesMs) })), null, 2))
} finally {
  await browser.close()
  await new Promise(resolve => server.close(resolve))
}
