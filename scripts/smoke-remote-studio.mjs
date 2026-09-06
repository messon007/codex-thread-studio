import { spawn } from 'node:child_process'
import { mkdtemp, readFile, rm, access } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
const { chromium } = await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
// Optional integration test; build the binary and provide an installed Playwright.
const binary = process.env.STUDIO_REMOTE_BINARY || fileURLToPath(new URL('../target/debug/codex-thread-studio', import.meta.url))
const profile = await mkdtemp(join(tmpdir(), 'ssh-studio-smoke-'))
const env = { ...process.env, XDG_CONFIG_HOME:profile, XDG_DATA_HOME:join(profile,'data'), CODEX_HOME:join(profile,'codex') }
const children = []
const check = (condition, message) => { if (!condition) throw Error(message) }
function start(args) {
  const child = spawn(binary, args, {env,stdio:['ignore','pipe','pipe']})
  children.push(child)
  child.log = ''
  child.stdout.on('data', bytes => child.log += bytes)
  child.stderr.on('data', bytes => child.log += bytes)
  return child
}
function exited(child) { return new Promise((resolve,reject) => {child.on('exit',resolve);child.on('error',reject)}) }
async function ready(child) {
  const deadline = Date.now()+15000
  while (Date.now()<deadline) {
    const url = child.log.match(/http:\/\/127\.0\.0\.1:\d+\/#token=[a-f0-9]{32}/)?.[0]
    if (url) return new URL(url)
    if (child.exitCode !== null) throw Error(`server startup failed: ${child.log}`)
    await new Promise(resolve => setTimeout(resolve,50))
  }
  throw Error('server readiness timeout')
}
let browser
try {
  const server = start(['--serve','--listen','127.0.0.1:0'])
  const url = await ready(server)
  const settingsPath = join(profile,'codex-thread-studio/settings.json')
  const settings = await readFile(settingsPath,'utf8')
  await access(join(profile,'codex-thread-studio/studio.sqlite3'))
  for (const args of [['--serve','--listen','127.0.0.1:0'], []]) {
    const duplicate = start(args)
    const code = await exited(duplicate)
    check(code === 2 && duplicate.log.includes('already in use'), 'duplicate desktop/server was not rejected')
  }
  check(await readFile(settingsPath,'utf8') === settings, 'rejected instance changed settings')
  const unauth = await fetch(new URL('/studio/session-state',url))
  check(unauth.status === 401, 'API accepts missing credentials')
  browser = await chromium.launch({headless:true,executablePath:process.env.STUDIO_CHROMIUM_PATH})
  const context = await browser.newContext()
  // Exercise the actual served bootstrap and auth API without loading app.js,
  // which would connect to and start backend sessions.
  await context.route(`${url.origin}/`, route => route.fulfill({contentType:'text/html',body:'<script src="/remote-bootstrap.js"></script><button id="link">Link</button>'}))
  const page = await context.newPage()
  await page.goto(url.href)
  const authStatus = () => page.evaluate(async () => (await fetch('/studio/session-state',{headers:{Authorization:`Bearer ${window.__CODEX_THREAD_STUDIO_GATEWAY__.token}`}})).status)
  check(await authStatus() === 200, 'initial authentication failed')
  check(!page.url().includes('token'), 'token was not cleared')
  await page.reload()
  check(await authStatus() === 200, 'reload authentication failed')
  // Import real embedded modules in the browser, not just local test files.
  // Fake adapters exercise dispatch without starting paid/backend sessions.
  await page.evaluate(async () => {
    const { installBackendRegistry, backendDescriptor } = await import('/backends.mjs')
    const { SessionDispatchRegistry } = await import('/session-dispatch.mjs')
    const { mergeCatalogMetadata } = await import('/session-catalog.mjs')
    const { normalizeStoredTurnOptions, sessionModelPreferencePayload } = await import('/session-model-preferences.mjs')
    const { codexLifecycleStreamMessage, claimLifecycleNotification } = await import('/codex-lifecycle-diagnostics.mjs')
    const { storeCachedSession, routeCodexNotification } = await import('/session-model-cache.mjs')
    const { createCodexViewModel, applyCodexNotification } = await import('/codex-native.mjs')
    const { applyOpenCodeEvent } = await import('/opencode-native.mjs')
    const { createCodexHistoryLoader } = await import('/codex-history-loader.mjs')
    const { coordinateHistoryLoad } = await import('/history-load-coordinator.mjs')
    const check = (value, message) => { if (!value) throw Error(message) }
    installBackendRegistry([{ id: 'ept-codex', kind: 'codex' }])
    check(backendDescriptor('ept-codex').kind === 'codex', 'configured backend missing')
    const registry = new SessionDispatchRegistry()
    let preparations = 0
    for (const backend of ['codex', 'ept-codex', 'opencode']) {
      registry.register(backend, {
        read: ref => ref.key,
        prepareTurn: () => { preparations += 1 },
        startTurn: ref => ref.key,
      })
      const ref = { backend, id: 'same-id' }
      await registry.prepareTurn(ref)
      await registry.prepareTurn(ref)
      check(registry.startTurn(ref, 'hello') === `${backend}:same-id`, 'dispatch crossed backend')
    }
    check(preparations === 3, 'preparation deduplication failed')
    check(mergeCatalogMetadata('codex', { cwd: '/current' }, { cwd: '/old' }).cwd === '/current', 'catalog metadata changed')
    const preferences = normalizeStoredTurnOptions({ 'ept-codex:one': { model: 'saved-model' } })
    check(sessionModelPreferencePayload('ept-codex:one', preferences['ept-codex:one']).model === 'saved-model', 'model preference route failed')
    const notification = { method: 'turn/completed', params: { threadId: 'one', turnId: 'turn' } }
    const envelope = codexLifecycleStreamMessage({ method: 'studio/codexLifecycle/event', params: { backend: 'ept-codex', message: notification } })
    check(envelope?.type === 'event' && envelope.backend === 'ept-codex', 'lifecycle decoding failed')
    check(claimLifecycleNotification('ept-codex', notification, new Map(), new Map(), () => 0), 'lifecycle claim failed')
    const cache = new Map()
    const model = { threadId: 'one', activeTurnId: 'turn', turns: [{ id: 'turn' }] }
    storeCachedSession(cache, 'ept-codex', 'one', model, null, 1)
    check(routeCodexNotification(notification, {
      backend: 'ept-codex', selectedBackend: 'codex', selectedId: 'other', selectedModel: {}, cache,
      hiddenThreads: new Set(), hiddenTurns: new Set(),
      sessionKey: (backend, id) => `${backend}:${id}`, turnKey: (backend, id) => `${backend}:${id}`,
    }) === model, 'background notification routing failed')
    installBackendRegistry()
    const view = createCodexViewModel()
    applyCodexNotification(view, { method: 'turn/started', params: { turn: { id: 'turn', status: 'inProgress', items: [] } } })
    check(view.activeTurnId === 'turn', 'Codex reducer failed')
    applyOpenCodeEvent(view, { type: 'session.idle', properties: { sessionID: 'one' } }, 'one')
    check(view.activeTurnId === null && view.status === 'idle', 'OpenCode reducer route failed')
    const history = createCodexHistoryLoader({
      historyTailCapability: () => false,
      requestCodexResume: async () => ({ thread: { id: 'one', turns: [] } }),
    })
    check((await history.loadCodexHistoryForSelection('codex', 'one', null)).historyMode === 'full', 'history loader failed')
    const flights = new Map()
    const flight = coordinateHistoryLoad(flights, 'codex:one', 'codex', null, async () => 'ready')
    check(coordinateHistoryLoad(flights, 'codex:one', 'codex', null, async () => 'duplicate') === flight, 'history flight not shared')
    check(await flight === 'ready' && flights.size === 0, 'history flight cleanup failed')
  })
  const native = await context.newPage()
  await native.addInitScript(() => Object.defineProperty(window,'__CODEX_THREAD_STUDIO_GATEWAY__',{value:Object.freeze({token:'native-credential',hostPlatform:'linux'}),configurable:false}))
  await native.goto(url.origin)
  check(await native.evaluate(() => window.__CODEX_THREAD_STUDIO_GATEWAY__.token === 'native-credential' && !window.__CODEX_THREAD_STUDIO_GATEWAY__.remote), 'desktop credentials overwritten')
  const stopped = exited(server)
  server.kill('SIGKILL')
  await stopped
  const restarted = start(['--serve','--listen','127.0.0.1:0'])
  await ready(restarted)
  console.log('PASS: headless startup, current studio DB, HTTP auth, reload auth, embedded TypeScript modules and backend dispatch, native bootstrap preservation, duplicate server/desktop exclusion, settings unchanged, lock released after SIGKILL')
} finally {
  await browser?.close()
  for (const child of children) if (child.exitCode === null && child.signalCode === null) { const done=exited(child);child.kill('SIGKILL');await done }
  await rm(profile,{recursive:true,force:true})
}
