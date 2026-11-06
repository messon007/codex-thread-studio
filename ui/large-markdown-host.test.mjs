import assert from 'node:assert/strict'
import test from 'node:test'
import { createLargeMarkdownHost, createReaderSelectionHost, readerCommentReference } from './large-markdown-host.mjs'
function fixture(initialView, capabilities = {}, callbacks = {}) {
  const requests=[],states=[],selections=[]
  let listener,closed=0
  let state={state:'ready',view:'preview',documentId:'doc',revision:'a'.repeat(64),capabilities:{edit:true,source:true,sourceRangeNavigation:true,...capabilities},timings:{firstUsefulContentMs:10},document:{sourceLines:100,fileBytes:2000},outline:{open:false,available:true,total:2},dirty:false,busy:false}
  const client={ready:async()=>state,async request(method,params,{signal}={}){
    requests.push({method,params,signal})
    if(method==='setView')state={...state,view:params.view}
    if(method==='contents')state={...state,outline:{...state.outline,open:params.open}}
    if(method==='search')return callbacks.searchResponse || {matches:[{line:8,location:{kind:'cell',table:0,row:4,column:0}}]}
    if(method==='save'){if(state.error)throw Error(state.error);state={...state,dirty:false}}
    return state
  },async dispose({force}){assert.equal(force,true);closed++}}
  const frame={src:'http://127.0.0.1:4000/markdown-reader/test/#token',getBoundingClientRect:()=>({left:100,top:200}),get contentDocument(){throw Error('Private child DOM accessed')},get contentWindow(){throw Error('Private child global accessed')}}
  const host=createLargeMarkdownHost(frame,state=>states.push(state),value=>selections.push(value),{initialView,...callbacks,createClient(_frame,options){listener=options.onEvent;return client}})
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
 await host.dispose();assert.equal(selections.at(-1),null);const count=selections.length;emit('selection',{quote:'late'});assert.equal(selections.length,count)
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
 for(const stage of ['settings','source','location','reference']){
  const timers=[],originalInterval=globalThis.setInterval
  globalThis.setInterval=(...args)=>{const timer=originalInterval(...args);timers.push(timer);return timer}
  let finish
  const owner={state:'ready',documentId:'doc',revision:'a'.repeat(64),view:'preview',capabilities:{source:true}}
  const client={ready:async()=>owner,request:async method=>{
   if(method===(stage==='settings'?'settings':stage==='source'?'setView':'navigate'))return new Promise(resolve=>{finish=resolve})
   return owner
  },dispose:async()=>{}}
  const host=createLargeMarkdownHost({src:'http://127.0.0.1:4000/markdown-reader/test/#token'},()=>{},()=>{},{initialView:stage==='source'?'source':undefined,initialLocation:stage==='location'?{kind:'epub',cfi:'epubcfi(/6/2!/4/2/1:0)'}:undefined,initialReference:stage==='reference'?{revision:owner.revision,precision:'exact',location:{kind:'line',line:2}}:undefined,createClient:()=>client})
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

test('EPUB initial presentation and CFI are restored before initialized state is published',async()=>{
 const requests=[],states=[]
 const owner={state:'ready',view:'preview',documentId:'epub',revision:'c'.repeat(64),capabilities:{source:false,edit:false},format:'epub',formatState:{ready:true,cfi:'epubcfi(/6/2!/4/2/1:0)'}}
 const saved='epubcfi(/6/4!/4/2/1:0)'
 const client={ready:async()=>owner,request:async(method,params)=>{
  requests.push({method,params})
  if(method==='navigate')owner.formatState={...owner.formatState,cfi:params.location.cfi}
  return owner
 },dispose:async()=>{}}
 const host=createLargeMarkdownHost({src:'http://127.0.0.1:4000/markdown-reader/book/#token'},value=>states.push(value),()=>{},{createClient:()=>client,
  presentation:{epub:{theme:'sepia',flow:'scrolled',fontScale:1.25}},initialLocation:{kind:'epub',cfi:saved}})
 try{
  await host.ready
  assert.deepEqual(requests.map(value=>value.method),['settings','navigate'])
  assert.equal(requests[0].params.epub.theme,'sepia')
  assert.equal(states.filter(value=>value.initialized).length,1)
  assert.equal(states.at(-1).formatState.cfi,saved)
 }finally{await host.dispose()}
})


test('an initial stale reference is rejected before showing initialized content',async()=>{
 const owner={state:'ready',view:'preview',documentId:'doc',revision:'a'.repeat(64),capabilities:{}}
 const requests=[],states=[]
 const client={ready:async()=>owner,request:async method=>{requests.push(method);return owner},dispose:async()=>{}}
 const host=createLargeMarkdownHost({src:'http://127.0.0.1:4000/markdown-reader/doc/#token'},value=>states.push(value),()=>{},
  {createClient:()=>client,initialReference:{revision:'b'.repeat(64),precision:'exact',location:{kind:'epub',cfi:'epubcfi(/6/2!/4/2/1:0)'}}})
 try{
  await assert.rejects(host.ready,/document has changed/)
  assert.deepEqual(requests,['settings'])
  assert.equal(states.some(value=>value.initialized),false)
 }finally{await host.dispose()}
})

test('Notes ownership is delegated only when the fixed package declares the capability',async()=>{
 const {host,requests}=fixture(undefined,{notes:true,presentation:{notesOwner:true}})
 try{
  await host.ready
  assert.equal(requests[0].params.notes,'host')
  assert.equal(requests[0].params.selectionMenu,'host')
  await host.notes({operation:'list',offset:0},{documentId:'old-doc',revision:'c'.repeat(64)})
  assert.equal(requests.at(-1).params.documentId,'old-doc')
  assert.equal(requests.at(-1).params.revision,'c'.repeat(64))
 }finally{await host.dispose()}
})

test('text menus clear on collapsed selection, view/revision changes, and disposal',async()=>{
 const {host,emit,selections,setState}=fixture();await host.ready
 const selection={documentId:'doc',revision:'a'.repeat(64),quote:'text',location:{kind:'line',line:1},precision:'approximate'}
 emit('selection',selection);emit('selection',null);assert.equal(selections.at(-1),null)
 emit('selection',selection);setState({view:'source'});assert.equal(selections.at(-1),null)
 emit('selection',selection);setState({revision:'b'.repeat(64)});assert.equal(selections.at(-1),null)
 await host.dispose();assert.equal(selections.at(-1),null)
})

test('host controls stay unready while a saved reading location is being restored',async()=>{
 let finish
 const states=[],owner={state:'ready',documentId:'book',revision:'a'.repeat(64),view:'preview',capabilities:{}}
 const client={ready:async()=>owner,request:async method=>method==='navigate'?new Promise(resolve=>{finish=resolve}):owner,dispose:async()=>{}}
 const host=createLargeMarkdownHost({src:'http://127.0.0.1:4000/markdown-reader/book/#token'},state=>states.push(state),()=>{},
  {createClient:()=>client,initialLocation:{kind:'epub',cfi:'epubcfi(/6/4!/4/2/1:0)'}})
 try{
  for(let i=0;i<20&&!finish;i++)await Promise.resolve()
  assert.equal(typeof finish,'function')
  assert.equal(states.at(-1).ready,false)
  finish(owner)
  await host.ready
  assert.equal(states.at(-1).ready,true)
 }finally{await host.dispose()}
})

test('reader links retain owner identity, reject stale events and report host failures',async()=>{
 const links=[],errors=[]
 const {host,emit}=fixture(undefined,{}, {onLinkRequested:async value=>{links.push(value);if(value.target==='broken')throw Error('Host denied the link')},onError:error=>errors.push(error.message)})
 await host.ready
 emit('linkRequested',null);emit('linkRequested',{})
 const link={documentId:'doc',revision:'a'.repeat(64),target:'sibling.md',localPath:'/project/sibling.md',precision:'exact',reference:{startByte:3,endByte:20}}
 emit('linkRequested',{...link,documentId:'other'})
 emit('linkRequested',{...link,revision:'b'.repeat(64)})
 emit('linkRequested',{...link,target:'x'.repeat(8193)})
 emit('linkRequested',link)
 await new Promise(resolve=>setTimeout(resolve,0))
 assert.deepEqual(links,[link])
 emit('linkRequested',{...link,target:'broken'})
 await new Promise(resolve=>setTimeout(resolve,0))
 assert.deepEqual(errors,['Host denied the link'])
 await host.dispose();emit('linkRequested',link)
 await new Promise(resolve=>setTimeout(resolve,0))
 assert.equal(links.length,2)
})

test('read-only renderer work stays cancellable while editable operations retain close protection',async()=>{
 for(const edit of [false,true]) {
  const {host,states,setState,closed}=fixture(undefined,{edit,source:edit})
  await host.ready
  setState({busy:true})
  assert.equal(host.busy,edit)
  assert.equal(states.at(-1).busy,edit)
  await host.dispose()
  assert.equal(closed(),1)
 }
})

test('removing a reader disconnects local transport without reporting expected late close errors',async()=>{
 let disconnects=0
 const errors=[]
 const client={ready:async()=>({state:'ready',capabilities:{}}),request:async()=>({state:'ready'}),dispose:()=>Promise.reject(Error('Detached frame')),disconnect(){disconnects++}}
 const host=createLargeMarkdownHost({src:'http://127.0.0.1:4000/reader/#token'},()=>{},()=>{},{createClient:()=>client,onError:error=>errors.push(error)})
 await host.ready
 await assert.rejects(host.dispose(),/Detached frame/)
 assert.equal(disconnects,1)
 assert.deepEqual(errors,[])
})

for(const mode of ['saved','failed','replaced','cleared','disposed'])test(`SQL Comment ${mode} waits for trusted provenance and never exposes a stale menu`,async()=>{
 const selections=[],errors=[]
 let resolveSnapshot,rejectSnapshot
 const deferred=new Promise((resolve,reject)=>{resolveSnapshot=resolve;rejectSnapshot=reject})
 const host={querySnapshot:()=>deferred}
 const controller=createReaderSelectionHost({onSelection:selection=>selections.push(selection),onError:error=>errors.push(error.message)})
 const owner={ready:true,documentId:'one',revision:'a'.repeat(64),view:'preview'}
 controller.update(owner,host)
 const selection={documentId:'one',revision:owner.revision,quote:'2',precision:'exact',location:{kind:'queryCell',queryId:'query',row:0,column:0}}
 const pending=controller.capture(selection)
 assert.equal(selections.filter(Boolean).length,0)
 if(mode==='replaced')controller.update({...owner,documentId:'two'},host)
 if(mode==='cleared')await controller.capture(null)
 if(mode==='disposed')controller.dispose()
 if(mode==='failed')rejectSnapshot(Error('Result expired'))
 else resolveSnapshot({revision:owner.revision,quote:'2',anchor:{query:{sql:'SELECT COUNT(*) FROM t'}}})
 await pending
 if(mode==='saved') {
  assert.equal(selections.filter(Boolean).length,1)
  assert.equal(selections.at(-1).querySnapshot.anchor.query.sql,'SELECT COUNT(*) FROM t')
 } else assert.equal(selections.filter(Boolean).length,0)
 assert.deepEqual(errors,mode==='failed'?['Result expired']:[])
 controller.dispose()
})

test('comment excerpts honor UTF-8 limits before SQL verification while Copy preserves the selected text',async()=>{
 let verified
 const host={querySnapshot:async value=>{verified=value;return {revision:value.revision,quote:value.quote,anchor:{quoteTruncated:value.quoteTruncated}}}}
 const original='中文😀'.repeat(5000)
 const captured=await readerCommentReference(host,{documentId:'one',revision:'a'.repeat(64),quote:original,location:{kind:'queryCell',queryId:'query',row:0,column:0}},new AbortController().signal)
 assert.ok(Buffer.byteLength(verified.quote)<=16384)
 assert.ok(original.startsWith(verified.quote));assert.equal(verified.quote.includes('\ufffd'),false)
 assert.equal(verified.quoteTruncated,true);assert.equal(captured.querySnapshot.quote,verified.quote)
 assert.equal(captured.copyText,original)
})

test('jq host uses the shared component and captured owner, and removes late/obsolete controls',async()=>{
 const {createReaderJqHost}=await import('./large-markdown-host.mjs')
 const {readerJqProvider}=await import('../crates/marknoria/web/jq-query.mjs')
 class Control extends EventTarget { setAttribute(){} remove(){this.removed=true} }
 const buttons=[],mounts=[],requests=[],pending=[]
 const root={ownerDocument:{body:{},createElement(){return new Control()}}}
 const toolbar={append(button){buttons.push(button)}}
 let defer=true
 const module={readerJqProvider,mountJqQuery(_root,options){const record={options,disposed:false};mounts.push(record);return {dispose(){record.disposed=true}}}}
 const adapter=createReaderJqHost(root,toolbar,{load:()=>defer?new Promise(resolve=>pending.push(resolve)):Promise.resolve(module)})
 const owner={initialized:true,ready:true,documentId:'one',revision:'a'.repeat(64),name:'data.jsonl',size:2*1024*1024,capabilities:{queries:true}}
 const host={async request(method,params){requests.push({method,params});return {}}}
 try{
  adapter.update(owner,host);buttons[0].dispatchEvent(new Event('click'))
  adapter.update({...owner,documentId:'two',revision:'b'.repeat(64)},host)
  pending[0](module);await new Promise(resolve=>setImmediate(resolve))
  assert.equal(mounts.length,0);assert.equal(buttons[0].removed,true)
  defer=false;buttons[1].dispatchEvent(new Event('click'));await new Promise(resolve=>setImmediate(resolve))
  assert.equal(mounts.length,1);assert.equal(mounts[0].options.autoRun,false)
  await mounts[0].options.provider.run({filter:'.id',slurp:false})
  assert.equal(requests[0].method,'jq');assert.equal(requests[0].params.documentId,'two');assert.equal(requests[0].params.revision,'b'.repeat(64))
  adapter.update({...owner,name:'plain.xml'},host)
  assert.equal(mounts[0].disposed,true);assert.equal(buttons[1].removed,true)
  buttons[1].dispatchEvent(new Event('click'));await new Promise(resolve=>setImmediate(resolve));assert.equal(mounts.length,1)
  adapter.update({...owner,size:200},host);buttons[2].dispatchEvent(new Event('click'));await new Promise(resolve=>setImmediate(resolve))
  assert.equal(mounts[1].options.autoRun,true)
 }finally{adapter.dispose()}
 assert.equal(mounts.at(-1).disposed,true)
})
