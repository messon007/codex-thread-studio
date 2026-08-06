import assert from 'node:assert/strict'
import test from 'node:test'
import {
  createFileRangeTarget,
  fileDisplayName,
  lineNumberAt,
  locateQuote,
  normalizeAnnotationTarget,
  snapshotAnnotationSelection,
} from './document-review.mjs'

test('locates a selected file range and preserves nearby anchors', () => {
  const file = { path: '/work/docs/guide.md', root: '/work', hash: 'abc', content: 'one\ntwo\nthree' }
  const target = createFileRangeTarget(file, 'two')
  assert.equal(target.startOffset, 4)
  assert.equal(target.endOffset, 7)
  assert.equal(target.filePath, '/work/docs/guide.md')
  assert.equal(lineNumberAt(file.content, target.startOffset), 2)
})

test('marks repeated quotes as ambiguous', () => {
  assert.deepEqual(locateQuote('same and same', 'same'), { startOffset: 0, endOffset: 4, ambiguous: true })
})

test('normalizes legacy chat annotations without losing anchors', () => {
  assert.deepEqual(normalizeAnnotationTarget({ itemId: 'i1', turnId: 't1' }), {
    kind: 'chatRange', itemId: 'i1', turnId: 't1',
  })
})

test('extracts a portable file display name', () => {
  assert.equal(fileDisplayName('C:\\work\\README.md'), 'README.md')
})

test('snapshots a document annotation independently from transient selection state', () => {
  const selection = {
    quote: 'selected paragraph',
    target: {
      kind: 'fileRange', filePath: '/work/README.md', root: '/work', baseHash: 'abc',
      startOffset: 12, endOffset: 30, prefix: 'before', suffix: 'after',
    },
  }
  const snapshot = snapshotAnnotationSelection(selection)
  selection.quote = ''
  selection.target.filePath = '/changed.md'

  assert.equal(snapshot.quote, 'selected paragraph')
  assert.equal(snapshot.target.filePath, '/work/README.md')
  assert.equal(snapshot.target.startOffset, 12)
})
