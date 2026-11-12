// Synthetic real-shell geometry regression. DOM reads are QA measurements only;
// production adapters use the public protocol and never read child internals.
import assert from 'node:assert/strict'
import {createHash} from 'node:crypto'
import {spawn,execFile} from 'node:child_process'
import {promisify} from 'node:util'
import {mkdir,mkdtemp,readFile,writeFile,readdir} from 'node:fs/promises'
import {join} from 'node:path'
import {fileURLToPath} from 'node:url'
const root=fileURLToPath(new URL('../',import.meta.url)),output=join(root,'target/reader-layout-smoke')
await mkdir(output,{recursive:true})
const dir=await mkdtemp(join(output,'run-')),project=join(dir,'project'),cache=join(dir,'reader-cache')
await mkdir(project);await mkdir(cache)
const file=join(project,'layout.md'),text=Array.from({length:24},(_,section)=>`# Section ${section}\n\n`+('A paragraph for reading layout. '.repeat(20)+'\n\n').repeat(3)+'| Topic | Kind | Question |\n| --- | --- | --- |\n'+Array.from({length:14},(_,i)=>`| Topic ${i} | Language | Example question ${i} |`).join('\n')+'\n\n'+('Text after the table must not overlap it. '.repeat(12)+'\n\n').repeat(5)).join('\n')
await writeFile(file,text)
const env={PATH:'/usr/bin:/bin',LANG:'C.UTF-8',CODEX_HOME:join(dir,'codex'),XDG_CONFIG_HOME:join(dir,'config'),XDG_DATA_HOME:join(dir,'data'),XDG_CACHE_HOME:join(dir,'cache'),READER_CACHE_DIR:cache,CODEX_THREAD_STUDIO_CODEX_BIN:join(root,'scripts/fixtures/reader-comments-codex.py'),CODEX_THREAD_STUDIO_CODEX_CONNECTION:'embedded',CODEX_THREAD_STUDIO_OPENCODE_BIN:'/intentionally-unavailable-opencode',STUDIO_SYNC_FIXTURE_PROJECT:project}
const server=spawn(process.env.STUDIO_REMOTE_BINARY||join(root,'target/release/codex-thread-studio'),['--serve','--listen','127.0.0.1:0'],{env,stdio:['ignore','pipe','pipe']})
let log='',browser;server.stdout.on('data',data=>log+=data);server.stderr.on('data',()=>{})
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms)),report={cases:[],dir}
try {
 let found;const deadline=Date.now()+30000
 while(!(found=log.match(/http:\/\/127\.0\.0\.1:\d+\/#token=[a-f0-9]{32}/))){assert.ok(Date.now()<deadline&&server.exitCode===null,'Server startup failed');await pause(30)}
 const url=found[0],{chromium}=await import(process.env.STUDIO_PLAYWRIGHT_MODULE||'playwright')
 browser=await chromium.launch({executablePath:process.env.STUDIO_CHROMIUM_PATH})
 const page=await browser.newPage({viewport:{width:1800,height:1100}}),errors=[]
 page.on('pageerror',error=>errors.push(error.message))
 await page.goto(url)
 await page.locator('.thread-row[data-thread-id="00000000-0000-4000-8000-000000000777"]').click()
 await page.waitForFunction(()=>document.querySelector('#thread-title').textContent.includes('Document integration fixture'))
 assert.equal(await (await page.request.get(new URL('/large-markdown-host.mjs',url).href)).text(),await readFile(join(root,'ui/large-markdown-host.mjs'),'utf8'),'Binary must embed the actual adapter')
 let frame
 const open=async()=>{
  await page.evaluate(({root,path})=>__studioDeveloper.openArtifact({root,path}),{root:project,path:file})
  await page.waitForFunction(()=>!document.querySelector('#artifact-preview').disabled)
  frame=await page.locator('.artifact-large-markdown-frame').elementHandle().then(node=>node.contentFrame())
  const state=await page.evaluate(async()=>{const {createReaderClient}=await import('/reader-assets/host-client.mjs');const client=createReaderClient(document.querySelector('.artifact-large-markdown-frame'));await client.ready();const state=await client.request('state');client.disconnect();return state})
  assert.ok(state.document.projectionPages>50)
  assert.equal(state.presentation.notes,'host');assert.equal(state.presentation.selectionMenu,'host')
  report.projectionPages=state.document.projectionPages
  const asset=await (await page.request.get(new URL('reader.mjs',frame.url()).href)).text()
  assert.equal(asset,await (await page.request.get(new URL('/reader-assets/reader.mjs',url).href)).text(),'Both Studio asset routes must serve the same compatibility layer')
  assert.match(asset,/function remeasureMountedPages\(\)/)
  report.derivedAssetSha256=createHash('sha256').update(asset).digest('hex')
 }
 const sample=async(name)=>{
  let geometry,stable=0;const until=Date.now()+5000
  while(Date.now()<until){
   await pause(100)
   geometry=await frame.evaluate(()=>{
    const nodes=[...document.querySelectorAll('#space > article[data-ready="true"]')].map(node=>{const rect=node.getBoundingClientRect();return {index:Number(node.dataset.index),top:rect.top,bottom:rect.bottom}}).sort((a,b)=>a.index-b.index)
    return {mounted:document.querySelectorAll('#space > article').length,dom:document.querySelectorAll('#space *').length,gaps:nodes.slice(1).flatMap((node,i)=>node.index===nodes[i].index+1?[node.top-nodes[i].bottom]:[])}
   })
   if(geometry.gaps.length&&geometry.gaps.every(gap=>Math.abs(gap)<1)){if(++stable===3)break}else stable=0
  }
  assert.equal(stable,3,`${name}: adjacent projections overlap or leave a gap: ${JSON.stringify(geometry)}`)
  assert.ok(geometry.mounted<=30&&geometry.mounted<report.projectionPages/2,'Projection DOM must remain windowed')
  assert.ok(geometry.dom<15000,'Projection DOM budget exceeded')
  report.cases.push({name,...geometry})
 }
 const scroll=async(name)=>{for(const offset of [0,400,1200,2400,5000,0]){await frame.evaluate(y=>{document.querySelector('#viewport').scrollTop=y},offset);await sample(`${name}-${offset}`)}}
 const settings=async(theme,locale,typography)=>{
  await page.evaluate(()=>__studioDeveloper.click('#settings-button'))
  await page.locator('[data-settings-pane=general]').click()
  await page.locator('#theme-select').selectOption(theme);await page.locator('#language-select').selectOption(locale)
  if(typography){await page.locator('[data-settings-pane=typography]').click();await page.locator('#content-font-size').fill(String(typography.content));await page.locator('#code-font-size').fill(String(typography.code));await page.locator('#reader-automatic-code-size').setChecked(typography.auto)}
  await page.locator('#settings-form').evaluate(form=>form.requestSubmit());await page.locator('#settings-dialog').waitFor({state:'hidden'})
 }
 await open();await scroll('initial')
 await settings('dark','en-US');await scroll('theme')
 await settings('dark','zh-CN');await scroll('locale')
 await settings('dark','zh-CN',{content:15,code:14,auto:true});await scroll('code-auto')
 await settings('dark','zh-CN',{content:15,code:14,auto:false});await scroll('code-only')
 await settings('dark','zh-CN',{content:18,code:24,auto:false});await scroll('actual-font')
 for(const width of [1200,2100,1800]){await page.setViewportSize({width,height:1100});await scroll(`resize-${width}`)}
 await page.locator('#close-artifact').click();await open();await scroll('reopen')
 await page.locator('#close-artifact').click()
 if(process.argv.includes('--webkit')){
  const result=await promisify(execFile)('/usr/bin/python3',[join(root,'scripts/smoke-reader-shell-webkit.py'),url,JSON.stringify({project,cases:[['layout',file]],output:dir,layout:true})],{timeout:175000,maxBuffer:1000000})
  report.webkit=JSON.parse(result.stdout);assert.equal(report.webkit.ok,true)
 }
 const until=Date.now()+10000;while((await readdir(cache)).length&&Date.now()<until)await pause(30)
 assert.deepEqual(await readdir(cache),[]);assert.equal(await readFile(file,'utf8'),text);assert.deepEqual(errors,[])
 report.status='passed';report.sourceUnchanged=true;report.ownedCacheCleaned=true
}catch(error){report.status='failed';report.error=String(error);process.exitCode=1}
finally{
 await browser?.close();server.kill('SIGTERM');await pause(300);if(server.exitCode===null)server.kill('SIGKILL')
 await writeFile(join(dir,'report.json'),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(report))
}
