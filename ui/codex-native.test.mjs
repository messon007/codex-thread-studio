import assert from 'node:assert/strict'
import test from 'node:test'

import {
  applyCodexNotification,
  beginOptimisticCodexTurn,
  createCodexViewModel,
  hydrateCodexThread,
  reconcileOptimisticCodexTurn,
  resolveCodexApproval,
  rollbackOptimisticCodexTurn,
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

test('turn completion never drops the original user message from a partial snapshot', () => {
  const model = createCodexViewModel()
  model.threadId = 'thread-1'
  applyCodexNotification(model, { method: 'turn/started', params: { turn: { id: 'turn-1', status: 'inProgress', items: [] } } })
  applyCodexNotification(model, {
    method: 'item/started',
    params: {
      turnId: 'turn-1',
      item: { id: 'question', type: 'userMessage', content: [{ type: 'text', text: 'Keep my question visible' }] },
    },
  })
  applyCodexNotification(model, {
    method: 'item/completed',
    params: { turnId: 'turn-1', item: { id: 'answer', type: 'agentMessage', text: 'Done.' } },
  })

  applyCodexNotification(model, {
    method: 'turn/completed',
    params: {
      turn: {
        id: 'turn-1',
        status: 'completed',
        items: [{ id: 'answer', type: 'agentMessage', text: 'Done.' }],
      },
    },
  })

  assert.deepEqual(model.turns[0].items.map((item) => item.id), ['question', 'answer'])
  assert.equal(textFromUserContent(model.turns[0].items[0].content), 'Keep my question visible')
})

test('reconciles an optimistic user message with the authoritative turn', () => {
  const model = createCodexViewModel()
  const clientId = 'client-1'
  const pendingId = beginOptimisticCodexTurn(model, {
    clientUserMessageId: clientId,
    input: [{ type: 'text', text: 'Show this immediately' }],
  })
  assert.equal(model.activeTurnId, pendingId)
  assert.equal(textFromUserContent(model.turns[0].items[0].content), 'Show this immediately')

  reconcileOptimisticCodexTurn(model, pendingId, { id: 'turn-1', status: 'inProgress', items: [] })
  applyCodexNotification(model, {
    method: 'item/started',
    params: {
      turnId: 'turn-1',
      item: { id: 'server-user', type: 'userMessage', clientId, content: [{ type: 'text', text: 'Show this immediately' }] },
    },
  })

  assert.equal(model.turns.length, 1)
  assert.equal(model.turns[0].items.filter((item) => item.type === 'userMessage').length, 1)
  assert.equal(model.turns[0].items[0].id, 'server-user')
  assert.equal(model.turns[0].items[0].studioOptimistic, undefined)
})

test('rolls back an optimistic turn without disturbing a server turn', () => {
  const model = createCodexViewModel()
  const pendingId = beginOptimisticCodexTurn(model, {
    clientUserMessageId: 'client-2',
    input: [{ type: 'text', text: 'Temporary' }],
  })
  applyCodexNotification(model, { method: 'turn/started', params: { turn: { id: 'turn-live', status: 'inProgress', items: [] } } })
  rollbackOptimisticCodexTurn(model, pendingId)
  assert.deepEqual(model.turns.map((turn) => turn.id), ['turn-live'])
  assert.equal(model.activeTurnId, 'turn-live')
})

test('reconciles when server notifications arrive before the turn start response', () => {
  const model = createCodexViewModel()
  const pendingId = beginOptimisticCodexTurn(model, {
    clientUserMessageId: 'client-race',
    input: [{ type: 'text', text: 'Race-safe message' }],
  })
  applyCodexNotification(model, {
    method: 'turn/started',
    params: { turn: { id: 'turn-race', status: 'inProgress', items: [] } },
  })
  applyCodexNotification(model, {
    method: 'item/started',
    params: {
      turnId: 'turn-race',
      item: { id: 'server-user', type: 'userMessage', content: [{ type: 'text', text: 'Race-safe message' }] },
    },
  })

  reconcileOptimisticCodexTurn(model, pendingId, { id: 'turn-race', status: 'inProgress', items: [] })
  assert.deepEqual(model.turns.map((turn) => turn.id), ['turn-race'])
  assert.equal(model.turns[0].items.filter((item) => item.type === 'userMessage').length, 1)
})

test('does not revive a fast turn that completed before the start response', () => {
  const model = createCodexViewModel()
  const pendingId = beginOptimisticCodexTurn(model, {
    clientUserMessageId: 'client-fast',
    input: [{ type: 'text', text: 'Fast message' }],
  })
  applyCodexNotification(model, {
    method: 'turn/completed',
    params: { turn: { id: 'turn-fast', status: 'completed', items: [] } },
  })

  reconcileOptimisticCodexTurn(model, pendingId, { id: 'turn-fast', status: 'inProgress', items: [] })
  assert.equal(model.turns[0].status, 'completed')
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
