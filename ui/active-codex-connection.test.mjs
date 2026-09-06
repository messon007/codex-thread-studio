import assert from 'node:assert/strict'
import test from 'node:test'
import { createActiveCodexConnection } from './active-codex-connection.mjs'
import { createCodexViewModel } from './codex-native.mjs'

function fixture() {
  const calls = [], state = {
    backend: 'codex', ready: false, selectedId: null, socketGeneration: 1,
    model: createCodexViewModel(), threadModels: new Map(), threadLoads: new Map(), appServerGenerations: {},
  }
  const loads = new Map(), services = {
    $: () => ({ textContent: '' }), backendDescriptor: () => ({ name: 'Codex' }),
    codexBackendsNeedingRestartRecovery: new Set(), sessionDispatch: { clearPrepared: b => calls.push(['clear', b]) },
    clearStartedThreadsForBackend: b => calls.push(['clear-started', b]), setBackendState: () => {}, setNativeError: () => {},
    loadBackendModels: async () => {}, loadThreads: async () => calls.push(['load']),
    sessionManagement: { archive: { isOpen: () => false } }, backendSelectionLoads: loads,
    handleThreadCatalogFailure: () => {}, threadCatalogKey: (b, id) => `${b}:${id}`,
    refreshSelectedThread: async () => { calls.push(['refresh']); return true },
    toast: () => {}, t: s => s,
  }
  return { state, services, calls, loads, controller: () => createActiveCodexConnection(state, services) }
}
const ready = { method: 'studio/appServer/status', params: { state: 'ready', generation: 1 } }
test('each new frontend initialization loads its catalog; duplicate ready does not', async () => {
  for (let refresh = 0; refresh < 3; refresh++) {
    const f = fixture(), c = f.controller()
    c.handleAppServerMessage(ready)
    c.handleAppServerMessage(ready)
    await Promise.all(f.loads.values())
    assert.equal(f.calls.filter(([n]) => n === 'load').length, 1)
    assert.equal(f.state.ready, true)
  }
})
test('App Server generation change invalidates preparation independently of first-ready loading', () => {
  const f = fixture()
  f.state.ready = true
  f.state.appServerGenerations.codex = 1
  f.controller().handleAppServerMessage({ ...ready, params: { state: 'ready', generation: 2 } })
  assert.deepEqual(f.calls, [['clear', 'codex'], ['clear-started', 'codex']])
})
test('lag recovery never refreshes a selection that changed while awaiting history', async () => {
  const f = fixture(); f.state.ready = true; f.state.selectedId = 'a'
  let resolve
  f.state.threadLoads.set('codex:a', new Promise(r => { resolve = r }))
  const result = f.controller().resynchronizeSelectedThreadAfterLag({ backend: 'codex', threadId: 'a', socketGeneration: 1, skipped: 2 })
  f.state.selectedId = 'b'; resolve()
  assert.equal(await result, false)
  assert.equal(f.calls.length, 0)
})
test('RPC replies are settled before notification routing', () => {
  const f = fixture()
  f.services.sessionMap = { captureWorkerResponse: () => f.calls.push(['worker']) }
  f.services.pendingRpcRequests = { settle: () => f.calls.push(['settle']) }
  f.controller().handleAppServerMessage({ id: 2, result: {} })
  assert.deepEqual(f.calls, [['worker'], ['settle']])
})
