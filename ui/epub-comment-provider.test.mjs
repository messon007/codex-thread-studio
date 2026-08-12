import assert from 'node:assert/strict'
import test from 'node:test'

import { createEpubCommentProvider, epubCommentSource } from './epub-comment-provider.mjs'

test('EPUB comments keep CFI source semantics outside the generic Comment core', async () => {
  const source = epubCommentSource({
    root: '/books', filePath: 'library/example.epub', bookHash: 'fnv1a64:0123456789abcdef',
    cfiRange: 'epubcfi(/6/2!/4/2,/1:0,/1:5)', chapterLabel: 'Chapter 1',
  })
  const provider = createEpubCommentProvider()
  const draft = { excerpt: 'quoted text', source }
  assert.equal(provider.describe(draft, {}), 'example.epub · Chapter 1')
  assert.match(provider.promptAnchor(draft), /EPUB CFI epubcfi/)
  let reopened = null
  await provider.reopen(draft, { openEpubSource: (anchor) => { reopened = anchor } })
  assert.equal(reopened.cfiRange, source.anchor.cfiRange)
})
