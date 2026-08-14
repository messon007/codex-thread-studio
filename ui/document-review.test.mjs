import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import {
  artifactSearchAvailable,
  createFileRangeTarget,
  findTextMatchRanges,
  fileAnnotationAnchor,
  fileDisplayName,
  formatLineAnchor,
  isHtmlFile,
  lineRangeForOffsets,
  lineRangeForTarget,
  lineNumberAt,
  locateQuote,
  normalizeAnnotationTarget,
  snapshotAnnotationSelection,
  STATIC_HTML_FORBIDDEN_ATTRIBUTES,
  STATIC_HTML_FORBIDDEN_TAGS,
} from './document-review.mjs'

const documentReviewHtml = readFileSync(new URL('./index.html', import.meta.url), 'utf8')

test('locates a selected file range and preserves nearby anchors', () => {
  const file = { path: '/work/docs/guide.md', root: '/work', hash: 'abc', content: 'one\ntwo\nthree' }
  const target = createFileRangeTarget(file, 'two')
  assert.equal(target.startOffset, 4)
  assert.equal(target.endOffset, 7)
  assert.deepEqual({ startLine: target.startLine, endLine: target.endLine }, { startLine: 2, endLine: 2 })
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

test('normalizes invalid zero-length file anchors as chat fallback fields', () => {
  assert.deepEqual(normalizeAnnotationTarget({ target: { kind: 'fileRange', filePath: '/work/docs/guide.md', baseHash: 'abc', startOffset: 0, endOffset: 0 } }), {
    kind: 'fileRange',
    filePath: '/work/docs/guide.md',
    root: '',
    baseHash: 'abc',
    startOffset: null,
    endOffset: null,
    startLine: null,
    endLine: null,
    prefix: '',
    suffix: '',
  })
})

test('does not coerce absent or partial file offsets to zero', () => {
  const target = {
    kind: 'fileRange',
    filePath: '/work/docs/guide.md',
    startOffset: null,
    endOffset: null,
  }
  assert.equal(normalizeAnnotationTarget({ target }).startOffset, null)
  assert.equal(normalizeAnnotationTarget({ target: { ...target, endOffset: 5 } }).startOffset, null)
  assert.equal(normalizeAnnotationTarget({ target: { ...target, startOffset: 5 } }).endOffset, null)
  assert.equal(normalizeAnnotationTarget({ target: { ...target, startOffset: '', endOffset: 5 } }).startOffset, null)
})

test('maps exclusive source offsets to an inclusive line range', () => {
  assert.deepEqual(lineRangeForOffsets('one\ntwo\nthree', 4, 13), { startLine: 2, endLine: 3 })
  assert.deepEqual(lineRangeForOffsets('one\ntwo\nthree', 4, 8), { startLine: 2, endLine: 2 })
  assert.deepEqual(lineRangeForOffsets('one\ntwo', null, 4), { startLine: null, endLine: null })
})

test('formats stable line anchors for annotation prompts', () => {
  assert.equal(formatLineAnchor(2, 2), 'line 2')
  assert.equal(formatLineAnchor(2, 4), 'lines 2-4')
  assert.equal(formatLineAnchor(null, 4), '')
  const target = {
    kind: 'fileRange', filePath: '/work/guide.md', baseHash: 'abc',
    startOffset: 4, endOffset: 13, startLine: 2, endLine: 3,
  }
  assert.deepEqual(lineRangeForTarget(target), { startLine: 2, endLine: 3 })
  assert.equal(fileAnnotationAnchor(target), '/work/guide.md / lines 2-3 / base abc')
  assert.equal(
    fileAnnotationAnchor({ ...target, startLine: null, endLine: null }, 'one\ntwo\nthree'),
    '/work/guide.md / lines 2-3 / base abc',
  )
  assert.equal(
    fileAnnotationAnchor({ ...target, startLine: null, endLine: null }),
    '/work/guide.md / offset 4-13 / base abc',
  )
})

test('keeps document search available in preview and source modes', () => {
  const file = { content: '# Document', loading: false, error: '' }
  assert.equal(artifactSearchAvailable(file, 'preview'), true)
  assert.equal(artifactSearchAvailable(file, 'source'), true)
  assert.equal(artifactSearchAvailable({ ...file, loading: true }, 'source'), false)

  const searchPosition = documentReviewHtml.indexOf('id="artifact-search-toggle"')
  const viewPosition = documentReviewHtml.indexOf('id="artifact-view-switch"')
  assert.ok(searchPosition >= 0 && searchPosition < viewPosition)
  assert.ok(documentReviewHtml.includes('id="artifact-search-panel"'))
})

test('document outline stays inside the document shell and uses compact header actions', () => {
  const shellPosition = documentReviewHtml.indexOf('id="artifact-reader-shell"')
  const outlinePosition = documentReviewHtml.indexOf('id="artifact-outline"')
  const contentPosition = documentReviewHtml.indexOf('id="artifact-content"')
  assert.ok(documentReviewHtml.includes('id="artifact-outline-toggle" class="icon-button artifact-action-icon hidden"'))
  assert.ok(shellPosition >= 0 && shellPosition < outlinePosition && outlinePosition < contentPosition)
})

test('static HTML preview blocks executable and externally loaded content', () => {
  for (const tag of ['script', 'iframe', 'object', 'form', 'style']) assert.ok(STATIC_HTML_FORBIDDEN_TAGS.includes(tag))
  for (const attribute of ['src', 'href', 'style', 'srcdoc']) assert.ok(STATIC_HTML_FORBIDDEN_ATTRIBUTES.includes(attribute))
})

test('keeps an unresolved rendered selection unresolved after normalization', () => {
  const file = { path: '/work/docs/guide.md', root: '/work', hash: 'abc', content: '**formatted text**' }
  const target = createFileRangeTarget(file, 'formatted text plus another paragraph')
  assert.equal(target.startOffset, null)
  assert.equal(normalizeAnnotationTarget({ target }).startOffset, null)
  assert.equal(normalizeAnnotationTarget({ target }).endOffset, null)
})

test('finds every non-overlapping match in one text node', () => {
  assert.deepEqual(findTextMatchRanges('Test and test and TEST', 'test'), [
    { start: 0, end: 4 },
    { start: 9, end: 13 },
    { start: 18, end: 22 },
  ])
})

test('createFileRangeTarget rejects zero-length quotes', () => {
  const file = { path: '/work/docs/guide.md', root: '/work', hash: 'abc', content: 'one\ntwo\nthree' }
  assert.deepEqual(createFileRangeTarget(file, '', 0), {
    kind: 'fileRange',
    filePath: '/work/docs/guide.md',
    root: '/work',
    baseHash: 'abc',
    startOffset: null,
    endOffset: null,
    startLine: null,
    endLine: null,
    prefix: '',
    suffix: '',
    ambiguous: false,
  })
})

test('extracts a portable file display name', () => {
  assert.equal(fileDisplayName('C:\\work\\README.md'), 'README.md')
  assert.equal(isHtmlFile('/work/index.html'), true)
  assert.equal(isHtmlFile('/work/page.htm'), true)
  assert.equal(isHtmlFile('/work/page.md'), false)
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
