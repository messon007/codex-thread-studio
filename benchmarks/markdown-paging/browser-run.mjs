import assert from 'node:assert/strict'
import { execFile } from 'node:child_process'
import { createServer } from 'node:http'
import { readFile, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { promisify } from 'node:util'

const root = fileURLToPath(new URL('../../', import.meta.url))
const directory = resolve(process.argv[2])
const app = await readFile(resolve(root, 'ui/app.js'), 'utf8')
const render = app.match(/function renderMarkdownHtml\(source,[\s\S]*?\n\}/u)[0]
const actualRender = `import { diagramKind, markdownParserForSource } from '/diagram-source.mjs';\nimport DOMPurify from '/vendor/purify.es.mjs';\nconst t = value => value; const diagramGeneration = 1;\nexport ${render}`
const rust = await readFile(resolve(root, 'src-tauri/src/main.rs'), 'utf8')
const csp = rust.match(/"(default-src 'self'[^"\n]+)"/u)[1]
const config = { rounds: Number(process.env.ROUNDS || 3) }
const resources = new Map([
  ['/', { type: 'text/html', body: '<!doctype html><link rel="stylesheet" href="/vendor/github-markdown.css"><link rel="stylesheet" href="/styles.css"><style>body{display:flex;gap:16px;padding:10px}aside{width:200px;overflow:auto}aside button{display:block}main{width:950px;height:850px;flex:none!important}input{width:120px}</style><input aria-label="Heartbeat"><aside></aside><main class="artifact-content markdown-body"></main>' }],
  ['/actual-render.mjs', { type: 'text/javascript', body: actualRender }],
  ['/config.json', { type: 'application/json', body: JSON.stringify(config) }],
  ['/browser.mjs', { type: 'text/javascript', body: await readFile(resolve(root, 'benchmarks/markdown-paging/browser.mjs')) }],
])
for (const path of ['/diagram-source.mjs', '/document-outline.mjs', '/vendor/marked.esm.js', '/vendor/purify.es.mjs', '/styles.css', '/vendor/github-markdown.css']) {
  resources.set(path, { type: path.endsWith('.css') ? 'text/css' : 'text/javascript', body: await readFile(resolve(root, 'ui', path.slice(1))) })
}
for (const name of ['initial', 'jump-1', 'jump-2', 'jump-3']) resources.set(`/payload/${name}.json`, { type: 'application/json', body: await readFile(resolve(directory, `${name}.json`)) })
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
try {
  if (process.argv.includes('--webkit')) {
    const { stdout } = await promisify(execFile)('/usr/bin/python3', [resolve(root, 'benchmarks/markdown-paging/webkit.py'), origin], { timeout: 120_000, maxBuffer: 4 * 1024 * 1024 })
    const report = JSON.parse(stdout)
    await writeFile(resolve(directory, 'webkit.json'), JSON.stringify(report, null, 2))
    console.log(JSON.stringify({ directory, engine: 'webkit', samples: report.samples.length }))
  } else {
    const { chromium } = await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
    const browser = await chromium.launch({ headless: true, executablePath: process.env.STUDIO_CHROMIUM_PATH })
    try {
      const page = await browser.newPage({ viewport: { width: 1400, height: 1000 } })
      const errors = []
      page.on('pageerror', error => errors.push(error.message))
      await page.goto(origin)
      const report = await page.evaluate(async () => (await import('/browser.mjs')).measure(await (await fetch('/config.json')).json()))
      assert.deepEqual(errors, [])
      const client = await page.context().newCDPSession(page)
      await client.send('HeapProfiler.collectGarbage')
      const retainedJsHeap = await client.send('Runtime.getHeapUsage')
      await writeFile(resolve(directory, 'chromium.json'), JSON.stringify({ browserVersion: browser.version(), retainedJsHeap, ...report }, null, 2))
      console.log(JSON.stringify({ directory, engine: 'chromium', samples: report.samples.length }))
    } finally { await browser.close() }
  }
} finally { await new Promise(resolve => server.close(resolve)) }
