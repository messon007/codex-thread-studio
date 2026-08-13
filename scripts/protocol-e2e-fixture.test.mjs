import assert from 'node:assert/strict'
import test from 'node:test'
import { applyCodexNotification, createCodexViewModel, hydrateCodexThread, resolveCodexInteraction } from '../ui/codex-native.mjs'

test('reconnect fixture restores a running turn, answers input, and reconciles completion', () => {
  const model = createCodexViewModel()
  hydrateCodexThread(model, { id: 'thread-1', turns: [{ id: 'turn-1', status: 'inProgress', items: [{ id: 'user-1', type: 'userMessage', content: [{ type: 'text', text: 'Run it' }] }] }] })
  assert.equal(model.status, 'running')
  applyCodexNotification(model, { id: 17, method: 'item/tool/requestUserInput', params: { threadId: 'thread-1', turnId: 'turn-1', itemId: 'tool-1', isBlocking: true, questions: [{ id: 'confirm', header: 'Confirm', question: 'Continue?', options: [{ label: 'Yes', description: 'Continue' }] }] } })
  assert.equal(model.interactions.length, 1)
  resolveCodexInteraction(model, 17)
  applyCodexNotification(model, { method: 'turn/completed', params: { turn: { id: 'turn-1', status: 'completed', items: [] } } })
  assert.equal(model.status, 'idle')
  assert.equal(model.turns[0].items[0].type, 'userMessage')
})
