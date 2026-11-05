// Actual Studio app + public SDK: regressions for #38/#52/#54 and attachment #53.
import assert from 'node:assert/strict'
import {spawn,execFile} from 'node:child_process'
import {promisify} from 'node:util'
import {once} from 'node:events'
import {mkdir,mkdtemp,readFile,readdir,writeFile} from 'node:fs/promises'
import {join,dirname,resolve} from 'node:path'
import {fileURLToPath} from 'node:url'
import {createHash} from 'node:crypto'
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..'),output=join(root,'target/reader-issue-regressions')
await mkdir(output,{recursive:true});const dir=await mkdtemp(join(output,'run-')),project=join(dir,'project'),cache=join(dir,'reader-cache')
await mkdir(project);await mkdir(cache)
const count=50000, needle='DCDC-1297522-CURRENT'
const cases=[
 ['large.xml','<root>'+('<item>中文😀 '+ 'bounded '.repeat(12)+'</item>').repeat(65000)+'</root>'],
 ['large.json',JSON.stringify({items:Array.from({length:count},(_,id)=>({id,name:'中文😀',value:id}))})],
 ['large.jsonl',Array.from({length:count},(_,id)=>JSON.stringify({id,name:'中文😀',active:id%2===0})+'\r\n').join('')],
 ['long-record.jsonl',JSON.stringify({id:7,text:'中文😀'.repeat(90000)})+'\r\n'],
 ['table.md',`# Filters\n\n| Name | Value | Signal |\n|---|---|---|\n| equal | 1 | ${needle} |\n| contains | 2 | prefix ${needle} suffix |\n| other | 3 | other |\n`],
]
for(const [name,text]of cases)await writeFile(join(project,name),text)
assert.ok(Buffer.byteLength(cases[0][1])>5*1024*1024)
const hash=bytes=>createHash('sha256').update(bytes).digest('hex')
const originals=new Map(cases.map(([name,text])=>[name,hash(Buffer.from(text))]))
const environment={PATH:'/usr/bin:/bin',LANG:'C.UTF-8',CODEX_HOME:join(dir,'codex'),XDG_CONFIG_HOME:join(dir,'config'),XDG_DATA_HOME:join(dir,'data'),XDG_CACHE_HOME:join(dir,'cache'),READER_CACHE_DIR:cache}
const server=spawn(process.env.STUDIO_REMOTE_BINARY||join(root,'target/release/codex-thread-studio'),['--serve','--listen','127.0.0.1:0'],{env:environment,stdio:['ignore','pipe','pipe']})
let log='',browser;for(const stream of [server.stdout,server.stderr])stream.on('data',bytes=>{log=(log+bytes).slice(-100000)})
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms)),results=[]
try{
 const until=Date.now()+30000
 while(!/http:\/\/127\.0\.0\.1:\d+\/#token=[a-f0-9]{32}/u.test(log)){assert.ok(Date.now()<until&&server.exitCode===null,'Studio failed to start');await pause(30)}
 const url=log.match(/http:\/\/127\.0\.0\.1:\d+\/#token=[a-f0-9]{32}/u)[0]
 const {chromium}=await import(process.env.STUDIO_PLAYWRIGHT_MODULE||'playwright')
 browser=await chromium.launch({executablePath:process.env.STUDIO_CHROMIUM_PATH})
 const page=await browser.newPage({viewport:{width:1440,height:1000}}),errors=[]
 page.on('pageerror',error=>errors.push(error.message))
 await page.goto(url);await page.waitForFunction(()=>Boolean(window.__studioDeveloper))
 const command=(method,params={})=>page.evaluate(({method,params})=>issueClient.request(method,params),{method,params})
 for(const [name,text]of cases){
  await page.evaluate(async()=>{if(window.issueClient){await issueClient.request('setView',{view:'source'});issueClient.disconnect()}})
  await page.evaluate(({root,path})=>__studioDeveloper.openArtifact({root,path}),{root:project,path:join(project,name)})
  await page.waitForFunction(name=>document.querySelector('#artifact-title')?.textContent.trim()===name&&document.querySelector('.artifact-large-markdown-frame')&&!document.querySelector('#artifact-preview').disabled,name)
  const owner=await page.evaluate(async()=>{const {createReaderClient}=await import('/reader-assets/host-client.mjs');window.issueClient=createReaderClient(document.querySelector('.artifact-large-markdown-frame'));return issueClient.ready()})
  assert.equal(owner.view,'preview');assert.equal(owner.capabilities.source,true)
  const previewFrame=await page.locator('.artifact-large-markdown-frame').elementHandle().then(e=>e.contentFrame())
  if(name!=='table.md')await previewFrame.locator('pre[data-document-code]').first().waitFor({state:'visible'})
  const source=(await command('sourceWindow',{line:1})).result
  assert.deepEqual(Buffer.from(source.text),Buffer.from(text).subarray(source.start,source.end))
  assert.equal((await command('setView',{view:'source'})).view,'source')
  assert.equal((await command('setView',{view:'preview'})).view,'preview')
  if(name.endsWith('.xml'))assert.equal(await page.locator('#artifact-query-tools button').count(),0)
  if(name.endsWith('.json')||name.endsWith('.jsonl')){
   await page.locator('#artifact-query-tools button[aria-label="jq"]').click()
   const dialog=page.locator('.jq-query-dialog')
   await dialog.waitFor()
   const bounds=await dialog.boundingBox(),viewport=page.viewportSize();assert.ok(bounds.x>=0&&bounds.y>=0&&bounds.x+bounds.width<=viewport.width+1&&bounds.y+bounds.height<=viewport.height+1)
   const filter=name==='large.json'?'.items | length':name==='large.jsonl'?`select(.id == ${count-1}) | .id`:'.id'
   const expected=name==='large.json'?String(count):name==='large.jsonl'?String(count-1):'7'
   await dialog.getByRole('textbox',{name:'jq filter',exact:true}).fill(filter)
   await dialog.getByRole('button',{name:'Run filter',exact:true}).click()
   await page.waitForFunction(value=>document.querySelector('.jq-results')?.textContent.trim()===value,expected)
   await dialog.getByRole('button',{name:'Close',exact:true}).click()
   assert.equal((await command('state')).documentId,owner.documentId)
  }
  if(name==='table.md'){
   assert.equal(await page.locator('#artifact-query-tools button').count(),0)
   const frame=await page.locator('.artifact-large-markdown-frame').elementHandle().then(e=>e.contentFrame())
   await frame.locator('.table-query-open').click()
   const dialog=frame.locator('.table-query-dialog');await frame.waitForFunction(()=>!document.querySelector('.table-query-run')?.disabled)
   await dialog.locator('summary').filter({hasText:'Columns and query help'}).click()
   assert.match(await dialog.locator('.table-query-schema').textContent(),/single quotes/)
   const oldId=await dialog.locator('.table-content').getAttribute('data-query-id')
   await dialog.getByRole('textbox',{name:'SQL query',exact:true}).fill(`SELECT * FROM t where "${needle}" in c3`)
   await dialog.getByRole('button',{name:'Run query',exact:true}).click()
   await frame.waitForFunction(()=>document.querySelector('.table-query-actions [role="status"]')?.textContent.includes('no such table: c3'))
   assert.equal(await dialog.locator('.table-content').getAttribute('data-query-id'),oldId)
   for(const [sql,expected]of [[`SELECT c1 FROM t WHERE c3 LIKE '%${needle}%'`,['equal','contains']],[`SELECT c1 FROM t WHERE instr(c3, '${needle}') > 0`,['equal','contains']],[`SELECT c1 FROM t WHERE c3 IN ('${needle}', 'other')`,['equal','other']]]){
    const before=await dialog.locator('.table-content').getAttribute('data-query-id')
    await dialog.getByRole('textbox',{name:'SQL query',exact:true}).fill(sql);await dialog.getByRole('button',{name:'Run query',exact:true}).click()
    await frame.waitForFunction(id=>document.querySelector('.table-content')?.dataset.queryId!==id&&!document.querySelector('.table-query-run')?.disabled,before)
    assert.deepEqual(await dialog.locator('tbody tr td').allTextContents(),expected)
   }
   await dialog.getByRole('button',{name:'Close',exact:true}).click()
  }
  results.push({name,bytes:Buffer.byteLength(text),preview:true,source:true,jq:/\.jsonl?$/.test(name)})
 }
 await page.evaluate(()=>issueClient.disconnect());await page.locator('#close-artifact').click()
 // Invoke the unchanged production rendering function for a layout-only fixture.
 const app=await readFile(join(root,'ui/app.js'),'utf8'),start=app.indexOf('function renderComposerImages()'),end=app.indexOf('\nasync function addComposerImages',start)
 const render=app.slice(start,end)
 const markup={dataset:{},classList:{toggle(){}},innerHTML:''}
 const imageState={pendingImages:{fixture:[{id:'fixture',name:'image.png',size:64,url:'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAIAAAABCAYAAAD0In+KAAAADklEQVR4nGP4z8AAQv8BD/kD/YURmXYAAAAASUVORK5CYII='}]}}
 Function('state','selectedStateKey','$','escapeHtml','formatImageSize','t',render+';renderComposerImages()')(imageState,()=> 'fixture',()=>markup,s=>s,()=> '64 B',s=>s)
 await page.evaluate(html=>{
  const controls=document.querySelector('#composer-images')
  for(let node=controls;node&&node!==document.body;node=node.parentElement){node.classList.remove('hidden');node.hidden=false}
  controls.innerHTML=html
 },markup.innerHTML)
 const remove=page.locator('[data-remove-composer-image="fixture"]')
 await remove.waitFor({state:'visible'})
 const geometry=await remove.evaluate(button=>{const b=button.getBoundingClientRect(),s=button.querySelector('svg').getBoundingClientRect(),style=getComputedStyle(button);return {dx:Math.abs(b.x+b.width/2-s.x-s.width/2),dy:Math.abs(b.y+b.height/2-s.y-s.height/2),padding:style.padding,border:style.borderTopWidth,radius:style.borderRadius}})
 assert.ok(geometry.dx<1&&geometry.dy<1);assert.equal(geometry.padding,'0px');assert.equal(geometry.border,'0px');assert.equal(geometry.radius,'5px')
 await page.screenshot({path:join(dir,'attachment-remove.png')})
 let webkit
 if(process.argv.includes('--webkit')){
  const config={project,attachmentMarkup:markup.innerHTML,cases:cases.filter(([name])=>name!=='table.md').map(([name,text])=>({name,prefix:text.slice(0,24),filter:name==='large.json'?'.items | length':name==='large.jsonl'?`select(.id == ${count-1}) | .id`:name==='long-record.jsonl'?'.id':null,expected:name==='large.json'?String(count):name==='large.jsonl'?String(count-1):'7'}))}
  const run=await promisify(execFile)('/usr/bin/python3',[join(root,'scripts/smoke-reader-issue-regressions-webkit.py'),url,JSON.stringify(config)],{timeout:175000,maxBuffer:1000000})
  webkit=JSON.parse(run.stdout.split('\n').findLast(line=>line.startsWith('{"ok"')));assert.equal(webkit.ok,true)
 }
 const deadline=Date.now()+10000;while((await readdir(cache)).length&&Date.now()<deadline)await pause(30)
 assert.deepEqual(await readdir(cache),[]);assert.deepEqual(errors,[])
 for(const [name]of cases)assert.equal(hash(await readFile(join(project,name))),originals.get(name))
 console.log(JSON.stringify({ok:true,actualStudioApp:true,publicReaderProtocol:true,results,sqlErrorAndFilters:true,attachmentGeometry:geometry,webkit,cleanup:true,artifacts:dir}))
}finally{
 await browser?.close()
 if(server.exitCode===null&&server.signalCode===null){const exited=once(server,'exit');server.kill('SIGTERM');await Promise.race([exited,pause(5000)]);if(server.exitCode===null)server.kill('SIGKILL')}
}
