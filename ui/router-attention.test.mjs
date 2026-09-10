import test from 'node:test'
import assert from 'node:assert/strict'
import { routerAttentionEntries, responseIsVisible } from './router-attention.mjs'
import { RouterTurnCoordinator } from './router-coordination.mjs'

test('only the controller session model latest turn is eligible', () => {
  const dispatches = new Map(), controllers = new Map()
  const add = (key, target, requestedAt, status, unread = true, controller = 'codex:router') => {
    controllers.set(key, controller)
    dispatches.set(key, { decision: { targetSessionKey: target }, requestedAt, status, unread })
  }
  add('old-worker', 'codex:worker', 1, 'completed')
  add('old-other-backend', 'opencode:worker', 2, 'completed')
  add('latest', 'codex:error', 3, 'running', false)
  add('other-router', 'codex:worker', 5, 'completed', true, 'codex:other')
  assert.deepEqual(routerAttentionEntries(dispatches, controllers, 'codex:router', 'latest'), [])
  assert.deepEqual(routerAttentionEntries(dispatches, controllers, 'codex:router', 'non-routed-latest'), [])
  dispatches.get('latest').status = 'failed'; dispatches.get('latest').unread = true
  assert.deepEqual(routerAttentionEntries(dispatches, controllers, 'codex:router', 'latest').map(x => x.key), ['latest'])
  dispatches.get('latest').unread = false
  assert.deepEqual(routerAttentionEntries(dispatches, controllers, 'codex:router', 'latest'), [])
  const restored = new Map(JSON.parse(JSON.stringify([...dispatches])))
  assert.deepEqual(routerAttentionEntries(restored, controllers, 'codex:router', 'latest'), routerAttentionEntries(dispatches, controllers, 'codex:router', 'latest'))
})

test('completion is unread once; interrupted work is an exception rather than success', () => {
  for (const status of ['completed', 'failed', 'interrupted']) {
    const state = { pending: new Map(), dispatches: new Map([['r', { status: 'running', requestedAt: 1 }]]), targetTurns: new Map([['t', { routerTurnId: 'r', targetSessionKey: 'codex:a' }]]) }
    const flow = new RouterTurnCoordinator(state)
    assert.equal(flow.finishTarget('t', { status }), true)
    assert.equal(state.dispatches.get('r').unread, true)
    assert.equal(state.dispatches.get('r').status, status === 'completed' ? 'completed' : 'failed')
    state.dispatches.get('r').unread = false
    assert.equal(flow.finishTarget('t', { status }), false)
    assert.equal(state.dispatches.get('r').unread, false)
  }
})

test('read visibility requires actual content inside viewport, not just an adjacent Turn title', () => {
  const viewport = { top: 100, bottom: 600 }
  assert.equal(responseIsVisible({ top: 620, bottom: 700, height: 80 }, viewport), false)
  assert.equal(responseIsVisible({ top: 595, bottom: 675, height: 80 }, viewport), false)
  assert.equal(responseIsVisible({ top: 200, bottom: 280, height: 80 }, viewport), true)
  assert.equal(responseIsVisible({ top: 200, bottom: 220, height: 20 }, viewport), true)
  assert.equal(responseIsVisible({ top: 200, bottom: 200, height: 0 }, viewport), false)
})
