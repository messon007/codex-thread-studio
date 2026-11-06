import assert from 'node:assert/strict'
import test from 'node:test'
import { createReaderCommentHost, readerCommentMark } from './reader-comment-host.mjs'
import { normalizeReaderReference } from './document-review.mjs'
const revision='a'.repeat(64),file={root:'/work',path:'/work/book.md'}
const owner={ready:true,documentId:'new-pane',revision,capabilities:{commentMarks:{version:1,records:512}}}
const draft={id:'comment-1',excerpt:'word',source:{provider:'document',anchor:{...file,filePath:file.path,readerReference:normalizeReaderReference({documentId:'old-pane',revision,precision:'approximate',location:{kind:'line',line:1},quote:' word ',textRange:{version:1,segments:[{scope:{kind:'projection',page:0},startUtf16:2,endUtf16:8,textHash:'b'.repeat(64)}]}})}}}
const tick=()=>new Promise(resolve=>setImmediate(resolve))

test('only owned exact comments paint after reopen, never favorites, stale records or guessed legacy lines',()=>{
 const mark=readerCommentMark(draft,file,owner)
 assert.equal(mark.id,draft.id);assert.equal(mark.quote,' word ')
 assert.deepEqual(mark.anchor.textRange,draft.source.anchor.readerReference.textRange)
 assert.equal(readerCommentMark(draft,{...file,path:'/work/other.md'},owner),null)
 assert.equal(readerCommentMark(draft,file,{...owner,revision:'c'.repeat(64)}),null)
 assert.equal(readerCommentMark({...draft,source:{provider:'favorite',anchor:draft.source.anchor}},file,owner),null)
 const old=structuredClone(draft);delete old.source.anchor.readerReference.textRange
 assert.equal(readerCommentMark(old,file,owner),null)
 assert.equal(old.excerpt,'word','coarse reference is retained, never relocated')
 old.source.anchor.readerReference.location={kind:'source',startByte:0,endByte:4}
 assert.equal(readerCommentMark(old,file,owner),null,'declared approximate source positions are never upgraded to exact marks')
 old.source.anchor.readerReference.precision='exact'
 assert.ok(readerCommentMark(old,file,owner))
})

test('replacement is serialized and bounded to the latest owned set; events cannot open stale or foreign comments',async()=>{
 const calls=[],errors=[],opened=[];let finish
 const transport={request(method,params,{signal}){calls.push({method,params,signal});return calls.length===1?new Promise(resolve=>finish=resolve):Promise.resolve({})}}
 const bridge=createReaderCommentHost({onError:error=>errors.push(error),open:id=>opened.push(id)})
 bridge.update(owner,transport,[draft],file);await tick()
 bridge.refresh([],file)
 assert.equal(calls[0].signal.aborted,true)
 bridge.open({...owner,id:draft.id});assert.equal(opened.length,0)
 finish();await tick()
 assert.equal(calls.length,2);assert.deepEqual(calls[1].params.marks,[])
 bridge.refresh([draft],file);await tick()
 bridge.open({...owner,id:draft.id});bridge.open({...owner,id:'foreign'});bridge.open({...owner,revision:'c'.repeat(64),id:draft.id})
 assert.deepEqual(opened,[draft.id])
 const count=calls.length;bridge.update({...owner,view:'source'},transport,[draft],file);await tick();assert.equal(calls.length,count,'view events do not redundantly replace marks')
 bridge.dispose();bridge.open({...owner,id:draft.id});assert.deepEqual(opened,[draft.id]);assert.deepEqual(errors,[])
})
