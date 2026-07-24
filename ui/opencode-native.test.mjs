import assert from 'node:assert/strict'
import test from 'node:test'

import {
  applyOpenCodeEvent,
  normalizeOpenCodeSessions,
  openCodeModelList,
  openCodeThreadFromHistory,
  splitOpenCodeModel,
} from './opencode-native.mjs'

test('normalizes sessions and busy status', () => {
  const sessions = normalizeOpenCodeSessions([{ id: 'ses-1', title: 'Demo', directory: '/tmp/demo' }], { 'ses-1': { type: 'busy' } })
  assert.equal(sessions[0].id, 'ses-1')
  assert.equal(sessions[0].name, 'Demo')
  assert.equal(sessions[0].cwd, '/tmp/demo')
  assert.equal(sessions[0].status, 'running')
})

test('treats omitted status as idle and reads the persisted provider model', () => {
  const [session] = normalizeOpenCodeSessions([{ id: 'ses-1', model: { providerID: 'openai', id: 'gpt-5' } }], {})
  assert.equal(session.status, 'idle')
  assert.equal(session.model, 'openai/gpt-5')
})

test('groups assistant parts under their user interaction', () => {
  const thread = openCodeThreadFromHistory({ id: 'ses-1', directory: '/tmp/demo' }, [
    { info: { id: 'msg-user', role: 'user' }, parts: [{ id: 'p1', type: 'text', text: 'hello' }] },
    { info: { id: 'msg-agent', parentID: 'msg-user', role: 'assistant' }, parts: [
      { id: 'p2', messageID: 'msg-agent', type: 'reasoning', text: 'thinking' },
      { id: 'p3', messageID: 'msg-agent', type: 'text', text: 'world' },
    ] },
  ], { type: 'idle' })
  assert.equal(thread.turns.length, 1)
  assert.deepEqual(thread.turns[0].items.map((item) => item.type), ['userMessage', 'reasoning', 'agentMessage'])
  assert.equal(thread.messageTurns['msg-agent'], 'msg-user')
})

test('applies streamed OpenCode deltas and status', () => {
  const model = { turns: [{ id: 'turn-1', status: 'inProgress', items: [] }], activeTurnId: 'turn-1', status: 'running', approvals: [], messageTurns: { 'msg-a': 'turn-1' } }
  const result = applyOpenCodeEvent(model, { type: 'message.part.delta', properties: { sessionID: 'ses-1', messageID: 'msg-a', partID: 'part-1', field: 'text', delta: 'hello' } }, 'ses-1')
  assert.equal(result.kind, 'stream')
  assert.equal(model.turns[0].items[0].text, 'hello')
  applyOpenCodeEvent(model, { type: 'session.status', properties: { sessionID: 'ses-1', status: { type: 'idle' } } }, 'ses-1')
  assert.equal(model.status, 'idle')
})

test('keeps streamed user parts as user messages', () => {
  const model = { turns: [], activeTurnId: null, status: 'idle', approvals: [], messageTurns: {} }
  applyOpenCodeEvent(model, { type: 'message.updated', properties: { info: { id: 'msg-user', sessionID: 'ses-1', role: 'user' } } }, 'ses-1')
  applyOpenCodeEvent(model, { type: 'message.part.updated', properties: { part: { id: 'prt-user', sessionID: 'ses-1', messageID: 'msg-user', type: 'text', text: 'hello' } } }, 'ses-1')
  assert.equal(model.turns[0].items[0].type, 'userMessage')
  assert.equal(model.turns[0].items[0].content[0].text, 'hello')
})

test('normalizes configured providers without loading the complete catalog', () => {
  const models = openCodeModelList({
    providers: [{ id: 'openai', name: 'OpenAI', models: { 'gpt-5': { name: 'GPT-5' } } }],
    default: { openai: 'gpt-5' },
  })
  assert.equal(models[0].id, 'openai/gpt-5')
  assert.equal(models[0].isDefault, true)
})

test('splits provider-qualified OpenCode models', () => {
  assert.deepEqual(splitOpenCodeModel('openai/gpt-5.5'), { providerID: 'openai', modelID: 'gpt-5.5' })
  assert.equal(splitOpenCodeModel('gpt-5.5'), null)
})
