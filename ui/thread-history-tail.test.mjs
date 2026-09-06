import test from 'node:test'
import assert from 'node:assert/strict'

import {
  collectCodexTurnTail,
  isHistoryPaginationCompatibilityError,
} from './thread-history-tail.mjs'

test('replaces the inclusive anchor when the latest turn changed', async () => {
  const cachedTurns = [turn('one', 'old one'), turn('two', 'old two')]
  const result = await collectCodexTurnTail({
    cachedTurns,
    initialPage: page([turn('two', 'new two')], null, 'newest'),
  })

  assert.equal(result.matched, true)
  assert.equal(result.anchorTurnId, 'two')
  assert.deepEqual(result.turns.map(answer), ['old one', 'new two'])
  assert.equal(result.appendedTurnCount, 0)
})

test('appends every queued turn across as many pages as needed', async () => {
  const cachedTurns = [turn('one'), turn('anchor', 'before completion')]
  const pages = new Map([
    ['older-1', page([turn('queued-2'), turn('queued-1')], 'older-2')],
    ['older-2', page([turn('anchor', 'completed')])],
  ])
  const calls = []
  const result = await collectCodexTurnTail({
    cachedTurns,
    initialPage: page([turn('queued-4'), turn('queued-3')], 'older-1', 'latest-anchor'),
    fetchPage: async (cursor) => {
      calls.push(cursor)
      return pages.get(cursor)
    },
  })

  assert.equal(result.matched, true)
  assert.deepEqual(calls, ['older-1', 'older-2'])
  assert.deepEqual(result.turns.map(({ id }) => id), [
    'one', 'anchor', 'queued-1', 'queued-2', 'queued-3', 'queued-4',
  ])
  assert.equal(answer(result.turns[1]), 'completed')
  assert.equal(result.appendedTurnCount, 4)
  assert.equal(result.pageCount, 3)
})

test('truncates a cached suffix after a server rollback reaches an older anchor', async () => {
  const result = await collectCodexTurnTail({
    cachedTurns: [turn('one'), turn('kept'), turn('rolled-back')],
    initialPage: page([turn('kept', 'authoritative')]),
  })

  assert.deepEqual(result.turns.map(({ id }) => id), ['one', 'kept'])
  assert.equal(answer(result.turns.at(-1)), 'authoritative')
})

test('requires a full fallback when bounded pages do not intersect the cache', async () => {
  const result = await collectCodexTurnTail({
    cachedTurns: [turn('cached')],
    initialPage: page([turn('newest')], 'repeat'),
    fetchPage: async () => page([turn('older')], 'repeat'),
  })

  assert.equal(result.matched, false)
  assert.deepEqual(result.turns.map(({ id }) => id), ['cached'])
  assert.equal(result.pageCount, 2)
})

test('recognizes pagination compatibility failures without hiding ordinary errors', () => {
  assert.equal(isHistoryPaginationCompatibilityError(new Error('thread/turns/list is not supported yet')), true)
  assert.equal(isHistoryPaginationCompatibilityError(new Error('Invalid params: unknown field initialTurnsPage')), true)
  assert.equal(isHistoryPaginationCompatibilityError(new Error('thread abc not found')), false)
  assert.equal(isHistoryPaginationCompatibilityError(new Error('thread/turns/list request timed out')), false)
  assert.equal(isHistoryPaginationCompatibilityError(new Error('connection closed')), false)
})

function turn(id, text = id) {
  return { id, status: 'completed', items: [{ id: `answer-${id}`, type: 'agentMessage', text }] }
}

function answer(value) {
  return value.items.at(-1).text
}

function page(data, nextCursor = null, backwardsCursor = null) {
  return { data, nextCursor, backwardsCursor }
}
