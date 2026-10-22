import assert from 'node:assert/strict'
import { createServer } from 'node:http'
import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { markdownParserForSource } from '../../ui/diagram-source.mjs'

const root = fileURLToPath(new URL('../../', import.meta.url))
const file = process.argv.indexOf('--file')
let source = file >= 0 ? await readFile(resolve(process.argv[file + 1]), 'utf8') : '# Heading &amp; **bold**\n\n| A | B |\n| :- | -: |\n| x | y |\n\n- [x] Done\n\n```mermaid\ngraph TD; A-->B\n```\n\n```d2\na -> b\n```\n\n<script>alert(1)</script>\n\n<img src="x" onerror="alert(1)">'
const expected = markdownParserForSource(source).parse(source)
const index = (await readFile(resolve(root, 'ui/index.html'), 'utf8')).replace(/<script\b[^>]*>[\s\S]*?<\/script>/gu, '')
const app = `${await readFile(resolve(root, 'ui/app.js'), 'utf8')}\nglobalThis.fixture = { state, openArtifact, renderMarkdownHtml, startTranslationObserver, documentWorkspace, setArtifactView };`
const rust = await readFile(resolve(root, 'src-tauri/src/main.rs'), 'utf8')
const csp = rust.match(/"(default-src 'self'[^"\n]+)"/u)[1]
let failWasm = false
let slowWorker = false
const server = createServer(async (request, response) => {
  const path = new URL(request.url, 'http://localhost').pathname
  response.setHeader('Content-Security-Policy', csp)
  response.setHeader('Cache-Control', 'no-store')
  if (path === '/studio/client-log') { response.writeHead(204); response.end(); return }
  if (path === '/studio/review-file') {
    let body = ''
    for await (const chunk of request) body += chunk
    const oversized = JSON.parse(body).path === 'oversized.md' || Buffer.byteLength(source) > 5 * 1024 * 1024
    response.writeHead(oversized ? 413 : 200, { 'Content-Type': 'application/json' })
    response.end(JSON.stringify(oversized ? { error: { message: 'file exceeds the 5 MiB review limit' } } : { root: '/fixture', path: '/fixture/document.md', relativePath: 'document.md', content: source, size: Buffer.byteLength(source), lineCount: source.split('\n').length }))
    return
  }
  if (failWasm && path.endsWith('.wasm')) { response.writeHead(404); response.end(); return }
  try {
    let body = path === '/' ? index : path === '/app.js' ? app : await readFile(resolve(root, 'ui', path.slice(1)))
    if (slowWorker && path === '/markdown-document-worker.mjs') body = `await new Promise(resolve => setTimeout(resolve, 800));\n${body}`
    response.setHeader('Content-Type', path.endsWith('.wasm') ? 'application/wasm' : path.endsWith('.css') ? 'text/css' : /\.(?:js|mjs)$/u.test(path) ? 'text/javascript' : 'text/html')
    response.end(body)
  } catch { response.writeHead(404); response.end() }
})
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve))
const { chromium } = await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
const browser = await chromium.launch({ headless: true, executablePath: process.env.STUDIO_CHROMIUM_PATH })
try {
  const page = await browser.newPage({ viewport: { width: 1600, height: 1000 } })
  const errors = []
  page.on('pageerror', error => errors.push(error.message))
  await page.goto(`http://127.0.0.1:${server.address().port}/`)
  await page.evaluate(async () => {
    window.__CODEX_THREAD_STUDIO_GATEWAY__ = { token: 'fixture', hostPlatform: 'linux', remote: true }
    await import('/app.js')
    const { state, startTranslationObserver, documentWorkspace } = window.fixture
    state.selectedId = 'fixture'
    state.threads = [{ id: 'fixture', cwd: '/fixture' }]
    document.querySelector('#empty-workspace').classList.add('hidden')
    document.querySelector('#native-workspace').classList.remove('hidden')
    startTranslationObserver()
    documentWorkspace.bind()
  })
  const result = await page.evaluate(async expected => {
    const { openArtifact, state, renderMarkdownHtml } = window.fixture
    let last = performance.now()
    let maxGap = 0
    const heartbeat = setInterval(() => { const now = performance.now(); maxGap = Math.max(maxGap, now - last); last = now }, 20)
    const start = performance.now()
    await openArtifact({ path: 'document.md' })
    await new Promise(requestAnimationFrame)
    await new Promise(requestAnimationFrame)
    clearInterval(heartbeat)
    const elapsedMs = performance.now() - start
    const digest = async value => [...new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(JSON.stringify(value))))].map(byte => byte.toString(16).padStart(2, '0')).join('')
    const signature = async element => {
      const text = node => node.textContent.replace(/\s+/gu, ' ').trim()
      const nodes = selector => [...element.querySelectorAll(selector)]
      return {
        tables: nodes('table').length,
        cells: await digest(nodes('th,td').map(node => [node.tagName, text(node), node.getAttribute('align')])),
        headings: await digest(nodes('h1,h2,h3,h4,h5,h6').map(node => [node.tagName, text(node)])),
        links: await digest(nodes('a').map(node => [node.getAttribute('href'), text(node)])),
        code: await digest(nodes('pre code').map(node => [node.className, node.textContent.trimEnd()])),
        unsafe: nodes('script,iframe,object,embed').length + nodes('*').filter(node => [...node.attributes].some(attribute => /^on/iu.test(attribute.name) || /javascript:/iu.test(attribute.value))).length,
      }
    }
    const reference = document.createElement('div')
    reference.innerHTML = renderMarkdownHtml('', { dirtyHtml: expected, documentImages: true })
    return { elapsedMs, maxGap, outlineItems: state.artifact.markdownDocument.result.outline.length, actual: await signature(document.querySelector('#artifact-content')), expected: await signature(reference), metrics: window.codexThreadStudioPerformance.snapshot().records }
  }, expected)
  for (const key of Object.keys(result.expected)) assert.equal(result.actual[key], result.expected[key], `Rendered signature differs: ${key}`)
  assert.equal(result.actual.unsafe, 0)
  assert.equal(result.metrics.at(-1).engine, /@startuml\b/i.test(source) ? 'marked' : 'comrak-wasm')
  await page.evaluate(async () => {
    const { state, documentWorkspace } = window.fixture
    const heading = state.artifact.markdownDocument.result.outline[0]
    if (heading) documentWorkspace.jumpToLine(heading.target.line)
    await new Promise(requestAnimationFrame)
    await new Promise(requestAnimationFrame)
  })
  assert.equal(await page.evaluate(() => window.fixture.state.artifactView), 'preview')
  await page.evaluate(() => window.fixture.setArtifactView('source'))
  assert.equal(await page.locator('#artifact-content .artifact-source').count(), 1)
  await page.evaluate(() => window.fixture.setArtifactView('preview'))
  await page.waitForFunction(() => document.querySelector('#artifact-content').classList.contains('markdown-body'))
  slowWorker = true
  source = '# Cancel this document'
  await page.evaluate(() => { window.openingDocument = window.fixture.openArtifact({ path: 'cancel.md' }) })
  await page.locator('#composer-input').fill('Still responsive')
  await page.locator('#close-artifact').click()
  await page.evaluate(() => window.openingDocument)
  assert.equal(await page.evaluate(() => window.fixture.state.artifact), null)
  slowWorker = false
  failWasm = true
  source = '# Unavailable parser'
  await page.evaluate(() => window.fixture.openArtifact({ path: 'unavailable.md' }))
  const parserFailure = await page.evaluate(() => ({
    error: window.fixture.state.artifact.error,
    loading: window.fixture.state.artifact.loading,
    hasDocument: Boolean(window.fixture.state.artifact.markdownDocument),
    errorVisible: document.querySelector('#artifact-error').getBoundingClientRect().height > 0,
  }))
  assert.ok(parserFailure.error)
  assert.equal(parserFailure.loading, false)
  assert.equal(parserFailure.hasDocument, false)
  assert.equal(parserFailure.errorVisible, true)
  const rejection = await page.evaluate(async () => {
    try { await window.fixture.openArtifact({ path: 'oversized.md' }) } catch (error) { return { error: error.message, loading: window.fixture.state.artifact.loading } }
  })
  assert.deepEqual(rejection, { error: 'file exceeds the 5 MiB review limit', loading: false })
  assert.deepEqual(errors, [])
  console.log(JSON.stringify({ elapsedMs: result.elapsedMs, maxMainThreadGapMs: result.maxGap, tables: result.actual.tables, outlineItems: result.outlineItems, metrics: result.metrics, renderedSignaturesMatch: true, cancellation: true, wasmFailureReported: true, oversizedRejection: true }, null, 2))
} finally {
  await browser.close()
  await new Promise(resolve => server.close(resolve))
}
