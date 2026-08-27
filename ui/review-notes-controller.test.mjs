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
  assert.equal(second.editingAnnotationId, null)
})

test('chat comment markers reuse the existing comment dialog for editing', () => {
  const source = readFileSync(new URL('./review-notes-controller.mjs', import.meta.url), 'utf8')
  const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8')
  const index = readFileSync(new URL('./index.html', import.meta.url), 'utf8')

  assert.match(source, /locateCommentIntervals/u)
  assert.match(source, /renderChatCommentMarkers/u)
  assert.match(source, /openAnnotationEditor/u)
  assert.match(source, /editing \? 'Save changes' : 'Add to draft'/u)
  assert.match(styles, /\.chat-comment-anchor \{[^}]*text-decoration-style: dashed;/u)
  assert.match(index, /id="annotation-dialog-title"/u)
  assert.match(index, /id="save-annotation"/u)
})

test('Review Notes restores the injected Session Map view without a global dependency', () => {
  const source = readFileSync(new URL('./review-notes-controller.mjs', import.meta.url), 'utf8')

  assert.match(source, /renderSessionMap\(\)/u)
  assert.doesNotMatch(source, /sessionMap\.render\(\)/u)
})
