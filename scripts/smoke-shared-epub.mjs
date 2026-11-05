// Actual Studio workspace adoption over the fixed public EPUB reader.
import assert from 'node:assert/strict'
import { spawn, execFile, execFileSync } from 'node:child_process'
import { promisify } from 'node:util'
import { mkdir, mkdtemp, writeFile, readFile, readdir, rm } from 'node:fs/promises'
import { resolve, join } from 'node:path'
import { once } from 'node:events'
const root=resolve(new URL('../',import.meta.url).pathname)
const output=join(root,'target/shared-epub-smoke')
await mkdir(output,{recursive:true})
const directory=await mkdtemp(join(output,'run-')), workspace=join(directory,'workspace')
await mkdir(workspace)
const generated=JSON.parse(execFileSync('/usr/bin/python3',[join(root,'crates/marknoria/tests/browser/epub_fixtures.py'),workspace],{encoding:'utf8'}))
const file=generated.book,original=await readFile(file)
let digest=0xcbf29ce484222325n
for(const value of original)digest=BigInt.asUintN(64,(digest^BigInt(value))*0x100000001b3n)
const bookHash='fnv1a64:'+digest.toString(16).padStart(16,'0')
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
 assert.equal((await post({root:workspace,path:file})).status,401)
 assert.equal((await post({root:workspace,path:file},{Authorization:'Bearer '+token,Origin:'https://forbidden.invalid'})).status,403)
 const outside=join(directory,'outside.epub');await writeFile(outside,original)
 assert.equal((await post({root:workspace,path:outside},{Authorization:'Bearer '+token})).status,403)
 const {chromium}=await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
 browser=await chromium.launch({executablePath:process.env.STUDIO_CHROMIUM_PATH})
 const page=await browser.newPage({viewport:{width:1500,height:950}}),errors=[],requests=[]
 page.on('pageerror',error=>errors.push(error.message))
 page.on('request',request=>requests.push(new URL(request.url()).pathname))
 await page.route(`${url.origin}/app.js`,route=>route.fulfill({contentType:'text/javascript',body:`
  import {createDocumentWorkspaceController,createDocumentWorkspaceState} from '/document-workspace-controller.mjs';
  window.testState=createDocumentWorkspaceState();
  const gatewayFetch=(path,options={})=>fetch(path,{...options,headers:{...options.headers,Authorization:'Bearer '+window.__CODEX_THREAD_STUDIO_GATEWAY__.token}});
  window.controller=createDocumentWorkspaceController({state:testState,gatewayFetch,randomId:()=>crypto.randomUUID(),reportError:error=>{throw error},view:{
   selectedThread:()=>({cwd:${JSON.stringify(workspace)}}),selectedStateKey:()=>'epub-fixture',activateRightWorkspace:tool=>{testState.activeRightWorkspace=tool},
   hideComposerMenu(){},closeActionMenus(){},applyRightRailWidth(){},disconnectMarkdownImageObserver(){},disposeMarkdownImageAssets(){},hideSelection(){},
   setSelection(selection,rect){window.captured={selection,rect}},openCommentForSelection(selection){window.captured={selection}},renderMarkdownDocument(){throw Error('Legacy parser must not run')},openResources(){window.openRequests=(window.openRequests||0)+1}
  }});controller.bind();
 `}))
 await page.goto(url.href)
 await page.waitForFunction(()=>Boolean(window.controller))
 const started=Date.now()
 const open=async()=>page.evaluate(async({root,path})=>{
  await controller.open({root,path},{allowDetachedRoot:true})
  const {createReaderClient}=await import('/reader-assets/host-client.mjs')
  window.client=createReaderClient(document.querySelector('.artifact-large-markdown-frame'))
  await client.ready()
 },{root:workspace,path:file})
 const command=(method,params={})=>page.evaluate(({method,params})=>client.request(method,params),{method,params})
 await open()
 let frame=await page.locator('.artifact-large-markdown-frame').elementHandle().then(element=>element.contentFrame())
 await frame.locator('.epub-reader').waitFor()
 await page.waitForFunction(()=>testState.artifact.readingState?.cfi)
 const hostFirstBookMs=Date.now()-started,opening=(await command('state')).timings
 assert.equal(await page.evaluate(()=>testState.artifact.hash),bookHash)
 assert.equal((await command('state')).format,'epub')
 assert.equal((await command('state')).capabilities.edit,false)
 assert.equal(await page.locator('#artifact-source').isVisible(),false)
 assert.equal(await page.locator('#artifact-edit').isVisible(),false)
 const outline=await command('outline')
 assert.equal(outline.items.length,2)
 await command('navigate',{location:outline.items[1].location})
 await command('settings',{epub:{theme:'sepia',fontScale:1.25,flow:'paginated'}})
 const chapter=frame.frameLocator('iframe[title="Second chapter"]')
 await chapter.getByText('EPUB sentinel two 中文😀.',{exact:true}).waitFor()
 assert.equal(await chapter.locator('body').evaluate(element=>element.ownerDocument.defaultView.frameElement.sandbox.contains('allow-scripts')),false)
 assert.equal(await page.evaluate(()=>window.compromised===true),false)
 const searched=await command('search',{query:'EPUB sentinel'})
 assert.equal(searched.matches.length,2)
 await chapter.getByText('EPUB sentinel two 中文😀.',{exact:true}).click({clickCount:3})
 await page.locator('.notes-toolbar [data-action=ask-ai]').click()
 await page.waitForFunction(()=>window.captured?.selection?.source?.anchor?.readerReference?.location?.kind==='epub')
 const source=await page.evaluate(()=>captured.selection.source)
 assert.equal(source.provider,'epub')
 assert.equal(source.anchor.bookHash,bookHash)
 assert.equal(source.anchor.cfiRange,source.anchor.readerReference.location.cfi)
 const selected=await page.evaluate(()=>captured.selection.source.anchor.readerReference)
 assert.equal(selected.precision,'exact')
 assert.match(await page.evaluate(()=>captured.selection.quote),/EPUB sentinel two/)
 const saved=(await command('state')).formatState
 assert.equal(saved.theme,'sepia');assert.equal(saved.fontScale,1.25)
 const lease=await page.evaluate(()=>testState.artifact.largeMarkdown)
 await page.evaluate(()=>controller.close({restoreMap:false}))
 const persisted=async()=>{
  const response=await fetch(new URL('/studio/epub/state',url),{method:'POST',headers:{Authorization:'Bearer '+token,'Content-Type':'application/json'},body:JSON.stringify({root:workspace,path:file,bookHash})})
  return response.status===204?null:response.json()
 }
 let stored
 const savedDeadline=Date.now()+5000
 do{stored=await persisted();if(stored?.cfi===saved.cfi&&stored?.theme==='sepia')break;await pause(30)}while(Date.now()<savedDeadline)
 assert.equal(stored.cfi,saved.cfi);assert.equal(stored.theme,'sepia');assert.equal(stored.fontScale,1.25)
 await open()
 await page.waitForFunction(cfi=>testState.artifact.readerReady && !testState.artifact.saving && testState.artifact.readingState?.cfi===cfi,saved.cfi)
 frame=await page.locator('.artifact-large-markdown-frame').elementHandle().then(element=>element.contentFrame())
 await frame.frameLocator('iframe[title="Second chapter"]').getByText('EPUB sentinel two 中文😀.',{exact:true}).waitFor()
 const beforeReopen=(await command('state')).formatState
 await page.evaluate(source=>controller.reopenEpubSource(source.anchor),source)
 const reopened=(await command('state')).formatState
 if(reopened.cfi!==saved.cfi) console.error(JSON.stringify({stage:'EPUB-reopen',saved:saved.cfi,before:beforeReopen.cfi,source:source.anchor.cfiRange,selected:selected.location.cfi,after:reopened.cfi,parent:await page.evaluate(()=>({cfi:testState.artifact.readingState?.cfi,ready:testState.artifact.readerReady,busy:testState.artifact.saving}))}))
 assert.equal(reopened.cfi,saved.cfi);assert.equal(reopened.theme,'sepia');assert.equal(reopened.fontScale,1.25)
 assert.notEqual((await page.evaluate(()=>testState.artifact.largeMarkdown.id)),lease.id)
 const indexed=await command('epubProgress',{operation:'build'})
 assert.equal(indexed.formatState.progressPrecision,'indexed')
 await command('epubProgress',{operation:'seek',percentage:0})
 await frame.frameLocator('iframe[title="First chapter"]').getByText('EPUB sentinel one',{exact:true}).waitFor()
 await command('settings',{epub:{flow:'scrolled'}})
 await command('navigate',{location:{kind:'epub',cfi:selected.location.cfi}})
 await frame.frameLocator('iframe[title="Second chapter"]').getByText('EPUB sentinel two 中文😀.',{exact:true}).waitFor()
 const opens=await page.evaluate(()=>window.openRequests||0)
 await frame.locator('#viewport').press('Control+o')
 await page.waitForFunction(before=>window.openRequests>before,opens)
 let webkit
 if(process.argv.includes('--webkit')){
  const active=await page.evaluate(()=>testState.artifact.largeMarkdown)
  webkit=JSON.parse((await promisify(execFile)('/usr/bin/python3',[join(root,'scripts/smoke-large-markdown-webkit.py'),new URL(active.url,url).href,'--epub'],{env:environment,timeout:110000,maxBuffer:1000000})).stdout)
  assert.equal(webkit.epub,true);assert.equal(webkit.cleanup,true)
 }
 await page.evaluate(()=>controller.close({restoreMap:false}))
 const closeDeadline=Date.now()+5000
 while((await readdir(environment.READER_CACHE_DIR)).length&&Date.now()<closeDeadline)await pause(30)
 assert.deepEqual(await readdir(environment.READER_CACHE_DIR),[])
 assert.deepEqual(await readFile(file),original)
 assert.equal(requests.includes('/studio/review-epub'),false)
 assert.deepEqual(errors,[])
 console.log(JSON.stringify({ok:true,workspace:true,gateway:true,bookHashCompatible:true,readingStateReopened:true,typedCfiSelection:true,wholeBookSearchAndProgress:true,originalsRestored:true,opening:{...opening,hostFirstBookMs},webkit}))
}finally{
 await browser?.close()
 if(server.exitCode==null){server.kill('SIGTERM');await Promise.race([once(server,'exit'),pause(3000)]);if(server.exitCode==null)server.kill('SIGKILL')}
 await rm(directory,{recursive:true,force:true})
}
