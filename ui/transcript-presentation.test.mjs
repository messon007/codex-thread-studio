import test from 'node:test'
import assert from 'node:assert/strict'
import { markTranscriptModelChanged } from './model-revision.mjs'
import {
  TranscriptPresentationCache,
  activityOutputPreview,
  commandKind,
  presentTurn,
  presentRoutedTurn,
  presentationActivityBlocks,
  presentationActivityEntries,
  reasoningStage,
  shouldShowTurnPlaceholder,
} from './transcript-presentation.mjs'

test('Router hides all intermediate responses until completion and publishes only the last result', () => {
  const turn = { id: 'r', status: 'inProgress', items: [
    { id: 'u', type: 'userMessage', content: [] },
    { id: 'a', type: 'agentMessage', text: 'Investigating' },
    { id: 'c', type: 'commandExecution', command: 'ls', status: 'completed' },
    { id: 'b', type: 'agentMessage', text: 'Final result' },
  ] }
  let presentation = presentRoutedTurn(turn)
  assert.deepEqual(presentation.blocks.map(block => block.type), ['activity'])
  assert.deepEqual(presentation.blocks[0].entries.map(entry => entry.itemId), ['a', 'c', 'b'])
  assert.equal(presentation.blocks[0].active, true)
  turn.status = 'completed'
  presentation = presentRoutedTurn(turn)
  assert.deepEqual(presentation.blocks.map(block => block.type), ['activity', 'assistant'])
  assert.equal(presentation.blocks[1].item.id, 'b')
  assert.deepEqual(presentation.blocks[0].entries.map(entry => entry.itemId), ['a', 'c'])
  assert.equal(presentation.blocks[0].active, false)
})

test('Router errors do not publish an intermediate message as a successful final answer', () => {
  const result = presentRoutedTurn({ id: 'r', status: 'failed', error: { message: 'offline' }, items: [{ id: 'a', type: 'agentMessage', text: 'Starting' }] })
  assert.deepEqual(result.blocks.map(block => block.type), ['activity', 'error'])
})

test('groups work items into one activity and keeps the final answer prominent', () => {
  const turn = {
    id: 'turn-1',
    status: 'completed',
    items: [
      { id: 'u', type: 'userMessage', content: [{ type: 'text', text: 'Fix it' }] },
      { id: 'preamble', type: 'agentMessage', text: 'I will inspect the renderer.' },
      { id: 'r', type: 'reasoning', summary: ['**Inspecting the renderer**\nLooking at the current code.'] },
      { id: 'c1', type: 'commandExecution', command: ['rg', '-n', 'renderItem', 'ui'], status: 'completed' },
      { id: 'c2', type: 'commandExecution', command: 'npm test', status: 'completed' },
      { id: 'f', type: 'fileChange', changes: [{ path: 'ui/app.js', kind: 'update' }], status: 'completed' },
      { id: 'answer', type: 'agentMessage', text: 'Done.' },
    ],
  }

  const presentation = presentTurn(turn)
  assert.deepEqual(presentation.blocks.map((block) => block.type), ['user', 'activity', 'assistant'])
  const activity = presentation.blocks[1]
  assert.equal(activity.summary.searches, 1)
  assert.equal(activity.summary.commands, 1)
  assert.equal(activity.summary.changedFiles, 1)
  assert.equal(activity.latestStage, 'Inspecting the renderer')
  assert.equal(presentation.blocks[2].itemId, 'answer')
})

test('merges consecutive reasoning into the activity instead of separate blocks', () => {
  const presentation = presentTurn({
    id: 'turn-2',
    status: 'inProgress',
    items: [
      { id: 'r1', type: 'reasoning', summary: ['**Planning**\nOne'] },
      { id: 'r2', type: 'reasoning', summary: ['**Checking tests**\nTwo'] },
    ],
  })
  assert.equal(presentation.blocks.length, 1)
  assert.equal(presentation.blocks[0].type, 'activity')
  assert.equal(presentation.blocks[0].summary.reasoning, 2)
  assert.equal(presentation.blocks[0].displayEntries.length, 1)
  assert.equal(presentation.blocks[0].displayEntries[0].itemId, 'r2')
  assert.equal(presentation.blocks[0].latestStage, 'Checking tests')
  assert.equal(presentation.blocks[0].active, true)
})

test('keeps the working placeholder beside an optimistic user message', () => {
  const presentation = presentTurn({
    id: 'pending-turn',
    status: 'inProgress',
    items: [{ id: 'user', type: 'userMessage', content: [{ type: 'text', text: 'Hello' }] }],
  })
  assert.equal(shouldShowTurnPlaceholder(presentation), true)

  presentation.blocks.push({ type: 'activity' })
  assert.equal(shouldShowTurnPlaceholder(presentation), false)
})

test('keeps a progress message inside activity until a trailing final answer exists', () => {
  const presentation = presentTurn({
    id: 'turn-progress',
    status: 'inProgress',
    items: [
      { id: 'preamble', type: 'agentMessage', text: 'I will inspect the renderer.' },
      { id: 'command', type: 'commandExecution', command: 'rg renderItem ui', status: 'inProgress' },
    ],
  })
  assert.deepEqual(presentation.blocks.map((block) => block.type), ['activity'])
  assert.deepEqual(presentation.blocks[0].entries.map((entry) => entry.kind), ['progress', 'command'])
})

test('recovers a phase-less final answer displaced before trailing activity in completed history', () => {
  const presentation = presentTurn({
    id: 'turn-displaced-final',
    status: 'completed',
    items: [
      { id: 'user', type: 'userMessage', content: [{ type: 'text', text: 'Organize it' }] },
      { id: 'rs_0', type: 'reasoning', summary: ['Done. Summarize the result.'] },
      { id: 'msg_1', type: 'agentMessage', phase: null, text: 'The completed summary.' },
      { id: 'command-1', type: 'commandExecution', command: 'read files', status: 'completed' },
      { id: 'command-2', type: 'commandExecution', command: 'write docs', status: 'completed' },
    ],
  })

  assert.deepEqual(presentation.blocks.map((block) => block.type), ['user', 'activity', 'assistant'])
  assert.deepEqual(presentation.blocks[1].entries.map((entry) => entry.itemId), ['rs_0', 'command-1', 'command-2'])
  assert.equal(presentation.blocks[2].itemId, 'msg_1')
})

test('does not promote explicit commentary from a completed turn with no final answer', () => {
  const presentation = presentTurn({
    id: 'turn-commentary-only',
    status: 'completed',
    items: [
      { id: 'progress', type: 'agentMessage', phase: 'commentary', text: 'Still checking.' },
      { id: 'command', type: 'commandExecution', command: 'inspect', status: 'completed' },
    ],
  })

  assert.deepEqual(presentation.blocks.map((block) => block.type), ['activity'])
  assert.deepEqual(presentation.blocks[0].entries.map((entry) => entry.itemId), ['progress', 'command'])
})

test('assigns unique activity blocks around a steered user message and keeps the full activity history', () => {
  const presentation = presentTurn({
    id: 'turn-steered',
    status: 'completed',
    items: [
      { id: 'user-1', type: 'userMessage', content: [{ type: 'text', text: 'Start' }] },
      { id: 'progress-1', type: 'agentMessage', phase: 'commentary', text: 'First progress' },
      { id: 'command-1', type: 'commandExecution', command: 'first', status: 'completed' },
      { id: 'user-2', type: 'userMessage', content: [{ type: 'text', text: 'Use another command' }] },
      { id: 'progress-2', type: 'agentMessage', phase: 'commentary', text: 'Second progress' },
      { id: 'command-2', type: 'commandExecution', command: 'second', status: 'completed' },
      { id: 'answer', type: 'agentMessage', phase: 'final_answer', text: 'Done' },
    ],
  })

  const activities = presentationActivityBlocks(presentation)
  assert.deepEqual(presentation.blocks.map((block) => block.type), ['user', 'activity', 'user', 'activity', 'assistant'])
  assert.deepEqual(activities.map((block) => block.id), ['activity-turn-steered-0', 'activity-turn-steered-1'])
  assert.deepEqual(activities.map((block) => block.sourceItemIds), [
    ['progress-1', 'command-1'],
    ['progress-2', 'command-2'],
  ])
  assert.deepEqual(presentationActivityEntries(presentation).map((entry) => entry.itemId), [
    'progress-1', 'command-1', 'progress-2', 'command-2',
  ])
})

test('keeps trailing system activity separate from a final answer without reusing an activity id', () => {
  const presentation = presentTurn({
    id: 'turn-trailing-system',
    status: 'completed',
    items: [
      { id: 'user', type: 'userMessage', content: [{ type: 'text', text: 'Run it' }] },
      { id: 'command', type: 'commandExecution', command: 'run', status: 'completed' },
      { id: 'answer', type: 'agentMessage', phase: 'final_answer', text: 'Done' },
      { id: 'finish', type: 'stepFinish' },
    ],
  })

  const activities = presentationActivityBlocks(presentation)
  assert.deepEqual(presentation.blocks.map((block) => block.type), ['user', 'activity', 'assistant', 'activity'])
  assert.deepEqual(activities.map((block) => block.id), [
    'activity-turn-trailing-system-0',
    'activity-turn-trailing-system-1',
  ])
  assert.deepEqual(presentationActivityEntries(presentation).map((entry) => entry.itemId), ['command', 'finish'])
})

test('output preview retains head and tail without copying the complete output', () => {
  const preview = activityOutputPreview('1\n2\n3\n4\n5\n6\n7', 5)
  assert.deepEqual(preview.lines, ['1', '2', '3', '6', '7'])
  assert.equal(preview.omitted, 2)
  assert.equal(preview.splitAt, 3)
})

test('output preview normalizes CRLF and ignores a trailing newline', () => {
  const preview = activityOutputPreview('one\r\ntwo\r\n', 5)
  assert.deepEqual(preview.lines, ['one', 'two'])
  assert.equal(preview.omitted, 0)
})

test('classifies common exploration commands', () => {
  assert.equal(commandKind(['rg', '-n', 'needle', 'ui']), 'searches')
  assert.equal(commandKind('rg --files ui'), 'lists')
  assert.equal(commandKind('sed -n 1,20p ui/app.js'), 'reads')
  assert.equal(commandKind('cargo test'), 'commands')
})

test('extracts a compact reasoning stage', () => {
  assert.equal(reasoningStage({ summary: ['**Inspecting files**\nMore detail'] }), 'Inspecting files')
  assert.equal(reasoningStage({ content: ['Checking the current implementation\nMore detail'] }), 'Checking the current implementation')
})

test('stops at the newest activity stage and bounds long stage summaries', () => {
  const olderReasoning = { id: 'older', type: 'reasoning' }
  Object.defineProperty(olderReasoning, 'summary', {
    get() { throw new Error('an older stage should not be inspected') },
  })
  const longProgress = `Latest progress ${'x'.repeat(1_000_000)}`
  const presentation = presentTurn({
    id: 'bounded-stage',
    status: 'inProgress',
    items: [
      olderReasoning,
      { id: 'latest', type: 'agentMessage', text: longProgress },
      { id: 'command', type: 'commandExecution', command: 'pwd', status: 'inProgress' },
    ],
  })

  const stage = presentation.blocks[0].latestStage
  assert.equal(stage.length, 180)
  assert.match(stage, /^Latest progress x+…$/u)

  const summary = ['**First bounded stage**\nMore detail']
  Object.defineProperty(summary, 1, {
    get() { throw new Error('a later summary part should not be inspected') },
  })
  assert.equal(reasoningStage({ summary }), 'First bounded stage')
})

test('caches completed turns and only rebuilds a changed turn', () => {
  const cache = new TranscriptPresentationCache({ visibleTurns: 2 })
  const model = {
    turns: [
      { id: '1', status: 'completed', items: [{ id: 'a', type: 'agentMessage', text: 'One' }] },
      { id: '2', status: 'completed', items: [{ id: 'b', type: 'agentMessage', text: 'Two' }] },
      { id: '3', status: 'completed', items: [{ id: 'c', type: 'agentMessage', text: 'Three' }] },
    ],
  }
  const first = cache.get('codex:thread', model)
  const oldTurn = first.turns.get('2').presentation
  assert.equal(first.visibleStart, 1)

  model.turns[2].items[0].text = 'Three updated'
  const second = cache.get('codex:thread', model)
  assert.equal(second.turns.get('2').presentation, oldTurn)
  assert.equal(second.turns.get('3').presentation.blocks[0].item.text, 'Three updated')

  cache.showTurn('codex:thread', model, '1')
  assert.equal(second.visibleStart, 0)
  assert.equal(second.visibleEnd, 2)
})

test('reuses tracked presentation entries without rescanning unchanged item text', () => {
  let reads = 0
  let text = 'Initial response'
  const item = { id: 'answer', type: 'agentMessage' }
  Object.defineProperty(item, 'text', {
    configurable: true,
    enumerable: true,
    get() {
      reads += 1
      return text
    },
  })
  const model = {
    turns: [{ id: '1', status: 'completed', items: [item] }],
  }
  const cache = new TranscriptPresentationCache()
  markTranscriptModelChanged(model)

  const first = cache.get('codex:tracked', model)
  const firstPresentation = first.turns.get('1').presentation
  assert.ok(reads > 0)
  reads = 0

  const cached = cache.get('codex:tracked', model)
  assert.equal(cached, first)
  assert.equal(cached.turns.get('1').presentation, firstPresentation)
  assert.equal(reads, 0)
  assert.equal(cache.peekCurrent('codex:tracked', model), cached)

  text = 'Updated response'
  markTranscriptModelChanged(model)
  const updated = cache.get('codex:tracked', model)
  assert.ok(reads > 0)
  assert.notEqual(updated.turns.get('1').presentation, firstPresentation)
  assert.equal(updated.turns.get('1').presentation.blocks[0].item.text, 'Updated response')
})

test('detects equal-length message replacement and incrementally rebuilds one turn', () => {
  const cache = new TranscriptPresentationCache({ visibleTurns: 2 })
  const model = {
    turns: [
      { id: '1', status: 'completed', items: [{ id: 'a', type: 'agentMessage', text: 'One' }] },
      { id: '2', status: 'inProgress', items: [{ id: 'b', type: 'agentMessage', text: 'Old' }] },
    ],
  }
  const entry = cache.get('thread', model)
  const firstPresentation = entry.turns.get('1').presentation
  model.turns[1].items[0].text = 'New'
  cache.invalidateTurn('thread', '2')
  const updated = cache.updateTurn('thread', model, '2')
  assert.equal(updated.turns.get('1').presentation, firstPresentation)
  assert.equal(updated.turns.get('2').presentation.blocks[0].item.text, 'New')
})

test('rebuilds a cached turn when user message text changes', () => {
  const cache = new TranscriptPresentationCache()
  const model = {
    turns: [{
      id: '1',
      status: 'inProgress',
      items: [{ id: 'question', type: 'userMessage', content: [{ type: 'text', text: 'Original question' }] }],
    }],
  }
  const first = cache.get('thread', model).turns.get('1').presentation
  model.turns[0].items[0].content[0].text = 'Updated question'
  const second = cache.get('thread', model).turns.get('1').presentation

  assert.notEqual(second, first)
  assert.equal(second.blocks[0].item.content[0].text, 'Updated question')
})

test('notices reasoning summary length changes without joining the full summary', () => {
  const cache = new TranscriptPresentationCache()
  const model = {
    turns: [{
      id: 'reasoning-turn',
      status: 'inProgress',
      items: [{
        id: 'reasoning',
        type: 'reasoning',
        summary: ['first', 'second', 'third', 'middle', 'fifth', 'sixth', 'last'],
      }],
    }],
  }
  const first = cache.get('thread', model).turns.get('reasoning-turn').presentation
  model.turns[0].items[0].summary[3] = 'a substantially longer middle summary'
  const second = cache.get('thread', model).turns.get('reasoning-turn').presentation

  assert.notEqual(second, first)
})

test('keeps the rendered history window bounded until the user loads earlier turns', () => {
  const cache = new TranscriptPresentationCache({ visibleTurns: 2 })
  const model = { turns: [1, 2, 3].map((id) => ({ id: String(id), status: 'completed', items: [] })) }
  const entry = cache.get('thread', model)
  assert.equal(entry.visibleStart, 1)
  assert.equal(entry.visibleEnd, 3)
  model.turns.push({ id: '4', status: 'completed', items: [] })
  cache.get('thread', model)
  assert.equal(entry.visibleStart, 2)
  assert.equal(entry.visibleEnd, 4)
  cache.showEarlier('thread', model, 2)
  assert.equal(entry.visibleStart, 0)
  assert.equal(entry.visibleEnd, 4)
})

test('builds presentations only for visible turns and fills newly revealed windows', () => {
  const cache = new TranscriptPresentationCache({ visibleTurns: 2 })
  const hiddenReasoning = { id: 'hidden-reasoning', type: 'reasoning' }
  Object.defineProperty(hiddenReasoning, 'summary', {
    get() { throw new Error('hidden turn presentation should stay lazy') },
  })
  const model = {
    turns: Array.from({ length: 100 }, (_, index) => ({
      id: String(index),
      status: 'completed',
      items: index === 0 ? [hiddenReasoning] : [],
    })),
  }

  const entry = cache.get('thread', model)
  assert.deepEqual([...entry.turns.keys()], ['98', '99'])

  cache.showTurn('thread', model, '2')
  assert.deepEqual([entry.visibleStart, entry.visibleEnd], [1, 3])
  assert.deepEqual([...entry.turns.keys()], ['1', '2'])

  cache.showLater('thread', model, 2)
  assert.deepEqual([entry.visibleStart, entry.visibleEnd], [1, 5])
  assert.deepEqual([...entry.turns.keys()], ['1', '2', '3', '4'])
})

test('keeps a restored anchor in a fixed-size window instead of rendering through the latest turn', () => {
  const cache = new TranscriptPresentationCache({ visibleTurns: 30 })
  const model = {
    turns: Array.from({ length: 500 }, (_, index) => ({
      id: `turn-${index}`,
      status: 'completed',
      items: [],
    })),
  }

  const entry = cache.showTurn('thread', model, 'turn-50')
  assert.equal(entry.visibleStart, 48)
  assert.equal(entry.visibleEnd, 78)
  assert.equal(entry.visibleEnd - entry.visibleStart, 30)

  const resynced = cache.get('thread', model)
  assert.equal(resynced.visibleStart, 48)
  assert.equal(resynced.visibleEnd, 78)
  assert.equal(resynced.historyWindow, 30)
})

test('force-restores an anchor into a centered bounded window', () => {
  const cache = new TranscriptPresentationCache({ visibleTurns: 4 })
  const model = {
    turns: Array.from({ length: 12 }, (_, index) => ({ id: String(index), status: 'completed', items: [] })),
  }
  const expanded = cache.get('thread', model)
  cache.showEarlier('thread', model, 20)
  assert.deepEqual([expanded.visibleStart, expanded.visibleEnd], [0, 12])

  const hiddenReasoning = { id: 'hidden-reasoning', type: 'reasoning' }
  Object.defineProperty(hiddenReasoning, 'summary', {
    get() { throw new Error('force restore should not rebuild the expanded window') },
  })
  model.turns[0].items = [hiddenReasoning]

  const restored = cache.restoreTurn('thread', model, '5')
  assert.equal(restored, expanded)
  assert.deepEqual([restored.visibleStart, restored.visibleEnd], [3, 7])
  assert.deepEqual(restored.orderedIds.slice(restored.visibleStart, restored.visibleEnd), ['3', '4', '5', '6'])
  assert.equal(restored.historyWindow, 4)
  assert.equal(restored.windowMode, 'fixed')
  assert.deepEqual([...restored.turns.keys()], ['3', '4', '5', '6'])
})

test('keeps near-tail restores pinned while new turns arrive', () => {
  const cache = new TranscriptPresentationCache({ visibleTurns: 4 })
  const makeTurn = (id) => ({ id: String(id), status: 'completed', items: [] })
  const model = { turns: Array.from({ length: 8 }, (_, index) => makeTurn(index)) }
  let entry = cache.restoreTurn('thread', model, '7')
  assert.equal(entry.windowMode, 'fixed')
  assert.deepEqual(entry.orderedIds.slice(entry.visibleStart, entry.visibleEnd), ['4', '5', '6', '7'])

  model.turns.push(...Array.from({ length: 5 }, (_, index) => makeTurn(index + 8)))
  entry = cache.get('thread', model)
  assert.equal(entry.windowMode, 'fixed')
  assert.deepEqual(entry.orderedIds.slice(entry.visibleStart, entry.visibleEnd), ['4', '5', '6', '7'])
})

test('returns a pinned reader to the latest window before appending a new user turn', () => {
  const cache = new TranscriptPresentationCache({ visibleTurns: 4 })
  const makeTurn = (id) => ({ id: String(id), status: 'completed', items: [] })
  const model = { turns: Array.from({ length: 8 }, (_, index) => makeTurn(index)) }
  let entry = cache.restoreTurn('thread', model, '7')
  assert.equal(entry.windowMode, 'fixed')

  cache.followLatest('thread', model)
  model.turns.push(makeTurn(8))
  entry = cache.get('thread', model)

  assert.equal(entry.windowMode, 'latest')
  assert.deepEqual(entry.orderedIds.slice(entry.visibleStart, entry.visibleEnd), ['5', '6', '7', '8'])
  assert.equal(entry.orderedIds.length - entry.visibleEnd, 0)
})

test('returns a pinned reader to the latest window immediately', () => {
  const cache = new TranscriptPresentationCache({ visibleTurns: 4 })
  const model = {
    turns: Array.from({ length: 8 }, (_, index) => ({ id: String(index), status: 'completed', items: [] })),
  }
  const entry = cache.showTurn('thread', model, '2')
  assert.deepEqual(entry.orderedIds.slice(entry.visibleStart, entry.visibleEnd), ['0', '1', '2', '3'])

  cache.followLatest('thread', model)

  assert.equal(entry.windowMode, 'latest')
  assert.deepEqual(entry.orderedIds.slice(entry.visibleStart, entry.visibleEnd), ['4', '5', '6', '7'])
  assert.equal(entry.orderedIds.length - entry.visibleEnd, 0)
})

test('pins the current latest window when the reader manually pauses', () => {
  const cache = new TranscriptPresentationCache({ visibleTurns: 4 })
  const makeTurn = (id) => ({ id: String(id), status: 'completed', items: [] })
  const model = { turns: Array.from({ length: 8 }, (_, index) => makeTurn(index)) }
  let entry = cache.get('thread', model)
  assert.deepEqual(entry.orderedIds.slice(entry.visibleStart, entry.visibleEnd), ['4', '5', '6', '7'])

  cache.pinCurrent('thread')
  model.turns.push(...Array.from({ length: 5 }, (_, index) => makeTurn(index + 8)))
  entry = cache.get('thread', model)

  assert.equal(entry.windowMode, 'fixed')
  assert.deepEqual(entry.orderedIds.slice(entry.visibleStart, entry.visibleEnd), ['4', '5', '6', '7'])
})

test('keeps fixed windows on stable turn ids and falls back to bounded latest history', () => {
  const cache = new TranscriptPresentationCache({ visibleTurns: 4 })
  const model = {
    turns: Array.from({ length: 12 }, (_, index) => ({ id: String(index), status: 'completed', items: [] })),
  }
  const entry = cache.showTurn('thread', model, '3')
  assert.deepEqual(entry.orderedIds.slice(entry.visibleStart, entry.visibleEnd), ['1', '2', '3', '4'])

  model.turns = [
    { id: 'older-a', status: 'completed', items: [] },
    { id: 'older-b', status: 'completed', items: [] },
    ...model.turns,
  ]
  cache.get('thread', model)
  assert.deepEqual([entry.visibleStart, entry.visibleEnd], [3, 7])
  assert.deepEqual(entry.orderedIds.slice(entry.visibleStart, entry.visibleEnd), ['1', '2', '3', '4'])

  model.turns = ['new-a', 'new-b', 'new-c'].map((id) => ({ id, status: 'completed', items: [] }))
  cache.get('thread', model)
  assert.equal(entry.windowMode, 'latest')
  assert.deepEqual([entry.visibleStart, entry.visibleEnd], [0, 3])
  assert.deepEqual(entry.orderedIds.slice(entry.visibleStart, entry.visibleEnd), ['new-a', 'new-b', 'new-c'])
})

test('loads omitted turns in either direction without automatically expanding an anchor window', () => {
  const cache = new TranscriptPresentationCache({ visibleTurns: 4 })
  const model = {
    turns: Array.from({ length: 12 }, (_, index) => ({ id: String(index), status: 'completed', items: [] })),
  }

  const entry = cache.showTurn('thread', model, '3')
  assert.deepEqual([entry.visibleStart, entry.visibleEnd], [1, 5])
  cache.showEarlier('thread', model, 1)
  assert.deepEqual([entry.visibleStart, entry.visibleEnd], [0, 5])
  cache.showLater('thread', model, 2)
  assert.deepEqual([entry.visibleStart, entry.visibleEnd], [0, 7])

  model.turns.push({ id: '12', status: 'completed', items: [] })
  cache.get('thread', model)
  assert.deepEqual([entry.visibleStart, entry.visibleEnd], [0, 7])

  cache.showTurn('thread', model, '12')
  assert.deepEqual([entry.visibleStart, entry.visibleEnd], [9, 13])
})

test('peeks at cached presentation metadata without synchronizing a changed model', () => {
  const cache = new TranscriptPresentationCache({ visibleTurns: 2 })
  const model = { turns: [{ id: '1', status: 'completed', items: [] }] }
  const entry = cache.get('thread', model)
  model.turns.push({ id: '2', status: 'completed', items: [] })

  assert.equal(cache.peek('thread'), entry)
  assert.deepEqual(cache.peek('thread').orderedIds, ['1'])
  assert.equal(cache.peek('missing'), null)
})

test('reveals a turn in an existing entry without synchronizing the model again', () => {
  const cache = new TranscriptPresentationCache({ visibleTurns: 2 })
  const model = {
    turns: [1, 2, 3].map((id) => ({ id: String(id), status: 'completed', items: [] })),
  }
  const entry = cache.get('thread', model)
  model.turns.push({ id: '4', status: 'completed', items: [] })

  assert.equal(cache.revealTurn(entry, '1'), entry)
  assert.deepEqual(entry.orderedIds, ['1', '2', '3'])
  assert.deepEqual([entry.visibleStart, entry.visibleEnd], [0, 2])
})

test('keeps per-session reading positions in the in-memory presentation cache', () => {
  const cache = new TranscriptPresentationCache()
  cache.get('codex:first', { turns: [] })
  cache.get('codex:second', { turns: [] })
  cache.setScrollState('codex:first', {
    scrollTop: 640,
    anchorTurnId: 'turn-3',
    anchorOffset: -24,
    followOnReturn: false,
  })

  assert.deepEqual(cache.scrollState('codex:first'), {
    scrollTop: 640,
    anchorTurnId: 'turn-3',
    anchorOffset: -24,
    followOnReturn: false,
  })
  assert.equal(cache.scrollState('codex:second'), null)
})
