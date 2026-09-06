import test from 'node:test'
import assert from 'node:assert/strict'
import { cachedSession, storeCachedSession, validateCachedModel, unvalidateCachedModel, cachedModelThreadId, routeCodexNotification } from './session-model-cache.mjs'
import { createCodexViewModel, hydrateCodexThread, applyCodexNotification } from './codex-native.mjs'
import { sessionRefKey } from './thread-router.mjs'
import { routerRuntimeKey } from './thread-router-controller.mjs'
import { claimLifecycleNotification } from './codex-lifecycle-diagnostics.mjs'

function model(id, turn = 'turn') {
  const value = createCodexViewModel()
  hydrateCodexThread(value, { id, turns: [{ id: turn, status: 'inProgress', items: [] }] })
  return value
}
function context(cache, selectedModel, selectedBackend = 'codex', backend = 'codex') {
  return {
    backend, selectedBackend, selectedId: selectedModel.threadId, selectedModel,
    cache, hiddenThreads: new Set(), hiddenTurns: new Set(),
    sessionKey: sessionRefKey, turnKey: routerRuntimeKey,
  }
}

test('cache retains model identity across repeated selections and isolates backend IDs', () => {
  const cache = new Map()
  const a = model('same')
  const b = model('same')
  assert.equal(storeCachedSession(cache, 'codex', 'same', a, null, 10), true)
  storeCachedSession(cache, 'ept-codex', 'same', b, null, 20)
  for (let i = 0; i < 20; i++) {
    assert.equal(cachedSession(cache, 'codex', 'same').model, a)
    assert.equal(cachedSession(cache, 'ept-codex', 'same').model, b)
  }
  assert.equal(storeCachedSession(cache, 'codex', 'different', a, null, 30), false)
  assert.equal(cache.size, 2)
  assert.equal(cachedModelThreadId(cache, 'codex', b), null)
  assert.equal(cachedModelThreadId(cache, 'ept-codex', b), 'same')
})

test('OpenCode live events do not advance an older history epoch; full history does', () => {
  const cache = new Map()
  const value = model('one')
  storeCachedSession(cache, 'opencode', 'one', value, 1, 10)
  validateCachedModel(cache, 'opencode', value, 2, 20)
  assert.equal(cachedSession(cache, 'opencode', 'one').historyEpoch, 1)
  assert.equal(cachedSession(cache, 'opencode', 'one').validatedAt, 20)
  storeCachedSession(cache, 'opencode', 'one', value, 2, 30)
  assert.equal(cachedSession(cache, 'opencode', 'one').historyEpoch, 2)
  unvalidateCachedModel(cache, 'opencode', value)
  assert.equal(cachedSession(cache, 'opencode', 'one').validatedAt, 0)
})

test('background completion updates only the addressed backend and retains an incomplete-history marker', () => {
  const cache = new Map()
  const a = model('same', 'shared-turn')
  const b = model('same', 'shared-turn')
  storeCachedSession(cache, 'codex', 'same', a, null, 10)
  storeCachedSession(cache, 'ept-codex', 'same', b, null, 10)
  const message = { method: 'turn/completed', params: { threadId: 'same', turn: { id: 'shared-turn', status: 'completed', items: [] } } }
  const routed = routeCodexNotification(message, context(cache, a, 'codex', 'ept-codex'))
  assert.equal(routed, b)
  assert.equal(applyCodexNotification(routed, message), true)
  unvalidateCachedModel(cache, 'ept-codex', routed)
  assert.equal(b.status, 'idle')
  assert.equal(b.activeTurnId, null)
  assert.equal(a.status, 'running')
  assert.equal(cachedSession(cache, 'ept-codex', 'same').validatedAt, 0)
  assert.equal(cachedSession(cache, 'codex', 'same').validatedAt, 10)
})

test('notification routing preserves explicit IDs, hidden utility sessions, and origin-only fallback', () => {
  const cache = new Map()
  const a = model('a', 'a-turn')
  const b = model('b', 'b-turn')
  storeCachedSession(cache, 'codex', 'b', b, null, 10)
  const ctx = context(cache, a)
  assert.equal(routeCodexNotification({ params: { turnId: 'b-turn' } }, ctx), b)
  assert.equal(routeCodexNotification({ params: { threadId: 'missing', turnId: 'b-turn' } }, ctx), null)
  assert.equal(routeCodexNotification({}, ctx), a)
  assert.equal(routeCodexNotification({}, { ...ctx, backend: 'ept-codex' }), null)
  ctx.hiddenThreads.add(sessionRefKey('codex', 'b'))
  assert.equal(routeCodexNotification({ params: { threadId: ' b ' } }, ctx), null)
  ctx.hiddenTurns.add(routerRuntimeKey('codex', 'b-turn'))
  assert.equal(routeCodexNotification({ params: { turnId: 'b-turn' } }, ctx), null)
})

test('a delayed duplicate completion cannot clear the next running turn', () => {
  const cache = new Map()
  const value = model('one', 'old')
  storeCachedSession(cache, 'codex', 'one', value, null, 10)
  const turns = new Map()
  const statuses = new Map()
  const deliver = (message) => {
    if (!claimLifecycleNotification('codex', message, turns, statuses, () => 0)) return false
    return applyCodexNotification(routeCodexNotification(message, context(cache, value)), message)
  }
  const oldCompletion = { method: 'turn/completed', params: { threadId: 'one', turn: { id: 'old', status: 'completed', items: [] } } }
  assert.equal(deliver(oldCompletion), true)
  assert.equal(deliver({ method: 'turn/started', params: { threadId: 'one', turn: { id: 'new', status: 'inProgress', items: [] } } }), true)
  assert.equal(deliver(oldCompletion), false)
  assert.equal(value.activeTurnId, 'new')
  assert.equal(value.status, 'running')
})
