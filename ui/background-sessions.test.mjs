import assert from 'node:assert/strict'
import test from 'node:test'
import { createBackgroundSessions } from './background-sessions.mjs'
import { createCodexViewModel, hydrateCodexThread } from './codex-native.mjs'
import { codexLifecycleEvent } from './codex-lifecycle-diagnostics.mjs'

function fixture() {
  const calls = []
  const model = createCodexViewModel()
  const state = {
    backend: 'codex', selectedId: 'visible', ready: true, model,
    threadModels: new Map(), threadsByBackend: {}, appServerGenerations: {}, backendStates: {},
    selectedByBackend: {}, pinnedSessions: new Set(), annotationDrafts: {}, annotationAdditional: {},
    openingMessages: {}, turnOptions: {}, messageQueues: {}, pausedMessageQueues: new Set(), messageQueueErrors: new Map(),
  }
  const record = (name) => (...args) => calls.push([name, ...args])
  const services = {
    $: () => ({ open: false }), getBackendIds: () => ['codex', 'ept-codex'],
    codexBackendsNeedingRestartRecovery: new Set(), isCodexBackend: (b) => b !== 'opencode',
    backendDescriptor: (name) => ({ name }), sessionDispatch: { clearPrepared: record('clearPrepared') },
    clearStartedThreadsForBackend: record('clearStarted'), reportSessionLifecycle: record('log'),
    refreshBackendCatalog: async () => {}, threadStatus: (t) => t?.status,
    threadCatalogKey: (b, id) => `${b}:${id}`, sessionRefKey: (b, id) => `${b}:${id}`,
    cachedThreadModel: (b, id) => state.threadModels.get(`${b}:${id}`),
    cacheThreadModel: (b, id, m) => state.threadModels.set(`${b}:${id}`, { model: m, validatedAt: 1 }),
    mergeThreadIntoCatalog: record('merge'), handleQueuedTurnCompletion: record('queue'),
    markCachedModelUnvalidated: record('unvalidate'), markCachedModelValidated: record('validate'),
    claimCodexLifecycleNotification: (_, msg) => codexLifecycleEvent(msg),
    captureStructuredUtilityNotification: () => false, codexLifecycleThreadId: (_, e) => e.threadId,
    scheduleStartedThreadCatalogConfirmation: record('confirm'), updateCodexCatalogActivity: record('catalog'),
    observeCodexTurnLatency: () => {}, codexNotificationModel: (_, b) => state.threadModels.get(`${b}:a`)?.model,
    reportCodexLifecycleNotification: record('notification'), sessionResources: { invalidate: record('resources') },
    notifyDesktop: record('notify'), t: (s) => s, threadTitle: (t) => t.name,
    threadRouter: { completeTurn: async () => {}, removeSession: () => false },
    transcriptUpdateKind: () => 'full', replaceRenderedTurn: () => false,
    renderTranscript: record('render'), renderComposerState: record('composer'), renderWorkspace: record('workspace'),
    sessionMap: { processInlineUpdate: async () => {} },
    openCodeEventThreadId: (e) => e.properties?.sessionID || '', bufferOpenCodeHistoryEvent: record('buffer'),
    openCodeLoopGuard: { observe: () => null }, hiddenUtilityThread: () => false,
    scheduleOpenCodeListRefresh: record('list'), openCodeCompletionSignal: () => false,
    updateOpenCodeCatalogActivity: record('catalog'),
    forgetStartedThread: record('forget'), deletePersistedSessionState: record('delete'),
    discardComposerSessionState: record('discard'), invalidateThreadModel: record('invalidate'),
  }
  return { state, services, calls, controller: () => createBackgroundSessions(state, services) }
}
function running() {
  const m = createCodexViewModel()
  hydrateCodexThread(m, { id: 'a', turns: [{ id: 't', status: 'inProgress', items: [] }] })
  return m
}
const completed = { method: 'turn/completed', params: { threadId: 'a', turn: { id: 't', status: 'completed' } } }

test('offscreen same-backend completion updates the cached model without rendering the visible session', () => {
  const f = fixture(), m = running()
  f.state.threadModels.set('codex:a', { model: m })
  assert.equal(f.controller().handleCodexLifecycleNotification('codex', completed), true)
  assert.equal(m.activeTurnId, null)
  assert.equal(m.status, 'idle')
  assert.ok(f.calls.some(([n]) => n === 'validate'))
  assert.ok(f.calls.some(([n]) => n === 'queue'))
  assert.equal(f.calls.some(([n]) => ['render', 'composer'].includes(n)), false)
})

test('hidden utility notifications do not update the visible catalog or models', () => {
  const f = fixture()
  f.services.captureStructuredUtilityNotification = () => true
  f.controller().handleCodexLifecycleNotification('ept-codex', completed)
  assert.deepEqual(f.calls, [])
})

test('background history is discarded if the cache instance changes during the request', async () => {
  const f = fixture(), m = running()
  f.state.threadModels.set('ept-codex:a', { model: m })
  let resolve
  f.services.loadCodexHistoryForBackground = () => new Promise((r) => { resolve = r })
  const pending = f.controller().refreshOffscreenCodexHistoryAfterCompletion('ept-codex', 'a', m)
  const replacement = running()
  f.state.threadModels.set('ept-codex:a', { model: replacement })
  resolve({ result: { thread: { id: 'a', turns: [] } }, historyMode: 'tail' })
  await pending
  assert.equal(f.state.threadModels.get('ept-codex:a').model, replacement)
  assert.equal(f.calls.some(([n]) => n === 'merge'), false)
})

test('background history cannot replace a session selected while its request was in flight', async () => {
  const f = fixture(), m = running()
  f.state.threadModels.set('ept-codex:a', { model: m })
  let resolve
  f.services.loadCodexHistoryForBackground = () => new Promise((r) => { resolve = r })
  const pending = f.controller().refreshOffscreenCodexHistoryAfterCompletion('ept-codex', 'a', m)
  f.state.backend = 'ept-codex'
  f.state.selectedId = 'a'
  f.state.model = m
  resolve({ result: { thread: { id: 'a', turns: [] } }, historyMode: 'tail' })
  await pending
  assert.equal(f.state.model, m)
  assert.equal(f.calls.some(([n]) => n === 'merge'), false)
})

test('selected completion still renders and clears the active turn', () => {
  const f = fixture(), m = running()
  f.state.model = m
  f.state.selectedId = 'a'
  f.state.threadModels.set('codex:a', { model: m })
  f.controller().handleCodexLifecycleNotification('codex', completed)
  assert.equal(m.activeTurnId, null)
  assert.ok(f.calls.some(([n]) => n === 'render'))
  assert.ok(f.calls.some(([n]) => n === 'composer'))
})

test('OpenCode history failure always releases its event buffer', async () => {
  const f = fixture(), events = []
  f.services.getOpenCodeHistoryEpoch = () => 9
  f.services.beginOpenCodeHistoryEventBuffer = () => events
  f.services.endOpenCodeHistoryEventBuffer = (...args) => f.calls.push(['release', ...args])
  f.services.sessionDispatch.read = async () => { throw Error('read failed') }
  await assert.rejects(f.controller().ensureSessionModel({ backend: 'opencode', id: 'a' }), /read failed/)
  assert.deepEqual(f.calls, [['release', 'a', events]])
})

test('reconciliation installs completed background history and advances only its queue', async () => {
  const f = fixture(), m = running()
  f.state.threadModels.set('ept-codex:a', { model: m })
  f.state.threadsByBackend['ept-codex'] = [{ id: 'a', status: 'idle' }]
  f.services.loadCodexHistoryForBackground = async () => ({ result: { thread: { id: 'a', turns: [{ id: 't', status: 'completed', items: [] }] } }, historyMode: 'tail' })
  await f.controller().reconcileCodexLifecycleBackend('ept-codex')
  assert.equal(f.state.threadModels.get('ept-codex:a').model.status, 'idle')
  assert.deepEqual(f.calls.find(([n]) => n === 'queue'), ['queue', { backend: 'ept-codex', id: 'a' }, 'completed'])
  assert.equal(f.calls.some(([n]) => n === 'render'), false)
})

test('background restart invalidates only the restarted backend caches and preparation', () => {
  const f = fixture()
  for (const b of ['codex', 'ept-codex']) f.state.threadModels.set(`${b}:a`, { model: running(), validatedAt: 42 })
  f.state.appServerGenerations['ept-codex'] = 1
  f.controller().handleInactiveCodexAppServerStatus('ept-codex', { params: { state: 'ready', generation: 2 } })
  assert.equal(f.state.threadModels.get('ept-codex:a').validatedAt, 0)
  assert.equal(f.state.threadModels.get('codex:a').validatedAt, 42)
  assert.deepEqual(f.calls[0], ['clearPrepared', 'ept-codex'])
})

test('OpenCode idle updates its offscreen model without touching the visible composer', () => {
  const f = fixture(), m = running()
  f.state.threadModels.set('opencode:a', { model: m })
  f.controller().handleOpenCodeServerEvent({ type: 'session.idle', properties: { sessionID: 'a' } })
  assert.equal(m.activeTurnId, null)
  assert.equal(m.status, 'idle')
  assert.equal(f.calls.some(([n]) => ['render', 'composer'].includes(n)), false)
})

test('offscreen OpenCode deletion clears only its persisted and queued state', () => {
  const f = fixture()
  f.state.messageQueues['opencode:a'] = ['draft']
  f.state.messageQueues['codex:visible'] = ['keep']
  f.state.pinnedSessions.add('opencode:a')
  f.controller().handleOpenCodeServerEvent({ type: 'session.deleted', properties: { sessionID: 'a' } })
  assert.equal(f.state.messageQueues['opencode:a'], undefined)
  assert.deepEqual(f.state.messageQueues['codex:visible'], ['keep'])
  assert.equal(f.state.selectedId, 'visible')
  assert.equal(f.calls.some(([n]) => n === 'workspace'), false)
})
