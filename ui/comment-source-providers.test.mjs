import assert from 'node:assert/strict'
import test from 'node:test'
import { CommentSourceRegistry, createCommentDraft } from './comment-core.mjs'
import {
  createChatCommentProvider,
  createDocumentCommentProvider,
  legacyCommentSource,
} from './comment-source-providers.mjs'

const registry = new CommentSourceRegistry()
  .register(createChatCommentProvider())
  .register(createDocumentCommentProvider())

test('legacy chat targets stay internal and do not enter the user message', () => {
  const source = legacyCommentSource({ itemId: 'item-1', turnId: 'turn-1' })
  const draft = createCommentDraft({ excerpt: 'Answer', source }, { registry, idFactory: () => 'id', now: () => 'now' })
  assert.equal(draft.source.provider, 'chat')
  assert.equal(registry.promptAnchor(draft), '')
  assert.equal(draft.source.anchor.turnId, 'turn-1')
  assert.equal(draft.source.anchor.itemId, 'item-1')
})

test('chat comments preserve bounded rendered-text offsets', () => {
  const source = legacyCommentSource({ itemId: 'item-1', turnId: 'turn-1' })
  const positioned = createCommentDraft({
    excerpt: 'Answer',
    source: { ...source, anchor: { ...source.anchor, startOffset: 12, endOffset: 18 } },
  }, { registry, idFactory: () => 'id', now: () => 'now' })
  assert.deepEqual(positioned.source.anchor, {
    turnId: 'turn-1',
    itemId: 'item-1',
    startOffset: 12,
    endOffset: 18,
  })
})

test('legacy file targets migrate through the document provider', () => {
  const source = legacyCommentSource({
    target: {
      kind: 'fileRange',
      filePath: '/work/guide.md',
      root: '/work',
      baseHash: 'fnv1a64:abc',
      startOffset: 4,
      endOffset: 8,
      startLine: 2,
      endLine: 2,
    },
  })
  const draft = createCommentDraft({ excerpt: 'text', source }, { registry, idFactory: () => 'id', now: () => 'now' })
  assert.equal(draft.source.provider, 'document')
  assert.equal(registry.promptAnchor(draft), '/work/guide.md / line 2 / base fnv1a64:abc')
})

test('saved SQL AI drafts retain verified provenance and reopen history without navigating an expired result',async()=>{
 const {documentCommentSource}=await import('./comment-source-providers.mjs')
 const {normalizeReaderReference}=await import('./document-review.mjs')
 const reference={documentId:'original-doc',revision:'a'.repeat(64),precision:'exact',location:{kind:'queryCell',queryId:'old-query',row:0,column:1},querySnapshot:{revision:'a'.repeat(64),quote:'NULL',anchor:{quoteTruncated:false,query:{table:0,sql:'SELECT COUNT(*), NULL AS empty FROM t',row:0,column:1,columnName:'empty',totalRows:1,sourceRows:123,valueIsNull:true}}}}
 const draft=createCommentDraft({excerpt:'NULL',source:documentCommentSource({kind:'fileRange',root:'/project',filePath:'/project/data.csv',readerReference:reference})},{registry,idFactory:()=> 'query-draft',now:()=> 'now'})
 const restored=createCommentDraft(JSON.parse(JSON.stringify(draft)),{registry})
 const prompt=registry.promptAnchor(restored)
 assert.match(prompt,/SQL result snapshot; not a source cell/)
 assert.match(prompt,/SELECT COUNT/);assert.match(prompt,/valueIsNull/);assert.match(prompt,/sourceRows/)
 let opened=null,navigations=0
 await registry.reopen(restored,{openQuerySnapshot:snapshot=>{opened=snapshot},openDocument:()=>{navigations++}})
 assert.equal(opened.quote,'NULL');assert.equal(opened.anchor.query.sourceRows,123)
 assert.equal(navigations,0)
 const omitted=structuredClone(reference);delete omitted.querySnapshot.anchor.quoteTruncated
 assert.equal(normalizeReaderReference(omitted).querySnapshot.anchor.quoteTruncated,false)
 assert.throws(()=>normalizeReaderReference({...reference,querySnapshot:{...reference.querySnapshot,revision:'b'.repeat(64)}}),/snapshot/)
 assert.throws(()=>normalizeReaderReference({...reference,querySnapshot:{...reference.querySnapshot,quote:'x'.repeat(32769)}}),/snapshot/)
 assert.throws(()=>normalizeReaderReference({...reference,querySnapshot:{...reference.querySnapshot,anchor:{...reference.querySnapshot.anchor,query:{...reference.querySnapshot.anchor.query,sql:'x'.repeat(16385)}}}}),/snapshot/)
})
