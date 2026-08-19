import assert from 'node:assert/strict'
import test from 'node:test'

import {
  applyOpenCodeEvent,
  collectOpenCodeRootSessions,
  fetchOpenCodeDirectoryStatuses,
  normalizeOpenCodeSessions,
  openCodeModelList,
  openCodeThreadFromHistory,
  splitOpenCodeModel,
} from './opencode-native.mjs'

test('loads every root session with OpenCode cursor pagination', async () => {
  const calls = []
  const pages = [
    [
      { id: 'ses-3', directory: '/work/visible', time: { updated: 30 } },
      { id: 'ses-2', directory: '/work/hidden', time: { updated: 20 } },
    ],
    [{ id: 'ses-1', directory: '/work/visible', time: { updated: 10 } }],
  ]
  const sessions = await collectOpenCodeRootSessions(async (params) => {
    calls.push(params)
    return pages.shift()
  }, 2)

  assert.deepEqual(sessions.map(({ id, directory }) => [id, directory]), [
    ['ses-3', '/work/visible'],
    ['ses-2', '/work/hidden'],
    ['ses-1', '/work/visible'],
  ])
  assert.deepEqual(calls, [
    { limit: 2, archived: false, roots: true },
    { limit: 2, archived: false, roots: true, cursor: 20 },
  ])
})

test('stops safely when an older OpenCode server ignores the cursor', async () => {
  const page = [
    { id: 'ses-2', time: { updated: 20 } },
    { id: 'ses-1', time: { updated: 10 } },
  ]
  let calls = 0
  const sessions = await collectOpenCodeRootSessions(async () => {
    calls += 1
    return page
  }, 2)

  assert.deepEqual(sessions.map(({ id }) => id), ['ses-2', 'ses-1'])
  assert.equal(calls, 2)
})

test('bounds OpenCode directory status request concurrency', async () => {
  let active = 0
  let peak = 0
  const statuses = await fetchOpenCodeDirectoryStatuses(
    ['/work/a', '/work/b', '/work/c', '/work/d', '/work/e', '/work/a'],
    async (directory) => {
      active += 1
      peak = Math.max(peak, active)
      await new Promise((resolve) => setTimeout(resolve, 2))
      active -= 1
      return { [directory]: { type: 'idle' } }
    },
    2,
  )

  assert.equal(peak, 2)
  assert.deepEqual(Object.keys(statuses).sort(), ['/work/a', '/work/b', '/work/c', '/work/d', '/work/e'])
})

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

test('keeps structured OpenCode output readable in native history', () => {
  const decision = { action: 'dispatch', targetSessionKey: 'codex:one' }
  const thread = openCodeThreadFromHistory({ id: 'ses-1', directory: '/tmp/demo' }, [
    { info: { id: 'msg-user', role: 'user' }, parts: [{ id: 'p1', type: 'text', text: 'route this' }] },
    { info: { id: 'msg-agent', parentID: 'msg-user', role: 'assistant', structured: decision }, parts: [] },
  ], { type: 'idle' })
  assert.equal(thread.turns[0].items.at(-1).type, 'agentMessage')
  assert.deepEqual(JSON.parse(thread.turns[0].items.at(-1).text), decision)
})

test('omits OpenCode step lifecycle markers from the visible transcript', () => {
  const thread = openCodeThreadFromHistory({ id: 'ses-1', directory: '/tmp/demo' }, [
    { info: { id: 'msg-user', role: 'user' }, parts: [{ id: 'p1', type: 'text', text: 'hello' }] },
    { info: { id: 'msg-agent', parentID: 'msg-user', role: 'assistant' }, parts: [
      { id: 'step-1', messageID: 'msg-agent', type: 'step-start' },
      { id: 'p2', messageID: 'msg-agent', type: 'reasoning', text: 'thinking' },
      { id: 'p3', messageID: 'msg-agent', type: 'text', text: 'world' },
      { id: 'step-2', messageID: 'msg-agent', type: 'step-finish', reason: 'stop' },
    ] },
  ], { type: 'idle' })

  assert.deepEqual(thread.turns[0].items.map((item) => item.type), ['userMessage', 'reasoning', 'agentMessage'])
})

test('applies streamed OpenCode deltas and status', () => {
  const model = { turns: [{ id: 'turn-1', status: 'inProgress', items: [] }], activeTurnId: 'turn-1', status: 'running', approvals: [], messageTurns: { 'msg-a': 'turn-1' } }
  const result = applyOpenCodeEvent(model, { type: 'message.part.delta', properties: { sessionID: 'ses-1', messageID: 'msg-a', partID: 'part-1', field: 'text', delta: 'hello' } }, 'ses-1')
  assert.equal(result.kind, 'stream')
  assert.equal(model.turns[0].items[0].text, 'hello')
  applyOpenCodeEvent(model, { type: 'session.status', properties: { sessionID: 'ses-1', status: { type: 'idle' } } }, 'ses-1')
  assert.equal(model.status, 'idle')
  assert.equal(model.activeTurnId, null)
  assert.equal(model.turns[0].status, 'completed')
})

test('does not add streamed OpenCode step lifecycle markers to a turn', () => {
  const model = { turns: [{ id: 'turn-1', status: 'inProgress', items: [] }], activeTurnId: 'turn-1', status: 'running', approvals: [], messageTurns: { 'msg-a': 'turn-1' }, messageRoles: { 'msg-a': 'assistant' } }
  applyOpenCodeEvent(model, { type: 'message.part.updated', properties: { part: { id: 'step-1', sessionID: 'ses-1', messageID: 'msg-a', type: 'step-start' } } }, 'ses-1')
  applyOpenCodeEvent(model, { type: 'message.part.updated', properties: { part: { id: 'step-2', sessionID: 'ses-1', messageID: 'msg-a', type: 'step-finish', reason: 'stop' } } }, 'ses-1')
  assert.deepEqual(model.turns[0].items, [])
})

test('keeps streamed user parts as user messages', () => {
  const model = { turns: [], activeTurnId: null, status: 'idle', approvals: [], messageTurns: {} }
  const messageUpdate = applyOpenCodeEvent(model, { type: 'message.updated', properties: { info: { id: 'msg-user', sessionID: 'ses-1', role: 'user' } } }, 'ses-1')
  const partUpdate = applyOpenCodeEvent(model, { type: 'message.part.updated', properties: { part: { id: 'prt-user', sessionID: 'ses-1', messageID: 'msg-user', type: 'text', text: 'hello' } } }, 'ses-1')
  assert.equal(messageUpdate.turnId, 'msg-user')
  assert.equal(partUpdate.turnId, 'msg-user')
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
