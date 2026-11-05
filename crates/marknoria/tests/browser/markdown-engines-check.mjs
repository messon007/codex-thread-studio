// Optional fixed-Git-object comparison. No Studio worktree/build files are read.
import assert from 'node:assert/strict'
import { execFileSync, spawn } from 'node:child_process'
import { createServer } from 'node:http'
import { once } from 'node:events'
import { createHash } from 'node:crypto'
import { mkdtemp, readFile, realpath, mkdir, writeFile, readdir } from 'node:fs/promises'
import { join, resolve } from 'node:path'
const [studio, revision] = process.argv.slice(2)
if (!studio || !/^[0-9a-f]{40}$/.test(revision || ''))
  throw Error('Pass a Studio Git repository and exact 40-character commit')
const blob = (path) =>
  execFileSync('git', ['-C', studio, 'show', `${revision}:${path}`], {
    maxBuffer: 32 * 1024 * 1024
  })
const recorded = JSON.parse(blob('ui/vendor/comrak-assets.json'))
const modules = new Map()
for (const name of ['comrak.js', 'comrak_bg.wasm']) {
  const path = 'ui/vendor/' + name,
    bytes = blob(path)
  assert.equal(createHash('sha256').update(bytes).digest('hex'), recorded[path])
  modules.set('/' + name, bytes)
}
const directory = await mkdtemp(join(await realpath('target'), 'markdown-wasm-comparison-'))
const corpus = JSON.parse(
  execFileSync(resolve('target/release/examples/reader-corpus'), [directory], { encoding: 'utf8' })
)
const original = await readFile(corpus.markdown, 'utf8'),
  large = join(directory, 'large.md')
const source = [
  original,
  ...Array.from({ length: 511 }, () => original.replace(/^\ufeff/, ''))
].join('\r\n\r\n')
await writeFile(large, source)
const cache = join(directory, 'cache')
await mkdir(cache)
const server = createServer((req, res) => {
  res.setHeader('Cache-Control', 'no-store')
  if (req.url === '/worker.mjs') {
    res.setHeader('Content-Type', 'text/javascript')
    res.end(
      `import init,{parse_document} from './comrak.js';self.onmessage=async({data})=>{try{const began=performance.now();const wasm=await init();const loaded=performance.now();const value=parse_document(data);const html=value.take_html();const headings=JSON.parse(value.headings_json());value.free();postMessage({html,headings,loadMs:loaded-began,parseMs:performance.now()-loaded,wasmMemoryBytes:wasm.memory.buffer.byteLength})}catch(error){postMessage({error:String(error)})}}`
    )
    return
  }
  const bytes = modules.get(req.url)
  if (bytes) {
    res.setHeader(
      'Content-Type',
      req.url.endsWith('.wasm') ? 'application/wasm' : 'text/javascript'
    )
    res.end(bytes)
    return
  }
  res.setHeader('Content-Type', 'text/html')
  res.end('<!doctype html><html><body></body></html>')
})
server.listen(0, '127.0.0.1')
await once(server, 'listening')
const child = spawn(resolve('target/release/marknoria-server'), [large], {
  env: { ...process.env, READER_CACHE_DIR: cache },
  stdio: ['ignore', 'pipe', 'pipe']
})
let output = '',
  errors = '',
  browser
child.stdout.on('data', (bytes) => (output += bytes))
child.stderr.on('data', (bytes) => (errors += bytes))
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
try {
  const { chromium } = await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
  browser = await chromium.launch({ executablePath: process.env.STUDIO_CHROMIUM_PATH })
  const page = await browser.newPage()
  await page.goto(`http://127.0.0.1:${server.address().port}/`)
  const wasm = await page.evaluate(async (source) => {
    const samples = []
    for (let i = 0; i < 3; i++) {
      const worker = new Worker('/worker.mjs', { type: 'module' }),
        began = performance.now()
      const result = await new Promise((resolve, reject) => {
        const timer = setTimeout(() => reject(Error('WASM comparison timed out')), 15000)
        worker.onmessage = ({ data }) => {
          clearTimeout(timer)
          data.error ? reject(Error(data.error)) : resolve(data)
        }
        worker.onerror = (event) => {
          clearTimeout(timer)
          reject(Error(event.message))
        }
        worker.postMessage(source)
      }).finally(() => worker.terminate())
      const html = result.html,
        dom = new DOMParser().parseFromString(html, 'text/html')
      if (
        !dom.querySelector('h1')?.textContent.includes('Public 中文😀 corpus') ||
        !dom.querySelector('table td')?.textContent.includes('中文|😀') ||
        !dom.querySelector('details[open] summary') ||
        dom.querySelector('pre a')
      )
        throw Error('Frozen WASM corpus semantics changed')
      samples.push({
        loadMs: result.loadMs,
        parseMs: result.parseMs,
        roundTripMs: performance.now() - began,
        wasmMemoryBytes: result.wasmMemoryBytes,
        htmlBytes: new TextEncoder().encode(html).length,
        headings: result.headings.length,
        outlineFirst: result.headings.slice(0, 2).map(({ label, line }) => ({ label, line }))
      })
    }
    return samples
  }, source)
  const deadline = Date.now() + 30000
  while (!output.includes('http://') && Date.now() < deadline && child.exitCode == null)
    await pause(20)
  assert.match(output, /http:\/\/127\.0\.0\.1:/, errors)
  const url = new URL(output.trim().split('\n')[0]),
    headers = { 'x-reader-token': url.hash.slice(1) }
  let document
  while (Date.now() < deadline) {
    document = await (await fetch(new URL('api/document', url), { headers })).json()
    if (document.state === 'ready') break
    assert.notEqual(document.state, 'error', document.error)
    await pause(20)
  }
  assert.equal(document.state, 'ready')
  headers['x-reader-document'] = document.documentId
  const began = performance.now(),
    response = await fetch(
      new URL(`api/projection?revision=${document.index.revision}&start=0&count=5`, url),
      { headers }
    ),
    first = await response.text()
  assert.equal(response.status, 200)
  assert.match(first, /Public 中文/)
  const report = {
    ok: true,
    studioRevision: revision,
    wasmSourceHashes: {
      js: recorded['ui/vendor/comrak.js'],
      wasm: recorded['ui/vendor/comrak_bg.wasm']
    },
    sourceBytes: Buffer.byteLength(source),
    wasm,
    core: {
      indexMs: document.index.indexMs,
      peakRssKiB: document.index.peakRssKib,
      pages: document.index.pages.length,
      headings: document.index.toc.length,
      outlineFirst: document.index.toc.slice(0, 2).map(({ label, line }) => ({ label, line })),
      totalHtmlBytes: document.index.pages.reduce((sum, page) => sum + page.bytes, 0),
      firstWindowTransportMs: performance.now() - began,
      firstWindowBytes: Buffer.byteLength(first)
    },
    scope:
      'WASM worker vs supervised Rust index; different work/transport, no universal engine winner',
    artifacts: directory
  }
  assert.equal(wasm[0].headings, report.core.headings)
  assert.deepEqual(wasm[0].outlineFirst, report.core.outlineFirst)
  await writeFile(join(directory, 'result.json'), JSON.stringify(report, null, 2) + '\n')
  console.log(JSON.stringify(report))
} finally {
  await browser?.close()
  server.closeAllConnections()
  server.close()
  if (child.exitCode == null && child.signalCode == null) {
    const exited = once(child, 'exit')
    child.kill('SIGINT')
    await exited
  }
  assert.deepEqual(await readdir(cache), [])
}
