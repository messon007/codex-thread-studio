// Actual Studio comment persistence and case search with an offline, non-inference backend.
import assert from 'node:assert/strict'
import {spawn} from 'node:child_process'
import {mkdir,writeFile,readFile,mkdtemp,readdir} from 'node:fs/promises'
import {join} from 'node:path'
import {fileURLToPath} from 'node:url'
const {chromium}=await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
const repo=fileURLToPath(new URL('../',import.meta.url)),fixture=join(repo,'scripts/fixtures/reader-comments-codex.py')
const binary=process.env.STUDIO_REMOTE_BINARY || join(repo,'target/release/codex-thread-studio')
const base=join(repo,'target/reader-comments-smoke');await mkdir(base,{recursive:true})
const dir=await mkdtemp(join(base,'run-')),project=join(dir,'project');await mkdir(project)
const path=join(project,'document.md')
await writeFile(path,'# Host integration\n\nRepeated TARGET 中文 😀 word needle. Repeated TARGET 中文 😀 word needle.\n\nNeedle needle NEEDLE K k K\n')
const originalBytes=await readFile(path)
const env={...process.env,PATH:'/usr/bin:/bin',LANG:'C.UTF-8',CODEX_HOME:join(dir,'codex'),XDG_CONFIG_HOME:join(dir,'config'),XDG_DATA_HOME:join(dir,'data'),XDG_CACHE_HOME:join(dir,'cache'),READER_CACHE_DIR:join(dir,'reader-cache'),CODEX_THREAD_STUDIO_CODEX_BIN:fixture,CODEX_THREAD_STUDIO_CODEX_CONNECTION:'embedded',CODEX_THREAD_STUDIO_OPENCODE_BIN:'/intentionally-unavailable-opencode',STUDIO_SYNC_FIXTURE_PROJECT:project}
const server=spawn(binary,['--serve','--listen','127.0.0.1:0'],{cwd:repo,env,stdio:['ignore','pipe','pipe']})
let logs='',browser,report={dir,checks:[]};server.stdout.on('data',x=>logs+=x);server.stderr.on('data',()=>{})
const pause=ms=>new Promise(r=>setTimeout(r,ms))
try{
 const deadline=Date.now()+30000;let found
 while(!(found=logs.match(/http:\/\/127\.0\.0\.1:\d+\/#token=[a-f0-9]{32}/))){if(Date.now()>deadline||server.exitCode!=null)throw Error('Fixture gateway startup failed');await pause(40)}
 const url=found[0];browser=await chromium.launch({executablePath:process.env.STUDIO_CHROMIUM_PATH})
 const page=await browser.newPage({viewport:{width:1500,height:1000}}),errors=[],leases=new Set();
 page.on('response',async response=>{if(new URL(response.url()).pathname==='/studio/reader/open'&&response.ok()){const opened=await response.json();if(opened.reader?.id)leases.add(opened.reader.id)}});
 page.on('pageerror',e=>{errors.push(e.message);report.pageErrors=[...errors]});page.on('requestfailed',r=>{(report.failedRequests||=[]).push({path:new URL(r.url()).pathname,error:r.failure()?.errorText})});page.on('response',r=>{if(r.status()>=400&&!new URL(r.url()).pathname.startsWith('/studio/session-map/')&&!['/opencode/global/event','/studio/opencode'].includes(new URL(r.url()).pathname))(report.failedResponses||=[]).push({path:new URL(r.url()).pathname,status:r.status()})})
 await page.goto(url);await page.waitForFunction(()=>Boolean(window.__studioDeveloper))
 const row=page.locator('.thread-row[data-thread-id="00000000-0000-4000-8000-000000000777"]');await row.waitFor();await row.click()
 await page.waitForFunction(()=>document.querySelector('#thread-title').textContent.includes('Document integration fixture'))
 report.checks.push('actual Studio selects a thread through isolated fake app-server; no application replacement or state injection')
 await page.evaluate(({root,path})=>__studioDeveloper.openArtifact({root,path}),{root:project,path})
 await page.waitForFunction(()=>document.querySelector('.artifact-large-markdown-frame')&&!document.querySelector('#artifact-preview').disabled)
 const frame=page.frameLocator('.artifact-large-markdown-frame'),paragraph=frame.locator('p').filter({hasText:'Repeated TARGET'}).first()
 const raw=await page.request.get(new URL('/reader-comment-host.mjs',url).href);assert.equal(raw.status(),200);assert.equal(await raw.text(),await readFile(join(repo,'ui/reader-comment-host.mjs'),'utf8'))
 await page.evaluate(async()=>{const {createReaderClient}=await import('/reader-assets/host-client.mjs');window.hostReview=createReaderClient(document.querySelector('.artifact-large-markdown-frame'));await hostReview.ready()})
 const readerState=await page.evaluate(()=>hostReview.request('state'));assert.equal(readerState.capabilities.commentMarks.version,1);assert.equal(readerState.presentation.notes,'host');assert.equal(readerState.presentation.selectionMenu,'host');assert.equal(await page.locator('#release-thread').count(),0)
 const target=await paragraph.evaluate(p=>{const walker=document.createTreeWalker(p,NodeFilter.SHOW_TEXT),entries=[];let n,text='';while(n=walker.nextNode()){entries.push({n,start:text.length,end:text.length+n.length});text+=n.data}const start=text.indexOf('TARGET',text.indexOf('TARGET')+1),a=entries.find(e=>e.end>start),b=entries.find(e=>e.end>=start+6);const range=document.createRange();range.setStart(a.n,start-a.start);range.setEnd(b.n,start+6-b.start);const r=range.getBoundingClientRect();return{x:r.left+r.width/2,y:r.top+r.height/2}})
 const frameBox=await page.locator('.artifact-large-markdown-frame').boundingBox();await page.mouse.dblclick(frameBox.x+target.x,frameBox.y+target.y)
 await page.locator('#selection-comment').waitFor({state:'visible'})
 const selection=await page.evaluate(()=>hostReview.request('selection',{textOnly:true}));assert.equal(selection.quote,'TARGET');assert.equal(selection.textRange.version,1)
 await page.locator('#selection-comment').click();await page.locator('#annotation-dialog').waitFor({state:'visible'});await page.locator('#annotation-comment').fill('Persistent host comment fixture')
 const persisted=page.waitForResponse(r=>r.url().endsWith('/studio/session-state/annotations')&&r.request().method()==='PUT');await page.locator('#save-annotation').click();await page.locator('#annotation-dialog').waitFor({state:'hidden'});assert.equal((await persisted).status(),204)
 const token=new URL(url).hash.slice(7)
 const savedDrafts=async()=>{const saved=await page.request.get(new URL('/studio/session-state',url).href,{headers:{Authorization:'Bearer '+token}});assert.equal(saved.status(),200);const state=await saved.json();return state.annotationDrafts?.['codex:00000000-0000-4000-8000-000000000777']||[]}
 let drafts=await savedDrafts();assert.equal(drafts.length,1);assert.equal(drafts[0].note,'Persistent host comment fixture');assert.deepEqual(drafts[0].source.anchor.readerReference.textRange,selection.textRange)
 const commentId=drafts[0].id, exactDraft=structuredClone(drafts[0])
 report.checks.push('real double-click selects second repeated word; exact textRange survives gateway and SQLite save')
 const marks=()=>frame.locator('body').evaluate(()=>[...CSS.highlights.entries()].filter(([n])=>n.startsWith('marknoria-comments-')).flatMap(([,ranges])=>[...ranges].map(range=>{const p=range.startContainer.parentElement.closest('p');let preceding=null;if(p){const before=document.createRange();before.selectNodeContents(p);before.setEnd(range.startContainer,range.startOffset);preceding=before.toString().split('TARGET').length-1}const r=range.getBoundingClientRect();return{text:range.toString(),preceding,x:r.left+r.width/2,y:r.top+r.height/2}})))
 const waitMarks=async(count)=>{for(let i=0;i<100;i++){const current=await marks();if(current.length===count)return current;await pause(30)}assert.fail(`Expected ${count} comment ranges, got ${JSON.stringify(await marks())}`)}
 let visible=await waitMarks(1);assert.equal(visible[0].text,'TARGET');assert.equal(visible[0].preceding,1)
 assert.match(await frame.locator('style[data-comment-style]').innerText(),/text-decoration-style:dashed/)
 await frame.locator('#space h1').first().click();visible=await waitMarks(1)
 report.checks.push('only second occurrence has a dashed underline; clearing selection retains it')
 await page.mouse.move(frameBox.x+visible[0].x,frameBox.y+visible[0].y);await frame.locator('.comment-mark-popup').waitFor({state:'visible'})
 await frame.locator('.comment-mark-popup').getByRole('button',{name:'View comment',exact:true}).click();await page.locator('#annotation-dialog').waitFor({state:'visible'});assert.equal(await page.locator('#annotation-comment').inputValue(),'Persistent host comment fixture')
 await page.keyboard.press('Escape');report.checks.push('real hover action crosses iframe into Studio editor for the owned saved comment')
 await page.evaluate(()=>hostReview.disconnect());await page.reload();await page.waitForFunction(()=>Boolean(window.__studioDeveloper));await page.locator('.thread-row[data-thread-id="00000000-0000-4000-8000-000000000777"]').click();await page.waitForFunction(()=>document.querySelector('#thread-title').textContent.includes('Document integration fixture'))
 await page.evaluate(({root,path})=>__studioDeveloper.openArtifact({root,path}),{root:project,path});await page.waitForFunction(()=>document.querySelector('.artifact-large-markdown-frame')&&!document.querySelector('#artifact-preview').disabled)
 await page.evaluate(async()=>{const {createReaderClient}=await import('/reader-assets/host-client.mjs');window.hostReview=createReaderClient(document.querySelector('.artifact-large-markdown-frame'));await hostReview.ready()})
 drafts=await savedDrafts();assert.equal(drafts[0].id,commentId);assert.deepEqual(drafts[0].source.anchor.readerReference.textRange,selection.textRange);visible=await waitMarks(1);assert.equal(visible[0].preceding,1)
 report.checks.push('page reload and document reopen restore the same exact saved underline from SQLite')
 await page.locator('#artifact-search-toggle').click();await page.locator('#artifact-search-input').fill('needle')
 const searchCase=page.locator('#artifact-search-match-case');assert.equal(await searchCase.isChecked(),true)
 async function searchState(total,matchCase){const end=Date.now()+30000;let value;while(Date.now()<end){value=await page.evaluate(()=>hostReview.request('state'));if(value.search.total===total&&value.search.matchCase===matchCase&&value.search.current===0)return value;await pause(25)}throw Error('Search state timeout: '+JSON.stringify(value?.search))}
 const strict=await searchState(3,true);await searchCase.uncheck();const folded=await searchState(5,false);assert.notEqual(strict.search.searchId,folded.search.searchId)
 const stale=await page.evaluate(async id=>{try{await hostReview.request('searchActivate',{searchId:id,index:0});return null}catch(e){return e.code}},strict.search.searchId);assert.equal(stale,'SEARCH_ID')
 await page.locator('#artifact-search-input').fill('k');const kelvin=await searchState(3,false);assert.equal(kelvin.search.active.sourceLocation.endByte-kelvin.search.active.sourceLocation.startByte,3)
 await page.locator('#artifact-search-input').fill('');await pause(200);assert.equal(await searchCase.isChecked(),false)
 await page.locator('#artifact-search-toggle').click();await page.locator('#artifact-search-toggle').click();assert.equal(await searchCase.isChecked(),false)
 report.checks.push('host case toggle changes identity/counts, retains Kelvin original three-byte range and survives query clear/panel reopen')
 await page.setViewportSize({width:700,height:600});await pause(100);const layout=await page.locator('#artifact-search-panel').evaluate(e=>({width:e.clientWidth,scrollWidth:e.scrollWidth}));report.searchPanelNarrow=layout;assert.ok(layout.scrollWidth<=layout.width+1,'Search controls overflow narrow reader panel');await page.setViewportSize({width:1500,height:1000})
 await page.locator('#open-thread-comments').click();const removed=page.waitForResponse(r=>r.url().endsWith('/studio/session-state/annotations')&&r.request().method()==='PUT');await page.locator(`.annotation-card[data-draft-id="${commentId}"] .annotation-delete`).click();assert.equal((await removed).status(),204);assert.equal((await savedDrafts()).length,0)
 await page.evaluate(({root,path})=>__studioDeveloper.openArtifact({root,path}),{root:project,path});await page.waitForFunction(()=>!document.querySelector('#artifact-preview').disabled);await waitMarks(0)
 report.checks.push('deleting the actual Studio draft clears only its saved underline and persisted record')
 await page.evaluate(async()=>{hostReview.disconnect();const {createReaderClient}=await import('/reader-assets/host-client.mjs');window.hostReview=createReaderClient(document.querySelector('.artifact-large-markdown-frame'));await hostReview.ready()})
 await page.locator('#artifact-source').click();await page.waitForFunction(()=>document.querySelector('#artifact-source').classList.contains('active'))
 const line=frame.locator('#source-code-view .cm-line').filter({hasText:'Repeated TARGET'}).first()
 const sourcePoint=await line.evaluate(p=>{const walker=document.createTreeWalker(p,NodeFilter.SHOW_TEXT),entries=[];let n,text='';while(n=walker.nextNode()){entries.push({n,start:text.length,end:text.length+n.length});text+=n.data}const start=text.indexOf('TARGET',text.indexOf('TARGET')+1),a=entries.find(e=>e.end>start),b=entries.find(e=>e.end>=start+6),r=document.createRange();r.setStart(a.n,start-a.start);r.setEnd(b.n,start+6-b.start);const box=r.getBoundingClientRect();return{x:box.left+box.width/2,y:box.top+box.height/2}})
 const sourceFrame=await page.locator('.artifact-large-markdown-frame').boundingBox();await page.mouse.dblclick(sourceFrame.x+sourcePoint.x,sourceFrame.y+sourcePoint.y);await page.locator('#selection-comment').waitFor({state:'visible'})
 const sourceSelection=await page.evaluate(()=>hostReview.request('selection',{textOnly:true}));assert.equal(sourceSelection.quote,'TARGET');assert.equal(sourceSelection.location.kind,'source');assert.equal(sourceSelection.precision,'exact')
 const original=(await readFile(path)).toString(),secondAt=original.indexOf('TARGET',original.indexOf('TARGET')+1);assert.equal(sourceSelection.location.startByte,Buffer.byteLength(original.slice(0,secondAt)))
 await page.locator('#selection-comment').click();await page.locator('#annotation-comment').fill('Source byte comment');const sourceSaved=page.waitForResponse(r=>r.url().endsWith('/studio/session-state/annotations')&&r.request().method()==='PUT');await page.locator('#save-annotation').click();assert.equal((await sourceSaved).status(),204);await page.locator('#annotation-dialog').waitFor({state:'hidden'});await waitMarks(1)
 drafts=await savedDrafts();assert.equal(drafts.length,1);assert.deepEqual(drafts[0].source.anchor.readerReference.location,sourceSelection.location)
 await page.locator('#artifact-preview').click();await waitMarks(0);assert.equal((await savedDrafts()).length,1);await page.locator('#artifact-source').click();await waitMarks(1)
 report.checks.push('real CodeMirror double-click saves exact original Unicode byte range; source-owned underline restores across view changes without guessing Preview coordinates')
 await page.locator('#open-thread-comments').click();const sourceRemoved=page.waitForResponse(r=>r.url().endsWith('/studio/session-state/annotations')&&r.request().method()==='PUT');await page.locator('.annotation-delete').click();assert.equal((await sourceRemoved).status(),204)
 const overlap=Array.from({length:2},(_,i)=>({...structuredClone(exactDraft),id:'overlap-'+i,note:'Overlap '+i}))
 const seeded=await page.request.put(new URL('/studio/session-state/annotations',url).href,{headers:{Authorization:'Bearer '+token},data:{sessionKey:'codex:00000000-0000-4000-8000-000000000777',drafts:overlap,additional:''}});assert.equal(seeded.status(),204)
 await page.evaluate(()=>hostReview.disconnect());await page.reload();await page.waitForFunction(()=>Boolean(window.__studioDeveloper));await page.locator('.thread-row[data-thread-id="00000000-0000-4000-8000-000000000777"]').click();await page.waitForFunction(()=>document.querySelector('#thread-title').textContent.includes('Document integration fixture'));await page.evaluate(({root,path})=>__studioDeveloper.openArtifact({root,path}),{root:project,path});await page.waitForFunction(()=>document.querySelector('.artifact-large-markdown-frame')&&!document.querySelector('#artifact-preview').disabled)
 visible=await waitMarks(2);assert.ok(visible.every(x=>x.text==='TARGET'&&x.preceding===1));const overlapFrame=await page.locator('.artifact-large-markdown-frame').boundingBox();await page.mouse.move(overlapFrame.x+visible[0].x,overlapFrame.y+visible[0].y)
 await frame.locator('.comment-mark-popup').waitFor({state:'visible'});assert.equal(await frame.locator('.comment-mark-popup button').count(),2);for(const index of [0,1]){
  await page.mouse.move(0,0);await page.mouse.move(overlapFrame.x+visible[0].x,overlapFrame.y+visible[0].y)
  await frame.locator('.comment-mark-popup').waitFor({state:'visible'})
  await frame.locator('.comment-mark-popup button').nth(index).click();await page.locator('#annotation-dialog').waitFor({state:'visible'});assert.equal(await page.locator('#annotation-comment').inputValue(),'Overlap '+index);await page.keyboard.press('Escape')
 }
 await page.locator('#open-thread-comments').click();page.once('dialog',dialog=>dialog.accept());const cleared=page.waitForResponse(r=>r.url().endsWith('/studio/session-state/annotations')&&r.request().method()==='PUT');await page.locator('#clear-annotations').click();assert.equal((await cleared).status(),204);assert.equal((await savedDrafts()).length,0);await page.evaluate(({root,path})=>__studioDeveloper.openArtifact({root,path}),{root:project,path});await page.waitForFunction(()=>!document.querySelector('#artifact-preview').disabled);await waitMarks(0)
 report.checks.push('overlapping saved references expose two separate owned hover actions; actual Clear comments removes persisted records and every underline')

 await page.screenshot({path:join(dir,'final-reader.png')})
 await page.evaluate(()=>window.hostReview?.disconnect());await page.locator('#close-artifact').click()
 // Browser reload deliberately abandons its old lease until the bounded idle sweep.
 // Close only this fixture's observed gateway leases instead of waiting two minutes.
 for(const id of leases){const closed=await page.request.post(new URL('/studio/markdown/close',url).href,{headers:{Authorization:'Bearer '+token},data:{id}});assert.ok(closed.ok())}
 const cleanupDeadline=Date.now()+10000;let cached=[]
 do{cached=await readdir(join(dir,'reader-cache')).catch(error=>{if(error.code==='ENOENT')return [];throw error});if(!cached.length)break;await pause(30)}while(Date.now()<cleanupDeadline)
 assert.deepEqual(cached,[]);assert.deepEqual(await readFile(path),originalBytes);report.sourceUnchanged=true;report.ownedCacheCleaned=true
 assert.deepEqual(errors,[]);report.status='passed';console.log(JSON.stringify(report,null,2))
}catch(error){const page=browser?.contexts()[0]?.pages()[0];if(page){report.visible=await page.locator('body').innerText();await page.screenshot({path:join(dir,'failure.png')}).catch(()=>{});report.readyState=await page.evaluate(()=>document.readyState)}report.status='failed';report.error=String(error);report.stack=error.stack;console.log(JSON.stringify(report,null,2));process.exitCode=1}
finally{await writeFile(join(dir,'review.json'),JSON.stringify(report,null,2)+'\n');await browser?.close();server.kill('SIGTERM');await Promise.race([new Promise(r=>server.once('exit',r)),pause(3000)]);if(server.exitCode==null)server.kill('SIGKILL')}
