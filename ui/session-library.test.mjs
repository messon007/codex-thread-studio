import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
const app = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
const sessionManagement = readFileSync(new URL('./session-management.mjs', import.meta.url), 'utf8')
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
  assert.match(sessionManagement, /'thread\/list',[\s\S]*archived: true/u)
  assert.match(sessionManagement, /rpc\('thread\/unarchive'/u)
  assert.match(sessionManagement, /BACKEND_IDS\.filter\(isCodexBackend\)/u)
})

test('archived previews disable per-turn forks as well as header actions', () => {
  assert.match(app, /forkable: !isArchivedPreview\(\) && isTurnForkable/u)
  assert.match(app, /if \(forkButton\) \{\s+if \(isArchivedPreview\(\)\) return/u)
  assert.match(app, /if \(!sourceThreadId \|\| isArchivedPreview\(\)\) return/u)
})

test('archive loading retains per-backend failures and pagination state', () => {
  assert.match(sessionManagement, /errorsByBackend: \{\}/u)
  assert.match(sessionManagement, /const nextCursors = \{ \.\.\.library\.nextCursors \}/u)
  assert.match(sessionManagement, /errorsByBackend\[backend\] = error\?\.message/u)
  assert.match(sessionManagement, /archive\.load\(\{ backends: failed\.length \? failed : null \}\)/u)
  assert.match(app, /socket\.onclose = \(\) => finish\(new Error\(t\('The \{backend\} App Server connection closed'/u)
})

test('restored sessions are installed before the bounded live catalog reload', () => {
  const restore = sessionManagement.slice(
    sessionManagement.indexOf('async restore()'),
    sessionManagement.indexOf('markStale('),
  )
  assert.ok(restore.indexOf('installBackendCatalog(selected.backend') < restore.indexOf('archive.close({ restoredId:'))
})

test('session search is a middle-workspace surface with shared suggestion styling', () => {
  assert.match(html, /id="open-thread-search"/u)
  assert.match(html, /id="thread-content-search-results"[^>]*role="listbox"/u)
  assert.match(sessionManagement, /addEventListener\('focus', \(\) => search\.render\(\)\)/u)
  assert.match(styles, /\.thread-content-search-results[^}]*var\(--shadow-md\)/u)
  assert.match(styles, /\.thread-content-search-result:hover, \.thread-content-search-result\.selected[^}]*var\(--brand-soft\)/u)
})

test('session search prefers backend results and Escape closes the popup first', () => {
  const search = sessionManagement.slice(
    sessionManagement.indexOf('async perform()'),
    sessionManagement.indexOf('render(error ='),
  )
  assert.ok(search.indexOf("rpc('thread/searchOccurrences'") < search.indexOf('localSessionOccurrences(state.model, query)'))
  assert.match(search, /normalizeRemoteSessionOccurrences/u)
  assert.match(sessionManagement, /if \(!results\.classList\.contains\('hidden'\)\) \{\s+search\.hideResults\(\)\s+return/u)
})
