import assert from 'node:assert/strict'
import test from 'node:test'

import { CommentSourceRegistry, createCommentDraft } from './comment-core.mjs'
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


test('EPUB comments preserve typed public references and the e-book AI instruction',async()=>{
 const readerReference={documentId:'public-book',revision:'a'.repeat(64),precision:'exact',location:{kind:'epub',cfi:'epubcfi(/6/2!/4/2,/1:0,/1:5)',href:'OPS/one.xhtml'},quoteTruncated:false}
 const source=epubCommentSource({root:'/books',filePath:'example.epub',readerReference})
 assert.deepEqual(source.anchor.readerReference,readerReference)
 const provider=createEpubCommentProvider()
 const registry=new CommentSourceRegistry().register(provider)
 const draft=createCommentDraft({source,excerpt:'quote'},{registry})
 const roundTrip=createCommentDraft(JSON.parse(JSON.stringify(draft)),{registry})
 assert.deepEqual(roundTrip.source.anchor.readerReference,readerReference)
 let restored
 await provider.reopen({source,excerpt:'quote'},{openEpubSource:anchor=>{restored=anchor}})
 assert.deepEqual(restored.readerReference,readerReference)
 assert.match(provider.promptInstruction({source},{translate:value=>value}),/do not assume the e-book itself can be edited/)
 assert.throws(()=>epubCommentSource({readerReference:{...readerReference,revision:'stale'}}),/Invalid public reader reference/)
})
