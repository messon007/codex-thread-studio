import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')

test('remote browser bootstrap runs before the Studio application module', () => {
  const bootstrap = html.indexOf('src="/remote-bootstrap.js"')
  const application = html.indexOf('type="module" src="/app.js"')
  assert.ok(bootstrap >= 0)
  assert.ok(application > bootstrap)
})

test('remote clients identify the workspace and open URL resources in a browser tab', () => {
  assert.match(source, /remoteClient: Boolean\(window\.__CODEX_THREAD_STUDIO_GATEWAY__\?\.remote\)/u)
  assert.match(source, /state\.remoteClient \? t\('Remote workspace'\) : t\('Local workspace'\)/u)
  assert.match(source, /window\.open\(String\(url \|\| ''\), '_blank', 'noopener,noreferrer'\)/u)
})
