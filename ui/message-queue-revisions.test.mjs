import test from 'node:test'
import assert from 'node:assert/strict'
import {createVersionedMessageQueueWriter} from './session-state-persistence.mjs'
const response=(status,revision)=>({ok:status===204,status,headers:new Headers({'x-studio-queue-revision':String(revision)})})
test('Queue writes snapshot input but use the latest acknowledged revision in FIFO order',async()=>{
 const state={messageQueues:{key:[{id:'one',text:'original',input:[],createdAt:1}]},messageQueueRevisions:{key:3}},bodies=[]
 const writer=createVersionedMessageQueueWriter(state,async(_path,options)=>{bodies.push(JSON.parse(options.body));return response(204,3+bodies.length)},()=>true,()=>assert.fail('unexpected conflict'))
 const first=writer.write('key');state.messageQueues.key[0].text='edited';const second=writer.write('key');await Promise.all([first,second])
 assert.deepEqual(bodies.map(b=>b.expectedRevision),[3,4]);assert.deepEqual(bodies.map(b=>b.messages[0].text),['original','edited']);assert.equal(state.messageQueueRevisions.key,5)
})
test('A stale page cannot silently advance its revision or overwrite another page',async()=>{
 const state={messageQueues:{key:[]},messageQueueRevisions:{key:2}},conflicts=[]
 const writer=createVersionedMessageQueueWriter(state,async()=>response(409,3),()=>true,(key,error)=>conflicts.push([key,error.message]))
 await assert.rejects(writer.write('key'),/Another window/);assert.equal(state.messageQueueRevisions.key,2);assert.equal(conflicts.length,1)
})
test('A lost persistence response pauses consumption instead of guessing the revision',async()=>{
 const state={messageQueues:{key:[]},messageQueueRevisions:{key:2}},conflicts=[]
 const writer=createVersionedMessageQueueWriter(state,async()=>{throw Error('network lost')},()=>true,(_key,error)=>conflicts.push(error))
 await assert.rejects(writer.write('key'),/network lost/);assert.equal(conflicts.length,1);assert.equal(state.messageQueueRevisions.key,2)
})
