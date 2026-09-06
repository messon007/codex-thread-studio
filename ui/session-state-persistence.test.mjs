import test from 'node:test'
import assert from 'node:assert/strict'
import { createSessionStatePersistence } from './session-state-persistence.mjs'
import { createSerializedStateWriter } from './serialized-state-writer.mjs'

const emptyState = () => ({ annotationDrafts: {}, annotationAdditional: {}, openingMessages: {}, turnOptions: {}, messageQueues: {} })

test('session persistence suppresses empty keys and writes before preferences load', async () => {
  let ready = false, writes = 0
  const persistence = createSessionStatePersistence(emptyState(), { write: async () => { writes++ } }, () => ready)
  for (const method of ['annotations', 'openingMessage', 'turnOptions', 'messageQueue', 'remove', 'pin']) {
    await persistence[method]('codex:a', true)
    ready = true
    await persistence[method]('', true)
    ready = false
  }
  assert.equal(writes, 0)
})

test('session persistence snapshots FIFO payloads and isolates sessions even when state maps are replaced', async () => {
  const state = emptyState(), calls = []
  const writer = createSerializedStateWriter(async (path, request) => {
    calls.push([path, request.method, JSON.parse(request.body)])
    return { ok: true, status: 200 }
  }, assert.fail)
  const persistence = createSessionStatePersistence(state, writer, () => true)
  state.messageQueues['codex:a'] = [{ id: 'q', text: 'first', input: [], createdAt: 1 }]
  const first = persistence.messageQueue('codex:a')
  state.messageQueues['codex:a'][0].text = 'changed'
  state.turnOptions = { 'opencode:b': { model: 'provider/model', effort: 'low' } }
  const second = persistence.turnOptions('opencode:b')
  const third = persistence.pin('codex:a', false)
  const fourth = persistence.remove('opencode:b')
  await Promise.all([first, second, third, fourth])
  assert.equal(calls[0][2].messages[0].text, 'first')
  assert.deepEqual(calls.slice(1), [
    ['/studio/session-state/turn-options', 'PUT', { sessionKey: 'opencode:b', model: 'provider/model', effort: 'low' }],
    ['/studio/session-state/pin', 'PUT', { sessionKey: 'codex:a', pinned: false }],
    ['/studio/session-state/session', 'DELETE', { sessionKey: 'opencode:b' }],
  ])
})

test('missing session values use explicit clearing payloads without mutating memory', async () => {
  const state = emptyState(), calls = []
  const persistence = createSessionStatePersistence(state, { write: async (path, body) => calls.push([path, body]) }, () => true)
  await persistence.annotations('codex:a')
  await persistence.openingMessage('codex:a')
  await persistence.turnOptions('codex:a')
  assert.deepEqual(calls.map(([, body]) => body), [
    { sessionKey: 'codex:a', drafts: [], additional: '' },
    { sessionKey: 'codex:a', message: null },
    { sessionKey: 'codex:a', model: '', effort: '' },
  ])
  assert.deepEqual(state, emptyState())
})
