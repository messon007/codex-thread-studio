// Actual local links, host history and owned reader leases; isolated offline backend.
import assert from 'node:assert/strict'
import {spawn,execFile} from 'node:child_process'
import {promisify} from 'node:util'
import {mkdir,mkdtemp,readFile,writeFile,readdir} from 'node:fs/promises'
import {fileURLToPath} from 'node:url'
import {join} from 'node:path'
const {chromium}=await import(process.env.STUDIO_PLAYWRIGHT_MODULE||'playwright')
const repo=fileURLToPath(new URL('../',import.meta.url)),base=join(repo,'target/document-back-smoke');await mkdir(base,{recursive:true})
const dir=await mkdtemp(join(base,'run-')),project=join(dir,'project');await mkdir(project)
const a='# A document\n\n[Same document](#a-target)\n\n'+Array.from({length:160},(_,i)=>`Paragraph A ${i}.\n\n`).join('')+'## A target\n\n[Open B](B.md#b-target)\n\nEnd of A.\n'
const b='# B document\n\n'+Array.from({length:100},(_,i)=>`Paragraph B ${i}.\n\n`).join('')+'## B target\n\n[Open C](C.md)\n\n'+Array.from({length:100},(_,i)=>`Second part B ${i}.\n\n`).join('')+'## B second\n\n[Open C again](C.md)\n\nEnd of B.\n'
for(const [name,text] of [['A.md',a],['B.md',b],['C.md','# C document\n\n[Open D](D.md)\n'],['D.md','# D document\n\n[Return B](B.md#b-second)\n\n[Return A](A.md#a-target)\n']])await writeFile(join(project,name),text)
const env={...process.env,PATH:'/usr/bin:/bin',CODEX_HOME:join(dir,'codex'),XDG_CONFIG_HOME:join(dir,'config'),XDG_DATA_HOME:join(dir,'data'),XDG_CACHE_HOME:join(dir,'cache'),READER_CACHE_DIR:join(dir,'cache-reader'),CODEX_THREAD_STUDIO_CODEX_BIN:join(repo,'scripts/fixtures/reader-comments-codex.py'),CODEX_THREAD_STUDIO_CODEX_CONNECTION:'embedded',CODEX_THREAD_STUDIO_OPENCODE_BIN:'/intentionally-unavailable-opencode',STUDIO_SYNC_FIXTURE_PROJECT:project}
const server=spawn(process.env.STUDIO_REMOTE_BINARY||join(repo,'target/release/codex-thread-studio'),['--serve','--listen','127.0.0.1:0'],{cwd:repo,env,stdio:['ignore','pipe','pipe']});let logs='',browser;server.stdout.on('data',x=>logs+=x);server.stderr.on('data',()=>{})
const pause=ms=>new Promise(r=>setTimeout(r,ms)),report={dir,checks:[]}
try{
 let match;for(let i=0;!(match=logs.match(/http:\/\/127\.0\.0\.1:\d+\/#token=[a-f0-9]{32}/));i++){if(i>600)throw Error('Startup timeout');await pause(50)}
 const url=match[0];browser=await chromium.launch({executablePath:process.env.STUDIO_CHROMIUM_PATH});const page=await browser.newPage({viewport:{width:1500,height:1000}}),errors=[];page.on('pageerror',error=>errors.push(error.message))
 await page.goto(url);await page.locator('.thread-row[data-thread-id="00000000-0000-4000-8000-000000000777"]').click();await page.waitForFunction(()=>document.querySelector('#thread-title').textContent.includes('Document integration fixture'))
 const ready=async name=>{
  await page.waitForFunction(name=>document.querySelector('#artifact-title').textContent===name&&document.querySelector('.artifact-large-markdown-frame')&&!document.querySelector('.artifact-large-markdown-frame').hasAttribute('aria-hidden')&&!document.querySelector('#artifact-preview').disabled,name)
  await page.evaluate(async()=>{window.backQa?.disconnect();const {createReaderClient}=await import('/reader-assets/host-client.mjs');window.backQa=createReaderClient(document.querySelector('.artifact-large-markdown-frame'));await backQa.ready()})
  return page.frameLocator('.artifact-large-markdown-frame')
 }
 const state=()=>page.evaluate(()=>backQa.request('state')),back=()=>page.locator('#close-artifact').click()
 const label=async value=>assert.equal(await page.locator('#close-artifact').getAttribute('aria-label'),value)
 const link=async name=>{const target=page.frameLocator('.artifact-large-markdown-frame').getByRole('link',{name,exact:true});await target.scrollIntoViewIfNeeded();await pause(100);const before=(await state()).position;await target.click();return before}
 await page.locator('#open-workspace-files').click();await page.locator('.workspace-tree-row[data-path="A.md"]').click();await ready('A.md');await label('Back to Files')
 await link('Same document');await label('Back to Files')
 const positionA=await link('Open B');await ready('B.md');await label('Back to A.md');assert.ok((await state()).position.location.line>100)
 const positionB=await link('Open C');await ready('C.md');await label('Back to B.md')
 await back();await ready('B.md');assert.ok(Math.abs((await state()).position.location.line-positionB.location.line)<=3)
 await page.locator('#refresh-artifact').click();await ready('B.md');await label('Back to A.md')
 await back();await ready('A.md');assert.ok(Math.abs((await state()).position.location.line-positionA.location.line)<=3);await label('Back to Files');await back();await page.locator('.workspace-tree-row[data-path="A.md"]').waitFor({state:'visible'})
 report.checks.push('Files A → B#fragment → C returns through B and A, restores public line positions, survives reload, then returns to Files; same-file anchor adds no entry')
 await page.locator('.workspace-tree-row[data-path="A.md"]').click();await ready('A.md');await page.evaluate(()=>backQa.request('navigateFragment',{fragment:'a-target'}))
 const visits=[];let current='A.md'
 for(const [action,next] of [['Open B','B.md'],['Open C','C.md'],['Open D','D.md'],['Return B','B.md'],['Open C again','C.md'],['Open D','D.md'],['Return A','A.md']]){
  visits.push({name:current,position:await link(action)});await ready(next);current=next
 }
 const expected=visits.slice(-4).reverse();assert.deepEqual(expected.map(v=>v.name),['D.md','C.md','B.md','D.md'])
 for(const visit of expected){await label('Back to '+visit.name);await back();await ready(visit.name);assert.ok(Math.abs((await state()).position.location.line-visit.position.location.line)<=3)}
 await label('Back to Files');await back();await page.locator('.workspace-tree-row[data-path="A.md"]').waitFor({state:'visible'})
 report.checks.push('A → B → C → D → B → C → D → A keeps exactly four previous visits, with independent repeated-file positions: Back returns D, C, B, D, then Files')

 await page.locator('.workspace-tree-row[data-path="A.md"]').click();await ready('A.md');await page.evaluate(()=>backQa.request('navigateFragment',{fragment:'a-target'}));await link('Open B');const frame=await ready('B.md')
 await page.locator('#artifact-edit').click();const editor=frame.locator('#editor-mount .cm-content');await editor.waitFor();await editor.click();await page.keyboard.press('Control+End');await page.keyboard.type('\nUnsaved back regression');await page.waitForFunction(()=>!document.querySelector('#artifact-save').disabled)
 page.once('dialog',dialog=>dialog.dismiss());await back();assert.match(await page.locator('#artifact-title').textContent(),/^B.md/);await label('Back to A.md')
 page.once('dialog',dialog=>dialog.accept());await back();await ready('A.md');assert.equal(await readFile(join(project,'B.md'),'utf8'),b)
 report.checks.push('Back from dirty editor prompts; cancellation retains document/history, confirmation discards only unsaved fixture changes')
 await link('Open B');await ready('B.md');await writeFile(join(project,'A.md'),'# Updated A\n\nChanged while away.\n');await back();await ready('A.md');assert.equal(await page.locator('#artifact-error').isVisible(),false);assert.equal((await state()).position.location.line,1);await writeFile(join(project,'A.md'),a)
 report.checks.push('return reopens through normal grants; changed revision loads safely without restoring obsolete line coordinates')
 await page.evaluate(({project})=>__studioDeveloper.openArtifact({root:project,path:project+'/A.md'}),{project})
 await ready('A.md');await label('Close document');await page.evaluate(()=>backQa.request('navigateFragment',{fragment:'a-target'}));await link('Open B');await ready('B.md');await label('Back to A.md')
 await page.evaluate(({project})=>__studioDeveloper.openArtifact({root:project,path:project+'/C.md'}),{project});await ready('C.md');await label('Close document');await back()
 report.checks.push('unrelated document opens reset link history; direct entry retains Close document')
 await page.evaluate(()=>window.backQa?.disconnect())
 if(process.argv.includes('--webkit')){
  const native=await promisify(execFile)('/usr/bin/python3',[join(repo,'scripts/smoke-document-back-webkit.py'),url,JSON.stringify({project})],{timeout:100000,maxBuffer:1000000});report.webkit=JSON.parse(native.stdout);assert.equal(report.webkit.ok,true)
 }
 let caches=[];for(let i=0;i<200;i++){caches=await readdir(join(dir,'cache-reader')).catch(()=>[]);if(!caches.length)break;await pause(50)}assert.deepEqual(caches,[]);assert.deepEqual(errors,[]);assert.equal(await readFile(join(project,'A.md'),'utf8'),a);assert.equal(await readFile(join(project,'B.md'),'utf8'),b);report.status='passed';report.ownedCacheCleaned=true
}catch(error){report.status='failed';report.error=String(error).replace(/#token=[a-f0-9]+/g,'#token=[redacted]');report.stack=String(error.stack).replace(/#token=[a-f0-9]+/g,'#token=[redacted]');process.exitCode=1}
finally{await browser?.close();server.kill('SIGTERM');await writeFile(join(dir,'report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report,null,2))}
