import test from 'node:test'
import assert from 'node:assert/strict'
import { createSubmissionController } from './submission-controller.mjs'
import { createCodexViewModel } from './codex-native.mjs'

function fixture() {
  const state = { backend: 'codex', selectedId: 'a', model: createCodexViewModel(), pendingSkills: {}, pendingFiles: {}, pendingImages: {}, messageQueues: {}, runningMessageQueues: new Set(), pausedMessageQueues: new Set(), messageQueueErrors: new Map() }
  const calls = [], input = { value: 'hello', disabled: false }, button = { disabled: false }
  const services = {
    $: selector => selector === '#composer-input' ? input : button,
    selectedStateKey: (id = state.selectedId, backend = state.backend) => `${backend}:${id}`,
    shellCommandFromComposer: () => null, matchingSlashCommands: () => [], executeSlashCommand: async () => {},
    isRouterThread: () => false, threadRouter: { startTurn: async () => calls.push('router') },
    rpc: async (method, params) => { calls.push([method, params]); return {} },
    dispatchBackendRpc: async (backend, method, params) => { calls.push([backend, method, params]); return { turn: { id: 'turn', status: 'inProgress', items: [] } } },
    sessionDispatch: { clearPreparedSession: () => calls.push('clear-prepared') }, prepareComposerTurn: async () => calls.push('prepare'),
    isCodexBackend: backend => backend === 'codex', isSupportedBackend: () => true,
    backendDescriptor: () => ({ kind: 'codex' }), currentBackend: () => ({ name: 'Codex' }), threadForRef: () => ({ cwd: '/project' }),
    configuredTurnOptions: () => ({ model: 'selected-model' }), queuedTurnOptions: () => ({ model: 'session-model' }), messageQueueModel: () => state.model,
    setComposerDraftValue: (_key, text) => { input.value = text }, composerDrafts: { value: () => input.value },
    hideComposerMenu() {}, renderComposerState: () => calls.push('render-composer'), renderTranscript() {}, beginTranscriptFollowingLatest() {},
    setCatalogThreadActivity: () => 'activity', rollbackCatalogThreadActivity: () => calls.push('rollback'), markCachedModelValidated() {},
    beginTurnLatencyTrace: () => ({}), bindTurnLatencyTrace() {}, markTurnLatency() {}, finishTurnLatencyTrace() {},
    randomId: () => 'client', persistMessageQueue: async () => calls.push('persist'), showError: error => calls.push(['error', error.message]), toast() {}, t: text => text,
  }
  return { state, services, calls, input, controller: createSubmissionController(state, services) }
}
const event = { preventDefault() {} }

test('normal send prepares once, binds cwd/model and reconciles optimistic input', async () => {
  const f = fixture()
  assert.equal(await f.controller.sendComposer(event), true)
  const request = f.calls.find(call => Array.isArray(call) && call[1] === 'turn/start')
  assert.equal(request[2].cwd, '/project')
  assert.equal(request[2].model, 'selected-model')
  assert.deepEqual(request[2].input, [{ type: 'text', text: 'hello' }])
  assert.equal(f.calls.filter(call => call === 'prepare').length, 1)
  assert.equal(f.state.model.turns.length, 1)
  assert.equal(f.state.model.turns[0].id, 'turn')
  assert.equal(f.input.value, '')
})

test('rejected send restores the draft but does not overwrite newly typed text', async () => {
  for (const changed of [false, true]) {
    const f = fixture()
    f.services.dispatchBackendRpc = async () => { if (changed) f.input.value = 'new draft'; throw new Error('rejected') }
    f.controller = createSubmissionController(f.state, f.services)
    assert.equal(await f.controller.sendComposer(event), false)
    assert.equal(f.input.value, changed ? 'new draft' : 'hello')
    assert.equal(f.state.model.turns.length, 0)
    assert.ok(f.calls.includes('rollback'))
  }
})

test('Steer targets the active turn without preparing another turn', async () => {
  const f = fixture()
  f.state.model.activeTurnId = 'active'
  await f.controller.sendComposer(event)
  assert.equal(f.calls.includes('prepare'), false)
  assert.equal(f.calls.find(call => Array.isArray(call))[0], 'turn/steer')
  assert.equal(f.calls.find(call => Array.isArray(call))[1].expectedTurnId, 'active')
})

test('queued send uses current session model and persists removal after acknowledgement', async () => {
  const f = fixture()
  f.state.messageQueues['codex:a'] = [{ id: 'q', text: 'queued', input: [], createdAt: 1 }]
  await Promise.all([f.controller.runNextQueuedMessage({ backend: 'codex', id: 'a' }), f.controller.runNextQueuedMessage({ backend: 'codex', id: 'a' })])
  const requests = f.calls.filter(call => Array.isArray(call) && call[1] === 'turn/start')
  assert.equal(requests.length, 1)
  assert.equal(requests[0][2].model, 'session-model')
  assert.equal(f.state.messageQueues['codex:a'], undefined)
  assert.equal(f.state.runningMessageQueues.size, 0)
})

test('late in-progress acknowledgement cannot resurrect a completed turn', () => {
  const f = fixture()
  f.state.model.turns.push({ id: 'done', status: 'completed', items: [] })
  assert.equal(f.controller.applyTurnAcknowledgement(f.state.model, { id: 'done', status: 'inProgress' }), false)
  assert.equal(f.state.model.activeTurnId, null)
})
