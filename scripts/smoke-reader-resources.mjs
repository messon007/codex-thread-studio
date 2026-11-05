// Actual Studio workspace + public reader resource-policy adoption.
import assert from 'node:assert/strict'
import { spawn, execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { once } from 'node:events'
import { mkdir, mkdtemp, readFile, readdir, writeFile } from 'node:fs/promises'
import { resolve, join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { pdfFixture } from '../crates/marknoria/tests/browser/pdf-fixture.mjs'
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..'), output=join(root,'target/reader-resources-smoke')
await mkdir(output,{recursive:true})
const directory=await mkdtemp(join(output,'run-')), project=join(directory,'project'), shared=join(directory,'shared'), cache=join(directory,'reader-cache')
for(const path of [project,shared,cache])await mkdir(path)
const png=Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAIAAAABCAYAAAD0In+KAAAADklEQVR4nGP4z8AAQv8BD/kD/YURmXYAAAAASUVORK5CYII=','base64')
await writeFile(join(project,'project.png'),png);await writeFile(join(shared,'shared.png'),png)
const source=join(project,'resources.md'), sibling=join(project,'sibling.csv')
const original='# Resources\n\n![project](project.png) ![inline](data:image/png;base64,'+png.toString('base64')+')\n\n![shared](../shared/shared.png) [sibling](sibling.csv) [external](https://example.invalid/reference)\n\n'+'Bounded reading content.\n\n'.repeat(50000)
await writeFile(source,original);await writeFile(sibling,'Name,Value\nLinked document,42\n')
const commonFiles = [
 ['small.md','# Common sentinel 中文\n\n<img src="project.png" alt="raw resource">\n\n| Name | Value |\n|---|---|\n| sentinel | 42 |\n'],
 ['plain.txt','Common sentinel 中文 😀\r\nSecond line\r\n'],
 ['script.py','def sentinel():\n    return "中文 😀"\n'],
 ['main.c','int sentinel(void) { return 42; }\n'],
 ['main.cpp','auto sentinel = "中文";\n'],
 ['lib.rs','fn sentinel() -> usize { 42 }\n'],
 ['data.json','{"sentinel":"中文 😀","value":42}\n'],
 ['data.jsonl','{"sentinel":"中文"}\n{"sentinel":"second"}\n'],
 ['config.yaml','sentinel: 中文\nvalue: 42\n'],
 ['config.toml','sentinel = "中文"\nvalue = 42\n'],
 ['data.xml','<root><sentinel>中文</sentinel></root>\n'],
 ['page.html','<h1>Common sentinel 中文</h1><p>Readable content</p>\n'],
 ['config.ini','[sentinel]\nvalue=42\n']
]
if(process.argv.includes('--common-entries')) {
 for(const [path,text] of commonFiles)await writeFile(join(project,path),text)
 await writeFile(join(project,'book.pdf'),pdfFixture())
}
const environment={PATH:'/usr/bin:/bin',LANG:'C.UTF-8',XDG_CONFIG_HOME:join(directory,'config'),XDG_DATA_HOME:join(directory,'data'),XDG_CACHE_HOME:join(directory,'cache'),READER_CACHE_DIR:cache,READER_CHROMIUM:'/intentionally-unavailable-chromium'}
for(const key of ['XDG_RUNTIME_DIR','WAYLAND_DISPLAY','GDK_BACKEND','DBUS_SESSION_BUS_ADDRESS','WEBKIT_DISABLE_COMPOSITING_MODE'])if(process.env[key])environment[key]=process.env[key]
const child=spawn(process.env.STUDIO_REMOTE_BINARY || join(root,'target/release/codex-thread-studio'),['--serve','--listen','127.0.0.1:0'],{env:environment,stdio:['ignore','pipe','pipe']})
let log='',browser
child.stdout.on('data',bytes=>log=(log+bytes).slice(-100000));child.stderr.on('data',bytes=>log=(log+bytes).slice(-100000))
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms))
try{
 const deadline=Date.now()+30000
 while(!/http:\/\/127\.0\.0\.1:\d+\/#token=[a-f0-9]{32}/u.test(log)){if(Date.now()>deadline||child.exitCode!=null)throw Error('Resource fixture gateway did not start');await pause(30)}
 const url=new URL(log.match(/http:\/\/127\.0\.0\.1:\d+\/#token=[a-f0-9]{32}/u)[0]), token=new URLSearchParams(url.hash.slice(1)).get('token')
 const authorization={Authorization:'Bearer '+token}, jsonHeaders={...authorization,'Content-Type':'application/json'}
 const post=(path,body)=>fetch(new URL(path,url),{method:'POST',headers:jsonHeaders,body:JSON.stringify(body)})
 const preferences=await fetch(new URL('/studio/preferences',url),{headers:authorization}).then(response=>response.json())
 const grant=async enabled=>assert.equal((await fetch(new URL('/studio/preferences',url),{method:'PUT',headers:jsonHeaders,body:JSON.stringify({...preferences,sharedDocumentDirectories:enabled?[shared]:[]})})).status,200)
 const {chromium}=await import(process.env.STUDIO_PLAYWRIGHT_MODULE || 'playwright')
 browser=await chromium.launch({executablePath:process.env.STUDIO_CHROMIUM_PATH})
 const page=await browser.newPage({viewport:{width:1500,height:1000}}), errors=[]
 page.on('pageerror',error=>errors.push(error.message))
 await page.route(`${url.origin}/app.js`,route=>route.fulfill({contentType:'text/javascript',body:`
  import {createDocumentWorkspaceController,createDocumentWorkspaceState} from '/document-workspace-controller.mjs';
  window.testState=createDocumentWorkspaceState();window.externalLinks=[];window.fixtureErrors=[];
  const gatewayFetch=(path,options={})=>fetch(path,{...options,headers:{...options.headers,Authorization:'Bearer '+window.__CODEX_THREAD_STUDIO_GATEWAY__.token}});
  window.controller=createDocumentWorkspaceController({state:testState,gatewayFetch,randomId:()=>crypto.randomUUID(),reportError:error=>fixtureErrors.push(error.message),view:{
   selectedThread:()=>({cwd:${JSON.stringify(project)}}),selectedStateKey:()=>'resource-fixture',activateRightWorkspace:tool=>{testState.activeRightWorkspace=tool},
   hideComposerMenu(){},closeActionMenus(){},applyRightRailWidth(){},disconnectMarkdownImageObserver(){},disposeMarkdownImageAssets(){},hideSelection(){},setSelection(){},
   renderMarkdownDocument(){throw Error('Legacy document parser must not run')},openBrowserUrl:async value=>externalLinks.push(value)
  }});controller.bind();`}))
 await page.goto(url.href);await page.waitForFunction(()=>Boolean(window.controller))
 await page.evaluate(({root,path})=>controller.open({root,path},{allowDetachedRoot:true}),{root:project,path:source})
 await page.waitForFunction(()=>testState.artifact?.readerReady)
 const reader=await page.evaluate(()=>testState.artifact.largeMarkdown)
 const owner=await page.evaluate(async()=>{const {createReaderClient}=await import('/reader-assets/host-client.mjs');window.resourceClient=createReaderClient(document.querySelector('.artifact-large-markdown-frame'));return resourceClient.ready()})
 assert.equal(owner.document.references,5);assert.equal(owner.capabilities.referenceProfile.dynamicRoots,true)
 const frame=await page.locator('.artifact-large-markdown-frame').elementHandle().then(element=>element.contentFrame())
 await frame.locator('[data-image-reference="0"] [data-asset-state="ready"] img').waitFor()
 await frame.locator('[data-image-reference="1"] [data-asset-state="ready"] img').waitFor()
 const command=(method,params={})=>page.evaluate(({method,params})=>resourceClient.request(method,params),{method,params})
 const readyImage=async id=>{const until=Date.now()+30000;while(Date.now()<until){try{const value=await command('referenceImageInfo',{id});if(value.state==='ready')return value}catch(error){if(!/busy|preparing|budget/i.test(error.message))throw error}await pause(30)}throw Error('Resource did not become ready')}
 const denied=async(method,params)=>assert.match(await command(method,params).then(()=>'',error=>error.message),/\(403\)/)
 await denied('referenceImageInfo',{id:2})
 const endpoint=new URL(reader.url,url), scoped={'x-reader-token':endpoint.hash.slice(1)}
 const api=new URL('api/reference?id=0&revision='+owner.revision,endpoint)
 assert.equal((await fetch(api)).status,401)
 assert.equal((await fetch(api,{headers:{...scoped,Origin:'https://untrusted.invalid'}})).status,403)
 assert.equal((await fetch(api,{method:'POST',headers:scoped})).status,405)
 await command('setView',{view:'edit'})
 const editor=frame.locator('.cm-content[contenteditable="true"]');await editor.click();await editor.press('Control+Home');await page.keyboard.insertText('Studio policy draft.\n')
 await page.waitForFunction(()=>testState.artifact?.dirty)
 await grant(true);const image=await readyImage(2)
 await command('referenceImageFrame',{id:2,imageRevision:image.imageRevision,frame:0})
 await grant(false)
 await denied('referenceImageInfo',{id:2});await denied('referenceImageFrame',{id:2,imageRevision:image.imageRevision,frame:0})
 assert.equal((await command('state')).documentId,owner.documentId)
 assert.equal((await command('state')).dirty,true)
 assert.match(await editor.innerText(),/Studio policy draft/)
 await readyImage(0);await command('save')
 assert.match(await readFile(source,'utf8'),/^Studio policy draft\./)
 await grant(true);await command('setView',{view:'preview'})
 await frame.locator('[data-image-reference="2"] [data-asset-state="ready"] img').waitFor()
 await frame.locator('[data-link-reference="4"]').click()
 await page.waitForFunction(()=>externalLinks.length===1)
 assert.deepEqual(await page.evaluate(()=>externalLinks),['https://example.invalid/reference'])
 let webkit
 if(process.argv.includes('--webkit')){
  const response=await post('/studio/reader/open',{root:project,path:source});assert.equal(response.status,200)
  const extra=await response.json(), extraUrl=new URL(extra.reader.url,url)
  try{webkit=JSON.parse((await promisify(execFile)('/usr/bin/python3',[join(root,'scripts/smoke-reader-resources-webkit.py'),extraUrl.href],{env:environment,timeout:110000})).stdout);assert.equal(webkit.resources,true)}
  finally{assert.equal((await post('/studio/markdown/close',{id:extra.reader.id})).status,200)}
 }
 await frame.locator('[data-link-reference="3"]').click()
 await page.waitForFunction(path=>testState.artifact?.path===path&&testState.artifact.readerReady,sibling)
 assert.equal(await page.locator('#artifact-title').textContent(),'sibling.csv')
 const linkedFrame=await page.locator('.artifact-large-markdown-frame').elementHandle().then(element=>element.contentFrame())
 await linkedFrame.getByText('Linked document',{exact:true}).waitFor()
 const commonEntries=[]
 if(process.argv.includes('--common-entries')) {
  for(const [name,text] of [...commonFiles,['project.png',null],['book.pdf',null]]) {
   const started=Date.now(), path=join(project,name)
   await page.evaluate(({root,path})=>controller.open({root,path},{allowDetachedRoot:true}),{root:project,path})
   await page.waitForFunction(()=>testState.artifact?.readerReady&&testState.artifact.readerReadingReady)
   const entry=await page.evaluate(async()=>{const {createReaderClient}=await import('/reader-assets/host-client.mjs');window.commonClient=createReaderClient(document.querySelector('.artifact-large-markdown-frame'));return commonClient.ready()})
   assert.equal(entry.view,'preview');assert.equal(entry.capabilities.notes,true)
   await page.locator('#artifact-notes-tools button[aria-label="Notes"]').waitFor()
   const request=(method,params={})=>page.evaluate(({method,params})=>commonClient.request(method,params),{method,params})
   assert.equal(entry.capabilities.source,text!==null)
   if(text!==null) {
    assert.equal((await request('sourceWindow',{line:1})).result.text,text)
    assert.equal((await request('setView',{view:'source'})).view,'source')
    assert.equal((await request('setView',{view:'preview'})).view,'preview')
   }
   if(name==='small.md') {
    const commonFrame=await page.locator('.artifact-large-markdown-frame').elementHandle().then(element=>element.contentFrame())
    await commonFrame.locator('[data-image-reference="0"] [data-asset-state="ready"] img').waitFor()
    assert.equal((await request('tableInfo',{table:0})).rows,1)
   }
   if(name==='book.pdf') {
    await request('settings',{pdf:{zoom:1,continuous:false}})
    await request('navigate',{location:{kind:'pdf',pages:[{page:7,rects:[{x:0,y:0,width:1,height:0.000001}]}]}})
    await page.waitForFunction(()=>testState.artifact.page===7&&testState.artifact.pdfPresentation?.zoom===1)
    await page.evaluate(({root,path})=>controller.open({root,path},{allowDetachedRoot:true,viewState:{page:7,pdfPresentation:{zoom:1,continuous:false}}}),{root:project,path})
    await page.waitForFunction(()=>testState.artifact?.readerReady&&testState.artifact.page===7&&testState.artifact.pdfPresentation?.zoom===1&&testState.artifact.pdfPresentation?.continuous===false)
   }
   let entryWebkit
   if(process.argv.includes('--webkit')) {
    const response=await post('/studio/reader/open',{root:project,path});assert.equal(response.status,200)
    const extra=await response.json()
    try {
     entryWebkit=JSON.parse((await promisify(execFile)('/usr/bin/python3',[join(root,'scripts/smoke-reader-entry-webkit.py'),new URL(extra.reader.url,url).href,JSON.stringify({name,text})],{env:environment,timeout:110000})).stdout)
     assert.equal(entryWebkit.commonEntry,true)
    } finally { assert.equal((await post('/studio/markdown/close',{id:extra.reader.id})).status,200) }
   }
   commonEntries.push({name,entryCheckMs:Date.now()-started,webkit:entryWebkit})
  }
  for(const [path,text] of commonFiles)assert.equal(await readFile(join(project,path),'utf8'),text)
 }
 assert.equal(await page.evaluate(()=>controller.close({restoreMap:false})),true)
 const until=Date.now()+10000;while((await readdir(cache)).length&&Date.now()<until)await pause(30)
 assert.deepEqual(await readdir(cache),[]);assert.deepEqual(errors,[]);assert.deepEqual(await page.evaluate(()=>fixtureErrors),[])
 assert.match(await readFile(source,'utf8'),/^Studio policy draft\.\n# Resources/)
 console.log(JSON.stringify({ok:true,actualStudioWorkspace:true,publicResources:true,dynamicRevocation:true,draftPreservedAndSaved:true,hostLinks:true,commonEntries,webkit,browserVersion:browser.version(),artifacts:directory}))
}finally{
 await browser?.close()
 if(child.exitCode==null&&child.signalCode==null){const exited=once(child,'exit');child.kill('SIGTERM');await exited}
}
