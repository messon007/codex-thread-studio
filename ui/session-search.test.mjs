import test from 'node:test'
import assert from 'node:assert/strict'
import {
  filterSessionOccurrences,
  localSessionOccurrences,
  matchedSnippet,
  mergeSessionOccurrences,
  normalizeRemoteSessionOccurrences,
} from './session-search.mjs'

const model = {
  turns: [{
    id: 'turn-1',
    items: [
      { id: 'user-1', type: 'userMessage', content: [{ type: 'text', text: 'Change the working directory' }] },
      { id: 'progress-1', type: 'agentMessage', text: 'Inspecting the working directory now' },
      { id: 'command-1', type: 'commandExecution', command: 'pwd', aggregatedOutput: '/tmp/working directory' },
      { id: 'answer-1', type: 'agentMessage', text: 'The working directory changed.' },
    ],
  }],
}

test('local session search classifies visible messages and activity', () => {
  const entries = localSessionOccurrences(model, 'working directory')
  assert.deepEqual(entries.map((entry) => entry.type), ['user', 'activity', 'activity', 'assistant'])
  assert.ok(entries.every((entry) => entry.snippetMatchRange.end > entry.snippetMatchRange.start))
})

test('remote visible-message occurrences replace local message duplicates', () => {
  const local = localSessionOccurrences(model, 'working directory')
  const remote = [{ turnId: 'turn-1', itemId: 'user-1', snippet: 'working directory', snippetMatchRange: { start: 0, end: 17 } }]
  const merged = mergeSessionOccurrences(remote, local)
  assert.equal(merged.filter((entry) => entry.itemId === 'user-1').length, 1)
  assert.equal(merged.find((entry) => entry.itemId === 'user-1').type, 'user')
  assert.equal(merged.filter((entry) => entry.type === 'activity').length, 2)
})

test('classifies remote occurrences without scanning item text', () => {
  const remoteModel = {
    turns: [{
      id: 'turn-1',
      items: [
        { id: 'user-1', type: 'userMessage' },
        { id: 'tool-1', type: 'commandExecution', get aggregatedOutput() { throw new Error('text was scanned') } },
        { id: 'answer-1', type: 'agentMessage' },
      ],
    }],
  }
  const normalized = normalizeRemoteSessionOccurrences([
    { turnId: 'turn-1', itemId: 'answer-1', snippet: 'answer' },
    { turnId: 'turn-1', itemId: 'user-1', snippet: 'question' },
    { turnId: 'turn-1', itemId: 'tool-1', snippet: 'command' },
  ], remoteModel)

  assert.deepEqual(normalized.map(({ itemId, type }) => [itemId, type]), [
    ['user-1', 'user'],
    ['tool-1', 'activity'],
    ['answer-1', 'assistant'],
  ])
})

test('occurrence filters preserve the requested result type', () => {
  const entries = localSessionOccurrences(model, 'working directory')
  assert.deepEqual(filterSessionOccurrences(entries, 'assistant').map((entry) => entry.itemId), ['answer-1'])
  assert.equal(filterSessionOccurrences(entries, 'all').length, entries.length)
})

test('snippet matching is case insensitive and bounded', () => {
  const match = matchedSnippet(`${'x'.repeat(120)}Important Result${'y'.repeat(120)}`, 'important result')
  assert.ok(match.snippet.startsWith('…'))
  assert.ok(match.snippet.endsWith('…'))
  assert.equal(match.snippet.slice(match.range.start, match.range.end), 'Important Result')
})
