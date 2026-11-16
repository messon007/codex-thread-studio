// End-to-end cold resume via the production shared WebSocket connector.
// Synthetic >64 MiB history only; no real sessions, daemon or inference calls.
import assert from 'node:assert/strict'
import {spawn,execFile} from 'node:child_process'
import {promisify} from 'node:util'
import {mkdtemp,mkdir,readFile,writeFile} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join} from 'node:path'
import {fileURLToPath} from 'node:url'
const root=fileURLToPath(new URL('../',import.meta.url)),dir=await mkdtemp(join(tmpdir(),'studio-large-history-')),project=join(dir,'project')
await mkdir(project)
const env={PATH:'/usr/bin:/bin',LANG:'C.UTF-8',CODEX_HOME:join(dir,'codex'),XDG_CONFIG_HOME:join(dir,'config'),XDG_DATA_HOME:join(dir,'data'),XDG_CACHE_HOME:join(dir,'cache'),READER_CACHE_DIR:join(dir,'reader-cache'),CODEX_THREAD_STUDIO_CODEX_BIN:join(root,'scripts/fixtures/large-history-codex.py'),CODEX_THREAD_STUDIO_CODEX_CONNECTION:'shared',CODEX_THREAD_STUDIO_OPENCODE_BIN:'/intentionally-unavailable-opencode',STUDIO_SYNC_FIXTURE_PROJECT:project}
const binary=process.env.STUDIO_REMOTE_BINARY||join(root,'target/release/codex-thread-studio')
const server=spawn(binary,['--serve','--listen','127.0.0.1:0'],{cwd:root,env,stdio:['ignore','pipe','pipe']})
const pause=ms=>new Promise(r=>setTimeout(r,ms)),report={dir,binary},baseline=process.env.STUDIO_EXPECT_OVERSIZE==='1'
let log='',browser,page;server.stdout.on('data',data=>log+=data);server.stderr.on('data',()=>{})
try{
 let url;const until=Date.now()+30000
 while(!(url=log.match(/http:\/\/127\.0\.0\.1:\d+\/#token=[a-f0-9]{32}/)?.[0])){assert.ok(Date.now()<until&&server.exitCode===null,'server startup');await pause(50)}
 const {chromium}=await import(process.env.STUDIO_PLAYWRIGHT_MODULE||'playwright')
 browser=await chromium.launch({executablePath:process.env.STUDIO_CHROMIUM_PATH})
 page=await browser.newPage({viewport:{width:1600,height:1000}})
 const errors=[],stops=[]
 report.errors=errors;report.stops=stops;report.lifecycle=[]
 page.on('console',message=>{const text=message.text();if(text.includes('[session.lifecycle]'))report.lifecycle.push(text)})
 page.setDefaultTimeout(20000)
 page.on('pageerror',error=>errors.push(error.message))
 page.on('websocket',socket=>socket.on('framereceived',event=>{
  try{const v=JSON.parse(event.payload);if(v.method==='studio/appServer/status'&&v.params?.state==='stopped')stops.push(v.params.reason)}catch{}
 }))
 await page.addInitScript(()=>{window.__historyQaLongTasks=[];new PerformanceObserver(list=>window.__historyQaLongTasks.push(...list.getEntries().map(x=>x.duration))).observe({entryTypes:['longtask']})})
 await page.goto(url)
 const start=Date.now()
 await page.locator('.thread-row[data-thread-id="00000000-0000-4000-8000-000000000777"]').click()
 if(baseline){
  const until=Date.now()+20000;while(!stops.length&&Date.now()<until)await pause(100)
  assert.ok(stops.length,'old binary must reproduce the transport failure')
  report.baselineFailure=stops[0]
 }else{
  assert.equal(await (await page.request.get(new URL('/codex-history-loader.mjs',url).href)).text(),await readFile(join(root,'ui/codex-history-loader.mjs'),'utf8'))
  await page.waitForFunction(()=>document.querySelectorAll('#turn-navigator-list [data-turn-nav-id]').length===402,null,{timeout:60000})
  report.coldResumeMs=Date.now()-start
  assert.deepEqual(await page.locator('#turn-navigator-list [data-turn-nav-id]').evaluateAll(nodes=>nodes.map(n=>n.dataset.turnNavId)),Array.from({length:402},(_,i)=>`turn-${i+1}`))
  for(const id of [1,402]){
   await page.locator(`[data-turn-nav-id="turn-${id}"]`).click()
   await page.waitForFunction(i=>document.querySelector(`.turn[data-turn-id="turn-${i}"]`)?.textContent.includes(`Fixture answer ${i}`),id)
  }
  // A browser refresh removes the in-memory prefix cache: cold paging must work again.
  report.navigation='first and last turns verified'
  await page.reload()
  await page.locator('.thread-row[data-thread-id="00000000-0000-4000-8000-000000000777"]').click()
  await page.waitForFunction(()=>document.querySelectorAll('#turn-navigator-list [data-turn-nav-id]').length===402,null,{timeout:60000})
  assert.deepEqual(stops,[]);assert.deepEqual(errors,[])
  report.turns=402
  report.longTasks=await page.evaluate(()=>({count:window.__historyQaLongTasks.length,maxMs:Math.max(0,...window.__historyQaLongTasks),heapBytes:performance.memory?.usedJSHeapSize}))
 }
 if(!baseline&&process.argv.includes('--webkit')){
  report.webkit=JSON.parse((await promisify(execFile)('/usr/bin/python3',[join(root,'scripts/smoke-codex-large-history-webkit.py'),url],{timeout:100000})).stdout)
  assert.equal(report.webkit.ok,true)
 }
 const records=(await readFile(join(project,'rpc.jsonl'),'utf8')).trim().split('\n').map(JSON.parse)
 const history=records.filter(r=>r.params?.threadId==='00000000-0000-4000-8000-000000000777'&&['thread/resume','thread/read','thread/turns/list'].includes(r.method))
 report.maxResponseBytes=Math.max(...history.map(r=>r.bytes));report.totalResponseBytes=history.reduce((n,r)=>n+r.bytes,0)
 if(baseline)assert.ok(report.maxResponseBytes>64*1024*1024)
 else{
  assert.ok(report.totalResponseBytes>2*64*1024*1024)
  assert.ok(report.maxResponseBytes<1024*1024)
  assert.ok(history.filter(r=>r.method==='thread/resume').every(r=>r.params.excludeTurns===true))
  assert.ok(history.filter(r=>r.method==='thread/read').every(r=>r.params.includeTurns!==true))
  assert.ok(history.filter(r=>r.method==='thread/turns/list').length>=200)
 }
 report.status='passed'
}catch(error){report.status='failed';report.error=String(error);report.page=await page?.evaluate(()=>({title:document.querySelector('#thread-title')?.textContent,connection:document.querySelector('#native-connection')?.textContent,errors:[...document.querySelectorAll('[id*=error]')].filter(n=>n.textContent?.trim()).map(n=>[n.id,n.textContent.slice(0,300)]),rows:[...document.querySelectorAll('.thread-row')].map(n=>n.dataset.threadId),nav:document.querySelectorAll('[data-turn-nav-id]').length})).catch(()=>null);process.exitCode=1}
finally{
 await browser?.close();server.kill('SIGTERM');await pause(300);if(server.exitCode===null)server.kill('SIGKILL')
 await writeFile(join(dir,'report.json'),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(report))
}
