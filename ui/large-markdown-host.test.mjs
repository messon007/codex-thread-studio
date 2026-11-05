import assert from 'node:assert/strict'
import test from 'node:test'
import { createLargeMarkdownHost } from './large-markdown-host.mjs'
function fixture() {
  const requests=[],states=[],selections=[]
  let listener,closed=0
  let state={state:'ready',view:'preview',documentId:'doc',revision:'a'.repeat(64),capabilities:{edit:true},timings:{firstUsefulContentMs:10},document:{sourceLines:100,fileBytes:2000},outline:{open:false,available:true,total:2},dirty:false,busy:false}
  const client={ready:async()=>state,async request(method,params,{signal}={}){
    requests.push({method,params,signal})
    if(method==='setView')state={...state,view:params.view}
    if(method==='contents')state={...state,outline:{...state.outline,open:params.open}}
    if(method==='search')return {matches:[{line:8,location:{kind:'cell',table:0,row:4,column:0}}]}
    if(method==='save'){if(state.error)throw Error(state.error);state={...state,dirty:false}}
    return state
  },async dispose({force}){assert.equal(force,true);closed++}}
  const frame={src:'http://127.0.0.1:4000/markdown-reader/test/#token',getBoundingClientRect:()=>({left:100,top:200}),get contentDocument(){throw Error('Private child DOM accessed')},get contentWindow(){throw Error('Private child global accessed')}}
  const host=createLargeMarkdownHost(frame,state=>states.push(state),value=>selections.push(value),{createClient(_frame,options){listener=options.onEvent;return client}})
  return {host,requests,states,selections,emit:(type,value)=>listener(type,value),setState:patch=>{state={...state,...patch};listener('state',state)},closed:()=>closed}
}
test('host controls use only the public protocol and preserve dirty/save errors',async()=>{
 const {host,requests,states,setState,closed}=fixture()
 await host.ready
 assert.equal(states.at(-1).lineCount,100)
 assert.equal(states.at(-1).readingReady,true)
 assert.equal(requests[0].method,'settings');assert.equal(requests[0].params.toolbar,'host')
 await host.setView('source');assert.equal(states.at(-1).view,'source')
 await host.setOutline(true);assert.equal(states.at(-1).outlineOpen,true)
 setState({dirty:true,error:'Changed outside Studio'})
 await assert.rejects(host.save(),/Changed outside/);assert.equal(host.dirty,true)
 setState({error:null});assert.equal(await host.save(),true);assert.equal(host.dirty,false)
 const matches=await host.search('needle');await host.navigateMatch(matches[0])
 assert.deepEqual(requests.at(-1).params.location,{kind:'line',line:8})
 await host.setView('preview');await host.navigateMatch(matches[0])
 assert.deepEqual(requests.at(-1).params.location,{kind:'cell',table:0,row:4,column:0})
 await host.dispose();assert.equal(closed(),1)
 await assert.rejects(host.setView('source'),/disposed/)
})
test('typed selections preserve scope and precision while converting frame coordinates',async()=>{
 const {host,emit,selections}=fixture();await host.ready
 emit('selection',{documentId:'doc',revision:'a'.repeat(64),quote:'\u4e2d\u6587😀',precision:'exact',location:{kind:'source',startByte:3,endByte:13},viewportRect:{x:5,y:10,width:40,height:20}})
 const selection=selections[0]
 assert.equal(selection.documentId,'doc');assert.equal(selection.precision,'exact')
 assert.deepEqual(selection.location,{kind:'source',startByte:3,endByte:13})
 assert.equal(selection.rect.left,105);assert.equal(selection.rect.top,210)
 assert.equal(selection.startLine,null)
 await host.dispose();emit('selection',{quote:'late'});assert.equal(selections.length,1)
})
