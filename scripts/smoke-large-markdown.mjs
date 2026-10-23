import assert from 'node:assert/strict'
import { spawn, execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { mkdtemp, mkdir, readFile, readdir, rm, writeFile, copyFile, rename, symlink } from 'node:fs/promises'
import { once } from 'node:events'
import { resolve, join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const output = resolve(root, 'target/large-markdown-smoke')
await mkdir(output, { recursive: true })
const directory = await mkdtemp(join(output, 'run-'))
const fixture = join(directory, 'large.md')
if (process.argv[2]) await copyFile(resolve(process.argv[2]), fixture)
const { chromium } = await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
const binary = process.env.STUDIO_REMOTE_BINARY || resolve(root, 'target/release/codex-thread-studio')
const cache = join(directory, 'cache')
const environment = { ...process.env, XDG_CONFIG_HOME: join(directory, 'config'), XDG_DATA_HOME: join(directory, 'data'), XDG_CACHE_HOME: join(directory, 'xdg-cache'), CODEX_HOME: join(directory, 'codex'), READER_CACHE_DIR: cache, READER_CHROMIUM: '/intentionally-unavailable-chromium' }
let browser
let server
let log = ''
try {
  if (!process.argv[2]) await writeFile(fixture, '# Large report\n\n' + 'Bounded document content.\n\n'.repeat(70_000))
  const diagrams = join(directory, 'diagrams.md')
  await writeFile(diagrams, '# Mermaid\n\n```mermaid\nflowchart LR\n A[Complete] --> B[Done]\n```\n\n# PlantUML\n\n```plantuml\nAlice -> Bob: Hello\n```\n\n# D2\n\n```d2\na: Complete\nb: Diagram\na -> b: context\n```\n\n# Padding\n\n' + 'Read-only document.\n\n'.repeat(55_000))
  server = spawn(binary, ['--serve', '--listen', '127.0.0.1:0'], { env: environment, stdio: ['ignore', 'pipe', 'pipe'] })
  server.stdout.on('data', bytes => { log = (log + bytes).slice(-100_000) })
  server.stderr.on('data', bytes => { log = (log + bytes).slice(-100_000) })
  server.on('error', error => { log += error.message })
  const deadline = Date.now() + 20_000
  while (!/http:\/\/127\.0\.0\.1:\d+\/#token=[a-f0-9]{32}/u.test(log)) {
    if (Date.now() > deadline || server.exitCode != null) throw new Error(`Studio startup failed: ${log}`)
    await new Promise(resolve => setTimeout(resolve, 50))
  }
  const url = new URL(log.match(/http:\/\/127\.0\.0\.1:\d+\/#token=[a-f0-9]{32}/u)[0])
  const token = new URLSearchParams(url.hash.slice(1)).get('token')
  const request = async (path, value) => fetch(new URL(path, url), { method: 'POST', headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' }, body: JSON.stringify(value) })
  const small = join(directory, 'small.md')
  await writeFile(small, '# Editable small document\n\nOriginal content.\n')
  const smallResponse = await request('/studio/review-file', { root: directory, path: small }).then(response => response.json())
  assert.equal(smallResponse.largeMarkdown, undefined)
  assert.equal(smallResponse.readOnly, false)
  assert.equal(smallResponse.content, await readFile(small, 'utf8'))
  assert.equal((await fetch(new URL('/studio/review-file', url), { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ root: dirname(fixture), path: fixture }) })).status, 401)
  browser = await chromium.launch({ headless: true, executablePath: process.env.STUDIO_CHROMIUM_PATH })
  const page = await browser.newPage({ viewport: { width: 1500, height: 1000 } })
  const errors = []
  const editRequests = []
  page.on('request', request => { const parsed = new URL(request.url()); if (parsed.pathname.endsWith('/api/edit')) editRequests.push(`${request.method()} ${parsed.search}`) })
  page.on('pageerror', error => errors.push(error.message))
  await page.route(`${url.origin}/app.js`, route => route.fulfill({ contentType: 'text/javascript', body: `
    import { createDocumentWorkspaceController, createDocumentWorkspaceState } from '/document-workspace-controller.mjs';
    const state = createDocumentWorkspaceState();
    const gatewayFetch = (path, options = {}) => fetch(path, { ...options, headers: { ...options.headers, Authorization: 'Bearer ' + window.__CODEX_THREAD_STUDIO_GATEWAY__.token } });
    window.testState = state;
    window.controller = createDocumentWorkspaceController({ state, gatewayFetch, randomId: () => crypto.randomUUID(), view: {
      selectedThread: () => ({ cwd: '/' }), selectedStateKey: () => 'test-session',
      activateRightWorkspace: tool => { state.activeRightWorkspace = tool },
      hideComposerMenu() {}, closeActionMenus() {}, applyRightRailWidth() {},
      disconnectMarkdownImageObserver() {}, disposeMarkdownImageAssets() {}, hideSelection() {},
      renderMarkdownDocument() { throw Error('Whole-document parsing must not run') },
    }});
    window.controller.bind();
    const composer = document.querySelector('#composer-input');
    for (let node = composer; node && node !== document.body; node = node.parentElement) node.classList.remove('hidden');
  ` }))
  await page.goto(url.href)
  await page.waitForFunction(() => Boolean(window.controller))
  const opened = []
  for (const path of [fixture, diagrams]) {
    const started = Date.now()
    await page.evaluate(path => window.controller.open({ root: path.slice(0, path.lastIndexOf('/')), path }, { allowDetachedRoot: true }), path)
    const reader = await page.evaluate(() => window.testState.artifact.largeMarkdown)
    assert.ok(reader?.url)
    assert.equal(await page.locator('#artifact-edit').isVisible(), true)
    const frame = await page.locator('.artifact-large-markdown-frame').elementHandle().then(element => element.contentFrame())
    await frame.waitForFunction(() => window.readerExperiment?.metrics.firstContentMs != null, null, { timeout: 60_000 })
    const snapshot = await frame.evaluate(() => ({ sourceBytes: readerExperiment.index.sourceBytes, pages: readerExperiment.index.pages.length, tables: readerExperiment.index.tables.length, mounted: document.querySelectorAll('#space *').length, diagrams: readerExperiment.index.diagrams.length, mode: readerExperiment.diagramMetrics.mode }))
    assert.equal(await frame.locator('.reader-header').isVisible(), false)
    assert.equal(await frame.locator('.reader-footer').isVisible(), false)
    assert.equal(await page.locator('#artifact-view-switch').isVisible(), true)
    await page.waitForFunction(() => document.querySelector('#artifact-meta').textContent.includes('lines'))
    assert.equal(await page.locator('#artifact-hint').textContent(), '')
    const actionLayout = await page.locator('.artifact-header').evaluate(header => {
      const bounds = header.getBoundingClientRect()
      return [...header.querySelectorAll('button')].filter(button => button.getBoundingClientRect().width).every(button => { const rect = button.getBoundingClientRect(); return rect.top >= bounds.top && rect.bottom <= bounds.bottom })
    })
    assert.equal(actionLayout, true, 'Document controls moved outside the shared header')
    assert.ok(snapshot.sourceBytes >= 1024 * 1024)
    assert.ok(snapshot.mounted < 5000)
    assert.equal(snapshot.mode, 'browser')
    const base = new URL(reader.url, url)
    const readerToken = base.hash.slice(1)
    const api = (path, options) => fetch(new URL(path, base), options)
    assert.equal((await api('api/source?line=1')).status, 401)
    assert.equal((await api('api/files')).status, 404)
    assert.equal((await api('api/open', { method: 'GET' })).status, 404)
    assert.equal((await api('api/diagram?id=0', { headers: { 'x-reader-token': readerToken } })).status, 404)
    assert.equal((await api('api/edit')).status, 401)
    assert.equal((await api('api/edit', { method: 'POST', headers: { 'x-reader-token': readerToken, Origin: 'http://untrusted.invalid', 'Content-Type': 'application/json' }, body: '{}' })).status, 403)
    assert.equal((await api('api/source', { method: 'POST', headers: { 'x-reader-token': readerToken } })).status, 405)
    assert.match((await api('')).headers.get('content-security-policy'), /form-action 'none'/u)
    assert.equal((await api('api/edit', { method: 'POST', headers: { 'x-reader-token': readerToken, 'Content-Type': 'application/json' }, body: 'x'.repeat(2 * 1024 * 1024 + 1) })).status, 413)
    assert.equal((await api('api/document', { headers: { 'x-reader-token': readerToken, Origin: 'http://untrusted.invalid' } })).status, 403)
    await page.locator('#artifact-source').click()
    await frame.waitForSelector('#source-view[data-ready="true"]')
    await page.waitForSelector('#artifact-source.active')
    const source = await api('api/source?line=1', { headers: { 'x-reader-token': readerToken } }).then(response => response.json())
    const original = await readFile(path)
    assert.equal(source.result.text, original.subarray(source.result.start, source.result.end).toString('utf8'))
    await page.locator('#artifact-preview').click()
    await page.waitForSelector('#artifact-preview.active')
    const hasOutline = await frame.evaluate(() => readerExperiment.index.toc.length > 0)
    if (hasOutline) {
      await page.locator('#artifact-outline-toggle').click()
      await frame.locator('#reader-outline').waitFor({ state: 'visible' })
      await frame.locator('.toc-row').first().click()
      await frame.locator('#reader-outline').waitFor({ state: 'hidden' })
      await page.waitForFunction(() => !document.querySelector('#artifact-outline-toggle').classList.contains('active'))
      const query = await frame.evaluate(() => readerExperiment.index.toc[0].label)
      await page.locator('#artifact-search-toggle').click()
      await page.locator('#artifact-search-input').fill(query)
      await page.waitForFunction(() => /1 \/ \d+/u.test(document.querySelector('#artifact-search-summary').textContent))
      await page.locator('#artifact-search-next').click()
      await page.locator('#artifact-search-input').fill('')
      await page.locator('#artifact-search-toggle').click()
    }
    if (snapshot.tables) {
      const tableId = await frame.evaluate(() => readerExperiment.index.tables.reduce((best, table, id, tables) => table.rows * table.columns.length > tables[best].rows * tables[best].columns.length ? id : best, 0))
      await frame.evaluate(async id => { await readerExperiment.jump(readerExperiment.index.tables[id].firstPage, null, 0) }, tableId)
      const selector = `article[data-table="${tableId}"]`
      await frame.waitForSelector(`${selector}[data-ready]`)
      const table = frame.locator(selector)
      const cells = await table.locator('tbody td').count()
      assert.ok(cells <= 512, `Unbounded table DOM: ${cells}`)
      const next = table.getByRole('button', { name: 'Next page', exact: true })
      if (await next.isVisible()) {
        const before = await frame.evaluate(selector => ({ scroll: document.querySelector('#viewport').scrollTop, top: document.querySelector(selector).getBoundingClientRect().top, row: document.querySelector(selector).dataset.rowStart }), selector)
        await next.click()
        await frame.waitForFunction(({ selector, row }) => document.querySelector(selector)?.dataset.loading === 'false' && document.querySelector(selector)?.dataset.rowStart !== row, { selector, row: before.row })
        const after = await frame.evaluate(selector => ({ scroll: document.querySelector('#viewport').scrollTop, top: document.querySelector(selector).getBoundingClientRect().top }), selector)
        assert.ok(Math.abs(after.scroll - before.scroll) <= 2, 'Pagination moved the document viewport')
        assert.ok(Math.abs(after.top - before.top) <= 2, 'Pagination moved the table')
      }
      await table.locator('.column-picker summary').click()
      await table.getByRole('button', { name: 'Clear', exact: true }).click()
      await table.locator('.column-choices input').first().check()
      await table.locator('.column-choices input').last().check()
      const checkbox = await table.locator('.column-choices input').first().evaluate(input => {
        const style = getComputedStyle(input)
        const mark = getComputedStyle(input, '::after')
        return { width: input.getBoundingClientRect().width, height: input.getBoundingClientRect().height, padding: style.padding, left: mark.left, top: mark.top, transform: mark.transform, clip: mark.clipPath }
      })
      assert.deepEqual({ ...checkbox, clip: undefined }, { width: 16, height: 16, padding: '0px', left: '7px', top: '7px', transform: 'matrix(1, 0, 0, 1, -6, -5)', clip: undefined })
      assert.ok(checkbox.clip.startsWith('polygon('))
      await page.screenshot({ path: resolve(output, 'checkboxes.png') })
      await table.getByRole('button', { name: 'Apply', exact: true }).click()
      await frame.waitForFunction(selector => document.querySelector(selector)?.dataset.loading === 'false' && document.querySelector(selector)?.querySelectorAll('th').length === 2, selector)
      assert.ok(await table.locator('tbody td').count() <= 100)
    }
    if (snapshot.diagrams) {
      for (const id of [0, 1, 2]) {
        await frame.evaluate(async id => { const diagram = readerExperiment.index.diagrams[id]; await readerExperiment.jump(diagram.page) }, id)
        await frame.waitForSelector(`figure[data-diagram-id="${id}"][data-diagram-state="rendered"]`, { timeout: 60_000 })
      }
      const renderer = await api('api/renderer', { headers: { 'x-reader-token': readerToken } }).then(response => response.json())
      assert.equal(renderer.pid, null)
      assert.equal(renderer.stats.renders, 0)
    }
    await page.locator('#composer-input').fill('Chat stays interactive')
    if (process.argv.includes('--webkit')) {
      const { stdout } = await promisify(execFile)('/usr/bin/python3', [resolve(root, 'scripts/smoke-large-markdown-webkit.py'), base.href], { env: environment, timeout: 110_000 })
      snapshot.webkit = JSON.parse(stdout)
      if (path === fixture) {
        const nativeFile = join(directory, 'native-edit.md')
        await copyFile(fixture, nativeFile)
        const nativeMetadata = await request('/studio/review-file', { root: directory, path: nativeFile }).then(response => response.json())
        const nativeUrl = new URL(nativeMetadata.largeMarkdown.url, url)
        const { stdout: edited } = await promisify(execFile)('/usr/bin/python3', [resolve(root, 'scripts/smoke-large-markdown-webkit.py'), nativeUrl.href, '--edit'], { env: environment, timeout: 110_000 })
        snapshot.nativeEditing = JSON.parse(edited)
        const nativeBytes = await readFile(nativeFile)
        const beforeNative = await readFile(fixture)
        const firstNewline = beforeNative.indexOf(10)
        const newline = firstNewline > 0 && beforeNative[firstNewline - 1] === 13 ? '\r\n' : '\n'
        assert.deepEqual(nativeBytes, Buffer.concat([Buffer.from('Native Studio edit' + newline), beforeNative]))
        assert.equal((await request('/studio/markdown/close', { id: nativeMetadata.largeMarkdown.id })).status, 200)
      }
    }
    if (path === fixture) {
      const original = await readFile(fixture)
      const prefix = 'Studio edit 中文 😀\n'
      const firstNewline = original.indexOf(10)
      const newline = firstNewline > 0 && original[firstNewline - 1] === 13 ? '\r\n' : '\n'
      const insert = value => frame.evaluate(value => readerExperiment.editor.code.view.dispatch({ changes: { from: 0, insert: value } }), value)
      await page.locator('#artifact-edit').click()
      await frame.waitForFunction(() => marknoriaHost.state.view === 'edit' && !marknoriaHost.state.busy)
      await page.waitForSelector('#artifact-edit.active')
      await frame.locator('#edit-line').fill('1')
      await frame.locator('#edit-go button').click()
      await frame.waitForFunction(() => readerExperiment.editor.data?.window.start === 0 && !marknoriaHost.state.busy, null, { timeout: 5000 }).catch(async error => {
        console.error(JSON.stringify({ ...await frame.evaluate(() => ({ ...marknoriaHost.state, editorLoaded: Boolean(readerExperiment.editor.code), pageStart: readerExperiment.editor.data?.window.start, pageLine: readerExperiment.editor.data?.window.line, input: document.querySelector('#edit-line').value, valid: document.querySelector('#edit-line').checkValidity() })), editRequests }))
        throw error
      })
      assert.equal(await frame.locator('#edit-save').isVisible(), false)
      assert.equal(await frame.locator('#edit-reload').isVisible(), false)
      await insert(prefix)
      await page.waitForFunction(() => testState.artifact.dirty && !document.querySelector('#artifact-save').disabled)
      assert.equal(await page.evaluate(() => testState.artifact.content), undefined)
      assert.ok((await frame.evaluate(() => readerExperiment.editor.code.text.length)) < 600000)
      assert.deepEqual(await readFile(fixture), original)
      await page.locator('#artifact-source').click()
      await page.waitForSelector('#artifact-source.active')
      assert.equal(await page.evaluate(() => testState.artifact.dirty), true)
      assert.ok(!(await frame.locator('#source-text').textContent()).startsWith(prefix))
      await page.locator('#artifact-preview').click()
      await page.waitForSelector('#artifact-preview.active')
      await page.locator('#artifact-edit').click()
      await page.waitForSelector('#artifact-edit.active')
      assert.ok(await frame.evaluate(prefix => readerExperiment.editor.code.text.startsWith(prefix), prefix))
      assert.equal(await frame.locator('#edit-notice').isVisible(), false)
      await page.evaluate(() => { document.documentElement.dataset.theme = 'dark' })
      await frame.waitForFunction(() => document.documentElement.dataset.theme === 'dark')
      assert.equal(await page.evaluate(() => testState.artifact.dirty), true)
      await page.evaluate(() => { document.documentElement.dataset.theme = 'light' })
      await frame.waitForFunction(() => document.documentElement.dataset.theme === 'light')
      page.once('dialog', dialog => dialog.dismiss())
      await page.evaluate(() => controller.close({ restoreMap: false }))
      assert.equal(await page.locator('.artifact-large-markdown-frame').count(), 1)
      const first = await frame.evaluate(() => readerExperiment.index.toc[0])
      const endLine = await frame.evaluate(() => readerExperiment.editor.data.window.endLine)
      if (first && first.line <= endLine) {
        await page.locator('#artifact-outline-toggle').click()
        await frame.locator('#reader-outline').waitFor({ state: 'visible' })
        await frame.locator('.toc-row').first().click()
        await frame.waitForFunction(line => readerExperiment.editor.line() === line, first.line + 1)
        assert.equal(await page.evaluate(() => testState.artifact.dirty), true)
      }
      await page.locator('#artifact-save').click()
      await page.waitForFunction(() => !testState.artifact.dirty && !testState.artifact.saving)
      const expected = Buffer.concat([Buffer.from(prefix.replaceAll('\n', newline)), original])
      assert.deepEqual(await readFile(fixture), expected)
      await frame.locator('#edit-undo').click()
      await page.waitForFunction(() => testState.artifact.dirty)
      await frame.locator('#edit-redo').click()
      await page.waitForFunction(() => !testState.artifact.dirty)
      await frame.locator('#edit-next').click()
      await frame.waitForFunction(() => readerExperiment.editor.data.window.start > 0 && !marknoriaHost.state.busy)
      await insert('Keep on conflict\n')
      const external = Buffer.concat([Buffer.from('External change\n'), expected])
      await writeFile(fixture, external)
      await page.waitForFunction(() => testState.artifact.dirty && !testState.artifact.saving)
      await page.locator('#artifact-save').click()
      await page.waitForFunction(() => testState.artifact.dirty && !testState.artifact.saving && /changed outside/.test(document.querySelector('#artifact-hint').textContent))
      assert.deepEqual(await readFile(fixture), external)
      await page.locator('#composer-input').fill('Chat stays interactive while editing')
      await page.screenshot({ path: join(output, 'editor.png') })
      snapshot.editing = { conflictProtected: true, unsavedViews: true, closeProtected: true, boundedPage: true }
      page.once('dialog', dialog => dialog.accept())
    }
    const retained = await page.evaluate(() => { const frame = document.querySelector('.artifact-large-markdown-frame'); controller.render(); return frame === document.querySelector('.artifact-large-markdown-frame') })
    assert.equal(retained, true)
    await page.screenshot({ path: join(output, snapshot.diagrams ? 'diagrams.png' : 'real-document.png') })
    await page.evaluate(() => controller.close({ restoreMap: false }))
    const cleanupDeadline = Date.now() + 5000
    let closedStatus
    do {
      closedStatus = (await api('api/document', { headers: { 'x-reader-token': readerToken } })).status
      if (closedStatus === 410) break
      await new Promise(resolve => setTimeout(resolve, 50))
    } while (Date.now() < cleanupDeadline)
    assert.equal(closedStatus, 410)
    while ((await readdir(cache)).length && Date.now() < cleanupDeadline) await new Promise(resolve => setTimeout(resolve, 50))
    assert.deepEqual(await readdir(cache), [])
    opened.push({ ...snapshot, totalCheckMs: Date.now() - started })
  }
  const project = join(directory, 'scoped-project')
  const shared = join(directory, 'shared-documents')
  await mkdir(project)
  await mkdir(shared)
  const sharedFile = join(shared, 'readonly.md')
  const sharedText = '# Shared read only\n\n' + 'Shared content.\n\n'.repeat(70_000)
  await writeFile(sharedFile, sharedText)
  const preferences = await fetch(new URL('/studio/preferences', url), { headers: { Authorization: `Bearer ${token}` } }).then(response => response.json())
  assert.equal((await fetch(new URL('/studio/preferences', url), { method: 'PUT', headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' }, body: JSON.stringify({ ...preferences, sharedDocumentDirectories: [shared] }) })).status, 200)
  const readonly = await request('/studio/review-file', { root: project, path: sharedFile }).then(response => response.json())
  assert.equal(readonly.readOnly, true)
  const readonlyUrl = new URL(readonly.largeMarkdown.url, url)
  for (const method of ['GET', 'POST']) assert.equal((await fetch(new URL('api/edit', readonlyUrl), { method, headers: { 'x-reader-token': readonlyUrl.hash.slice(1), 'Content-Type': 'application/json' }, ...(method === 'POST' ? { body: '{}' } : {}) })).status, 403)
  assert.equal(await readFile(sharedFile, 'utf8'), sharedText)
  await page.evaluate(({ root, path }) => controller.open({ root, path }, { allowDetachedRoot: true }), { root: project, path: sharedFile })
  await page.waitForFunction(() => testState.artifact.readerReady)
  assert.equal(await page.locator('#artifact-edit').isVisible(), false)
  await page.locator('#artifact-source').click()
  await page.waitForSelector('#artifact-source.active')
  await page.locator('#artifact-preview').click()
  await page.waitForSelector('#artifact-preview.active')
  await page.evaluate(() => controller.close({ restoreMap: false }))
  assert.equal((await request('/studio/markdown/close', { id: readonly.largeMarkdown.id })).status, 200)
  if (process.platform !== 'win32') {
    const changed = join(project, 'changed.md')
    await writeFile(changed, sharedText)
    const metadata = await request('/studio/review-file', { root: project, path: changed }).then(response => response.json())
    assert.equal(metadata.readOnly, false)
    const changedUrl = new URL(metadata.largeMarkdown.url, url)
    await rename(changed, changed + '.original')
    await symlink(sharedFile, changed)
    assert.equal((await fetch(new URL('api/edit', changedUrl), { method: 'POST', headers: { 'x-reader-token': changedUrl.hash.slice(1), 'Content-Type': 'application/json' }, body: '{}' })).status, 403)
    assert.equal(await readFile(sharedFile, 'utf8'), sharedText)
    assert.equal((await request('/studio/markdown/close', { id: metadata.largeMarkdown.id })).status, 200)
  }
  assert.deepEqual(errors, [])
  console.log(JSON.stringify({ opened, sourceFidelity: true, securityBoundaries: true, cacheCleanup: true, serverChromeRequired: false }, null, 2))
} finally {
  await browser?.close()
  if (server && server.exitCode == null) { const exited = once(server, 'exit'); server.kill('SIGTERM'); await exited }
  await rm(directory, { recursive: true, force: true })
}
