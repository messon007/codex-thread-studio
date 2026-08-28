import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')

test('same-process Codex reconnects reuse only a catalog-fresh cached history', () => {
  const start = source.indexOf('function handleAppServerMessage(')
  const end = source.indexOf("if (message.method === 'studio/appServer/log')", start)
  const statusHandler = source.slice(start, end)

  assert.match(statusHandler, /const appServerRestarted = previousGeneration != null && nextGeneration !== previousGeneration/u)
  assert.match(statusHandler, /if \(!appServerRestarted && freshThreadModel\(backend, selectedId, \{ reconnectValidation: true \}\)\) return/u)
  assert.match(statusHandler, /if \(state\.backend !== backend[\s\S]*state\.socketGeneration !== socketGeneration[\s\S]*sessionManagement\.archive\.isOpen\(\)[\s\S]*!state\.selectedId\) return/u)
  assert.doesNotMatch(statusHandler, /const reconnecting = previousGeneration != null/u)
})

test('Codex App Server restart does not read again after loadThreads resumed the selection', () => {
  const start = source.indexOf('function handleAppServerMessage(')
  const end = source.indexOf("if (message.method === 'studio/appServer/log')", start)
  const statusHandler = source.slice(start, end)

  const snapshot = statusHandler.indexOf('const cachedModelsBeforeCatalog = new Map(state.threadModels)')
  const load = statusHandler.indexOf('loadThreads({')
  const replacementCheck = statusHandler.indexOf('state.threadModels.get(selectedKey) !== cachedModelsBeforeCatalog.get(selectedKey)')
  const refresh = statusHandler.indexOf('await refreshSelectedThread({')
  const activeHistory = statusHandler.indexOf('const activeHistory = state.threadLoads.get(selectedKey)')

  assert.ok(snapshot >= 0 && snapshot < load)
  assert.ok(load < activeHistory)
  assert.ok(activeHistory < replacementCheck)
  assert.ok(replacementCheck < refresh)
  assert.match(statusHandler, /await activeHistory[\s\S]*state\.selectedId !== selectedId/u)
  assert.match(statusHandler, /environmentRoot,[\s\S]*environmentRevision:/u)
})

test('catalog loads cannot install a response after its backend selection changed', () => {
  const start = source.indexOf('async function loadThreads(')
  const end = source.indexOf('\nfunction setActiveThreads', start)
  const loader = source.slice(start, end)

  assert.match(loader, /const backend = state\.backend/u)
  assert.match(loader, /const socketGeneration = state\.socketGeneration/u)
  assert.ok(loader.indexOf("await dispatchBackendRpc(backend, 'thread/list'") < loader.indexOf('state.backend !== backend'))
  assert.ok(loader.indexOf('state.backend !== backend') < loader.indexOf('setActiveThreads('))
  assert.match(loader, /selectThread\(nextId, \{ force: true, backend \}\)/u)
})

test('backend switches discard a stale information request before connecting', () => {
  const start = source.indexOf('async function switchBackend(')
  const end = source.indexOf('\nfunction applyBackendCopy', start)
  const switcher = source.slice(start, end)
  assert.match(switcher, /const transitionGeneration = state\.socketGeneration/u)
  assert.match(switcher, /await loadBackendInfo\(backend\)[\s\S]*state\.backend !== backend \|\| state\.socketGeneration !== transitionGeneration[\s\S]*connectBackend/u)
})

test('lag recovery serializes behind the selected history load', () => {
  const start = source.indexOf('async function resynchronizeSelectedThreadAfterLag(')
  const end = source.indexOf('\nfunction ', start + 1)
  const recovery = source.slice(start, end)
  assert.ok(recovery.indexOf('await activeHistory') < recovery.indexOf('await refreshSelectedThread('))
  assert.match(recovery, /state\.selectedId !== threadId/u)
  assert.match(recovery, /state\.socketGeneration !== socketGeneration/u)
})

test('selected history refreshes share the same per-session flight within one connection epoch', () => {
  const start = source.indexOf('async function refreshSelectedThread(')
  const end = source.indexOf('\nfunction freshThreadModel', start)
  const refresh = source.slice(start, end)
  assert.match(refresh, /const key = threadCatalogKey\(backend, threadId\)/u)
  assert.match(refresh, /const activeLoad = state\.threadLoads\.get\(key\)/u)
  assert.match(refresh, /backend !== 'opencode' \|\| activeLoad\.historyEpoch === historyEpoch/u)
  assert.match(refresh, /state\.threadLoads\.set\(key, load\)/u)
  assert.match(refresh, /state\.threadLoads\.get\(key\) === load/u)
})
