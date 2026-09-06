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
    const check = (value, message) => { if (!value) throw Error(message) }
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
    const { TranscriptPresentationCache } = await import('/transcript-presentation.mjs')
    const { transcriptRestorePlan } = await import('/transcript-scroll.mjs')
    const { createTranscriptDom } = await import('/transcript-dom.mjs')
    const { filterCatalogEntries } = await import('/thread-catalog.mjs')
    const { localSessionOccurrences } = await import('/session-search.mjs')
    const { createSerializedStateWriter } = await import('/serialized-state-writer.mjs')
    const { CommentSourceRegistry, createCommentDraft } = await import('/comment-core.mjs')
    const { CommentSubmissionCoordinator } = await import('/comment-submission.mjs')
    const { waitForUtilityResult } = await import('/utility-task.mjs')
    const { executeQueuedMessage } = await import('/queue-execution.mjs')
    check(await executeQueuedMessage({ messageQueues: {}, runningMessageQueues: new Set(), pausedMessageQueues: new Set(), messageQueueErrors: new Map() }, 'empty', {}) === false, 'queue execution module failed')
    check((await waitForUtilityResult({ read: async () => ({ status: 'completed', prompt: 'fixture' }), ensureCurrent() {}, intervalMs: 100, timeoutMs: 1000, timeoutMessage: 'timeout', errorMessage: text => text })).prompt === 'fixture', 'utility task module failed')
    const { createChatCommentProvider, createDocumentCommentProvider } = await import('/comment-source-providers.mjs')
    const { createBrowserCommentProvider } = await import('/browser-comment-provider.mjs')
    const { createPdfCommentProvider } = await import('/pdf-comment-provider.mjs')
    const { createEpubCommentProvider } = await import('/epub-comment-provider.mjs')
    const { createTableCommentProvider } = await import('/table-comment-provider.mjs')
    const { locateCommentIntervals } = await import('/comment-markers.mjs')
    const { parseRouterDecision } = await import('/thread-router.mjs')
    const { RouterTurnCoordinator } = await import('/router-coordination.mjs')
    const { normalizeSessionMap, safeAssistantOperations, SessionMapWorkerPool } = await import('/session-map.mjs')
    const { SessionMapCoordinator } = await import('/session-map-coordination.mjs')
    const comments = new CommentSourceRegistry()
    for (const factory of [createChatCommentProvider, createDocumentCommentProvider, createBrowserCommentProvider, createPdfCommentProvider, createEpubCommentProvider, createTableCommentProvider]) comments.register(factory())
    const comment = createCommentDraft({ excerpt: 'fixture', source: { provider: 'chat', anchor: {} } }, { registry: comments })
    const commentStore = { annotationDrafts: { fixture: [comment] }, annotationAdditional: {} }
    const commentSend = await new CommentSubmissionCoordinator().submit({ key: 'fixture', store: commentStore, prompt: 'fixture', composerText: '', insert() {}, send: async () => false, persist() { throw Error('failed send persisted a clear') } })
    check(commentSend === 'retained' && commentStore.annotationDrafts.fixture.length === 1, 'failed comment send lost drafts')
    check(locateCommentIntervals('a fixture', [comment])[0].start === 2, 'comment source/marker modules failed')
    check(parseRouterDecision(JSON.stringify({ action: 'dispatch', targetSessionKey: 'codex:fixture', forwardedPrompt: 'hello' }), ['codex:fixture']).forwardedPrompt === 'hello', 'Router decision module failed')
    const routerFlow = new RouterTurnCoordinator({ pending: new Map(), dispatches: new Map(), targetTurns: new Map() })
    check(await routerFlow.start('fixture', async () => 'ready', 'busy') === 'ready', 'Router coordination module failed')
    check(normalizeSessionMap({ id: 'map', backend: 'codex', threadId: 'fixture' }).revision === 0, 'Session Map normalization failed')
    check(safeAssistantOperations({ operations: [{ op: 'setState', itemId: 'x', state: 'done' }] }).length === 0, 'unsafe Map operation accepted')
    check(await new SessionMapWorkerPool().enqueue('fixture', () => 42) === 42, 'Map worker result lost')
    const mapFlow = new SessionMapCoordinator({ sessionMaps: new Map(), sessionMapLoads: new Map(), sessionMapSync: new Map() }, () => {})
    check((await mapFlow.load('fixture', async () => ({ id: 'map', backend: 'codex', threadId: 'fixture' }))).id === 'map', 'Map coordination module failed')
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
    const transcript = { turns: Array.from({ length: 35 }, (_, index) => ({ id: `t${index}`, status: 'completed', items: [{ id: `i${index}`, type: 'agentMessage', text: 'browser fixture' }] })) }
    const presentation = new TranscriptPresentationCache()
    check(presentation.get('codex:fixture', transcript).visibleStart === 5, 'bounded transcript window changed')
    check(presentation.restoreTurn('codex:fixture', transcript, 't0').visibleStart === 0, 'earlier-turn restore failed')
    check(transcriptRestorePlan({ saved: { anchorTurnId: 't0' }, orderedTurnIds: ['t0'] }).type === 'anchor', 'scroll plan failed')
    const container = document.createElement('div')
    document.body.append(container)
    const dom = createTranscriptDom()
    dom.render(container, 'fixture', [{ id: 'turn', html: '<p>fixture</p>' }])
    const firstNode = container.firstChild
    dom.render(container, 'fixture', [{ id: 'turn', html: '<p>fixture</p>' }])
    check(container.firstChild === firstNode, 'unchanged transcript DOM was replaced')
    container.remove()
    check(localSessionOccurrences(transcript, 'browser').length === 35, 'session occurrence search failed')
    check(filterCatalogEntries({ codex: [{ id: 'one', status: 'active' }] }, { filter: 'active' }).length === 1, 'catalog filtering failed')
    const writes = []
    const writer = createSerializedStateWriter(async (path, request) => { writes.push({ path, ...request }); return { ok: true, status: 200 } }, () => {})
    await writer.write('/fake-state-only', { sessionKey: 'codex:fixture' })
    check(JSON.parse(writes[0].body).sessionKey === 'codex:fixture', 'state writer snapshot failed')
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
