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

test('Browser status is driven by the protected event stream without polling', () => {
  assert.match(source, /gatewayEventSource\('\/studio\/browser\/events'\)/)
  assert.doesNotMatch(source, /setInterval\([^\n]*[Bb]rowser/)
  assert.doesNotMatch(source, /browserWorkspacePoll/)
})

test('A global Browser click validates live state before activating a tab', () => {
  const action = source.match(/async function openGlobalBrowser\(\) \{([\s\S]*?)\n\}/)?.[1] || ''
  assert.match(action, /browserRequest\('\/studio\/browser\/workspace'\)/)
  assert.match(action, /activateFirstBrowserPage/)
})

test('Opening Local workspace refreshes the visible Browser status', () => {
  assert.match(source, /if \(opening\) refreshGlobalBrowserStatus\(\)/)
  assert.match(source, /async function refreshGlobalBrowserStatus\(\)/)
})
