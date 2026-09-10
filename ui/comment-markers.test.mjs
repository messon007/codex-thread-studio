import assert from 'node:assert/strict'
import test from 'node:test'

import { locateCommentIntervals, locateDocumentCommentIntervals } from './comment-markers.mjs'
import { createDocumentCommentProvider } from './comment-source-providers.mjs'

test('document markers persist preview offsets and disambiguate repeated rendered text', () => {
  const anchor = createDocumentCommentProvider().normalizeAnchor({filePath:'/book.md',root:'/project',baseHash:'v1',previewHash:'v1',previewStartOffset:7,previewEndOffset:13})
  const doc = { id:'doc', excerpt:'repeat',source:{provider:'document',anchor} }
  assert.deepEqual(locateDocumentCommentIntervals('repeat repeat',[doc],{path:'/book.md',root:'/project',hash:'v1'}),[{id:'doc',start:7,end:13}])
  assert.deepEqual(locateDocumentCommentIntervals('repeat repeat',[doc],{path:'/other.md',root:'/project',hash:'v1'}),[])
  assert.deepEqual(locateDocumentCommentIntervals('repeat repeat',[doc],{path:'/book.md',root:'/other',hash:'v1'}),[])
  assert.deepEqual(locateDocumentCommentIntervals('repeat repeat',[doc],{path:'/book.md',root:'/project',hash:'v2'}),[])
  assert.deepEqual(locateDocumentCommentIntervals('repeat repeat',[doc],{path:'/book.md',root:'/project',hash:'v1',dirty:true}),[])
  assert.deepEqual(locateDocumentCommentIntervals('moved repeat',[doc],{path:'/book.md',root:'/project',hash:'v2'}),[{id:'doc',start:6,end:12}])
})

test('raw Markdown offsets are never treated as rendered preview offsets', () => {
  const doc = {id:'legacy',excerpt:'same',source:{provider:'document',anchor:{filePath:'/book.md',startOffset:0,endOffset:4}}}
  assert.deepEqual(locateDocumentCommentIntervals('same same',[doc],{path:'/book.md'}),[])
  assert.deepEqual(locateDocumentCommentIntervals('unique same',[doc],{path:'/book.md'}),[{id:'legacy',start:7,end:11}])
})

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
