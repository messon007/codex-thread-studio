// Real HTTP reader: bounded projections must never become complete input.
import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { once } from 'node:events'
import { mkdtemp, mkdir, readFile, readdir, realpath, writeFile } from 'node:fs/promises'
import { join, resolve } from 'node:path'

const directory = await mkdtemp(join(await realpath('target'), 'large-markdown-tables-'))
const cache = join(directory, 'cache'), file = join(directory, 'report.md')
await mkdir(cache)
const prefix = '| Key | Value |\r\n| --- | --- |\r\n'
const value = '中😀&'.repeat(27000) + 'OriginalTailNeedle'
const firstRow = `| large-0 | ${value} |\r\n`
const text = '\ufeff# Large report\r\n\r\n' + prefix + firstRow +
  Array.from({ length: 63 }, (_, n) => `| large-${n + 1} | ${'x'.repeat(215606)} |\r\n`).join('') +
  '\r\n## Dense row\r\n\r\n' + prefix + `| dense | ${'*v* '.repeat(6000)}DenseTail |\r\n` +
  '\r\n## Dense pages\r\n\r\n' + prefix + Array.from({ length: 16 }, (_, n) => `| row-${n} | ${'*v* '.repeat(350)}tail-${n} |\r\n`).join('')
await writeFile(file, text)
const nestedText = '\ufeff# Nested source fallback NestedHeadNeedle NestedHeadNeedle\r\n\r\n> | Key | Value |\r\n> |---|---|\r\n> | a | ' + 'x'.repeat(140000) + 'NestedFallbackTail |\r\n'
await writeFile(join(directory, 'nested.md'), nestedText)
const server = spawn(resolve(process.argv[2] || 'target/debug/marknoria-server'), [file, '--root', directory], {
  env: { ...process.env, READER_CACHE_DIR: cache, MARKNORIA_CONFIG_DIR: join(directory, 'config') }, stdio: ['ignore', 'pipe', 'pipe']
})
let stdout = '', stderr = '', browser, page, assertions = 0
server.stdout.on('data', value => stdout += value)
server.stderr.on('data', value => stderr += value)
const check = (actual, expected) => { assertions++; assert.deepEqual(actual, expected) }
try {
  const deadline = Date.now() + 30000
  while (!stdout.includes('http://') && Date.now() < deadline && server.exitCode == null) await new Promise(resolve => setTimeout(resolve, 30))
  assert.match(stdout, /http:\/\/127\.0\.0\.1:/, stderr)
  const url = new URL(stdout.trim().split('\n')[0]), token = url.hash.slice(1)
  const call = (path, options = {}) => fetch(new URL(path, url), { ...options, headers: { 'x-reader-token': token, 'Content-Type': 'application/json' } })
  const api = async (path, options) => { const response = await call(path, options); assert.equal(response.status, 200, await response.clone().text()); return response.json() }
  const { chromium } = await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
  browser = await chromium.launch({ executablePath: process.env.STUDIO_CHROMIUM_PATH })
  page = await browser.newPage({ viewport: { width: 1440, height: 900 } })
  const errors = []
  page.on('pageerror', error => errors.push(String(error)))
  await page.goto(url.href)
  await page.waitForFunction(() => window.readerExperiment?.metrics.firstContentMs != null)
  const index = await page.evaluate(() => readerExperiment.index)
  const firstContentMs = await page.evaluate(() => readerExperiment.metrics.firstContentMs)
  check(index.tables.map(table => table.rows), [64, 1, 16])
  check(index.pages.every(page => page.bytes <= 128 * 1024), true)
  check(index.sourceBytes, Buffer.byteLength(text))
  const table = page.locator('article[data-table="0"]')
  await table.locator('td[data-cell-preview="true"]').first().waitFor()
  check(await table.locator('.table-cell-preview-notice').isVisible(), true)
  await table.locator('td[data-cell-preview="true"]').first().click()
  check(await table.locator('.table-cell-copy').textContent(), 'Copy preview')
  await table.locator('.table-cell-view').click()
  const dialog = page.getByRole('dialog', { name: 'Original source row', exact: true })
  const parts = []
  for (let n = 0; n < 5; n++) {
    await page.waitForFunction(() => { const pre = document.querySelector('.table-cell-dialog pre'); return pre?.textContent && !pre.hasAttribute('aria-busy') })
    const part = await dialog.locator('pre').textContent()
    check(Buffer.byteLength(part) <= 128 * 1024, true)
    check(await dialog.locator('pre').count(), 1)
    parts.push(part)
    if (await dialog.locator('.table-source-next').isDisabled()) break
    await dialog.locator('.table-source-next').click()
  }
  const rowStart = Buffer.from(text).indexOf(Buffer.from(firstRow)), rowEnd = rowStart + Buffer.byteLength(firstRow)
  check(Buffer.byteLength(parts.join('')), rowEnd - rowStart)
  check(Buffer.from(parts.join('')).equals(Buffer.from(text).subarray(rowStart, rowEnd)), true)
  check(parts.length > 1, true)
  check(await page.evaluate(() => readerExperiment.tables.selection()), null)
  await page.screenshot({ path: join(directory, 'source-row-window.png') })
  await dialog.getByRole('button', { name: 'Close', exact: true }).click()
  const tooLarge = await call('/api/table-query', { method: 'POST', body: JSON.stringify({ revision: index.revision, table: 0, sql: 'SELECT COUNT(*) FROM t' }) })
  check(tooLarge.status, 400)
  check(/Complete table row exceeds.*budget/.test(await tooLarge.text()), true)
  const dense = await api('/api/table-query', { method: 'POST', body: JSON.stringify({ revision: index.revision, table: 1, sql: 'SELECT length(c2), substr(c2,-9) FROM t' }) })
  check(dense.rows, [['12009', 'DenseTail']])
  const cumulative = await api('/api/table-query', { method: 'POST', body: JSON.stringify({ revision: index.revision, table: 2, sql: 'SELECT COUNT(*) FROM t' }) })
  check(cumulative.rows, [['16']])
  let row = 0, seen = []
  while (row < 16) {
    const window = (await api(`/api/table?id=2&row=${row}&columns=0`)).result
    check(window.rowStart <= row && window.rowEnd > row, true)
    seen.push(...[...window.html.matchAll(/<td[^>]*>(row-\d+)<\/td>/g)].map(match => match[1]))
    row = window.rowEnd
  }
  check(seen, Array.from({ length: 16 }, (_, n) => `row-${n}`))
  const hits = await api('/api/search?query=OriginalTailNeedle')
  check(hits.matches.length, 1)
  check(Buffer.from(text).subarray(hits.matches[0].start, hits.matches[0].end).toString(), 'OriginalTailNeedle')
  // Omissions belong to the original row, including a hidden source column.
  await table.locator('.column-picker summary').click()
  await table.locator('.column-picker input[type="checkbox"]').nth(1).uncheck()
  await table.getByRole('button', { name: 'Apply', exact: true }).click()
  await page.waitForFunction(() => document.querySelector('article[data-table="0"]').dataset.selectedColumns === '[0]')
  check(await table.locator('tbody tr').first().getAttribute('data-row-preview'), 'true')
  await page.locator('#search-toggle').click()
  await page.locator('#query').fill('OriginalTailNeedle'); await page.locator('#query').press('Enter')
  await page.locator('#matches button').first().waitFor(); await page.locator('#matches button').first().click()
  await page.waitForFunction(() => document.querySelector('#source-view').dataset.ready === 'true' && document.querySelector('#source-mode').getAttribute('aria-pressed') === 'true')
  check(await page.locator('#source-text').textContent().then(text => text.includes('OriginalTailNeedle')), true)
  await page.waitForFunction(() => document.querySelector('#search-count').textContent === '1/1')
  check((await page.locator('#search-count').textContent()).includes('1/1'), true)
  await page.screenshot({ path: join(directory, 'omitted-search-source.png') })
  await api('/api/open?path=nested.md', { method: 'POST' })
  await page.reload()
  await page.waitForFunction(() => window.readerExperiment?.index?.name === 'nested.md' && readerExperiment.metrics.firstContentMs != null)
  check(await page.evaluate(() => readerExperiment.index.textPages), true)
  check(await page.evaluate(() => readerExperiment.index.tables.length), 0)
  check(await page.locator('.document-preview-note').first().textContent().then(text => text.includes('complete original Markdown source')), true)
  check(await page.locator('.table-query-open').count(), 0)
  await page.locator('#source-mode').click()
  await page.waitForFunction(() => document.querySelector('#source-view').dataset.ready === 'true')
  check(await page.locator('#source-text').textContent().then(text => text.includes('Nested source fallback')), true)
  await page.locator('#preview-mode').click()
  if (!await page.locator('#query').isVisible()) await page.locator('#search-toggle').click()
  await page.locator('#query').fill('NestedFallbackTail'); await page.locator('#query').press('Enter')
  await page.locator('#matches button').first().waitFor(); await page.locator('#matches button').first().click()
  await page.waitForFunction(() => [...(CSS.highlights.get('reader-search-current') || [])].some(range => range.toString() === 'NestedFallbackTail'))
  const heads = await api('/api/search?query=NestedHeadNeedle')
  check(heads.matches.length, 2)
  await page.locator('#query').fill('NestedHeadNeedle'); await page.locator('#query').press('Enter')
  await page.locator('#matches button').nth(1).waitFor(); await page.locator('#matches button').nth(1).click()
  await page.waitForFunction(() => document.querySelector('#search-count').textContent === '2/2')
  const selectedByte = await page.evaluate(() => {
    const range = [...CSS.highlights.get('reader-search-current')][0]
    const span = range.startContainer.parentElement.closest('[data-source-start]')
    const prefix = document.createRange(); prefix.selectNodeContents(span); prefix.setEnd(range.startContainer, range.startOffset)
    return Number(span.dataset.sourceStart) + new TextEncoder().encode(prefix.toString()).length
  })
  check(selectedByte, heads.matches[1].start)
  await page.screenshot({ path: join(directory, 'nested-fallback-search.png') })
  check(await readFile(join(directory, 'nested.md'), 'utf8'), nestedText)
  check(errors, [])
  check(await readFile(file, 'utf8'), text)
  await writeFile(join(directory, 'report.json'), JSON.stringify({ ok: true, assertions, sourceBytes: Buffer.byteLength(text), tables: 3, pageCount: index.pages.length, firstContentMs, indexMs: index.indexMs, indexPeakRssKiB: index.peakRssKib, nestedSourceFallback: true, nestedSourceBytes: Buffer.byteLength(nestedText), nestedFirstContentMs: await page.evaluate(() => readerExperiment.metrics.firstContentMs) }, null, 2))
  console.log(JSON.stringify({ ok: true, assertions, sourceBytes: Buffer.byteLength(text), artifacts: directory }))
} catch (error) {
  if (page && !page.isClosed()) {
    await page.screenshot({ path: join(directory, 'failure.png') })
    const state = await page.evaluate(() => ({ query: document.querySelector('#query')?.value, count: document.querySelector('#search-count')?.textContent,
      status: document.querySelector('#status')?.textContent, search: window.readerExperiment?.search?.state?.(),
      pending: window.readerExperiment?.pendingNavigation, matches: [...document.querySelectorAll('#matches button')].map(node => node.textContent.slice(0, 200)) }))
    await writeFile(join(directory, 'failure.json'), JSON.stringify({ error: error.message, state }, null, 2))
    console.error(JSON.stringify(state))
  }
  console.error('Large Markdown table artifacts:', directory, stderr)
  throw error
} finally {
  await browser?.close()
  if (server.exitCode == null && server.signalCode == null) { const exit = once(server, 'exit'); server.kill('SIGINT'); await exit }
  check(await readdir(cache), [])
}
