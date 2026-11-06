// Real Studio selection/search regression; no AI session, isolated backend configs.
import assert from 'node:assert/strict'
import {spawn} from 'node:child_process'
import {mkdir,writeFile,readFile,mkdtemp} from 'node:fs/promises'
import {join} from 'node:path'
import {fileURLToPath} from 'node:url'
const {chromium}=await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
const repo=fileURLToPath(new URL('../',import.meta.url)),base=join(repo,'target/reader-selection-smoke')
await mkdir(base,{recursive:true});const dir=await mkdtemp(join(base,'run-')),project=join(dir,'project');await mkdir(project)
const path=join(project,'document.md'),alternate=join(project,'other.md'),outside=join(dir,'outside.md')
await writeFile(path,'# Independent host review\n\nSTUDIO_COPY_SENTINEL hello 中文 😀 selectable paragraph.\n\n## Search corpus\n\n'+Array.from({length:130},()=>`中文😀ReviewNeedle`).join(' ')+'\n\n'+Array.from({length:120},(_,i)=>`ReviewNeedle paragraph ${i+1}.\n\n`).join(''))
await writeFile(alternate,'# Other owner\n\nOTHER_DOCUMENT_SENTINEL.\n');await writeFile(outside,'outside project')
const env={...process.env,PATH:'/usr/bin:/bin',LANG:'C.UTF-8',CODEX_HOME:join(dir,'codex'),XDG_CONFIG_HOME:join(dir,'config'),XDG_DATA_HOME:join(dir,'data'),XDG_CACHE_HOME:join(dir,'cache'),READER_CACHE_DIR:join(dir,'reader-cache'),CODEX_THREAD_STUDIO_CODEX_BIN:'/intentionally-unavailable-codex',CODEX_THREAD_STUDIO_OPENCODE_BIN:'/intentionally-unavailable-opencode'}
const binary=process.env.STUDIO_REMOTE_BINARY||join(repo,'target/release/codex-thread-studio')
const server=spawn(binary,['--serve','--listen','127.0.0.1:0'],{cwd:repo,env,stdio:['ignore','pipe','pipe']})
let logs='',browser,report={dir,checks:[]};server.stdout.on('data',x=>logs+=x);server.stderr.on('data',()=>{})
const pause=ms=>new Promise(r=>setTimeout(r,ms))
try{
 const deadline=Date.now()+30000;let found
 while(!(found=logs.match(/http:\/\/127\.0\.0\.1:\d+\/#token=[a-f0-9]{32}/))){if(Date.now()>deadline||server.exitCode!=null)throw Error('Isolated gateway startup failed');await pause(40)}
 const url=found[0];browser=await chromium.launch({executablePath:process.env.STUDIO_CHROMIUM_PATH})
 const context=await browser.newContext({viewport:{width:1500,height:1000},permissions:['clipboard-read','clipboard-write']});const page=await context.newPage(),errors=[]
 page.on('pageerror',error=>errors.push(error.message))
 await page.goto(url)
 await page.waitForFunction(()=>window.__studioDeveloper&&document.documentElement.style.getPropertyValue('--ui-font-size'))
 const raw=await page.request.get(new URL('/large-markdown-host.mjs',url).href)
 assert.equal(await raw.text(),await readFile(join(repo,'ui/large-markdown-host.mjs'),'utf8'),'stale embedded adapter')
 const translations=await page.request.get(new URL('/i18n.mjs',url).href);assert.equal(await translations.text(),await readFile(join(repo,'ui/i18n.mjs'),'utf8'),'stale translations');report.checks.push('actual unmodified app; embedded adapter and translations match candidate')
 const token=new URLSearchParams(new URL(url).hash.slice(1)).get('token')
 const postOpen=(body,headers={})=>page.request.post(new URL('/studio/reader/open',url).href,{data:body,headers})
 assert.equal((await postOpen({root:project,path})).status(),401)
 assert.equal((await postOpen({root:project,path},{Authorization:'Bearer '+token,Origin:'https://forbidden.invalid'})).status(),403)
 assert.equal((await postOpen({root:project,path:outside},{Authorization:'Bearer '+token})).status(),403)
 report.checks.push('reader open rejects missing token, foreign Origin and outside-root path')
 await page.evaluate(({root,path})=>__studioDeveloper.openArtifact({root,path}),{root:project,path})
 await page.locator('.artifact-large-markdown-frame').waitFor({state:'visible'})
 await page.waitForFunction(()=>!document.querySelector('#artifact-preview').disabled)
 const frame=page.frameLocator('.artifact-large-markdown-frame')
 await page.evaluate(async()=>{const {createReaderClient}=await import('/reader-assets/host-client.mjs');window.reviewClient=createReaderClient(document.querySelector('.artifact-large-markdown-frame'));await reviewClient.ready()})
 const state=await page.evaluate(()=>reviewClient.request('state'))
 assert.equal(state.presentation.notes,'host');assert.equal(state.presentation.selectionMenu,'host');assert.equal(state.presentation.toolbar,'host')
 assert.equal(await page.locator('#artifact-notes-tools button').count(),0,'unwanted reader Notes/favorite entry')
 report.checks.push('startup Notes/selection/toolbar ownership; no reader Notes/favorite entry')
 const readerToken=await page.locator('.artifact-large-markdown-frame').evaluate(f=>new URL(f.src).hash.slice(1))
 assert.equal((await page.request.get(new URL('/studio/backends',url).href,{headers:{Authorization:'Bearer '+readerToken}})).status(),401)
 report.checks.push('scoped reader token cannot authorize Studio gateway')
 const target=frame.locator('p').filter({hasText:'STUDIO_COPY_SENTINEL'}).first();await target.click({clickCount:3})
 try{await page.locator('#selection-popover').waitFor({state:'visible',timeout:6000})}
 catch(error){report.diagnostics=await page.evaluate(()=>{const p=document.querySelector('#selection-popover');return {popover:p.outerHTML,ancestors:[...function*(x){for(;x;x=x.parentElement)yield x}(p)].map(e=>({tag:e.tagName,id:e.id,classes:e.className,display:getComputedStyle(e).display}))}});await page.screenshot({path:join(dir,'selection-failure.png')});throw error}
 assert.equal(await page.locator('#selection-copy').isVisible(),true)
 assert.equal(await page.locator('#selection-comment').isVisible(),true)
 assert.equal(await page.locator('#selection-translate').isVisible(),true)
 assert.equal(await page.locator('#selection-favorite').isVisible(),false)
 assert.equal(await frame.locator('.notes-toolbar:visible').count(),0)
 await page.screenshot({path:join(dir,'studio-selection.png')})
 await page.locator('#selection-copy').click()
 const copied=await page.evaluate(()=>navigator.clipboard.readText());assert.match(copied,/STUDIO_COPY_SENTINEL hello 中文 😀 selectable paragraph\./)
 report.checks.push('real pointer selection; only Copy/Comment/Translate; clipboard exact Unicode text')
 await target.click({clickCount:3});await page.locator('#selection-popover').waitFor({state:'visible'});await page.locator('#selection-comment').click()
 await page.locator('#annotation-dialog').waitFor({state:'visible'});report.checks.push('Comment opens Studio annotation dialog')
 await page.keyboard.press('Escape')
 await target.click({clickCount:3});await page.locator('#selection-popover').waitFor({state:'visible'})
 await frame.locator('#space h1').first().click();await page.locator('#selection-popover').waitFor({state:'hidden'})
 report.checks.push('cleared child selection clears Studio menu')
 await target.click({clickCount:3});await page.locator('#selection-popover').waitFor({state:'visible'})
 await page.evaluate(({root,path})=>__studioDeveloper.openArtifact({root,path}),{root:project,path:alternate})
 await page.waitForFunction(()=>document.querySelector('#artifact-title').textContent.trim()==='other.md'&&!document.querySelector('#artifact-preview').disabled)
 await page.locator('#selection-popover').waitFor({state:'hidden'});report.checks.push('replacement owner cannot retain old selection/menu')
 await page.evaluate(()=>reviewClient.disconnect())
 await page.evaluate(({root,path})=>__studioDeveloper.openArtifact({root,path}),{root:project,path})
 await page.waitForFunction(()=>document.querySelector('#artifact-title').textContent.trim()==='document.md'&&!document.querySelector('#artifact-preview').disabled)
 await page.evaluate(async()=>{const {createReaderClient}=await import('/reader-assets/host-client.mjs');window.reviewClient=createReaderClient(document.querySelector('.artifact-large-markdown-frame'));await reviewClient.ready()})
 await page.locator('#artifact-search-toggle').click();await page.locator('#artifact-search-input').fill('ReviewNeedle')
 const summary=page.locator('#artifact-search-summary'),next=page.locator('#artifact-search-next'),prev=page.locator('#artifact-search-prev')
 async function current(index,total=250){
  await page.waitForFunction(({index,total})=>document.querySelector('#artifact-search-summary').textContent.trim()===`${index+1} / ${total}`,{index,total})
  await page.waitForFunction(async({index,total})=>{const s=await reviewClient.request('state');return s.search?.current===index&&s.search.total===total},{index,total})
  const s=await page.evaluate(()=>reviewClient.request('state'));assert.equal(s.search.active.index,index);assert.equal(s.search.totalExact,true)
  return s
 }
 await current(0);assert.doesNotMatch(await summary.textContent(),/\+|limited/i)
 const marker=async()=>frame.locator('body').evaluate(()=>{const ranges=CSS.highlights?.get('reader-search-current');return [...(ranges||[])].map(r=>({text:r.toString(),rectangles:r.getClientRects().length})).concat([...document.querySelectorAll('.reader-search-paint i.current')].map(e=>({text:null,rectangles:e.getClientRects().length})))})
 let painted=await marker();assert.ok(painted.length);assert.ok(painted.some(x=>x.text==='ReviewNeedle'||x.rectangles>0));report.checks.push('exact 250 total on first bounded window and painted first Preview occurrence')
 await prev.click();await current(249);await next.click();await current(0);report.checks.push('previous wraps to last result across all pages, next returns to first')
 const latency=[]
 for(let index=1;index<=101;index++){const started=performance.now();await next.click();await current(index);latency.push(performance.now()-started)}
 await prev.click();const boundary=await current(100)
 assert.equal(boundary.search.active.occurrence,100)
 const paintedOrdinal=await frame.locator('body').evaluate(()=>{const r=[...(CSS.highlights?.get('reader-search-current')||[])][0];if(!r)return null;const p=r.startContainer.parentElement.closest('p');if(!p)return null;const prefix=document.createRange();prefix.selectNodeContents(p);prefix.setEnd(r.startContainer,r.startOffset);return prefix.toString().split('ReviewNeedle').length-1})
 assert.equal(paintedOrdinal,100,'Preview current highlight must identify occurrence 100, not the first identical word');report.checks.push('Preview painted range identifies the exact same-line occurrence beyond window 100')
 report.checks.push('real host controls cross 99/100 in both directions with absolute same-line UTF-8 occurrence identity')
 const bytes=await readFile(path),text=bytes.toString(),needle='ReviewNeedle';let at=-1;for(let i=0;i<=100;i++)at=text.indexOf(needle,at+1)
 const expectedStart=Buffer.byteLength(text.slice(0,at))
 assert.equal(boundary.search.active.sourceLocation.startByte,expectedStart);assert.equal(boundary.search.active.sourceLocation.endByte,expectedStart+needle.length)
 await page.locator('#artifact-source').click();await page.waitForFunction(()=>document.querySelector('#artifact-source').classList.contains('active'));let source=await current(100)
 await next.click();source=await current(101);assert.equal(source.view,'source');assert.equal(source.search.active.occurrence,101)
 const sourceSelection=await page.evaluate(()=>reviewClient.request('selection',{textOnly:true}));assert.equal(sourceSelection.quote,'ReviewNeedle');assert.equal(sourceSelection.location.kind,'source');assert.equal(sourceSelection.location.startByte,source.search.active.sourceLocation.startByte);report.checks.push('Source preserves active search and selects exact verified UTF-8 range')
 await page.locator('#artifact-preview').click();await page.waitForFunction(()=>document.querySelector('#artifact-preview').classList.contains('active'));await current(101)
 await page.evaluate(()=>{for(let i=0;i<11;i++)document.querySelector('#artifact-search-next').click();for(let i=0;i<5;i++)document.querySelector('#artifact-search-prev').click()});await current(107);report.checks.push('rapid concurrent next/previous honors final requested ordinal without stale activation')
 await page.screenshot({path:join(dir,'search250-preview.png')})
 await page.locator('#artifact-search-input').fill('paragraph ');await current(0,120);report.checks.push('query replacement gets a new result set without stale occurrence')
 const oldId=(await page.evaluate(()=>reviewClient.request('state'))).search.searchId
 await page.locator('#artifact-search-input').fill('');await page.waitForFunction(async()=>{const s=await reviewClient.request('state');return s.search.searchId===null&&s.search.current===null&&s.search.total===0})
 assert.equal(await next.isDisabled(),true);assert.equal(await prev.isDisabled(),true);assert.equal((await marker()).length,0)
 const stale=await page.evaluate(async oldId=>{try{await reviewClient.request('searchActivate',{searchId:oldId,index:0});return null}catch(e){return e.code}},oldId);assert.equal(stale,'SEARCH_ID');report.checks.push('clearing query cancels state, removes highlights, disables navigation and invalidates old identity')
 await page.locator('#artifact-search-input').fill('ReviewNeedle');await page.locator('#artifact-search-input').fill('not-found');await page.waitForFunction(()=>/No matches found/.test(document.querySelector('#artifact-search-summary').textContent))
 assert.equal((await page.evaluate(()=>reviewClient.request('state'))).search.total,0)
 await page.locator('#artifact-search-input').fill('界'.repeat(86));await pause(500)
 assert.equal((await page.evaluate(()=>reviewClient.request('state'))).search.total,0);report.checks.push('rapid query replacement and oversized UTF-8 query do not restore earlier state')
 await page.locator('#artifact-search-input').fill(' ');await current(0,text.split(' ').length-1);report.checks.push('whitespace remains a literal searchable query')
 await page.locator('#artifact-search-input').fill('ReviewNeedle');await current(0)
 const invalid=await page.evaluate(async()=>{const out=[];for(const params of[{query:'ReviewNeedle',offset:'100'},{query:'ReviewNeedle',offset:251},{query:'ReviewNeedle',revision:'stale'}]){try{await reviewClient.request('search',params);out.push('success')}catch(e){out.push(e.code)}}return out});assert.deepEqual(invalid,['PARAMS','RANGE','REVISION'])
 await page.evaluate(()=>reviewClient.disconnect());await page.evaluate(({root,path})=>__studioDeveloper.openArtifact({root,path}),{root:project,path:alternate});await page.waitForFunction(()=>document.querySelector('#artifact-title').textContent.trim()==='other.md'&&!document.querySelector('#artifact-preview').disabled)
 report.checks.push('public contract rejects malformed offsets, out-of-range and stale revision; owner replacement completes')
 latency.sort((a,b)=>a-b);report.search={total:250,available:boundary.search.available,retainedOffset:boundary.search.offset,unicodeStartByte:expectedStart,stepMedianMs:Math.round(latency[Math.floor(latency.length*.5)]),stepP95Ms:Math.round(latency[Math.floor(latency.length*.95)]),pageErrors:errors}
 assert.deepEqual(errors,[])
 report.status='passed'
 console.log(JSON.stringify(report,null,2))
}catch(error){report.status='failed';report.error=String(error);console.log(JSON.stringify(report,null,2));process.exitCode=1}
finally{
 await writeFile(join(dir,'review.json'),JSON.stringify(report,null,2)+'\n')
 await browser?.close();server.kill('SIGTERM');await Promise.race([new Promise(r=>server.once('exit',r)),pause(3000)]);if(server.exitCode==null)server.kill('SIGKILL')
}
