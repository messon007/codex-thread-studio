import assert from 'node:assert/strict'
import { spawn, execFile, execFileSync } from 'node:child_process'
import { once } from 'node:events'
import { mkdir, mkdtemp, readFile, readdir, realpath, rm, writeFile } from 'node:fs/promises'
import { createServer } from 'node:http'
import { join, resolve } from 'node:path'
import { promisify } from 'node:util'

const directory = await mkdtemp(join(await realpath('target'), 'embedded-reading-'))
const binary = resolve(process.argv.slice(2).find(value => !value.startsWith('--')) || 'target/debug/examples/embedded-host')
const children = [], urls = {}, caches = []
let browser
const pause = ms => new Promise(resolve => setTimeout(resolve, ms))
const host = createServer(async (request, response) => {
  const modules = {
    '/host-client.mjs': 'web/host-client.mjs',
    '/reader-bridge.mjs': 'web/reader-bridge.mjs',
    '/note-passage.mjs': 'web/note-passage.mjs',
    '/comment-ranges.mjs': 'web/comment-ranges.mjs',
    '/embedded-reading.mjs': 'tests/browser/contracts/embedded-reading.mjs',
  }
  if (request.url === '/contract.mjs') {
    response.setHeader('Content-Type', 'text/javascript')
    response.end(`import {embeddedReadingContract} from '/embedded-reading.mjs'; export const run = options => embeddedReadingContract(${JSON.stringify(urls)}, options);`)
  } else if (Object.hasOwn(modules, request.url)) {
    response.setHeader('Content-Type', 'text/javascript')
    response.end(await readFile(modules[request.url]))
  } else {
    response.setHeader('Content-Type', 'text/html')
    response.end('<!doctype html><html><body>Independent reader host</body></html>')
  }
})
host.listen(0, '127.0.0.1')
await once(host, 'listening')
const origin = `http://127.0.0.1:${host.address().port}`

function pdfFixture() {
  const objects = [
    '<< /Type /Catalog /Pages 2 0 R >>',
    '<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>',
    '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 700] /Resources <<>> >>',
    '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 1000 1100] /Resources <<>> >>',
  ]
  let pdf = '%PDF-1.4\n'
  const offsets = objects.map((object, i) => {
    const offset = pdf.length
    pdf += `${i + 1} 0 obj\n${object}\nendobj\n`
    return offset
  })
  const xref = pdf.length
  return pdf + `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n`
    + offsets.map(offset => `${String(offset).padStart(10, '0')} 00000 n \n`).join('')
    + `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`
}

try {
  const fixtures = {
    'small.md': '# Reader\r\n\r\nrepeat 中文😀 **and then** repeat 中文😀 end.\r\n\r\n'
      + Array.from({ length: 250 }, (_, i) => `Paragraph ${i} ${'word '.repeat(16)}\r\n\r\n`).join('') + 'End 中文😀',
    'table.md': '# Table\n\n| name | value |\n| --- | --- |\n'
      + `| long | visible prefix ${'很长'.repeat(16000)} omitted-tail |\n`
      + Array.from({ length: 235 }, (_, i) => `| row ${i} | ${i} |\n`).join(''),
    'long.txt': 'First\n' + '中文😀 '.repeat(17500) + '\r\nLast 中文😀',
    'pdf.pdf': pdfFixture(),
  }
  for (const [filename, contents] of Object.entries(fixtures)) await writeFile(join(directory, filename), contents)
  const epub = JSON.parse(execFileSync('python3', ['tests/browser/epub_fixtures.py', directory], { encoding: 'utf8' })).book
  const documents = { small: 'small.md', reopened: 'small.md', table: 'table.md', long: 'long.txt', pdf: 'pdf.pdf', epub }
  for (const [name, file] of Object.entries(documents)) {
    const cache = join(directory, `cache-${name}`)
    await mkdir(cache)
    caches.push(cache)
    const child = spawn(binary, [resolve(directory, file), `--parent=${origin}`, '--no-queries', ...(name === 'small' ? ['--write'] : [])], {
      env: { ...process.env, READER_CACHE_DIR: cache, MARKNORIA_CONFIG_DIR: join(directory, 'config') },
      stdio: ['pipe', 'pipe', 'pipe'],
    })
    children.push(child)
    let stdout = '', stderr = ''
    child.stdout.on('data', bytes => { stdout += bytes })
    child.stderr.on('data', bytes => { stderr += bytes })
    const deadline = Date.now() + 30000
    while (!stdout.includes('\n') && child.exitCode == null && Date.now() < deadline) await pause(20)
    assert.match(stdout, /http:\/\/127\.0\.0\.1:/, stderr)
    urls[name] = stdout.trim()
  }
  if (process.argv.includes('--webkit')) {
    const result = await promisify(execFile)('/usr/bin/python3', ['tests/browser/contract-webkit.py', origin], { timeout: 190000 })
    console.log(result.stdout.trim())
  } else {
    const { chromium } = await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
    browser = await chromium.launch({ executablePath: process.env.STUDIO_CHROMIUM_PATH })
    const page = await browser.newPage({ viewport: { width: 1280, height: 820 } })
    const errors = []
    page.on('pageerror', error => errors.push(error.message))
    await page.goto(origin)
    const contract = await page.evaluate(async () => (await import('/contract.mjs')).run({ keepMounted: true })).catch(async error => {
      const frames = await Promise.all(page.frames().slice(1).map(frame => frame.evaluate(() => {
        const reader = window.readerExperiment, code = reader?.source?.code, scroll = code?.view.scrollDOM
        return { name: reader?.index.name, lines: reader?.index.sourceLines, source: reader?.source?.visible,
          cursor: code?.cursorLine(), position: reader?.position(),
          scroll: scroll && [scroll.scrollTop, scroll.clientHeight, scroll.scrollHeight] }
      }).catch(() => null)))
      console.error(JSON.stringify(frames))
      throw error
    })
    const pdf = page.frameLocator('iframe[title=pdf]')
    await pdf.locator('[data-pdf-page="2"]').waitFor()
    const visible = await pdf.locator('[data-pdf-page="2"]').evaluate(node => {
      const page = node.getBoundingClientRect(), viewport = node.closest('#viewport')
      const view = viewport.getBoundingClientRect(), x = page.left + 0.88 * page.width, y = page.top + 0.62 * page.height
      return { horizontal: x >= view.left && x < view.right, vertical: y >= view.top - 2 && y < view.bottom }
    })
    assert.deepEqual(visible, { horizontal: true, vertical: true }, 'PDF region outside the viewport')
    const action = (name, method, params) => page.evaluate(({ name, method, params }) => readingFixture[name].command(method, params), { name, method, params })
    const heldSource = '**/api/source?**'
    async function interrupt({ routeMatch, start, interrupt, expected }) {
      let hit, release
      const reached = new Promise(resolve => { hit = resolve }), gate = new Promise(resolve => { release = resolve })
      const handler = async route => {
        if (!routeMatch(new URL(route.request().url()))) { await route.continue(); return }
        const response = await route.fetch()
        hit()
        await gate
        await route.fulfill({ response }).catch(() => {})
      }
      await page.route(heldSource, handler)
      try {
        await start()
        await Promise.race([reached, pause(10000).then(() => { throw Error('Source request was not held') })])
        await interrupt()
        assert.equal(await page.evaluate(() => readingFixture.pending), expected)
      } finally { release(); await page.unroute(heldSource, handler) }
    }
    await interrupt({
      routeMatch: url => url.searchParams.get('line') === '190' && url.searchParams.get('before') === '24',
      start: () => page.evaluate(() => {
        const abort = new AbortController()
        readingFixture.abort = abort
        readingFixture.pending = readingFixture.forced.command('navigate', { location: { kind: 'line', line: 190 } }, { signal: abort.signal })
          .then(() => 'success', error => error.name)
      }),
      interrupt: () => page.evaluate(() => readingFixture.abort.abort()),
      expected: 'AbortError',
    })
    const match = (await action('forced', 'search', { query: 'Paragraph 120' })).matches[0].sourceLocation
    await interrupt({
      routeMatch: url => url.searchParams.get('offset') === String(match.startByte),
      start: () => page.evaluate(location => {
        const { owner, command } = readingFixture.forced
        const anchor = { version: 1, documentId: owner.documentId, revision: owner.revision, location }
        readingFixture.pending = command('navigatePassage', { anchor }).then(() => 'success', error => error.code)
      }, match),
      interrupt: () => action('forced', 'navigate', { location: { kind: 'line', line: 45 } }),
      expected: 'CANCELLED',
    })
    assert.ok((await action('forced', 'state')).position.location.line < 45)
    await action('small', 'setView', { view: 'edit' })
    const source = page.frameLocator('iframe[title=small]').first()
    await source.locator('#editor-mount .cm-content').press('Home')
    await source.locator('#editor-mount .cm-content').press('A')
    assert.equal((await action('small', 'state')).dirty, true)
    const reOpen = () => page.evaluate(() => readingFixture.small.command('navigatePassage', { anchor: readingFixture.prose }).then(() => 'success', error => error.code))
    assert.equal(await reOpen(), 'DIRTY')
    assert.equal((await action('small', 'state')).dirty, true)
    await action('small', 'save')
    await action('small', 'setView', { view: 'preview' })
    assert.equal(await reOpen(), 'REVISION')
    assert.deepEqual(errors, [])
    await page.evaluate(async () => {
      const { pending, abort, prose, ...panes } = readingFixture
      await Promise.all(Object.values(panes).map(pane => pane.client.dispose({ force: true })))
    })
    console.log(JSON.stringify({ ...contract, chromium: { pdfGeometry: true, cancelledSource: true, supersededPassage: true, dirtyAndChangedRevision: true } }))
  }
} finally {
  await browser?.close()
  await Promise.all(children.map(async child => {
    if (child.exitCode == null && child.signalCode == null) {
      const exit = once(child, 'exit')
      child.stdin.end()
      await exit
    }
  }))
  host.close()
  for (const cache of caches) assert.deepEqual(await readdir(cache), [])
  await rm(directory, { recursive: true, force: true })
}
