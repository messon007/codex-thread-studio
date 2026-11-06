import assert from 'node:assert/strict'
import { createServer } from 'node:http'
import { once } from 'node:events'
import { mkdtemp, readFile, realpath, writeFile } from 'node:fs/promises'
import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { join } from 'node:path'
const dir = await mkdtemp(join(await realpath('target'), 'epub-layout-'))
const actual = process.argv.indexOf('--book')
const generated = JSON.parse((await promisify(execFile)('python3', actual < 0
  ? ['tests/browser/epub_layout_fixtures.py', dir]
  : ['-c', 'import zipfile,json,base64,sys; z=zipfile.ZipFile(sys.argv[1]); print(json.dumps({"package":"content.opf","chapter":"text/part0008.html","entries":{n:base64.b64encode(z.read(n)).decode() for n in z.namelist() if not n.endswith("/")}}))', process.argv[actual + 1]], { maxBuffer: 32 * 1024 * 1024 })).stdout)
const content = new Map(Object.entries(generated.entries).map(([name, value]) => ['/book/' + name, Buffer.from(value, 'base64')]))
const requests = []
const server = createServer(async (request, response) => {
  response.setHeader('Content-Security-Policy', "default-src 'none'; script-src 'self'; frame-src 'self' blob:; connect-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data: blob:; object-src 'none'; base-uri 'none'; form-action 'none'")
  const path = decodeURIComponent(new URL(request.url, 'http://localhost').pathname)
  if (path === '/') { response.setHeader('Content-Type', 'text/html'); response.end('<!doctype html><html><body></body></html>'); return }
  if (path === '/contract.mjs') {
    response.setHeader('Content-Type', 'text/javascript')
    response.end("import {epubLayoutContract} from '/epub-layout-contract.mjs'; export const run=()=>epubLayoutContract(" + JSON.stringify({ package: generated.package, chapter: generated.chapter, diagnose: process.argv.includes('--diagnose'), synthetic: actual < 0 }) + ');')
    return
  }
  if (path === '/favicon.ico') { response.writeHead(204); response.end(); return }
  if (content.has(path)) { requests.push(path); response.setHeader('Content-Type', path.endsWith('.css') ? 'text/css' : path.endsWith('.png') ? 'image/png' : 'application/xml'); response.end(content.get(path)); return }
  const files = { '/epub-layout-contract.mjs': 'tests/browser/contracts/epub-layout.mjs', '/epub-reader.mjs': 'web/epub-reader.mjs', '/translation.mjs': 'web/translation.mjs', '/epub-reader.css': 'web/epub-reader.css', '/vendor/epub.mjs': 'web/vendor/epub.mjs' }
  if (!files[path]) { response.writeHead(404); response.end(); return }
  response.setHeader('Content-Type', path.endsWith('.css') ? 'text/css' : 'text/javascript'); response.end(await readFile(files[path]))
})
server.listen(0, '127.0.0.1'); await once(server, 'listening')
let browser
try {
  const url = `http://127.0.0.1:${server.address().port}/`
  let result
  if (process.argv.includes('--webkit')) {
    result = JSON.parse((await promisify(execFile)('/usr/bin/python3', [
      'tests/browser/contract-webkit.py', url, '1300', '950'
    ], { timeout: 190000, maxBuffer: 1024 * 1024 })).stdout)
  } else {
    const { chromium } = await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
    browser = await chromium.launch({ executablePath: process.env.STUDIO_CHROMIUM_PATH })
    const page = await browser.newPage({ viewport: { width: 1300, height: 950 } })
    const pageErrors = []
    page.on('pageerror', error => pageErrors.push(String(error)))
    await page.goto(url)
    result = await page.evaluate(async () => (await import('/contract.mjs')).run())
    assert.deepEqual(pageErrors, [])
    result.browserVersion = browser.version()
  }
  await writeFile(join(dir, 'result.json'), JSON.stringify(result, null, 2))
  assert.deepEqual(result.errors, [])
  console.log(JSON.stringify({ ok: true, ...result, requests: requests.length, artifacts: dir, book: generated.book }))
} finally { await browser?.close(); server.closeAllConnections(); server.close() }
