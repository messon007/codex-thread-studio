import test from 'node:test'
import assert from 'node:assert/strict'
import { executeQueuedMessage } from './queue-execution.mjs'
const fixture = () => ({ messageQueues: { a: [{ id: 'one', text: 'hi', input: [], createdAt: 1 }] }, runningMessageQueues: new Set(), pausedMessageQueues: new Set(), messageQueueErrors: new Map() })
const defaults = { acknowledged() {}, failed() {}, persist: async () => {}, acceptedPersistenceError: 'verify before retry' }
test('queue executes once and removes the acknowledged head only after acceptance', async () => {
  const state = fixture(); let resolve
  const first = executeQueuedMessage(state, 'a', { ...defaults, send: () => new Promise(done => { resolve = done }) })
  assert.equal(await executeQueuedMessage(state, 'a', { ...defaults, send: async () => { assert.fail('duplicate') } }), false)
  state.messageQueues.a.push({ id: 'two', text: 'next', input: [], createdAt: 2 })
  resolve({})
  assert.equal(await first, true)
  assert.deepEqual(state.messageQueues.a.map(m => m.id), ['two'])
  assert.equal(state.runningMessageQueues.size, 0)
})
test('queue retains failures and pauses on accepted-but-unpersisted messages', async () => {
  for (const accepted of [false, true]) {
    const state = fixture()
    await assert.rejects(executeQueuedMessage(state, 'a', { ...defaults, send: async () => { if (!accepted) throw Error('offline'); return {} }, persist: async () => { throw Error('disk failure') } }))
    assert.equal(state.messageQueues.a[0].id, 'one')
    assert.equal(state.pausedMessageQueues.has('a'), true)
    assert.equal(state.messageQueueErrors.get('a'), accepted ? 'verify before retry' : 'offline')
    assert.equal(state.runningMessageQueues.size, 0)
  }
})
