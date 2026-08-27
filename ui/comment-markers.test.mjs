import assert from 'node:assert/strict'
import test from 'node:test'

import { locateCommentIntervals } from './comment-markers.mjs'

function draft(id, excerpt, anchor = {}) {
  return { id, excerpt, source: { provider: 'chat', anchor } }
}

test('stored offsets distinguish repeated chat excerpts', () => {
  const text = 'same text, then same text'
  assert.deepEqual(locateCommentIntervals(text, [draft('second', 'same text', { startOffset: 16, endOffset: 25 })]), [
    { id: 'second', start: 16, end: 25 },
  ])
})

test('legacy chat comments are marked only when their excerpt is unique', () => {
  assert.deepEqual(locateCommentIntervals('before unique after', [draft('unique', 'unique')]), [
    { id: 'unique', start: 7, end: 13 },
  ])
  assert.deepEqual(locateCommentIntervals('repeat repeat', [draft('ambiguous', 'repeat')]), [])
})

test('stale offsets safely fall back to a unique excerpt', () => {
  assert.deepEqual(locateCommentIntervals('moved excerpt', [draft('moved', 'excerpt', { startOffset: 0, endOffset: 7 })]), [
    { id: 'moved', start: 6, end: 13 },
  ])
})
