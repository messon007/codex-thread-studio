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

test('session search is a middle-workspace surface with shared suggestion styling', () => {
  assert.match(html, /id="open-thread-search"/u)
  assert.match(html, /id="thread-content-search-results"[^>]*role="listbox"/u)
  assert.match(app, /addEventListener\('focus', \(\) => renderThreadContentSearch\(\)\)/u)
  assert.match(styles, /\.thread-content-search-results[^}]*var\(--shadow-md\)/u)
  assert.match(styles, /\.thread-content-search-result:hover, \.thread-content-search-result\.selected[^}]*var\(--brand-soft\)/u)
})
