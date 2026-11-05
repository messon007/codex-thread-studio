// Same synthetic corpus as the ordinary component, through the actual Rust ZIP service.
import assert from 'node:assert/strict'
import { spawn, execFileSync } from 'node:child_process'
import { once } from 'node:events'
import { createServer } from 'node:http'
import { mkdir, mkdtemp, readFile, readdir, realpath } from 'node:fs/promises'
import { join, resolve } from 'node:path'

const dir = await mkdtemp(join(await realpath('target'), 'epub-progress-corpus-'))
const generated = JSON.parse(execFileSync('python3', ['tests/browser/epub_fixtures.py', dir, '--corpus'], { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 }))
const cache = join(dir, 'cache'); await mkdir(cache)
const parent = createServer(async (request, response) => {
  if (['/host-client.mjs', '/reader-bridge.mjs'].includes(request.url)) {
    response.setHeader('Content-Type', 'text/javascript'); response.end(await readFile('web' + request.url)); return
  }
  response.setHeader('Content-Type', 'text/html'); response.end('<!doctype html><html><body></body></html>')
})
parent.listen(0, '127.0.0.1'); await once(parent, 'listening')
const parentOrigin = `http://127.0.0.1:${parent.address().port}`
const child = spawn(resolve(process.argv[2] || 'target/release/examples/embedded-host'), [generated.corpusBook, `--parent=${parentOrigin}`], {
  env: { ...process.env, READER_CACHE_DIR: cache, MARKNORIA_POPPLER_DIR: join(dir, 'no-poppler'), READER_CHROMIUM: join(dir, 'no-backend-chromium') }, stdio: ['pipe', 'pipe', 'pipe']
})
let output = '', errors = '', browser, peakHostRssKiB = 0, sampling = false
child.stdout.on('data', b => output += b); child.stderr.on('data', b => errors += b)
// Linux host RSS only. This deliberately does not label JS heap as renderer RSS.
const sampler = setInterval(async () => {
  if (sampling) return
  sampling = true
  try {
    const status = await readFile(`/proc/${child.pid}/status`, 'utf8')
    peakHostRssKiB = Math.max(peakHostRssKiB, Number(status.match(/^VmRSS:\s+(\d+)/m)?.[1] || 0))
  } catch {} finally { sampling = false }
}, 40)
const pause = ms => new Promise(resolve => setTimeout(resolve, ms))
try {
  const deadline = Date.now() + 30000
  while (!output.includes('\n') && child.exitCode == null && Date.now() < deadline) await pause(20)
  assert.match(output, /http:\/\/127\.0\.0\.1:/, errors)
  const { chromium } = await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
  browser = await chromium.launch({ executablePath: process.env.STUDIO_CHROMIUM_PATH })
  const page = await browser.newPage({ viewport: { width: 1400, height: 1000 } }), pageErrors = []
  page.on('pageerror', error => pageErrors.push(String(error)))
  await page.addInitScript(() => {
    let last = performance.now(); window.heartbeat = { maximumGapMs: 0, ticks: 0 }
    setInterval(() => { const now = performance.now(); heartbeat.maximumGapMs = Math.max(heartbeat.maximumGapMs, now - last); heartbeat.ticks++; last = now }, 10)
  })
  await page.goto(parentOrigin)
  const initial = await page.evaluate(async url => {
    const start = performance.now(), frame = document.createElement('iframe')
    frame.title = 'EPUB corpus'; frame.src = url; frame.style.cssText = 'width:1300px;height:900px;border:0'; document.body.append(frame)
    const { createReaderClient } = await import('/host-client.mjs'); window.client = createReaderClient(frame)
    const state = await client.ready(); return { state, firstUsefulMs: performance.now() - start }
  }, output.trim())
  assert.equal(initial.state.format, 'epub')
  assert.equal(initial.state.formatState.progressPrecision, 'chapter')
  const command = (method, params = {}) => page.evaluate(({ method, params }) => client.request(method, params), { method, params })
  const saved = initial.state.position.location.cfi
  const started = performance.now(), indexed = await command('epubProgress', { operation: 'build' }), indexRoundTripMs = performance.now() - started
  assert.equal(indexed.formatState.progressIndex.status, 'ready')
  assert.equal(indexed.formatState.progressIndex.scanned, 64)
  assert.equal(indexed.formatState.progressIndex.locations, 2560)
  assert.equal(indexed.position.location.cfi, saved)
  await command('settings', { epub: { flow: 'paginated' } })
  assert.ok((await command('epubProgress', { operation: 'seek', percentage: 1 })).position.location.href.includes('chapter-63.xhtml'))
  assert.ok((await command('epubProgress', { operation: 'seek', percentage: 0 })).position.location.href.includes('one.xhtml'))
  const heartbeat = await page.frameLocator('iframe[title="EPUB corpus"]').locator('html').evaluate(() => window.heartbeat)
  const session = await page.context().newCDPSession(page)
  await session.send('Performance.enable')
  const metrics = await session.send('Performance.getMetrics')
  const jsHeapUsedBytes = metrics.metrics.find(metric => metric.name === 'JSHeapUsedSize')?.value
  await session.detach()
  assert.deepEqual(pageErrors, [])
  await page.evaluate(() => client.dispose()); await page.locator('iframe').evaluate(frame => frame.remove())
  await browser.close(); browser = null
  const exited = once(child, 'exit'); child.stdin.end('\n'); await Promise.race([exited, pause(10000)])
  assert.equal(child.exitCode, 0, errors); assert.deepEqual(await readdir(cache), [])
  console.log(JSON.stringify({ ok: true, assertions: 11, actualRustZipService: true, firstUsefulMs: initial.firstUsefulMs, indexRoundTripMs, progressIndex: indexed.formatState.progressIndex, heartbeat, sampledHostPeakRssKiB: peakHostRssKiB, topLevelPageJsHeapUsedBytes: jsHeapUsedBytes, artifacts: dir }))
} finally {
  clearInterval(sampler); await browser?.close(); parent.closeAllConnections(); parent.close()
  if (child.exitCode == null && child.signalCode == null) { const exited = once(child, 'exit'); child.kill('SIGKILL'); await exited }
}
