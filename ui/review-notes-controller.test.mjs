import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import { createReviewNotesState, deactivateChatCommentMarker } from './review-notes-controller.mjs'

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

test('single comment input matches Composer Enter behavior', () => {
  const source = readFileSync(new URL('./review-notes-controller.mjs', import.meta.url), 'utf8')
  assert.match(source, /#annotation-comment'[)]\?\.addEventListener\('keydown', handleAnnotationCommentKeydown\)/u)
  assert.match(source, /function handleAnnotationCommentKeydown\(event\) \{[\s\S]*event\.isComposing[\s\S]*event\.key !== 'Enter'[\s\S]*event\.shiftKey[\s\S]*event\.preventDefault\(\)[\s\S]*requestSubmit\(\$\('#save-annotation'\)\)/u)
})

test('clearing chat comments keeps marker spans in the text layout', () => {
  const classes = new Set(['chat-comment-anchor'])
  const removedAttributes = []
  const marker = {
    classList: {
      remove: (name) => classes.delete(name),
    },
    removeAttribute: (name) => removedAttributes.push(name),
  }

  deactivateChatCommentMarker(marker)

  assert.deepEqual([...classes], [])
  assert.deepEqual(removedAttributes, ['data-comment-ids', 'role', 'tabindex', 'title', 'aria-label'])
  const source = readFileSync(new URL('./review-notes-controller.mjs', import.meta.url), 'utf8')
  assert.match(source, /if \(!byItem\.size\) \{\s*deactivateChatCommentMarkers\(transcript\)\s*return/u)
  assert.match(source, /deactivatedChatCommentMarkers\.add\(marker\)/u)
})

test('closing the Comment rail preserves the live transcript layout anchor', () => {
  const source = readFileSync(new URL('./review-notes-controller.mjs', import.meta.url), 'utf8')
  const start = source.indexOf('function closeAnnotationRail(')
  const end = source.indexOf('\nfunction renderAnnotationRail', start)
  const close = source.slice(start, end)
  assert.match(close, /if \(preserveTranscriptLayout\) preserveTranscriptLayout\(close\)/u)
})

test('Review Notes restores the injected Session Map view without a global dependency', () => {
  const source = readFileSync(new URL('./review-notes-controller.mjs', import.meta.url), 'utf8')

  assert.match(source, /renderSessionMap\(\)/u)
  assert.doesNotMatch(source, /sessionMap\.render\(\)/u)
})

test('Send and Clear submits the composer before clearing only the selected session comments', () => {
  const source = readFileSync(new URL('./review-notes-controller.mjs', import.meta.url), 'utf8')
  const index = readFileSync(new URL('./index.html', import.meta.url), 'utf8')
  const styles = readFileSync(new URL('./styles.css', import.meta.url), 'utf8')
  const start = source.indexOf('function sendAndClearAnnotations(')
  const end = source.indexOf('\nasync function favoriteRequest', start)
  const sendAndClear = source.slice(start, end)

  assert.match(index, /id="send-clear-annotations"[^>]*>Send and Clear<\/button>/u)
  assert.match(index, /id="composer-review-send-clear"[^>]*>Send and Clear<\/button>/u)
  assert.match(source, /#send-clear-annotations'[)]\?\.addEventListener\('click', sendAndClearAnnotations\)/u)
  assert.match(source, /#composer-review-send-clear'[)]\?\.addEventListener\('click', sendAndClearAnnotations\)/u)
  assert.match(styles, /\.composer-review-context \{[^}]*grid-template-columns: 24px minmax\(0, 1fr\) auto auto auto;/u)
  assert.ok(sendAndClear.indexOf('setComposerValue(') < sendAndClear.indexOf('form.requestSubmit(sendButton)'))
  assert.ok(sendAndClear.indexOf('form.requestSubmit(sendButton)') < sendAndClear.indexOf('delete state.annotationDrafts[key]'))
  assert.match(sendAndClear, /delete state\.annotationDrafts\[key\][\s\S]*delete state\.annotationAdditional\[key\][\s\S]*persistAnnotationState\(key\)[\s\S]*renderAnnotationRail\(\)/u)
  assert.match(sendAndClear, /sendButton\.disabled \|\| form\.classList\.contains\('hidden'\) \|\| form\.classList\.contains\('shell-mode'\)/u)
  assert.doesNotMatch(sendAndClear, /confirm\(/u)
})
