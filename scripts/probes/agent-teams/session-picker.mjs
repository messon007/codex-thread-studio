// Render the actual Teams panel with synthetic catalog pages; no real sessions or model calls.
import assert from 'node:assert/strict'
import {readFile} from 'node:fs/promises'
import {fileURLToPath, pathToFileURL} from 'node:url'
import path from 'node:path'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..')
const playwright = process.env.STUDIO_PLAYWRIGHT_MODULE
if (!playwright) throw Error('Set STUDIO_PLAYWRIGHT_MODULE to an installed Playwright module')
const {chromium} = await import(pathToFileURL(playwright).href)
const source = await readFile(process.env.STUDIO_TEAMS_MODULE || path.join(root, 'ui/teams.mjs'), 'utf8')
const css = await readFile(path.join(root, 'ui/styles.css'), 'utf8')
const browser = await chromium.launch({headless: true, ...(process.env.STUDIO_CHROMIUM_PATH ? {executablePath: process.env.STUDIO_CHROMIUM_PATH} : {}), args: ['--no-sandbox']})
try {
  const page = await browser.newPage({viewport: {width: 900, height: 900}})
  const errors = []
  page.on('pageerror', e => errors.push(e.message))
  await page.route('**/*', async route => {
    const url = new URL(route.request().url())
    if (url.origin !== 'http://127.0.0.1:31987') return route.abort()
    if (url.pathname === '/teams.mjs') return route.fulfill({contentType: 'text/javascript', body: source})
    if (url.pathname === '/styles.css') return route.fulfill({contentType: 'text/css', body: css})
    if (url.pathname === '/') return route.fulfill({contentType: 'text/html', body: '<!doctype html><link rel="stylesheet" href="/styles.css"><main><header id="heading"></header><div id="menu"></div><section id="content"></section></main>'})
    return route.abort()
  })
  await page.goto('http://127.0.0.1:31987/')
  await page.evaluate(async () => {
    const {createTeamsPanel} = await import('/teams.mjs')
    const session = id => ({id, name: 'Same title', cwd: '/synthetic/project'})
    window.fixture = {calls: [], pending: null, delay: false}
    const snapshot = {enabled: true, backends: ['codex', 'other'], tasks: [], queue: [], reservations: [], registrations: [], groups: [
      {id: 'group', name: 'Fixture group', paused: true, revision: 1, members: [{id: 'member', name: 'Member', responsibility: 'Fixture', session: {backend: 'codex', thread: 'bound'}}]}
    ]}
    const request = async body => {
      window.fixture.calls.push(body)
      if (body.action === 'snapshot') return snapshot
      if (body.action !== 'sessions') throw Error('Unexpected mutation in read-only fixture')
      if (body.backend === 'other') return {data: [session('a'), session('bound')], nextCursor: null}
      if (body.cursor) {
        const result = {data: [session('a'), session('b'), session('c'), session('c'), session('bound')], nextCursor: null}
        if (window.fixture.delay) return new Promise(resolve => {window.fixture.pending = () => resolve(result)})
        return result
      }
      return {data: [session('a'), session('a'), session('b'), session('bound'), {id: ''}, {}], nextCursor: 'page-two'}
    }
    window.panel = createTeamsPanel({request}, async () => {}, {root: document.querySelector('#content'), heading: document.querySelector('#heading'), menuSlot: document.querySelector('#menu'), activate() {}, close() {}, translate: t => t, toggleMenu() {}, closeMenus() {}})
    await window.panel.open()
  })
  await page.getByRole('button', {name: 'Add member', exact: true}).click()
  const dialog = page.getByRole('dialog', {name: 'Add member', exact: true})
  const picker = dialog.getByRole('combobox', {name: 'Existing session', exact: true})
  const backend = dialog.getByRole('combobox', {name: 'Codex backend', exact: true})
  const ids = () => picker.locator('option').evaluateAll(options => options.map(o => o.value))
  await dialog.getByRole('textbox', {name: 'Display name', exact: true}).waitFor()
  assert.deepEqual(await ids(), ['a', 'b'], 'One option per ID; same-name distinct sessions survive; bound/invalid entries excluded')
  await picker.selectOption('b')
  await dialog.getByRole('button', {name: 'Load more', exact: true}).click()
  await page.waitForFunction(() => [...document.querySelectorAll('.teams-dialog option')].some(o => o.value === 'c'))
  assert.deepEqual(await ids(), ['a', 'b', 'c'], 'Overlapping catalog pages must not duplicate options')
  assert.equal(await picker.inputValue(), 'b', 'Pagination must preserve selection')
  assert.equal(await dialog.getByRole('button', {name: 'Load more', exact: true}).isVisible(), false)
  await backend.selectOption('other')
  await page.waitForFunction(() => [...document.querySelectorAll('.teams-dialog option')].some(o => o.value === 'bound'))
  assert.deepEqual(await ids(), ['a', 'bound'], 'IDs and group membership are scoped to their backend')
  await backend.selectOption('codex')
  await page.waitForFunction(() => [...document.querySelectorAll('.teams-dialog option')].some(o => o.value === 'b'))
  assert.deepEqual(await ids(), ['a', 'b'], 'Returning to a backend resets pagination and deduplication')
  await page.evaluate(() => {window.fixture.delay = true})
  await dialog.getByRole('button', {name: 'Load more', exact: true}).click()
  await page.waitForFunction(() => window.fixture.pending !== null)
  await backend.selectOption('other')
  await page.waitForFunction(() => [...document.querySelectorAll('.teams-dialog option')].some(o => o.value === 'bound'))
  await page.evaluate(() => window.fixture.pending())
  await page.waitForFunction(() => ![...document.querySelectorAll('.teams-dialog button')].find(b => b.textContent === 'Load more').hasAttribute('aria-disabled'))
  assert.deepEqual(await ids(), ['a', 'bound'], 'Late pages from a previous backend must be ignored')
  page.on('dialog', d => d.accept())
  await dialog.getByRole('button', {name: 'Cancel', exact: true}).click()
  await page.getByRole('button', {name: 'Add member', exact: true}).click()
  await dialog.getByRole('textbox', {name: 'Display name', exact: true}).waitFor()
  assert.deepEqual(await ids(), ['a', 'b'], 'A new dialog must have a fresh catalog')
  assert.deepEqual(errors, [])
  assert.equal(await page.evaluate(() => window.fixture.calls.every(c => ['snapshot', 'sessions'].includes(c.action))), true)
  console.log('PASS: same-page duplicates, overlapping pages, same-name distinct IDs, bound/invalid IDs, selection, backend scope, late responses, dialog reopen; no mutations.')
} finally {
  await browser.close()
}
