import assert from 'node:assert/strict'
import { spawn, execFileSync, execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { once } from 'node:events'
import { createServer } from 'node:http'
import { mkdir, mkdtemp, readFile, readdir, realpath, writeFile } from 'node:fs/promises'
import { join, resolve } from 'node:path'

const dir = await mkdtemp(join(await realpath('target'), 'embedded-search-'))
const binary = resolve(process.argv.slice(2).find(value => !value.startsWith('--')) || 'target/debug/examples/embedded-host')
const source = '\ufeff# Public search\r\n\r\n' + '中文😀 needle '.repeat(130) + '\r\n\r\n' + Array.from({ length: 120 }, (_, n) => `Paragraph ${n}: needle\r\n\r\n`).join('') + '\r\n```text\r\n😀 K k K\r\n```\r\n'
const csv = 'ID,Text\r\n' + Array.from({ length: 250 }, (_, n) => `${n},table-needle-${n}\r\n`).join('')
const files = [join(dir, 'search.md'), join(dir, 'table.csv'), join(dir, 'search.pdf')]
function searchPdf() {
  const objects = [Buffer.from('<< /Type /Catalog /Pages 2 0 R >>'), Buffer.from('<< /Type /Pages /Kids [4 0 R 6 0 R] /Count 2 >>'), Buffer.from('<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>')]
  for (let n = 0; n < 2; n++) {
    const text = `BT /F1 14 Tf 40 1560 Td (PDF sentinel page ${n + 1}) Tj ET\n` + Array.from({ length: 60 }, (_, row) => `BT /F1 6 Tf 40 ${1530 - row * 18} Td (${'capneedle '.repeat(10)}) Tj ET\n`).join('')
    objects.push(Buffer.from(`<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 1600] /Resources << /Font << /F1 3 0 R >> >> /Contents ${5 + n * 2} 0 R >>`), Buffer.from(`<< /Length ${Buffer.byteLength(text)} >>\nstream\n${text}endstream`))
  }
  let bytes = Buffer.from('%PDF-1.4\n'), offsets = []
  for (const [i, object] of objects.entries()) { offsets.push(bytes.length); bytes = Buffer.concat([bytes, Buffer.from(`${i + 1} 0 obj\n`), object, Buffer.from('\nendobj\n')]) }
  const at = bytes.length
  return Buffer.concat([bytes, Buffer.from(`xref\n0 ${objects.length + 1}\n0000000000 65535 f \n${offsets.map(offset => `${String(offset).padStart(10, '0')} 00000 n \n`).join('')}trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${at}\n%%EOF\n`)])
}
await writeFile(files[0], source); await writeFile(files[1], csv); await writeFile(files[2], searchPdf())
const generated = JSON.parse(execFileSync('python3', ['tests/browser/epub_fixtures.py', dir], { encoding: 'utf8' }))
execFileSync('python3', ['-c', 'import sys,zipfile\np=sys.argv[1]\nwith zipfile.ZipFile(p) as z: entries={n:z.read(n) for n in z.namelist()}\nentries["OPS/one.xhtml"]=entries["OPS/one.xhtml"].replace(b"</body>",("".join("<p>capneedle %d</p>"%i for i in range(600))+"</body>").encode())\nwith zipfile.ZipFile(p,"w",compression=zipfile.ZIP_DEFLATED) as z:\n for n,v in entries.items(): z.writestr(n,v)', generated.book])
files.push(generated.book)
const originals = await Promise.all(files.map(file => readFile(file)))
const children = [], caches = [], urls = []
let browser, assertions = 0
const equal = (actual, expected, message) => { assert.deepEqual(actual, expected, message); assertions++ }
const pause = ms => new Promise(resolve => setTimeout(resolve, ms))
const host = createServer(async (request, response) => {
  const modules = new Map([
    ['/host-client.mjs', 'web/host-client.mjs'], ['/reader-bridge.mjs', 'web/reader-bridge.mjs'],
    ['/embedded-search.mjs', 'tests/browser/contracts/embedded-search.mjs']
  ])
  response.setHeader('Content-Type', request.url.endsWith('.mjs') ? 'text/javascript' : 'text/html')
  if (modules.has(request.url)) { response.end(await readFile(modules.get(request.url))); return }
  if (request.url === '/contract.mjs') { response.end(`import {embeddedSearchContract} from '/embedded-search.mjs';export const run=()=>embeddedSearchContract(${JSON.stringify(urls[0])});`); return }
  response.end('<!doctype html><html><body style="margin:0"><h1>Public search host</h1></body></html>')
})
host.listen(0, '127.0.0.1'); await once(host, 'listening')
const parentOrigin = `http://127.0.0.1:${host.address().port}`
try {
  for (const [i, file] of files.entries()) {
    if (process.argv.includes('--webkit') && i > 0) break
    const cache = join(dir, `cache-${i}`); await mkdir(cache); caches.push(cache)
    const child = spawn(binary, [file, `--parent=${parentOrigin}`, '--no-queries'], { env: { ...process.env, READER_CACHE_DIR: cache, MARKNORIA_CONFIG_DIR: join(dir, 'config') }, stdio: ['pipe', 'pipe', 'pipe'] })
    children.push(child)
    let output = '', errors = ''
    child.stdout.on('data', data => output += data); child.stderr.on('data', data => errors += data)
    const deadline = Date.now() + 30000
    while (!output.includes('\n') && child.exitCode == null && Date.now() < deadline) await pause(20)
    assert.match(output, /http:\/\/127\.0\.0\.1:/, errors); urls.push(output.trim())
  }
  if (process.argv.includes('--webkit')) {
    const { stdout } = await promisify(execFile)('/usr/bin/python3', ['tests/browser/contract-webkit.py', parentOrigin], { encoding: 'utf8', timeout: 190000 })
    const result = JSON.parse(stdout)
    console.log(JSON.stringify({ ok: true, ...result, nativeWayland: true, artifacts: dir }))
  } else {
    const { chromium } = await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
    browser = await chromium.launch({ executablePath: process.env.STUDIO_CHROMIUM_PATH })
    const page = await browser.newPage({ viewport: { width: 1550, height: 1000 } }), errors = []
    page.on('pageerror', error => errors.push(String(error)))
    await page.goto(parentOrigin)
    const contract = await page.evaluate(async url => (await import('/embedded-search.mjs')).embeddedSearchContract(url, { keepMounted: true }), urls[0])
    const command = (method, params = {}, pane = 0) => page.evaluate(({ method, params, pane }) => searchTestHost.panes[pane].client.request(method, params), { method, params, pane })
    const mount = url => page.evaluate(async url => { const pane = await searchTestHost.mount(url, `Search ${searchTestHost.panes.length}`); searchTestHost.panes.push(pane); return searchTestHost.panes.length - 1 }, url)
    const childFrame = async pane => (await page.locator('iframe').nth(pane).elementHandle()).contentFrame()
    let result = await command('search', { query: 'needle', offset: 100 })
    const id = result.searchId
    equal(Buffer.from(source).subarray(result.matches[0].sourceLocation.startByte, result.matches[0].sourceLocation.endByte).toString(), 'needle')
    let state = await command('searchActivate', { searchId: id, index: 101 })
    const textFrame = await childFrame(1) // pane A was remounted after pane B by the shared contract.
    equal(await textFrame.locator('#search').isVisible(), false, 'Host search opened the child toolbar')
    equal(await textFrame.locator('#matches').isVisible(), false, 'Host search opened the child results overlay')
    const sourceEvidence = async () => textFrame.evaluate(() => {
      const range = [...(CSS.highlights?.get('reader-search-current') || [])][0]
      if (!range) return null
      const owner = range.startContainer.parentElement.closest('[data-source-start]')
      const prefix = document.createRange(); prefix.selectNodeContents(owner); prefix.setEnd(range.startContainer, range.startOffset)
      const box = range.getBoundingClientRect(), viewport = document.getElementById('viewport').getBoundingClientRect()
      return { text: range.toString(), byte: Number(owner.dataset.sourceStart) + new TextEncoder().encode(prefix.toString()).length, visible: box.bottom > viewport.top && box.top < viewport.bottom }
    })
    let evidence = await sourceEvidence()
    equal(evidence.text, 'needle'); equal(evidence.byte, state.search.active.sourceLocation.startByte); equal(evidence.visible, true)
    const unicodeCase = await command('search', { query: 'k', matchCase: false })
    equal(unicodeCase.total, 3)
    for (const [index, text] of ['K', 'k', 'K'].entries()) {
      state = await command('searchActivate', { searchId: unicodeCase.searchId, index })
      evidence = await sourceEvidence()
      equal(evidence.text, text); equal(evidence.visible, true)
      equal(Buffer.from(source).subarray(state.search.active.start, state.search.active.end).toString(), text)
    }
    await command('setView', { view: 'source' })
    state = await command('searchActivate', { searchId: unicodeCase.searchId, index: 0 })
    equal((await command('selection')).quote, 'K')
    equal((await command('selection')).location.endByte - state.search.active.start, 3)
    await command('setView', { view: 'preview' })
    result = await command('search', { query: 'needle', offset: 100 })
    // A different option/query creates a new revision-owned identity.
    const restoredId = result.searchId
    await command('setView', { view: 'source' })
    await command('searchActivate', { searchId: restoredId, index: 102 })
    const selected = await command('selection')
    equal(selected.quote, 'needle'); equal(selected.location.kind, 'source')
    await command('setView', { view: 'preview' })
    state = await command('searchActivate', { searchId: restoredId, index: 129 })
    evidence = await sourceEvidence(); equal(evidence.byte, state.search.active.sourceLocation.startByte)
    await command('searchClear')
    equal(await textFrame.evaluate(() => (CSS.highlights?.get('reader-search-current')?.size || 0) + (CSS.highlights?.get('reader-search')?.size || 0)), 0)

    // Hold a real HTTP response, supersede it through the public protocol, then
    // release it. Its actual bridge response must say CANCELLED, not COMMAND.
    let release, reached
    const held = new Promise(resolve => reached = resolve), gate = new Promise(resolve => release = resolve)
    await page.route('**/api/search?**', async route => {
      const url = new URL(route.request().url())
      if (url.searchParams.get('query') !== 'needle' || url.searchParams.get('offset') !== '100') { await route.continue(); return }
      const response = await route.fetch(); reached(); await gate
      await route.fulfill({ response }).catch(() => {})
    })
    await page.evaluate(() => {
      searchTestHost.pending = searchTestHost.panes[0].client.request('search', { query: 'needle', offset: 100 }).then(() => ({ ok: true }), error => ({ name: error.name, code: error.code }))
    })
    await Promise.race([held, pause(10000).then(() => { throw Error('Delayed source search did not reach server') })])
    result = await command('search', { query: 'Paragraph' })
    const cancelled = await page.evaluate(() => searchTestHost.pending)
    equal(cancelled.code, 'CANCELLED')
    equal(result.total, 120)
    release(); await pause(100)
    state = await command('state'); equal(state.search.query, 'Paragraph'); equal(state.search.total, 120)
    await page.unroute('**/api/search?**')

    // A canceled activation must not install its older Source selection after
    // a newer occurrence has become current.
    await command('setView', { view: 'source' })
    result = await command('search', { query: 'needle', offset: 100 })
    const activationId = result.searchId, oldStart = result.matches[1].sourceLocation.startByte
    await command('searchActivate', { searchId: activationId, index: 100 })
    let releaseActivation, reachedActivation
    const activationHeld = new Promise(resolve => reachedActivation = resolve)
    const activationGate = new Promise(resolve => releaseActivation = resolve)
    await page.route('**/api/source?**', async route => {
      const url = new URL(route.request().url())
      if (url.searchParams.get('offset') !== String(oldStart)) { await route.continue(); return }
      const response = await route.fetch(); reachedActivation(); await activationGate
      await route.fulfill({ response }).catch(() => {})
    })
    await page.evaluate(searchId => {
      searchTestHost.abort = new AbortController()
      searchTestHost.pending = searchTestHost.panes[0].client.request('searchActivate', { searchId, index: 101 }, { signal: searchTestHost.abort.signal }).then(() => ({ ok: true }), error => ({ name: error.name, code: error.code }))
    }, activationId)
    await Promise.race([activationHeld, pause(10000).then(() => { throw Error('Delayed Source activation did not reach server') })])
    await page.evaluate(() => searchTestHost.abort.abort())
    equal((await page.evaluate(() => searchTestHost.pending)).ok, undefined)
    equal((await command('state')).search.current, null)
    equal(await command('selection'), null, 'Cancelled Source activation left the previous occurrence selected')
    state = await command('searchActivate', { searchId: activationId, index: 102 })
    releaseActivation(); await pause(100)
    equal((await command('state')).search.current, 102)
    equal((await command('selection')).location.startByte, state.search.active.sourceLocation.startByte)
    await page.unroute('**/api/source?**')
    await command('setView', { view: 'preview' })

    const tablePane = await mount(urls[1])
    result = await command('search', { query: 'table-needle', offset: 200 }, tablePane)
    equal(result.total, 250); equal(result.matches.length, 50); equal(result.matches[0].location.kind, 'cell')
    equal(result.matches[0].location.row, 200)
    state = await command('searchActivate', { searchId: result.searchId, index: 249 }, tablePane)
    equal(state.search.current, 249); equal(state.search.active.location.row, 249)
    const tableFrame = await childFrame(tablePane)
    await tableFrame.waitForFunction(() => [...(CSS.highlights?.get('reader-search-current') || [])].some(range => range.startContainer.parentElement.closest('tr')?.textContent.includes('table-needle-249')))
    const tableEvidence = await tableFrame.evaluate(() => {
      const range = [...CSS.highlights.get('reader-search-current')][0]
      const row = range.startContainer.parentElement.closest('tr')
      const rect = range.getBoundingClientRect(), viewport = document.getElementById('viewport').getBoundingClientRect()
      return { text: range.toString(), line: Number(row.dataset.sourceLine), row: row.textContent,
        visible: rect.bottom > viewport.top && rect.top < viewport.bottom && rect.right > viewport.left && rect.left < viewport.right }
    })
    equal(tableEvidence.text, 'table-needle'); equal(tableEvidence.line, 251); equal(tableEvidence.visible, true)
    assert.ok(tableEvidence.row.includes('table-needle-249')); assertions++

    const containerResults = []
    for (const [url, format, query] of [[urls[2], 'pdf', 'PDF sentinel'], [urls[3], 'epub', 'EPUB sentinel']]) {
      const pane = await mount(url)
      result = await command('search', { query }, pane)
      equal(result.total, 2); equal(result.totalExact, true); equal(result.matches[1].location.kind, format)
      equal(result.matches[1].locationPrecision, format === 'pdf' ? 'page' : 'exact')
      const capability = (await command('state', {}, pane)).capabilities.search
      equal(capability.pagination, 'capped'); equal(capability.resultLimit, 500)
      if (format === 'pdf') equal(result.matches[1].location.pages[0].page, 2)
      else assert.match(result.matches[1].location.cfi, /^epubcfi\(/)
      state = await command('searchActivate', { searchId: result.searchId, index: 1 }, pane)
      equal(state.search.current, 1); equal(state.search.active.location.kind, format)
      const frame = await childFrame(pane)
      equal(await frame.locator('[data-search-current]').count(), 1)
      state = await command('setView', { view: 'preview' }, pane)
      equal(state.search.current, 1, 'Idempotent Preview switch lost the active occurrence')
      const painted = await frame.evaluate(() => {
        const owner = document.querySelector('[data-search-current]')
        if (!owner) return null
        const key = owner.dataset.searchHighlight
        const documents = [document, ...[...owner.querySelectorAll('iframe')].map(frame => frame.contentDocument).filter(Boolean)]
        const ranges = documents.flatMap(document => [...(document.defaultView.CSS?.highlights?.get(key) || [])])
        return { owner: owner.dataset.searchCurrent, key, text: ranges.map(range => range.toString()).join(''), rectangles: owner.querySelectorAll('[data-search-current-rect]').length }
      })
      assert.ok(painted && (painted.text.includes(query) || painted.rectangles > 0), `${format} activation did not paint a current match: ${JSON.stringify(painted)}`); assertions++
      await command('searchClear', {}, pane)
      equal(await frame.locator('[data-search-current]').count(), 0)
      equal(await frame.evaluate(key => [document, ...[...document.querySelectorAll('iframe')].map(frame => frame.contentDocument).filter(Boolean)].some(document => document.defaultView.CSS?.highlights?.has(key)), painted.key), false, 'Clear left an owned renderer highlight registry entry')
      result = await command('search', { query: 'capneedle', offset: 400 }, pane)
      equal(result.totalExact, false); equal(result.available, 500); equal(result.total, 500); equal(result.matches.length, 100)
      if (format === 'pdf') {
        await command('settings', { pdf: { continuous: false } }, pane)
        state = await command('searchActivate', { searchId: result.searchId, index: 499 }, pane)
        equal(state.search.current, 499)
        const visible = await frame.evaluate(() => {
          const owner = document.querySelector('[data-search-current]'), range = [...(CSS.highlights?.get(owner?.dataset.searchHighlight) || [])][0]
          if (!range) return false
          const rect = range.getBoundingClientRect(), viewport = document.getElementById('viewport').getBoundingClientRect()
          return rect.width > 0 && rect.height > 0 && rect.bottom > viewport.top && rect.top < viewport.bottom
        })
        equal(visible, true, 'PDF paginated search did not reveal the lower occurrence on a previously hidden page')
      }
      const beyond = await page.evaluate(({ pane }) => searchTestHost.panes[pane].client.request('search', { query: 'capneedle', offset: 501 }).then(() => null, error => error.code), { pane })
      equal(beyond, 'RANGE')
      containerResults.push({ format, cap: result.available, totalExact: result.totalExact })
      await page.evaluate(async pane => { await searchTestHost.panes[pane].client.dispose(); searchTestHost.panes[pane].frame.style.display = 'none' }, pane)
    }
    await page.evaluate(async () => { for (const pane of searchTestHost.panes) { await pane.client.dispose().catch(() => {}); pane.frame.remove() } })
    equal(errors, [])
    console.log(JSON.stringify({ ok: true, assertions: assertions + contract.assertions, publicProtocolOnly: true, sourceChecks: contract.checks, containers: containerResults, currentHighlightEvidence: true, cancelledLateResponse: true, cancelledLateActivation: true, artifacts: dir }))
  }
} finally {
  await browser?.close()
  host.closeAllConnections(); host.close()
  for (const child of children) {
    if (child.exitCode == null && child.signalCode == null) { const exited = once(child, 'exit'); child.stdin.end('\n'); await exited }
  }
  for (const cache of caches) assert.deepEqual(await readdir(cache), [])
  for (const [i, file] of files.entries()) assert.deepEqual(await readFile(file), originals[i])
}
