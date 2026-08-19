import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import { createReviewNotesState } from './review-notes-controller.mjs'

test('Review Notes state factories isolate mutable collections', () => {
  const first = createReviewNotesState()
  const second = createReviewNotesState()

  first.annotationDrafts.thread = [{ id: 'comment-1' }]
  first.favorites.push({ id: 'favorite-1' })
  first.favoriteIndex.push({ key: 'codex:thread:turn:item' })

  assert.deepEqual(second.annotationDrafts, {})
  assert.deepEqual(second.favorites, [])
  assert.deepEqual(second.favoriteIndex, [])
  assert.equal(second.pendingSelection, null)
})

test('Review Notes restores the injected Session Map view without a global dependency', () => {
  const source = readFileSync(new URL('./review-notes-controller.mjs', import.meta.url), 'utf8')

  assert.match(source, /renderSessionMap\(\)/u)
  assert.doesNotMatch(source, /sessionMap\.render\(\)/u)
})
