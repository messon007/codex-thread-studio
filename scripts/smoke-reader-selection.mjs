// Real Studio menu regression; no AI session, backend executables disabled.
import assert from 'node:assert/strict'
import {spawn} from 'node:child_process'
import {mkdir,writeFile,readFile,mkdtemp} from 'node:fs/promises'
import {join} from 'node:path'
import {fileURLToPath} from 'node:url'
const {chromium}=await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
const repo=fileURLToPath(new URL('../',import.meta.url)),base=join(repo,'target/reader-selection-smoke')
await mkdir(base,{recursive:true});const dir=await mkdtemp(join(base,'run-')),project=join(dir,'project');await mkdir(project)
const path=join(project,'document.md'),alternate=join(project,'other.md')
await writeFile(path,'# Independent host review\n\nSTUDIO_COPY_SENTINEL hello 中文 😀 selectable paragraph.\n\n## Search corpus\n\n'+Array.from({length:150},(_,i)=>`ReviewNeedle line ${i+1}.\n\n`).join(''))
await writeFile(alternate,'# Other owner\n\nOTHER_DOCUMENT_SENTINEL.\n')
const env={...process.env,PATH:'/usr/bin:/bin',LANG:'C.UTF-8',XDG_CONFIG_HOME:join(dir,'config'),XDG_DATA_HOME:join(dir,'data'),XDG_CACHE_HOME:join(dir,'cache'),READER_CACHE_DIR:join(dir,'reader-cache'),CODEX_THREAD_STUDIO_CODEX_BIN:'/intentionally-unavailable-codex',CODEX_THREAD_STUDIO_OPENCODE_BIN:'/intentionally-unavailable-opencode'}
const binary=process.env.STUDIO_REMOTE_BINARY || join(repo,'target/debug/codex-thread-studio')
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
 report.checks.push('actual unmodified app; embedded adapter matches candidate')
 await page.evaluate(({root,path})=>__studioDeveloper.openArtifact({root,path}),{root:project,path})
 await page.locator('.artifact-large-markdown-frame').waitFor({state:'visible'})
 await page.waitForFunction(()=>!document.querySelector('#artifact-preview').disabled)
 const frame=page.frameLocator('.artifact-large-markdown-frame')
 await page.evaluate(async()=>{const {createReaderClient}=await import('/reader-assets/host-client.mjs');window.reviewClient=createReaderClient(document.querySelector('.artifact-large-markdown-frame'));await reviewClient.ready()})
 const state=await page.evaluate(()=>reviewClient.request('state'))
 assert.equal(state.presentation.notes,'host');assert.equal(state.presentation.selectionMenu,'host');assert.equal(state.presentation.toolbar,'host')
 assert.equal(await page.locator('#artifact-notes-tools button').count(),0,'unwanted reader Notes/favorite entry')
 report.checks.push('startup Notes/selection/toolbar ownership; no reader Notes/favorite entry')
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
 await page.locator('#artifact-search-toggle').click()
 await page.locator('#artifact-search-input').fill('ReviewNeedle')
 await page.waitForFunction(()=>document.querySelector('#artifact-search-summary').textContent.includes('100+'))
 assert.match(await page.locator('#artifact-search-summary').textContent(),/Navigation limited to first 100 results/)
 await page.locator('#artifact-search-next').click()
 assert.match(await page.locator('#artifact-search-summary').textContent(),/^2 \/ 100\+/)
 await page.locator('#artifact-source').click()
 await page.waitForFunction(async()=>(await reviewClient.request('state')).view==='source')
 await page.locator('#artifact-search-next').click()
 assert.match(await page.locator('#artifact-search-summary').textContent(),/^3 \/ 100\+/)
 await page.locator('#artifact-preview').click()
 await page.waitForFunction(async()=>(await reviewClient.request('state')).view==='preview')
 report.checks.push('host search reports capped results honestly and navigates in Preview/Source')
 await page.evaluate(()=>reviewClient.request('navigate',{location:{kind:'line',line:1}}))
 await target.click({clickCount:3});await page.locator('#selection-popover').waitFor({state:'visible'})
 await page.evaluate(({root,path})=>__studioDeveloper.openArtifact({root,path}),{root:project,path:alternate})
 await page.waitForFunction(()=>document.querySelector('#artifact-title').textContent.trim()==='other.md'&&!document.querySelector('#artifact-preview').disabled)
 await page.locator('#selection-popover').waitFor({state:'hidden'});report.checks.push('replacement owner cannot retain old selection/menu')
 assert.deepEqual(errors,[])
 report.status='passed'
 console.log(JSON.stringify(report,null,2))
}catch(error){report.status='failed';report.error=String(error);console.log(JSON.stringify(report,null,2));process.exitCode=1}
finally{
 await writeFile(join(dir,'review.json'),JSON.stringify(report,null,2)+'\n')
 await browser?.close();server.kill('SIGTERM');await Promise.race([new Promise(r=>server.once('exit',r)),pause(3000)]);if(server.exitCode==null)server.kill('SIGKILL')
}
