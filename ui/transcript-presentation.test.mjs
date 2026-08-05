import test from 'node:test'
import assert from 'node:assert/strict'
import {
  TranscriptPresentationCache,
  activityOutputPreview,
  commandKind,
  presentTurn,
  reasoningStage,
} from './transcript-presentation.mjs'

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
  const oldTurn = first.turns.get('1').presentation
  assert.equal(first.visibleStart, 1)

  model.turns[2].items[0].text = 'Three updated'
  const second = cache.get('codex:thread', model)
  assert.equal(second.turns.get('1').presentation, oldTurn)
  assert.equal(second.turns.get('3').presentation.blocks[0].item.text, 'Three updated')

  cache.showTurn('codex:thread', model, '1')
  assert.equal(second.visibleStart, 0)
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

test('keeps the rendered history window bounded until the user loads earlier turns', () => {
  const cache = new TranscriptPresentationCache({ visibleTurns: 2 })
  const model = { turns: [1, 2, 3].map((id) => ({ id: String(id), status: 'completed', items: [] })) }
  const entry = cache.get('thread', model)
  assert.equal(entry.visibleStart, 1)
  model.turns.push({ id: '4', status: 'completed', items: [] })
  cache.get('thread', model)
  assert.equal(entry.visibleStart, 2)
  cache.showEarlier('thread', model, 2)
  assert.equal(entry.visibleStart, 0)
})
