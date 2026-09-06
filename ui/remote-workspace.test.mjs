import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import vm from 'node:vm'

const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
const bootstrap = readFileSync(new URL('./remote-bootstrap.js', import.meta.url), 'utf8')
  .replace('__STUDIO_HOST_PLATFORM__', '"linux"')

function loadPage(url, storage = new Map(), { native = null, blockedStorage = false } = {}) {
  const window = {
    location: new URL(url),
    sessionStorage: {
      getItem(key) { if (blockedStorage) throw Error('storage blocked'); return storage.get(key) },
      setItem(key, value) { if (blockedStorage) throw Error('storage blocked'); storage.set(key, value) },
    },
    history: { replaceState(_state, _title, value) { window.location = new URL(value, window.location) } },
  }
  if (native) Object.defineProperty(window, '__CODEX_THREAD_STUDIO_GATEWAY__', { value: native })
  vm.runInNewContext(bootstrap, { window, URLSearchParams })
  return window
}

test('remote authentication survives reload and a new server token replaces the old one', () => {
  const storage = new Map()
  const token = 'a'.repeat(32)
  const first = loadPage(`http://127.0.0.1:38080/?view=chat#token=${token}`, storage)
  assert.equal(first.__CODEX_THREAD_STUDIO_GATEWAY__.token, token)
  assert.equal(first.location.href, 'http://127.0.0.1:38080/?view=chat')
  assert.equal(loadPage(first.location.href, storage).__CODEX_THREAD_STUDIO_GATEWAY__.token, token)
  const nextToken = 'b'.repeat(32)
  const replacement = loadPage(`http://127.0.0.1:38080/?token=${nextToken}`, storage)
  assert.equal(loadPage(replacement.location.href, storage).__CODEX_THREAD_STUDIO_GATEWAY__.token, nextToken)
  assert.equal(loadPage(replacement.location.href).__CODEX_THREAD_STUDIO_GATEWAY__, undefined)
})

test('remote bootstrap preserves desktop credentials and rejects malformed credentials', () => {
  const native = Object.freeze({ token: 'desktop', hostPlatform: 'linux' })
  const storage = new Map()
  const page = loadPage('http://127.0.0.1:38080/', storage, { native })
  assert.equal(page.__CODEX_THREAD_STUDIO_GATEWAY__, native)
  assert.equal(storage.size, 0)
  assert.equal(loadPage('http://127.0.0.1:38080/?token=invalid').__CODEX_THREAD_STUDIO_GATEWAY__, undefined)
})

test('storage-disabled browsers can reload using a fragment without retaining a query token', () => {
  const first = loadPage(`http://127.0.0.1:38080/?token=${'c'.repeat(32)}`, new Map(), { blockedStorage: true })
  assert.equal(first.location.search, '')
  assert.match(first.location.hash, /^#token=/u)
  assert.equal(loadPage(first.location.href, new Map(), { blockedStorage: true }).__CODEX_THREAD_STUDIO_GATEWAY__.token, 'c'.repeat(32))
})

test('remote link opening does not misinterpret noopener null as a blocked popup', async () => {
  const start = source.indexOf('async function openBrowserUrl(')
  const end = source.indexOf('\nasync function openSessionResource(', start)
  assert.ok(start >= 0 && end > start)
  const calls = []
  const context = vm.createContext({
    state: { remoteClient: true }, usesEmbeddedBrowser: () => false,
    t: value => value, window: { open: (...args) => { calls.push(args); return null } },
  })
  vm.runInContext(source.slice(start, end), context)
  await vm.runInContext("openBrowserUrl('https://example.com')", context)
  assert.deepEqual(calls, [['https://example.com', '_blank', 'noopener,noreferrer']])
})

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
