import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')

test('Browser has one global entry and no session or right-rail duplicate', () => {
  assert.equal((html.match(/id="open-browser-workspace"/g) || []).length, 1)
  const menu = html.match(/<div id="studio-menu"[\s\S]*?<\/div>/)?.[0] || ''
  assert.match(menu, /id="open-browser-workspace"/)
  assert.doesNotMatch(html, /id="browser-workspace-rail"/)
  assert.doesNotMatch(html, /class="[^\"]*browser-entry-button/)
})

test('Browser status is local to the embedded runtime without an external event stream or polling', () => {
  assert.doesNotMatch(source, /\/studio\/browser\/events/)
  assert.doesNotMatch(source, /\/studio\/browser\/workspace/)
  assert.doesNotMatch(source, /setInterval\([^\n]*[Bb]rowser/)
  assert.doesNotMatch(source, /browserWorkspacePoll/)
})

test('A global Browser click only shows the embedded presentation', () => {
  assert.match(source, /usesEmbeddedBrowser\(\)/)
  assert.match(source, /studio-action:\/\/show-browser/)
  assert.doesNotMatch(source, /launchGlobalBrowser/)
  assert.doesNotMatch(source, /activateFirstBrowserPage/)
})

test('Embedded Browser selection enters the shared Comment Core flow', () => {
  assert.match(source, /browserCommentSource\(\{/)
  assert.match(source, /openEmbeddedBrowserComment\(selection\)/)
  assert.match(source, /openAnnotationFromSelection\(\)/)
  assert.match(source, /window\.__studioEmbeddedBrowser = Object\.freeze/)
  assert.match(source, /setWidth\(metrics\)/)
  assert.match(source, /embeddedBrowserWidthTimer = setTimeout\(persistPreferences, 250\)/)
})

test('Browser navigation controls use consistently aligned vector icons', () => {
  const toolbar = readFileSync(new URL('./embedded-browser.html', import.meta.url), 'utf8')
  assert.match(toolbar, /\.icon \{[^}]*display: grid; place-items: center;/)
  assert.match(toolbar, /data-action="back"[^>]*aria-label="网页后退"[^>]*><svg/)
  assert.match(toolbar, /data-action="forward"[^>]*><svg/)
  assert.match(toolbar, /data-action="reload"[^>]*><svg/)
})
