import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { transcriptModelRevision } from './model-revision.mjs'

import {
  applyOpenCodeEvent,
  collectOpenCodeMessageHistory,
  collectOpenCodeMessageTail,
  collectOpenCodeRootSessions,
  createOpenCodeLoopGuard,
  fetchOpenCodeDirectoryStatuses,
  mergeOpenCodeThreadTail,
  normalizeOpenCodeSessions,
  openCodeCommandTurn,
  openCodeModelList,
  openCodeThreadFromHistory,
  replayOpenCodeEventsAfterHistory,
  selectOpenCodeStartedUserMessage,
  sliceOpenCodeMessageTail,
  splitOpenCodeModel,
} from './opencode-native.mjs'

test('advances the presentation revision for selected OpenCode events only', () => {
  const model = { turns: [], activeTurnId: null, status: 'idle', error: null }
  assert.equal(transcriptModelRevision(model), null)
  assert.equal(applyOpenCodeEvent(model, {
    type: 'session.status',
    properties: { sessionID: 'ses-1', status: { type: 'busy' } },
  }, 'ses-1').handled, true)
  assert.equal(transcriptModelRevision(model), 1)
  assert.equal(applyOpenCodeEvent(model, {
    type: 'session.status',
    properties: { sessionID: 'ses-2', status: { type: 'idle' } },
  }, 'ses-1').handled, false)
  assert.equal(transcriptModelRevision(model), 1)
})

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

test('paginates complete OpenCode histories and refreshes the newest page', async () => {
  const messages = Array.from({ length: 260 }, (_, index) => {
    const userId = `msg-user-${index}`
    return [
      { info: { id: userId, role: 'user' }, parts: [{ id: `user-part-${index}`, type: 'text', text: `Question ${index}` }] },
      { info: { id: `msg-agent-${index}`, parentID: userId, role: 'assistant' }, parts: [{ id: `agent-part-${index}`, type: 'text', text: `Answer ${index}` }] },
    ]
  }).flat()
  const calls = []
  let newestReads = 0
  const history = await collectOpenCodeMessageHistory(async (params) => {
    calls.push(params)
    if (params.before) return { messages: structuredClone(messages.slice(0, 20)), cursor: null }
    newestReads += 1
    const newest = structuredClone(messages.slice(20))
    if (newestReads === 2) newest.at(-1).parts[0].text = 'Updated while paging'
    return { messages: newest, cursor: 'older-page' }
  })
  const thread = openCodeThreadFromHistory({ id: 'ses-long' }, history.messages, { type: 'idle' })

  assert.equal(messages.length, 520)
  assert.equal(history.complete, true)
  assert.deepEqual(calls, [
    { limit: 500 },
    { limit: 500, before: 'older-page' },
    { limit: 500 },
  ])
  assert.equal(thread.turns.length, 260)
  assert.equal(thread.turns[0].id, 'msg-user-0')
  assert.equal(thread.turns.at(-1).id, 'msg-user-259')
  assert.equal(thread.turns.at(-1).items.at(-1).text, 'Updated while paging')

  const source = readFileSync(new URL('../ui-src/opencode-protocol.mts', import.meta.url), 'utf8')
  const rpcStart = source.indexOf('async function openCodeRpc(')
  const rpcEnd = source.indexOf('\nfunction openCodeFilePart', rpcStart)
  assert.match(source.slice(rpcStart, rpcEnd), /fetchOpenCodeMessageHistory\(params\.threadId, session\.directory, fetchOptions, \{[\s\S]*anchorTurnIds:/u)
})

test('keeps history bounded when an older OpenCode server does not expose a message cursor', async () => {
  const page = Array.from({ length: 500 }, (_, index) => ({ info: { id: `msg-${index}` }, parts: [] }))
  const history = await collectOpenCodeMessageHistory(async () => ({ messages: page, cursor: null }))

  assert.equal(history.complete, false)
  assert.equal(history.messages.length, 500)
  const source = readFileSync(new URL('../ui-src/opencode-protocol.mts', import.meta.url), 'utf8')
  const start = source.indexOf('async function fetchOpenCodeMessageHistory(')
  const end = source.indexOf('\nasync function fetchOpenCodeCatalog', start)
  const fetchHistory = source.slice(start, end)
  assert.match(fetchHistory, /return \{\s*\.\.\.history,/u)
  assert.doesNotMatch(fetchHistory, /openCodeFetch\(withDirectory\(path, directory\), fetchOptions\)/u)
})

test('continues through cursor-backed short pages and completes on a full final page', async () => {
  const calls = []
  const history = await collectOpenCodeMessageHistory(async (params) => {
    calls.push(params)
    if (!params.before) return { messages: [{ info: { id: '3' } }], cursor: 'older' }
    return { messages: [{ info: { id: '1' } }, { info: { id: '2' } }], cursor: null }
  }, 2)

  assert.equal(history.complete, true)
  assert.deepEqual(history.messages.map((message) => message.info.id), ['1', '2', '3'])
  assert.deepEqual(calls, [{ limit: 2 }, { limit: 2, before: 'older' }, { limit: 2 }])
})

test('loads and merges an OpenCode tail across every queued turn until a cached anchor', async () => {
  const messagesFor = (id, answer) => [
    { info: { id, role: 'user' }, parts: [{ id: `${id}-question`, type: 'text', text: `Question ${id}` }] },
    { info: { id: `${id}-assistant`, parentID: id, role: 'assistant' }, parts: [{ id: `${id}-answer`, type: 'text', text: answer }] },
  ]
  const calls = []
  let latestReads = 0
  const history = await collectOpenCodeMessageTail(async (params) => {
    calls.push(params)
    if (!params.before) {
      latestReads += 1
      return { messages: messagesFor('turn-4', latestReads > 1 ? 'four latest' : 'four'), cursor: 'older-1' }
    }
    if (params.before === 'older-1') return { messages: messagesFor('turn-3', 'three'), cursor: 'older-2' }
    return { messages: messagesFor('turn-2', 'two completed'), cursor: 'older-3' }
  }, ['turn-1', 'turn-2'], 2)

  assert.equal(history.matched, true)
  assert.equal(history.anchorTurnId, 'turn-2')
  assert.deepEqual(history.messages.filter((message) => message.info.role === 'user').map((message) => message.info.id), [
    'turn-2', 'turn-3', 'turn-4',
  ])
  assert.deepEqual(calls, [
    { limit: 2 },
    { limit: 2, before: 'older-1' },
    { limit: 2, before: 'older-2' },
    { limit: 2 },
  ])

  const cached = openCodeThreadFromHistory({ id: 'ses-tail' }, [
    ...messagesFor('turn-1', 'one'),
    ...messagesFor('turn-2', 'two streaming'),
  ], { type: 'busy' })
  cached.threadId = cached.id
  cached.historyComplete = true
  const incoming = openCodeThreadFromHistory({ id: 'ses-tail' }, history.messages, { type: 'idle' })
  assert.equal(mergeOpenCodeThreadTail(cached, incoming, history.anchorTurnId), true)
  assert.deepEqual(cached.turns.map((turn) => turn.id), ['turn-1', 'turn-2', 'turn-3', 'turn-4'])
  assert.deepEqual(cached.turns.map((turn) => turn.items.at(-1).text), [
    'one', 'two completed', 'three', 'four latest',
  ])
  assert.equal(cached.messageTurns['turn-1-assistant'], 'turn-1')
  assert.equal(cached.messageTurns['turn-2-assistant'], 'turn-2')
  assert.equal(cached.historyComplete, true)
})

test('reports a complete replacement when an OpenCode tail no longer intersects the cache', async () => {
  const history = await collectOpenCodeMessageTail(async () => ({
    messages: [{ info: { id: 'new-turn', role: 'user' }, parts: [] }],
    cursor: null,
  }), ['old-turn'], 2)

  assert.equal(history.matched, false)
  assert.equal(history.complete, true)
  assert.deepEqual(history.messages.map((message) => message.info.id), ['new-turn'])
})

test('recovers an OpenCode tail from a wider bounded page when the fast page missed its anchor', () => {
  const messages = [
    { info: { id: 'old', role: 'user' } },
    { info: { id: 'old-answer', parentID: 'old', role: 'assistant' } },
    { info: { id: 'anchor', role: 'user' } },
    { info: { id: 'anchor-answer', parentID: 'anchor', role: 'assistant' } },
    { info: { id: 'queued', role: 'user' } },
  ]
  const tail = sliceOpenCodeMessageTail(messages, ['anchor'])

  assert.equal(tail.matched, true)
  assert.equal(tail.anchorTurnId, 'anchor')
  assert.deepEqual(tail.messages.map((message) => message.info.id), [
    'anchor', 'anchor-answer', 'queued',
  ])
})

test('replays buffered OpenCode deltas without losing history prefixes or duplicating captured text', () => {
  const messages = [
    { info: { id: 'msg-user', role: 'user' }, parts: [{ id: 'user-part', type: 'text', text: 'Question' }] },
    { info: { id: 'msg-agent', parentID: 'msg-user', role: 'assistant' }, parts: [{ id: 'agent-part', type: 'text', text: 'prefixsuffix' }] },
  ]
  const historyModel = openCodeThreadFromHistory({ id: 'ses-1' }, messages, { type: 'busy' })
  const delta = {
    type: 'message.part.delta',
    properties: { sessionID: 'ses-1', messageID: 'msg-agent', partID: 'agent-part', field: 'text', delta: 'suffix' },
  }
  replayOpenCodeEventsAfterHistory(historyModel, [{ event: delta, sequence: 1 }], 'ses-1', {
    messageSnapshots: { 'msg-agent': { afterSequence: 1, ambiguousThroughSequence: 1 } },
  })

  assert.equal(historyModel.turns[0].items.at(-1).text, 'prefixsuffix')

  const earlierHistory = openCodeThreadFromHistory({ id: 'ses-1' }, [messages[0], {
    ...messages[1],
    parts: [{ id: 'agent-part', type: 'text', text: 'prefix' }],
  }], { type: 'busy' })
  replayOpenCodeEventsAfterHistory(earlierHistory, [{ event: delta, sequence: 2 }], 'ses-1', {
    messageSnapshots: { 'msg-agent': { afterSequence: 1, ambiguousThroughSequence: 1 } },
  })
  assert.equal(earlierHistory.turns[0].items.at(-1).text, 'prefixsuffix')

  const streamingHistory = openCodeThreadFromHistory({ id: 'ses-1' }, [messages[0], {
    ...messages[1],
    parts: [{ id: 'agent-part', type: 'text', text: '' }],
  }], { type: 'busy' })
  replayOpenCodeEventsAfterHistory(streamingHistory, [{
    event: { ...delta, properties: { ...delta.properties, delta: 'foo' } },
    sequence: 1,
  }], 'ses-1', {
    messageSnapshots: { 'msg-agent': { afterSequence: 1, ambiguousThroughSequence: 1 } },
  })
  assert.equal(streamingHistory.turns[0].items.at(-1).text, 'foo')

  const postSnapshotHistory = openCodeThreadFromHistory({ id: 'ses-1' }, [messages[0], {
    ...messages[1],
    parts: [{ id: 'agent-part', type: 'text', text: 'hello' }],
  }], { type: 'busy' })
  replayOpenCodeEventsAfterHistory(postSnapshotHistory, [{
    event: { ...delta, properties: { ...delta.properties, delta: 'o' } },
    sequence: 2,
  }], 'ses-1', {
    messageSnapshots: { 'msg-agent': { afterSequence: 1, ambiguousThroughSequence: 1 } },
  })
  assert.equal(postSnapshotHistory.turns[0].items.at(-1).text, 'helloo')

  const interleavedHistory = openCodeThreadFromHistory({ id: 'ses-1' }, [messages[0], {
    ...messages[1],
    parts: [{ id: 'agent-part', type: 'text', text: '' }],
  }], { type: 'busy' })
  replayOpenCodeEventsAfterHistory(interleavedHistory, [
    { event: { ...delta, properties: { ...delta.properties, delta: 'a' } }, sequence: 1 },
    { event: { type: 'session.status', properties: { sessionID: 'ses-1', status: { type: 'busy' } } }, sequence: 2 },
    { event: { ...delta, properties: { ...delta.properties, delta: 'a' } }, sequence: 3 },
  ], 'ses-1')
  assert.equal(interleavedHistory.turns[0].items.at(-1).text, 'aa')

  const reasoningHistory = openCodeThreadFromHistory({ id: 'ses-1' }, [messages[0], {
    ...messages[1],
    parts: [{ id: 'agent-part', type: 'reasoning', text: 'think' }],
  }], { type: 'busy' })
  replayOpenCodeEventsAfterHistory(reasoningHistory, [{
    event: { ...delta, properties: { ...delta.properties, delta: 'ing' } },
    sequence: 2,
  }], 'ses-1', {
    messageSnapshots: { 'msg-agent': { afterSequence: 1, ambiguousThroughSequence: 1 } },
  })
  assert.equal(reasoningHistory.turns[0].items.at(-1).content[0], 'thinking')
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  assert.match(source, /beginOpenCodeHistoryEventBuffer\(id\)[\s\S]*installSelectedHistory\([\s\S]*replayOpenCodeEventsAfterHistory\(model, events, id, options\)[\s\S]*endOpenCodeHistoryEventBuffer\(id, historyEvents\)/u)
assert.match(readFileSync(new URL('../ui-src/background-sessions.mts', import.meta.url), 'utf8'), /bufferOpenCodeHistoryEvent\(eventThreadId, event\)/u)
})

test('reapplies an authoritative idle snapshot after earlier buffered content events', () => {
  const model = openCodeThreadFromHistory({ id: 'ses-1' }, [], { type: 'idle' })
  replayOpenCodeEventsAfterHistory(model, [
    {
      sequence: 1,
      event: {
        type: 'message.updated',
        properties: { sessionID: 'ses-1', info: { id: 'msg-user', role: 'user' } },
      },
    },
    {
      sequence: 2,
      event: { type: 'session.idle', properties: { sessionID: 'ses-1' } },
    },
  ], 'ses-1', {
    statusAfterSequence: 2,
    authoritativeStatus: 'idle',
  })

  assert.equal(model.status, 'idle')
  assert.equal(model.activeTurnId, null)
  assert.equal(model.turns[0].status, 'completed')

  replayOpenCodeEventsAfterHistory(model, [{
    sequence: 3,
    event: {
      type: 'message.updated',
      properties: { sessionID: 'ses-1', info: { id: 'msg-next', role: 'user' } },
    },
  }], 'ses-1', {
    statusAfterSequence: 2,
    authoritativeStatus: 'idle',
  })
  assert.equal(model.status, 'running')
  assert.equal(model.activeTurnId, 'msg-next')

  const errorModel = openCodeThreadFromHistory({ id: 'ses-1' }, [], { type: 'idle' })
  replayOpenCodeEventsAfterHistory(errorModel, [{
    sequence: 1,
    event: {
      type: 'session.error',
      properties: { sessionID: 'ses-1', error: { message: 'Rate limited' } },
    },
  }], 'ses-1', { statusAfterSequence: 1, authoritativeStatus: 'idle' })
  assert.equal(errorModel.status, 'idle')
  assert.equal(errorModel.error, 'Rate limited')
})

test('does not replay stale full-part snapshots already covered by the latest history page', () => {
  const model = openCodeThreadFromHistory({ id: 'ses-1' }, [
    { info: { id: 'msg-user', role: 'user' }, parts: [{ id: 'user-part', type: 'text', text: 'Question' }] },
    { info: { id: 'msg-agent', parentID: 'msg-user', role: 'assistant' }, parts: [{ id: 'agent-part', type: 'text', text: 'foo' }] },
  ], { type: 'busy' })
  replayOpenCodeEventsAfterHistory(model, [
    {
      sequence: 1,
      event: {
        type: 'message.part.updated',
        properties: {
          sessionID: 'ses-1',
          part: { id: 'agent-part', messageID: 'msg-agent', type: 'text', text: '' },
        },
      },
    },
    {
      sequence: 2,
      event: {
        type: 'message.part.delta',
        properties: { sessionID: 'ses-1', messageID: 'msg-agent', partID: 'agent-part', field: 'text', delta: 'foo' },
      },
    },
  ], 'ses-1', {
    messageSnapshots: { 'msg-agent': { afterSequence: 2, ambiguousThroughSequence: 2 } },
  })

  assert.equal(model.turns[0].items.at(-1).text, 'foo')

  const transformedModel = openCodeThreadFromHistory({ id: 'ses-1' }, [
    {
      info: { id: 'transform-user', role: 'user' },
      parts: [{ id: 'transform-user-part', type: 'text', text: 'Transform' }],
    },
    {
      info: { id: 'transform-agent', parentID: 'transform-user', role: 'assistant' },
      parts: [{ id: 'transform-part', type: 'text', text: 'bar' }],
    },
  ], { type: 'idle' })
  replayOpenCodeEventsAfterHistory(transformedModel, [
    {
      sequence: 1,
      event: {
        type: 'message.part.delta',
        properties: { sessionID: 'ses-1', messageID: 'transform-agent', partID: 'transform-part', field: 'text', delta: 'foo' },
      },
    },
    {
      sequence: 2,
      event: {
        type: 'message.part.updated',
        properties: {
          sessionID: 'ses-1',
          part: { id: 'transform-part', messageID: 'transform-agent', type: 'text', text: 'bar' },
        },
      },
    },
  ], 'ses-1', {
    messageSnapshots: { 'transform-agent': { afterSequence: 2, ambiguousThroughSequence: 2 } },
  })
  assert.equal(transformedModel.turns[0].items.at(-1).text, 'bar')

  const toolModel = openCodeThreadFromHistory({ id: 'ses-1' }, [
    { info: { id: 'tool-user', role: 'user' }, parts: [{ id: 'tool-user-part', type: 'text', text: 'Run' }] },
    { info: { id: 'tool-agent', parentID: 'tool-user', role: 'assistant' }, parts: [{
      id: 'tool-part',
      messageID: 'tool-agent',
      type: 'tool',
      tool: 'bash',
      state: { status: 'completed', input: { command: 'pwd' }, output: '/tmp' },
    }] },
  ], { type: 'idle' })
  replayOpenCodeEventsAfterHistory(toolModel, [{
    sequence: 1,
    event: {
      type: 'message.part.updated',
      properties: {
        sessionID: 'ses-1',
        part: {
          id: 'tool-part',
          messageID: 'tool-agent',
          type: 'tool',
          tool: 'bash',
          state: { status: 'running', input: { command: 'pwd' } },
        },
      },
    },
  }], 'ses-1', {
    messageSnapshots: { 'tool-agent': { afterSequence: 1, ambiguousThroughSequence: 1 } },
  })
  assert.equal(toolModel.turns[0].items.at(-1).status, 'completed')

  const recreatedPartModel = openCodeThreadFromHistory({ id: 'ses-1' }, [
    {
      info: { id: 'recreate-user', role: 'user' },
      parts: [{ id: 'recreate-user-part', type: 'text', text: 'Recreate' }],
    },
    {
      info: { id: 'recreate-agent', parentID: 'recreate-user', role: 'assistant' },
      parts: [{ id: 'recreate-part', type: 'text', text: 'new' }],
    },
  ], { type: 'idle' })
  replayOpenCodeEventsAfterHistory(recreatedPartModel, [
    {
      sequence: 1,
      event: {
        type: 'message.part.removed',
        properties: { sessionID: 'ses-1', messageID: 'recreate-agent', partID: 'recreate-part' },
      },
    },
    {
      sequence: 2,
      event: {
        type: 'message.part.updated',
        properties: {
          sessionID: 'ses-1',
          part: { id: 'recreate-part', messageID: 'recreate-agent', type: 'text', text: 'new' },
        },
      },
    },
  ], 'ses-1', {
    messageSnapshots: { 'recreate-agent': { afterSequence: 2, ambiguousThroughSequence: 2 } },
  })
  assert.equal(recreatedPartModel.turns[0].items.at(-1).text, 'new')

  const olderPageModel = openCodeThreadFromHistory({ id: 'ses-1' }, [
    { info: { id: 'old-user', role: 'user' }, parts: [{ id: 'old-user-part', type: 'text', text: 'Old' }] },
    { info: { id: 'old-agent', parentID: 'old-user', role: 'assistant' }, parts: [{ id: 'old-part', type: 'text', text: 'a' }] },
  ], { type: 'busy' })
  replayOpenCodeEventsAfterHistory(olderPageModel, [{
    sequence: 1,
    event: {
      type: 'message.part.delta',
      properties: { sessionID: 'ses-1', messageID: 'old-agent', partID: 'old-part', field: 'text', delta: 'b' },
    },
  }], 'ses-1', {
    messageSnapshots: {
      'old-agent': { afterSequence: 0, ambiguousThroughSequence: 0 },
      'new-agent': { afterSequence: 1, ambiguousThroughSequence: 1 },
    },
  })
  assert.equal(olderPageModel.turns[0].items.at(-1).text, 'ab')
})

test('removes OpenCode parts and their owning messages from live history', () => {
  const model = openCodeThreadFromHistory({ id: 'ses-1' }, [
    { info: { id: 'msg-user', role: 'user' }, parts: [{ id: 'user-part', type: 'text', text: 'Question' }] },
    { info: { id: 'msg-agent', parentID: 'msg-user', role: 'assistant' }, parts: [
      { id: 'part-a', type: 'text', text: 'Answer' },
      { id: 'part-b', type: 'reasoning', text: 'Thought' },
    ] },
  ], { type: 'busy' })

  applyOpenCodeEvent(model, {
    type: 'message.part.removed',
    properties: { sessionID: 'ses-1', messageID: 'msg-agent', partID: 'part-b' },
  }, 'ses-1')
  assert.deepEqual(model.turns[0].items.map((item) => item.id), ['msg-user', 'part-a'])

  applyOpenCodeEvent(model, {
    type: 'message.removed',
    properties: { sessionID: 'ses-1', messageID: 'msg-agent' },
  }, 'ses-1')
  assert.deepEqual(model.turns[0].items.map((item) => item.id), ['msg-user'])
  assert.equal(model.messageTurns['msg-agent'], undefined)

  const structuredModel = openCodeThreadFromHistory({ id: 'ses-1' }, [
    { info: { id: 'structured-user', role: 'user' }, parts: [] },
  ], { type: 'busy' })
  applyOpenCodeEvent(structuredModel, {
    type: 'message.updated',
    properties: {
      sessionID: 'ses-1',
      info: { id: 'structured-agent', parentID: 'structured-user', role: 'assistant', structured: { ok: true } },
    },
  }, 'ses-1')
  applyOpenCodeEvent(structuredModel, {
    type: 'message.removed',
    properties: { sessionID: 'ses-1', messageID: 'structured-agent' },
  }, 'ses-1')
  assert.deepEqual(structuredModel.turns[0].items.map((item) => item.id), ['structured-user'])

  const erroredModel = openCodeThreadFromHistory({ id: 'ses-1' }, [
    { info: { id: 'error-user', role: 'user' }, parts: [{ id: 'question', type: 'text', text: 'Question' }] },
    {
      info: {
        id: 'error-agent',
        parentID: 'error-user',
        role: 'assistant',
        error: { message: 'Provider failed' },
      },
      parts: [],
    },
  ], { type: 'idle' })
  assert.equal(erroredModel.turns[0].error.message, 'Provider failed')
  applyOpenCodeEvent(erroredModel, {
    type: 'message.removed',
    properties: { sessionID: 'ses-1', messageID: 'error-agent' },
  }, 'ses-1')
  assert.equal(erroredModel.turns[0].error, undefined)
  assert.equal(erroredModel.turns[0].status, 'completed')

  const deletedBeforeSnapshot = openCodeThreadFromHistory({ id: 'ses-1' }, [], { type: 'busy' })
  replayOpenCodeEventsAfterHistory(deletedBeforeSnapshot, [
    {
      sequence: 1,
      event: {
        type: 'message.part.delta',
        properties: { sessionID: 'ses-1', messageID: 'ghost-agent', partID: 'ghost-part', field: 'text', delta: 'ghost' },
      },
    },
    {
      sequence: 2,
      event: { type: 'message.removed', properties: { sessionID: 'ses-1', messageID: 'ghost-agent' } },
    },
  ], 'ses-1')
  assert.deepEqual(deletedBeforeSnapshot.turns, [])
})

test('replaying a user part update preserves sibling user content and supports part removal', () => {
  const model = openCodeThreadFromHistory({ id: 'ses-1' }, [{
    info: { id: 'msg-user', role: 'user' },
    parts: [
      { id: 'part-a', type: 'text', text: 'first' },
      { id: 'part-b', type: 'text', text: 'second' },
    ],
  }], { type: 'busy' })

  replayOpenCodeEventsAfterHistory(model, [{
    type: 'message.part.updated',
    properties: {
      sessionID: 'ses-1',
      part: { id: 'part-b', messageID: 'msg-user', type: 'text', text: 'updated' },
    },
  }], 'ses-1')
  assert.deepEqual(model.turns[0].items[0].content.map((item) => item.text), ['first', 'updated'])

  applyOpenCodeEvent(model, {
    type: 'message.part.removed',
    properties: { sessionID: 'ses-1', messageID: 'msg-user', partID: 'part-a' },
  }, 'ses-1')
  assert.deepEqual(model.turns[0].items[0].content.map((item) => item.text), ['updated'])
})

test('preserves image file parts as structured user images', () => {
  const url = 'data:image/png;base64,iVBORw0KGgo='
  const thread = openCodeThreadFromHistory({ id: 'ses-image', directory: '/tmp/demo' }, [
    { info: { id: 'msg-user', role: 'user' }, parts: [
      { id: 'p1', type: 'text', text: 'inspect this' },
      { id: 'p2', type: 'file', mime: 'image/png', filename: 'image.png', url },
    ] },
  ], { type: 'idle' })
  assert.deepEqual(thread.turns[0].items[0].content, [
    { type: 'text', text: 'inspect this' },
    { type: 'image', url },
  ])
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

test('groups streamed user image parts with their prompt', () => {
  const model = { turns: [], activeTurnId: null, status: 'idle', approvals: [], messageTurns: {} }
  const url = 'data:image/png;base64,iVBORw0KGgo='
  applyOpenCodeEvent(model, { type: 'message.updated', properties: { info: { id: 'msg-user', sessionID: 'ses-1', role: 'user' } } }, 'ses-1')
  applyOpenCodeEvent(model, { type: 'message.part.updated', properties: { part: { id: 'text', sessionID: 'ses-1', messageID: 'msg-user', type: 'text', text: 'inspect' } } }, 'ses-1')
  applyOpenCodeEvent(model, { type: 'message.part.updated', properties: { part: { id: 'image', sessionID: 'ses-1', messageID: 'msg-user', type: 'file', mime: 'image/png', url } } }, 'ses-1')
  assert.equal(model.turns[0].items.length, 1)
  assert.deepEqual(model.turns[0].items[0].content, [{ type: 'text', text: 'inspect' }, { type: 'image', url }])
})

test('keeps multiple streamed user text parts in event order', () => {
  const model = { turns: [], activeTurnId: null, status: 'idle', approvals: [], messageTurns: {} }
  applyOpenCodeEvent(model, { type: 'message.updated', properties: { info: { id: 'msg-user', sessionID: 'ses-1', role: 'user' } } }, 'ses-1')
  applyOpenCodeEvent(model, { type: 'message.part.updated', properties: { part: { id: 'first', sessionID: 'ses-1', messageID: 'msg-user', type: 'text', text: 'first' } } }, 'ses-1')
  applyOpenCodeEvent(model, { type: 'message.part.updated', properties: { part: { id: 'second', sessionID: 'ses-1', messageID: 'msg-user', type: 'text', text: 'second' } } }, 'ses-1')
  assert.deepEqual(model.turns[0].items[0].content, [{ type: 'text', text: 'first' }, { type: 'text', text: 'second' }])
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

test('uses authoritative command and post-baseline user identities', () => {
  const commandTurn = openCodeCommandTurn({
    info: { id: 'assistant', parentID: 'user-command', role: 'assistant', finish: 'stop' },
    parts: [{ id: 'answer', messageID: 'assistant', type: 'text', text: 'done' }],
  }, [{ type: 'skill', name: 'review' }])
  assert.equal(commandTurn.id, 'user-command')
  assert.equal(commandTurn.status, 'completed')
  assert.equal(commandTurn.items.at(-1).text, 'done')

  const messages = [
    { info: { id: 'old-user', role: 'user' }, parts: [{ type: 'text', text: 'same prompt' }] },
    { info: { id: 'other-new-user', role: 'user' }, parts: [{ type: 'text', text: 'different prompt' }] },
    { info: { id: 'expected-new-user', role: 'user' }, parts: [{ type: 'text', text: 'same prompt' }] },
  ]
  assert.equal(selectOpenCodeStartedUserMessage(messages, {
    baselineIds: ['old-user'],
    expectedText: 'same prompt',
  })?.info.id, 'expected-new-user')
  assert.equal(selectOpenCodeStartedUserMessage(messages.slice(0, 2), {
    baselineIds: ['old-user'],
    expectedText: 'same prompt',
  }), null)
})

test('Studio leaves OpenCode message identity to the server', () => {
  const source = readFileSync(new URL('../ui-src/opencode-protocol.mts', import.meta.url), 'utf8') + readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const start = source.indexOf("if (method === 'turn/start')")
  const end = source.indexOf("throw new Error(t('The OpenCode backend", start)
  const turnStart = source.slice(start, end)
  assert.match(turnStart, /prompt_async/u)
  assert.match(turnStart, /return openCodeStartedTurn\(/u)
  assert.match(turnStart, /return openCodeStartedTurn\([\s\S]*allowInactive: true/u)
  assert.match(turnStart, /openCodeUserMessageBaseline/u)
  assert.match(turnStart, /openCodeCommandTurn/u)
  assert.doesNotMatch(turnStart, /messageID|openCodeMessageId/u)
  for (const newline of ['\n', '\r\n']) {
    const checkoutSource = source.replace(/\r?\n/gu, newline)
    const adapterStart = checkoutSource.indexOf(".register('opencode'")
    const adapterEnd = checkoutSource.indexOf('marked.setOptions', adapterStart)
    assert.ok(adapterStart >= 0 && adapterEnd > adapterStart, 'OpenCode adapter boundaries exist')
    assert.doesNotMatch(checkoutSource.slice(adapterStart, adapterEnd), /clientUserMessageId|messageID/u)
  }
})

test('OpenCode keeps one cross-backend event stream and refreshes after an SSE continuity gap', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const connect = source.slice(
    source.indexOf('async function connectOpenCode('),
    source.indexOf('\nfunction cleanupSocket', source.indexOf('async function connectOpenCode(')),
  )
  const stream = source.slice(
    source.indexOf('function startOpenCodeEventStream('),
    source.indexOf('\nfunction connectCodexLifecycleStream', source.indexOf('function startOpenCodeEventStream(')),
  )
  const cleanup = source.slice(
    source.indexOf('function cleanupConnections('),
    source.indexOf('\nfunction beginTurnLatencyTrace', source.indexOf('function cleanupConnections(')),
  )
const events = readFileSync(new URL('../ui-src/background-sessions.mts', import.meta.url), 'utf8')
  const connectedBranch = events.slice(
    events.indexOf("if (payload.type === 'server.connected')"),
    events.indexOf("if (payload.type.startsWith('session.'))"),
  )
  const lifecycle = readFileSync(new URL('./lifecycle-connection.mjs', import.meta.url), 'utf8')
  assert.match(stream, /connectEventStream\(openCodeStreamState/u)
  assert.match(lifecycle, /if \(state\.stream\)[\s\S]*return state\.stream/u)
  assert.match(lifecycle, /events\.onopen[\s\S]*state\.openCount \+= 1/u)
  assert.match(connectedBranch, /server\.connected[\s\S]*EventSource\.onopen owns the continuity epoch/u)
  assert.doesNotMatch(connectedBranch, /scheduleOpenCodeListRefresh/u)
  assert.match(lifecycle, /state\.openCount > 0 \|\| state\.missedBarrier/u)
  assert.match(stream, /if \(needsReconciliation\)[\s\S]*openCodeHistoryEpoch \+= 1[\s\S]*scheduleOpenCodeListRefresh/u)
  assert.match(connect, /await waitForOpenCodeEventStream\(\)[\s\S]*openCodeStreamState\.missedBarrier = true[\s\S]*loadThreads\(\)/u)
  assert.doesNotMatch(cleanup, /openCodeEventStream|openCodeStatusReconcileTimers/u)
  assert.match(events, /const selectedTarget = state\.backend === 'opencode' && eventThreadId === state\.selectedId/u)
  assert.match(source, /state\.backend === 'opencode' && state\.ready[\s\S]*refreshOpenCodeThreadList[\s\S]*refreshBackendCatalog\('opencode'\)/u)
  assert.match(source, /backend === 'opencode' && cached\.historyEpoch !== openCodeHistoryEpoch/u)
  assert.match(source, /historyEpoch = backend === 'opencode' \? openCodeHistoryEpoch : null/u)
  assert.match(source, /storeCachedSession\(state\.threadModels, backend, id, model, historyEpoch, Date\.now\(\)\)/u)
  assert.match(source, /const historyEpoch = backend === 'opencode' \? openCodeHistoryEpoch : null[\s\S]*cacheThreadModel\(backend, id, model, \{ historyEpoch \}\)/u)
  assert.match(events, /const historyEpoch = ref\.backend === 'opencode' \? getOpenCodeHistoryEpoch\(\) : null[\s\S]*cacheThreadModel\(ref\.backend, ref\.id, model, \{ historyEpoch \}\)/u)
})

test('OpenCode never reuses or installs a history load from an older connection epoch', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const resume = source.slice(
    source.indexOf('async function resumeThread('),
    source.indexOf('\nasync function refreshSelectedThread(', source.indexOf('async function resumeThread(')),
  )
  const refresh = source.slice(
    source.indexOf('async function refreshSelectedThread('),
    source.indexOf('\nfunction freshThreadModel(', source.indexOf('async function refreshSelectedThread(')),
  )

  for (const load of [resume, refresh]) {
    assert.match(load, /coordinateHistoryLoad\(state\.threadLoads, key, backend, historyEpoch,/u)
    const epochGuard = load.indexOf('historyEpoch !== openCodeHistoryEpoch')
    const hydrate = load.indexOf('installSelectedHistory(')
    assert.ok(epochGuard >= 0 && epochGuard < hydrate)
    const catchBranch = load.slice(load.indexOf('} catch (error) {'))
    assert.ok(catchBranch.indexOf('historyEpoch !== openCodeHistoryEpoch')
      < catchBranch.indexOf('state.backend !== backend'))
  }
})

test('OpenCode catalog refresh cannot start history for a newly selected session', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const start = source.indexOf('async function refreshOpenCodeThreadList(')
  const end = source.indexOf('\nfunction rpc(', start)
  const refresh = source.slice(start, end)
  assert.match(refresh, /const selectedId = state\.selectedId/u)
  assert.match(refresh, /forceSelectedHistory \|\| !freshThreadModel\('opencode', selectedId\)/u)
  assert.match(refresh, /state\.selectedId !== selectedId/u)
  assert.match(refresh, /threadCatalogKey\('opencode', selectedId\)/u)
  assert.ok(refresh.lastIndexOf('state.selectedId !== selectedId') < refresh.indexOf('await refreshSelectedThread('))
})

test('trips an OpenCode loop guard only on repeated empty terminal assistants', () => {
  const guard = createOpenCodeLoopGuard()
  const update = (id, finish = 'stop') => ({
    type: 'message.updated',
    properties: {
      sessionID: 'session-1',
      info: { id, role: 'assistant', parentID: 'user-1', finish },
    },
  })

  guard.observe({
    type: 'message.part.updated',
    properties: { part: { sessionID: 'session-1', messageID: 'assistant-tool', id: 'tool-1', type: 'tool' } },
  })
  assert.equal(guard.observe(update('assistant-tool')), null)
  guard.observe({
    type: 'message.part.updated',
    properties: { part: { sessionID: 'session-1', messageID: 'assistant-file', id: 'file-1', type: 'file', url: 'file:///tmp/result' } },
  })
  assert.equal(guard.observe(update('assistant-file')), null)
  assert.equal(guard.observe({
    type: 'message.updated',
    properties: {
      sessionID: 'session-1',
      info: { id: 'assistant-output', role: 'assistant', parentID: 'user-1', finish: 'stop', tokens: { output: 2 } },
    },
  }), null)
  assert.equal(guard.observe(update('assistant-empty-1')), null)
  assert.equal(guard.observe(update('assistant-empty-1')), null)
  assert.equal(guard.observe(update('assistant-empty-2')), null)
  assert.deepEqual(guard.observe(update('assistant-empty-3')), {
    sessionId: 'session-1',
    parentId: 'user-1',
    assistantIds: ['assistant-empty-1', 'assistant-empty-2', 'assistant-empty-3'],
  })
  assert.deepEqual(guard.observe(update('assistant-empty-4'))?.assistantIds, [
    'assistant-empty-1', 'assistant-empty-2', 'assistant-empty-3', 'assistant-empty-4',
  ])
})

test('an idle OpenCode session resets the duplicate-terminal loop guard', () => {
  const guard = createOpenCodeLoopGuard()
  const update = (id) => ({
    type: 'message.updated',
    properties: { sessionID: 'session-1', info: { id, role: 'assistant', parentID: 'user-1', finish: 'stop' } },
  })
  guard.observe(update('assistant-1'))
  guard.observe({ type: 'session.status', properties: { sessionID: 'session-1', status: { type: 'idle' } } })
  assert.equal(guard.observe(update('assistant-2')), null)
})

test('Studio aborts a live OpenCode session when the terminal loop guard trips', () => {
  const source = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const handlerStart = source.indexOf('function handleOpenCodeServerEvent(')
  const handlerEnd = source.indexOf('\nfunction openCodeCompletionSignal', handlerStart)
  const handler = source.slice(handlerStart, handlerEnd)
assert.match(readFileSync(new URL('../ui-src/background-sessions.mts', import.meta.url), 'utf8'), /openCodeLoopGuard\.observe\(payload\)/u)
  assert.match(handler, /\/session\/\$\{encodeURIComponent\(sessionId\)\}\/abort/u)
  assert.match(handler, /openCodeLoopAbortRequests\.has\(sessionId\)/u)
})
