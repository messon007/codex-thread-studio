import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
const app = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8')

test('archived sessions live in the Local Workspace menu rather than primary filters', () => {
  const studioMenu = html.match(/<div id="studio-menu"[\s\S]*?<\/div>\s*<\/div>\s*<\/footer>/u)?.[0] || ''
  const filters = html.match(/<div class="filters thread-filters"[\s\S]*?<\/div>/u)?.[0] || ''
  assert.match(studioMenu, /id="open-archived-sessions"/u)
  assert.doesNotMatch(filters, /archiv/iu)
})

test('archived preview exposes one text-only restore action', () => {
  assert.equal((html.match(/id="restore-archived-session"/gu) || []).length, 1)
  const restore = html.match(/<button id="restore-archived-session"[\s\S]*?<\/button>/u)?.[0] || ''
  assert.match(restore, />Restore session<\/button>/u)
  assert.doesNotMatch(restore, /<svg/iu)
})

test('archive catalog and restore use native Codex protocol methods', () => {
  assert.match(app, /'thread\/list',[\s\S]*archived: true/u)
  assert.match(app, /rpc\('thread\/unarchive'/u)
  assert.match(app, /BACKEND_IDS\.filter\(isCodexBackend\)/u)
})

test('archived previews disable per-turn forks as well as header actions', () => {
  assert.match(app, /forkable: !isArchivedPreview\(\) && isTurnForkable/u)
  assert.match(app, /if \(forkButton\) \{\s+if \(isArchivedPreview\(\)\) return/u)
  assert.match(app, /if \(!sourceThreadId \|\| isArchivedPreview\(\)\) return/u)
})

test('archive loading retains per-backend failures and pagination state', () => {
  assert.match(app, /errorsByBackend: \{\}/u)
  assert.match(app, /const nextCursors = \{ \.\.\.state\.sessionLibrary\.nextCursors \}/u)
  assert.match(app, /errorsByBackend\[backend\] = error\?\.message/u)
  assert.match(app, /loadArchivedSessions\(\{ backends: failed\.length \? failed : null \}\)/u)
  assert.match(app, /socket\.onclose = \(\) => finish\(new Error\(t\('The \{backend\} App Server connection closed'/u)
})

test('restored sessions are installed before the bounded live catalog reload', () => {
  const restore = app.slice(
    app.indexOf('async function restoreArchivedSession('),
    app.indexOf('function renderThreadList('),
  )
  assert.ok(restore.indexOf('installBackendCatalog(selected.backend') < restore.indexOf('closeArchivedSessions({ restoredId:'))
})

test('session search is a middle-workspace surface with shared suggestion styling', () => {
  assert.match(html, /id="open-thread-search"/u)
  assert.match(html, /id="thread-content-search-results"[^>]*role="listbox"/u)
  assert.match(app, /addEventListener\('focus', \(\) => renderThreadContentSearch\(\)\)/u)
  assert.match(styles, /\.thread-content-search-results[^}]*var\(--shadow-md\)/u)
  assert.match(styles, /\.thread-content-search-result:hover, \.thread-content-search-result\.selected[^}]*var\(--brand-soft\)/u)
})

test('session search prefers backend results and Escape closes the popup first', () => {
  const search = app.slice(
    app.indexOf('async function performThreadContentSearch('),
    app.indexOf('function filteredThreadSearchEntries('),
  )
  assert.ok(search.indexOf("rpc('thread/searchOccurrences'") < search.indexOf('localSessionOccurrences(state.model, query)'))
  assert.match(search, /normalizeRemoteSessionOccurrences/u)
  assert.match(app, /if \(!results\.classList\.contains\('hidden'\)\) \{\s+hideThreadContentSearchResults\(\)\s+return/u)
})
