import assert from 'node:assert/strict'
import test from 'node:test'
import { CommentSourceRegistry, createCommentDraft } from './comment-core.mjs'
import { createDocumentCommentProvider, documentCommentSource } from './comment-source-providers.mjs'
import { normalizeReaderReference } from './document-review.mjs'
const reference={documentId:'doc',revision:'a'.repeat(64),precision:'exact',location:{kind:'source',startByte:3,endByte:13}}
const registry=new CommentSourceRegistry().register(createDocumentCommentProvider())
for(const location of [reference.location,{kind:'cell',table:0,row:9,column:2},{kind:'projection',page:3,startUtf16:2,endUtf16:6},{kind:'pdf',pages:[{page:2,rects:[{x:.1,y:.2,width:.3,height:.4}]}]},{kind:'epub',cfi:'epubcfi(/6/2!/4/2:3)',href:'chapter.xhtml'}]){
 test(`Studio comment preserves public ${location.kind} coordinates through persistence`,async()=>{
  const input={...reference,location}
  const source=documentCommentSource({kind:'fileRange',root:'/work',filePath:'/work/book.md',readerReference:input})
  const draft=createCommentDraft({excerpt:'\u4e2d\u6587\u{1f600}',source},{registry})
  const reopened=createCommentDraft(JSON.parse(JSON.stringify(draft)),{registry})
  assert.deepEqual(reopened.source.anchor.readerReference.location,location)
  assert.equal(reopened.source.anchor.startOffset,null)
  assert.equal(reopened.source.anchor.endOffset,null)
  assert.equal(reopened.source.anchor.startLine,null)
  assert.match(registry.promptAnchor(reopened),/precision exact/)
  assert.ok(registry.promptAnchor(reopened).includes(JSON.stringify(location)))
  let target
  await registry.reopen(reopened,{openDocument:value=>{target=value}})
  assert.deepEqual(target.readerReference.location,location)
  input.location={kind:'line',line:99}
  assert.deepEqual(reopened.source.anchor.readerReference.location,location)
 })
}
test('typed references reject malformed scope, precision, and oversized locations',()=>{
 for(const patch of [{documentId:''},{documentId:'x'.repeat(257)},{revision:'invalid'},{precision:'guess'},{location:{kind:'file',path:'/etc/passwd'}},{location:{kind:'epub',cfi:'x'.repeat(32769)}}])assert.throws(()=>normalizeReaderReference({...reference,...patch}),/Invalid public reader reference/)
})
test('transient SQL coordinates are labelled rather than claimed as source bytes',()=>{
 const source=documentCommentSource({kind:'fileRange',filePath:'/work/data.csv',readerReference:{...reference,location:{kind:'queryCell',queryId:'live',row:1,column:2}}})
 const draft=createCommentDraft({excerpt:'value',source},{registry})
 assert.match(registry.promptAnchor(draft),/transient SQL result; not a source location/)
 assert.equal(draft.source.anchor.startOffset,null)
})

test('rendered selectors survive Studio draft save/reload normalization without changing their coordinate units', () => {
 const textRange={version:1,segments:[{scope:{kind:'projection',page:2},startUtf16:3,endUtf16:9,textHash:'b'.repeat(64)}]}
 const source=documentCommentSource({kind:'fileRange',root:'/work',filePath:'/work/book.md',readerReference:{...reference,quote:'  中文😀  ',textRange}})
 const saved=JSON.parse(JSON.stringify(source))
 const restored=normalizeReaderReference(saved.anchor.readerReference)
 assert.deepEqual(restored.textRange,textRange)
 assert.equal(restored.quote,'  中文😀  ')
 textRange.segments[0].startUtf16=0
 assert.equal(restored.textRange.segments[0].startUtf16,3)
})

test('rendered selector validation rejects malformed, duplicate, unbounded and foreign scopes', () => {
 const segment={scope:{kind:'queryCell',queryId:'query_1',row:0,column:1},startUtf16:0,endUtf16:3,textHash:'b'.repeat(64)}
 const good={version:1,segments:[segment]}
 assert.deepEqual(normalizeReaderReference({...reference,textRange:good}).textRange,good)
 for(const textRange of [{version:2,segments:[segment]},{version:1,segments:[]},{version:1,segments:Array(33).fill(segment)},
  {version:1,segments:[segment,segment]},...[
   {startUtf16:-1},{endUtf16:262145},{endUtf16:0},{textHash:'not-hash'},
   {scope:{kind:'file',path:'/etc/passwd'}},{scope:{kind:'queryCell',queryId:'../../escape',row:0,column:1}},
   {scope:{kind:'cell',table:0,row:1,column:0,path:'/etc/passwd'}},
  ].map(patch=>({version:1,segments:[{...segment,...patch}]}))]) {
  assert.throws(()=>normalizeReaderReference({...reference,textRange}),/Invalid public reader text range/)
 }
})
