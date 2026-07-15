import assert from 'node:assert/strict'
import test from 'node:test'

import {
  applyCodexNotification,
  createCodexViewModel,
  hydrateCodexThread,
  resolveCodexApproval,
  textFromUserContent,
} from './codex-native.mjs'

test('hydrates persisted turns and detects an active turn', () => {
  const model = createCodexViewModel()
  hydrateCodexThread(model, { id: 'thread-1', turns: [{ id: 'turn-1', status: 'inProgress', items: [] }] })
  assert.equal(model.threadId, 'thread-1')
  assert.equal(model.activeTurnId, 'turn-1')
  assert.equal(model.status, 'running')
})

test('assembles streamed structured items and completion state', () => {
  const model = createCodexViewModel()
  model.threadId = 'thread-1'
  applyCodexNotification(model, { method: 'turn/started', params: { turn: { id: 'turn-1', status: 'inProgress', items: [] } } })
  applyCodexNotification(model, { method: 'item/started', params: { turnId: 'turn-1', item: { id: 'item-1', type: 'agentMessage', text: '' } } })
  applyCodexNotification(model, { method: 'item/agentMessage/delta', params: { turnId: 'turn-1', itemId: 'item-1', delta: 'hello' } })
  applyCodexNotification(model, { method: 'turn/completed', params: { turn: { id: 'turn-1', status: 'completed', items: [] } } })
  assert.equal(model.turns[0].items[0].text, 'hello')
  assert.equal(model.activeTurnId, null)
  assert.equal(model.status, 'idle')
})

test('tracks approval requests and responses', () => {
  const model = createCodexViewModel()
  applyCodexNotification(model, { id: 41, method: 'item/commandExecution/requestApproval', params: { command: 'cargo test' } })
  assert.equal(model.approvals.length, 1)
  resolveCodexApproval(model, 41)
  assert.equal(model.approvals.length, 0)
})

test('extracts only text user inputs', () => {
  assert.equal(textFromUserContent([{ type: 'text', text: 'one' }, { type: 'localImage', path: '/tmp/a.png' }, { type: 'text', text: 'two' }]), 'one\ntwo')
})
