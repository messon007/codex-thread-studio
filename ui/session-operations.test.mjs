import test from 'node:test'
import assert from 'node:assert/strict'
import { createSessionOperations } from './session-operations.mjs'
import { createCodexViewModel } from './codex-native.mjs'

function fixture() {
  const state = { backend: 'codex', selectedId: 'old', ready: true, hostPlatform: 'linux', model: createCodexViewModel(), threads: [], threadsByBackend: { codex: [] }, backendModels: { codex: [{ id: 'model' }] }, turnOptions: {}, selectedByBackend: { codex: 'old' }, pinnedSessions: new Set(), annotationDrafts: {}, annotationAdditional: {}, openingMessages: {}, messageQueues: {}, pausedMessageQueues: new Set(), messageQueueErrors: new Map() }
  const calls = [], elements = {}
  const services = {
    $: selector => elements[selector] ||= { value: '', disabled: false, textContent: '', isConnected: true, classList: { add() {}, remove() {} }, reset() {} },
    t: text => text, confirm: () => true,
    rpc: async (method, params) => { calls.push([method, params]); return { thread: { id: 'new', turns: [] } } },
    selectedStateKey: (id = state.selectedId, backend = state.backend) => `${backend}:${id}`,
    selectedThread: () => state.threads.find(thread => thread.id === state.selectedId) || null,
    currentBackend: () => ({ name: 'Codex' }), backendDescriptor: () => ({ name: 'Codex' }), isCodexBackend: () => true, isArchivedPreview: () => false,
    defaultTurnOptions: () => ({ effort: 'low' }), switchBackend: async backend => { state.backend = backend }, waitFor: async () => {},
    rememberStartedThread: (backend, thread) => { calls.push('remember'); state.threads.push(thread) }, forgetStartedThread: () => calls.push('forget'),
    hydrateOpenCodeModelMetadata() {}, cacheThreadModel: () => calls.push('cache'), sessionDispatch: { markPrepared: () => calls.push('prepared') },
    selectThread: (id) => { state.selectedId = id; calls.push('selected'); return Promise.resolve() }, connectionReady() {}, freshThreadModel: () => true,
    reportSessionLifecycle: phase => calls.push(phase), scheduleStartedThreadCatalogConfirmation: () => calls.push('confirmation'), threadForRef: () => ({}),
    closeNewThreadDialog: () => calls.push('close-new'), closeRenameThreadDialog() {}, persistSessionTurnOptions: async key => calls.push(['model', key]),
    persistSessionPin: async (...args) => calls.push(['pin', ...args]), deletePersistedSessionState: key => calls.push(['delete-state', key]),
    discardComposerSessionState: () => calls.push('discard'), invalidateThreadModel: () => calls.push('invalidate'),
    sessionManagement: { archive: { markStale() {}, remove: () => calls.push('archive-remove') } }, persistPreferences() {}, loadThreads: async () => calls.push('load'),
    renderThreadList() {}, renderWorkspace() {}, renderTranscript() {}, toast() {}, showError: error => calls.push(['error', error.message]),
  }
  services.$('#new-thread-backend').value = 'codex'
  services.$('#new-thread-cwd').value = '/project'
  return { state, calls, services, operations: createSessionOperations(state, services) }
}

test('creation activates and prepares before closing, without a redundant resume', async () => {
  const f = fixture()
  f.services.$('#new-thread-model').value = 'model'
  await f.operations.createThread({ preventDefault() {} })
  assert.equal(f.state.selectedId, 'new')
  assert.ok(f.calls.indexOf('cache') < f.calls.indexOf('prepared'))
  assert.ok(f.calls.indexOf('prepared') < f.calls.indexOf('close-new'))
  assert.equal(f.calls.some(call => Array.isArray(call) && call[0] === 'thread/resume'), false)
  assert.deepEqual(f.state.turnOptions['codex:new'], { effort: 'low', model: 'model' })
})

test('unsupported model prevents creation and re-enables the create button', async () => {
  const f = fixture()
  f.services.$('#new-thread-model').value = 'missing'
  await f.operations.createThread({ preventDefault() {} })
  assert.equal(f.calls.length, 0)
  assert.equal(f.services.$('#create-thread').disabled, false)
  assert.match(f.services.$('#new-thread-error').textContent, /not in/)
})

test('fork inherits session model and uses the authoritative activation path', async () => {
  const f = fixture()
  f.state.turnOptions['codex:old'] = { model: 'model', effort: 'high' }
  await f.operations.forkThread('turn')
  assert.equal(f.state.selectedId, 'new')
  assert.deepEqual(f.state.turnOptions['codex:new'], { model: 'model', effort: 'high' })
  assert.equal(f.calls.filter(call => call === 'prepared').length, 1)
})

test('archive clears pin only after acceptance; rejection preserves selection', async () => {
  for (const reject of [false, true]) {
    const f = fixture()
    f.state.pinnedSessions.add('codex:old')
    if (reject) { f.services.rpc = async () => { throw new Error('denied') }; f.operations = createSessionOperations(f.state, f.services) }
    await f.operations.archiveSelectedThread()
    assert.equal(f.state.pinnedSessions.has('codex:old'), reject)
    assert.equal(f.state.selectedId, reject ? 'old' : null)
  }
})

test('delete removes per-session state and archived preview avoids a live catalog load', async () => {
  const f = fixture()
  f.services.isArchivedPreview = () => true
  f.operations = createSessionOperations(f.state, f.services)
  f.state.annotationDrafts['codex:old'] = ['draft']
  f.state.messageQueues['codex:old'] = ['queue']
  await f.operations.deleteSelectedThread()
  assert.equal(f.state.annotationDrafts['codex:old'], undefined)
  assert.equal(f.state.messageQueues['codex:old'], undefined)
  assert.ok(f.calls.includes('archive-remove'))
  assert.equal(f.calls.includes('load'), false)
  assert.equal(f.state.selectedByBackend.codex, 'old')
})
