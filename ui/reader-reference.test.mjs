import assert from 'node:assert/strict'
import test from 'node:test'
import { CommentSourceRegistry, createCommentDraft } from './comment-core.mjs'
import { createDocumentCommentProvider, documentCommentSource, sameCommentSelection } from './comment-source-providers.mjs'
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

test('durable resource identity survives persistence and rejects malformed identities',()=>{
 const reference={documentId:'old-session',resourceId:'stable-resource',revision:'a'.repeat(64),precision:'exact',location:{kind:'source',startByte:0,endByte:5}}
 assert.equal(normalizeReaderReference(reference).resourceId,'stable-resource')
 assert.equal(normalizeReaderReference({...reference,resourceId:undefined}).resourceId,undefined)
 for(const resourceId of ['',3,'x'.repeat(257),'中'.repeat(100)])assert.throws(()=>normalizeReaderReference({...reference,resourceId}),/Invalid public reader reference/)
})

test('same selection reuses identity across reader sessions, never by repeated quote text', () => {
 const make=ref=>({excerpt:'same text',source:documentCommentSource({kind:'fileRange',root:'/work',filePath:'/work/a.md',readerReference:ref})})
 const first=make({...reference,resourceId:'file-a'})
 const reopened=make({...reference,resourceId:'file-a',documentId:'another-session'})
 assert.equal(sameCommentSelection(first,reopened),true)
 for(const patch of [{revision:'b'.repeat(64)},{resourceId:'file-b'},{location:{...reference.location,startByte:20,endByte:30}},{precision:'draft'},{precision:'approximate'}]) {
  assert.equal(sameCommentSelection(first,make({...reference,resourceId:'file-a',...patch})),false)
 }
 const elsewhere=structuredClone(reopened);elsewhere.source.anchor.filePath='/work/b.md';assert.equal(sameCommentSelection(first,elsewhere),false)
 const otherRoot=structuredClone(reopened);otherRoot.source.anchor.root='/other';assert.equal(sameCommentSelection(first,otherRoot),false)
})
test('painted text ranges compare scoped positions; coarse line or query references do not merge', () => {
 const segment={scope:{kind:'projection',page:2},startUtf16:10,endUtf16:16,textHash:'b'.repeat(64)}
 const ref={...reference,precision:'approximate',location:{kind:'line',line:3},textRange:{version:1,segments:[segment]}}
 const make=ref=>({source:documentCommentSource({kind:'fileRange',root:'/work',filePath:'/work/a.md',readerReference:ref})})
 assert.equal(sameCommentSelection(make(ref),make({...ref,documentId:'new'})),true)
 const snapshot=make(ref);snapshot.source.anchor.readerReference.querySnapshot={version:1};assert.equal(sameCommentSelection(snapshot,snapshot),false)
 const changed=structuredClone(ref);changed.textRange.segments[0].scope.page=3
 assert.equal(sameCommentSelection(make(ref),make(changed)),false)
 assert.equal(sameCommentSelection(make({...ref,textRange:undefined}),make({...ref,textRange:undefined})),false)
})
test('chat selections and legacy file offsets require complete owned position identity', () => {
 const chat={source:{provider:'chat',anchor:{turnId:'turn',itemId:'item',startOffset:2,endOffset:6}}}
 assert.equal(sameCommentSelection(chat,structuredClone(chat)),true)
 const other=structuredClone(chat);other.source.anchor.startOffset=3;assert.equal(sameCommentSelection(chat,other),false)
 const missing=structuredClone(chat);missing.source.anchor.startOffset=null;assert.equal(sameCommentSelection(missing,missing),false)
 const file={source:{provider:'document',anchor:{root:'/work',filePath:'/work/a',baseHash:'hash',startOffset:2,endOffset:6}}}
 assert.equal(sameCommentSelection(file,structuredClone(file)),true)
 delete file.source.anchor.baseHash;assert.equal(sameCommentSelection(file,file),false)
})
