// Real Files entry and first-wheel regression; no real sessions or inference.
import assert from 'node:assert/strict'
import {fileURLToPath} from 'node:url'
import {promisify} from 'node:util'
import {spawn,execFile} from 'node:child_process'
import {mkdir,mkdtemp,writeFile,readFile,readdir} from 'node:fs/promises'
import {join} from 'node:path'
const {chromium}=await import(process.env.STUDIO_PLAYWRIGHT_MODULE||'playwright')
const repo=fileURLToPath(new URL('../',import.meta.url)),output=join(repo,'target/reader-wheel-smoke');await mkdir(output,{recursive:true})
const dir=await mkdtemp(join(output,'run-')),project=join(dir,'project');await mkdir(project)
await writeFile(join(project,'scroll.md'),Array.from({length:250},(_,i)=>`## Section ${i}\n\nParagraph ${i} — mouse wheel scrolling should work immediately. `.repeat(4)+'\n\n').join(''))
const env={...process.env,PATH:'/usr/bin:/bin',CODEX_HOME:join(dir,'codex'),XDG_CONFIG_HOME:join(dir,'config'),XDG_DATA_HOME:join(dir,'data'),XDG_CACHE_HOME:join(dir,'cache'),READER_CACHE_DIR:join(dir,'reader-cache'),CODEX_THREAD_STUDIO_CODEX_BIN:repo+'/scripts/fixtures/reader-comments-codex.py',CODEX_THREAD_STUDIO_CODEX_CONNECTION:'embedded',CODEX_THREAD_STUDIO_OPENCODE_BIN:'/intentionally-unavailable-opencode',STUDIO_SYNC_FIXTURE_PROJECT:project}
const original=await readFile(join(project,'scroll.md'))
const server=spawn(process.env.STUDIO_REMOTE_BINARY||repo+'/target/release/codex-thread-studio',['--serve','--listen','127.0.0.1:0'],{cwd:repo,env,stdio:['ignore','pipe','pipe']});let logs='',browser,releaseIndex;server.stdout.on('data',x=>logs+=x);server.stderr.on('data',()=>{})
const pause=ms=>new Promise(r=>setTimeout(r,ms));let report={dir,project,cases:[]}
try{
 let match;for(let i=0;!(match=logs.match(/http:\/\/127\.0\.0\.1:\d+\/#token=[a-f0-9]{32}/));i++){if(i>600)throw Error('server timeout');await pause(50)}
 const url=match[0];browser=await chromium.launch({executablePath:process.env.STUDIO_CHROMIUM_PATH});const page=await browser.newPage({viewport:{width:1500,height:1000}})
 const errors=[];page.on('pageerror',error=>errors.push(error.message))
 let firstIndex=true;const indexGate=new Promise(resolve=>releaseIndex=resolve)
 await page.route('**/api/document**',async route=>{if(firstIndex){firstIndex=false;await indexGate}await route.continue()})
 await page.goto(url);await page.locator('.thread-row[data-thread-id="00000000-0000-4000-8000-000000000777"]').click();await page.waitForFunction(()=>document.querySelector('#thread-title')?.textContent.includes('Document integration fixture'))
 for(let i=0;i<5;i++){
  if(i===0)await page.locator('#open-workspace-files').click()
  await page.locator('.workspace-tree-row[data-path="scroll.md"]').click()
  if(i===0){
   const iframe=page.locator('.artifact-large-markdown-frame');await iframe.waitFor({state:'attached'})
   for(let wait=0;firstIndex&&wait<200;wait++)await pause(25);assert.equal(firstIndex,false,'Reader document response must be held during input-gate checks')
   const gate=await iframe.evaluate(frame=>({inert:frame.inert,tabIndex:frame.tabIndex,hidden:frame.getAttribute('aria-hidden'),pointerEvents:getComputedStyle(frame).pointerEvents,covered:[[8,8],[-8,8],[8,-8],[-8,-8]].every(([x,y])=>{const r=frame.getBoundingClientRect();return document.elementFromPoint(x>0?r.left+x:r.right+x,y>0?r.top+y:r.bottom+y)?.closest('#artifact-loading')!=null})}))
   assert.deepEqual(gate,{inert:false,tabIndex:-1,hidden:'true',pointerEvents:'auto',covered:true})
   for(let tab=0;tab<15;tab++){await page.keyboard.press('Tab');assert.equal(await iframe.evaluate(frame=>document.activeElement===frame),false)}
   report.initializationGate=gate;releaseIndex()
  }
  await page.waitForFunction(()=>document.querySelector('.artifact-large-markdown-frame')&&!document.querySelector('#artifact-preview').disabled)
  const el=page.locator('.artifact-large-markdown-frame'),frame=await el.contentFrame(),vp=frame.locator('#viewport');await vp.locator('.page').first().waitFor()
  const state=()=>vp.evaluate(e=>({top:e.scrollTop,height:e.clientHeight,total:e.scrollHeight,focused:document.activeElement?.id,inert:window.frameElement.inert}))
  const before=await state(),box=await vp.boundingBox();await page.mouse.move(box.x+box.width/2,box.y+box.height/2);await page.mouse.wheel(0,500);await pause(700)
  const after=await state();assert.ok(after.top>before.top,'First wheel must scroll without pointer down, focus or scrollbar manipulation')
  await page.mouse.wheel(0,-250);await pause(350);const up=await state();assert.ok(up.top<after.top,'Upward wheel must scroll')
  assert.equal(await el.evaluate(frame=>getComputedStyle(frame).pointerEvents),'auto');assert.equal(await el.getAttribute('tabindex'),'0');assert.equal(await el.getAttribute('aria-hidden'),null)
  report.cases.push({before,after,up,scrolled:after.top>before.top})
  await page.locator('#close-artifact').click()
 }
 if(process.argv.includes('--webkit')){
  const native=await promisify(execFile)('/usr/bin/python3',[join(repo,'scripts/smoke-reader-wheel-webkit.py'),url],{timeout:100000,maxBuffer:1000000})
  report.webkit=JSON.parse(native.stdout);assert.equal(report.webkit.ok,true)
 }
 const until=Date.now()+10000;let caches;do{caches=await readdir(join(dir,'reader-cache')).catch(()=>[]);if(!caches.length)break;await pause(50)}while(Date.now()<until)
 assert.deepEqual(caches,[]);assert.deepEqual(await readFile(join(project,'scroll.md')),original);assert.deepEqual(errors,[])
 report.status='passed';report.sourceUnchanged=true;report.ownedCacheCleaned=true
}catch(e){report.status='failed';report.error=String(e).replace(/#token=[a-f0-9]+/g,'#token=[redacted]');report.stack=String(e.stack).replace(/#token=[a-f0-9]+/g,'#token=[redacted]');process.exitCode=1}
finally{releaseIndex?.();await writeFile(join(dir,'report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report,null,2));await browser?.close();server.kill('SIGTERM')}
