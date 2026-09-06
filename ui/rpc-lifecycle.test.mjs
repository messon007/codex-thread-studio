import test from 'node:test'
import assert from 'node:assert/strict'
import { PendingRpcRequests, requestSocketRpc, connectSelectedSocket } from './rpc-lifecycle.mjs'
class Socket {
  onmessage = null; onerror = null; onclose = null; sent = []; closes = 0
  send(value) { this.sent.push(JSON.parse(value)) }
  close() { this.closes++; this.onclose?.() }
  emit(value) { this.onmessage?.({ data: JSON.stringify(value) }) }
}
const options = socket => ({ socket, id: 1, method: 'thread/read', params: {}, timeoutMs: 1000, timeoutMessage: 'timeout', unavailableMessage: 'unavailable', connectMessage: 'connect', closeMessage: 'closed', responseError: e => e.message })
test('pending RPC matches exactly once and disconnect rejects every outstanding request', async () => {
  const requests = new PendingRpcRequests(new Map())
  const first = requests.request(1, 'read', () => {}, 1000, 'timeout')
  assert.equal(requests.settle({ id: 99, result: 'other' }), false)
  assert.equal(requests.settle({ id: 1, result: 'ok' }), true)
  assert.equal(requests.settle({ id: 1, result: 'duplicate' }), false)
  assert.equal(await first, 'ok')
  const a = requests.request(2, 'read', () => {}, 1000, 'timeout'), b = requests.request(3, 'read', () => {}, 1000, 'timeout')
  requests.rejectAll(Error('disconnected'))
  await Promise.all([assert.rejects(a, /disconnected/), assert.rejects(b, /disconnected/)])
  assert.equal(requests.pending.size, 0)
})
test('pending RPC cleans up synchronous send failure and timeout', async () => {
  const requests = new PendingRpcRequests(new Map())
  await assert.rejects(requests.request(1, 'read', () => { throw Error('send failed') }, 1000, 'timeout'), /send failed/)
  assert.equal(requests.pending.size, 0)
  await assert.rejects(requests.request(2, 'read', () => {}, 5, 'timeout'), /timeout/)
  assert.equal(requests.pending.size, 0)
})
test('one-shot sockets wait for readiness, send once, isolate IDs and close once', async () => {
  const socket = new Socket(), result = requestSocketRpc(options(socket))
  socket.emit({ method: 'studio/appServer/status', params: { state: 'ready' } })
  socket.emit({ method: 'studio/appServer/status', params: { state: 'ready' } })
  socket.emit({ id: 99, result: 'other' })
  assert.equal(socket.sent.length, 1)
  socket.emit({ id: 1, result: 'result' })
  socket.emit({ id: 1, result: 'duplicate' })
  assert.equal(await result, 'result')
  assert.equal(socket.closes, 1)
})
test('one-shot socket failure, close and timeout reject and release the socket', async () => {
  for (const mode of ['error', 'close', 'timeout']) {
    const socket = new Socket(), result = requestSocketRpc({ ...options(socket), timeoutMs: 5 })
    if (mode === 'error') socket.onerror()
    if (mode === 'close') socket.onclose()
    await assert.rejects(result, mode === 'error' ? /connect/ : mode === 'close' ? /closed/ : /timeout/)
    assert.equal(socket.closes, 1)
  }
})
test('old selected sockets cannot dispatch messages or schedule reconnects after a backend switch', () => {
  const state = { socket: null, socketGeneration: 0, ready: true, reconnectTimer: null }, sockets = [], messages = []
  const effects = { cleanup() {}, starting() {}, open() { const socket = new Socket(); sockets.push(socket); return socket }, message: data => messages.push(data), error() {}, closed() {}, reconnect() {} }
  connectSelectedSocket(state, effects)
  connectSelectedSocket(state, effects)
  sockets[0].emit({ stale: true }); sockets[0].onclose()
  assert.equal(state.reconnectTimer, null)
  sockets[1].emit({ current: true })
  assert.deepEqual(messages, ['{"current":true}'])
})
