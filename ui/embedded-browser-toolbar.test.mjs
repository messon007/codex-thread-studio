import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const source = readFileSync(new URL('./embedded-browser.html', import.meta.url), 'utf8')

test('embedded browser tab strip follows normal browser control order', () => {
  const tabs = source.indexOf('id="tabs"')
  const newTab = source.indexOf('class="new-tab"')
  const closeBrowser = source.indexOf('class="workspace-close"')

  assert.ok(tabs >= 0)
  assert.ok(newTab > tabs, 'new-tab belongs immediately after the tab list')
  assert.ok(closeBrowser > newTab, 'workspace close remains at the far right')
  assert.match(source.slice(newTab, closeBrowser), /data-action="new-tab"/u)
  assert.match(source.slice(closeBrowser), /data-action="toggle-browser"/u)
  assert.match(source, /\.tabs \{[^}]*flex: 0 1 auto;/u)
  assert.match(source, /\.workspace-close \{[^}]*margin: 0 0 1px auto;/u)
})
