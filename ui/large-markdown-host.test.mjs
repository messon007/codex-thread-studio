import assert from 'node:assert/strict'
import test from 'node:test'
import { createLargeMarkdownHost } from './large-markdown-host.mjs'
function fixture(initialView, capabilities = {}) {
  const requests=[],states=[],selections=[]
  let listener,closed=0
  let state={state:'ready',view:'preview',documentId:'doc',revision:'a'.repeat(64),capabilities:{edit:true,source:true,sourceRangeNavigation:true,...capabilities},timings:{firstUsefulContentMs:10},document:{sourceLines:100,fileBytes:2000},outline:{open:false,available:true,total:2},dirty:false,busy:false}
  const client={ready:async()=>state,async request(method,params,{signal}={}){
    requests.push({method,params,signal})
    if(method==='setView')state={...state,view:params.view}
    if(method==='contents')state={...state,outline:{...state.outline,open:params.open}}
    if(method==='search')return {matches:[{line:8,location:{kind:'cell',table:0,row:4,column:0}}]}
    if(method==='save'){if(state.error)throw Error(state.error);state={...state,dirty:false}}
    return state
  },async dispose({force}){assert.equal(force,true);closed++}}
  const frame={src:'http://127.0.0.1:4000/markdown-reader/test/#token',getBoundingClientRect:()=>({left:100,top:200}),get contentDocument(){throw Error('Private child DOM accessed')},get contentWindow(){throw Error('Private child global accessed')}}
  const host=createLargeMarkdownHost(frame,state=>states.push(state),value=>selections.push(value),{initialView,createClient(_frame,options){listener=options.onEvent;return client}})
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
test('typed reference navigation retains coordinates and refuses stale, draft, and transient targets',async()=>{
 const {host,requests}=fixture();await host.ready
 const reference={revision:'a'.repeat(64),precision:'exact',location:{kind:'source',startByte:3,endByte:13}}
 await host.navigateReference(reference)
 assert.deepEqual(requests.at(-1),{method:'navigate',params:{location:reference.location},signal:undefined})
 const count=requests.length
 await assert.rejects(host.navigateReference({...reference,revision:'b'.repeat(64)}),/changed/)
 await assert.rejects(host.navigateReference({...reference,precision:'draft'}),/draft/)
 await assert.rejects(host.navigateReference({...reference,location:{kind:'queryCell',queryId:'old',row:0,column:0}}),/transient/)
 assert.equal(requests.length,count)
 await host.dispose()
})
test('late foreign document or revision selections never enter Studio actions',async()=>{
 const {host,emit,selections}=fixture();await host.ready
 const selection={documentId:'doc',revision:'a'.repeat(64),quote:'quote',location:{kind:'line',line:2},precision:'approximate'}
 emit('selection',{...selection,documentId:'another'})
 emit('selection',{...selection,revision:'b'.repeat(64)})
 assert.equal(selections.length,0)
 emit('selection',selection);assert.equal(selections.length,1)
 await host.dispose()
})
test('reader Open shortcut delegates to its Studio host',async()=>{
 let opens=0,listener
 const client={ready:async()=>({state:'ready',documentId:'doc',revision:'a'.repeat(64)}),request:async()=>({state:'ready'}),dispose:async()=>{}}
 const frame={src:'http://127.0.0.1:4000/markdown-reader/test/#token'}
 const host=createLargeMarkdownHost(frame,()=>{},()=>{},{createClient(_frame,options){listener=options.onEvent;return client},onOpenRequested:()=>{opens++}})
 await host.ready
 listener('openRequested',{})
 assert.equal(opens,1)
 await host.dispose()
 listener('openRequested',{})
 assert.equal(opens,1)
})
test('saved Source mode is restored through the public client after initialization',async()=>{
 const {host,states,requests}=fixture('source')
 await host.ready
 assert.equal(states.at(-1).view,'source')
 assert.deepEqual(requests.filter(value=>value.method==='setView').map(value=>value.params),[{view:'source'}])
 await host.dispose()
})
test('binary capability never restores an unsupported Source or Edit view',async()=>{
 for(const view of ['source','edit']){
  const {host,states,requests}=fixture(view,{source:false,edit:false})
  await host.ready
  assert.equal(states.at(-1).view,'preview')
  assert.equal(requests.filter(value=>value.method==='setView').length,0)
  await host.dispose()
 }
})
test('closing during settings or restored-mode initialization never creates a late heartbeat',async()=>{
 for(const stage of ['settings','source']){
  const timers=[],originalInterval=globalThis.setInterval
  globalThis.setInterval=(...args)=>{const timer=originalInterval(...args);timers.push(timer);return timer}
  let finish
  const owner={state:'ready',documentId:'doc',revision:'a'.repeat(64),view:'preview',capabilities:{source:true}}
  const client={ready:async()=>owner,request:async method=>{
   if(method===(stage==='settings'?'settings':'setView'))return new Promise(resolve=>{finish=resolve})
   return owner
  },dispose:async()=>{}}
  const host=createLargeMarkdownHost({src:'http://127.0.0.1:4000/markdown-reader/test/#token'},()=>{},()=>{},{initialView:'source',createClient:()=>client})
  try{
   for(let i=0;i<20&&!finish;i++)await Promise.resolve()
   assert.equal(typeof finish,'function')
   await host.dispose()
   finish({...owner,view:stage==='source'?'source':'preview'})
   await host.ready
   assert.equal(timers.length,0)
  }finally{
   await host.dispose()
   for(const timer of timers)clearInterval(timer)
   globalThis.setInterval=originalInterval
  }
 }
})
