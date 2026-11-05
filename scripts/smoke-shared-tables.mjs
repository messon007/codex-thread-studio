// Independent Studio fixtures; the existing locked ExcelJS bundle is the oracle.
import assert from 'node:assert/strict'
import { spawn, execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { mkdir, mkdtemp, writeFile, readFile, readdir, rm } from 'node:fs/promises'
import { resolve, join } from 'node:path'
import { once } from 'node:events'
import { Workbook } from '../ui/vendor/artifact-table.mjs'
// Expected Excel fixture values only; document parsing belongs to the SDK.
const cellText = value => value == null ? '' : value instanceof Date ? value.toISOString() : typeof value === 'object' ? String(value.text ?? value.result ?? value.formula ?? value.hyperlink ?? '') : String(value)
const root=resolve(new URL('../',import.meta.url).pathname)
const output=join(root,'target/shared-tables-smoke')
await mkdir(output,{recursive:true})
const directory=await mkdtemp(join(output,'run-'))
const workspace=join(directory,'workspace')
await mkdir(workspace)
const fields=['ID','Value','Code','Note']
const rows=Array.from({length:123},(_,row)=>[`row-${row}`,String(row),'001',row===0?'quoted; comma,\nline':'literal <b>text</b>'])
const encoded=delimiter=>'\ufeff'+[fields,...rows].map(row=>row.map(value=>'"'+value.replaceAll('"','""')+'"').join(delimiter)).join('\r\n')+'\r\n'
const files=[{path:join(workspace,'data.csv'),format:'csv',first:rows[0]}, {path:join(workspace,'data.tsv'),format:'tsv',first:rows[0]}]
await writeFile(files[0].path,encoded(';'))
await writeFile(files[1].path,encoded('\t'))
const book=new Workbook(),sheet=book.addWorksheet('Data'),other=book.addWorksheet('Other')
sheet.addRow([...fields,'Date','Uncached','Cached','Flag'])
for(const [row,values] of rows.entries())sheet.addRow([...values,row===0?new Date('2024-01-02T03:04:05.000Z'):null,row===0?{formula:'B2+1'}:null,row===0?{formula:'1+1',result:2}:null,false])
other.addRow(['Name']);other.addRow(['Second-sheet sentinel'])
const workbook=await book.xlsx.writeBuffer()
const xlsx=join(workspace,'data.xlsx');await writeFile(xlsx,workbook)
files.push({path:xlsx,format:'xlsx',first:sheet.getRow(2).values.slice(1).map(cellText)})
const originals=await Promise.all(files.map(file=>readFile(file.path)))
const environment={PATH:'/usr/bin:/bin',LANG:'C.UTF-8',XDG_CONFIG_HOME:join(directory,'config'),XDG_DATA_HOME:join(directory,'state'),XDG_CACHE_HOME:join(directory,'cache'),READER_CACHE_DIR:join(directory,'reader-cache'),READER_CHROMIUM:'/intentionally-unavailable-chromium'}
for(const name of ['XDG_RUNTIME_DIR','WAYLAND_DISPLAY','GDK_BACKEND','DBUS_SESSION_BUS_ADDRESS','WEBKIT_DISABLE_COMPOSITING_MODE'])if(process.env[name])environment[name]=process.env[name]
const server=spawn(process.env.STUDIO_REMOTE_BINARY || join(root,'target/release/codex-thread-studio'),['--serve','--listen','127.0.0.1:0'],{env:environment,stdio:['ignore','pipe','pipe']})
let log='',browser
server.stdout.on('data',bytes=>{log=(log+bytes).slice(-100000)})
server.stderr.on('data',()=>{})
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms))
try{
 const deadline=Date.now()+30000
 while(!/http:\/\/127\.0\.0\.1:\d+\/#token=[a-f0-9]{32}/u.test(log)){
  if(Date.now()>deadline||server.exitCode!=null)throw Error('Synthetic table gateway did not start')
  await pause(30)
 }
 const url=new URL(log.match(/http:\/\/127\.0\.0\.1:\d+\/#token=[a-f0-9]{32}/u)[0])
 const token=new URLSearchParams(url.hash.slice(1)).get('token')
 const post=(value,headers={})=>fetch(new URL('/studio/reader/open',url),{method:'POST',headers:{'Content-Type':'application/json',...headers},body:JSON.stringify(value)})
 assert.equal((await post({root:workspace,path:files[0].path})).status,401)
 assert.equal((await post({root:workspace,path:files[0].path},{Authorization:'Bearer '+token,Origin:'https://forbidden.invalid'})).status,403)
 const outside=join(directory,'outside.csv');await writeFile(outside,'ID,Value\noutside,1\n')
 assert.equal((await post({root:workspace,path:outside},{Authorization:'Bearer '+token})).status,403)
 const {chromium}=await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
 browser=await chromium.launch({executablePath:process.env.STUDIO_CHROMIUM_PATH})
 const page=await browser.newPage({viewport:{width:1500,height:950}}),errors=[]
 page.on('pageerror',error=>errors.push(error.message))
 await page.route(`${url.origin}/app.js`,route=>route.fulfill({contentType:'text/javascript',body:`
  import {createDocumentWorkspaceController,createDocumentWorkspaceState} from '/document-workspace-controller.mjs';
  window.testState=createDocumentWorkspaceState();
  const gatewayFetch=(path,options={})=>fetch(path,{...options,headers:{...options.headers,Authorization:'Bearer '+window.__CODEX_THREAD_STUDIO_GATEWAY__.token}});
  window.controller=createDocumentWorkspaceController({state:testState,gatewayFetch,randomId:()=>crypto.randomUUID(),reportError:error=>{throw error},view:{
   selectedThread:()=>({cwd:${JSON.stringify(workspace)}}),selectedStateKey:()=>'codex:table-fixture',activateRightWorkspace:tool=>{testState.activeRightWorkspace=tool;document.querySelector('#artifact-rail').classList.toggle('hidden',tool!=='document');document.querySelector('#annotation-rail').classList.toggle('hidden',tool!=='comments')},
   hideComposerMenu(){},closeActionMenus(){},applyRightRailWidth(){},disconnectMarkdownImageObserver(){},disposeMarkdownImageAssets(){},hideSelection(){},
   setSelection(selection,rect){window.captured={selection,rect}},openCommentForSelection(selection){window.captured={selection};window.reviewNotes?.openCommentForSelection(selection)},renderMarkdownDocument(){throw Error('Legacy parser must not run')},openResources(){window.openRequests=(window.openRequests||0)+1}
  }});controller.bind();
 `}))
 await page.goto(url.href)
 await page.waitForFunction(()=>Boolean(window.controller))
 const results=[]
 for(const file of files){
  const started=Date.now()
  await page.evaluate(async({path,root})=>{window.captured=null;await controller.open({root,path},{allowDetachedRoot:true})},{path:file.path,root:workspace})
  assert.equal(await page.evaluate(()=>testState.artifact.kind),'large-markdown')
  const owner=await page.evaluate(async()=>{const {createReaderClient}=await import('/reader-assets/host-client.mjs');window.client=createReaderClient(document.querySelector('.artifact-large-markdown-frame'));return client.ready()})
  assert.equal(owner.format,file.format)
  assert.equal(owner.view,'preview')
  assert.equal(owner.capabilities.source,file.format!=='xlsx')
  assert.equal(owner.capabilities.edit,file.format!=='xlsx')
  assert.equal(await page.locator('#artifact-source').isVisible(),file.format!=='xlsx')
  assert.equal(await page.locator('#artifact-edit').isVisible(),file.format!=='xlsx')
  const command=(method,params={})=>page.evaluate(({method,params})=>client.request(method,params),{method,params})
  let frame=await page.locator('.artifact-large-markdown-frame').elementHandle().then(element=>element.contentFrame())
  await frame.locator('.table-content tbody tr').first().waitFor()
  const hostFirstRowsMs=Date.now()-started
  const opening=(await command('state')).timings
  const info=await command('tableInfo',{table:0})
  assert.equal(info.rows,123)
  const queried=await command('sql',{table:0,sql:'SELECT COUNT(*),SUM(CAST(c2 AS INTEGER)),MIN(c3) FROM t'})
  assert.deepEqual(queried.rows[0],['123','7503','001'])
  await command('sqlClose',{id:queried.id})
  const first=await command('sql',{table:0,sql:'SELECT '+info.columns.map((_,index)=>'c'+(index+1)).join(',')+' FROM t LIMIT 1'})
  assert.deepEqual(first.rows[0],file.first)
  await command('sqlClose',{id:first.id})
  const chart=await command('tableChart',{source:{kind:'table',table:0},x:0,y:1})
  assert.equal(chart.datasetScope,'complete-source-table')
  assert.equal(chart.points.length,123)
  assert.equal(chart.points[122].value,122)
  await command('navigate',{location:{kind:'cell',table:0,row:122,column:2}})
  await page.locator('.notes-toolbar [data-action=ask-ai]').click()
  await page.waitForFunction(()=>captured?.selection?.source?.anchor?.readerReference?.location?.row===122)
  const selected=await page.evaluate(()=>captured.selection)
  assert.equal(selected.quote,'001')
  assert.equal(selected.source.anchor.readerReference.precision,'exact')
  assert.deepEqual(selected.source.anchor.readerReference.location,{kind:'cell',table:0,row:122,column:2})
  assert.equal(owner.capabilities.notes,true)
  assert.equal(owner.capabilities.queryNotes,true)
  await page.locator('.notes-open').waitFor()
  assert.equal(await frame.locator('.notes-toolbar').count(),0)
  assert.equal((await command('notes',{request:{operation:'list',offset:0}})).total,0)
  let notesChecked=false
  if(file.format==='csv'){
   const listNotes=()=>command('notes',{request:{operation:'list',offset:0}})
   const waitNotes=async count=>{
    const deadline=Date.now()+5000
    do{const result=await listNotes();if(result.total===count)return result;await pause(30)}while(Date.now()<deadline)
    throw Error('Notes did not reach '+count+' records')
   }
   const cell=frame.locator('.table-content tbody tr').last().locator('td').nth(2)
   await cell.click()
   await page.locator('.notes-toolbar').getByRole('button',{name:'Favorite',exact:true}).click()
   const favorite=(await waitNotes(1)).items[0]
   assert.deepEqual(favorite.anchor.location,{kind:'cell',table:0,row:122,column:2})
   assert.equal(favorite.quote,'001')
   await cell.click()
   await page.locator('.notes-toolbar').getByRole('button',{name:'Comment',exact:true}).click()
   await page.locator('.notes-editor').getByRole('textbox',{name:'Comment',exact:true}).fill('Local cell comment')
   await page.locator('.notes-editor').getByRole('button',{name:'Save comment',exact:true}).click()
   await waitNotes(2)
   await page.locator('.notes-dialog[open]').getByRole('button',{name:'Close',exact:true}).click()
   await frame.getByRole('button',{name:'SQL',exact:true}).first().click()
   await frame.getByRole('textbox',{name:'SQL query',exact:true}).fill('SELECT SUM(CAST(c2 AS INTEGER)) AS total FROM t')
   await frame.getByRole('button',{name:'Run query',exact:true}).click()
   const aggregate=frame.locator('.table-query-result tbody td')
   await aggregate.getByText('7503',{exact:true}).waitFor()
   await aggregate.getByText('7503',{exact:true}).click()
   await page.locator('.notes-toolbar').getByRole('button',{name:'Favorite',exact:true}).click()
   const withQuery=await waitNotes(3)
   const snapshot=withQuery.items.find(item=>item.anchor.query)
   assert.equal(snapshot.quote,'7503')
   assert.equal(snapshot.anchor.location,null)
   assert.equal(snapshot.anchor.query.sourceRows,123)
   assert.equal(snapshot.anchor.query.sql,'SELECT SUM(CAST(c2 AS INTEGER)) AS total FROM t')
   await frame.locator('.table-query-dialog').getByRole('button',{name:'Close',exact:true}).click()
   const copied=await command('notes',{request:{operation:'copy',id:snapshot.id}})
   assert.match(copied.text,/snapshot, not a source cell/)
   await page.locator('.notes-open').click()
   await page.locator('.notes-dialog[open] .note-card').waitFor()
   const libraryBounds=await page.locator('.notes-dialog[open]').boundingBox(), viewport=page.viewportSize()
   assert.ok(libraryBounds.x>=0&&libraryBounds.y>=0&&libraryBounds.x+libraryBounds.width<=viewport.width&&libraryBounds.y+libraryBounds.height<=viewport.height)
   if(process.env.STUDIO_SMOKE_SCREENSHOTS){await mkdir(process.env.STUDIO_SMOKE_SCREENSHOTS,{recursive:true});await page.screenshot({path:join(process.env.STUDIO_SMOKE_SCREENSHOTS,'notes-library.png')})}
   await page.locator('.notes-filters').getByRole('button',{name:'Comments',exact:true}).click()
   await page.locator('.notes-filters').getByRole('button',{name:'Clear comments',exact:true}).click()
   await page.locator('.notes-confirm').getByRole('button',{name:'Clear all',exact:true}).click()
   const remaining=await waitNotes(2)
   assert.ok(remaining.items.every(item=>item.kind==='favorite'))
   await page.locator('.notes-dialog[open]').getByRole('button',{name:'Close',exact:true}).click()
   // Exercise the real business draft controller and durable session writer,
   // rather than treating a serialized selection alone as AI-draft adoption.
   await page.evaluate(async()=>{
    const {createReviewNotesController,createReviewNotesState}=await import('/review-notes-controller.mjs')
    const {CommentSourceRegistry,createCommentDraft}=await import('/comment-core.mjs')
    const {createDocumentCommentProvider}=await import('/comment-source-providers.mjs')
    const {createSerializedStateWriter}=await import('/serialized-state-writer.mjs')
    const {createSessionStatePersistence}=await import('/session-state-persistence.mjs')
    Object.assign(testState,createReviewNotesState(),{selectedId:'table-fixture',backend:'codex',model:{turns:[]},activeAnnotationPromptTemplate:'{{annotations}}\n{{additional}}'})
    window.draftRegistry=new CommentSourceRegistry().register(createDocumentCommentProvider())
    window.gatewayForDrafts=(path,options={})=>fetch(path,{...options,headers:{...options.headers,Authorization:'Bearer '+window.__CODEX_THREAD_STUDIO_GATEWAY__.token}})
    const persistence=createSessionStatePersistence(testState,createSerializedStateWriter(gatewayForDrafts,error=>{throw error}),()=>true)
    window.reviewNotes=createReviewNotesController({state:testState,commentSources:draftRegistry,gatewayFetch:gatewayForDrafts,randomId:()=>crypto.randomUUID(),persistAnnotationState:key=>window.annotationSaved=persistence.annotations(key),reportError:error=>{throw error},view:{
     selectedStateKey:()=>'codex:table-fixture',selectedThread:()=>({cwd:testState.artifact.root}),threadTitle:()=> 'Reader fixture',
     closeActionMenus(){},activateRightWorkspace:tool=>{testState.activeRightWorkspace=tool;document.querySelector('#artifact-rail').classList.toggle('hidden',tool!=='document');document.querySelector('#annotation-rail').classList.toggle('hidden',tool!=='comments')},syncRightWorkspaceLaunchers(){},
     renderArtifact:()=>controller.render(),renderSessionMap(){},openArtifact:(file,options)=>controller.open(file,options),setArtifactView:view=>controller.setView(view),
     renderedItem:()=>null,pauseTranscript(){},setComposerValue(){},submitComposer(){throw Error('No AI request is permitted in this fixture')}
    }})
    reviewNotes.bind()
    window.restoreDrafts=async()=>{const snapshot=await(await gatewayForDrafts('/studio/session-state')).json();testState.annotationDrafts=Object.fromEntries(Object.entries(snapshot.annotationDrafts).map(([key,drafts])=>[key,drafts.map(draft=>createCommentDraft(draft,{registry:draftRegistry}))]));reviewNotes.renderAnnotations();return reviewNotes.currentAnnotations()}
   })
   for(const [query,expected] of [['SELECT SUM(CAST(c2 AS INTEGER)) AS total FROM t','7503'],["SELECT '"+'v'.repeat(16300)+"' AS saved_value FROM t LIMIT 1",'v'.repeat(16000)]]) {
    await frame.getByRole('button',{name:'SQL',exact:true}).first().click()
    await frame.getByRole('textbox',{name:'SQL query',exact:true}).fill(query)
    await frame.getByRole('button',{name:'Run query',exact:true}).click()
    await frame.locator('.table-query-editor[aria-busy="false"]').waitFor()
    const cell=frame.locator('.table-query-result tbody td').first();await cell.waitFor();await cell.click()
    await page.locator('.notes-toolbar').getByRole('button',{name:'Ask AI',exact:true}).click()
    await page.locator('#annotation-dialog[open]').waitFor().catch(async error=>{throw Error(error.message+' '+JSON.stringify({errors,captured:await page.evaluate(()=>({quoteLength:window.captured?.selection?.quote?.length,snapshot:Boolean(window.captured?.selection?.source?.anchor?.readerReference?.querySnapshot),status:document.querySelector('.notes-status')?.textContent,pending:Boolean(testState.pendingAnnotation)}))}))})
    assert.equal(await page.locator('#annotation-quote').textContent(),expected)
    await page.locator('#annotation-comment').fill('Explain this derived result')
    await page.locator('#save-annotation').click();await page.evaluate(()=>annotationSaved)
    await frame.locator('.table-query-dialog').getByRole('button',{name:'Close',exact:true}).click()
   }
   assert.equal((await listNotes()).total,2,'AI drafts must not create Notes records')
   assert.equal(await page.evaluate(()=>controller.close({restoreMap:false})),true)
   const restored=await page.evaluate(()=>restoreDrafts())
   assert.equal(restored.length,2)
   assert.equal(restored[0].source.anchor.readerReference.querySnapshot.quote,'7503')
   assert.equal(restored[1].source.anchor.readerReference.querySnapshot.anchor.quoteTruncated,true)
   assert.ok(Buffer.byteLength(JSON.stringify(restored[1].source.anchor))>32768)
   const prompt=await page.evaluate(()=>reviewNotes.buildPrompt(reviewNotes.currentAnnotations()))
   assert.match(prompt,/SQL result snapshot; not a source cell/);assert.match(prompt,/SELECT SUM/);assert.match(prompt,/sourceRows/)
   const queryRequests=[];page.on('request',request=>{if(request.url().includes('/api/table-query'))queryRequests.push(request.url())})
   const before=queryRequests.length
   await page.setViewportSize({width:480,height:480})
   await page.evaluate(()=>reviewNotes.openAnnotations())
   await page.locator('.annotation-source').nth(1).click()
   const inspector=page.getByRole('dialog',{name:'SQL result snapshot',exact:true});await inspector.waitFor()
   assert.equal(await inspector.locator('pre').first().textContent(),'v'.repeat(16000))
   const bounds=await inspector.boundingBox();assert.ok(bounds.x>=0&&bounds.y>=0&&bounds.x+bounds.width<=480&&bounds.y+bounds.height<=480)
   await inspector.press('Escape')
   assert.equal(queryRequests.length,before,'Opening a historical snapshot must not rerun SQL')
   assert.equal(await page.evaluate(()=>testState.artifact),null)
   await page.evaluate(()=>{reviewNotes.closeAnnotations();window.reviewNotes=undefined})
   await page.setViewportSize({width:1500,height:950})
   await page.evaluate(async({path,root})=>{await controller.open({root,path},{allowDetachedRoot:true});const {createReaderClient}=await import('/reader-assets/host-client.mjs');window.client=createReaderClient(document.querySelector('.artifact-large-markdown-frame'));await client.ready()}, {path:file.path,root:workspace})
   frame=await page.locator('.artifact-large-markdown-frame').elementHandle().then(element=>element.contentFrame())
   notesChecked=true
  }

  if(file.format==='xlsx'){
   assert.equal(owner.document.tables,2)
   assert.equal((await command('tableInfo',{table:1})).label,'Other')
   const second=await command('sql',{table:1,sql:'SELECT c1 FROM t'})
   assert.deepEqual(second.rows,[['Second-sheet sentinel']])
   await command('sqlClose',{id:second.id})
   assert.equal(await page.evaluate(()=>client.request('setView',{view:'edit'}).then(()=>false,()=>true)),true)
  }else{
   await command('setView',{view:'source'})
   const source=await command('sourceWindow',{line:1})
   assert.equal(source.result.text,await readFile(file.path,'utf8'))
   await command('setView',{view:'preview'})
  }
  assert.ok(await frame.locator('.table-query-open').count())
  assert.ok(await frame.locator('.table-chart-open').count())
  if(file.format!=='xlsx'){
   await command('setView',{view:'edit'})
   const editor=frame.locator('#editor-mount .cm-content')
   await editor.click();await editor.press('Control+End')
   const delimiter=file.format==='csv'?';':'\t'
   const added=['row-123','123','001','added'].join(delimiter)
   await page.keyboard.insertText(added)
   await command('save')
   await command('setView',{view:'preview'})
   const updated=await command('sql',{table:0,sql:'SELECT COUNT(*) FROM t'})
   assert.deepEqual(updated.rows,[['124']])
   await command('sqlClose',{id:updated.id})
   await command('setView',{view:'edit'})
   await frame.locator('#edit-undo').click()
   await command('save')
   await command('setView',{view:'preview'})
   const restored=await command('sql',{table:0,sql:'SELECT COUNT(*) FROM t'})
   assert.deepEqual(restored.rows,[['123']])
   await command('sqlClose',{id:restored.id})
   await page.evaluate(async({path,root})=>{
    await controller.open({root,path},{allowDetachedRoot:true,viewState:{view:'source'}})
    const {createReaderClient}=await import('/reader-assets/host-client.mjs')
    window.client=createReaderClient(document.querySelector('.artifact-large-markdown-frame'))
    await client.ready()
   },{path:file.path,root:workspace})
   await page.waitForFunction(()=>testState.artifactView==='source')
   assert.equal((await command('state')).view,'source')
   assert.equal(await page.locator('#artifact-preview').isEnabled(),true)
   await command('setView',{view:'preview'})
   frame=await page.locator('.artifact-large-markdown-frame').elementHandle().then(element=>element.contentFrame())

  }

  if(notesChecked) assert.equal((await command('notes',{request:{operation:'list',offset:0}})).total,2)
  const opens=await page.evaluate(()=>window.openRequests||0)
  await frame.locator('#viewport').press('Control+o')
  await page.waitForFunction(before=>window.openRequests>before,opens)
  let webkit
  if(process.argv.includes('--webkit')){
   const lease=await page.evaluate(()=>testState.artifact.largeMarkdown)
   const native=await promisify(execFile)('/usr/bin/python3',[join(root,'scripts/smoke-large-markdown-webkit.py'),new URL(lease.url,url).href,'--tables'],{env:environment,timeout:110000,maxBuffer:1000000})
   webkit=JSON.parse(native.stdout)
   assert.equal(webkit.wholeInputQuery,true);assert.equal(webkit.typedCells,true);assert.equal(webkit.cleanup,true);assert.equal(webkit.durableSqlNotes,true)
  }
  results.push({notesChecked,durableAiDrafts:notesChecked,format:file.format,rows:info.rows,sheets:owner.document.tables,typedCells:true,completeQueryAndChart:true,opening:{indexMs:opening.indexMs,indexPeakRssKiB:opening.indexPeakRssKiB,readerFirstUsefulMs:opening.firstUsefulContentMs,maxOpeningHeartbeatGapMs:opening.maxHeartbeatGapMs,hostFirstRowsMs},webkit})
  await page.evaluate(()=>controller.close({restoreMap:false}))
  await page.waitForFunction(()=>client.active===0)
 }
 const closeDeadline=Date.now()+5000
 while((await readdir(environment.READER_CACHE_DIR)).length && Date.now()<closeDeadline)await pause(30)
 assert.deepEqual(await readdir(environment.READER_CACHE_DIR),[])
 for(const [index,file]of files.entries())assert.deepEqual(await readFile(file.path),originals[index])
 assert.deepEqual(errors,[])
 console.log(JSON.stringify({ok:true,workspace:true,gateway:true,originalsRestored:true,results}))
}finally{
 await browser?.close()
 if(server.exitCode==null){server.kill('SIGTERM');await Promise.race([once(server,'exit'),pause(3000)]);if(server.exitCode==null)server.kill('SIGKILL')}
 await rm(directory,{recursive:true,force:true})
}
