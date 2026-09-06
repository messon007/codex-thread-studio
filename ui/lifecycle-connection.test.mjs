import test from 'node:test'
import assert from 'node:assert/strict'
import { connectLifecycleStream, connectEventStream, waitForEventStream } from './lifecycle-connection.mjs'

test('SSE shares one connection and cleans ready/timeout waiters without replacing native reconnect', async t => {
  t.mock.timers.enable({ apis: ['setTimeout'] })
  const state = { stream: null, ready: false, missedBarrier: false, waiters: new Set(), openCount: 0 }
  const socket = { onopen: null, onmessage: null, onerror: null }
  const calls = []
  const effects = { open: () => socket, connected: gap => calls.push(gap), message: value => calls.push(value), invalid: () => calls.push('invalid'), disconnected: () => calls.push('error') }
  const waiting = waitForEventStream(state)
  assert.equal(connectEventStream(state, effects), socket)
  assert.equal(connectEventStream(state, { ...effects, open: () => assert.fail('duplicate stream') }), socket)
  socket.onopen()
  assert.equal(await waiting, true)
  assert.equal(state.waiters.size, 0)
  socket.onerror()
  const timeout = waitForEventStream(state)
  t.mock.timers.tick(1500)
  assert.equal(await timeout, false)
  assert.equal(state.waiters.size, 0)
  socket.onopen()
  socket.onmessage({ data: '{"type":"session.idle"}' })
  socket.onmessage({ data: 'bad JSON' })
  assert.deepEqual(calls, [false, 'error', true, { type: 'session.idle' }, 'invalid'])
})

test('SSE first late connection reconciles a selection that missed its barrier', () => {
  const state = { stream: null, ready: false, missedBarrier: true, waiters: new Set(), openCount: 0 }
  let reconciled = false
  const socket = connectEventStream(state, {
    open: () => ({}), connected: gap => { reconciled = gap },
    message() {}, invalid() {}, disconnected() {},
  })
  socket.onopen()
  assert.equal(reconciled, true)
  assert.equal(state.missedBarrier, false)
})

function fixture() {
  const state = { socket: null, reconnectTimer: null, ready: false, generation: 0, openCount: 0 }
  const sockets = [], calls = []
  const effects = {
    open: () => { const socket = { close() {}, send() {}, onmessage: null, onclose: null, onerror: null }; sockets.push(socket); return socket },
    invalid: () => calls.push('invalid'), report: value => calls.push(value.state),
    reconcile: () => calls.push('reconcile'), supports: backend => ['codex', 'ept-codex'].includes(backend),
    status: backend => calls.push(`status:${backend}`), lag: backend => calls.push(`lag:${backend}`),
    notification: backend => calls.push(`notification:${backend}`),
  }
  const ready = socket => socket.onmessage({ data: JSON.stringify({ method: 'studio/codexLifecycle/ready', params: { backends: ['codex', 'ept-codex'] } }) })
  return { state, sockets, calls, effects, ready }
}

test('lifecycle barrier timeout does not kill a slow connection', async t => {
  t.mock.timers.enable({ apis: ['setTimeout'] })
  const f = fixture()
  const barrier = connectLifecycleStream(f.state, f.effects)
  t.mock.timers.tick(750)
  assert.equal(await barrier, false)
  f.ready(f.sockets[0])
  assert.equal(f.state.ready, true)
  assert.deepEqual(f.calls, ['connected'])
})

test('lifecycle reconnect retains delay and rejects callbacks from replaced connections', async t => {
  t.mock.timers.enable({ apis: ['setTimeout'] })
  const f = fixture()
  const first = connectLifecycleStream(f.state, f.effects)
  const old = f.sockets[0], oldMessage = old.onmessage, oldError = old.onerror
  f.ready(old)
  assert.equal(await first, true)
  old.onclose()
  assert.equal(f.state.ready, false)
  t.mock.timers.tick(1799)
  assert.equal(f.sockets.length, 1)
  t.mock.timers.tick(1)
  assert.equal(f.sockets.length, 2)
  f.ready(f.sockets[1])
  oldError()
  oldMessage({ data: JSON.stringify({ method: 'studio/codexLifecycle/ready', params: {} }) })
  assert.equal(f.state.ready, true)
  assert.equal(f.state.openCount, 2)
  assert.deepEqual(f.calls, ['connected', 'disconnected', 'reconnected', 'reconcile'])
})

test('lifecycle validates envelopes and routes backend status, lag and notifications separately', async t => {
  t.mock.timers.enable({ apis: ['setTimeout'] })
  const f = fixture()
  const barrier = connectLifecycleStream(f.state, f.effects)
  const socket = f.sockets[0]
  f.ready(socket)
  await barrier
  socket.onmessage({ data: 'invalid JSON' })
  for (const [backend, method] of [['codex', 'studio/appServer/status'], ['ept-codex', 'studio/appServer/lagged'], ['ept-codex', 'turn/completed'], ['opencode', 'turn/completed']]) {
    socket.onmessage({ data: JSON.stringify({ method: 'studio/codexLifecycle/event', params: { backend, message: { method } } }) })
  }
  assert.deepEqual(f.calls, ['connected', 'invalid', 'status:codex', 'lag:ept-codex', 'notification:ept-codex'])
})
