import test from 'node:test'
import assert from 'node:assert/strict'
import {createTeamsClient} from './teams.mjs'
const fixture = replies => {
  const calls=[]
  const client=createTeamsClient(async (_url,options)=>{calls.push(JSON.parse(options.body));const value=replies.shift();if(value instanceof Error)throw value;return {ok:true,json:async()=>value}})
  return {client,calls}
}
test('Teams reservation preserves arbitrary ordinary turn payload and releases only after acknowledgement',async()=>{
 const {client,calls}=fixture([{granted:true,id:'hold-1'},{released:true}]);const payload={input:[{type:'image',url:'fixture'}],model:'original-model',effort:'high'}
 const response=await client.guarded('codex','thread','queue',async()=>payload)
 assert.equal(response,payload);assert.deepEqual(calls.map(c=>c.action),['reserve','release'])
})
test('Teams never calls sender when native queue has an in-flight delivery',async()=>{
 const {client,calls}=fixture([{granted:false,id:'pending-hold'},{released:true}]);let sent=false
 await assert.rejects(client.guarded('codex','thread','queue',async()=>{sent=true}),/pending or uncertain/)
 assert.equal(sent,false);assert.deepEqual(calls.map(c=>c.action),['reserve','release'])
})
test('An uncertain ordinary sender retains its durable hold across page closure',async()=>{
 const {client,calls}=fixture([{granted:true,id:'hold'}]);await assert.rejects(client.guarded('codex','thread','queue',async()=>{throw Error('transport closed')}),/transport closed/)
 assert.equal(calls.length,1)
})
test('Nonmember send is a no-op reservation with no release',async()=>{
 const {client,calls}=fixture([{granted:true,id:null}]);assert.equal(await client.guarded('codex','thread','send',async()=>42),42);assert.equal(calls.length,1)
})
test('Gateway rejection is surfaced without sending a task',async()=>{
 const client=createTeamsClient(async()=>({ok:false,json:async()=>({error:'RevisionConflict'})}));await assert.rejects(client.reserve('codex','thread','send'),/RevisionConflict/)
})

test('Retry after an uncertain task mutation reuses its exact idempotency key',async()=>{
 const {client,calls}=fixture([Error('connection lost'),{task:{id:'created'}},{task:{id:'intentional-second'}}])
 const body={action:'command',group_id:'g',request_id:'first-id',command:{op:'Delegate',delegation:{title:'same work'}}}
 await assert.rejects(client.request(body),/connection lost/)
 await client.request({...body,request_id:'new-click-id'})
 assert.equal(calls[1].request_id,'first-id')
 await client.request({...body,request_id:'intentional-next-id'})
 assert.equal(calls[2].request_id,'intentional-next-id')
})

test('Managed user start uses the host receipt and preserves exact options',async()=>{
 const {client,calls}=fixture([{managed:true,result:{turn:{id:'native'}}}]);let sent=false
 const params={threadId:'member',clientUserMessageId:'queued-stable',input:[{type:'image',url:'fixture'}],model:'model',effort:'high'}
 assert.deepEqual(await client.start('codex',params,async()=>{sent=true}),{turn:{id:'native'}})
 assert.equal(sent,false);assert.deepEqual(calls[0].params,params)
})
test('Unmanaged session starts retain the original transport',async()=>{
 const {client}=fixture([{managed:false}]);assert.equal(await client.start('codex',{threadId:'outside'},async()=>19),19)
})
